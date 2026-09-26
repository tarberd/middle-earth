use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::config::model::InstanceUuid;

/// Indicates the level of filesystem and storage consistency achieved by a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupConsistencyLevel {
    /// Guest agent successfully coordinated with Windows VSS for full application consistency.
    VssQuiesced,
    /// Atomic disk-only snapshot without guest agent coordination.
    CrashConsistent,
    /// VM was shut off; disk overlays directly archived without hypervisor intervention.
    Offline,
}

/// Metadata describing an individual backed-up disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskBackupEntry {
    pub target_device: String,
    pub archive_filename: String,
    pub virtual_size_bytes: u64,
    pub archive_size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backing_file: Option<PathBuf>,
}

/// Structured manifest stored in `<backup_dir>/manifest.json` enabling single-command restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub instance_name: String,
    pub instance_uuid: InstanceUuid,
    pub timestamp: String,
    pub consistency_level: BackupConsistencyLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_pool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_image_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_image_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub golden_master_filename: Option<String>,
    pub disks: Vec<DiskBackupEntry>,
    pub domain_xml_filename: String,
    pub nvram_filename: String,
}

/// Options controlling backup execution.
#[derive(Debug, Clone)]
pub struct BackupOptions {
    pub quiesce: bool,
    pub compress: bool,
    pub timestamp: Option<String>,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            quiesce: true,
            compress: true,
            timestamp: None,
        }
    }
}

/// Options controlling restore execution.
#[derive(Debug, Clone, Default)]
pub struct RestoreOptions {
    pub target_pool: Option<String>,
    pub depot_store_dir: Option<PathBuf>,
    pub nvram_dir: Option<PathBuf>,
    pub allow_overwrite: bool,
}
