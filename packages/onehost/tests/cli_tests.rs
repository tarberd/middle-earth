use std::collections::HashMap;
use std::path::PathBuf;

use onehost::backup::model::{BackupConsistencyLevel, BackupManifest, DiskBackupEntry};
use onehost::cli::{
    self, resolve_manifest_path, ApplyArgs, BackupArgs, Cli, Commands, DestroyArgs,
    ImageBuildArgs, ImageCommand, ImageSubcommands, InstanceStatusReport, PlanArgs,
    RestoreArgs, StatusArgs,
};
use onehost::config::model::{
    FlavorConfiguration, ImageChangePolicy, InstanceConfiguration, InstanceLifecycleConfiguration,
    OnehostManifest, StorageDirectoriesConfiguration,
};
use onehost::hypervisor::mock::MockHypervisor;
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost::hypervisor::Hypervisor;
use onehost::lifecycle::planner::OnehostPlan;
use onehost::process::mock::MockProcessRunner;
use onehost::storage::mock::{MockImageRecord, MockStorageManager, RecordedStorageAction};

struct CliTestFixture {
    temporary_directory: tempfile::TempDir,
    manifest_path: PathBuf,
    template_xml_path: PathBuf,
    depot_store_directory: PathBuf,
    pool_directory: PathBuf,
}

impl CliTestFixture {
    fn new() -> Self {
        let temporary_directory = tempfile::tempdir().expect("temporary directory creation");
        let temporary_path = temporary_directory.path();

        let depot_store_directory = temporary_path.join("depot-store");
        let depot_iso_directory = temporary_path.join("depot-iso");
        let depot_backup_directory = temporary_path.join("depot-backups");
        let nvram_directory = temporary_path.join("nvram");
        let nvram_template = temporary_path.join("OVMF_VARS.fd");
        let ovmf_code = temporary_path.join("OVMF_CODE.fd");
        let pool_directory = temporary_path.join("pool-default");

        let template_xml_path = temporary_path.join("win-template.xml");
        let template_xml_content = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>win-workstation</name>
  <uuid>550e8400-e29b-41d4-a716-446655440000</uuid>
  <memory unit='KiB'>8388608</memory>
  <vcpu placement='static'>8</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-8.2'>hvm</type>
    <nvram>/var/lib/libvirt/qemu/nvram/win-workstation_VARS.fd</nvram>
  </os>
  <devices>
    <disk type='file' device='disk' onehost:role='os-disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/dummy-template-disk.qcow2'/>
      <target dev='sda' bus='sata'/>
    </disk>
  </devices>
</domain>"#;

        std::fs::write(&template_xml_path, template_xml_content).expect("write template xml");

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_directory.clone(),
            depot_iso_directory,
            depot_backup_directory,
            nvram_directory,
            nvram_template,
            ovmf_code,
            Some("default".to_string()),
        );

        let oemdrv_path = PathBuf::from("/nix/store/11111111111111111111111111111111-oemdrv-looking-glass");
        let mut flavors = HashMap::new();
        flavors.insert(
            "looking-glass".to_string(),
            FlavorConfiguration::new(oemdrv_path, "ba6eafb7"),
        );

        let mut instances = HashMap::new();
        instances.insert(
            "win-workstation".to_string(),
            InstanceConfiguration::new(
                "550e8400-e29b-41d4-a716-446655440000"
                    .parse()
                    .expect("valid uuid"),
                template_xml_path.clone(),
                "win11/26300.9457.pro.en-us/looking-glass"
                    .parse()
                    .expect("valid image tag"),
                Some("default".to_string()),
                false,
                InstanceLifecycleConfiguration::new(ImageChangePolicy::Protect, false),
            ),
        );

        let manifest = OnehostManifest::new("1.0", storage_config, flavors, instances)
            .expect("manifest validation succeeds");

        let manifest_path = temporary_path.join("onehost.json");
        let manifest_content = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        std::fs::write(&manifest_path, manifest_content).expect("write manifest");

        Self {
            temporary_directory,
            manifest_path,
            template_xml_path,
            depot_store_directory,
            pool_directory,
        }
    }

    fn golden_master_path(&self) -> PathBuf {
        self.depot_store_directory
            .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2")
    }

    fn default_pool_xml(&self) -> String {
        format!(
            "<pool type='dir'><name>default</name><target><path>{}</path></target></pool>",
            self.pool_directory.display()
        )
    }

    fn domain_info(&self) -> DomainInfo {
        DomainInfo {
            name: "win-workstation".to_string(),
            state: DomainState::Shutoff,
            vcpu_count: Some(8),
            memory_kib: Some(8_388_608),
            autostart: false,
        }
    }

    fn template_xml_content(&self) -> String {
        std::fs::read_to_string(&self.template_xml_path).expect("read template xml")
    }
}

