use crate::config::model::OnehostManifest;
use crate::hypervisor::traits::{DomainState, Hypervisor};
use crate::image::tag::ContentAddressedImageResolver;
use crate::lifecycle::LifecycleError;
use crate::storage::traits::StorageManager;

/// Options controlling domain destruction.
#[derive(Debug, Clone, Default)]
pub struct DestroyOptions {
    pub force: bool,
    pub delete_disk: bool,
    pub allow_destroy_protected: bool,
}

/// Destroyer handling safe deprovisioning of managed domain instances.
pub struct DomainLifecycleDestroyer<'a, H: Hypervisor, S: StorageManager> {
    pub hypervisor: &'a H,
    pub storage: &'a S,
}

impl<'a, H: Hypervisor, S: StorageManager> DomainLifecycleDestroyer<'a, H, S> {
    /// Creates a new DomainLifecycleDestroyer.
    pub fn new(hypervisor: &'a H, storage: &'a S) -> Self {
        Self {
            hypervisor,
            storage,
        }
    }

    /// Destroys a specified managed domain instance adhering strictly to lifecycle guardrails.
    pub fn destroy(
        &self,
        instance_name: &str,
        manifest: &OnehostManifest,
        options: &DestroyOptions,
    ) -> Result<(), LifecycleError> {
        let instance_configuration = manifest
            .instances
            .get(instance_name)
            .ok_or_else(|| LifecycleError::ConfigurationError {
                details: format!("Instance '{instance_name}' is not declared in the manifest"),
            })?;

        // Lifecycle guardrail: prevent_destroy check
        if instance_configuration.lifecycle.prevent_destroy && !options.allow_destroy_protected {
            return Err(LifecycleError::LifecyclePolicyViolation {
                instance_name: instance_name.to_string(),
                details: format!(
                    "Instance '{instance_name}' has prevent_destroy enabled; destruction aborted. Use --allow-destroy-protected to override."
                ),
            });
        }

        let effective_pool = instance_configuration
            .effective_pool(&manifest.storage)
            .map_err(|validation_error| LifecycleError::ConfigurationError {
                details: validation_error.to_string(),
            })?;
        let target_pool_directory = self.hypervisor.resolve_pool_path(effective_pool)?;

        // Stop running VM
        if self
            .hypervisor
            .domain_info(instance_name)
            .is_ok_and(|info| info.state == DomainState::Running)
        {
            if options.force {
                self.hypervisor.destroy_domain(instance_name)?;
            } else {
                self.hypervisor.shutdown_domain(instance_name)?;
            }
        }

        // Undefine domain in Libvirt and clean up instance NVRAM
        let _ = self.hypervisor.undefine_domain(instance_name, true);

        // Delete volatile instance CoW overlay if requested
        if options.delete_disk {
            let overlay_path = ContentAddressedImageResolver::resolve_instance_overlay_path(
                &target_pool_directory,
                instance_name,
            );

            self.storage.delete_image(&overlay_path)?;
        }

        // Refresh storage pool volume inventory
        self.hypervisor.pool_refresh(effective_pool)?;

        Ok(())
    }
}
