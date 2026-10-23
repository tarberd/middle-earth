use std::path::{Path, PathBuf};
use tempfile::tempdir;

use onehost::backup::model::{
    BackupConsistencyLevel, BackupManifest, BackupOptions, DiskBackupEntry, RestoreOptions,
};
use onehost::backup::{BackupEngine, BackupError, RestoreEngine, RestoreError};
use onehost::config::model::InstanceUuid;
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost::storage::qemu_img::QemuImgStorage;
use onehost_test_support::FakeHypervisor;

fn create_sample_domain_xml(
    instance_name: &str,
    instance_uuid: &str,
    disk_path: &Path,
    nvram_path: &Path,
) -> String {
    format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid}</uuid>\
           <os>\
             <type arch='x86_64' machine='q35'>hvm</type>\
             <nvram>{}</nvram>\
           </os>\
           <devices>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='{}'/>\
               <target dev='sda' bus='sata'/>\
             </disk>\
           </devices>\
         </domain>",
        nvram_path.display(),
        disk_path.display()
    )
}

fn create_sample_multi_disk_domain_xml(
    instance_name: &str,
    instance_uuid: &str,
    disk1_path: &Path,
    disk2_path: &Path,
    nvram_path: &Path,
) -> String {
    format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid}</uuid>\
           <os>\
             <type arch='x86_64' machine='q35'>hvm</type>\
             <nvram>{}</nvram>\
           </os>\
           <devices>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='{}'/>\
               <target dev='sda' bus='sata'/>\
             </disk>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='{}'/>\
               <target dev='sdb' bus='virtio'/>\
             </disk>\
           </devices>\
         </domain>",
        nvram_path.display(),
        disk1_path.display(),
        disk2_path.display()
    )
}

fn create_sample_pool_xml(pool_name: &str, target_path: &Path) -> String {
    format!(
        "<pool type='dir'>\
           <name>{pool_name}</name>\
           <target>\
             <path>{}</path>\
           </target>\
         </pool>",
        target_path.display()
    )
}

#[test]
fn test_offline_backup_success() {
    let instance_name = "win11-offline";
    let instance_uuid_literal = "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d";
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let backup_directory = depot_directory.join("backup");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let base_path = depot_directory.join("win11-base.qcow2");
    storage
        .create_empty_disk(&base_path, 20 * 1024 * 1024)
        .expect("create base disk");

    let overlay_path = pool_directory.join(format!("{instance_name}.qcow2"));
    storage
        .create_cow_overlay(&base_path, &overlay_path)
        .expect("create overlay");

    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));
    std::fs::write(&nvram_path, b"NVRAM_OFFLINE_DATA").expect("write nvram");

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &overlay_path,
        &nvram_path,
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, &domain_xml);
    let engine = BackupEngine::new(&hypervisor, &storage);

    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T20:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, &backup_directory, &options, None)
        .expect("Offline backup should succeed");

    assert_eq!(backup_manifest.instance_name, instance_name);
    assert_eq!(
        backup_manifest.instance_uuid.as_str(),
        instance_uuid_literal
    );
    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::Offline
    );
    assert_eq!(backup_manifest.disks.len(), 1);
    assert_eq!(backup_manifest.disks[0].target_device, "sda");
    assert_eq!(backup_manifest.disks[0].archive_filename, "sda.qcow2");
    assert_eq!(
        backup_manifest.disks[0].virtual_size_bytes,
        20 * 1024 * 1024
    );

    // Physical file assertions
    let archive_path = backup_directory.join("sda.qcow2");
    assert!(archive_path.exists(), "Archive file must exist on disk");
    let inspection = storage
        .inspect_image(&archive_path)
        .expect("inspect archive");
    assert_eq!(inspection.format, "qcow2");
    assert_eq!(inspection.virtual_size_bytes, 20 * 1024 * 1024);
    assert_eq!(inspection.backing_file, Some(base_path));

    let nvram_backup = backup_directory.join("nvram.fd");
    assert!(nvram_backup.exists(), "NVRAM file must exist in backup");
    assert_eq!(
        std::fs::read(&nvram_backup).expect("read nvram"),
        b"NVRAM_OFFLINE_DATA"
    );

    let snapshot = hypervisor.snapshot();
    assert_eq!(
        snapshot
            .domains
            .get(instance_name)
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );
    let devices = &snapshot
        .domains
        .get(instance_name)
        .expect("block devices")
        .block_devices;
    assert_eq!(devices[0].source_file, Some(overlay_path));
    assert!(
        !pool_directory
            .join(format!("{instance_name}.sda.snap"))
            .exists(),
        "Offline backup should not create snapshots"
    );
}

