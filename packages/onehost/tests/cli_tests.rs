use std::collections::HashMap;
use std::path::PathBuf;

use onehost::backup::model::{BackupConsistencyLevel, BackupManifest, DiskBackupEntry};
use onehost::cli::{
    self, ApplyArgs, BackupArgs, Cli, Commands, DestroyArgs, ImageBuildArgs, ImageCommand,
    ImageSubcommands, InstanceStatusReport, PlanArgs, RestoreArgs, StatusArgs,
    resolve_manifest_path,
};
use onehost::config::model::{
    FlavorConfiguration, InstanceConfiguration, OnehostManifest, StorageDirectoriesConfiguration,
};
use onehost::hypervisor::Hypervisor;
use onehost::hypervisor::traits::{DomainInfo, DomainState};
use onehost_test_support::FakeHypervisor;
use onehost::lifecycle::planner::OnehostPlan;
use onehost::process::mock::MockProcessRunner;
use onehost::storage::QemuImgStorage;

pub struct CliTestFixture {
    pub temporary_directory: tempfile::TempDir,
    pub manifest_path: PathBuf,
    pub template_xml_path: PathBuf,
    pub depot_store_directory: PathBuf,
    pub pool_directory: PathBuf,
    pub nvram_directory: PathBuf,
    pub content_hash: String,
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

        std::fs::create_dir_all(&depot_store_directory).expect("create depot store dir");
        std::fs::create_dir_all(&depot_iso_directory).expect("create depot iso dir");
        std::fs::create_dir_all(&depot_backup_directory).expect("create depot backup dir");
        std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");
        std::fs::create_dir_all(&pool_directory).expect("create pool dir");
        std::fs::write(&nvram_template, b"NVRAM_TEMPLATE_VARS").expect("write nvram template");
        std::fs::write(&ovmf_code, b"OVMF_CODE_DATA").expect("write ovmf code");

        let template_nvram_path = nvram_directory.join("win-workstation_VARS.fd");
        let template_xml_path = temporary_path.join("win-template.xml");
        let template_xml_content = format!(
            r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>win-workstation</name>
  <uuid>550e8400-e29b-41d4-a716-446655440000</uuid>
  <memory unit='KiB'>8388608</memory>
  <vcpu placement='static'>8</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-8.2'>hvm</type>
    <nvram>{}</nvram>
  </os>
  <devices>
    <disk type='file' device='disk' onehost:role='os-disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/dummy-template-disk.qcow2'/>
      <target dev='sda' bus='sata'/>
    </disk>
  </devices>
</domain>"#,
            template_nvram_path.display()
        );

        std::fs::write(&template_xml_path, template_xml_content).expect("write template xml");

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_directory.clone(),
            depot_iso_directory,
            depot_backup_directory,
            nvram_directory.clone(),
            nvram_template,
            ovmf_code,
        );

        let oemdrv_path = temporary_path.join("oemdrv");
        std::fs::write(&oemdrv_path, b"test-oemdrv-content").expect("write oemdrv");
        let extra_isos = HashMap::new();
        let content_hash =
            onehost::storage::execute_compute_content_hash(&oemdrv_path, &extra_isos)
                .expect("compute content hash");

        let mut flavors = HashMap::new();
        flavors.insert(
            "looking-glass".to_string(),
            FlavorConfiguration::new(oemdrv_path.clone(), extra_isos),
        );

        let mut instances = HashMap::new();
        instances.insert(
            "win-workstation".to_string(),
            InstanceConfiguration::new(
                "550e8400-e29b-41d4-a716-446655440000"
                    .parse()
                    .expect("valid uuid"),
                template_xml_path.clone(),
                "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass"
                    .parse()
                    .expect("valid image tag"),
                "default",
            ),
        );

        let manifest = OnehostManifest::new("1.0", storage_config, flavors, instances)
            .expect("manifest validation succeeds");

        let manifest_path = temporary_path.join("onehost.json");
        let manifest_content = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        std::fs::write(&manifest_path, manifest_content).expect("write manifest");

        let golden_master_path = depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{content_hash}.qcow2"
        ));
        let storage = QemuImgStorage::new();
        storage
            .create_empty_disk(&golden_master_path, 20 * 1024 * 1024)
            .expect("create golden master");

        Self {
            temporary_directory,
            manifest_path,
            template_xml_path,
            depot_store_directory,
            pool_directory,
            nvram_directory,
            content_hash,
        }
    }

    fn golden_master_path(&self) -> PathBuf {
        self.depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
            self.content_hash
        ))
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

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());
    let storage = QemuImgStorage::new();
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

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());
    let storage = QemuImgStorage::new();
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

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());
    let storage = QemuImgStorage::new();
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

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());
    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Apply(ApplyArgs {
            instance: None,
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

    let hypervisor = FakeHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&overlay_path, 20 * 1024 * 1024)
        .expect("create overlay");

    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Destroy(DestroyArgs {
            instance: "win-workstation".to_string(),
            delete_disk: true,
            force: true,
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

    // Verify disk is deprovisioned from storage
    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    assert!(storage.inspect_image(&overlay_path).is_err());
}

#[test]
fn test_cli_backup_creates_staged_artifacts() {
    let fixture = CliTestFixture::new();
    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let nvram_path = fixture.nvram_directory.join("win-workstation_VARS.fd");
    std::fs::write(&nvram_path, b"NVRAM_DATA").expect("write nvram");

    let domain_xml = fixture
        .template_xml_content()
        .replace(
            "/dummy-template-disk.qcow2",
            &overlay_path.to_string_lossy(),
        );
    let target_backup_directory = fixture
        .temporary_directory
        .path()
        .join("test-backup-output");

    let hypervisor = FakeHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&overlay_path, 20 * 1024 * 1024)
        .expect("create overlay");

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
        base_image_tag: Some(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass".to_string(),
        ),
        base_image_hash: Some(fixture.content_hash.clone()),
        golden_master_filename: Some(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
            fixture.content_hash
        )),
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

    let hypervisor = FakeHypervisor::new().with_pool_xml("default", fixture.default_pool_xml());

    std::fs::create_dir_all(&backup_stage_directory).expect("create backup stage dir");
    std::fs::write(&backup_manifest_path, manifest_json).expect("write backup manifest");
    std::fs::write(&backup_xml_path, fixture.template_xml_content()).expect("write backup xml");
    std::fs::write(&backup_nvram_path, b"NVRAM_BINARY_DATA").expect("write backup nvram");

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&backup_disk_path, 20 * 1024 * 1024)
        .expect("create thin backup disk");

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

    // Physical disk and nvram state verification
    let restored_disk_path = fixture.pool_directory.join("win-workstation.qcow2");
    assert!(restored_disk_path.exists());
    let restored_nvram_path = fixture.nvram_directory.join("win-workstation_VARS.fd");
    assert!(restored_nvram_path.exists());
    assert_eq!(
        std::fs::read(&restored_nvram_path).expect("read restored nvram"),
        b"NVRAM_BINARY_DATA"
    );
}

