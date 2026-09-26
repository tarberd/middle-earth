use std::path::{Path, PathBuf};

use crate::backup::guard::{ActiveSnapshotInfo, SnapshotCleanupGuard};
use crate::backup::model::{
    BackupConsistencyLevel, BackupManifest, BackupOptions, DiskBackupEntry,
};
use crate::backup::BackupError;
use crate::config::model::{InstanceUuid, OnehostManifest};
use crate::domain::element::DomainXmlElement;
use crate::hypervisor::traits::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainState, Hypervisor,
};
use crate::storage::traits::{StorageError, StorageManager};

/// Engine orchestrating online and offline thin backup workflows.
pub struct BackupEngine<'a, H: Hypervisor, S: StorageManager> {
    pub hypervisor: &'a H,
    pub storage: &'a S,
}

impl<'a, H: Hypervisor, S: StorageManager> BackupEngine<'a, H, S> {
    /// Creates a new BackupEngine.
    pub fn new(hypervisor: &'a H, storage: &'a S) -> Self {
        Self {
            hypervisor,
            storage,
        }
    }

    /// Backs up a declared instance to the target backup directory.
    pub fn backup_instance(
        &self,
        instance_name: &str,
        target_backup_directory: &Path,
        options: &BackupOptions,
        manifest_config: Option<&OnehostManifest>,
    ) -> Result<BackupManifest, BackupError> {
        let domain_info = self.hypervisor.domain_info(instance_name)?;

        let block_devices = self.hypervisor.list_block_devices(instance_name)?;
        let disk_devices: Vec<BlockDeviceInfo> = block_devices
            .into_iter()
            .filter(|device| device.source_file.is_some())
            .collect();

        (!disk_devices.is_empty())
            .then_some(())
            .ok_or_else(|| BackupError::NoDisksFound {
                instance_name: instance_name.to_string(),
            })?;

        let (consistency_level, disk_entries) = match domain_info.state {
            DomainState::Running => self.perform_online_backup(
                instance_name,
                &disk_devices,
                target_backup_directory,
                options,
            ),
            DomainState::Shutoff => self.perform_offline_backup(
                &disk_devices,
                target_backup_directory,
                options,
            ),
            unsupported_state => Err(BackupError::UnsupportedDomainState {
                instance_name: instance_name.to_string(),
                state: unsupported_state,
            }),
        }?;

        let is_domain_shutoff = domain_info.state == DomainState::Shutoff;
        let raw_domain_xml = self.hypervisor.dump_xml(instance_name, is_domain_shutoff)?;
        self.storage
            .write_file(&target_backup_directory.join("domain.xml"), &raw_domain_xml)?;

        let parsed_domain = DomainXmlElement::parse(&raw_domain_xml)?;

        let raw_instance_uuid = parsed_domain
            .path_text(&["uuid"])
            .ok_or_else(|| BackupError::MissingInstanceUuid {
                instance_name: instance_name.to_string(),
            })?;
        let instance_uuid = InstanceUuid::parse(raw_instance_uuid)?;

        let nvram_source_path = parsed_domain
            .path_text(&["os", "nvram"])
            .map(PathBuf::from)
            .ok_or_else(|| BackupError::MissingNvramPath {
                instance_name: instance_name.to_string(),
            })?;

        self.storage.initialize_nvram(
            &nvram_source_path,
            &target_backup_directory.join("nvram.fd"),
        )?;

        let instance_declaration = manifest_config
            .and_then(|config| config.instances.get(instance_name));

        let base_image_tag = instance_declaration.map(|declaration| declaration.image.to_string());
        let base_image_hash = instance_declaration.and_then(|declaration| {
            manifest_config.and_then(|config| {
                config
                    .flavors
                    .get(&declaration.image.flavor_name)
                    .map(|flavor| flavor.hash.clone())
            })
        });

        let golden_master_filename = match (instance_declaration, &base_image_hash) {
            (Some(declaration), Some(hash)) => Some(format!(
                "{}-{}-{}-{}.qcow2",
                declaration.image.operating_system,
                declaration.image.build_version,
                declaration.image.flavor_name,
                hash
            )),
            _ => disk_entries
                .iter()
                .find_map(|disk| disk.backing_file.as_ref())
                .and_then(|backing_file| backing_file.file_name())
                .and_then(|file_name| file_name.to_str())
                .map(ToString::to_string),
        };

        let storage_pool = instance_declaration.and_then(|declaration| {
            manifest_config.and_then(|config| {
                declaration
                    .effective_pool(&config.storage)
                    .ok()
                    .map(ToString::to_string)
            })
        });

        let timestamp = options
            .timestamp
            .clone()
            .unwrap_or_else(generate_utc_timestamp);

        let manifest = BackupManifest {
            instance_name: instance_name.to_string(),
            instance_uuid,
            timestamp,
            consistency_level,
            storage_pool,
            base_image_tag,
            base_image_hash,
            golden_master_filename,
            disks: disk_entries,
            domain_xml_filename: "domain.xml".to_string(),
            nvram_filename: "nvram.fd".to_string(),
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        self.storage
            .write_file(&target_backup_directory.join("manifest.json"), &manifest_json)?;

        Ok(manifest)
    }

    fn perform_online_backup(
        &self,
        instance_name: &str,
        disk_devices: &[BlockDeviceInfo],
        target_backup_directory: &Path,
        options: &BackupOptions,
    ) -> Result<(BackupConsistencyLevel, Vec<DiskBackupEntry>), BackupError> {
        let active_snapshots: Vec<ActiveSnapshotInfo> = disk_devices
            .iter()
            .filter_map(|device| {
                let base_path = device.source_file.clone()?;
                let parent_directory = base_path.parent().unwrap_or_else(|| Path::new("."));
                let snapshot_path = parent_directory
                    .join(format!("{}.{}.snap", instance_name, device.target_device));

                Some(ActiveSnapshotInfo {
                    target_device: device.target_device.clone(),
                    base_path,
                    snapshot_path,
                })
            })
            .collect();

        let disk_specifications: Vec<DiskSnapshotSpecification> = active_snapshots
            .iter()
            .map(|snapshot| DiskSnapshotSpecification {
                target_device: snapshot.target_device.clone(),
                snapshot_file_path: snapshot.snapshot_path.clone(),
            })
            .collect();

        let snapshot_name = format!("backup-{instance_name}");

        let consistency_level = if options.quiesce {
            match self.hypervisor.create_snapshot(
                instance_name,
                &snapshot_name,
                &disk_specifications,
                true,
            ) {
                Ok(()) => BackupConsistencyLevel::VssQuiesced,
                Err(quiesce_error) => {
                    tracing::warn!(
                        instance_name = %instance_name,
                        error = %quiesce_error,
                        "VSS quiesced snapshot failed; falling back to crash-consistent snapshot"
                    );

                    self.hypervisor.create_snapshot(
                        instance_name,
                        &snapshot_name,
                        &disk_specifications,
                        false,
                    )?;

                    BackupConsistencyLevel::CrashConsistent
                }
            }
        } else {
            self.hypervisor.create_snapshot(
                instance_name,
                &snapshot_name,
                &disk_specifications,
                false,
            )?;

            BackupConsistencyLevel::CrashConsistent
        };

        let mut cleanup_guard =
            SnapshotCleanupGuard::new(self.hypervisor, instance_name, active_snapshots);

        let disk_entries: Vec<DiskBackupEntry> = cleanup_guard
            .snapshots()
            .iter()
            .map(|snapshot| {
                let archive_filename = format!("{}.qcow2", snapshot.target_device);
                let staging_archive_tmp =
                    target_backup_directory.join(format!(".{archive_filename}.tmp"));
                let final_archive_path = target_backup_directory.join(&archive_filename);

                let inspection_info = self.storage.inspect_image(&snapshot.base_path)?;

                self.storage.convert_thin_backup(
                    &snapshot.base_path,
                    &staging_archive_tmp,
                    inspection_info.backing_file.as_deref(),
                    options.compress,
                )?;

                self.storage
                    .move_file_safely(&staging_archive_tmp, &final_archive_path)?;

                let archive_inspection = self.storage.inspect_image(&final_archive_path)?;

                Ok(DiskBackupEntry {
                    target_device: snapshot.target_device.clone(),
                    archive_filename,
                    virtual_size_bytes: inspection_info.virtual_size_bytes,
                    archive_size_bytes: archive_inspection.actual_size_bytes,
                    backing_file: inspection_info.backing_file,
                })
            })
            .collect::<Result<Vec<DiskBackupEntry>, StorageError>>()?;

        cleanup_guard.snapshots().iter().try_for_each(|snapshot| {
            self.hypervisor.blockcommit(
                instance_name,
                &snapshot.target_device,
                Some(&snapshot.base_path),
                Some(&snapshot.snapshot_path),
                true,
                true,
            )?;

            self.storage.delete_image(&snapshot.snapshot_path)?;
            Ok::<(), BackupError>(())
        })?;

        cleanup_guard.disarm();

        Ok((consistency_level, disk_entries))
    }

    fn perform_offline_backup(
        &self,
        disk_devices: &[BlockDeviceInfo],
        target_backup_directory: &Path,
        options: &BackupOptions,
    ) -> Result<(BackupConsistencyLevel, Vec<DiskBackupEntry>), BackupError> {
        let disk_entries: Vec<DiskBackupEntry> = disk_devices
            .iter()
            .map(|device| {
                let source_file_path = device.source_file.as_deref().ok_or_else(|| {
                    StorageError::SourceFileNotFound {
                        path: PathBuf::from(&device.target_device),
                    }
                })?;

                let archive_filename = format!("{}.qcow2", device.target_device);
                let staging_archive_tmp =
                    target_backup_directory.join(format!(".{archive_filename}.tmp"));
                let final_archive_path = target_backup_directory.join(&archive_filename);

                let inspection_info = self.storage.inspect_image(source_file_path)?;

                self.storage.convert_thin_backup(
                    source_file_path,
                    &staging_archive_tmp,
                    inspection_info.backing_file.as_deref(),
                    options.compress,
                )?;

                self.storage
                    .move_file_safely(&staging_archive_tmp, &final_archive_path)?;

                let archive_inspection = self.storage.inspect_image(&final_archive_path)?;

                Ok(DiskBackupEntry {
                    target_device: device.target_device.clone(),
                    archive_filename,
                    virtual_size_bytes: inspection_info.virtual_size_bytes,
                    archive_size_bytes: archive_inspection.actual_size_bytes,
                    backing_file: inspection_info.backing_file,
                })
            })
            .collect::<Result<Vec<DiskBackupEntry>, StorageError>>()?;

        Ok((BackupConsistencyLevel::Offline, disk_entries))
    }
}

fn generate_utc_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let epoch_seconds = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format_epoch_seconds_to_iso8601(epoch_seconds)
}

fn format_epoch_seconds_to_iso8601(epoch_seconds: u64) -> String {
    let seconds_in_day = 86_400;
    let day_number = epoch_seconds / seconds_in_day;
    let seconds_remaining = epoch_seconds % seconds_in_day;

    let hour = seconds_remaining / 3600;
    let minute = (seconds_remaining % 3600) / 60;
    let second = seconds_remaining % 60;

    let z = day_number + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_year = if m <= 2 { y + 1 } else { y };

    format!("{final_year:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}
