use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use onehost::domain::element::DomainXmlElement;
use onehost::hypervisor::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor,
    HypervisorError,
};

/// Internal record representing domain state, XML configuration, and attached devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeDomainEntry {
    pub info: DomainInfo,
    pub xml: String,
    pub block_devices: Vec<BlockDeviceInfo>,
}

/// Internal in-memory state tracked by `FakeHypervisor`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FakeHypervisorState {
    pub domains: HashMap<String, FakeDomainEntry>,
    pub storage_pools: HashMap<String, String>,
    pub injected_domain_errors: HashMap<String, String>,
    pub injected_pool_errors: HashMap<String, String>,
    pub fail_quiesce: bool,
}

/// In-memory fake hypervisor implementing the `Hypervisor` trait boundary port.
///
/// Models domain presence, defined XML, running states, block device attachments,
/// and storage pools purely in memory with zero call-recording action spies.
#[derive(Debug, Default, Clone)]
pub struct FakeHypervisor {
    pub state: Arc<Mutex<FakeHypervisorState>>,
}

fn extract_block_devices_from_domain_xml(domain_xml: &str) -> Vec<BlockDeviceInfo> {
    DomainXmlElement::parse(domain_xml)
        .ok()
        .and_then(|parsed_root| {
            parsed_root
                .find_child_by_tag("devices")
                .map(|devices_node| {
                    devices_node
                        .find_children("disk")
                        .map(|child| {
                            let target_device = child
                                .child_attribute("target", "dev")
                                .unwrap_or("sda")
                                .to_string();
                            let device_type =
                                child.get_attribute("device").unwrap_or("disk").to_string();
                            let source_file =
                                child.child_attribute("source", "file").map(PathBuf::from);

                            BlockDeviceInfo {
                                target_device,
                                device_type,
                                source_file,
                            }
                        })
                        .collect()
                })
        })
        .unwrap_or_default()
}

