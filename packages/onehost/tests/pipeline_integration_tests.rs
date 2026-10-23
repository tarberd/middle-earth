use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

use onehost::config::loader::ManifestLoader;
use onehost::hypervisor::traits::{DomainState, Hypervisor};
use onehost::lifecycle::LifecycleError;
use onehost::lifecycle::applier::DomainLifecycleApplier;
use onehost::lifecycle::destroyer::{DestroyOptions, DomainLifecycleDestroyer};
use onehost::lifecycle::planner::{DomainLifecyclePlanner, InstancePlanAction};
use onehost::storage::qemu_img::QemuImgStorage;
use onehost_test_support::FakeHypervisor;

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

pub struct TestWorkspace {
    pub workspace_directory: tempfile::TempDir,
    pub manifest_path: std::path::PathBuf,
    pub template_xml_path: std::path::PathBuf,
    pub depot_store_directory: std::path::PathBuf,
    pub nvram_directory: std::path::PathBuf,
    pub default_pool_directory: std::path::PathBuf,
    pub nvme_pool_directory: std::path::PathBuf,
    pub oemdrv_path: std::path::PathBuf,
    pub content_hash: String,
}

impl TestWorkspace {
    fn new() -> Self {
        let temporary_workspace_directory = tempdir().expect("create workspace tempdir");
        let workspace_root_path = temporary_workspace_directory.path();

        let depot_directory = workspace_root_path.join("depot");
        let depot_store_directory = depot_directory.join("store");
        let depot_iso_directory = depot_directory.join("iso");
        let depot_backup_directory = depot_directory.join("backup");
        let nvram_directory = workspace_root_path.join("nvram");
        let default_pool_directory = workspace_root_path.join("default_pool");
        let nvme_pool_directory = workspace_root_path.join("nvme_pool");

        std::fs::create_dir_all(&depot_store_directory).expect("create depot_store");
        std::fs::create_dir_all(&depot_iso_directory).expect("create depot_iso");
        std::fs::create_dir_all(&depot_backup_directory).expect("create depot_backup");
        std::fs::create_dir_all(&nvram_directory).expect("create nvram_dir");
        std::fs::create_dir_all(&default_pool_directory).expect("create default_pool_dir");
        std::fs::create_dir_all(&nvme_pool_directory).expect("create nvme_pool_dir");

        let nvram_template_path = workspace_root_path.join("edk2-vars.fd");
        let mut nvram_file = File::create(&nvram_template_path).expect("create nvram template");
        nvram_file
            .write_all(b"NVRAM_TEMPLATE_VARS")
            .expect("write nvram template");

        let template_xml_path = workspace_root_path.join("win11-template.xml");
        std::fs::write(&template_xml_path, SAMPLE_TEMPLATE_XML).expect("write template xml");

        let oemdrv_path = workspace_root_path.join("oemdrv");
        std::fs::write(&oemdrv_path, b"OEMDRV_PAYLOAD").expect("write oemdrv");
        let extra_isos = std::collections::HashMap::new();
        let content_hash =
            onehost::storage::execute_compute_content_hash(&oemdrv_path, &extra_isos)
                .expect("compute content hash");

        let initial_master_path = depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
            content_hash
        ));
        let storage = QemuImgStorage::new();
        storage
            .create_empty_disk(&initial_master_path, 20 * 1024 * 1024)
            .expect("create golden master");

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
    "ovmf_code": "/run/libvirt/nix-ovmf/code.fd"
  }},
  "flavors": {{
    "looking-glass": {{
      "oemdrv_path": "{}",
      "extra_isos": {{}}
    }}
  }},
  "instances": {{
    "win11-gollum": {{
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "{}",
      "image": "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
      "pool": "default"
    }}
  }}
}}"#,
            depot_store_directory.display(),
            depot_iso_directory.display(),
            depot_backup_directory.display(),
            nvram_directory.display(),
            nvram_template_path.display(),
            oemdrv_path.display(),
            template_xml_path.display(),
        );

        let manifest_file_path = workspace_root_path.join("onehost.json");
        std::fs::write(&manifest_file_path, manifest_content).expect("write manifest");

        Self {
            workspace_directory: temporary_workspace_directory,
            manifest_path: manifest_file_path,
            template_xml_path,
            depot_store_directory,
            nvram_directory,
            default_pool_directory,
            nvme_pool_directory,
            oemdrv_path,
            content_hash,
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

#[test]
fn test_end_to_end_fresh_provisioning_and_idempotent_noop() {
    let workspace = TestWorkspace::new();

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = QemuImgStorage::new();

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

    assert!(matches!(
        &initial_plan.actions[0],
        InstancePlanAction::Create { instance_name, .. } if instance_name == "win11-gollum"
    ));

    // 3. Execute apply
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    applier
        .apply(&initial_plan)
        .expect("initial apply must succeed");

    // 4. Verify hypervisor and storage state
    assert!(hypervisor.snapshot().domains.contains_key("win11-gollum"));
    let domain_info = hypervisor
        .domain_info("win11-gollum")
        .expect("domain must be registered in hypervisor");
    assert_eq!(domain_info.name, "win11-gollum");
    assert_eq!(domain_info.state, DomainState::Shutoff);

    let registered_xml = hypervisor
        .dump_xml("win11-gollum", false)
        .expect("dump xml");
    assert!(registered_xml.contains("<name>win11-gollum</name>"));

    let expected_overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    let expected_overlay_fragment =
        format!(r#"<source file="{}"/>"#, expected_overlay_path.display());
    assert!(registered_xml.contains(&expected_overlay_fragment));
    assert!(!registered_xml.contains("onehost:role"));
    assert!(!registered_xml.contains("xmlns:onehost"));

    assert!(expected_overlay_path.exists());
    let overlay_inspection = storage
        .inspect_image(&expected_overlay_path)
        .expect("overlay image must exist and be valid qcow2");
    assert_eq!(overlay_inspection.format, "qcow2");
    let expected_base_cache_path = workspace.default_pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        workspace.content_hash
    ));
    assert_eq!(
        overlay_inspection.backing_file,
        Some(expected_base_cache_path)
    );
    assert!(storage.check_image(&expected_overlay_path).is_ok());

    let nvram_target = workspace.nvram_directory.join("win11-gollum_VARS.fd");
    assert!(
        nvram_target.exists(),
        "NVRAM target file must exist on disk"
    );
    let nvram_content = std::fs::read(&nvram_target).expect("read NVRAM target");
    assert_eq!(nvram_content, b"NVRAM_TEMPLATE_VARS");

    // 5. Assert IDEMPOTENCY: second plan emits NoOp
    let idempotent_plan = planner
        .plan(&manifest)
        .expect("idempotent planning must succeed");
    assert_eq!(idempotent_plan.actions.len(), 1);

    assert!(matches!(
        &idempotent_plan.actions[0],
        InstancePlanAction::NoOp { instance_name } if instance_name == "win11-gollum"
    ));
}

