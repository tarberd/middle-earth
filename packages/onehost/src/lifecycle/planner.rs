use std::path::{Path, PathBuf};

use crate::config::model::{InstanceConfiguration, OnehostManifest};
use crate::domain::diff::DomainXmlDiffer;
use crate::domain::template::{DomainTemplateEngine, DomainTemplateInjectionParameters};
use crate::hypervisor::traits::{Hypervisor, HypervisorError};
use crate::image::tag::{
    ContentAddressedImageResolver, FlavorDerivationMetadata, FlavorResolutionError,
};
use serde::{Deserialize, Serialize};

use crate::lifecycle::LifecycleError;
use crate::storage::traits::{StorageError, StorageManager};

/// Alert describing a dangling snapshot from an interrupted backup that needs recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanglingSnapshotAlert {
    pub instance_name: String,
    pub target_device: String,
    pub snapshot_path: PathBuf,
}

/// Action to reconcile a declared instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstancePlanAction {
    Create {
        instance_name: String,
        concrete_xml: String,
        target_pool: String,
        overlay_path: PathBuf,
        base_cache_path: PathBuf,
        golden_master_path: PathBuf,
        nvram_path: PathBuf,
        nvram_template_path: PathBuf,
    },
    UpdateDomainXml {
        instance_name: String,
        concrete_xml: String,
        diff_summary: String,
    },
    Recreate {
        instance_name: String,
        reason: String,
        concrete_xml: String,
        target_pool: String,
        overlay_path: PathBuf,
        base_cache_path: PathBuf,
        golden_master_path: PathBuf,
        nvram_path: PathBuf,
        nvram_template_path: PathBuf,
    },
    RelocateStoragePool {
        instance_name: String,
        source_pool: String,
        target_pool: String,
        source_overlay_path: PathBuf,
        target_overlay_path: PathBuf,
        target_base_cache_path: PathBuf,
        golden_master_path: PathBuf,
        concrete_xml: String,
    },
    Delete {
        instance_name: String,
        target_pool: String,
    },
    NoOp {
        instance_name: String,
    },
}

impl InstancePlanAction {
    pub fn instance_name(&self) -> &str {
        match self {
            Self::Create { instance_name, .. }
            | Self::UpdateDomainXml { instance_name, .. }
            | Self::Recreate { instance_name, .. }
            | Self::RelocateStoragePool { instance_name, .. }
            | Self::Delete { instance_name, .. }
            | Self::NoOp { instance_name } => instance_name,
        }
    }
}

/// Structured execution plan comparing declared manifest state against live infrastructure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnehostPlan {
    pub actions: Vec<InstancePlanAction>,
    pub dangling_snapshots: Vec<DanglingSnapshotAlert>,
}

/// Type alias for custom template content resolver function.
pub type TemplateResolver = Box<dyn Fn(&Path) -> Result<String, LifecycleError> + Send + Sync>;

/// Pure, stateless planner calculating actions needed to reconcile manifest into reality.
pub struct DomainLifecyclePlanner<'a, H: Hypervisor, S: StorageManager> {
    pub hypervisor: &'a H,
    pub storage: &'a S,
    pub template_resolver: Option<TemplateResolver>,
}

impl<'a, H: Hypervisor, S: StorageManager> DomainLifecyclePlanner<'a, H, S> {
    /// Creates a new DomainLifecyclePlanner.
    pub fn new(hypervisor: &'a H, storage: &'a S) -> Self {
        Self {
            hypervisor,
            storage,
            template_resolver: None,
        }
    }

    /// Functional builder: injects a custom template content resolver (useful for testing).
    pub fn with_template_resolver<F>(self, resolver: F) -> Self
    where
        F: Fn(&Path) -> Result<String, LifecycleError> + Send + Sync + 'static,
    {
        Self {
            template_resolver: Some(Box::new(resolver)),
            ..self
        }
    }

