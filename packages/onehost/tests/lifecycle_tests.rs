use tempfile::tempdir;

use onehost::config::loader::ManifestLoader;
use onehost::hypervisor::mock::{MockHypervisor, RecordedHypervisorAction};
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost::lifecycle::LifecycleError;
use onehost::lifecycle::applier::DomainLifecycleApplier;
use onehost::lifecycle::destroyer::{DestroyOptions, DomainLifecycleDestroyer};
use onehost::lifecycle::planner::{InstancePlanAction, OnehostPlan};
use onehost::storage::qemu_img::QemuImgStorage;

const SAMPLE_MANIFEST_JSON: &str = r#"{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
  "version": "1.0",
  "storage": {
    "depot_store_dir": "/data/depot/virtualization/libvirt/store",
    "depot_iso_dir": "/data/depot/virtualization/libvirt/iso",
    "depot_backup_dir": "/data/depot/virtualization/libvirt/backup",
    "nvram_dir": "/var/lib/libvirt/qemu/nvram",
    "nvram_template": "/run/libvirt/nix-ovmf/edk2-i386-vars.fd",
    "ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd"
  },
  "flavors": {
    "looking-glass": {
      "oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass",
      "extra_isos": {}
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e12345678-win11-template.xml",
      "image": "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
      "pool": "default"
    }
  }
}"#;

const CONCRETE_XML: &str = "<domain><name>win11-gollum</name></domain>";

#[test]
fn test_apply_create_provisions_storage_and_registers_domain() {
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let golden_master_path = depot_directory.join("win11-base.qcow2");
    storage
        .create_empty_disk(&golden_master_path, 20 * 1024 * 1024)
        .expect("create golden master");

    let nvram_template_path = temporary_directory.path().join("edk2-vars.fd");
    std::fs::write(&nvram_template_path, b"NVRAM_TEMPLATE_VARS").expect("write nvram template");

    let base_cache_path = pool_directory.join("win11-base-cached.qcow2");
    let overlay_path = pool_directory.join("win11-gollum.qcow2");
    let nvram_path = nvram_directory.join("win11-gollum_VARS.fd");

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_directory.display()
    );

    let hypervisor = MockHypervisor::new().with_pool_xml("default", pool_xml);

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Create {
            instance_name: "win11-gollum".to_string(),
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: overlay_path.clone(),
            base_cache_path: base_cache_path.clone(),
            golden_master_path,
            nvram_path: nvram_path.clone(),
            nvram_template_path,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan)
        .expect("apply create should succeed");

    // Verify physical storage state
    assert!(
        base_cache_path.exists(),
        "Base cache image must exist on disk"
    );
    let base_inspection = storage
        .inspect_image(&base_cache_path)
        .expect("inspect base cache");
    assert_eq!(base_inspection.format, "qcow2");
    assert_eq!(base_inspection.virtual_size_bytes, 20 * 1024 * 1024);
    assert_eq!(base_inspection.backing_file, None);
    assert!(storage.check_image(&base_cache_path).is_ok());

    assert!(overlay_path.exists(), "CoW overlay must exist on disk");
    let overlay_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay image");
    assert_eq!(overlay_inspection.format, "qcow2");
    assert_eq!(overlay_inspection.backing_file, Some(base_cache_path));
    assert!(storage.check_image(&overlay_path).is_ok());

    assert!(nvram_path.exists(), "NVRAM file must exist on disk");
    assert_eq!(
        std::fs::read(&nvram_path).expect("read nvram"),
        b"NVRAM_TEMPLATE_VARS"
    );

    // Verify hypervisor actions
    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
            domain_xml: CONCRETE_XML.to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "default".to_string(),
        })
    );
}

#[test]
fn test_apply_update_domain_xml_redefines_domain() {
    let pool_directory = tempdir().expect("create tempdir");
    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_directory.path().display()
    );
    let hypervisor = MockHypervisor::new().with_pool_xml("default", pool_xml);
    let storage = QemuImgStorage::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::UpdateDomainXml {
            instance_name: "win11-gollum".to_string(),
            concrete_xml: "<domain><name>win11-gollum</name><vcpu>32</vcpu></domain>".to_string(),
            diff_summary: "Value mismatch at /domain/vcpu".to_string(),
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan)
        .expect("apply update should succeed");

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
            domain_xml: "<domain><name>win11-gollum</name><vcpu>32</vcpu></domain>".to_string(),
        })
    );
}

