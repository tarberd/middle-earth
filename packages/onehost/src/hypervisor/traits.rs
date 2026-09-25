use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::element::DomainXmlElement;

/// Represents the execution state of a Libvirt domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainState {
    Running,
    Shutoff,
    Paused,
    Crashed,
    Blocked,
    Pmsuspended,
    Unknown,
}

impl DomainState {
    /// Parses domain state from `virsh dominfo` state string.
    pub fn from_virsh_state(raw_state: &str) -> Self {
        let normalized = raw_state.trim().to_lowercase();
        if normalized.starts_with("running") {
            Self::Running
        } else if normalized.starts_with("shut off") || normalized.starts_with("shutoff") {
            Self::Shutoff
        } else if normalized.starts_with("paused") {
            Self::Paused
        } else if normalized.starts_with("crashed") {
            Self::Crashed
        } else if normalized.starts_with("blocked") {
            Self::Blocked
        } else if normalized.starts_with("pmsuspended") {
            Self::Pmsuspended
        } else {
            Self::Unknown
        }
    }
}

/// Snapshot of domain runtime information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainInfo {
    pub name: String,
    pub state: DomainState,
    pub vcpu_count: Option<u32>,
    pub memory_kib: Option<u64>,
    pub autostart: bool,
}

/// Information describing a block device attached to a domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDeviceInfo {
    pub target_device: String,
    pub source_file: Option<PathBuf>,
    pub device_type: String,
}

/// Specification for an individual disk to snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskSnapshotSpecification {
    pub target_device: String,
    pub snapshot_file_path: PathBuf,
}

/// Enumerates errors that can occur during hypervisor operations.
#[derive(Debug, Error)]
pub enum HypervisorError {
    #[error("Domain '{domain_name}' not found")]
    DomainNotFound { domain_name: String },

    #[error("Storage pool '{pool_name}' not found")]
    StoragePoolNotFound { pool_name: String },

    #[error("Failed to parse hypervisor command output: {details}")]
    OutputParseError { details: String },

    #[error("Hypervisor command '{command}' failed with exit code {exit_code:?}: {stderr}")]
    CommandExecutionFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Storage pool XML missing <target><path> element: {details}")]
    MissingPoolPathElement { details: String },

    #[error("I/O error during hypervisor interaction: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}

/// Helper function to parse the physical filesystem target path from Libvirt storage pool XML.
pub fn parse_pool_target_path(pool_xml: &str) -> Result<PathBuf, HypervisorError> {
    let pool_root = DomainXmlElement::parse(pool_xml).map_err(|error| {
        HypervisorError::OutputParseError {
            details: format!("Failed to parse storage pool XML: {error}"),
        }
    })?;

    let pool_target_path = pool_root
        .find_child_by_tag("target")
        .and_then(|target_section| target_section.find_child_by_tag("path"))
        .and_then(|path_node| path_node.text_content.as_deref())
        .ok_or_else(|| HypervisorError::MissingPoolPathElement {
            details: "No <target><path> child found in storage pool XML".to_string(),
        })?;

    Ok(PathBuf::from(pool_target_path))
}

/// Trait abstracting hypervisor and storage pool operations.
pub trait Hypervisor: Send + Sync {
    /// Retrieves current runtime status of a domain.
    fn domain_info(&self, domain_name: &str) -> Result<DomainInfo, HypervisorError>;

    /// Dumps active or inactive XML configuration of a domain.
    fn dump_xml(&self, domain_name: &str, inactive: bool) -> Result<String, HypervisorError>;

    /// Defines a domain in Libvirt from concrete XML.
    fn define_domain(&self, domain_xml: &str) -> Result<(), HypervisorError>;

    /// Undefines a domain, optionally unlinking its NVRAM file.
    fn undefine_domain(&self, domain_name: &str, cleanup_nvram: bool) -> Result<(), HypervisorError>;

    /// Powers on / boots a shut off domain.
    fn start_domain(&self, domain_name: &str) -> Result<(), HypervisorError>;

    /// Sends an ACPI shutdown signal to a running domain.
    fn shutdown_domain(&self, domain_name: &str) -> Result<(), HypervisorError>;

    /// Forcefully terminates / cuts power to a domain.
    fn destroy_domain(&self, domain_name: &str) -> Result<(), HypervisorError>;

    /// Lists block devices attached to a domain.
    fn list_block_devices(&self, domain_name: &str) -> Result<Vec<BlockDeviceInfo>, HypervisorError>;

    /// Creates an atomic live disk snapshot across one or more disks, optionally with guest agent quiescing.
    fn create_snapshot(
        &self,
        domain_name: &str,
        snapshot_name: &str,
        disk_specifications: &[DiskSnapshotSpecification],
        quiesce: bool,
    ) -> Result<(), HypervisorError>;

    /// Merges changes from snapshot back into base disk using active blockcommit with pivot.
    fn blockcommit(
        &self,
        domain_name: &str,
        target_device: &str,
        base_path: Option<&Path>,
        top_path: Option<&Path>,
        active: bool,
        pivot: bool,
    ) -> Result<(), HypervisorError>;

    /// Queries XML of a storage pool.
    fn pool_dump_xml(&self, pool_name: &str) -> Result<String, HypervisorError>;

    /// Refreshes Libvirt's in-memory index of a storage pool's volumes.
    fn pool_refresh(&self, pool_name: &str) -> Result<(), HypervisorError>;

    /// Resolves the physical directory path of a storage pool.
    fn resolve_pool_path(&self, pool_name: &str) -> Result<PathBuf, HypervisorError> {
        let pool_xml = self.pool_dump_xml(pool_name)?;
        parse_pool_target_path(&pool_xml)
    }

    /// Configures autostart on domain boot.
    fn set_autostart(&self, domain_name: &str, autostart: bool) -> Result<(), HypervisorError>;

    /// Lists names of all storage pools known to the hypervisor.
    fn list_storage_pools(&self) -> Result<Vec<String>, HypervisorError>;

    /// Resolves the storage pool name whose target path matches `target_path`.
    fn resolve_pool_name_by_path(&self, target_path: &Path) -> Result<Option<String>, HypervisorError> {
        let matched_pool = self
            .list_storage_pools()?
            .into_iter()
            .find(|pool_name| {
                self.resolve_pool_path(pool_name)
                    .is_ok_and(|pool_path| pool_path == target_path)
            });

        Ok(matched_pool)
    }
}
