use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::validation::ManifestValidationError;
use crate::image::tag::ImageTagSpecification;

/// Root declarative manifest configuring all instances, flavors, and storage boundaries for `onehost`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnehostManifest {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub version: String,
    pub storage: StorageDirectoriesConfiguration,
    #[serde(default)]
    pub flavors: HashMap<String, FlavorConfiguration>,
    #[serde(default)]
    pub instances: HashMap<String, InstanceConfiguration>,
}

impl OnehostManifest {
    /// Constructs and validates a new manifest instance, guaranteeing it is born valid.
    pub fn new(
        version: impl Into<String>,
        storage: StorageDirectoriesConfiguration,
        flavors: HashMap<String, FlavorConfiguration>,
        instances: HashMap<String, InstanceConfiguration>,
    ) -> Result<Self, ManifestValidationError> {
        let manifest = Self {
            schema: None,
            version: version.into(),
            storage,
            flavors,
            instances,
        };
        crate::config::validation::validate_manifest(&manifest)?;
        Ok(manifest)
    }
}

/// Global storage directories and pool boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageDirectoriesConfiguration {
    pub depot_store_dir: PathBuf,
    pub depot_iso_dir: PathBuf,
    pub depot_backup_dir: PathBuf,
    pub nvram_dir: PathBuf,
    pub nvram_template: PathBuf,
    pub ovmf_code: PathBuf,
}

impl StorageDirectoriesConfiguration {
    pub fn new(
        depot_store_dir: PathBuf,
        depot_iso_dir: PathBuf,
        depot_backup_dir: PathBuf,
        nvram_dir: PathBuf,
        nvram_template: PathBuf,
        ovmf_code: PathBuf,
    ) -> Self {
        Self {
            depot_store_dir,
            depot_iso_dir,
            depot_backup_dir,
            nvram_dir,
            nvram_template,
            ovmf_code,
        }
    }
}

/// Declarative configuration for an OEMDRV base image flavor derivation in Nix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlavorConfiguration {
    pub oemdrv_path: PathBuf,
    #[serde(default)]
    pub extra_isos: HashMap<String, PathBuf>,
}

impl FlavorConfiguration {
    pub fn new(oemdrv_path: impl Into<PathBuf>, extra_isos: HashMap<String, PathBuf>) -> Self {
        Self {
            oemdrv_path: oemdrv_path.into(),
            extra_isos,
        }
    }

    pub fn new_simple(oemdrv_path: impl Into<PathBuf>) -> Self {
        Self::new(oemdrv_path, HashMap::new())
    }
}

/// A strongly typed, born-valid RFC-4122 instance UUID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceUuid(String);

impl InstanceUuid {
    /// Validates and constructs an `InstanceUuid`.
    pub fn parse(raw_uuid: &str) -> Result<Self, ManifestValidationError> {
        if Self::is_valid_rfc4122(raw_uuid) {
            Ok(Self(raw_uuid.to_string()))
        } else {
            Err(ManifestValidationError::InvalidInstanceUuid {
                instance_name: String::new(),
                raw_uuid: raw_uuid.to_string(),
            })
        }
    }

    /// Validates if a raw string conforms to standard RFC-4122 8-4-4-4-12 hex syntax.
    pub fn is_valid_rfc4122(raw_uuid: &str) -> bool {
        let uuid_segments: Vec<&str> = raw_uuid.split('-').collect();
        uuid_segments.len() == 5
            && uuid_segments[0].len() == 8
            && uuid_segments[1].len() == 4
            && uuid_segments[2].len() == 4
            && uuid_segments[3].len() == 4
            && uuid_segments[4].len() == 12
            && uuid_segments.iter().all(|segment| {
                segment
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
            })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InstanceUuid {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::str::FromStr for InstanceUuid {
    type Err = ManifestValidationError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

impl std::ops::Deref for InstanceUuid {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for InstanceUuid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for InstanceUuid {
    type Error = ManifestValidationError;

    fn try_from(raw_uuid: &str) -> Result<Self, Self::Error> {
        Self::parse(raw_uuid)
    }
}

impl TryFrom<String> for InstanceUuid {
    type Error = ManifestValidationError;

    fn try_from(raw_uuid: String) -> Result<Self, Self::Error> {
        Self::parse(&raw_uuid)
    }
}

impl PartialEq<&str> for InstanceUuid {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<InstanceUuid> for &str {
    fn eq(&self, other: &InstanceUuid) -> bool {
        *self == other.0
    }
}

impl PartialEq<str> for InstanceUuid {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<InstanceUuid> for str {
    fn eq(&self, other: &InstanceUuid) -> bool {
        self == other.0
    }
}

impl PartialEq<String> for InstanceUuid {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<InstanceUuid> for String {
    fn eq(&self, other: &InstanceUuid) -> bool {
        other == self
    }
}

/// Declarative instance configuration specifying hardware template, base image, and explicit storage pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConfiguration {
    pub uuid: InstanceUuid,
    pub template_xml: PathBuf,
    pub image: ImageTagSpecification,
    pub pool: String,
}

impl InstanceConfiguration {
    pub fn new(
        uuid: InstanceUuid,
        template_xml: PathBuf,
        image: ImageTagSpecification,
        pool: impl Into<String>,
    ) -> Self {
        Self {
            uuid,
            template_xml,
            image,
            pool: pool.into(),
        }
    }

    pub fn with_pool(self, pool: impl Into<String>) -> Self {
        Self {
            pool: pool.into(),
            ..self
        }
    }

    /// Resolves the declared storage pool to use.
    /// Returns an error if the pool is empty (zero implicit defaults).
    pub fn effective_pool<'a>(&'a self, instance_name: &str) -> Result<&'a str, ManifestValidationError> {
        let trimmed = self.pool.trim();
        if trimmed.is_empty() {
            Err(ManifestValidationError::MissingStoragePool {
                instance_name: instance_name.to_string(),
            })
        } else {
            Ok(trimmed)
        }
    }
}
