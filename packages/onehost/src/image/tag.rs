use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

/// Enumerates errors that can occur while parsing an image tag specification.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImageTagParseError {
    #[error(
        "Invalid image tag format '{raw_tag}': expected exactly {expected_segments} segments separated by '/', but found {actual_segments}"
    )]
    InvalidSegmentCount {
        raw_tag: String,
        expected_segments: usize,
        actual_segments: usize,
    },

    #[error("Invalid image tag format '{raw_tag}': segment at index {segment_index} is empty")]
    EmptySegment {
        raw_tag: String,
        segment_index: usize,
    },

    #[error("Invalid image tag format '{raw_tag}': UUP ID '{uup_id}' is not a valid RFC-4122 UUID")]
    InvalidUupId { raw_tag: String, uup_id: String },

    #[error("Invalid image tag format '{raw_tag}': language '{language}' is not a valid BCP-47 language tag")]
    InvalidLanguage { raw_tag: String, language: String },
}

/// Enumerates errors that can occur during content-addressed flavor resolution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlavorResolutionError {
    #[error(
        "Unknown base image flavor '{requested_flavor}'. Available flavors: {available_flavors:?}"
    )]
    UnknownFlavor {
        requested_flavor: String,
        available_flavors: Vec<String>,
    },

    #[error("Nix store path for flavor must be an absolute path, but got relative path: {path}")]
    RelativeNixStorePathNotAllowed { path: PathBuf },

    #[error("Content hash for flavor derivation cannot be empty")]
    EmptyContentHash,
}

/// Helper function to validate if a string conforms to RFC-4122 8-4-4-4-12 hex syntax.
pub fn is_valid_rfc4122(raw_uuid: &str) -> bool {
    crate::config::model::InstanceUuid::is_valid_rfc4122(raw_uuid)
}

/// Helper function to validate if a string conforms to BCP-47 language tag syntax.
pub fn is_valid_bcp47(tag: &str) -> bool {
    let subtags: Vec<&str> = tag.split('-').collect();
    if subtags.is_empty() {
        return false;
    }
    let primary = subtags[0];
    if !(primary.len() == 2 || primary.len() == 3)
        || !primary.chars().all(|character| character.is_ascii_alphabetic())
    {
        return false;
    }
    subtags.iter().skip(1).all(|subtag| {
        subtag.len() >= 2
            && subtag.len() <= 8
            && subtag.chars().all(|character| character.is_ascii_alphanumeric())
    })
}

/// Canonical Release Tuple and flavor specification: `<uup_id>/<edition>/<language>/<flavor>`.
/// Example: `ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageTagSpecification {
    pub uup_id: String,
    pub edition: String,
    pub language: String,
    pub flavor: String,
}

impl ImageTagSpecification {
    /// Constructs a new `ImageTagSpecification` from verified segments.
    pub fn new(
        uup_id: impl Into<String>,
        edition: impl Into<String>,
        language: impl Into<String>,
        flavor: impl Into<String>,
    ) -> Result<Self, ImageTagParseError> {
        let uup_id = uup_id.into().trim().to_ascii_lowercase();
        let edition = edition.into().trim().to_ascii_lowercase();
        let language = language.into().trim().to_ascii_lowercase();
        let flavor = flavor.into().trim().to_string();

        let raw_tag = format!("{uup_id}/{edition}/{language}/{flavor}");

        if uup_id.is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 0,
            })
        } else if !is_valid_rfc4122(&uup_id) {
            Err(ImageTagParseError::InvalidUupId { raw_tag, uup_id })
        } else if edition.is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 1,
            })
        } else if language.is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 2,
            })
        } else if !is_valid_bcp47(&language) {
            Err(ImageTagParseError::InvalidLanguage { raw_tag, language })
        } else if flavor.is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 3,
            })
        } else {
            Ok(Self {
                uup_id,
                edition,
                language,
                flavor,
            })
        }
    }

    /// Canonical Release Tuple `<uup_id>-<edition>-<language>` without flavor or content hash.
    pub fn release_tuple(&self) -> String {
        format!("{}-{}-{}", self.uup_id, self.edition, self.language)
    }
}

impl FromStr for ImageTagSpecification {
    type Err = ImageTagParseError;

    fn from_str(raw_tag: &str) -> Result<Self, Self::Err> {
        let segments: Vec<&str> = raw_tag.split('/').collect();

        if segments.len() != 4 {
            Err(ImageTagParseError::InvalidSegmentCount {
                raw_tag: raw_tag.to_string(),
                expected_segments: 4,
                actual_segments: segments.len(),
            })
        } else {
            Self::new(segments[0], segments[1], segments[2], segments[3])
        }
    }
}

impl fmt::Display for ImageTagSpecification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}/{}/{}",
            self.uup_id, self.edition, self.language, self.flavor
        )
    }
}

#[derive(Serialize)]
struct StructuredImageTagRef<'a> {
    uup_id: &'a str,
    edition: &'a str,
    language: &'a str,
    flavor: &'a str,
}