    /// Evaluates the declared manifest against live hypervisor and storage state.
    pub fn plan(&self, manifest: &OnehostManifest) -> Result<OnehostPlan, LifecycleError> {
        let planned_instances = manifest
            .instances
            .iter()
            .map(|(instance_name, instance_configuration)| {
                self.plan_instance(instance_name, instance_configuration, manifest)
            })
            .collect::<Result<Vec<(InstancePlanAction, Vec<DanglingSnapshotAlert>)>, LifecycleError>>()?;

        let (actions, dangling_snapshots) = planned_instances.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut accumulated_actions, mut accumulated_alerts), (action, alerts)| {
                accumulated_actions.push(action);
                accumulated_alerts.extend(alerts);
                (accumulated_actions, accumulated_alerts)
            },
        );

        Ok(OnehostPlan {
            actions,
            dangling_snapshots,
        })
    }

    fn plan_instance(
        &self,
        instance_name: &str,
        instance_configuration: &InstanceConfiguration,
        manifest: &OnehostManifest,
    ) -> Result<(InstancePlanAction, Vec<DanglingSnapshotAlert>), LifecycleError> {
        let effective_pool = instance_configuration.effective_pool(instance_name)?;
        let target_pool_directory = self.hypervisor.resolve_pool_path(effective_pool)?;

        let flavor_config = manifest
            .flavors
            .get(&instance_configuration.image.flavor)
            .ok_or_else(|| FlavorResolutionError::UnknownFlavor {
                requested_flavor: instance_configuration.image.flavor.clone(),
                available_flavors: manifest.flavors.keys().cloned().collect(),
            })?;

        let content_hash = self
            .storage
            .compute_content_hash(&flavor_config.oemdrv_path, &flavor_config.extra_isos)?;

        let derivation_metadata =
            FlavorDerivationMetadata::new(flavor_config.oemdrv_path.clone(), content_hash)?;

        let golden_master_path = ContentAddressedImageResolver::resolve_depot_master_path(
            &manifest.storage.depot_store_dir,
            &instance_configuration.image,
            &derivation_metadata,
        );

        let base_cache_path = ContentAddressedImageResolver::resolve_storage_pool_base_path(
            &target_pool_directory,
            &instance_configuration.image,
            &derivation_metadata,
        );

        let overlay_path = ContentAddressedImageResolver::resolve_instance_overlay_path(
            &target_pool_directory,
            instance_name,
        );

        let nvram_path = manifest
            .storage
            .nvram_dir
            .join(format!("{instance_name}_VARS.fd"));

        // Read domain template XML via custom resolver or storage manager
        let template_xml_content = match &self.template_resolver {
            Some(resolver) => resolver(&instance_configuration.template_xml)?,
            None => self
                .storage
                .read_file(&instance_configuration.template_xml)?,
        };

        let injection_parameters = DomainTemplateInjectionParameters {
            instance_name,
            instance_uuid: &instance_configuration.uuid,
            overlay_disk_path: &overlay_path,
            instance_nvram_path: &nvram_path,
            nvram_template_path: Some(&manifest.storage.nvram_template),
            ovmf_code_path: Some(&manifest.storage.ovmf_code),
        };

        let concrete_xml = DomainTemplateEngine::synthesize_concrete_domain_xml(
            &template_xml_content,
            &injection_parameters,
        )?;

        match self.hypervisor.domain_info(instance_name) {
            Err(HypervisorError::DomainNotFound { .. }) => {
                let create_action = InstancePlanAction::Create {
                    instance_name: instance_name.to_string(),
                    concrete_xml,
                    target_pool: effective_pool.to_string(),
                    overlay_path,
                    base_cache_path,
                    golden_master_path,
                    nvram_path,
                    nvram_template_path: manifest.storage.nvram_template.clone(),
                };
                Ok((create_action, Vec::new()))
            }
            Err(hypervisor_failure) => Err(LifecycleError::from(hypervisor_failure)),
            Ok(_) => {
                let attached_devices = self.hypervisor.list_block_devices(instance_name)?;

                let dangling_snapshot_alerts: Vec<DanglingSnapshotAlert> = attached_devices
                    .iter()
                    .filter_map(|block_device| {
                        block_device
                            .source_file
                            .as_ref()
                            .filter(|source_path| source_path.to_string_lossy().ends_with(".snap"))
                            .map(|source_path| DanglingSnapshotAlert {
                                instance_name: instance_name.to_string(),
                                target_device: block_device.target_device.clone(),
                                snapshot_path: source_path.clone(),
                            })
                    })
                    .collect();

                let primary_disk = attached_devices
                    .iter()
                    .find(|device| device.target_device == "sda" || device.device_type == "disk")
                    .and_then(|device| device.source_file.as_ref());

                let live_xml = self.hypervisor.dump_xml(instance_name, false)?;
                let domain_diff_result =
                    DomainXmlDiffer::compare_domain_xmls(&concrete_xml, &live_xml)?;

                let storage_reconciliation_action: Option<InstancePlanAction> = primary_disk
                    .map(|active_disk_path| -> Result<Option<InstancePlanAction>, LifecycleError> {
                        match self.storage.inspect_image(active_disk_path) {
                            Ok(inspection_info) => {
                                let expected_base_filename = ContentAddressedImageResolver::resolve_golden_master_filename(
                                    &instance_configuration.image,
                                    &derivation_metadata,
                                );
                                let backing_matches_base = inspection_info
                                    .backing_file
                                    .as_ref()
                                    .is_some_and(|backing_file_path| {
                                        backing_file_path
                                            .file_name()
                                            .and_then(|name| name.to_str())
                                            .is_some_and(|name| name == expected_base_filename)
                                            || backing_file_path == &base_cache_path
                                            || backing_file_path == &golden_master_path
                                    });

                                let planned_action = if !backing_matches_base {
                                    Some(InstancePlanAction::Recreate {
                                        instance_name: instance_name.to_string(),
                                        reason: format!(
                                            "Base image changed to '{expected_base_filename}' (forces replacement)"
                                        ),
                                        concrete_xml: concrete_xml.clone(),
                                        target_pool: effective_pool.to_string(),
                                        overlay_path: overlay_path.clone(),
                                        base_cache_path: base_cache_path.clone(),
                                        golden_master_path: golden_master_path.clone(),
                                        nvram_path: nvram_path.clone(),
                                        nvram_template_path: manifest.storage.nvram_template.clone(),
                                    })
                                } else {
                                    let active_disk_directory = active_disk_path.parent();
                                    let target_overlay_directory = overlay_path.parent();

                                    if active_disk_directory != target_overlay_directory {
                                        let source_pool = active_disk_directory
                                            .and_then(|directory_path| {
                                                self.hypervisor
                                                    .resolve_pool_name_by_path(directory_path)
                                                    .ok()
                                                    .flatten()
                                             })
                                            .or_else(|| {
                                                active_disk_directory.map(|directory_path| directory_path.to_string_lossy().to_string())
                                            })
                                            .unwrap_or_else(|| "unknown".to_string());

                                        Some(InstancePlanAction::RelocateStoragePool {
                                            instance_name: instance_name.to_string(),
                                            source_pool,
                                            target_pool: effective_pool.to_string(),
                                            source_overlay_path: active_disk_path.clone(),
                                            target_overlay_path: overlay_path.clone(),
                                            target_base_cache_path: base_cache_path.clone(),
                                            golden_master_path: golden_master_path.clone(),
                                            concrete_xml: concrete_xml.clone(),
                                        })
                                    } else {
                                        None
                                    }
                                };
                                Ok(planned_action)
                            }
                            Err(StorageError::SourceFileNotFound { .. }) => Ok(None),
                            Err(storage_error) => Err(LifecycleError::from(storage_error)),
                        }
                    })
                    .transpose()?
                    .flatten();

                let action = match storage_reconciliation_action {
                    Some(reconciliation_action) => reconciliation_action,
                    None if domain_diff_result.has_drift => InstancePlanAction::UpdateDomainXml {
                        instance_name: instance_name.to_string(),
                        concrete_xml,
                        diff_summary: domain_diff_result.summary(),
                    },
                    None => InstancePlanAction::NoOp {
                        instance_name: instance_name.to_string(),
                    },
                };

                Ok((action, dangling_snapshot_alerts))
            }
        }
    }
}
