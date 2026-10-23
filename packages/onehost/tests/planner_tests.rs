use std::collections::HashMap;
use std::path::PathBuf;

use onehost::config::model::{
    FlavorConfiguration, InstanceConfiguration, OnehostManifest, StorageDirectoriesConfiguration,
};
use onehost::hypervisor::traits::{BlockDeviceInfo, DomainInfo, DomainState};
use onehost_test_support::FakeHypervisor;
use onehost::image::tag::FlavorResolutionError;
use onehost::lifecycle::{
    DanglingSnapshotAlert, DomainLifecyclePlanner, InstancePlanAction, LifecycleError,
};
use onehost::storage::QemuImgStorage;

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

pub struct PlannerTestFixture {
    pub temporary_directory: tempfile::TempDir,
    pub manifest: OnehostManifest,
    pub depot_store_dir: PathBuf,
    pub nvram_dir: PathBuf,
    pub pool_directory: PathBuf,
    pub bulk_pool_directory: PathBuf,
    pub content_hash: String,
    pub storage: QemuImgStorage,
}

impl PlannerTestFixture {
    fn new() -> Self {
        let temporary_directory = tempfile::tempdir().expect("create tempdir");
        let temporary_path = temporary_directory.path();

        let depot_store_dir = temporary_path.join("depot/store");
        let depot_iso_dir = temporary_path.join("depot/iso");
        let depot_backup_dir = temporary_path.join("depot/backup");
        let nvram_dir = temporary_path.join("nvram");
        let nvram_template = temporary_path.join("edk2-vars.fd");
        let ovmf_code = temporary_path.join("edk2-code.fd");
        let pool_directory = temporary_path.join("pool-default");
        let bulk_pool_directory = temporary_path.join("pool-bulk");

        std::fs::create_dir_all(&depot_store_dir).expect("create depot store");
        std::fs::create_dir_all(&depot_iso_dir).expect("create depot iso");
        std::fs::create_dir_all(&depot_backup_dir).expect("create depot backup");
        std::fs::create_dir_all(&nvram_dir).expect("create nvram dir");
        std::fs::create_dir_all(&pool_directory).expect("create pool dir");
        std::fs::create_dir_all(&bulk_pool_directory).expect("create bulk pool dir");
        std::fs::write(&nvram_template, b"NVRAM_TEMPLATE").expect("write nvram template");
        std::fs::write(&ovmf_code, b"OVMF_CODE").expect("write ovmf code");

        let oemdrv_path = temporary_path.join("oemdrv");
        std::fs::write(&oemdrv_path, b"test-oemdrv-content").expect("write oemdrv");
        let extra_isos = HashMap::new();
        let content_hash =
            onehost::storage::execute_compute_content_hash(&oemdrv_path, &extra_isos)
                .expect("compute content hash");

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_dir.clone(),
            depot_iso_dir,
            depot_backup_dir,
            nvram_dir.clone(),
            nvram_template,
            ovmf_code,
        );

        let mut flavors = HashMap::new();
        flavors.insert(
            "looking-glass".to_string(),
            FlavorConfiguration::new(oemdrv_path, extra_isos),
        );

        let mut instances = HashMap::new();
        instances.insert(
            "win11-gollum".to_string(),
            InstanceConfiguration::new(
                "e5a7d620-8931-4bf6-98ec-7e44a30e8c45"
                    .parse()
                    .expect("valid uuid"),
                PathBuf::from("/nix/store/7q8w9e12345678-win11-template.xml"),
                "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass"
                    .parse()
                    .expect("valid image tag"),
                "default",
            ),
        );

        let manifest = OnehostManifest::new("1.0", storage_config, flavors, instances)
            .expect("manifest validation succeeds");

        Self {
            temporary_directory,
            manifest,
            depot_store_dir,
            nvram_dir,
            pool_directory,
            bulk_pool_directory,
            content_hash,
            storage: QemuImgStorage::new(),
        }
    }

    fn default_pool_xml(&self) -> String {
        format!(
            "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
            self.pool_directory.display()
        )
    }

    fn bulk_pool_xml(&self) -> String {
        format!(
            "<pool type='dir'><name>bulk-data</name><target><path>{}</path></target></pool>",
            self.bulk_pool_directory.display()
        )
    }
}