#[test]
fn test_cli_plan_human_readable_output() {
    let fixture = CliTestFixture::new();
    let golden_master = fixture.golden_master_path();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, fixture.template_xml_content());

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Plan(PlanArgs { instance: None }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch plan succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("Onehost Execution Plan"));
    assert!(output_text.contains("+ create instance 'win-workstation' in pool 'default'"));
    assert!(output_text.contains("Plan: 1 to add, 0 to change, 0 to recreate, 0 to destroy."));
}

#[test]
fn test_cli_plan_json_output() {
    let fixture = CliTestFixture::new();
    let golden_master = fixture.golden_master_path();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, fixture.template_xml_content());

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: true,
        command: Commands::Plan(PlanArgs { instance: None }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch plan succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    let parsed_plan: OnehostPlan = serde_json::from_str(&output_text).expect("parse json plan");
    assert_eq!(parsed_plan.actions.len(), 1);
    assert_eq!(parsed_plan.actions[0].instance_name(), "win-workstation");
}

#[test]
fn test_cli_plan_filter_instance() {
    let fixture = CliTestFixture::new();
    let golden_master = fixture.golden_master_path();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, fixture.template_xml_content());

    let runner = MockProcessRunner::new();

    // Query for non-matching instance filter
    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: true,
        command: Commands::Plan(PlanArgs {
            instance: Some("non-existent-vm".to_string()),
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch plan succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    let parsed_plan: OnehostPlan = serde_json::from_str(&output_text).expect("parse json plan");
    assert!(parsed_plan.actions.is_empty());
}

#[test]
fn test_cli_apply_reconciles_infrastructure() {
    let fixture = CliTestFixture::new();
    let golden_master = fixture.golden_master_path();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, fixture.template_xml_content());

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Apply(ApplyArgs {
            instance: None,
            allow_recreate: false,
            auto_approve: true,
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch apply succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("Apply complete!"));

    // Verify domain was defined in hypervisor
    assert!(hypervisor.domain_info("win-workstation").is_ok());
}

#[test]
fn test_cli_destroy_deprovisions_instance() {
    let fixture = CliTestFixture::new();
    let domain_xml = fixture.template_xml_content();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let storage = MockStorageManager::new()
        .with_image(&overlay_path, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, &domain_xml);

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Destroy(DestroyArgs {
            instance: "win-workstation".to_string(),
            delete_disk: true,
            force: true,
            allow_destroy_protected: false,
            auto_approve: true,
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch destroy succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("Instance 'win-workstation' successfully deprovisioned"));

    // Verify domain is undefined
    assert!(hypervisor.domain_info("win-workstation").is_err());

    // Verify disk deletion was recorded
    let recorded_actions = storage.recorded_actions();
    assert!(recorded_actions
        .iter()
        .any(|action| matches!(action, RecordedStorageAction::DeleteImage { .. })));
}

#[test]
fn test_cli_backup_creates_staged_artifacts() {
    let fixture = CliTestFixture::new();
    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let domain_xml = fixture
        .template_xml_content()
        .replace("/dummy-template-disk.qcow2", &overlay_path.to_string_lossy());
    let target_backup_directory = fixture.temporary_directory.path().join("test-backup-output");

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let storage = MockStorageManager::new()
        .with_image(&overlay_path, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, &domain_xml);

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: true,
        command: Commands::Backup(BackupArgs {
            instance: "win-workstation".to_string(),
            target_dir: Some(target_backup_directory),
            crash_consistent: true,
            no_compress: false,
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch backup succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("\"instance_name\": \"win-workstation\""));
}

#[test]
fn test_cli_restore_restores_from_staged_directory() {
    let fixture = CliTestFixture::new();
    let temporary_path = fixture.temporary_directory.path();
    let backup_stage_directory = temporary_path.join("staged-backup-2026-09-27");

    let golden_master = fixture.golden_master_path();
    let backup_manifest = BackupManifest {
        instance_name: "win-workstation".to_string(),
        instance_uuid: "550e8400-e29b-41d4-a716-446655440000"
            .parse()
            .expect("valid uuid"),
        timestamp: "20260927T000000Z".to_string(),
        consistency_level: BackupConsistencyLevel::CrashConsistent,
        storage_pool: Some("default".to_string()),
        base_image_tag: Some("win11/26300.9457.pro.en-us/looking-glass".to_string()),
        base_image_hash: Some("ba6eafb7".to_string()),
        golden_master_filename: Some(
            "win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2".to_string(),
        ),
        disks: vec![DiskBackupEntry {
            target_device: "sda".to_string(),
            archive_filename: "sda.qcow2.thin".to_string(),
            virtual_size_bytes: 68_719_476_736,
            archive_size_bytes: 1_048_576,
            backing_file: Some(golden_master.clone()),
        }],
        domain_xml_filename: "domain.xml".to_string(),
        nvram_filename: "nvram.fd".to_string(),
    };
    let manifest_json =
        serde_json::to_string_pretty(&backup_manifest).expect("serialize backup manifest");

    let backup_manifest_path = backup_stage_directory.join("manifest.json");
    let backup_xml_path = backup_stage_directory.join("domain.xml");
    let backup_nvram_path = backup_stage_directory.join("nvram.fd");
    let backup_disk_path = backup_stage_directory.join("sda.qcow2.thin");

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml());

    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default())
        .with_image(&backup_disk_path, MockImageRecord::default())
        .with_file(&backup_manifest_path, manifest_json)
        .with_file(&backup_xml_path, fixture.template_xml_content())
        .with_file(&backup_nvram_path, "NVRAM_BINARY_DATA")
        .with_file(&fixture.template_xml_path, fixture.template_xml_content());

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Restore(RestoreArgs {
            backup_dir: backup_stage_directory.clone(),
            pool: Some("default".to_string()),
            allow_overwrite: false,
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch restore succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("Restore complete!"));

    // Verify domain was restored and defined in hypervisor
    assert!(hypervisor.domain_info("win-workstation").is_ok());
}

#[test]
fn test_cli_status_human_and_json() {
    let fixture = CliTestFixture::new();
    let domain_xml = fixture.template_xml_content();

    let hypervisor = MockHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let storage = MockStorageManager::new()
        .with_image(&overlay_path, MockImageRecord::default())
        .with_file(&fixture.template_xml_path, &domain_xml);

    let runner = MockProcessRunner::new();

    // Human table test
    let cli_human = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Status(StatusArgs { instance: None }),
    };

    let mut human_buffer = Vec::new();
    let exit_human = cli::dispatch(cli_human, &hypervisor, &storage, &runner, &mut human_buffer)
        .expect("dispatch status human");

    assert_eq!(exit_human, 0);
    let human_text = String::from_utf8(human_buffer).expect("utf8 string");
    assert!(human_text.contains("INSTANCE"));
    assert!(human_text.contains("win-workstation"));
    assert!(human_text.contains("shutoff"));

    // JSON report test
    let cli_json = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: true,
        command: Commands::Status(StatusArgs { instance: None }),
    };

    let mut json_buffer = Vec::new();
    let exit_json = cli::dispatch(cli_json, &hypervisor, &storage, &runner, &mut json_buffer)
        .expect("dispatch status json");

    assert_eq!(exit_json, 0);
    let json_text = String::from_utf8(json_buffer).expect("utf8 string");
    let reports: Vec<InstanceStatusReport> = serde_json::from_str(&json_text).expect("parse json reports");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].instance_name, "win-workstation");
    assert_eq!(reports[0].domain_state.as_deref(), Some("shutoff"));
}

