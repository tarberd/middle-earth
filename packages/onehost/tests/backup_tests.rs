use std::path::{Path, PathBuf};

use onehost::backup::model::{
    BackupConsistencyLevel, BackupManifest, BackupOptions, DiskBackupEntry, RestoreOptions,
};
use onehost::backup::{BackupEngine, BackupError, RestoreEngine, RestoreError};
use onehost::config::model::InstanceUuid;
use onehost::hypervisor::mock::{MockHypervisor, RecordedHypervisorAction};
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost::storage::mock::{MockImageRecord, MockStorageManager, RecordedStorageAction};

fn create_sample_domain_xml(instance_name: &str, instance_uuid: &str) -> String {
    format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid}</uuid>\
           <os>\
             <type arch='x86_64' machine='q35'>hvm</type>\
             <nvram>/var/lib/libvirt/qemu/nvram/{instance_name}_VARS.fd</nvram>\
           </os>\
           <devices>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='/var/lib/libvirt/images/{instance_name}.qcow2'/>\
               <target dev='sda' bus='sata'/>\
             </disk>\
           </devices>\
         </domain>"
    )
}

fn create_sample_multi_disk_domain_xml(instance_name: &str, instance_uuid: &str) -> String {
    format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid}</uuid>\
           <os>\
             <type arch='x86_64' machine='q35'>hvm</type>\
             <nvram>/var/lib/libvirt/qemu/nvram/{instance_name}_VARS.fd</nvram>\
           </os>\
           <devices>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='/var/lib/libvirt/images/{instance_name}.qcow2'/>\
               <target dev='sda' bus='sata'/>\
             </disk>\
             <disk type='file' device='disk'>\
               <driver name='qemu' type='qcow2'/>\
               <source file='/data/storage/{instance_name}_data.qcow2'/>\
               <target dev='sdb' bus='virtio'/>\
             </disk>\
           </devices>\
         </domain>"
    )
}

fn create_sample_pool_xml(pool_name: &str, target_path: &str) -> String {
    format!(
        "<pool type='dir'>\
           <name>{pool_name}</name>\
           <target>\
             <path>{target_path}</path>\
           </target>\
         </pool>"
    )
}

#[test]
fn test_offline_backup_success() {
    let instance_name = "win11-offline";
    let instance_uuid_literal = "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d";
    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, &domain_xml);

    let overlay_path = PathBuf::from(format!("/var/lib/libvirt/images/{instance_name}.qcow2"));
    let storage = MockStorageManager::new()
        .with_image(
            overlay_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 1_234_567,
                backing_file: Some(PathBuf::from("/data/depot/store/win11-base.qcow2")),
                is_corrupted: false,
            },
        );

    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-offline/2026-09-26T20:00:00Z");
    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T20:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, backup_directory, &options, None)
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
        68_719_476_736
    );

    let storage_actions = storage.recorded_actions();
    let has_convert_action = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::ConvertThinBackup { compress: true, .. }
        )
    });
    assert!(
        has_convert_action,
        "Storage manager should execute thin conversion with compression"
    );

    let has_nvram_init = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::InitializeNvram { destination_nvram_path, .. }
            if destination_nvram_path == &backup_directory.join("nvram.fd")
        )
    });
    assert!(has_nvram_init, "NVRAM should be copied to backup directory");

    let hypervisor_actions = hypervisor.recorded_actions();
    let has_snapshots = hypervisor_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::CreateSnapshot { .. }
        )
    });
    assert!(!has_snapshots, "Offline backup should not create snapshots");
}

