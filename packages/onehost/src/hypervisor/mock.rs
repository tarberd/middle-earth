use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::domain::element::DomainXmlElement;
use crate::hypervisor::traits::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor,
    HypervisorError,
};

/// Records an individual hypervisor operation for assertion in test suites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedHypervisorAction {
    DomainInfoRequested {
        domain_name: String,
    },
    DumpXmlRequested {
        domain_name: String,
        inactive: bool,
    },
    DefineDomain {
        domain_xml: String,
    },
    UndefineDomain {
        domain_name: String,
        cleanup_nvram: bool,
    },
    StartDomain {
        domain_name: String,
    },
    ShutdownDomain {
        domain_name: String,
    },
    DestroyDomain {
        domain_name: String,
    },
    ListBlockDevices {
        domain_name: String,
    },
    CreateSnapshot {
        domain_name: String,
        snapshot_name: String,
        disk_specifications: Vec<DiskSnapshotSpecification>,
        quiesce: bool,
    },
    Blockcommit {
        domain_name: String,
        target_device: String,
        base_path: Option<PathBuf>,
        top_path: Option<PathBuf>,
        active: bool,
        pivot: bool,
    },
    PoolDumpXml {
        pool_name: String,
    },
    PoolRefresh {
        pool_name: String,
    },
    SetAutostart {
        domain_name: String,
        autostart: bool,
    },
}

/// Internal in-memory state tracked by MockHypervisor.
#[derive(Debug, Default, Clone)]
pub struct MockHypervisorState {
    pub domains: HashMap<String, DomainInfo>,
    pub domain_xmls: HashMap<String, String>,
    pub block_devices: HashMap<String, Vec<BlockDeviceInfo>>,
    pub pool_xmls: HashMap<String, String>,
    pub refreshed_pools: Vec<String>,
    pub recorded_actions: Vec<RecordedHypervisorAction>,
    pub injected_domain_errors: HashMap<String, String>,
    pub injected_pool_errors: HashMap<String, String>,
}

/// In-memory mock implementing the Hypervisor trait for hermetic testing.
#[derive(Debug, Default, Clone)]
pub struct MockHypervisor {
    pub state: Arc<Mutex<MockHypervisorState>>,
}

fn extract_block_devices_from_domain_xml(domain_xml: &str) -> Vec<BlockDeviceInfo> {
    DomainXmlElement::parse(domain_xml)
        .ok()
        .and_then(|parsed_root| {
            parsed_root.find_child_by_tag("devices").map(|devices_node| {
                devices_node
                    .find_children("disk")
                    .map(|child| {
                        let target_device = child
                            .child_attribute("target", "dev")
                            .unwrap_or("sda")
                            .to_string();
                        let device_type = child
                            .get_attribute("device")
                            .unwrap_or("disk")
                            .to_string();
                        let source_file = child
                            .child_attribute("source", "file")
                            .map(PathBuf::from);

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

impl MockHypervisor {
    /// Creates an empty MockHypervisor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Functional builder: registers a domain with its runtime info and Domain XML.
    pub fn with_domain(self, info: DomainInfo, domain_xml: impl Into<String>) -> Self {
        let domain_name = info.name.clone();
        let xml_string = domain_xml.into();
        let extracted_devices = extract_block_devices_from_domain_xml(&xml_string);
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.domains.insert(domain_name.clone(), info);
            locked_state
                .domain_xmls
                .insert(domain_name.clone(), xml_string);
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

    /// Returns a copy of all recorded actions executed on this mock.
    pub fn recorded_actions(&self) -> Vec<RecordedHypervisorAction> {
        self.state
            .lock()
            .map(|locked_state| locked_state.recorded_actions.clone())
            .unwrap_or_default()
    }
}

impl Hypervisor for MockHypervisor {
    fn domain_info(&self, domain_name: &str) -> Result<DomainInfo, HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::DomainInfoRequested {
                domain_name: domain_name.to_string(),
            });

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
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::DumpXmlRequested {
                domain_name: domain_name.to_string(),
                inactive,
            });

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh dumpxml {domain_name}"),
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

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::DefineDomain {
                domain_xml: domain_xml.to_string(),
            });

        let parsed_root = DomainXmlElement::parse(domain_xml).map_err(|parse_error| {
            HypervisorError::OutputParseError {
                details: format!("Failed to parse domain XML during mock define: {parse_error}"),
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
            .map(|info| info.autostart)
            .unwrap_or(false);

        let defined_info = DomainInfo {
            name: domain_name.clone(),
            state: DomainState::Shutoff,
            vcpu_count: None,
            memory_kib: None,
            autostart: existing_autostart,
        };

        let extracted_devices = extract_block_devices_from_domain_xml(domain_xml);
        locked_state.domains.insert(domain_name.clone(), defined_info);
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

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::UndefineDomain {
                domain_name: domain_name.to_string(),
                cleanup_nvram,
            });

        if let Some(error_details) = locked_state.injected_domain_errors.get(domain_name) {
            return Err(HypervisorError::CommandExecutionFailed {
                command: format!("virsh undefine {domain_name}"),
                exit_code: Some(1),
                stderr: error_details.clone(),
            });
        }

        if locked_state.domains.remove(domain_name).is_some() {
            locked_state.domain_xmls.remove(domain_name);
            locked_state.block_devices.remove(domain_name);
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

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::StartDomain {
                domain_name: domain_name.to_string(),
            });

        let domain_info = locked_state
            .domains
            .get_mut(domain_name)
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        domain_info.state = DomainState::Running;
        Ok(())
    }

    fn shutdown_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::ShutdownDomain {
                domain_name: domain_name.to_string(),
            });

        let domain_info = locked_state
            .domains
            .get_mut(domain_name)
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        domain_info.state = DomainState::Shutoff;
        Ok(())
    }

    fn destroy_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::DestroyDomain {
                domain_name: domain_name.to_string(),
            });

        let domain_info = locked_state
            .domains
            .get_mut(domain_name)
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        domain_info.state = DomainState::Shutoff;
        Ok(())
    }

    fn list_block_devices(
        &self,
        domain_name: &str,
    ) -> Result<Vec<BlockDeviceInfo>, HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::ListBlockDevices {
                domain_name: domain_name.to_string(),
            });

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

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::CreateSnapshot {
                domain_name: domain_name.to_string(),
                snapshot_name: snapshot_name.to_string(),
                disk_specifications: disk_specifications.to_vec(),
                quiesce,
            });

        locked_state
            .domains
            .contains_key(domain_name)
            .then_some(())
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        Ok(())
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

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::Blockcommit {
                domain_name: domain_name.to_string(),
                target_device: target_device.to_string(),
                base_path: base_path.map(Path::to_path_buf),
                top_path: top_path.map(Path::to_path_buf),
                active,
                pivot,
            });

        locked_state
            .domains
            .contains_key(domain_name)
            .then_some(())
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
            })?;

        Ok(())
    }

    fn pool_dump_xml(&self, pool_name: &str) -> Result<String, HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::PoolDumpXml {
                pool_name: pool_name.to_string(),
            });

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
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::PoolRefresh {
                pool_name: pool_name.to_string(),
            });

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
            locked_state.refreshed_pools.push(pool_name.to_string());
            Ok(())
        }
    }

    fn set_autostart(&self, domain_name: &str, autostart: bool) -> Result<(), HypervisorError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedHypervisorAction::SetAutostart {
                domain_name: domain_name.to_string(),
                autostart,
            });

        let domain_info = locked_state
            .domains
            .get_mut(domain_name)
            .ok_or_else(|| HypervisorError::DomainNotFound {
                domain_name: domain_name.to_string(),
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
