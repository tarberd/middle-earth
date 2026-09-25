use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

use onehost::config::loader::ManifestLoader;
use onehost::config::model::ImageChangePolicy;
use onehost::hypervisor::mock::{MockHypervisor, RecordedHypervisorAction};
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost::lifecycle::applier::{ApplyOptions, DomainLifecycleApplier};
use onehost::lifecycle::destroyer::{DestroyOptions, DomainLifecycleDestroyer};
use onehost::lifecycle::planner::{InstancePlanAction, OnehostPlan};
use onehost::lifecycle::LifecycleError;
use onehost::storage::mock::{MockImageRecord, MockStorageManager, RecordedStorageAction};

const SAMPLE_MANIFEST_JSON: &str = r#"{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
  "version": "1.0",
  "storage": {
    "depot_store_dir": "/data/depot/virtualization/libvirt/store",
    "depot_iso_dir": "/data/depot/virtualization/libvirt/iso",
    "depot_backup_dir": "/data/depot/virtualization/libvirt/backup",
    "nvram_dir": "/var/lib/libvirt/qemu/nvram",
    "nvram_template": "/run/libvirt/nix-ovmf/edk2-i386-vars.fd",
    "ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd",
    "default_pool": "default"
  },
  "flavors": {
    "looking-glass": {
      "oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass",
      "hash": "ba6eafb7"
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e12345678-win11-template.xml",
      "image": "win11/26300.9457.pro.en-us/looking-glass",
      "pool": "default",
      "autostart": true,
      "lifecycle": {
        "on_image_change": "protect",
        "prevent_destroy": false
      }
    }
  }
}"#;

const DEFAULT_POOL_XML: &str = r#"<pool type='dir'>
  <name>default</name>
  <target>
    <path>/var/lib/libvirt/images</path>
  </target>
</pool>"#;

const CONCRETE_XML: &str = "<domain><name>win11-gollum</name></domain>";

#[test]
fn test_apply_create_provisions_storage_and_registers_domain() {
    let temporary_directory = tempdir().unwrap();
    let nvram_directory = temporary_directory.path().join("nvram");
    let nvram_path = nvram_directory.join("win11-gollum_VARS.fd");

    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Create {
            instance_name: "win11-gollum".to_string(),
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            base_cache_path: PathBuf::from(
                "/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2",
            ),
            golden_master_path: PathBuf::from(
                "/data/depot/virtualization/libvirt/store/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2",
            ),
            nvram_path,
            nvram_template_path: PathBuf::from("/run/libvirt/nix-ovmf/edk2-i386-vars.fd"),
            autostart: true,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan, &ApplyOptions::default())
        .expect("apply create should succeed");

    // Verify storage actions
    let storage_actions = storage.state.lock().unwrap().recorded_actions.clone();
    assert!(storage_actions.contains(&RecordedStorageAction::CopyBaseImage {
        source_depot_path: PathBuf::from(
            "/data/depot/virtualization/libvirt/store/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2"
        ),
        destination_pool_path: PathBuf::from(
            "/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2"
        ),
    }));
    assert!(storage_actions.contains(&RecordedStorageAction::CreateCowOverlay {
        backing_file_path: PathBuf::from(
            "/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2"
        ),
        overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
    }));

    // Verify hypervisor actions
    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
        domain_xml: CONCRETE_XML.to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::SetAutostart {
        domain_name: "win11-gollum".to_string(),
        autostart: true,
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
}

#[test]
fn test_apply_update_domain_xml_redefines_domain() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

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
        .apply(&plan, &ApplyOptions::default())
        .expect("apply update should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
        domain_xml: "<domain><name>win11-gollum</name><vcpu>32</vcpu></domain>".to_string(),
    }));
}

#[test]
fn test_apply_recreate_aborts_when_prevent_destroy_is_true() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Recreate {
            instance_name: "win11-gollum".to_string(),
            reason: "Base image changed (forces replacement)".to_string(),
            policy: ImageChangePolicy::Protect,
            prevent_destroy: true,
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            base_cache_path: PathBuf::from("/var/lib/libvirt/images/base.qcow2"),
            golden_master_path: PathBuf::from("/data/depot/store/master.qcow2"),
            nvram_path: PathBuf::from("/var/lib/libvirt/qemu/nvram/vars.fd"),
            nvram_template_path: PathBuf::from("/run/libvirt/template.fd"),
            autostart: false,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let result = applier.apply(
        &plan,
        &ApplyOptions {
            allow_recreate: true,
            auto_approve: true,
        },
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        LifecycleError::LifecyclePolicyViolation { instance_name, details } => {
            assert_eq!(instance_name, "win11-gollum");
            assert!(details.contains("prevent_destroy enabled"));
        }
        other_error => panic!("expected LifecyclePolicyViolation, got: {:?}", other_error),
    }
}

