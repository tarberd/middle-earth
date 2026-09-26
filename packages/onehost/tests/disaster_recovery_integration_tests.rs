use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

use onehost::backup::model::{BackupConsistencyLevel, BackupOptions, RestoreOptions};
use onehost::backup::{BackupEngine, RestoreEngine, RestoreError};
use onehost::config::loader::ManifestLoader;
use onehost::hypervisor::mock::{MockHypervisor, RecordedHypervisorAction};
use onehost::hypervisor::traits::{DomainState, Hypervisor};
use onehost::lifecycle::applier::{ApplyOptions, DomainLifecycleApplier};
use onehost::lifecycle::destroyer::{DestroyOptions, DomainLifecycleDestroyer};
use onehost::lifecycle::planner::{DomainLifecyclePlanner, InstancePlanAction};
use onehost::lifecycle::LifecycleError;
use onehost::storage::mock::{MockImageRecord, MockStorageManager, RecordedStorageAction};
use onehost::storage::qemu_img::QemuImgStorage;
use onehost::storage::traits::StorageManager;

const SAMPLE_TEMPLATE_XML: &str = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>TEMPLATE</name>
  <uuid>00000000-0000-0000-0000-000000000000</uuid>
  <vcpu placement='static'>4</vcpu>
  <memory unit='KiB'>8388608</memory>
  <os>
    <type arch='x86_64' machine='q35'>hvm</type>
    <nvram>/var/lib/libvirt/qemu/nvram/TEMPLATE_VARS.fd</nvram>
  </os>
  <devices>
    <disk type='file' device='disk' onehost:role='os-disk'>
      <target dev='sda' bus='sata'/>
    </disk>
  </devices>
</domain>"#;

const MULTI_DISK_TEMPLATE_XML: &str = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>TEMPLATE</name>
  <uuid>00000000-0000-0000-0000-000000000000</uuid>
  <vcpu placement='static'>4</vcpu>
  <memory unit='KiB'>8388608</memory>
  <os>
    <type arch='x86_64' machine='q35'>hvm</type>
    <nvram>/var/lib/libvirt/qemu/nvram/TEMPLATE_VARS.fd</nvram>
  </os>
  <devices>
    <disk type='file' device='disk' onehost:role='os-disk'>
      <target dev='sda' bus='sata'/>
    </disk>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='__SECONDARY_DISK_PATH__'/>
      <target dev='sdb' bus='virtio'/>
    </disk>
  </devices>
</domain>"#;

struct DisasterRecoveryWorkspace {
    _workspace_directory: tempfile::TempDir,
    manifest_path: PathBuf,
    _template_xml_path: PathBuf,
    _depot_store_directory: PathBuf,
    depot_backup_directory: PathBuf,
    _nvram_directory: PathBuf,
    default_pool_directory: PathBuf,
    initial_golden_master_path: PathBuf,
    secondary_disk_path: Option<PathBuf>,
}