#[test]
fn test_plan_creates_new_domain_when_unregistered_in_hypervisor() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&fixture.manifest)
        .expect("planning should succeed");

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
                &fixture.pool_directory.join("win11-gollum.qcow2")
            );
            assert_eq!(
                base_cache_path,
                &fixture.pool_directory.join(format!(
                    "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
                    fixture.content_hash
                ))
            );
            assert_eq!(
                golden_master_path,
                &fixture.depot_store_dir.join(format!(
                    "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
                    fixture.content_hash
                ))
            );
            assert_eq!(
                nvram_path,
                &fixture.nvram_dir.join("win11-gollum_VARS.fd")
            );
            assert_eq!(
                nvram_template_path,
                &fixture.manifest.storage.nvram_template
            );
            assert!(concrete_xml.contains("<name>win11-gollum</name>"));
            assert!(concrete_xml.contains("<uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>"));
            assert!(concrete_xml.contains(
                &fixture
                    .pool_directory
                    .join("win11-gollum.qcow2")
                    .to_string_lossy()
                    .to_string()
            ));
        }
        other_action => {
            unreachable!("expected Create action, but got: {:?}", other_action);
        }
    }
}

#[test]
fn test_plan_emits_noop_when_live_state_matches_declared_state() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&fixture.manifest).expect("initial plan");
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

    let base_cache_path = fixture.pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        fixture.content_hash
    ));
    let overlay_path = fixture.pool_directory.join("win11-gollum.qcow2");
    fixture
        .storage
        .create_empty_disk(&base_cache_path, 20 * 1024 * 1024)
        .expect("create base");
    fixture
        .storage
        .create_cow_overlay(&base_cache_path, &overlay_path)
        .expect("create overlay");

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(overlay_path),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&fixture.manifest)
        .expect("planning should succeed");

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
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&fixture.manifest).expect("initial plan");
    let expected_concrete_xml = match &initial_plan.actions[0] {
        InstancePlanAction::Create { concrete_xml, .. } => concrete_xml.clone(),
        _ => unreachable!(),
    };

    let drifted_live_xml = expected_concrete_xml.replace("16777216", "33554432");

    let domain_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(16),
        memory_kib: Some(16777216),
        autostart: true,
    };

    let base_cache_path = fixture.pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        fixture.content_hash
    ));
    let overlay_path = fixture.pool_directory.join("win11-gollum.qcow2");
    fixture
        .storage
        .create_empty_disk(&base_cache_path, 20 * 1024 * 1024)
        .expect("create base");
    fixture
        .storage
        .create_cow_overlay(&base_cache_path, &overlay_path)
        .expect("create overlay");

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(overlay_path),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, drifted_live_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&fixture.manifest)
        .expect("planning should succeed");

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
        other_action => {
            unreachable!("expected UpdateDomainXml action, but got: {other_action:?}");
        }
    }
}

#[test]
fn test_plan_emits_recreate_when_base_image_hash_changes() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&fixture.manifest).expect("initial plan");
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

    let old_base_path = fixture.pool_directory.join(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-c001cafe1234.qcow2",
    );
    let overlay_path = fixture.pool_directory.join("win11-gollum.qcow2");
    fixture
        .storage
        .create_empty_disk(&old_base_path, 20 * 1024 * 1024)
        .expect("create old base");
    fixture
        .storage
        .create_cow_overlay(&old_base_path, &overlay_path)
        .expect("create overlay");

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(overlay_path),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&fixture.manifest)
        .expect("planning should succeed");

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
            assert!(reason.contains(&fixture.content_hash));
            assert!(reason.contains("forces replacement"));
        }
        other_action => {
            unreachable!("expected Recreate action, but got: {other_action:?}");
        }
    }
}

#[test]
fn test_plan_emits_recreate_when_base_image_release_tuple_changes_with_identical_hash() {
    let fixture = PlannerTestFixture::new();
    let mut manifest = fixture.manifest.clone();
    if let Some(instance) = manifest.instances.get_mut("win11-gollum") {
        instance.image = "ce863cb6-6967-4632-a567-eb965a6f2ea3/enterprise/en-us/looking-glass"
            .parse()
            .expect("image tag parsing");
    }

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("initial plan");
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

    let old_pro_base_path = fixture.pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        fixture.content_hash
    ));
    let overlay_path = fixture.pool_directory.join("win11-gollum.qcow2");
    fixture
        .storage
        .create_empty_disk(&old_pro_base_path, 20 * 1024 * 1024)
        .expect("create old pro base");
    fixture
        .storage
        .create_cow_overlay(&old_pro_base_path, &overlay_path)
        .expect("create overlay");

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(overlay_path),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&manifest)
        .expect("planning should succeed");

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
                reason.contains("enterprise"),
                "reason did not mention expected new base image: {reason}"
            );
        }
        other_action => {
            unreachable!("expected Recreate action, but got: {other_action:?}");
        }
    }
}