impl FakeHypervisor {
    /// Creates an empty `FakeHypervisor`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Functional builder: registers a domain with its runtime info and Domain XML.
    pub fn with_domain(self, domain_info: DomainInfo, domain_xml: impl Into<String>) -> Self {
        let domain_name = domain_info.name.clone();
        let raw_domain_xml = domain_xml.into();
        let extracted_devices = extract_block_devices_from_domain_xml(&raw_domain_xml);
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.domains.insert(
                domain_name,
                FakeDomainEntry {
                    info: domain_info,
                    xml: raw_domain_xml,
                    block_devices: extracted_devices,
                },
            );
        }
        self
    }

    /// Functional builder: registers storage pool XML.
    pub fn with_pool_xml(self, pool_name: impl Into<String>, pool_xml: impl Into<String>) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .storage_pools
                .insert(pool_name.into(), pool_xml.into());
        }
        self
    }

    /// Functional builder: registers block devices for a domain.
    pub fn with_block_devices(
        self,
        domain_name: impl Into<String>,
        devices: Vec<BlockDeviceInfo>,
    ) -> Self {
        let name = domain_name.into();
        if let Ok(mut locked_state) = self.state.lock() {
            if let Some(entry) = locked_state.domains.get_mut(&name) {
                entry.block_devices = devices;
            } else {
                locked_state.domains.insert(
                    name.clone(),
                    FakeDomainEntry {
                        info: DomainInfo {
                            name,
                            state: DomainState::Shutoff,
                            vcpu_count: None,
                            memory_kib: None,
                            autostart: false,
                        },
                        xml: String::new(),
                        block_devices: devices,
                    },
                );
            }
        }
        self
    }

    /// Functional builder: injects a command failure error for a specific domain.
    pub fn with_injected_domain_error(
        self,
        domain_name: impl Into<String>,
        error_details: impl Into<String>,
    ) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .injected_domain_errors
                .insert(domain_name.into(), error_details.into());
        }
        self
    }

    /// Functional builder: injects a command failure error for a specific pool.
    pub fn with_injected_pool_error(
        self,
        pool_name: impl Into<String>,
        error_details: impl Into<String>,
    ) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .injected_pool_errors
                .insert(pool_name.into(), error_details.into());
        }
        self
    }

    /// Functional builder: sets whether guest agent quiesce should fail.
    pub fn with_quiesce_failure(self, fail_quiesce: bool) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.fail_quiesce = fail_quiesce;
        }
        self
    }

    /// Pure state snapshot projection returning an immutable `FakeHypervisorState` value.
    pub fn snapshot(&self) -> FakeHypervisorState {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl Hypervisor for FakeHypervisor {
    fn domain_info(&self, domain_name: &str) -> Result<DomainInfo, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh dominfo {domain_name}"),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            locked_state
                .domains
                .get(domain_name)
                .map(|entry| entry.info.clone())
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })
        }
    }

    fn dump_xml(&self, domain_name: &str, inactive: bool) -> Result<String, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            let command = if inactive {
                format!("virsh dumpxml {domain_name} --inactive")
            } else {
                format!("virsh dumpxml {domain_name}")
            };
            Err(HypervisorError::CommandExecutionFailed {
                command,
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            locked_state
                .domains
                .get(domain_name)
                .map(|entry| entry.xml.clone())
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })
        }
    }

    fn define_domain(&self, domain_xml: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let parsed_root = DomainXmlElement::parse(domain_xml).map_err(|parse_error| {
            HypervisorError::OutputParseError {
                details: format!("Failed to parse domain XML during fake define: {parse_error}"),
            }
        })?;

        let domain_name = parsed_root
            .find_child_by_tag("name")
            .and_then(|name_node| name_node.text_content.clone())
            .ok_or_else(|| HypervisorError::OutputParseError {
                details: "Domain XML missing <name> element".to_string(),
            })?;

        let defined_info = match locked_state.domains.get(&domain_name) {
            Some(existing) => DomainInfo {
                name: domain_name.clone(),
                state: existing.info.state,
                vcpu_count: existing.info.vcpu_count,
                memory_kib: existing.info.memory_kib,
                autostart: existing.info.autostart,
            },
            None => DomainInfo {
                name: domain_name.clone(),
                state: DomainState::Shutoff,
                vcpu_count: None,
                memory_kib: None,
                autostart: false,
            },
        };

        let extracted_devices = extract_block_devices_from_domain_xml(domain_xml);
        locked_state.domains.insert(
            domain_name,
            FakeDomainEntry {
                info: defined_info,
                xml: domain_xml.to_string(),
                block_devices: extracted_devices,
            },
        );

        Ok(())
    }

    fn undefine_domain(
        &self,
        domain_name: &str,
        cleanup_nvram: bool,
    ) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let command = if cleanup_nvram {
            format!("virsh undefine {domain_name} --nvram")
        } else {
            format!("virsh undefine {domain_name}")
        };

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command,
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else if let Some(domain_entry) = locked_state.domains.get(domain_name) {
            if domain_entry.info.state == DomainState::Running {
                Err(HypervisorError::CommandExecutionFailed {
                    command,
                    exit_code: Some(1),
                    stderr: format!(
                        "Cannot undefine running domain '{domain_name}'; shut down or destroy it first"
                    ),
                })
            } else {
                let removed_entry = locked_state.domains.remove(domain_name);
                if cleanup_nvram
                    && let Some(nvram_path) = removed_entry
                        .as_ref()
                        .and_then(|entry| DomainXmlElement::parse(&entry.xml).ok())
                        .and_then(|root| root.path_text(&["os", "nvram"]).map(PathBuf::from))
                    && nvram_path.exists()
                {
                    drop(std::fs::remove_file(nvram_path));
                }
                Ok(())
            }
        } else {
            Err(HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })
        }
    }

    fn start_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_entry = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_entry.info.state = DomainState::Running;
        Ok(())
    }

    fn shutdown_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_entry = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_entry.info.state = DomainState::Shutoff;
        Ok(())
    }

    fn destroy_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_entry = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_entry.info.state = DomainState::Shutoff;
        Ok(())
    }

    fn list_block_devices(
        &self,
        domain_name: &str,
    ) -> Result<Vec<BlockDeviceInfo>, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_entry = locked_state.domains.get(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        Ok(domain_entry.block_devices.clone())
    }

    fn create_snapshot(
        &self,
        domain_name: &str,
        snapshot_name: &str,
        disk_specifications: &[DiskSnapshotSpecification],
        quiesce: bool,
    ) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if quiesce && locked_state.fail_quiesce {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!(
                    "virsh snapshot-create-as {domain_name} {snapshot_name} --quiesce"
                ),
                exit_code: Some(1),
                stderr: "error: Guest agent is not responding: QEMU guest agent is not configured"
                    .to_string(),
            })
        } else {
            let domain_entry = locked_state
                .domains
                .get_mut(domain_name)
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })?;

            let created_files: Vec<std::io::Result<std::fs::File>> = disk_specifications
                .iter()
                .filter(|spec| {
                    spec.snapshot_file_path
                        .parent()
                        .is_some_and(|parent| parent.exists())
                })
                .map(|spec| std::fs::File::create(&spec.snapshot_file_path))
                .collect();
            drop(created_files);

            let updated_existing_devices = domain_entry
                .block_devices
                .iter()
                .map(|existing_device| {
                    let updated_source = disk_specifications
                        .iter()
                        .find(|spec| spec.target_device == existing_device.target_device)
                        .map(|spec| spec.snapshot_file_path.clone());

                    BlockDeviceInfo {
                        target_device: existing_device.target_device.clone(),
                        device_type: existing_device.device_type.clone(),
                        source_file: updated_source.or_else(|| existing_device.source_file.clone()),
                    }
                });

            let new_devices = disk_specifications
                .iter()
                .filter(|spec| {
                    !domain_entry
                        .block_devices
                        .iter()
                        .any(|device| device.target_device == spec.target_device)
                })
                .map(|spec| BlockDeviceInfo {
                    target_device: spec.target_device.clone(),
                    device_type: "disk".to_string(),
                    source_file: Some(spec.snapshot_file_path.clone()),
                });

            domain_entry.block_devices = updated_existing_devices.chain(new_devices).collect();

            Ok(())
        }
    }

    fn blockcommit(
        &self,
        domain_name: &str,
        target_device: &str,
        base_path: Option<&Path>,
        top_path: Option<&Path>,
        active: bool,
        pivot: bool,
    ) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh blockcommit {domain_name} {target_device}"),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let domain_entry = locked_state
                .domains
                .get_mut(domain_name)
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })?;

            if active && pivot {
                domain_entry.block_devices = domain_entry
                    .block_devices
                    .iter()
                    .map(|d| {
                        if d.target_device == target_device
                            && (top_path.is_none() || top_path == d.source_file.as_deref())
                        {
                            BlockDeviceInfo {
                                target_device: d.target_device.clone(),
                                device_type: d.device_type.clone(),
                                source_file: base_path.map(Path::to_path_buf),
                            }
                        } else {
                            d.clone()
                        }
                    })
                    .collect();
            }

            Ok(())
        }
    }

    fn pool_dump_xml(&self, pool_name: &str) -> Result<String, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if let Some(error_details) = locked_state.injected_pool_errors.get(pool_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh pool-dumpxml {pool_name}"),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            locked_state
                .storage_pools
                .get(pool_name)
                .cloned()
                .ok_or_else(|| HypervisorError::StoragePoolNotFound {
                    pool_name: pool_name.to_string(),
                })
        }
    }

    fn pool_refresh(&self, pool_name: &str) -> Result<(), HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        if let Some(error_details) = locked_state.injected_pool_errors.get(pool_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh pool-refresh {pool_name}"),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else if !locked_state.storage_pools.contains_key(pool_name) {
            Err(HypervisorError::StoragePoolNotFound {
                pool_name: pool_name.to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn set_autostart(&self, domain_name: &str, autostart: bool) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_entry = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_entry.info.autostart = autostart;
        Ok(())
    }

    fn list_storage_pools(&self) -> Result<Vec<String>, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;
        Ok(locked_state.storage_pools.keys().cloned().collect())
    }
}