#[test]
fn test_apply_recreate_succeeds_with_auto_approve() {
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    let nvram_directory = temporary_directory.path().join("nvram");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");
    std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");

    let storage = QemuImgStorage::new();

    let new_golden_master = depot_directory.join("new_master.qcow2");
    storage
        .create_empty_disk(&new_golden_master, 20 * 1024 * 1024)
        .expect("create new golden master");

    let old_overlay_path = pool_directory.join("win11-gollum.qcow2");
    storage
        .create_empty_disk(&old_overlay_path, 10 * 1024 * 1024)
        .expect("create old overlay");

    let nvram_template_path = temporary_directory.path().join("template.fd");
    std::fs::write(&nvram_template_path, b"NEW_NVRAM_TEMPLATE").expect("write nvram template");

    let new_base_cache_path = pool_directory.join("new_base.qcow2");
    let nvram_path = nvram_directory.join("vars.fd");

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_directory.display()
    );

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", pool_xml)
        .with_domain(domain_info, CONCRETE_XML);

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Recreate {
            instance_name: "win11-gollum".to_string(),
            reason: "Base image changed (forces replacement)".to_string(),
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: old_overlay_path.clone(),
            base_cache_path: new_base_cache_path.clone(),
            golden_master_path: new_golden_master,
            nvram_path: nvram_path.clone(),
            nvram_template_path,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan)
        .expect("apply recreate should succeed");

    // Physical storage state verification
    assert!(new_base_cache_path.exists());
    let base_inspected = storage
        .inspect_image(&new_base_cache_path)
        .expect("inspect base");
    assert_eq!(base_inspected.virtual_size_bytes, 20 * 1024 * 1024);
    assert!(storage.check_image(&new_base_cache_path).is_ok());

    assert!(old_overlay_path.exists());
    let overlay_inspected = storage
        .inspect_image(&old_overlay_path)
        .expect("inspect recreated overlay");
    assert_eq!(overlay_inspected.backing_file, Some(new_base_cache_path));
    assert!(storage.check_image(&old_overlay_path).is_ok());

    assert!(nvram_path.exists());
    assert_eq!(
        std::fs::read(&nvram_path).expect("read nvram"),
        b"NEW_NVRAM_TEMPLATE"
    );

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
            domain_name: "win11-gollum".to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
            domain_name: "win11-gollum".to_string(),
            cleanup_nvram: true,
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
            domain_xml: CONCRETE_XML.to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "default".to_string(),
        })
    );
}

#[test]
fn test_apply_relocate_storage_pool_moves_overlay_and_rebases() {
    let temporary_directory = tempdir().expect("create tempdir");
    let depot_directory = temporary_directory.path().join("depot");
    let default_pool_directory = temporary_directory.path().join("default_pool");
    let bulk_pool_directory = temporary_directory.path().join("bulk_pool");

    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&default_pool_directory).expect("create default pool dir");
    std::fs::create_dir_all(&bulk_pool_directory).expect("create bulk pool dir");

    let storage = QemuImgStorage::new();

    let golden_master_path = depot_directory.join("master.qcow2");
    storage
        .create_empty_disk(&golden_master_path, 20 * 1024 * 1024)
        .expect("create master");

    let source_overlay_path = default_pool_directory.join("win11-gollum.qcow2");
    storage
        .create_cow_overlay(&golden_master_path, &source_overlay_path)
        .expect("create source overlay");

    let target_base_cache_path = bulk_pool_directory.join("base.qcow2");
    let target_overlay_path = bulk_pool_directory.join("win11-gollum.qcow2");

    let default_pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        default_pool_directory.display()
    );
    let bulk_pool_xml = format!(
        "<pool type='dir'><name>bulk-data</name><target><path>{}</path></target></pool>",
        bulk_pool_directory.display()
    );

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: false,
    };

    let concrete_xml = format!(
        "<domain><name>win11-gollum</name><source file='{}'/></domain>",
        target_overlay_path.display()
    );

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", default_pool_xml)
        .with_pool_xml("bulk-data", bulk_pool_xml)
        .with_domain(domain_info, CONCRETE_XML);

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::RelocateStoragePool {
            instance_name: "win11-gollum".to_string(),
            source_pool: "default".to_string(),
            target_pool: "bulk-data".to_string(),
            source_overlay_path: source_overlay_path.clone(),
            target_overlay_path: target_overlay_path.clone(),
            target_base_cache_path: target_base_cache_path.clone(),
            golden_master_path,
            concrete_xml: concrete_xml.clone(),
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan)
        .expect("relocate storage pool should succeed");

    // Physical storage state verification
    assert!(
        !source_overlay_path.exists(),
        "Source overlay must no longer exist after move"
    );
    assert!(
        target_overlay_path.exists(),
        "Target overlay must exist in target pool"
    );
    assert!(
        target_base_cache_path.exists(),
        "Target base cache must exist in target pool"
    );

    let inspected_target = storage
        .inspect_image(&target_overlay_path)
        .expect("inspect relocated overlay");
    assert_eq!(inspected_target.backing_file, Some(target_base_cache_path.clone()));
    assert!(storage.check_image(&target_overlay_path).is_ok());
    assert!(storage.check_image(&target_base_cache_path).is_ok());

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
            domain_name: "win11-gollum".to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
            domain_xml: concrete_xml,
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "default".to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "bulk-data".to_string(),
        })
    );
}