#[test]
fn test_online_backup_vss_quiesced_success() {
    let instance_name = "win11-online";
    let instance_uuid_literal = "b2c3d4e5-f6a7-4b5c-8d9e-1f2a3b4c5d6e";
    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(8),
        memory_kib: Some(16_777_216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, &domain_xml);

    let overlay_path = PathBuf::from(format!("/var/lib/libvirt/images/{instance_name}.qcow2"));
    let storage = MockStorageManager::new()
        .with_image(
            overlay_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 2_345_678,
                backing_file: Some(PathBuf::from("/data/depot/store/win11-base.qcow2")),
                is_corrupted: false,
            },
        );

    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-online/2026-09-26T21:00:00Z");
    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T21:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, backup_directory, &options, None)
        .expect("Online backup should succeed");

    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::VssQuiesced
    );

    let hypervisor_actions = hypervisor.recorded_actions();
    let snapshot_action = hypervisor_actions.iter().find(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::CreateSnapshot { quiesce: true, .. }
        )
    });
    assert!(
        snapshot_action.is_some(),
        "Hypervisor should create atomic snapshot with VSS quiescing enabled"
    );

    let blockcommit_action = hypervisor_actions.iter().find(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::Blockcommit {
                active: true,
                pivot: true,
                ..
            }
        )
    });
    assert!(
        blockcommit_action.is_some(),
        "Hypervisor should collapse snapshot via active blockcommit with pivot"
    );

    let storage_actions = storage.recorded_actions();
    let snapshot_deleted = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::DeleteImage { image_path }
            if image_path.extension().and_then(|ext| ext.to_str()) == Some("snap")
        )
    });
    assert!(
        snapshot_deleted,
        "Temporary .snap overlay should be unlinked after successful blockcommit"
    );
}

#[test]
fn test_online_backup_quiesce_fallback_to_crash_consistent() {
    let instance_name = "win11-fallback";
    let instance_uuid_literal = "c3d4e5f6-a7b8-4c5d-9e1f-2a3b4c5d6e7f";
    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_domain(domain_info, &domain_xml)
        .with_quiesce_failure(true);

    let overlay_path = PathBuf::from(format!("/var/lib/libvirt/images/{instance_name}.qcow2"));
    let storage = MockStorageManager::new()
        .with_image(
            overlay_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 500_000,
                backing_file: None,
                is_corrupted: false,
            },
        );

    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-fallback/2026-09-26T22:00:00Z");
    let options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T22:00:00Z".to_string()),
    };

    let backup_manifest = engine
        .backup_instance(instance_name, backup_directory, &options, None)
        .expect("Backup should succeed via crash-consistent fallback");

    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::CrashConsistent
    );

    let hypervisor_actions = hypervisor.recorded_actions();
    let snapshot_attempts: Vec<_> = hypervisor_actions
        .iter()
        .filter(|recorded_action| {
            matches!(
                recorded_action,
                RecordedHypervisorAction::CreateSnapshot { .. }
            )
        })
        .collect();

    assert_eq!(
        snapshot_attempts.len(),
        2,
        "Should attempt quiesced snapshot first, then fall back to crash-consistent"
    );
    assert!(
        matches!(
            snapshot_attempts[0],
            RecordedHypervisorAction::CreateSnapshot { quiesce: true, .. }
        ),
        "First snapshot attempt should be quiesced"
    );
    assert!(
        matches!(
            snapshot_attempts[1],
            RecordedHypervisorAction::CreateSnapshot { quiesce: false, .. }
        ),
        "Second snapshot attempt should be unquiesced (crash-consistent)"
    );
}

#[test]
fn test_online_backup_raii_guard_triggers_blockcommit_on_failure() {
    let instance_name = "win11-raii-test";
    let instance_uuid_literal = "d4e5f6a7-b8c9-4d5e-1f2a-3b4c5d6e7f8a";
    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, &domain_xml);

    let overlay_path = PathBuf::from(format!("/var/lib/libvirt/images/{instance_name}.qcow2"));
    let storage = MockStorageManager::new()
        .with_image(
            &overlay_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 500_000,
                backing_file: None,
                is_corrupted: false,
            },
        )
        .with_injected_error(overlay_path, "Simulated storage I/O error during convert");

    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-raii/2026-09-26T23:00:00Z");
    let options = BackupOptions::default();

    let backup_result = engine.backup_instance(instance_name, backup_directory, &options, None);

    assert!(
        backup_result.is_err(),
        "Backup must fail when storage conversion errors"
    );

    let hypervisor_actions = hypervisor.recorded_actions();
    let blockcommit_action = hypervisor_actions.iter().find(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::Blockcommit {
                active: true,
                pivot: true,
                ..
            }
        )
    });

    assert!(
        blockcommit_action.is_some(),
        "RAII guard must execute blockcommit when conversion fails to prevent dangling snapshots"
    );
}