#[test]
fn test_plan_emits_relocate_storage_pool_when_pool_differs_with_unchanged_hash() {
    let fixture = PlannerTestFixture::new();
    let mut manifest = fixture.manifest.clone();
    if let Some(instance) = manifest.instances.get_mut("win11-gollum") {
        instance.pool = "bulk-data".to_string();
    }

    let hypervisor = FakeHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_pool_xml("bulk-data", fixture.bulk_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&manifest).expect("initial plan");
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

    let base_cache_path = fixture.pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        fixture.content_hash
    ));
    let overlay_path = fixture.pool_directory.join("win11-gollum.qcow2");
    fixture
        .storage
        .create_empty_disk(&base_cache_path, 20 * 1024 * 1024)
        .expect("create base");
    fixture
        .storage
        .create_cow_overlay(&base_cache_path, &overlay_path)
        .expect("create overlay");

    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(overlay_path.clone()),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&manifest)
        .expect("planning should succeed");

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
            assert_eq!(source_overlay_path, &overlay_path);
            assert_eq!(
                target_overlay_path,
                &fixture.bulk_pool_directory.join("win11-gollum.qcow2")
            );
            assert_eq!(
                target_base_cache_path,
                &fixture.bulk_pool_directory.join(format!(
                    "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
                    fixture.content_hash
                ))
            );
            assert_eq!(
                golden_master_path,
                &fixture.depot_store_dir.join(format!(
                    "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
                    fixture.content_hash
                ))
            );
        }
        other_action => {
            unreachable!("expected RelocateStoragePool action, but got: {other_action:?}");
        }
    }
}

#[test]
fn test_plan_detects_dangling_snapshot_from_interrupted_backup() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let initial_plan = planner.plan(&fixture.manifest).expect("initial plan");
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

    let snap_path = fixture.pool_directory.join("win11-gollum.snap");
    let block_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(snap_path.clone()),
        device_type: "disk".to_string(),
    }];

    let hypervisor = hypervisor
        .with_domain(domain_info, expected_concrete_xml)
        .with_block_devices("win11-gollum", block_devices);

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan = planner
        .plan(&fixture.manifest)
        .expect("planning should succeed");

    assert_eq!(plan.dangling_snapshots.len(), 1);
    assert_eq!(
        plan.dangling_snapshots[0],
        DanglingSnapshotAlert {
            instance_name: "win11-gollum".to_string(),
            target_device: "sda".to_string(),
            snapshot_path: snap_path,
        }
    );
}

#[test]
fn test_plan_returns_error_when_flavor_not_registered() {
    let fixture = PlannerTestFixture::new();
    let mut manifest = fixture.manifest.clone();
    if let Some(instance) = manifest.instances.get_mut("win11-gollum") {
        instance.image = "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/unregistered-flavor"
            .parse()
            .expect("image tag parsing");
    }

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan_result = planner.plan(&manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("is error") {
        LifecycleError::FlavorResolutionError {
            source:
                FlavorResolutionError::UnknownFlavor {
                    requested_flavor, ..
                },
        } => {
            assert_eq!(requested_flavor, "unregistered-flavor");
        }
        other_error => {
            unreachable!("expected FlavorResolutionError, but got: {other_error:?}");
        }
    }
}

#[test]
fn test_plan_propagates_hypervisor_pool_resolution_error() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new();

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage)
        .with_template_resolver(|_| Ok(SAMPLE_TEMPLATE_XML.to_string()));

    let plan_result = planner.plan(&fixture.manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("is error") {
        LifecycleError::HypervisorError { .. } => {}
        other_error => {
            unreachable!("expected HypervisorError, but got: {other_error:?}");
        }
    }
}

#[test]
fn test_plan_propagates_domain_synthesis_error_on_invalid_template() {
    let fixture = PlannerTestFixture::new();
    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    let planner = DomainLifecyclePlanner::new(&hypervisor, &fixture.storage).with_template_resolver(
        |_| Ok("<invalid>Not a domain template</invalid>".to_string()),
    );

    let plan_result = planner.plan(&fixture.manifest);

    assert!(plan_result.is_err());
    match plan_result.expect_err("is error") {
        LifecycleError::DomainSynthesisError { .. } => {}
        other_error => {
            unreachable!("expected DomainSynthesisError, but got: {other_error:?}");
        }
    }
}