#[test]
fn test_apply_delete_action_deprovisions() {
    let pool_directory = tempdir().expect("create tempdir");
    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_directory.path().display()
    );
    let hypervisor = MockHypervisor::new().with_pool_xml("default", pool_xml);
    let storage = QemuImgStorage::new();

    let delete_plan = OnehostPlan {
        actions: vec![InstancePlanAction::Delete {
            instance_name: "win11-gollum".to_string(),
            target_pool: "default".to_string(),
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&delete_plan)
        .expect("delete action should succeed");

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
            domain_name: "win11-gollum".to_string(),
            cleanup_nvram: true,
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "default".to_string(),
        })
    );
}

#[test]
fn test_destroyer_graceful_shutdown_and_undefine() {
    let pool_directory = tempdir().expect("create tempdir");
    let pool_path = pool_directory.path();
    let base_image_path = pool_path.join("base.qcow2");
    let overlay_path = pool_path.join("win11-gollum.qcow2");

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_path.display()
    );

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&base_image_path, 20 * 1024 * 1024)
        .expect("create base image");
    storage
        .create_cow_overlay(&base_image_path, &overlay_path)
        .expect("create overlay on disk");

    assert!(overlay_path.exists(), "overlay must exist before destroy");
    let pre_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay before destroy");
    assert_eq!(pre_inspection.backing_file, Some(base_image_path.clone()));
    assert!(storage.check_image(&overlay_path).is_ok());

    let manifest =
        ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("load sample manifest");

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", pool_xml)
        .with_domain(domain_info, CONCRETE_XML);

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: false,
                delete_disk: false,
            },
        )
        .expect("graceful destroy should succeed");

    assert!(
        overlay_path.exists(),
        "overlay disk must be physically preserved when delete_disk is false"
    );
    let post_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect preserved overlay");
    assert_eq!(post_inspection.backing_file, Some(base_image_path));
    assert!(storage.check_image(&overlay_path).is_ok());

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
            domain_name: "win11-gollum".to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
            domain_name: "win11-gollum".to_string(),
            cleanup_nvram: true,
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
            pool_name: "default".to_string(),
        })
    );
}