#[test]
fn test_cli_image_build_when_master_exists() {
    let fixture = CliTestFixture::new();
    let golden_master = fixture.golden_master_path();

    let hypervisor = MockHypervisor::new();
    let storage = MockStorageManager::new()
        .with_image(&golden_master, MockImageRecord::default());
    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Image(ImageCommand {
            action: ImageSubcommands::Build(ImageBuildArgs {
                tag: "win11/26300.9457.pro.en-us/looking-glass".to_string(),
                force: false,
                scratch_dir: None,
                iso: None,
                memory: 8192,
                cpus: 8,
            }),
        }),
    };

    let mut output_buffer = Vec::new();
    let exit_code = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer)
        .expect("dispatch image build succeeds");

    assert_eq!(exit_code, 0);

    let output_text = String::from_utf8(output_buffer).expect("valid utf8 output");
    assert!(output_text.contains("Golden master already exists at:"));
}

#[test]
fn test_cli_manifest_not_found_returns_typed_error() {
    let non_existent_manifest = PathBuf::from("/non/existent/path/onehost.json");
    let result = resolve_manifest_path(Some(&non_existent_manifest));

    assert!(result.is_err());
    let error = result.expect_err("is error");
    assert!(matches!(error, cli::CliError::ManifestNotFound { .. }));
}
