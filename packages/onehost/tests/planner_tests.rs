use std::path::{Path, PathBuf};

use onehost::config::loader::ManifestLoader;
use onehost::hypervisor::mock::MockHypervisor;
use onehost::hypervisor::traits::{BlockDeviceInfo, DomainInfo, DomainState};
use onehost::image::tag::FlavorResolutionError;
use onehost::lifecycle::{
    DanglingSnapshotAlert, DomainLifecyclePlanner, InstancePlanAction, LifecycleError,
};
use onehost::storage::mock::{MockImageRecord, MockStorageManager};

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

const SAMPLE_TEMPLATE_XML: &str = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>TEMPLATE-PLACEHOLDER</name>
  <uuid>00000000-0000-0000-0000-000000000000</uuid>
  <memory unit='KiB'>16777216</memory>
  <vcpu placement='static'>16</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-9.1'>hvm</type>
    <loader readonly='yes' type='pflash'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd'>/placeholder/nvram.fd</nvram>
  </os>
  <devices>
    <disk type='file' device='disk' onehost:role='os-disk'>
      <target dev='sda' bus='sata'/>
    </disk>
  </devices>
</domain>"#;

const DEFAULT_POOL_XML: &str = r#"<pool type='dir'>
  <name>default</name>
  <target>
    <path>/var/lib/libvirt/images</path>
  </target>
</pool>"#;

const BULK_DATA_POOL_XML: &str = r#"<pool type='dir'>
  <name>bulk-data</name>
  <target>
    <path>/data/kvm/images</path>
  </target>
</pool>"#;

const TEST_CONTENT_HASH: &str = "ba6eafb71234";

fn create_test_manifest() -> onehost::config::model::OnehostManifest {
    ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("should succeed")
}

fn create_test_storage() -> MockStorageManager {
    MockStorageManager::new().with_content_hash(
        Path::new("/nix/store/ba6eafb712345678-oemdrv-looking-glass"),
        TEST_CONTENT_HASH,
    )
}

#[test]
fn test_plan_creates_new_domain_when_unregistered_in_hypervisor() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    assert!(plan.dangling_snapshots.is_empty());

    match &plan.actions[0] {
        InstancePlanAction::Create {
            instance_name,
            concrete_xml,
            target_pool,
            overlay_path,
            base_cache_path,
            golden_master_path,
            nvram_path,
            nvram_template_path,
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(target_pool, "default");
            assert_eq!(
                overlay_path,
                Path::new("/var/lib/libvirt/images/win11-gollum.qcow2")
            );
            assert_eq!(
                base_cache_path,
                Path::new(
                    "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
                )
            );
            assert_eq!(
                golden_master_path,
                Path::new(
                    "/data/depot/virtualization/libvirt/store/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
                )
            );
            assert_eq!(
                nvram_path,
                Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd")
            );
            assert_eq!(
                nvram_template_path,
                Path::new("/run/libvirt/nix-ovmf/edk2-i386-vars.fd")
            );
            assert!(concrete_xml.contains("<name>win11-gollum</name>"));
            assert!(concrete_xml.contains("<uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>"));
            assert!(concrete_xml.contains("/var/lib/libvirt/images/win11-gollum.qcow2"));
        }
        other_action => unreachable!("expected Create action, but got: {:?}", other_action),
    }
}

#[test]
fn test_plan_emits_noop_when_live_state_matches_declared_state() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    // Get expected concrete XML from a preliminary plan
    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    }];

    let overlay_record = MockImageRecord {
        backing_file: Some(PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2",
        )),
        ..Default::default()
    };

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let storage = storage.with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        overlay_record,
    );

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    assert_eq!(
        plan.actions[0],
        InstancePlanAction::NoOp {
            instance_name: "win11-gollum".to_string()
        }
    );
    assert!(plan.dangling_snapshots.is_empty());
}

#[test]
fn test_plan_emits_update_domain_xml_when_xml_drift_detected() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    // Simulate drifted live XML: memory increased to 32 GiB
    let drifted_live_xml = expected_concrete_xml.replace("16777216", "33554432");

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    }];

    let overlay_record = MockImageRecord {
        backing_file: Some(PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2",
        )),
        ..Default::default()
    };

    let hypervisor = hypervisor
        .with_domain(domain_info, drifted_live_xml)
        .with_block_devices("win11-gollum", block_devices);

    let storage = storage.with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        overlay_record,
    );

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    match &plan.actions[0] {
        InstancePlanAction::UpdateDomainXml {
            instance_name,
            concrete_xml,
            diff_summary,
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert!(concrete_xml.contains("<memory unit=\"KiB\">16777216</memory>"));
            assert!(diff_summary.contains("Value mismatch"));
        }
        other_action => unreachable!(
            "expected UpdateDomainXml action, but got: {:?}",
            other_action
        ),
    }
}

#[test]
fn test_plan_emits_recreate_when_base_image_hash_changes() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    }];

    // Old backing file hash: c001cafe1234 instead of ba6eafb71234
    let overlay_record = MockImageRecord {
        backing_file: Some(PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-c001cafe1234.qcow2",
        )),
        ..Default::default()
    };

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let storage = storage.with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        overlay_record,
    );

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    match &plan.actions[0] {
        InstancePlanAction::Recreate {
            instance_name,
            reason,
            target_pool,
            ..
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(target_pool, "default");
            assert!(reason.contains("ba6eafb71234"));
            assert!(reason.contains("forces replacement"));
        }
        other_action => unreachable!("expected Recreate action, but got: {:?}", other_action),
    }
}

