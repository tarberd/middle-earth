use std::fs;

use crate::config::model::ImageChangePolicy;
use crate::hypervisor::traits::{DomainState, Hypervisor};
use crate::lifecycle::planner::{InstancePlanAction, OnehostPlan};
use crate::lifecycle::LifecycleError;
use crate::storage::traits::StorageManager;

/// Options controlling apply execution.
#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub allow_recreate: bool,
    pub auto_approve: bool,
}

/// Applier reconciling the execution plan against hypervisor and storage infrastructure.
pub struct DomainLifecycleApplier<'a, H: Hypervisor, S: StorageManager> {
    pub hypervisor: &'a H,
    pub storage: &'a S,
}

impl<'a, H: Hypervisor, S: StorageManager> DomainLifecycleApplier<'a, H, S> {
    /// Creates a new DomainLifecycleApplier.
    pub fn new(hypervisor: &'a H, storage: &'a S) -> Self {
        Self {
            hypervisor,
            storage,
        }
    }

    /// Executes the planned reconciliation actions sequentially.
    pub fn apply(&self, plan: &OnehostPlan, options: &ApplyOptions) -> Result<(), LifecycleError> {
        plan.actions
            .iter()
            .try_for_each(|action| self.apply_action(action, options))
    }

    fn apply_action(
        &self,
        action: &InstancePlanAction,
        options: &ApplyOptions,
    ) -> Result<(), LifecycleError> {
        match action {
            InstancePlanAction::Create {
                instance_name,
                concrete_xml,
                target_pool,
                overlay_path,
                base_cache_path,
                golden_master_path,
                nvram_path,
                nvram_template_path,
                autostart,
            } => {
                // Ensure base image is cached in target pool
                self.storage
                    .inspect_image(base_cache_path)
                    .is_err()
                    .then(|| self.storage.copy_base_image(golden_master_path, base_cache_path))
                    .transpose()?;

                // Create fresh CoW overlay
                self.storage
                    .create_cow_overlay(base_cache_path, overlay_path)?;

                // Initialize NVRAM from template
                nvram_path
                    .parent()
                    .filter(|parent_directory| !parent_directory.exists())
                    .map(fs::create_dir_all)
                    .transpose()?;

                (nvram_template_path.exists() && !nvram_path.exists())
                    .then(|| fs::copy(nvram_template_path, nvram_path))
                    .transpose()?;

                // Define domain in Libvirt
                self.hypervisor.define_domain(concrete_xml)?;

                // Configure autostart if requested
                autostart
                    .then(|| self.hypervisor.set_autostart(instance_name, true))
                    .transpose()?;

                // Synchronize storage pool volume inventory
                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::UpdateDomainXml { concrete_xml, .. } => {
                self.hypervisor.define_domain(concrete_xml)?;
                Ok(())
            }

            InstancePlanAction::Recreate {
                instance_name,
                reason,
                policy,
                prevent_destroy,
                concrete_xml,
                target_pool,
                overlay_path,
                base_cache_path,
                golden_master_path,
                nvram_path,
                nvram_template_path,
                autostart,
            } => {
                // Guardrail 1: prevent_destroy check
                (!*prevent_destroy)
                    .then_some(())
                    .ok_or_else(|| LifecycleError::LifecyclePolicyViolation {
                        instance_name: instance_name.clone(),
                        details: format!(
                            "Instance '{instance_name}' has prevent_destroy enabled; cannot recreate overlay ({reason})"
                        ),
                    })?;

                // Guardrail 2: on_image_change policy check
                (*policy != ImageChangePolicy::Protect || options.allow_recreate)
                    .then_some(())
                    .ok_or_else(|| LifecycleError::LifecyclePolicyViolation {
                        instance_name: instance_name.clone(),
                        details: format!(
                            "Instance '{instance_name}' base image changed with policy 'protect'; recreation requires --allow-recreate flag ({reason})"
                        ),
                    })?;

                // Graceful shutdown if running
                self.hypervisor
                    .domain_info(instance_name)
                    .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
                    .then(|| {
                        let _ = self.hypervisor.shutdown_domain(instance_name);
                    });

                // Undefine domain and clean up NVRAM
                let _ = self.hypervisor.undefine_domain(instance_name, true);

                // Unlink outdated overlay via storage manager
                let _ = self.storage.delete_image(overlay_path);

                // Cache new base image in storage pool
                self.storage
                    .inspect_image(base_cache_path)
                    .is_err()
                    .then(|| self.storage.copy_base_image(golden_master_path, base_cache_path))
                    .transpose()?;

                // Create fresh CoW overlay backed by new base image
                self.storage
                    .create_cow_overlay(base_cache_path, overlay_path)?;

                // Re-initialize fresh NVRAM vars
                nvram_template_path.exists().then(|| {
                    let _ = fs::copy(nvram_template_path, nvram_path);
                });

                // Define updated domain in Libvirt
                self.hypervisor.define_domain(concrete_xml)?;

                // Re-configure autostart
                autostart
                    .then(|| self.hypervisor.set_autostart(instance_name, true))
                    .transpose()?;

                // Synchronize storage pool
                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::RelocateStoragePool {
                instance_name,
                source_pool,
                target_pool,
                source_overlay_path,
                target_overlay_path,
                target_base_cache_path,
                golden_master_path,
                concrete_xml,
            } => {
                // Ensure VM is shut off before disk move
                self.hypervisor
                    .domain_info(instance_name)
                    .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
                    .then(|| self.hypervisor.shutdown_domain(instance_name))
                    .transpose()?;

                // Ensure golden master base image is cached in target pool
                self.storage
                    .inspect_image(target_base_cache_path)
                    .is_err()
                    .then(|| self.storage.copy_base_image(golden_master_path, target_base_cache_path))
                    .transpose()?;

                // Safely move overlay disk across storage pools
                self.storage
                    .move_file_safely(source_overlay_path, target_overlay_path)?;

                // Fast metadata-only rebase pointing to target pool base image
                self.storage
                    .rebase_overlay(target_overlay_path, target_base_cache_path, true)?;

                // Define updated domain pointing to target pool
                self.hypervisor.define_domain(concrete_xml)?;

                // Refresh both storage pools
                let _ = self.hypervisor.pool_refresh(source_pool);
                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::Delete {
                instance_name,
                target_pool,
                prevent_destroy,
            } => {
                (!*prevent_destroy)
                    .then_some(())
                    .ok_or_else(|| LifecycleError::LifecyclePolicyViolation {
                        instance_name: instance_name.clone(),
                        details: format!(
                            "Instance '{instance_name}' has prevent_destroy enabled; cannot delete"
                        ),
                    })?;

                self.hypervisor
                    .domain_info(instance_name)
                    .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
                    .then(|| {
                        let _ = self.hypervisor.shutdown_domain(instance_name);
                    });

                let _ = self.hypervisor.undefine_domain(instance_name, true);
                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::NoOp { .. } => Ok(()),
        }
    }
}