#[test]
fn test_apply_recreate_aborts_when_policy_is_protect_without_allow_recreate() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Recreate {
            instance_name: "win11-gollum".to_string(),
            reason: "Base image changed (forces replacement)".to_string(),
            policy: ImageChangePolicy::Protect,
            prevent_destroy: false,
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            base_cache_path: PathBuf::from("/var/lib/libvirt/images/base.qcow2"),
            golden_master_path: PathBuf::from("/data/depot/store/master.qcow2"),
            nvram_path: PathBuf::from("/var/lib/libvirt/qemu/nvram/vars.fd"),
            nvram_template_path: PathBuf::from("/run/libvirt/template.fd"),
            autostart: false,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let result = applier.apply(
        &plan,
        &ApplyOptions {
            allow_recreate: false,
            auto_approve: false,
        },
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        LifecycleError::LifecyclePolicyViolation { instance_name, details } => {
            assert_eq!(instance_name, "win11-gollum");
            assert!(details.contains("--allow-recreate flag"));
        }
        other_error => panic!("expected LifecyclePolicyViolation, got: {:?}", other_error),
    }
}

#[test]
fn test_apply_recreate_succeeds_when_allow_recreate_is_true() {
    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", DEFAULT_POOL_XML)
        .with_domain(domain_info, CONCRETE_XML);
    let storage = MockStorageManager::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Recreate {
            instance_name: "win11-gollum".to_string(),
            reason: "Base image changed (forces replacement)".to_string(),
            policy: ImageChangePolicy::Protect,
            prevent_destroy: false,
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            base_cache_path: PathBuf::from("/var/lib/libvirt/images/new_base.qcow2"),
            golden_master_path: PathBuf::from("/data/depot/store/new_master.qcow2"),
            nvram_path: PathBuf::from("/var/lib/libvirt/qemu/nvram/vars.fd"),
            nvram_template_path: PathBuf::from("/run/libvirt/template.fd"),
            autostart: true,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(
            &plan,
            &ApplyOptions {
                allow_recreate: true,
                auto_approve: true,
            },
        )
        .expect("apply recreate should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
        domain_name: "win11-gollum".to_string(),
        cleanup_nvram: true,
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
        domain_xml: CONCRETE_XML.to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::SetAutostart {
        domain_name: "win11-gollum".to_string(),
        autostart: true,
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
}

#[test]
fn test_apply_recreate_succeeds_when_policy_is_recreate_without_flag() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::Recreate {
            instance_name: "win11-gollum".to_string(),
            reason: "Base image changed (forces replacement)".to_string(),
            policy: ImageChangePolicy::Recreate,
            prevent_destroy: false,
            concrete_xml: CONCRETE_XML.to_string(),
            target_pool: "default".to_string(),
            overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            base_cache_path: PathBuf::from("/var/lib/libvirt/images/new_base.qcow2"),
            golden_master_path: PathBuf::from("/data/depot/store/new_master.qcow2"),
            nvram_path: PathBuf::from("/var/lib/libvirt/qemu/nvram/vars.fd"),
            nvram_template_path: PathBuf::from("/run/libvirt/template.fd"),
            autostart: false,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(
            &plan,
            &ApplyOptions {
                allow_recreate: false,
                auto_approve: true,
            },
        )
        .expect("cattle recreate should proceed");
}

#[test]
fn test_apply_relocate_storage_pool_moves_overlay_and_rebases() {
    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: false,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", DEFAULT_POOL_XML)
        .with_pool_xml(
            "bulk-data",
            "<pool type='dir'><name>bulk-data</name><target><path>/data/kvm/images</path></target></pool>",
        )
        .with_domain(domain_info, CONCRETE_XML);

    let storage = MockStorageManager::new().with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        MockImageRecord::default(),
    );

    let plan = OnehostPlan {
        actions: vec![InstancePlanAction::RelocateStoragePool {
            instance_name: "win11-gollum".to_string(),
            source_pool: "default".to_string(),
            target_pool: "bulk-data".to_string(),
            source_overlay_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
            target_overlay_path: PathBuf::from("/data/kvm/images/win11-gollum.qcow2"),
            target_base_cache_path: PathBuf::from("/data/kvm/images/base.qcow2"),
            golden_master_path: PathBuf::from("/data/depot/store/master.qcow2"),
            concrete_xml: "<domain><name>win11-gollum</name><source file='/data/kvm/images/win11-gollum.qcow2'/></domain>".to_string(),
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    applier
        .apply(&plan, &ApplyOptions::default())
        .expect("relocate storage pool should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::DefineDomain {
        domain_xml: "<domain><name>win11-gollum</name><source file='/data/kvm/images/win11-gollum.qcow2'/></domain>".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "bulk-data".to_string(),
    }));

    let storage_actions = storage.state.lock().unwrap().recorded_actions.clone();
    assert!(storage_actions.contains(&RecordedStorageAction::CopyBaseImage {
        source_depot_path: PathBuf::from("/data/depot/store/master.qcow2"),
        destination_pool_path: PathBuf::from("/data/kvm/images/base.qcow2"),
    }));
    assert!(storage_actions.contains(&RecordedStorageAction::MoveFileSafely {
        source_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        destination_path: PathBuf::from("/data/kvm/images/win11-gollum.qcow2"),
    }));
    assert!(storage_actions.contains(&RecordedStorageAction::RebaseOverlay {
        overlay_path: PathBuf::from("/data/kvm/images/win11-gollum.qcow2"),
        new_backing_file_path: PathBuf::from("/data/kvm/images/base.qcow2"),
        unsafe_mode: true,
    }));
}

#[test]
fn test_apply_delete_action_respects_prevent_destroy_and_deprovisions() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    // 1. Aborts if prevent_destroy is true
    let protected_plan = OnehostPlan {
        actions: vec![InstancePlanAction::Delete {
            instance_name: "win11-gollum".to_string(),
            target_pool: "default".to_string(),
            prevent_destroy: true,
        }],
        dangling_snapshots: Vec::new(),
    };

    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let abort_result = applier.apply(&protected_plan, &ApplyOptions::default());
    assert!(abort_result.is_err());

    // 2. Succeeds if prevent_destroy is false
    let delete_plan = OnehostPlan {
        actions: vec![InstancePlanAction::Delete {
            instance_name: "win11-gollum".to_string(),
            target_pool: "default".to_string(),
            prevent_destroy: false,
        }],
        dangling_snapshots: Vec::new(),
    };

    applier
        .apply(&delete_plan, &ApplyOptions::default())
        .expect("delete action should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
        domain_name: "win11-gollum".to_string(),
        cleanup_nvram: true,
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
}

#[test]
fn test_destroyer_aborts_when_prevent_destroy_without_override() {
    let mut manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).unwrap();
    manifest.instances.get_mut("win11-gollum").unwrap().lifecycle.prevent_destroy = true;

    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    let result = destroyer.destroy(
        "win11-gollum",
        &manifest,
        &DestroyOptions {
            force: false,
            delete_disk: false,
            allow_destroy_protected: false,
        },
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        LifecycleError::LifecyclePolicyViolation { instance_name, details } => {
            assert_eq!(instance_name, "win11-gollum");
            assert!(details.contains("prevent_destroy enabled"));
        }
        other_error => panic!("expected LifecyclePolicyViolation, got: {:?}", other_error),
    }
}

#[test]
fn test_destroyer_succeeds_with_allow_destroy_protected_override() {
    let mut manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).unwrap();
    manifest.instances.get_mut("win11-gollum").unwrap().lifecycle.prevent_destroy = true;

    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = MockStorageManager::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: false,
                delete_disk: false,
                allow_destroy_protected: true,
            },
        )
        .expect("destroyer should proceed when override is provided");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
        domain_name: "win11-gollum".to_string(),
        cleanup_nvram: true,
    }));
}