#[test]
fn test_multi_disk_atomic_snapshot_backup() {
    let instance_name = "win11-multidisk";
    let instance_uuid_literal = "e5f6a7b8-c9d0-4e1f-2a3b-4c5d6e7f8a9b";
    let domain_xml = create_sample_multi_disk_domain_xml(instance_name, instance_uuid_literal);

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(8),
        memory_kib: Some(16_777_216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, &domain_xml);

    let disk1_path = PathBuf::from(format!("/var/lib/libvirt/images/{instance_name}.qcow2"));
    let disk2_path = PathBuf::from(format!("/data/storage/{instance_name}_data.qcow2"));

    let storage = MockStorageManager::new()
        .with_image(
            disk1_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 1_000_000,
                backing_file: Some(PathBuf::from("/data/depot/store/win11-base.qcow2")),
                is_corrupted: false,
            },
        )
        .with_image(
            disk2_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 107_374_182_400,
                actual_size_bytes: 5_000_000,
                backing_file: None,
                is_corrupted: false,
            },
        );

    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-multidisk/2026-09-26T23:30:00Z");
    let options = BackupOptions::default();

    let backup_manifest = engine
        .backup_instance(instance_name, backup_directory, &options, None)
        .expect("Multi-disk backup should succeed");

    assert_eq!(backup_manifest.disks.len(), 2);

    let hypervisor_actions = hypervisor.recorded_actions();
    let snapshot_action = hypervisor_actions.iter().find_map(|recorded_action| {
        match recorded_action {
            RecordedHypervisorAction::CreateSnapshot {
                disk_specifications,
                ..
            } => Some(disk_specifications),
            _ => None,
        }
    });

    let snapshot_specs = snapshot_action.expect("CreateSnapshot should have been recorded");
    assert_eq!(
        snapshot_specs.len(),
        2,
        "Snapshot should include specifications for all attached disks"
    );

    let devices: Vec<&str> = snapshot_specs
        .iter()
        .map(|specification| specification.target_device.as_str())
        .collect();
    assert!(devices.contains(&"sda"));
    assert!(devices.contains(&"sdb"));
}

#[test]
fn test_backup_fails_when_no_disks_attached() {
    let instance_name = "win11-nodisks";
    let instance_uuid_literal = "f6a7b8c9-d0e1-4f2a-3b4c-5d6e7f8a9b0c";
    let domain_xml = format!(
        "<domain type='kvm'>\
           <name>{instance_name}</name>\
           <uuid>{instance_uuid_literal}</uuid>\
           <os>\
             <type arch='x86_64'>hvm</type>\
             <nvram>/var/lib/libvirt/qemu/nvram/{instance_name}_VARS.fd</nvram>\
           </os>\
           <devices/>\
         </domain>"
    );

    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(2),
        memory_kib: Some(4_194_304),
        autostart: false,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, domain_xml);
    let storage = MockStorageManager::new();
    let engine = BackupEngine::new(&hypervisor, &storage);

    let backup_directory = Path::new("/data/depot/backup/win11-nodisks");
    let options = BackupOptions::default();

    let backup_result = engine.backup_instance(instance_name, backup_directory, &options, None);

    assert!(
        matches!(backup_result, Err(BackupError::NoDisksFound { .. })),
        "Backup must fail with NoDisksFound when VM has no disks"
    );
}