#[test]
fn test_plan_emits_recreate_when_base_image_release_tuple_changes_with_identical_hash() {
    // Manifest specifies enterprise edition instead of pro
    let manifest_json = SAMPLE_MANIFEST_JSON.replace(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/enterprise/en-us/looking-glass",
    );
    let manifest = ManifestLoader::load_from_json_string(&manifest_json).expect("should succeed");
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    }];

    // Existing backing file has the old 'pro' release tuple, but identical content hash ba6eafb71234
    let overlay_record = MockImageRecord {
        backing_file: Some(PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2",
        )),
        ..Default::default()
    };

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let storage = storage.with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        overlay_record,
    );

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    match &plan.actions[0] {
        InstancePlanAction::Recreate {
            instance_name,
            reason,
            target_pool,
            ..
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(target_pool, "default");
            assert!(
                reason.contains("ce863cb6-6967-4632-a567-eb965a6f2ea3-enterprise-en-us-looking-glass-ba6eafb71234.qcow2")
                    || reason.contains("enterprise"),
                "reason did not mention expected new base image: {reason}"
            );
        }
        other_action => unreachable!("expected Recreate action, but got: {:?}", other_action),
    }
}

#[test]
fn test_plan_emits_relocate_storage_pool_when_pool_differs_with_unchanged_hash() {
    // Manifest declares instance on 'bulk-data' pool
    let manifest_json =
        SAMPLE_MANIFEST_JSON.replace("\"pool\": \"default\"", "\"pool\": \"bulk-data\"");
    let manifest = ManifestLoader::load_from_json_string(&manifest_json).expect("should succeed");

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", DEFAULT_POOL_XML)
        .with_pool_xml("bulk-data", BULK_DATA_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: false,
    };

    // Live domain is currently residing in /var/lib/libvirt/images/ (pool 'default')
    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    }];

    let overlay_record = MockImageRecord {
        backing_file: Some(PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2",
        )),
        ..Default::default()
    };

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let storage = storage.with_image(
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2"),
        overlay_record,
    );

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.actions.len(), 1);
    match &plan.actions[0] {
        InstancePlanAction::RelocateStoragePool {
            instance_name,
            source_pool,
            target_pool,
            source_overlay_path,
            target_overlay_path,
            target_base_cache_path,
            golden_master_path,
            ..
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(source_pool, "default");
            assert_eq!(target_pool, "bulk-data");
            assert_eq!(
                source_overlay_path,
                Path::new("/var/lib/libvirt/images/win11-gollum.qcow2")
            );
            assert_eq!(
                target_overlay_path,
                Path::new("/data/kvm/images/win11-gollum.qcow2")
            );
            assert_eq!(
                target_base_cache_path,
                Path::new(
                    "/data/kvm/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
                )
            );
            assert_eq!(
                golden_master_path,
                Path::new(
                    "/data/depot/virtualization/libvirt/store/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
                )
            );
        }
        other_action => unreachable!(
            "expected RelocateStoragePool action, but got: {:?}",
            other_action
        ),
    }
}

#[test]
fn test_plan_detects_dangling_snapshot_from_interrupted_backup() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("should succeed");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    // Live domain is attached to a temporary backup snapshot file
    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.snap")),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner.plan(&manifest).expect("planning should succeed");

    assert_eq!(plan.dangling_snapshots.len(), 1);
    assert_eq!(
        plan.dangling_snapshots[0],
        DanglingSnapshotAlert {
            instance_name: "win11-gollum".to_string(),
            target_device: "sda".to_string(),
            snapshot_path: PathBuf::from("/var/lib/libvirt/images/win11-gollum.snap"),
        }
    );
}

#[test]
fn test_plan_returns_error_when_flavor_not_registered() {
    // bypass high-level loader validation to specifically verify planner behavior
    let mut manifest = ManifestLoader::load_from_json_string(SAMPLE_MANIFEST_JSON).expect("should succeed");
    if let Some(instance) = manifest.instances.get_mut("win11-gollum") {
        instance.image = "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/unregistered-flavor"
            .parse()
            .expect("should succeed");
    }

    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan_result = planner.plan(&manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("should fail") {
        LifecycleError::FlavorResolutionError {
            source:
                FlavorResolutionError::UnknownFlavor {
                    requested_flavor, ..
                },
        } => {
            assert_eq!(requested_flavor, "unregistered-flavor");
        }
        other_error => unreachable!("expected FlavorResolutionError, but got: {:?}", other_error),
    }
}

#[test]
fn test_plan_propagates_hypervisor_pool_resolution_error() {
    let manifest = create_test_manifest();
    // Hypervisor has NO pool XML configured, so pool resolution will fail
    let hypervisor = MockHypervisor::new();
    let storage = create_test_storage();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan_result = planner.plan(&manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("should fail") {
        LifecycleError::HypervisorError { .. } => {}
        other_error => unreachable!("expected HypervisorError, but got: {:?}", other_error),
    }
}

#[test]
fn test_plan_propagates_domain_synthesis_error_on_invalid_template() {
    let manifest = create_test_manifest();
    let hypervisor = MockHypervisor::new().with_pool_xml("default", DEFAULT_POOL_XML);
    let storage = create_test_storage();

    // Template XML is missing the root <domain> tag
    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage).with_template_resolver(
        |_| Ok("<invalid>Not a domain template</invalid>".to_string()),
    );

    let plan_result = planner.plan(&manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("should fail") {
        LifecycleError::DomainSynthesisError { .. } => {}
        other_error => unreachable!("expected DomainSynthesisError, but got: {:?}", other_error),
    }
}
