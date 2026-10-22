use std::collections::HashMap;
use std::path::PathBuf;

use onehost::config::model::{
    FlavorConfiguration, OnehostManifest, StorageDirectoriesConfiguration,
};
use onehost::image::builder::{
    ImageBuildOptions, ImageBuildOutcome, ImageBuilder, ImageBuilderError,
};
use onehost::image::tag::{FlavorResolutionError, ImageTagSpecification};
use onehost::process::mock::{MockProcessRunner, RecordedProcessAction};
use onehost::storage::{QemuImgStorage, execute_compute_content_hash};

pub struct BuilderTestFixture {
    pub temporary_directory: tempfile::TempDir,
    pub manifest: OnehostManifest,
    pub image_tag: ImageTagSpecification,
    pub iso_path: PathBuf,
    pub oemdrv_path: PathBuf,
    pub depot_store_directory: PathBuf,
    pub depot_iso_directory: PathBuf,
    pub scratch_directory: PathBuf,
    pub content_hash: String,
}

impl BuilderTestFixture {
    fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("temp directory creation");
        let base_path = temp_dir.path();

        let depot_store_directory = base_path.join("depot-store");
        let depot_iso_directory = base_path.join("depot-iso");
        let depot_backup_directory = base_path.join("depot-backups");
        let nvram_directory = base_path.join("nvram");
        let nvram_template = base_path.join("OVMF_VARS.fd");
        let ovmf_code = base_path.join("OVMF_CODE.fd");
        let scratch_directory = base_path.join("scratch");

        std::fs::create_dir_all(&depot_store_directory).expect("create depot store dir");
        std::fs::create_dir_all(&depot_iso_directory).expect("create depot iso dir");
        std::fs::create_dir_all(&depot_backup_directory).expect("create depot backup dir");
        std::fs::create_dir_all(&nvram_directory).expect("create nvram dir");
        std::fs::create_dir_all(&scratch_directory).expect("create scratch dir");

        std::fs::write(&nvram_template, "dummy nvram template").expect("write nvram template");
        std::fs::write(&ovmf_code, "dummy ovmf code").expect("write ovmf code");

        let oemdrv_path = base_path.join("oemdrv-looking-glass.iso");
        std::fs::write(&oemdrv_path, "dummy oemdrv payload").expect("write oemdrv payload");

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_directory.clone(),
            depot_iso_directory.clone(),
            depot_backup_directory,
            nvram_directory,
            nvram_template,
            ovmf_code,
        );

        let extra_isos = HashMap::new();
        let content_hash = execute_compute_content_hash(&oemdrv_path, &extra_isos)
            .expect("compute content hash succeeds");

        let mut flavors = HashMap::new();
        flavors.insert(
            "looking-glass".to_string(),
            FlavorConfiguration::new(oemdrv_path.clone(), extra_isos),
        );

        let manifest = OnehostManifest::new("1.0", storage_config, flavors, HashMap::new())
            .expect("manifest creation succeeds");

        let image_tag: ImageTagSpecification =
            "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass"
                .parse()
                .expect("image tag parsing");

        let iso_path =
            depot_iso_directory.join("ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us.iso");
        std::fs::write(&iso_path, "dummy iso payload").expect("write iso payload");

        Self {
            temporary_directory: temp_dir,
            manifest,
            image_tag,
            iso_path,
            oemdrv_path,
            depot_store_directory,
            depot_iso_directory,
            scratch_directory,
            content_hash,
        }
    }

    fn golden_master_path(&self) -> PathBuf {
        self.depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
            self.content_hash
        ))
    }
}

#[test]
fn test_builder_already_existing_master_returns_early_without_process_spawning() {
    let fixture = BuilderTestFixture::new();
    let golden_master_path = fixture.golden_master_path();

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&golden_master_path, 1024 * 1024)
        .expect("create golden master disk");

    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let outcome = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect("build outcome");

    assert_eq!(
        outcome,
        ImageBuildOutcome::AlreadyExists {
            golden_master_path: golden_master_path.clone(),
        }
    );

    // Assert zero external processes were spawned
    assert!(runner.recorded_actions().is_empty());
}

#[test]
fn test_builder_missing_iso_fails_fast_with_typed_error() {
    let fixture = BuilderTestFixture::new();

    // Remove the ISO file from depot
    if fixture.iso_path.exists() {
        std::fs::remove_file(&fixture.iso_path).expect("remove iso file");
    }

    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must fail when ISO is missing");

    match build_error {
        ImageBuilderError::IsoNotFound {
            iso_directory,
            release_tuple,
        } => {
            assert_eq!(iso_directory, fixture.depot_iso_directory);
            assert_eq!(
                release_tuple,
                "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us"
            );
        }
        other_error => unreachable!("expected IsoNotFound, got: {other_error:?}"),
    }

    assert!(runner.recorded_actions().is_empty());
}

#[test]
fn test_builder_unregistered_flavor_fails_fast() {
    let fixture = BuilderTestFixture::new();

    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let unknown_tag: ImageTagSpecification =
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/nonexistent-flavor"
            .parse()
            .expect("image tag parsing");

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &unknown_tag, &options)
        .expect_err("must fail on unregistered flavor");

    match build_error {
        ImageBuilderError::FlavorError(FlavorResolutionError::UnknownFlavor {
            requested_flavor,
            ..
        }) => {
            assert_eq!(requested_flavor, "nonexistent-flavor");
        }
        other_error => unreachable!("expected UnknownFlavor, got: {other_error:?}"),
    }
}