impl DisasterRecoveryWorkspace {
    fn new(is_multi_disk: bool) -> Self {
        let temporary_workspace_directory = tempdir().expect("create workspace tempdir");
        let workspace_root_path = temporary_workspace_directory.path();

        let depot_directory = workspace_root_path.join("depot");
        let depot_store_directory = depot_directory.join("store");
        let depot_iso_directory = depot_directory.join("iso");
        let depot_backup_directory = depot_directory.join("backup");
        let nvram_directory = workspace_root_path.join("nvram");
        let default_pool_directory = workspace_root_path.join("default_pool");

        std::fs::create_dir_all(&depot_store_directory).expect("create depot_store");
        std::fs::create_dir_all(&depot_iso_directory).expect("create depot_iso");
        std::fs::create_dir_all(&depot_backup_directory).expect("create depot_backup");
        std::fs::create_dir_all(&nvram_directory).expect("create nvram_dir");
        std::fs::create_dir_all(&default_pool_directory).expect("create default_pool_dir");

        let nvram_template_path = workspace_root_path.join("edk2-vars.fd");
        let mut nvram_file = File::create(&nvram_template_path).expect("create nvram template");
        nvram_file
            .write_all(b"NVRAM_TEMPLATE_VARS")
            .expect("write nvram template");

        let initial_master_path =
            depot_store_directory.join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
        std::fs::write(&initial_master_path, b"GOLDEN_MASTER_QCOW2").expect("write golden master");

        let (template_xml_path, secondary_disk_path) = if is_multi_disk {
            let secondary_path = default_pool_directory.join("win11-gollum-sdb.qcow2");
            let rendered_multi_disk_template = MULTI_DISK_TEMPLATE_XML
                .replace("__SECONDARY_DISK_PATH__", &secondary_path.display().to_string());
            let template_path = workspace_root_path.join("win11-multi-template.xml");
            std::fs::write(&template_path, rendered_multi_disk_template).expect("write multi template");
            (template_path, Some(secondary_path))
        } else {
            let template_path = workspace_root_path.join("win11-template.xml");
            std::fs::write(&template_path, SAMPLE_TEMPLATE_XML).expect("write template xml");
            (template_path, None)
        };

        let manifest_content = format!(
            r#"{{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
  "version": "1.0",
  "storage": {{
    "depot_store_dir": "{}",
    "depot_iso_dir": "{}",
    "depot_backup_dir": "{}",
    "nvram_dir": "{}",
    "nvram_template": "{}",
    "ovmf_code": "/run/libvirt/nix-ovmf/code.fd",
    "default_pool": "default"
  }},
  "flavors": {{
    "looking-glass": {{
      "oemdrv_path": "/nix/store/ba6eafb71234-oemdrv-looking-glass",
      "hash": "ba6eafb7"
    }}
  }},
  "instances": {{
    "win11-gollum": {{
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "{}",
      "image": "win11/26300.9457.pro.en-us/looking-glass",
      "pool": "default",
      "autostart": false,
      "lifecycle": {{
        "on_image_change": "protect",
        "prevent_destroy": false
      }}
    }}
  }}
}}"#,
            depot_store_directory.display(),
            depot_iso_directory.display(),
            depot_backup_directory.display(),
            nvram_directory.display(),
            nvram_template_path.display(),
            template_xml_path.display()
        );

        let manifest_file_path = workspace_root_path.join("onehost.json");
        std::fs::write(&manifest_file_path, manifest_content).expect("write manifest");

        Self {
            _workspace_directory: temporary_workspace_directory,
            manifest_path: manifest_file_path,
            _template_xml_path: template_xml_path,
            _depot_store_directory: depot_store_directory,
            depot_backup_directory,
            _nvram_directory: nvram_directory,
            default_pool_directory,
            initial_golden_master_path: initial_master_path,
            secondary_disk_path,
        }
    }

    fn default_pool_xml(&self) -> String {
        format!(
            r#"<pool type='dir'>
  <name>default</name>
  <target>
    <path>{}</path>
  </target>
</pool>"#,
            self.default_pool_directory.display()
        )
    }
}

fn file_template_resolver(template_path: &Path) -> Result<String, LifecycleError> {
    std::fs::read_to_string(template_path).map_err(LifecycleError::from)
}