#[test]
fn test_online_backup_vss_quiesced_success() {
    let instance_name = "win11-online";
    let instance_uuid_literal = "b2c3d4e5-f6a7-4b5c-8d9e-1f2a3b4c5d6e";
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let backup_directory = depot_directory.join("backup");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let base_path = depot_directory.join("win11-base.qcow2");
    storage
        .create_empty_disk(&base_path, 20 * 1024 * 1024)
        .expect("create base disk");

    let overlay_path = pool_directory.join(format!("{instance_name}.qcow2"));
    storage
        .create_cow_overlay(&base_path, &overlay_path)
        .expect("create overlay");

    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));
    std::fs::write(&nvram_path, b"NVRAM_ONLINE_DATA").expect("write nvram");

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &overlay_path,
        &nvram_path,
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(8),
        memory_kib: Some(16_777_216),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, &domain_xml);
    let engine = BackupEngine::new(&hypervisor, &storage);

    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T21:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, &backup_directory, &options, None)
        .expect("Online backup should succeed");

    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::VssQuiesced
    );

    let snapshot = hypervisor.snapshot();
    assert_eq!(
        snapshot
            .domains
            .get(instance_name)
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );
    let devices = &snapshot
        .domains
        .get(instance_name)
        .expect("block devices")
        .block_devices;
    assert_eq!(devices[0].source_file, Some(overlay_path.clone()));
    assert!(
        !pool_directory
            .join(format!("{instance_name}.sda.snap"))
            .exists(),
        "temporary snapshot overlay must be collapsed and unlinked"
    );

    // Physical disk state verification
    let archive_path = backup_directory.join("sda.qcow2");
    assert!(archive_path.exists());
    assert!(storage.check_image(&archive_path).is_ok());

    let nvram_backup = backup_directory.join("nvram.fd");
    assert!(nvram_backup.exists());

    let snapshot_path = pool_directory.join(format!("{instance_name}.sda.snap"));
    assert!(
        !snapshot_path.exists(),
        "Temporary .snap overlay should be unlinked after successful blockcommit"
    );
}

#[test]
fn test_online_backup_quiesce_fallback_to_crash_consistent() {
    let instance_name = "win11-fallback";
    let instance_uuid_literal = "c3d4e5f6-a7b8-4c5d-9e1f-2a3b4c5d6e7f";
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let backup_directory = depot_directory.join("backup");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let base_path = depot_directory.join("win11-base.qcow2");
    storage
        .create_empty_disk(&base_path, 20 * 1024 * 1024)
        .expect("create base disk");

    let overlay_path = pool_directory.join(format!("{instance_name}.qcow2"));
    storage
        .create_cow_overlay(&base_path, &overlay_path)
        .expect("create overlay");

    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));
    std::fs::write(&nvram_path, b"NVRAM_FALLBACK_DATA").expect("write nvram");

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &overlay_path,
        &nvram_path,
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new()
        .with_domain(domain_info, &domain_xml)
        .with_quiesce_failure(true);

    let engine = BackupEngine::new(&hypervisor, &storage);

    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T22:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, &backup_directory, &options, None)
        .expect("Backup should succeed via crash-consistent fallback");

    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::CrashConsistent
    );

    let snapshot = hypervisor.snapshot();
    assert_eq!(
        snapshot
            .domains
            .get(instance_name)
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );
    let devices = &snapshot
        .domains
        .get(instance_name)
        .expect("block devices")
        .block_devices;
    assert_eq!(devices[0].source_file, Some(overlay_path.clone()));

    // Physical disk state verification
    let archive_path = backup_directory.join("sda.qcow2");
    assert!(archive_path.exists());
    assert!(storage.check_image(&archive_path).is_ok());

    let nvram_backup = backup_directory.join("nvram.fd");
    assert!(nvram_backup.exists());

    let snapshot_path = pool_directory.join(format!("{instance_name}.sda.snap"));
    assert!(!snapshot_path.exists());
}

