use std::path::{Path, PathBuf};

use crate::backup::model::{BackupManifest, RestoreOptions};
use crate::backup::RestoreError;
use crate::config::model::OnehostManifest;
use crate::domain::element::DomainXmlElement;
use crate::hypervisor::traits::Hypervisor;
use crate::storage::traits::StorageManager;

/// Engine orchestrating single-command disaster recovery and instance restoration from backups.
pub struct RestoreEngine<'a, H: Hypervisor, S: StorageManager> {
    pub hypervisor: &'a H,
    pub storage: &'a S,
}

impl<'a, H: Hypervisor, S: StorageManager> RestoreEngine<'a, H, S> {
    /// Creates a new RestoreEngine.
    pub fn new(hypervisor: &'a H, storage: &'a S) -> Self {
        Self {
            hypervisor,
            storage,
        }
    }

    /// Restores a backed-up instance from a backup directory into the designated storage pool.
    pub fn restore_instance(
        &self,
        backup_directory: &Path,
        options: &RestoreOptions,
        manifest_config: Option<&OnehostManifest>,
    ) -> Result<(), RestoreError> {
        let manifest_content = self
            .storage
            .read_file(&backup_directory.join("manifest.json"))?;

        let manifest: BackupManifest = serde_json::from_str(&manifest_content)?;

        (!manifest.disks.is_empty())
            .then_some(())
            .ok_or_else(|| RestoreError::EmptyDisks {
                instance_name: manifest.instance_name.clone(),
            })?;

        let domain_exists = self.hypervisor.domain_info(&manifest.instance_name).is_ok();
        (options.allow_overwrite || !domain_exists)
            .then_some(())
            .ok_or_else(|| RestoreError::DomainAlreadyExists {
                instance_name: manifest.instance_name.clone(),
            })?;

        let target_pool = options
            .target_pool
            .as_deref()
            .or(manifest.storage_pool.as_deref())
            .or_else(|| {
                manifest_config.and_then(|config| {
                    config
                        .instances
                        .get(&manifest.instance_name)
                        .and_then(|instance| instance.effective_pool(&config.storage).ok())
                })
            })
            .ok_or(RestoreError::UnresolvedStoragePool)?;

        let pool_directory_path = self.hypervisor.resolve_pool_path(target_pool)?;

        let maybe_base_cache_path = if let Some(ref golden_master_filename) =
            manifest.golden_master_filename
        {
            let base_cache_path = pool_directory_path.join(golden_master_filename);

            let is_already_cached = self.storage.inspect_image(&base_cache_path).is_ok();
            if !is_already_cached {
                let depot_store_directory = options
                    .depot_store_dir
                    .as_deref()
                    .or_else(|| manifest_config.map(|config| config.storage.depot_store_dir.as_path()));

                let depot_master_path = depot_store_directory
                    .map(|directory| directory.join(golden_master_filename))
                    .ok_or_else(|| RestoreError::BaseImageNotFound {
                        path: base_cache_path.clone(),
                    })?;

                let depot_master_exists = self.storage.inspect_image(&depot_master_path).is_ok();
                depot_master_exists
                    .then_some(())
                    .ok_or_else(|| RestoreError::BaseImageNotFound {
                        path: depot_master_path.clone(),
                    })?;

                self.storage
                    .copy_base_image(&depot_master_path, &base_cache_path)?;
            }

            Some(base_cache_path)
        } else {
            None
        };

        manifest.disks.iter().try_for_each(|disk_entry| {
            let archive_path = backup_directory.join(&disk_entry.archive_filename);
            let destination_overlay_path = if disk_entry.target_device == "sda" {
                pool_directory_path.join(format!("{}.qcow2", manifest.instance_name))
            } else {
                pool_directory_path.join(format!(
                    "{}-{}.qcow2",
                    manifest.instance_name, disk_entry.target_device
                ))
            };

            let backing_path = disk_entry
                .backing_file
                .as_ref()
                .and(maybe_base_cache_path.as_deref());

            self.storage.restore_thin_backup(
                &archive_path,
                &destination_overlay_path,
                backing_path,
            )
        })?;

        let domain_xml = self
            .storage
            .read_file(&backup_directory.join(&manifest.domain_xml_filename))?;

        let parsed_domain = DomainXmlElement::parse(&domain_xml)?;
        let target_nvram_path = if let Some(ref nvram_directory) = options.nvram_dir {
            nvram_directory.join(format!("{}_VARS.fd", manifest.instance_name))
        } else {
            parsed_domain
                .path_text(&["os", "nvram"])
                .map(PathBuf::from)
                .ok_or(RestoreError::MissingNvramPath)?
        };

        self.storage.initialize_nvram(
            &backup_directory.join(&manifest.nvram_filename),
            &target_nvram_path,
        )?;

        self.hypervisor.define_domain(&domain_xml)?;

        self.hypervisor.pool_refresh(target_pool)?;

        Ok(())
    }
}