#[test]
fn test_restore_engine_success() {
    let instance_name = "win11-restore";
    let instance_uuid_literal = "01234567-89ab-4cde-f012-3456789abcde";
    let backup_directory = Path::new("/data/depot/backup/win11-restore/2026-09-26T20:00:00Z");

    let pool_name = "default";
    let pool_path = "/var/lib/libvirt/images";
    let pool_xml = create_sample_pool_xml(pool_name, pool_path);

    let hypervisor = MockHypervisor::new().with_pool_xml(pool_name, &pool_xml);

    let golden_master_filename = "win11-26300-looking-glass-hash123.qcow2";
    let depot_store_directory = Path::new("/data/depot/store");
    let depot_golden_master_path = depot_store_directory.join(golden_master_filename);

    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);

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
            virtual_size_bytes: 68_719_476_736,
            archive_size_bytes: 1_500_000,
            backing_file: Some(PathBuf::from(format!("{pool_path}/{golden_master_filename}"))),
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");

    let storage = MockStorageManager::new()
        .with_file(backup_directory.join("manifest.json"), manifest_json)
        .with_file(backup_directory.join("domain.xml"), &domain_xml)
        .with_file(backup_directory.join("nvram.fd"), "NVRAM_BINARY_DATA")
        .with_image(
            backup_directory.join("sda.qcow2"),
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 1_500_000,
                backing_file: None,
                is_corrupted: false,
            },
        )
        .with_image(
            depot_golden_master_path,
            MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 10_000_000,
                backing_file: None,
                is_corrupted: false,
            },
        );

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        target_pool: Some(pool_name.to_string()),
        depot_store_dir: Some(depot_store_directory.to_path_buf()),
        nvram_dir: None,
        allow_overwrite: false,
    };

    let restore_result = restore_engine.restore_instance(backup_directory, &options, None);
    assert!(
        restore_result.is_ok(),
        "Restore should succeed without errors"
    );

    let storage_actions = storage.recorded_actions();
    let has_copied_base = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::CopyBaseImage { .. }
        )
    });
    assert!(has_copied_base, "Base image should be copied to target pool cache");

    let has_restored_thin = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::RestoreThinBackup { .. }
        )
    });
    assert!(
        has_restored_thin,
        "Storage manager should call restore_thin_backup for overlay disk"
    );

    let has_restored_nvram = storage_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedStorageAction::InitializeNvram { destination_nvram_path, .. }
            if destination_nvram_path == &PathBuf::from(format!("/var/lib/libvirt/qemu/nvram/{instance_name}_VARS.fd"))
        )
    });
    assert!(has_restored_nvram, "NVRAM should be restored to host path from domain XML");

    let hypervisor_actions = hypervisor.recorded_actions();
    let has_defined_domain = hypervisor_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::DefineDomain { .. }
        )
    });
    assert!(has_defined_domain, "Domain should be registered with hypervisor");

    let has_refreshed_pool = hypervisor_actions.iter().any(|recorded_action| {
        matches!(
            recorded_action,
            RecordedHypervisorAction::PoolRefresh { pool_name: refreshed_pool }
            if refreshed_pool == pool_name
        )
    });
    assert!(has_refreshed_pool, "Target storage pool should be refreshed in Libvirt");
}

#[test]
fn test_restore_engine_rejects_already_existing_domain() {
    let instance_name = "win11-existing";
    let instance_uuid_literal = "12345678-9abc-4def-0123-456789abcdef";
    let backup_directory = Path::new("/data/depot/backup/win11-existing");

    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);
    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, &domain_xml);

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
            virtual_size_bytes: 68_719_476_736,
            archive_size_bytes: 1_000_000,
            backing_file: None,
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    let storage =
        MockStorageManager::new().with_file(backup_directory.join("manifest.json"), manifest_json);

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        allow_overwrite: false,
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(backup_directory, &options, None);

    assert!(
        matches!(restore_result, Err(RestoreError::DomainAlreadyExists { .. })),
        "Restore must reject overwriting existing domain when allow_overwrite is false"
    );
}

