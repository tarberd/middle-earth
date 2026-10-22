use crate::hypervisor::traits::{DomainState, Hypervisor, HypervisorError};
use crate::lifecycle::LifecycleError;
use crate::lifecycle::planner::{InstancePlanAction, OnehostPlan};
use crate::storage::traits::{StorageError, StorageManager};

/// Options controlling apply execution.
#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {}

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
        _options: &ApplyOptions,
    ) -> Result<(), LifecycleError> {
        match action {
            InstancePlanAction::Create {
                instance_name: _,
                concrete_xml,
                target_pool,
                overlay_path,
                base_cache_path,
                golden_master_path,
                nvram_path,
                nvram_template_path,
            } => {
                // Ensure base image is cached in target pool
                self.storage
                    .inspect_image(base_cache_path)
                    .is_err()
                    .then(|| {
                        self.storage
                            .copy_base_image(golden_master_path, base_cache_path)
                    })
                    .transpose()?;

                // Create fresh CoW overlay
                self.storage
                    .create_cow_overlay(base_cache_path, overlay_path)?;

                // Initialize NVRAM from template
                self.storage
                    .initialize_nvram(nvram_template_path, nvram_path)?;

                // Define domain in Libvirt
                self.hypervisor.define_domain(concrete_xml)?;

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
                concrete_xml,
                target_pool,
                overlay_path,
                base_cache_path,
                golden_master_path,
                nvram_path,
                nvram_template_path,
                ..
            } => {
                // Graceful shutdown if running
                self.hypervisor
                    .domain_info(instance_name)
                    .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
                    .then(|| self.hypervisor.shutdown_domain(instance_name))
                    .transpose()?;

                // Undefine domain and clean up NVRAM (idempotent if already not defined)
                match self.hypervisor.undefine_domain(instance_name, true) {
                    Ok(()) | Err(HypervisorError::DomainNotFound { .. }) => Ok(()),
                    Err(error) => Err(error),
                }?;

                // Unlink outdated overlay via storage manager
                match self.storage.delete_image(overlay_path) {
                    Ok(()) | Err(StorageError::SourceFileNotFound { .. }) => Ok(()),
                    Err(error) => Err(error),
                }?;

                // Cache new base image in storage pool
                self.storage
                    .inspect_image(base_cache_path)
                    .is_err()
                    .then(|| {
                        self.storage
                            .copy_base_image(golden_master_path, base_cache_path)
                    })
                    .transpose()?;

                // Create fresh CoW overlay backed by new base image
                self.storage
                    .create_cow_overlay(base_cache_path, overlay_path)?;

                // Re-initialize fresh NVRAM vars
                self.storage
                    .initialize_nvram(nvram_template_path, nvram_path)?;

                // Define updated domain in Libvirt
                self.hypervisor.define_domain(concrete_xml)?;

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
                    .then(|| {
                        self.storage
                            .copy_base_image(golden_master_path, target_base_cache_path)
                    })
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
                self.hypervisor.pool_refresh(source_pool)?;
                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::Delete {
                instance_name,
                target_pool,
            } => {
                self.hypervisor
                    .domain_info(instance_name)
                    .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
                    .then(|| self.hypervisor.shutdown_domain(instance_name))
                    .transpose()?;

                match self.hypervisor.undefine_domain(instance_name, true) {
                    Ok(()) | Err(HypervisorError::DomainNotFound { .. }) => Ok(()),
                    Err(error) => Err(error),
                }?;

                self.hypervisor.pool_refresh(target_pool)?;
                Ok(())
            }

            InstancePlanAction::NoOp { .. } => Ok(()),
        }
    }
}
