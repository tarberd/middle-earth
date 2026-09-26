pub mod engine;
pub mod guard;
pub mod model;
pub mod restore;

use std::path::PathBuf;
use thiserror::Error;

use crate::config::validation::ManifestValidationError;
use crate::domain::element::DomainXmlParseError;
use crate::hypervisor::traits::{DomainState, HypervisorError};
use crate::storage::traits::StorageError;

pub use engine::BackupEngine;
pub use guard::{ActiveSnapshotInfo, SnapshotCleanupGuard};
pub use model::{
    BackupConsistencyLevel, BackupManifest, BackupOptions, DiskBackupEntry, RestoreOptions,
};
pub use restore::RestoreEngine;

/// Enumerates errors that can occur during backup operations.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Hypervisor error during backup: {0}")]
    HypervisorError(#[from] HypervisorError),

    #[error("Storage error during backup: {0}")]
    StorageError(#[from] StorageError),

    #[error("Instance '{instance_name}' has no attached disks with backing storage")]
    NoDisksFound { instance_name: String },

    #[error("Domain '{instance_name}' is in unsupported state '{state:?}' for backup")]
    UnsupportedDomainState {
        instance_name: String,
        state: DomainState,
    },

    #[error("Failed to parse domain XML: {0}")]
    XmlParseError(#[from] DomainXmlParseError),

    #[error("Failed to serialize backup manifest to JSON: {0}")]
    JsonSerializationError(#[from] serde_json::Error),

    #[error("Domain XML for '{instance_name}' is missing NVRAM path")]
    MissingNvramPath { instance_name: String },

    #[error("Domain XML for '{instance_name}' is missing UUID element")]
    MissingInstanceUuid { instance_name: String },

    #[error("Invalid instance UUID: {0}")]
    InvalidInstanceUuid(#[from] ManifestValidationError),

    #[error("I/O error during backup: {0}")]
    IoError(#[from] std::io::Error),
}

/// Enumerates errors that can occur during restore operations.
#[derive(Debug, Error)]
pub enum RestoreError {
    #[error("Hypervisor error during restore: {0}")]
    HypervisorError(#[from] HypervisorError),

    #[error("Storage error during restore: {0}")]
    StorageError(#[from] StorageError),

    #[error("Domain '{instance_name}' already exists in hypervisor")]
    DomainAlreadyExists { instance_name: String },

    #[error("Storage pool could not be resolved for restore: no target pool specified")]
    UnresolvedStoragePool,

    #[error("Base image not found at '{path}'")]
    BaseImageNotFound { path: PathBuf },

    #[error("Failed to parse backup manifest JSON: {0}")]
    ManifestParseError(#[from] serde_json::Error),

    #[error("Failed to parse domain XML: {0}")]
    XmlParseError(#[from] DomainXmlParseError),

    #[error("Backup manifest has empty disks list for '{instance_name}'")]
    EmptyDisks { instance_name: String },

    #[error("Domain XML is missing required NVRAM path")]
    MissingNvramPath,

    #[error("I/O error during restore: {0}")]
    IoError(#[from] std::io::Error),
}