#[test]
fn test_restore_engine_allows_overwrite_when_flag_set() {
    let instance_name = "win11-overwrite";
    let instance_uuid_literal = "23456789-abcd-4ef0-1234-56789abcdef0";
    let backup_directory = Path::new("/data/depot/backup/win11-overwrite");

    let pool_name = "default";
    let pool_path = "/var/lib/libvirt/images";
    let pool_xml = create_sample_pool_xml(pool_name, pool_path);

    let domain_xml = create_sample_domain_xml(instance_name, instance_uuid_literal);
    let domain_info = DomainInfo {
        name: instance_name.to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_domain(domain_info, &domain_xml)
        .with_pool_xml(pool_name, &pool_xml);

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
            virtual_size_bytes: 68_719_476_736,
            archive_size_bytes: 1_000_000,
            backing_file: None,
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    let storage = MockStorageManager::new()
        .with_file(backup_directory.join("manifest.json"), manifest_json)
        .with_file(backup_directory.join("domain.xml"), &domain_xml)
        .with_file(backup_directory.join("nvram.fd"), "NVRAM_DATA")
        .with_image(
            backup_directory.join("sda.qcow2"),
            MockImageRecord::default(),
        );

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        allow_overwrite: true,
        target_pool: Some(pool_name.to_string()),
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(backup_directory, &options, None);
    assert!(
        restore_result.is_ok(),
        "Restore must proceed when allow_overwrite is true"
    );
}

#[test]
fn test_restore_engine_fails_when_base_image_not_found() {
    let instance_name = "win11-missing-base";
    let instance_uuid_literal = "3456789a-bcde-4f01-2345-6789abcdef01";
    let backup_directory = Path::new("/data/depot/backup/win11-missing-base");

    let pool_name = "default";
    let pool_path = "/var/lib/libvirt/images";
    let pool_xml = create_sample_pool_xml(pool_name, pool_path);

    let hypervisor = MockHypervisor::new().with_pool_xml(pool_name, &pool_xml);

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
            virtual_size_bytes: 68_719_476_736,
            archive_size_bytes: 1_000_000,
            backing_file: Some(PathBuf::from("/var/lib/libvirt/images/missing-master.qcow2")),
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).expect("Valid JSON");
    let storage = MockStorageManager::new()
        .with_file(backup_directory.join("manifest.json"), manifest_json)
        .with_image(
            backup_directory.join("sda.qcow2"),
            MockImageRecord::default(),
        );

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions {
        target_pool: Some(pool_name.to_string()),
        depot_store_dir: Some(PathBuf::from("/data/depot/store")),
        ..Default::default()
    };

    let restore_result = restore_engine.restore_instance(backup_directory, &options, None);

    assert!(
        matches!(restore_result, Err(RestoreError::BaseImageNotFound { .. })),
        "Restore must fail when golden master base image is missing from store"
    );
}

#[test]
fn test_restore_engine_empty_disks_validation() {
    let instance_name = "win11-empty-disks";
    let instance_uuid_literal = "456789ab-cdef-4012-3456-789abcdef012";
    let backup_directory = Path::new("/data/depot/backup/win11-empty-disks");

    let hypervisor = MockHypervisor::new();

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
    let storage =
        MockStorageManager::new().with_file(backup_directory.join("manifest.json"), manifest_json);

    let restore_engine = RestoreEngine::new(&hypervisor, &storage);
    let options = RestoreOptions::default();

    let restore_result = restore_engine.restore_instance(backup_directory, &options, None);

    assert!(
        matches!(restore_result, Err(RestoreError::EmptyDisks { .. })),
        "Restore must fail when manifest has no disk entries"
    );
}
