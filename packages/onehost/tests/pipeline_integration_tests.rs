use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

use onehost::config::loader::ManifestLoader;
use onehost::hypervisor::mock::MockHypervisor;
use onehost::hypervisor::traits::{DomainState, Hypervisor};
use onehost::lifecycle::applier::{ApplyOptions, DomainLifecycleApplier};
use onehost::lifecycle::destroyer::{DestroyOptions, DomainLifecycleDestroyer};
use onehost::lifecycle::planner::{DomainLifecyclePlanner, InstancePlanAction};
use onehost::lifecycle::LifecycleError;
use onehost::storage::mock::{MockImageRecord, MockStorageManager};
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

const MODIFIED_TEMPLATE_XML: &str = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>TEMPLATE</name>
  <uuid>00000000-0000-0000-0000-000000000000</uuid>
  <vcpu placement='static'>8</vcpu>
  <memory unit='KiB'>16777216</memory>
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

struct TestWorkspace {
    _workspace_directory: tempfile::TempDir,
    manifest_path: std::path::PathBuf,
    template_xml_path: std::path::PathBuf,
    depot_store_directory: std::path::PathBuf,
    nvram_directory: std::path::PathBuf,
    default_pool_directory: std::path::PathBuf,
    nvme_pool_directory: std::path::PathBuf,
    initial_golden_master_path: std::path::PathBuf,
}

impl TestWorkspace {
    fn new(on_image_change_policy: &str) -> Self {
        let workspace = tempdir().expect("create workspace tempdir");
        let root = workspace.path();

        let depot_dir = root.join("depot");
        let depot_store = depot_dir.join("store");
        let depot_iso = depot_dir.join("iso");
        let depot_backup = depot_dir.join("backup");
        let nvram_dir = root.join("nvram");
        let default_pool_dir = root.join("default_pool");
        let nvme_pool_dir = root.join("nvme_pool");

        std::fs::create_dir_all(&depot_store).expect("create depot_store");
        std::fs::create_dir_all(&depot_iso).expect("create depot_iso");
        std::fs::create_dir_all(&depot_backup).expect("create depot_backup");
        std::fs::create_dir_all(&nvram_dir).expect("create nvram_dir");
        std::fs::create_dir_all(&default_pool_dir).expect("create default_pool_dir");
        std::fs::create_dir_all(&nvme_pool_dir).expect("create nvme_pool_dir");

        let nvram_template = root.join("edk2-vars.fd");
        let mut nvram_file = File::create(&nvram_template).expect("create nvram template");
        nvram_file
            .write_all(b"NVRAM_TEMPLATE_VARS")
            .expect("write nvram template");

        let template_xml = root.join("win11-template.xml");
        std::fs::write(&template_xml, SAMPLE_TEMPLATE_XML).expect("write template xml");

        let initial_master =
            depot_store.join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
        std::fs::write(&initial_master, b"GOLDEN_MASTER_QCOW2").expect("write golden master");

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
        "on_image_change": "{}",
        "prevent_destroy": false
      }}
    }}
  }}
}}"#,
            depot_store.display(),
            depot_iso.display(),
            depot_backup.display(),
            nvram_dir.display(),
            nvram_template.display(),
            template_xml.display(),
            on_image_change_policy
        );

        let manifest_file_path = root.join("onehost.json");
        std::fs::write(&manifest_file_path, manifest_content).expect("write manifest");

        Self {
            _workspace_directory: workspace,
            manifest_path: manifest_file_path,
            template_xml_path: template_xml,
            depot_store_directory: depot_store,
            nvram_directory: nvram_dir,
            default_pool_directory: default_pool_dir,
            nvme_pool_directory: nvme_pool_dir,
            initial_golden_master_path: initial_master,
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

    fn nvme_pool_xml(&self) -> String {
        format!(
            r#"<pool type='dir'>
  <name>fast-nvme</name>
  <target>
    <path>{}</path>
  </target>
</pool>"#,
            self.nvme_pool_directory.display()
        )
    }
}

fn file_template_resolver(template_path: &std::path::Path) -> Result<String, LifecycleError> {
    std::fs::read_to_string(template_path).map_err(LifecycleError::from)
}

fn count_pool_refreshes(hypervisor: &MockHypervisor, pool_name: &str) -> usize {
    hypervisor
        .state
        .lock()
        .expect("hypervisor state lock")
        .refreshed_pools
        .iter()
        .filter(|refreshed_pool| *refreshed_pool == pool_name)
        .count()
}