#[test]
fn test_end_to_end_template_drift_reconciliation_pipeline() {
    let workspace = TestWorkspace::new();

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = QemuImgStorage::new();

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan)
        .expect("initial apply");

    // 1. Modify the template XML file on disk (increase vcpu to 8)
    std::fs::write(&workspace.template_xml_path, MODIFIED_TEMPLATE_XML)
        .expect("write modified template");

    // 2. Plan detects template drift
    let drift_plan = planner
        .plan(&manifest)
        .expect("planning after template modification");
    assert_eq!(drift_plan.actions.len(), 1);

    assert!(matches!(
        &drift_plan.actions[0],
        InstancePlanAction::UpdateDomainXml {
            instance_name,
            diff_summary,
            ..
        } if instance_name == "win11-gollum" && diff_summary.contains("vcpu")
    ));

    // 3. Apply reconciliation without recreating overlay
    applier
        .apply(&drift_plan)
        .expect("apply update");

    assert!(hypervisor.snapshot().domains.contains_key("win11-gollum"));
    let updated_xml = hypervisor
        .dump_xml("win11-gollum", false)
        .expect("dump xml");
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
    let workspace = TestWorkspace::new();

    let hypervisor = FakeHypervisor::new()
        .with_pool_xml("default", workspace.default_pool_xml())
        .with_pool_xml("fast-nvme", workspace.nvme_pool_xml());

    let storage = QemuImgStorage::new();

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision in 'default' pool
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan)
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
    let relocation_plan = planner.plan(&relocated_manifest).expect("plan relocation");
    assert_eq!(relocation_plan.actions.len(), 1);

    assert!(matches!(
        &relocation_plan.actions[0],
        InstancePlanAction::RelocateStoragePool {
            instance_name,
            source_pool,
            target_pool,
            ..
        } if instance_name == "win11-gollum" && source_pool == "default" && target_pool == "fast-nvme"
    ));

    // 4. Apply relocation
    applier
        .apply(&relocation_plan)
        .expect("apply relocation");

    // 5. Verify new overlay location and backing file rebase
    let expected_new_overlay = workspace.nvme_pool_directory.join("win11-gollum.qcow2");
    assert!(storage.inspect_image(&expected_new_overlay).is_ok());

    let rebased_inspection = storage
        .inspect_image(&expected_new_overlay)
        .expect("inspect rebased image");
    let expected_new_base = workspace.nvme_pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        workspace.content_hash
    ));
    assert_eq!(rebased_inspection.backing_file, Some(expected_new_base));
    assert!(storage.check_image(&expected_new_overlay).is_ok());

    // 6. Verify domain XML points to new disk
    let updated_domain_xml = hypervisor
        .dump_xml("win11-gollum", false)
        .expect("dump xml");
    assert!(updated_domain_xml.contains(&expected_new_overlay.display().to_string()));

    // 7. Verify observable hypervisor and pool state
    let snapshot = hypervisor.snapshot();
    assert!(snapshot.storage_pools.contains_key("default"));
    assert!(snapshot.storage_pools.contains_key("fast-nvme"));
    assert!(snapshot.domains.contains_key("win11-gollum"));
    assert_eq!(
        snapshot
            .domains
            .get("win11-gollum")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );

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
fn test_end_to_end_recreate_pipeline() {
    let workspace = TestWorkspace::new();

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = QemuImgStorage::new();

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan)
        .expect("initial apply");

    // 1. Simulate base image hash change by modifying oemdrv content
    std::fs::write(&workspace.oemdrv_path, b"NEW_OEMDRV_PAYLOAD_CHANGED")
        .expect("write new oemdrv");
    let new_hash = onehost::storage::execute_compute_content_hash(
        &workspace.oemdrv_path,
        &std::collections::HashMap::new(),
    )
    .expect("compute new hash");

    let new_golden_master = workspace.depot_store_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        new_hash
    ));
    storage
        .create_empty_disk(&new_golden_master, 20 * 1024 * 1024)
        .expect("create new golden master");

    let drifted_manifest =
        ManifestLoader::load_from_path(&workspace.manifest_path).expect("reload manifest");

    // 2. Plan emits Recreate
    let recreate_plan = planner.plan(&drifted_manifest).expect("plan recreate");
    assert!(matches!(
        &recreate_plan.actions[0],
        InstancePlanAction::Recreate { .. }
    ));

    // 3. Apply recreate succeeds
    let success_result = applier.apply(&recreate_plan);
    assert!(success_result.is_ok());

    let snapshot = hypervisor.snapshot();
    assert!(snapshot.domains.contains_key("win11-gollum"));
    assert_eq!(
        snapshot
            .domains
            .get("win11-gollum")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );

    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(overlay_path.exists());
    let overlay_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect recreated overlay");
    let expected_new_base = workspace.default_pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        new_hash
    ));
    assert_eq!(overlay_inspection.backing_file, Some(expected_new_base));
    assert!(storage.check_image(&overlay_path).is_ok());

    // 4. Verify subsequent plan is NoOp
    let final_plan = planner.plan(&drifted_manifest).expect("final plan");
    assert!(matches!(
        &final_plan.actions[0],
        InstancePlanAction::NoOp { .. }
    ));
}