#[test]
fn test_builder_missing_oemdrv_fails_fast() {
    let fixture = BuilderTestFixture::new();

    // Remove OEMDRV file
    if fixture.oemdrv_path.exists() {
        std::fs::remove_file(&fixture.oemdrv_path).expect("remove oemdrv file");
    }

    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must fail when OEMDRV is missing");

    match build_error {
        ImageBuilderError::OemdrvNotFound { path } => {
            assert_eq!(path, fixture.oemdrv_path);
        }
        other_error => unreachable!("expected OemdrvNotFound, got: {other_error:?}"),
    }
}

#[test]
fn test_builder_atomic_promotion_sequence_and_scratch_cleanup() {
    let fixture = BuilderTestFixture::new();

    let storage = QemuImgStorage::new();
    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions {
        force: false,
        scratch_dir: Some(fixture.scratch_directory.clone()),
        iso_path: None,
        memory_mb: 8192,
        cpu_count: 8,
    };

    let outcome = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect("build succeeds");

    let expected_master_path = fixture.golden_master_path();

    match outcome {
        ImageBuildOutcome::Built {
            golden_master_path,
            virtual_size_bytes,
            archive_size_bytes,
        } => {
            assert_eq!(golden_master_path, expected_master_path);
            assert_eq!(virtual_size_bytes, 64 * 1024 * 1024 * 1024);
            assert!(archive_size_bytes > 0);
        }
        ImageBuildOutcome::AlreadyExists { .. } => {
            unreachable!("expected Built outcome");
        }
    }

    // Verify filesystem state:
    // 1. Golden master promoted and exists
    assert!(expected_master_path.exists());

    // 2. Scratch disk cleaned up
    let scratch_disk_path = fixture.scratch_directory.join("build-disk.qcow2");
    assert!(!scratch_disk_path.exists());

    // 3. Temporary master cleaned up / moved
    let temporary_master_path = fixture.depot_store_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2.tmp",
        fixture.content_hash
    ));
    assert!(!temporary_master_path.exists());

    // 4. Strict read-only permissions (0444) on promoted golden master
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(&expected_master_path)
            .expect("metadata")
            .permissions();
        assert_eq!(permissions.mode() & 0o777, 0o444);
    }

    // Verify process actions:
    // 1. spawn_daemon("swtpm")
    // 2. execute("qemu-system-x86_64")
    // 3. kill_daemon
    let process_actions = runner.recorded_actions();

    let spawned_swtpm = process_actions.iter().any(|action| {
        matches!(action, RecordedProcessAction::SpawnDaemon { program, .. } if program == "swtpm")
    });
    assert!(spawned_swtpm);

    let executed_qemu = process_actions.iter().any(|action| {
        matches!(action, RecordedProcessAction::Execute { program, .. } if program == "qemu-system-x86_64")
    });
    assert!(executed_qemu);

    let killed_daemon = process_actions
        .iter()
        .any(|action| matches!(action, RecordedProcessAction::KillDaemon { .. }));
    assert!(killed_daemon);
}

#[test]
fn test_builder_corruption_check_failure_aborts_promotion_and_cleans_up_tmp() {
    let fixture = BuilderTestFixture::new();

    let temporary_master_path = fixture.depot_store_directory.join(format!(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2.tmp",
        fixture.content_hash
    ));

    // Create wrapper script that fails when check is invoked
    let qemu_img_bin = std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join("qemu-img"))
                .find(|executable| executable.is_file())
        })
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "qemu-img".to_string());

    let wrapper_script = fixture.scratch_directory.join("fake-qemu-img.sh");
    let script_content = format!(
        r#"#!/bin/sh
if [ "$1" = "check" ]; then
    echo "Simulated cluster corruption" >&2
    exit 1
fi
exec "{qemu_img_bin}" "$@"
"#
    );
    std::fs::write(&wrapper_script, script_content).expect("write fake qemu-img wrapper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrapper_script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod wrapper script");
    }

    let storage = QemuImgStorage::new().with_executable(&wrapper_script);
    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions {
        scratch_dir: Some(fixture.scratch_directory.clone()),
        ..Default::default()
    };

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must abort on corruption detection");

    match build_error {
        ImageBuilderError::PromotionIntegrityCheckFailed { path, details } => {
            assert_eq!(path, temporary_master_path);
            assert!(details.contains("Simulated cluster corruption"));
        }
        other_error => {
            unreachable!("expected PromotionIntegrityCheckFailed, got: {other_error:?}");
        }
    }

    // Assert that the final golden master was NEVER created
    let golden_master_path = fixture.golden_master_path();
    assert!(!golden_master_path.exists());
    assert!(!temporary_master_path.exists());
}

#[test]
fn test_builder_qemu_process_failure_aborts_and_cleans_up() {
    let fixture = BuilderTestFixture::new();

    let storage = QemuImgStorage::new();

    // Runner: inject non-zero exit code on qemu-system-x86_64
    let runner = MockProcessRunner::new().with_output(
        "qemu-system-x86_64",
        1,
        "",
        "KVM hardware virtualization unavailable",
    );

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions {
        scratch_dir: Some(fixture.scratch_directory.clone()),
        ..Default::default()
    };

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must abort on QEMU failure");

    match build_error {
        ImageBuilderError::QemuExecutionFailed { exit_code, stderr } => {
            assert_eq!(exit_code, Some(1));
            assert!(stderr.contains("KVM hardware virtualization unavailable"));
        }
        other_error => unreachable!("expected QemuExecutionFailed, got: {other_error:?}"),
    }

    // swtpm daemon must have been killed
    let killed_daemon = runner
        .recorded_actions()
        .iter()
        .any(|action| matches!(action, RecordedProcessAction::KillDaemon { .. }));
    assert!(killed_daemon);

    // Scratch disk must be deleted
    let scratch_disk_path = fixture.scratch_directory.join("build-disk.qcow2");
    assert!(!scratch_disk_path.exists());
}
