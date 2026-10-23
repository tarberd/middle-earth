use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use onehost::domain::element::DomainXmlElement;
use onehost::hypervisor::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor,
    HypervisorError,
};

/// Internal in-memory state tracked by `FakeHypervisor`.
#[derive(Debug, Default, Clone)]
pub struct FakeHypervisorState {
    pub domains: HashMap<String, DomainInfo>,
    pub domain_xmls: HashMap<String, String>,
    pub block_devices: HashMap<String, Vec<BlockDeviceInfo>>,
    pub pool_xmls: HashMap<String, String>,
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
            locked_state
                .domains
                .insert(domain_name.clone(), domain_info);
            locked_state
                .domain_xmls
                .insert(domain_name.clone(), raw_domain_xml);
            locked_state
                .block_devices
                .entry(domain_name)
                .or_insert(extracted_devices);
        }
        self
    }

    /// Functional builder: registers storage pool XML.
    pub fn with_pool_xml(self, pool_name: impl Into<String>, pool_xml: impl Into<String>) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .pool_xmls
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
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .block_devices
                .insert(domain_name.into(), devices);
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

    /// State inspection helper: returns true if the domain exists in defined domains.
    pub fn is_domain_defined(&self, domain_name: &str) -> bool {
        self.state
            .lock()
            .ok()
            .is_some_and(|locked_state| locked_state.domains.contains_key(domain_name))
    }

    /// State inspection helper: returns a sorted list of all defined domain names.
    pub fn defined_domain_names(&self) -> Vec<String> {
        self.state
            .lock()
            .ok()
            .map(|locked_state| {
                let mut names: Vec<String> = locked_state.domains.keys().cloned().collect();
                names.sort();
                names
            })
            .unwrap_or_default()
    }

    /// State inspection helper: queries current execution state of a domain.
    pub fn domain_state(&self, domain_name: &str) -> Option<DomainState> {
        self.state
            .lock()
            .ok()
            .and_then(|locked_state| locked_state.domains.get(domain_name).map(|info| info.state))
    }

    /// State inspection helper: queries defined XML configuration for a domain.
    pub fn domain_xml(&self, domain_name: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|locked_state| locked_state.domain_xmls.get(domain_name).cloned())
    }

    /// State inspection helper: queries attached block devices for a domain.
    pub fn domain_block_devices(&self, domain_name: &str) -> Option<Vec<BlockDeviceInfo>> {
        self.state
            .lock()
            .ok()
            .and_then(|locked_state| locked_state.block_devices.get(domain_name).cloned())
    }

    /// State inspection helper: queries autostart setting for a domain.
    pub fn domain_autostart(&self, domain_name: &str) -> Option<bool> {
        self.state
            .lock()
            .ok()
            .and_then(|locked_state| {
                locked_state
                    .domains
                    .get(domain_name)
                    .map(|info| info.autostart)
            })
    }

    /// State inspection helper: returns true if the storage pool exists.
    pub fn has_storage_pool(&self, pool_name: &str) -> bool {
        self.state
            .lock()
            .ok()
            .is_some_and(|locked_state| locked_state.pool_xmls.contains_key(pool_name))
    }

    /// State inspection helper: queries XML configuration for a storage pool.
    pub fn storage_pool_xml(&self, pool_name: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|locked_state| locked_state.pool_xmls.get(pool_name).cloned())
    }

    /// State inspection helper: returns a sorted list of registered storage pool names.
    pub fn defined_storage_pools(&self) -> Vec<String> {
        self.state
            .lock()
            .ok()
            .map(|locked_state| {
                let mut pools: Vec<String> = locked_state.pool_xmls.keys().cloned().collect();
                pools.sort();
                pools
            })
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
                .cloned()
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
                .domain_xmls
                .get(domain_name)
                .cloned()
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

        let existing_autostart = locked_state
            .domains
            .get(&domain_name)
            .map(|existing_domain_info| existing_domain_info.autostart)
            .unwrap_or(false);

        let defined_info = DomainInfo {
            name: domain_name.clone(),
            state: DomainState::Shutoff,
            vcpu_count: None,
            memory_kib: None,
            autostart: existing_autostart,
        };

        let extracted_devices = extract_block_devices_from_domain_xml(domain_xml);
        locked_state
            .domains
            .insert(domain_name.clone(), defined_info);
        locked_state
            .domain_xmls
            .insert(domain_name.clone(), domain_xml.to_string());
        locked_state
            .block_devices
            .insert(domain_name, extracted_devices);

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

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            let command = if cleanup_nvram {
                format!("virsh undefine {domain_name} --nvram")
            } else {
                format!("virsh undefine {domain_name}")
            };
            Err(HypervisorError::CommandExecutionFailed {
                command,
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else if locked_state.domains.remove(domain_name).is_some() {
            let removed_xml = locked_state.domain_xmls.remove(domain_name);
            locked_state.block_devices.remove(domain_name);
            if cleanup_nvram
                && let Some(nvram_path) = removed_xml
                    .as_deref()
                    .and_then(|xml| DomainXmlElement::parse(xml).ok())
                    .and_then(|root| root.path_text(&["os", "nvram"]).map(PathBuf::from))
                && nvram_path.exists()
            {
                drop(std::fs::remove_file(nvram_path));
            }
            Ok(())
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

        let domain_info = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_info.state = DomainState::Running;
        Ok(())
    }

    fn shutdown_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_info = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_info.state = DomainState::Shutoff;
        Ok(())
    }

    fn destroy_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        let domain_info = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_info.state = DomainState::Shutoff;
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

        locked_state
            .domains
            .contains_key(domain_name)
            .then_some(())
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        Ok(locked_state
            .block_devices
            .get(domain_name)
            .cloned()
            .unwrap_or_default())
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
            locked_state
                .domains
                .contains_key(domain_name)
                .then_some(())
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })?;

            disk_specifications
                .iter()
                .filter(|spec| {
                    spec.snapshot_file_path
                        .parent()
                        .is_some_and(|parent| parent.exists())
                })
                .fold((), |(), spec| {
                    drop(std::fs::File::create(&spec.snapshot_file_path));
                });

            let devices = locked_state
                .block_devices
                .entry(domain_name.to_string())
                .or_default();

            disk_specifications.iter().fold((), |(), spec| {
                if let Some(existing_device) = devices
                    .iter_mut()
                    .find(|device| device.target_device == spec.target_device)
                {
                    existing_device.source_file = Some(spec.snapshot_file_path.clone());
                } else {
                    devices.push(BlockDeviceInfo {
                        target_device: spec.target_device.clone(),
                        device_type: "disk".to_string(),
                        source_file: Some(spec.snapshot_file_path.clone()),
                    });
                }
            });

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
            locked_state
                .domains
                .contains_key(domain_name)
                .then_some(())
                .ok_or_else(|| HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })?;

            if active
                && pivot
                && let Some(devices) = locked_state.block_devices.get_mut(domain_name)
                && let Some(device) = devices.iter_mut().find(|d| {
                    d.target_device == target_device
                        && (top_path.is_none() || top_path == d.source_file.as_deref())
                })
            {
                device.source_file = base_path.map(Path::to_path_buf);
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
                .pool_xmls
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
        } else if !locked_state.pool_xmls.contains_key(pool_name) {
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

        let domain_info = locked_state.domains.get_mut(domain_name).ok_or_else(|| {
            HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            }
        })?;

        domain_info.autostart = autostart;
        Ok(())
    }

    fn list_storage_pools(&self) -> Result<Vec<String>, HypervisorError> {
        let locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;
        Ok(locked_state.pool_xmls.keys().cloned().collect())
    }
}