#[test]
fn test_end_to_end_destroyer_lifecycle_pipeline() {
    let workspace = TestWorkspace::new();

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", workspace.default_pool_xml());
    let storage = QemuImgStorage::new();

    let manifest = ManifestLoader::load_from_path(&workspace.manifest_path).expect("load manifest");

    let planner = DomainLifecyclePlanner::new(&hypervisor, &storage)
        .with_template_resolver(file_template_resolver);
    let applier = DomainLifecycleApplier::new(&hypervisor, &storage);
    let destroyer = DomainLifecycleDestroyer::new(&hypervisor, &storage);

    // Initial provision
    let initial_plan = planner.plan(&manifest).expect("initial plan");
    applier
        .apply(&initial_plan)
        .expect("initial apply");
    assert!(hypervisor.domain_info("win11-gollum").is_ok());

    let overlay_path = workspace.default_pool_directory.join("win11-gollum.qcow2");
    assert!(overlay_path.exists(), "overlay must exist after initial apply");
    let pre_destroy_inspection = storage
        .inspect_image(&overlay_path)
        .expect("inspect overlay before destroy");
    assert_eq!(pre_destroy_inspection.format, "qcow2");
    let expected_base_cache_path = workspace.default_pool_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
        workspace.content_hash
    ));
    assert_eq!(
        pre_destroy_inspection.backing_file,
        Some(expected_base_cache_path.clone())
    );
    assert!(storage.check_image(&overlay_path).is_ok());

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
    assert!(!hypervisor.snapshot().domains.contains_key("win11-gollum"));
    assert!(!overlay_path.exists(), "overlay disk must be unlinked after destroy");
    assert!(storage.inspect_image(&overlay_path).is_err());
    assert!(
        expected_base_cache_path.exists(),
        "base cache image must be preserved after overlay unlinking"
    );
    assert!(storage.check_image(&expected_base_cache_path).is_ok());

    // 3. Plan now reflects Create action again
    let post_destroy_plan = planner.plan(&manifest).expect("post destroy plan");
    assert!(matches!(
        &post_destroy_plan.actions[0],
        InstancePlanAction::Create { .. }
    ));
}
