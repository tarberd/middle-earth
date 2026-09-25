use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

    #[error(
        "Invalid image tag format '{raw_tag}': segment at index {segment_index} is empty"
    )]
    EmptySegment {
        raw_tag: String,
        segment_index: usize,
    },
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

    #[error(
        "Nix store path for flavor must be an absolute path, but got relative path: {path}"
    )]
    RelativeNixStorePathNotAllowed { path: PathBuf },

    #[error("Content hash for flavor derivation cannot be empty")]
    EmptyContentHash,
}

/// Represents a parsed image tag specification: `<operating_system>/<build_version>/<flavor_name>`.
/// Example: `win11/26300.9457.pro.en-us/looking-glass`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageTagSpecification {
    pub operating_system: String,
    pub build_version: String,
    pub flavor_name: String,
}

impl ImageTagSpecification {
    /// Constructs a new `ImageTagSpecification` from verified segments.
    pub fn new(
        operating_system: impl Into<String>,
        build_version: impl Into<String>,
        flavor_name: impl Into<String>,
    ) -> Result<Self, ImageTagParseError> {
        let operating_system = operating_system.into();
        let build_version = build_version.into();
        let flavor_name = flavor_name.into();

        let raw_tag = format!("{operating_system}/{build_version}/{flavor_name}");

        if operating_system.trim().is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 0,
            })
        } else if build_version.trim().is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 1,
            })
        } else if flavor_name.trim().is_empty() {
            Err(ImageTagParseError::EmptySegment {
                raw_tag,
                segment_index: 2,
            })
        } else {
            Ok(Self {
                operating_system,
                build_version,
                flavor_name,
            })
        }
    }
}

impl FromStr for ImageTagSpecification {
    type Err = ImageTagParseError;

    fn from_str(raw_tag: &str) -> Result<Self, Self::Err> {
        let segments: Vec<&str> = raw_tag.split('/').collect();

        if segments.len() != 3 {
            Err(ImageTagParseError::InvalidSegmentCount {
                raw_tag: raw_tag.to_string(),
                expected_segments: 3,
                actual_segments: segments.len(),
            })
        } else {
            segments
                .iter()
                .enumerate()
                .try_for_each(|(segment_index, segment)| {
                    if segment.trim().is_empty() {
                        Err(ImageTagParseError::EmptySegment {
                            raw_tag: raw_tag.to_string(),
                            segment_index,
                        })
                    } else {
                        Ok(())
                    }
                })?;

            Ok(Self {
                operating_system: segments[0].to_string(),
                build_version: segments[1].to_string(),
                flavor_name: segments[2].to_string(),
            })
        }
    }
}

impl fmt::Display for ImageTagSpecification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}/{}",
            self.operating_system, self.build_version, self.flavor_name
        )
    }
}

impl Serialize for ImageTagSpecification {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ImageTagSpecification {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unparsed_tag = String::deserialize(deserializer)?;
        Self::from_str(&unparsed_tag).map_err(serde::de::Error::custom)
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
        flavor_registry
            .get(&image_tag.flavor_name)
            .ok_or_else(|| {
                let available_flavors: Vec<String> = flavor_registry
                    .keys()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                FlavorResolutionError::UnknownFlavor {
                    requested_flavor: image_tag.flavor_name.clone(),
                    available_flavors,
                }
            })
    }

    /// Generates the canonical filename for a golden master QCOW2 image:
    /// `${operating_system}-${build_version}-${flavor_name}-${content_hash}.qcow2`
    pub fn resolve_golden_master_filename(
        image_tag: &ImageTagSpecification,
        flavor_metadata: &FlavorDerivationMetadata,
    ) -> String {
        format!(
            "{}-{}-{}-{}.qcow2",
            image_tag.operating_system,
            image_tag.build_version,
            image_tag.flavor_name,
            flavor_metadata.content_hash
        )
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