#[test]
fn test_end_to_end_fresh_provisioning_and_idempotent_noop() {
    let workspace = TestWorkspace::new("protect");

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    // 1. Load manifest from real disk path
    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path)
        .expect("manifest loading from disk must succeed");

    // 2. Generate plan
    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);

    let initial_plan = planner
        .plan(&manifest)
        .expect("initial planning must succeed");
    assert_eq!(initial_plan.actions.len(), 1);

    match &initial_plan.actions[0] {
        InstancePlanAction::Create { instance_name, .. } => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other_action => panic!("expected Create action, but got: {:?}", other_action),
    }

    // 3. Execute apply
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply must succeed");

    // 4. Verify hypervisor and storage state
    let domain_info = hypervisor
        .domain_info("win11-gollum")
        .expect("domain must be registered in hypervisor");
    assert_eq!(domain_info.name, "win11-gollum");
    assert_eq!(domain_info.state, DomainState::Shutoff);

    let registered_xml = hypervisor.dump_xml("win11-gollum", false).unwrap();
    assert!(registered_xml.contains("<name>win11-gollum</name>"));

    let expected_overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    let expected_overlay_fragment = format!(r#"<source file="{}"/>"#, expected_overlay_path.display());
    assert!(registered_xml.contains(&expected_overlay_fragment));
    assert!(!registered_xml.contains("onehost:role"));
    assert!(!registered_xml.contains("xmlns:onehost"));

    assert!(storage.inspect_image(&expected_overlay_path).is_ok());

    let nvram_target = workspace.nvram_directory.join("win11-gollum_VARS.fd");
    assert!(nvram_target.exists());

    // 5. Assert IDEMPOTENCY: second plan emits NoOp
    let idempotent_plan = planner
        .plan(&manifest)
        .expect("idempotent planning must succeed");
    assert_eq!(idempotent_plan.actions.len(), 1);

    match &idempotent_plan.actions[0] {
        InstancePlanAction::NoOp { instance_name } => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other_action => panic!("expected NoOp action, but got: {:?}", other_action),
    }
}