#[test]
fn test_online_backup_raii_guard_triggers_blockcommit_on_failure() {
    let instance_name = "win11-raii-test";
    let instance_uuid_literal = "d4e5f6a7-b8c9-4d5e-1f2a-3b4c5d6e7f8a";
    let temporary_directory = tempdir().expect("create tempdir");
    let pool_directory = temporary_directory.path().join("pool");
    let backup_directory = temporary_directory.path().join("backup");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    // The domain disk overlay_path is deliberately absent from disk so that
    // inspect_image fails after snapshot creation, triggering the RAII guard blockcommit.
    let overlay_path = pool_directory.join(format!("{instance_name}.qcow2"));

    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));
    std::fs::write(&nvram_path, b"NVRAM_RAII_DATA").expect("write nvram");

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &overlay_path,
        &nvram_path,
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, &domain_xml);
    let engine = BackupEngine::new(&hypervisor, &storage);

    let options = BackupOptions::default();

    let backup_result = engine.backup_instance(instance_name, &backup_directory, &options, None);

    assert!(
        backup_result.is_err(),
        "Backup must fail when storage conversion errors"
    );

    let snapshot = hypervisor.snapshot();
    let devices = &snapshot
        .domains
        .get(instance_name)
        .expect("block devices")
        .block_devices;
    assert_eq!(
        devices[0].source_file,
        Some(overlay_path),
        "RAII guard must execute blockcommit when conversion fails to restore base disk"
    );
}

#[test]
fn test_multi_disk_atomic_snapshot_backup() {
    let instance_name = "win11-multidisk";
    let instance_uuid_literal = "e5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a9b";
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let backup_directory = depot_directory.join("backup");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let base_path = depot_directory.join("win11-base.qcow2");
    storage
        .create_empty_disk(&base_path, 20 * 1024 * 1024)
        .expect("create base disk");

    let disk1_path = pool_directory.join(format!("{instance_name}.qcow2"));
    storage
        .create_cow_overlay(&base_path, &disk1_path)
        .expect("create disk1 overlay");

    let disk2_path = pool_directory.join(format!("{instance_name}_data.qcow2"));
    storage
        .create_empty_disk(&disk2_path, 10 * 1024 * 1024)
        .expect("create disk2");

    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));
    std::fs::write(&nvram_path, b"NVRAM_MULTIDISK_DATA").expect("write nvram");

    let domain_xml = create_sample_multi_disk_domain_xml(
        instance_name,
        instance_uuid_literal,
        &disk1_path,
        &disk2_path,
        &nvram_path,
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(8),
        memory_kib: Some(16_777_216),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, &domain_xml);
    let engine = BackupEngine::new(&hypervisor, &storage);

    let options = BackupOptions::default();

    let backup_manifest = engine
        .backup_instance(instance_name, &backup_directory, &options, None)
        .expect("Multi-disk backup should succeed");

    assert_eq!(backup_manifest.disks.len(), 2);

    let snapshot = hypervisor.snapshot();
    let devices = &snapshot
        .domains
        .get(instance_name)
        .expect("block devices")
        .block_devices;
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].source_file, Some(disk1_path.clone()));
    assert_eq!(devices[1].source_file, Some(disk2_path.clone()));

    // Physical disk state verification
    let sda_archive = backup_directory.join("sda.qcow2");
    let sdb_archive = backup_directory.join("sdb.qcow2");
    assert!(sda_archive.exists());
    assert!(sdb_archive.exists());
    assert!(storage.check_image(&sda_archive).is_ok());
    assert!(storage.check_image(&sdb_archive).is_ok());

    let nvram_backup = backup_directory.join("nvram.fd");
    assert!(nvram_backup.exists());

    let snap1 = pool_directory.join(format!("{instance_name}.sda.snap"));
    let snap2 = pool_directory.join(format!("{instance_name}_data.sdb.snap"));
    assert!(!snap1.exists(), "Snapshot 1 overlay must be unlinked");
    assert!(!snap2.exists(), "Snapshot 2 overlay must be unlinked");
}