#[test]
fn test_destroyer_graceful_shutdown_and_undefine() {
    let manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).unwrap();

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", DEFAULT_POOL_XML)
        .with_domain(domain_info, CONCRETE_XML);
    let storage = MockStorageManager::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: false,
                delete_disk: false,
                allow_destroy_protected: false,
            },
        )
        .expect("graceful destroy should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::ShutdownDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
        domain_name: "win11-gollum".to_string(),
        cleanup_nvram: true,
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
}

#[test]
fn test_destroyer_force_cuts_power_when_force_flag_is_set() {
    let manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).unwrap();

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", DEFAULT_POOL_XML)
        .with_domain(domain_info, CONCRETE_XML);
    let storage = MockStorageManager::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: true,
                delete_disk: false,
                allow_destroy_protected: false,
            },
        )
        .expect("force destroy should succeed");

    let hypervisor_actions = hypervisor.state.lock().unwrap().recorded_actions.clone();
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::DestroyDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(hypervisor_actions.contains(&RecordedHypervisorAction::UndefineDomain {
        domain_name: "win11-gollum".to_string(),
        cleanup_nvram: true,
    }));
}

#[test]
fn test_destroyer_deletes_cow_overlay_when_delete_disk_is_true() {
    let temporary_directory = tempdir().unwrap();
    let pool_path = temporary_directory.path().to_path_buf();
    let overlay_path = pool_path.join("win11-gollum.qcow2");
    fs::write(&overlay_path, b"dummy-qcow2-data").unwrap();
    assert!(overlay_path.exists());

    let pool_xml = format!(
        "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
        pool_path.to_string_lossy()
    );

    let manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).unwrap();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", pool_xml);
    let storage = MockStorageManager::new();

    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);
    destroyer
        .destroy(
            "win11-gollum",
            &manifest,
            &DestroyOptions {
                force: false,
                delete_disk: true,
                allow_destroy_protected: false,
            },
        )
        .expect("destroy with delete_disk should succeed");

    let storage_actions = storage.state.lock().unwrap().recorded_actions.clone();
    assert!(storage_actions.contains(&RecordedStorageAction::DeleteImage {
        image_path: overlay_path.clone(),
    }));
    assert!(!overlay_path.exists(), "overlay disk should have been unlinked");
}