#[test]
fn test_cli_status_human_and_json() {
    let fixture = CliTestFixture::new();
    let domain_xml = fixture.template_xml_content();

    let hypervisor = FakeHypervisor::new()
        .with_pool_xml("default", fixture.default_pool_xml())
        .with_domain(fixture.domain_info(), &domain_xml);

    let overlay_path = fixture.pool_directory.join("win-workstation.qcow2");
    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&overlay_path, 20 * 1024 * 1024)
        .expect("create overlay");

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
    let reports: Vec<InstanceStatusReport> =
        serde_json::from_str(&json_text).expect("parse json reports");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].instance_name, "win-workstation");
    assert_eq!(reports[0].domain_state.as_deref(), Some("shutoff"));
}

#[test]
fn test_cli_image_build_when_master_exists() {
    let fixture = CliTestFixture::new();

    let hypervisor = FakeHypervisor::new();
    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Image(ImageCommand {
            action: ImageSubcommands::Build(ImageBuildArgs {
                tag: "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass".to_string(),
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

#[test]
fn test_cli_rejects_removed_allow_recreate_flag() {
    use clap::Parser;

    let apply_with_legacy_flag = Cli::try_parse_from(["onehost", "apply", "--allow-recreate"]);
    assert!(
        apply_with_legacy_flag.is_err(),
        "Expected --allow-recreate to be rejected on apply"
    );

    let plan_with_legacy_flag = Cli::try_parse_from(["onehost", "plan", "--allow-recreate"]);
    assert!(
        plan_with_legacy_flag.is_err(),
        "Expected --allow-recreate to be rejected on plan"
    );
}

#[test]
fn test_cli_rejects_removed_allow_destroy_protected_flag() {
    use clap::Parser;

    let destroy_with_legacy_flag =
        Cli::try_parse_from(["onehost", "destroy", "win11", "--allow-destroy-protected"]);
    assert!(
        destroy_with_legacy_flag.is_err(),
        "Expected --allow-destroy-protected to be rejected on destroy"
    );
}

#[test]
fn test_cli_image_build_rejects_informal_build_tags() {
    let fixture = CliTestFixture::new();
    let hypervisor = FakeHypervisor::new();
    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let cli_args = Cli {
        manifest: Some(fixture.manifest_path.clone()),
        json: false,
        command: Commands::Image(ImageCommand {
            action: ImageSubcommands::Build(ImageBuildArgs {
                tag: "win11/pro/en-us/looking-glass".to_string(),
                force: false,
                scratch_dir: None,
                iso: None,
                memory: 8192,
                cpus: 8,
            }),
        }),
    };

    let mut output_buffer = Vec::new();
    let result = cli::dispatch(cli_args, &hypervisor, &storage, &runner, &mut output_buffer);
    let err = result.expect_err("Expected image build with informal tag to be rejected");
    assert!(matches!(
        err,
        cli::CliError::TagError(onehost::image::tag::ImageTagParseError::InvalidUupId {
            ref raw_tag,
            ref uup_id,
        }) if raw_tag == "win11/pro/en-us/looking-glass" && uup_id == "win11"
    ));
}