#[test]
fn test_backup_fails_when_no_disks_attached() {
    let instance_name = "win11-nodisks";
    let instance_uuid_literal = "f5e4d3c2-b1a0-4f9e-8d7c-6b5a4e3d2c1b";
    let temporary_directory = tempdir().expect("create tempdir");
    let nvram_path = temporary_directory.path().join("win11-nodisks_VARS.fd");
    std::fs::write(&nvram_path, b"NVRAM").expect("write nvram");

    let domain_xml = format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid_literal}</uuid>\
           <os>\
             <type arch='x86_64' machine='q35'>hvm</type>\
             <nvram>{}</nvram>\
           </os>\
           <devices>\
           </devices>\
         </domain>",
        nvram_path.display()
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(2),
        memory_kib: Some(4_194_304),
        autostart: false,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, domain_xml);
    let storage = QemuImgStorage::new();
    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = temporary_directory.path().join("backup");
    let options = BackupOptions::default();

    let backup_result = engine.backup_instance(instance_name, &backup_directory, &options, None);

    assert!(
        matches!(backup_result, Err(BackupError::NoDisksFound { .. })),
        "Backup must fail with NoDisksFound when VM has no disks"
    );
}

#[test]
fn test_restore_engine_success() {
    let instance_name = "win11-restore";
    let instance_uuid_literal = "01234567-89ab-4cde-f012-3456789abcde";
    let temporary_directory = tempdir().expect("create tempdir");
    let backup_directory = temporary_directory.path().join("backup");
    let pool_directory = temporary_directory.path().join("pool");
    let depot_store_directory = temporary_directory.path().join("depot");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&depot_store_directory).expect("create depot dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let pool_name = "default";
    let pool_xml = create_sample_pool_xml(pool_name, &pool_directory);
    let hypervisor = FakeHypervisor::new().with_pool_xml(pool_name, &pool_xml);

    let storage = QemuImgStorage::new();

    let golden_master_filename = "win11-26300-looking-glass-hash123.qcow2";
    let depot_golden_master_path = depot_store_directory.join(golden_master_filename);
    storage
        .create_empty_disk(&depot_golden_master_path, 20 * 1024 * 1024)
        .expect("create depot golden master");

    let pool_golden_master_path = pool_directory.join(golden_master_filename);

    let overlay_disk_path = pool_directory.join(format!("{instance_name}.qcow2"));
    let nvram_target_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &overlay_disk_path,
        &nvram_target_path,
    );

    let backup_sda_path = backup_directory.join("sda.qcow2");
    storage
        .create_empty_disk(&backup_sda_path, 20 * 1024 * 1024)
        .expect("create backup sda disk");

    let manifest = BackupManifest {
        instance_name: instance_name.to_string(),
        instance_uuid: InstanceUuid::parse(instance_uuid_literal).expect("Valid UUID"),
        timestamp: "2026-09-26T20:00:00Z".to_string(),
        consistency_level: BackupConsistencyLevel::VssQuiesced,
        storage_pool: Some(pool_name.to_string()),
        base_image_tag: Some("win11/26300/looking-glass".to_string()),
        base_image_hash: Some("hash123".to_string()),
        golden_master_filename: Some(golden_master_filename.to_string()),
        disks: vec![DiskBackupEntry {
            target_device: "sda".to_string(),
            archive_filename: "sda.qcow2".to_string(),
            virtual_size_bytes: 20 * 1024 * 1024,
            archive_size_bytes: 1_500_000,
            backing_file: Some(pool_golden_master_path.clone()),
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    std::fs::write(backup_directory.join("manifest.json"), manifest_json).expect("write manifest");
    std::fs::write(backup_directory.join("domain.xml"), &domain_xml).expect("write domain xml");
    std::fs::write(backup_directory.join("nvram.fd"), b"NVRAM_BINARY_DATA").expect("write nvram");

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        target_pool: Some(pool_name.to_string()),
        depot_store_dir: Some(depot_store_directory),
        nvram_dir: None,
        allow_overwrite: false,
    };

    let restore_result = restore_engine.restore_instance(&backup_directory, &options, None);
    assert!(
        restore_result.is_ok(),
        "Restore should succeed without errors"
    );

    // Verify physical storage state
    assert!(
        pool_golden_master_path.exists(),
        "Base image should be copied to target pool cache"
    );
    assert!(storage.inspect_image(&pool_golden_master_path).is_ok());

    assert!(
        overlay_disk_path.exists(),
        "Restored overlay disk must exist in target pool"
    );
    let restored_inspection = storage
        .inspect_image(&overlay_disk_path)
        .expect("inspect restored overlay");
    assert_eq!(restored_inspection.format, "qcow2");
    assert_eq!(
        restored_inspection.backing_file,
        Some(pool_golden_master_path)
    );

    assert!(
        nvram_target_path.exists(),
        "NVRAM should be restored to host path from domain XML"
    );
    assert_eq!(
        std::fs::read(&nvram_target_path).expect("read nvram"),
        b"NVRAM_BINARY_DATA"
    );

    let snapshot = hypervisor.snapshot();
    assert!(
        snapshot.domains.contains_key(instance_name),
        "Domain should be registered with hypervisor"
    );
    assert_eq!(
        snapshot
            .domains
            .get(instance_name)
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );
    assert!(
        snapshot
            .domains
            .get(instance_name)
            .is_some_and(|entry| !entry.xml.is_empty()),
        "Domain XML should be defined in hypervisor"
    );
    assert!(
        snapshot.storage_pools.contains_key(pool_name),
        "Target storage pool should exist in hypervisor"
    );
}

#[test]
fn test_restore_engine_rejects_already_existing_domain() {
    let instance_name = "win11-existing";
    let instance_uuid_literal = "12345678-9abc-4def-0123-456789abcdef";
    let temporary_directory = tempdir().expect("create tempdir");
    let backup_directory = temporary_directory.path().join("backup");
    let pool_directory = temporary_directory.path().join("pool");
    let nvram_directory = temporary_directory.path().join("nvram");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let disk_path = pool_directory.join("existing.qcow2");
    let nvram_path = nvram_directory.join("existing_VARS.fd");

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &disk_path,
        &nvram_path,
    );
    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new().with_domain(domain_info, &domain_xml);

    let manifest = BackupManifest {
        instance_name: instance_name.to_string(),
        instance_uuid: InstanceUuid::parse(instance_uuid_literal).expect("Valid UUID"),
        timestamp: "2026-09-26T20:00:00Z".to_string(),
        consistency_level: BackupConsistencyLevel::Offline,
        storage_pool: Some("default".to_string()),
        base_image_tag: None,
        base_image_hash: None,
        golden_master_filename: None,
        disks: vec![DiskBackupEntry {
            target_device: "sda".to_string(),
            archive_filename: "sda.qcow2".to_string(),
            virtual_size_bytes: 20 * 1024 * 1024,
            archive_size_bytes: 1_000_000,
            backing_file: None,
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    std::fs::write(backup_directory.join("manifest.json"), manifest_json).expect("write manifest");

    let storage = QemuImgStorage::new();
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        allow_overwrite: false,
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(&backup_directory, &options, None);

    assert!(
        matches!(
            restore_result,
            Err(RestoreError::DomainAlreadyExists { .. })
        ),
        "Restore must reject overwriting existing domain when allow_overwrite is false"
    );
}

#[test]
fn test_restore_engine_allows_overwrite_when_flag_set() {
    let instance_name = "win11-overwrite";
    let instance_uuid_literal = "23456789-abcd-4ef0-1234-56789abcdef0";
    let temporary_directory = tempdir().expect("create tempdir");
    let backup_directory = temporary_directory.path().join("backup");
    let pool_directory = temporary_directory.path().join("pool");
    let nvram_directory = temporary_directory.path().join("nvram");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let pool_name = "default";
    let pool_xml = create_sample_pool_xml(pool_name, &pool_directory);

    let disk_path = pool_directory.join(format!("{instance_name}.qcow2"));
    let nvram_path = nvram_directory.join(format!("{instance_name}_VARS.fd"));

    let domain_xml = create_sample_domain_xml(
        instance_name,
        instance_uuid_literal,
        &disk_path,
        &nvram_path,
    );
    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = FakeHypervisor::new()
        .with_domain(domain_info, &domain_xml)
        .with_pool_xml(pool_name, &pool_xml);

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&backup_directory.join("sda.qcow2"), 20 * 1024 * 1024)
        .expect("create sda disk");

    let manifest = BackupManifest {
        instance_name: instance_name.to_string(),
        instance_uuid: InstanceUuid::parse(instance_uuid_literal).expect("Valid UUID"),
        timestamp: "2026-09-26T20:00:00Z".to_string(),
        consistency_level: BackupConsistencyLevel::Offline,
        storage_pool: Some(pool_name.to_string()),
        base_image_tag: None,
        base_image_hash: None,
        golden_master_filename: None,
        disks: vec![DiskBackupEntry {
            target_device: "sda".to_string(),
            archive_filename: "sda.qcow2".to_string(),
            virtual_size_bytes: 20 * 1024 * 1024,
            archive_size_bytes: 1_000_000,
            backing_file: None,
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    std::fs::write(backup_directory.join("manifest.json"), manifest_json).expect("write manifest");
    std::fs::write(backup_directory.join("domain.xml"), &domain_xml).expect("write domain xml");
    std::fs::write(backup_directory.join("nvram.fd"), b"NVRAM_DATA").expect("write nvram");

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        allow_overwrite: true,
        target_pool: Some(pool_name.to_string()),
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(&backup_directory, &options, None);
    assert!(
        restore_result.is_ok(),
        "Restore must proceed when allow_overwrite is true"
    );
    assert!(disk_path.exists());
    assert!(nvram_path.exists());
}

#[test]
fn test_restore_engine_fails_when_base_image_not_found() {
    let instance_name = "win11-missing-base";
    let instance_uuid_literal = "3456789a-bcde-4f01-2345-6789abcdef01";
    let temporary_directory = tempdir().expect("create tempdir");
    let backup_directory = temporary_directory.path().join("backup");
    let pool_directory = temporary_directory.path().join("pool");
    let depot_store_dir = temporary_directory.path().join("depot");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&depot_store_dir).expect("create depot dir");

    let storage = QemuImgStorage::new();
    let pool_name = "default";
    let pool_xml = create_sample_pool_xml(pool_name, &pool_directory);

    let hypervisor = FakeHypervisor::new().with_pool_xml(pool_name, &pool_xml);

    let manifest = BackupManifest {
        instance_name: instance_name.to_string(),
        instance_uuid: InstanceUuid::parse(instance_uuid_literal).expect("Valid UUID"),
        timestamp: "2026-09-26T20:00:00Z".to_string(),
        consistency_level: BackupConsistencyLevel::Offline,
        storage_pool: Some(pool_name.to_string()),
        base_image_tag: Some("win11/26300/looking-glass".to_string()),
        base_image_hash: Some("hash123".to_string()),
        golden_master_filename: Some("missing-master.qcow2".to_string()),
        disks: vec![DiskBackupEntry {
            target_device: "sda".to_string(),
            archive_filename: "sda.qcow2".to_string(),
            virtual_size_bytes: 20 * 1024 * 1024,
            archive_size_bytes: 1_000_000,
            backing_file: Some(PathBuf::from(format!(
                "{}/missing-master.qcow2",
                pool_directory.display()
            ))),
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    std::fs::write(backup_directory.join("manifest.json"), manifest_json).expect("write manifest");
    storage
        .create_empty_disk(&backup_directory.join("sda.qcow2"), 20 * 1024 * 1024)
        .expect("create sda disk");

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        target_pool: Some(pool_name.to_string()),
        depot_store_dir: Some(depot_store_dir),
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(&backup_directory, &options, None);

    assert!(
        matches!(restore_result, Err(RestoreError::BaseImageNotFound { .. })),
        "Restore must fail when golden master base image is missing from store"
    );
}

#[test]
fn test_restore_engine_empty_disks_validation() {
    let instance_name = "win11-empty-disks";
    let instance_uuid_literal = "456789ab-cdef-4012-3456-789abcdef012";
    let temporary_directory = tempdir().expect("create tempdir");
    let backup_directory = temporary_directory.path().join("backup");
    std::fs::create_dir_all(&backup_directory).expect("create backup dir");

    let hypervisor = FakeHypervisor::new();

    let manifest = BackupManifest {
        instance_name: instance_name.to_string(),
        instance_uuid: InstanceUuid::parse(instance_uuid_literal).expect("Valid UUID"),
        timestamp: "2026-09-26T20:00:00Z".to_string(),
        consistency_level: BackupConsistencyLevel::Offline,
        storage_pool: Some("default".to_string()),
        base_image_tag: None,
        base_image_hash: None,
        golden_master_filename: None,
        disks: Vec::new(),
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    std::fs::write(backup_directory.join("manifest.json"), manifest_json).expect("write manifest");

    let storage = QemuImgStorage::new();
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions::default();

    let restore_result = restore_engine.restore_instance(&backup_directory, &options, None);

    assert!(
        matches!(restore_result, Err(RestoreError::EmptyDisks { .. })),
        "Restore must fail when manifest has no disk entries"
    );
}