fn create_real_qcow2_image(path: &Path, size_megabytes: u64) {
    let size_argument = format!("{size_megabytes}M");
    let output = Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            path.to_str().expect("valid utf8 path"),
            &size_argument,
        ])
        .output()
        .expect("qemu-img must be available in test environment");

    assert!(
        output.status.success(),
        "failed to create test qcow2 image: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_end_to_end_disaster_recovery_round_trip_offline() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    // 1. Provision instance via declarative plan and apply
    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading must succeed");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    let initial_plan = planner.plan(&manifest).expect("initial plan must succeed");
    assert!(matches!(
        &initial_plan.actions[0],
        InstancePlanAction::Create { .. }
    ));

    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply must succeed");

    let domain_info = hypervisor
        .domain_info("win11-gollum")
        .expect("domain must be registered in hypervisor");
    assert_eq!(domain_info.state, DomainState::Shutoff);

    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&overlay_path).is_ok());

    let idempotent_plan = planner.plan(&manifest).expect("idempotent plan");
    assert!(matches!(
        &idempotent_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));

    // 2. Perform offline backup
    let backup_directory = workspace
        .depot_backup_directory
        .join("win11-gollum")
        .join("offline-snapshot");

    let backup_options = BackupOptions {
        quiesce: false,
        compress: true,
        timestamp: Some("2026-09-26T20:30:00Z".to_string()),
    };

    let backup_manifest = backup_engine
        .backup_instance("win11-gollum", &backup_directory, &backup_options, Some(&manifest))
        .expect("offline backup must succeed");

    assert_eq!(backup_manifest.instance_name, "win11-gollum");
    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::Offline
    );
    assert_eq!(backup_manifest.disks.len(), 1);
    assert_eq!(backup_manifest.disks[0].target_device, "sda");

    // 3. Simulate disaster: destroy instance and wipe its overlay disk
    let destroy_options = DestroyOptions {
        delete_disk: true,
        allow_destroy_protected: true,
        ..Default::default()
    };

    destroyer
        .destroy("win11-gollum", &manifest, &destroy_options)
        .expect("destroy instance must succeed");

    assert!(hypervisor.domain_info("win11-gollum").is_err());
    assert!(storage.inspect_image(&overlay_path).is_err());

    let post_disaster_plan = planner.plan(&manifest).expect("post disaster plan");
    assert!(matches!(
        &post_disaster_plan.actions[0],
        InstancePlanAction::Create { .. }
    ));

    // 4. Disaster Recovery: restore instance from staged backup
    let restore_options = RestoreOptions::default();
    restore_engine
        .restore_instance(&backup_directory, &restore_options, Some(&manifest))
        .expect("restore instance must succeed");

    let restored_domain = hypervisor
        .domain_info("win11-gollum")
        .expect("domain must exist after restore");
    assert_eq!(restored_domain.state, DomainState::Shutoff);

    let restored_image_info = storage
        .inspect_image(&overlay_path)
        .expect("overlay image must exist after restore");
    assert_eq!(restored_image_info.format, "qcow2");
    assert!(restored_image_info
        .backing_file
        .as_ref()
        .is_some_and(|backing| backing.to_string_lossy().contains("ba6eafb7")));

    // 5. Plan Verification: restored instance produces NoOp with zero drift
    let post_restore_plan = planner.plan(&manifest).expect("post restore plan");
    assert_eq!(post_restore_plan.actions.len(), 1);
    assert!(matches!(
        &post_restore_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_disaster_recovery_round_trip_online() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading must succeed");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    // Boot VM into running state
    hypervisor
        .start_domain("win11-gollum")
        .expect("start domain must succeed");

    let domain_info = hypervisor
        .domain_info("win11-gollum")
        .expect("domain info");
    assert_eq!(domain_info.state, DomainState::Running);

    // Perform online quiesced backup
    let backup_directory = workspace
        .depot_backup_directory
        .join("win11-gollum")
        .join("online-snapshot");

    let backup_options = BackupOptions {
        quiesce: true,
        compress: true,
        timestamp: Some("2026-09-26T20:31:00Z".to_string()),
    };

    let backup_manifest = backup_engine
        .backup_instance("win11-gollum", &backup_directory, &backup_options, Some(&manifest))
        .expect("online backup must succeed");

    assert_eq!(
        backup_manifest.consistency_level,
        BackupConsistencyLevel::VssQuiesced
    );

    // Verify online backup actions: snapshot creation, blockcommit pivot, and cleanup of temporary .snap file
    let recorded_hypervisor_actions = hypervisor.recorded_actions();
    let has_snapshot = recorded_hypervisor_actions
        .iter()
        .any(|action| matches!(action, RecordedHypervisorAction::CreateSnapshot { .. }));
    let has_blockcommit = recorded_hypervisor_actions
        .iter()
        .any(|action| matches!(action, RecordedHypervisorAction::Blockcommit { active: true, pivot: true, .. }));
    assert!(has_snapshot);
    assert!(has_blockcommit);

    let recorded_storage_actions = storage.recorded_actions();
    let has_delete_snapshot_overlay = recorded_storage_actions
        .iter()
        .any(|action| matches!(action, RecordedStorageAction::DeleteImage { image_path } if image_path.to_string_lossy().ends_with(".snap")));
    assert!(has_delete_snapshot_overlay);

    // Simulate disaster: destroy instance with delete_disk
    let destroy_options = DestroyOptions {
        delete_disk: true,
        allow_destroy_protected: true,
        force: true,
    };
    destroyer
        .destroy("win11-gollum", &manifest, &destroy_options)
        .expect("destroy instance");

    assert!(hypervisor.domain_info("win11-gollum").is_err());

    // Disaster Recovery: restore instance
    restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect("restore instance");

    assert!(hypervisor.domain_info("win11-gollum").is_ok());

    // Plan Verification: restored instance produces NoOp
    let post_restore_plan = planner.plan(&manifest).expect("post restore plan");
    assert!(matches!(
        &post_restore_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_disaster_recovery_with_real_qemu_img_toolchain() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    // Create real golden master QCOW2 image
    create_real_qcow2_image(&workspace.initial_golden_master_path, 20);

    let storage = QemuImgStorage::new();
    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading must succeed");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    // 1. Initial provision creates real QCOW2 overlay on disk
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(overlay_path.exists());
    assert!(storage.check_image(&overlay_path).is_ok());

    let initial_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay");
    assert_eq!(initial_inspection.format, "qcow2");
    assert_eq!(initial_inspection.virtual_size_bytes, 20 * 1024 * 1024);

    let idempotent_plan = planner.plan(&manifest).expect("idempotent plan");
    assert!(matches!(
        &idempotent_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));

    // 2. Backup instance into compressed thin archive
    let backup_directory = workspace
        .depot_backup_directory
        .join("win11-gollum")
        .join("real-backup");

    let backup_options = BackupOptions {
        quiesce: false,
        compress: true,
        timestamp: Some("2026-09-26T20:32:00Z".to_string()),
    };

    let backup_manifest = backup_engine
        .backup_instance("win11-gollum", &backup_directory, &backup_options, Some(&manifest))
        .expect("backup instance");

    let archive_path = backup_directory.join("sda.qcow2");
    assert!(archive_path.exists());
    assert!(storage.check_image(&archive_path).is_ok());

    let archive_inspection = storage
        .inspect_image(&archive_path)
        .expect("inspect archive");
    assert_eq!(archive_inspection.format, "qcow2");
    assert_eq!(archive_inspection.virtual_size_bytes, 20 * 1024 * 1024);
    assert_eq!(backup_manifest.disks.len(), 1);

    // 3. Simulate disaster: destroy VM and delete overlay from disk
    let destroy_options = DestroyOptions {
        delete_disk: true,
        allow_destroy_protected: true,
        ..Default::default()
    };
    destroyer
        .destroy("win11-gollum", &manifest, &destroy_options)
        .expect("destroy instance");

    assert!(!overlay_path.exists());
    assert!(hypervisor.domain_info("win11-gollum").is_err());

    // 4. Disaster Recovery: restore instance from archive
    restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect("restore instance");

    assert!(overlay_path.exists());
    assert!(storage.check_image(&overlay_path).is_ok());

    let restored_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect restored overlay");
    assert_eq!(restored_inspection.format, "qcow2");
    assert_eq!(restored_inspection.virtual_size_bytes, 20 * 1024 * 1024);

    // 5. Post-restore Plan: asserts NoOp with zero drift
    let post_restore_plan = planner.plan(&manifest).expect("post restore plan");
    assert!(matches!(
        &post_restore_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_disaster_recovery_missing_base_cache_automatic_repopulation() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    // 1. Provision instance
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    let cached_master_path = workspace
        .default_pool_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
    assert!(storage.inspect_image(&cached_master_path).is_ok());

    // 2. Perform backup
    let backup_directory = workspace
        .depot_backup_directory
        .join("win11-gollum")
        .join("snapshot");

    backup_engine
        .backup_instance(
            "win11-gollum",
            &backup_directory,
            &BackupOptions::default(),
            Some(&manifest),
        )
        .expect("backup instance");

    // 3. Catastrophic Storage Failure: destroy VM and wipe the cached base image from the local pool
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                delete_disk: true,
                allow_destroy_protected: true,
                ..Default::default()
            },
        )
        .expect("destroy instance");

    storage
        .delete_image(&cached_master_path)
        .expect("wipe cached base master from pool");
    assert!(storage.inspect_image(&cached_master_path).is_err());

    // 4. Disaster Recovery: restore engine must detect missing pool cache and repopulate it from depot store
    restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect("restore instance");

    assert!(storage.inspect_image(&cached_master_path).is_ok());
    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&overlay_path).is_ok());

    // 5. Verification: planner emits NoOp
    let post_restore_plan = planner.plan(&manifest).expect("post restore plan");
    assert!(matches!(
        &post_restore_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_disaster_recovery_multi_disk() {
    let workspace = DisasterRecoveryWorkspace::new(true);
    let secondary_disk_path = workspace
        .secondary_disk_path
        .clone()
        .expect("secondary disk path must be present in multi-disk workspace");

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default())
        .with_image(&secondary_disk_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    // 1. Provision multi-disk instance
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    let block_devices = hypervisor
        .list_block_devices("win11-gollum")
        .expect("list block devices");
    assert_eq!(block_devices.len(), 2);
    let has_sda = block_devices
        .iter()
        .any(|device| device.target_device == "sda");
    let has_sdb = block_devices
        .iter()
        .any(|device| device.target_device == "sdb");
    assert!(has_sda);
    assert!(has_sdb);

    let idempotent_plan = planner.plan(&manifest).expect("idempotent plan");
    assert!(matches!(
        &idempotent_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));

    // 2. Perform multi-disk backup
    let backup_directory = workspace
        .depot_backup_directory
        .join("win11-gollum")
        .join("multi-disk-snapshot");

    let backup_manifest = backup_engine
        .backup_instance(
            "win11-gollum",
            &backup_directory,
            &BackupOptions::default(),
            Some(&manifest),
        )
        .expect("multi-disk backup must succeed");

    assert_eq!(backup_manifest.disks.len(), 2);
    let backup_has_sda = backup_manifest
        .disks
        .iter()
        .any(|disk| disk.target_device == "sda" && disk.archive_filename == "sda.qcow2");
    let backup_has_sdb = backup_manifest
        .disks
        .iter()
        .any(|disk| disk.target_device == "sdb" && disk.archive_filename == "sdb.qcow2");
    assert!(backup_has_sda);
    assert!(backup_has_sdb);

    // 3. Catastrophic Disaster: destroy instance and wipe both disk images
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                delete_disk: true,
                allow_destroy_protected: true,
                ..Default::default()
            },
        )
        .expect("destroy instance");

    storage
        .delete_image(&secondary_disk_path)
        .expect("wipe secondary data disk");

    let primary_overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&primary_overlay_path).is_err());
    assert!(storage.inspect_image(&secondary_disk_path).is_err());

    // 4. Disaster Recovery: restore both disks from multi-disk backup archive
    restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect("restore instance");

    assert!(storage.inspect_image(&primary_overlay_path).is_ok());
    assert!(storage.inspect_image(&secondary_disk_path).is_ok());

    let restored_devices = hypervisor
        .list_block_devices("win11-gollum")
        .expect("list block devices after restore");
    assert_eq!(restored_devices.len(), 2);

    // 5. Verification: planner emits NoOp with zero drift across both disks
    let post_restore_plan = planner.plan(&manifest).expect("post restore plan");
    assert!(matches!(
        &post_restore_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_real_toolchain_thin_backup_and_restore_compression() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let base_image_path = temporary_directory.path().join("golden_master.qcow2");
    let overlay_disk_path = temporary_directory.path().join("instance_overlay.qcow2");
    let backup_archive_path = temporary_directory.path().join("thin_backup_archive.qcow2");
    let restored_overlay_path = temporary_directory.path().join("restored_overlay.qcow2");

    create_real_qcow2_image(&base_image_path, 25);

    let storage = QemuImgStorage::new();
    storage
        .create_cow_overlay(&base_image_path, &overlay_disk_path)
        .expect("create overlay");

    // Convert thin backup with compression
    storage
        .convert_thin_backup(
            &overlay_disk_path,
            &backup_archive_path,
            Some(&base_image_path),
            true,
        )
        .expect("convert thin backup must succeed");

    let archive_inspection = storage
        .inspect_image(&backup_archive_path)
        .expect("inspect backup archive");
    assert_eq!(archive_inspection.format, "qcow2");
    assert_eq!(archive_inspection.virtual_size_bytes, 25 * 1024 * 1024);
    assert_eq!(
        archive_inspection.backing_file,
        Some(base_image_path.clone())
    );

    // Thin archive file size on disk is substantially smaller than virtual size
    let archive_file_size = std::fs::metadata(&backup_archive_path)
        .expect("metadata")
        .len();
    assert!(archive_file_size < 1024 * 1024);

    // Restore thin backup
    storage
        .restore_thin_backup(
            &backup_archive_path,
            &restored_overlay_path,
            Some(&base_image_path),
        )
        .expect("restore thin backup must succeed");

    let restored_inspection = storage
        .inspect_image(&restored_overlay_path)
        .expect("inspect restored overlay");
    assert_eq!(restored_inspection.format, "qcow2");
    assert_eq!(restored_inspection.virtual_size_bytes, 25 * 1024 * 1024);
    assert_eq!(
        restored_inspection.backing_file,
        Some(base_image_path)
    );

    // Verify 0 corruption in restored image
    let check_result = storage.check_image(&restored_overlay_path);
    assert!(check_result.is_ok());
}

#[test]
fn test_disaster_recovery_guardrail_aborts_if_domain_exists_without_overwrite() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    // Provision instance and take backup
    let plan = planner.plan(&manifest).expect("plan");
    applier.apply(&plan, &ApplyOptions::default()).expect("apply");

    let backup_directory = workspace.depot_backup_directory.join("win11-gollum").join("guardrail-test");
    backup_engine
        .backup_instance("win11-gollum", &backup_directory, &BackupOptions::default(), Some(&manifest))
        .expect("backup instance");

    // Attempt restore without destroying domain and with allow_overwrite = false
    let restore_error = restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect_err("restore must fail when domain already exists and allow_overwrite is false");

    match restore_error {
        RestoreError::DomainAlreadyExists { instance_name } => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other_error => panic!("expected DomainAlreadyExists, but got: {:?}", other_error),
    }

    // Attempt restore with allow_overwrite = true succeeds
    let overwrite_options = RestoreOptions {
        allow_overwrite: true,
        ..Default::default()
    };
    restore_engine
        .restore_instance(&backup_directory, &overwrite_options, Some(&manifest))
        .expect("restore with allow_overwrite must succeed");
}

#[test]
fn test_disaster_recovery_fails_if_base_image_missing_from_both_pool_and_depot() {
    let workspace = DisasterRecoveryWorkspace::new(false);

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let backup_engine = BackupEngine::new(&hypervisor, &storage);
    let restore_engine = RestoreEngine::new(&hypervisor, &storage);

    let plan = planner.plan(&manifest).expect("plan");
    applier.apply(&plan, &ApplyOptions::default()).expect("apply");

    let backup_directory = workspace.depot_backup_directory.join("win11-gollum").join("missing-base-test");
    backup_engine
        .backup_instance("win11-gollum", &backup_directory, &BackupOptions::default(), Some(&manifest))
        .expect("backup instance");

    // Disaster: destroy instance, wipe base from pool, and wipe base from depot store
    destroyer
        .destroy("win11-gollum", &manifest, &DestroyOptions { delete_disk: true, allow_destroy_protected: true, ..Default::default() })
        .expect("destroy instance");

    let cached_master_path = workspace.default_pool_directory.join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
    storage.delete_image(&cached_master_path).expect("delete cached base");
    storage.delete_image(&workspace.initial_golden_master_path).expect("delete depot base");

    // Restore must fail fast with BaseImageNotFound
    let restore_error = restore_engine
        .restore_instance(&backup_directory, &RestoreOptions::default(), Some(&manifest))
        .expect_err("restore must fail when base image is missing from depot store");

    match restore_error {
        RestoreError::BaseImageNotFound { path } => {
            assert_eq!(path, workspace.initial_golden_master_path);
        }
        other_error => panic!("expected BaseImageNotFound, but got: {:?}", other_error),
    }
}