impl Serialize for ImageTagSpecification {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        StructuredImageTagRef {
            uup_id: &self.uup_id,
            edition: &self.edition,
            language: &self.language,
            flavor: &self.flavor,
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredImageTag {
    uup_id: String,
    edition: String,
    language: String,
    flavor: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ImageTagRepresentation {
    Structured(StructuredImageTag),
    String(String),
}

impl<'de> Deserialize<'de> for ImageTagSpecification {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let representation = ImageTagRepresentation::deserialize(deserializer)?;
        match representation {
            ImageTagRepresentation::String(tag_str) => {
                Self::from_str(&tag_str).map_err(serde::de::Error::custom)
            }
            ImageTagRepresentation::Structured(StructuredImageTag {
                uup_id,
                edition,
                language,
                flavor,
            }) => Self::new(uup_id, edition, language, flavor).map_err(serde::de::Error::custom),
        }
    }
}

/// Represents declarative metadata describing a specific base image flavor's OEMDRV derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlavorDerivationMetadata {
    pub oemdrv_nix_store_path: PathBuf,
    pub content_hash: String,
}

impl FlavorDerivationMetadata {
    /// Creates and validates a new `FlavorDerivationMetadata` instance.
    pub fn new(
        oemdrv_nix_store_path: PathBuf,
        content_hash: String,
    ) -> Result<Self, FlavorResolutionError> {
        if !oemdrv_nix_store_path.is_absolute() {
            Err(FlavorResolutionError::RelativeNixStorePathNotAllowed {
                path: oemdrv_nix_store_path,
            })
        } else if content_hash.trim().is_empty() {
            Err(FlavorResolutionError::EmptyContentHash)
        } else {
            Ok(Self {
                oemdrv_nix_store_path,
                content_hash,
            })
        }
    }
}

/// Utility for resolving image tags and flavor metadata into concrete filesystem artifact paths.
pub struct ContentAddressedImageResolver;

impl ContentAddressedImageResolver {
    /// Look up the `FlavorDerivationMetadata` corresponding to an `ImageTagSpecification`
    /// from a registry of available flavors.
    pub fn lookup_flavor<'a>(
        image_tag: &ImageTagSpecification,
        flavor_registry: &'a HashMap<String, FlavorDerivationMetadata>,
    ) -> Result<&'a FlavorDerivationMetadata, FlavorResolutionError> {
        flavor_registry.get(&image_tag.flavor).ok_or_else(|| {
            let available_flavors: Vec<String> = flavor_registry
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            FlavorResolutionError::UnknownFlavor {
                requested_flavor: image_tag.flavor.clone(),
                available_flavors,
            }
        })
    }

    /// Generates the canonical filename for a golden master QCOW2 image:
    /// `${uup_id}-${edition}-${language}-${flavor}-${content_hash}.qcow2`
    pub fn resolve_golden_master_filename(
        image_tag: &ImageTagSpecification,
        flavor_metadata: &FlavorDerivationMetadata,
    ) -> String {
        Self::resolve_base_image_filename(image_tag, &flavor_metadata.content_hash)
    }

    /// Generates the canonical base image filename from an image tag and content hash:
    /// `${uup_id}-${edition}-${language}-${flavor}-${content_hash}.qcow2`
    pub fn resolve_base_image_filename(
        image_tag: &ImageTagSpecification,
        content_hash: &str,
    ) -> String {
        format!(
            "{}-{}-{}.qcow2",
            image_tag.release_tuple(),
            image_tag.flavor,
            content_hash
        )
    }

    /// Generates the canonical filename for a raw installation ISO:
    /// `${uup_id}-${edition}-${language}.iso`
    pub fn resolve_iso_filename(image_tag: &ImageTagSpecification) -> String {
        format!("{}.iso", image_tag.release_tuple())
    }

    /// Resolves the absolute path to the raw installation ISO in the depot ISO directory:
    /// `<depot_iso_directory>/${iso_filename}`
    pub fn resolve_depot_iso_path(
        depot_iso_directory: &Path,
        image_tag: &ImageTagSpecification,
    ) -> PathBuf {
        depot_iso_directory.join(Self::resolve_iso_filename(image_tag))
    }

    /// Resolves the absolute path to the golden master in the depot store directory:
    /// `<depot_store_directory>/${master_filename}`
    pub fn resolve_depot_master_path(
        depot_store_directory: &Path,
        image_tag: &ImageTagSpecification,
        flavor_metadata: &FlavorDerivationMetadata,
    ) -> PathBuf {
        let filename = Self::resolve_golden_master_filename(image_tag, flavor_metadata);
        depot_store_directory.join(filename)
    }

    /// Resolves the absolute path to the cached base image in the local storage pool:
    /// `<storage_pool_directory>/${master_filename}`
    pub fn resolve_storage_pool_base_path(
        storage_pool_directory: &Path,
        image_tag: &ImageTagSpecification,
        flavor_metadata: &FlavorDerivationMetadata,
    ) -> PathBuf {
        let filename = Self::resolve_golden_master_filename(image_tag, flavor_metadata);
        storage_pool_directory.join(filename)
    }

    /// Resolves the absolute path to an instance's volatile CoW overlay disk:
    /// `<storage_pool_directory>/<instance_name>.qcow2`
    pub fn resolve_instance_overlay_path(
        storage_pool_directory: &Path,
        instance_name: &str,
    ) -> PathBuf {
        storage_pool_directory.join(format!("{instance_name}.qcow2"))
    }
}