#[test]
fn test_end_to_end_template_drift_reconciliation_pipeline() {
    let workspace = TestWorkspace::new("protect");

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    // 1. Modify the template XML file on disk (increase vcpu to 8)
    std::fs::write(&workspace.template_xml_path, MODIFIED_TEMPLATE_XML)
        .expect("write modified template");

    // 2. Plan detects template drift
    let drift_plan = planner
        .plan(&manifest)
        .expect("planning after template modification");
    assert_eq!(drift_plan.actions.len(), 1);

    match &drift_plan.actions[0] {
        InstancePlanAction::UpdateDomainXml {
            instance_name,
            diff_summary,
            ..
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert!(diff_summary.contains("vcpu"));
        }
        other_action => panic!("expected UpdateDomainXml action, but got: {:?}", other_action),
    }

    // 3. Apply reconciliation without recreating overlay
    applier
        .apply(&drift_plan, &ApplyOptions::default())
        .expect("apply update");

    let updated_xml = hypervisor.dump_xml("win11-gollum", false).unwrap();
    assert!(updated_xml.contains(r#"<vcpu placement="static">8</vcpu>"#));

    // 4. Verify planner returns NoOp
    let final_plan = planner.plan(&manifest).expect("final plan");
    assert!(matches!(
        &final_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_storage_pool_relocation_pipeline() {
    let workspace = TestWorkspace::new("protect");

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", workspace.default_pool_xml())
        .with_pool_xml("fast-nvme", workspace.nvme_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision in 'default' pool
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    // 1. Edit manifest file on disk: change pool to 'fast-nvme'
    let modified_manifest_content = std::fs::read_to_string(&workspace.manifest_path)
        .expect("read manifest")
        .replace(r#""pool": "default""#, r#""pool": "fast-nvme""#);
    std::fs::write(&workspace.manifest_path, modified_manifest_content).expect("write manifest");

    // 2. Re-load manifest from disk
    let relocated_manifest =
        ManifestLoader::load_from_path(&workspace.manifest_path).expect("reload manifest");

    // 3. Plan detects pool relocation
    let relocation_plan = planner
        .plan(&relocated_manifest)
        .expect("plan relocation");
    assert_eq!(relocation_plan.actions.len(), 1);

    match &relocation_plan.actions[0] {
        InstancePlanAction::RelocateStoragePool {
            instance_name,
            source_pool,
            target_pool,
            ..
        } => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(source_pool, "default");
            assert_eq!(target_pool, "fast-nvme");
        }
        other_action => panic!(
            "expected RelocateStoragePool action, but got: {:?}",
            other_action
        ),
    }

    // 4. Apply relocation
    applier
        .apply(&relocation_plan, &ApplyOptions::default())
        .expect("apply relocation");

    // 5. Verify new overlay location and backing file rebase
    let expected_new_overlay = workspace.nvme_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&expected_new_overlay).is_ok());

    let rebased_inspection = storage.inspect_image(&expected_new_overlay).unwrap();
    let expected_new_base = workspace
        .nvme_pool_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
    assert_eq!(rebased_inspection.backing_file, Some(expected_new_base));

    // 6. Verify domain XML points to new disk
    let updated_domain_xml = hypervisor.dump_xml("win11-gollum", false).unwrap();
    assert!(updated_domain_xml.contains(&expected_new_overlay.display().to_string()));

    // 7. Verify both pools refreshed
    assert_eq!(count_pool_refreshes(&hypervisor, "default"), 2);
    assert_eq!(count_pool_refreshes(&hypervisor, "fast-nvme"), 1);

    // 8. Subsequent plan is NoOp
    let post_relocation_plan = planner
        .plan(&relocated_manifest)
        .expect("post relocation plan");
    assert!(matches!(
        &post_relocation_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_policy_guardrail_recreate_rejection_and_override() {
    let workspace = TestWorkspace::new("protect");

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");

    // 1. Simulate base image hash change (drift from ba6eafb7 -> c1d2e3f4)
    let modified_manifest_content = std::fs::read_to_string(&workspace.manifest_path)
        .expect("read manifest")
        .replace("ba6eafb7", "c1d2e3f4");
    std::fs::write(&workspace.manifest_path, modified_manifest_content).expect("write manifest");

    let new_golden_master = workspace
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-c1d2e3f4.qcow2");
    std::fs::write(&new_golden_master, b"NEW_GOLDEN_MASTER").expect("write new golden master");
    storage
        .state
        .lock()
        .unwrap()
        .images
        .insert(new_golden_master, MockImageRecord::default());

    let drifted_manifest =
        ManifestLoader::load_from_path(&workspace.manifest_path).expect("reload manifest");

    // 2. Plan emits Recreate
    let recreate_plan = planner.plan(&drifted_manifest).expect("plan recreate");
    assert!(matches!(
        &recreate_plan.actions[0],
        InstancePlanAction::Recreate { .. }
    ));

    // 3. Apply without allow_recreate aborts with LifecyclePolicyViolation
    let abort_result = applier.apply(&recreate_plan, &ApplyOptions::default());
    assert!(abort_result.is_err());
    match abort_result.unwrap_err() {
        LifecycleError::LifecyclePolicyViolation { instance_name, .. } => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other_error => panic!(
            "expected LifecyclePolicyViolation, but got: {:?}",
            other_error
        ),
    }

    // 4. Apply with allow_recreate succeeds
    let override_options = ApplyOptions {
        allow_recreate: true,
        ..Default::default()
    };
    let success_result = applier.apply(&recreate_plan, &override_options);
    assert!(success_result.is_ok());

    // 5. Verify subsequent plan is NoOp
    let final_plan = planner.plan(&drifted_manifest).expect("final plan");
    assert!(matches!(
        &final_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_destroyer_lifecycle_pipeline() {
    let workspace = TestWorkspace::new("protect");

    let hypervisor =
        MockHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = MockStorageManager::new()
        .with_image(&workspace.initial_golden_master_path, MockImageRecord::default());

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan, &ApplyOptions::default())
        .expect("initial apply");
    assert!(hypervisor.domain_info("win11-gollum").is_ok());

    // 1. Destroy instance with delete_disk
    let destroy_options = DestroyOptions {
        delete_disk: true,
        ..Default::default()
    };
    destroyer
        .destroy("win11-gollum", &manifest, &destroy_options)
        .expect("destroy instance must succeed");

    // 2. Verify domain undefined and overlay unlinked
    assert!(hypervisor.domain_info("win11-gollum").is_err());
    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&overlay_path).is_err());

    // 3. Plan now reflects Create action again
    let post_destroy_plan = planner.plan(&manifest).expect("post destroy plan");
    assert!(matches!(
        &post_destroy_plan.actions[0],
        InstancePlanAction::Create { .. }
    ));
}