#[test]
fn test_destroyer_force_cuts_power_when_force_flag_is_set() {
    let pool_directory = tempdir().expect("create tempdir");
    let pool_path = pool_directory.path();
    let base_image_path = pool_path.join("base.qcow2");
    let overlay_path = pool_path.join("win11-gollum.qcow2");

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_path.display()
    );

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&base_image_path, 20 * 1024 * 1024)
        .expect("create base image");
    storage
        .create_cow_overlay(&base_image_path, &overlay_path)
        .expect("create overlay on disk");

    assert!(overlay_path.exists(), "overlay must exist before destroy");
    let pre_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay before destroy");
    assert_eq!(pre_inspection.backing_file, Some(base_image_path.clone()));
    assert!(storage.check_image(&overlay_path).is_ok());

    let manifest =
        ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("load sample manifest");

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", pool_xml)
        .with_domain(domain_info, CONCRETE_XML);

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: true,
                delete_disk: false,
            },
        )
        .expect("force destroy should succeed");

    assert!(
        overlay_path.exists(),
        "overlay disk must be physically preserved when delete_disk is false"
    );
    let post_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect preserved overlay");
    assert_eq!(post_inspection.backing_file, Some(base_image_path));
    assert!(storage.check_image(&overlay_path).is_ok());

    let hypervisor_actions = hypervisor
        .state
        .lock()
        .expect("lock state")
        .recorded_actions
        .clone();
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::DestroyDomain {
            domain_name: "win11-gollum".to_string(),
        })
    );
    assert!(
        hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
            domain_name: "win11-gollum".to_string(),
            cleanup_nvram: true,
        })
    );
}

#[test]
fn test_destroyer_deletes_cow_overlay_when_delete_disk_is_true() {
    let pool_directory = tempdir().expect("create tempdir");
    let pool_path = pool_directory.path();
    let base_image_path = pool_path.join("base.qcow2");
    let overlay_path = pool_path.join("win11-gollum.qcow2");

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_path.display()
    );

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&base_image_path, 20 * 1024 * 1024)
        .expect("create base image");
    storage
        .create_cow_overlay(&base_image_path, &overlay_path)
        .expect("create overlay on disk");

    assert!(overlay_path.exists(), "overlay must exist before destroy");
    let pre_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay before destroy");
    assert_eq!(pre_inspection.backing_file, Some(base_image_path.clone()));
    assert!(storage.check_image(&overlay_path).is_ok());

    let manifest =
        ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("load sample manifest");
    let hypervisor = MockHypervisor::new().with_pool_xml("default", pool_xml);

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: false,
                delete_disk: true,
            },
        )
        .expect("destroy with delete_disk should succeed");

    assert!(
        !overlay_path.exists(),
        "overlay disk must be physically unlinked from disk"
    );
    assert!(
        storage.inspect_image(&overlay_path).is_err(),
        "inspecting unlinked overlay should fail"
    );
    assert!(
        base_image_path.exists(),
        "base image must be preserved after overlay is unlinked"
    );
    assert!(
        storage.check_image(&base_image_path).is_ok(),
        "base image integrity must remain intact"
    );
}

#[test]
fn test_destroyer_returns_instance_not_declared_for_unregistered_instance() {
    let manifest =
        ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("load sample manifest");
    let hypervisor = MockHypervisor::new();
    let storage = QemuImgStorage::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let result = destroyer.destroy("non-existent-vm", &manifest, &DestroyOptions::default());

    let error = result.expect_err("expected InstanceNotDeclared error");
    assert!(matches!(
        error,
        LifecycleError::InstanceNotDeclared { instance_name } if instance_name == "non-existent-vm"
    ));
}

#[test]
fn test_destroyer_idempotent_undefine_ignores_domain_not_found_but_propagates_hypervisor_error() {
    let manifest =
        ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("load sample manifest");
    let pool_directory = tempdir().expect("create tempdir");
    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_directory.path().display()
    );

    // Case 1: Domain does not exist in hypervisor -> idempotent success
    let hypervisor_without_domain = MockHypervisor::new().with_pool_xml("default", pool_xml.clone());
    let storage = QemuImgStorage::new();
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor_without_domain, &storage);

    let result = destroyer.destroy("win11-gollum", &manifest, &DestroyOptions::default());
    assert!(
        result.is_ok(),
        "destroying when domain is not found in hypervisor must succeed idempotently"
    );

    // Case 2: Hypervisor undefine fails with fatal error -> bubbles up error
    let hypervisor_with_error = MockHypervisor::new()
        .with_pool_xml("default", pool_xml)
        .with_injected_domain_error("win11-gollum", "Libvirt daemon disconnected");
    let destroyer_error = DomainLifecycleDestroyer::new(&hypervisor_with_error, &storage);

    let error_result =
        destroyer_error.destroy("win11-gollum", &manifest, &DestroyOptions::default());
    assert!(error_result.is_err());
    assert!(matches!(
        error_result.expect_err("hypervisor error expected"),
        LifecycleError::HypervisorError { .. }
    ));
}
