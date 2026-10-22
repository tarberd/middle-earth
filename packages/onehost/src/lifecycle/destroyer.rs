use crate::config::model::OnehostManifest;
use crate::hypervisor::traits::{DomainState, Hypervisor, HypervisorError};
use crate::image::tag::ContentAddressedImageResolver;
use crate::lifecycle::LifecycleError;
use crate::storage::traits::StorageManager;

/// Options controlling domain destruction.
#[derive(Debug, Clone, Default)]
pub struct DestroyOptions {
    pub force: bool,
    pub delete_disk: bool,
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

    /// Destroys a specified managed domain instance.
    pub fn destroy(
        &self,
        instance_name: &str,
        manifest: &OnehostManifest,
        options: &DestroyOptions,
    ) -> Result<(), LifecycleError> {
        let instance_configuration = manifest.instances.get(instance_name).ok_or_else(|| {
            LifecycleError::InstanceNotDeclared {
                instance_name: instance_name.to_string(),
            }
        })?;

        let effective_pool = instance_configuration.effective_pool(instance_name)?;
        let target_pool_directory = self.hypervisor.resolve_pool_path(effective_pool)?;

        // Stop running VM
        self.hypervisor
            .domain_info(instance_name)
            .is_ok_and(|domain_info| domain_info.state == DomainState::Running)
            .then(|| {
                if options.force {
                    self.hypervisor.destroy_domain(instance_name)
                } else {
                    self.hypervisor.shutdown_domain(instance_name)
                }
            })
            .transpose()?;

        // Undefine domain in Libvirt and clean up instance NVRAM (idempotent if already not defined)
        match self.hypervisor.undefine_domain(instance_name, true) {
            Ok(()) | Err(HypervisorError::DomainNotFound { .. }) => Ok(()),
            Err(error) => Err(error),
        }?;

        // Delete volatile instance CoW overlay if requested
        options
            .delete_disk
            .then(|| {
                let overlay_path = ContentAddressedImageResolver::resolve_instance_overlay_path(
                    &target_pool_directory,
                    instance_name,
                );

                self.storage.delete_image(&overlay_path)
            })
            .transpose()?;

        // Refresh storage pool volume inventory
        self.hypervisor.pool_refresh(effective_pool)?;

        Ok(())
    }
}
