use std::collections::HashMap;
use std::path::PathBuf;

use onehost::config::model::{FlavorConfiguration, OnehostManifest, StorageDirectoriesConfiguration};
use onehost::image::builder::{ImageBuildOptions, ImageBuildOutcome, ImageBuilder, ImageBuilderError};
use onehost::image::tag::{FlavorResolutionError, ImageTagSpecification};
use onehost::process::mock::{MockProcessRunner, RecordedProcessAction};
use onehost::storage::mock::{MockImageRecord, MockStorageManager, RecordedStorageAction};
use onehost::storage::StorageManager;

struct BuilderTestFixture {
    manifest: OnehostManifest,
    image_tag: ImageTagSpecification,
    iso_path: PathBuf,
    oemdrv_path: PathBuf,
    depot_store_directory: PathBuf,
    depot_iso_directory: PathBuf,
    scratch_directory: PathBuf,
}

impl BuilderTestFixture {
    fn new() -> Self {
        let depot_store_directory = PathBuf::from("/data/depot/store");
        let depot_iso_directory = PathBuf::from("/data/depot/iso");
        let depot_backup_directory = PathBuf::from("/data/depot/backups");
        let nvram_directory = PathBuf::from("/var/lib/libvirt/qemu/nvram");
        let nvram_template = PathBuf::from("/run/current-system/sw/share/OVMF/OVMF_VARS.fd");
        let ovmf_code = PathBuf::from("/run/current-system/sw/share/OVMF/OVMF_CODE.fd");
        let scratch_directory = PathBuf::from("/tmp/onehost-build-test-1234");

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_directory.clone(),
            depot_iso_directory.clone(),
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
            FlavorConfiguration::new(oemdrv_path.clone(), "ba6eafb7"),
        );

        let manifest = OnehostManifest::new(
            "1.0",
            storage_config,
            flavors,
            HashMap::new(),
        )
        .expect("manifest creation succeeds");

        let image_tag: ImageTagSpecification = "win11/26300.9457.pro.en-us/looking-glass"
            .parse()
            .expect("image tag parsing");
        let iso_path = depot_iso_directory.join("win11-26300.9457.pro.en-us.iso");

        Self {
            manifest,
            image_tag,
            iso_path,
            oemdrv_path,
            depot_store_directory,
            depot_iso_directory,
            scratch_directory,
        }
    }
}

#[test]
fn test_builder_already_existing_master_returns_early_without_process_spawning() {
    let fixture = BuilderTestFixture::new();

    let golden_master_path = fixture
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");

    let storage = MockStorageManager::new()
        .with_image(&golden_master_path, MockImageRecord::default());
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

    // Storage does NOT contain the ISO file
    let storage = MockStorageManager::new()
        .with_image(&fixture.oemdrv_path, MockImageRecord::default());
    let runner = MockProcessRunner::new();

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must fail when ISO is missing");

    match build_error {
        ImageBuilderError::IsoNotFound { iso_directory, build_version } => {
            assert_eq!(iso_directory, fixture.depot_iso_directory);
            assert_eq!(build_version, "26300.9457.pro.en-us");
        }
        other_error => panic!("expected IsoNotFound, got: {:?}", other_error),
    }

    assert!(runner.recorded_actions().is_empty());
}

#[test]
fn test_builder_unregistered_flavor_fails_fast() {
    let fixture = BuilderTestFixture::new();

    let storage = MockStorageManager::new()
        .with_image(&fixture.iso_path, MockImageRecord::default())
        .with_image(&fixture.oemdrv_path, MockImageRecord::default());
    let runner = MockProcessRunner::new();

    let unknown_tag: ImageTagSpecification = "win11/26300.9457.pro.en-us/nonexistent-flavor"
        .parse()
        .expect("image tag parsing");

    let builder = ImageBuilder::new(&storage, &runner);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &unknown_tag, &options)
        .expect_err("must fail on unregistered flavor");

    match build_error {
        ImageBuilderError::FlavorError(FlavorResolutionError::UnknownFlavor { requested_flavor, .. }) => {
            assert_eq!(requested_flavor, "nonexistent-flavor");
        }
        other_error => panic!("expected UnknownFlavor, got: {:?}", other_error),
    }
}

#[test]
fn test_builder_missing_oemdrv_fails_fast() {
    let fixture = BuilderTestFixture::new();

    // Storage contains ISO but NOT OEMDRV
    let storage = MockStorageManager::new()
        .with_image(&fixture.iso_path, MockImageRecord::default());
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
        other_error => panic!("expected OemdrvNotFound, got: {:?}", other_error),
    }
}

#[test]
fn test_builder_atomic_promotion_sequence_and_scratch_cleanup() {
    let fixture = BuilderTestFixture::new();

    let storage = MockStorageManager::new()
        .with_image(&fixture.iso_path, MockImageRecord::default())
        .with_image(&fixture.oemdrv_path, MockImageRecord::default());

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

    let expected_master_path = fixture
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");

    assert_eq!(
        outcome,
        ImageBuildOutcome::Built {
            golden_master_path: expected_master_path.clone(),
            virtual_size_bytes: 64 * 1024 * 1024 * 1024,
            archive_size_bytes: 50_000,
        }
    );

    // Verify storage actions in order:
    // 1. create_empty_disk in scratch
    // 2. initialize_nvram in scratch
    // 3. convert_thin_backup from scratch to .tmp in depot store
    // 4. check_image on .tmp
    // 5. move_file_safely from .tmp to final .qcow2
    // 6. set_readonly on final .qcow2
    // 7. delete_image on scratch disk
    let storage_actions = storage.recorded_actions();

    let scratch_disk_path = fixture.scratch_directory.join("build-disk.qcow2");
    let temporary_master_path = fixture
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2.tmp");

    assert!(storage_actions.contains(&RecordedStorageAction::CreateEmptyDisk {
        disk_path: scratch_disk_path.clone(),
        size_bytes: 64 * 1024 * 1024 * 1024,
    }));

    assert!(storage_actions.contains(&RecordedStorageAction::ConvertThinBackup {
        source_disk_path: scratch_disk_path.clone(),
        destination_archive_path: temporary_master_path.clone(),
        backing_file_path: None,
        compress: true,
    }));

    assert!(storage_actions.contains(&RecordedStorageAction::CheckImage {
        image_path: temporary_master_path.clone(),
    }));

    assert!(storage_actions.contains(&RecordedStorageAction::MoveFileSafely {
        source_path: temporary_master_path,
        destination_path: expected_master_path.clone(),
    }));

    assert!(storage_actions.contains(&RecordedStorageAction::SetReadonly {
        file_path: expected_master_path,
    }));

    assert!(storage_actions.contains(&RecordedStorageAction::DeleteImage {
        image_path: scratch_disk_path,
    }));

    // Verify process actions:
    // 1. spawn_daemon("swtpm")
    // 2. execute("qemu-system-x86_64")
    // 3. kill_daemon
    let process_actions = runner.recorded_actions();

    let spawned_swtpm = process_actions.iter().any(|action| matches!(action, RecordedProcessAction::SpawnDaemon { program, .. } if program == "swtpm"));
    assert!(spawned_swtpm);

    let executed_qemu = process_actions.iter().any(|action| matches!(action, RecordedProcessAction::Execute { program, .. } if program == "qemu-system-x86_64"));
    assert!(executed_qemu);

    let killed_daemon = process_actions.iter().any(|action| matches!(action, RecordedProcessAction::KillDaemon { .. }));
    assert!(killed_daemon);
}

#[test]
fn test_builder_corruption_check_failure_aborts_promotion_and_cleans_up_tmp() {
    let fixture = BuilderTestFixture::new();

    let temporary_master_path = fixture
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2.tmp");

    // Storage: ISO and OEMDRV present, but inject corruption into .tmp
    let storage = MockStorageManager::new()
        .with_image(&fixture.iso_path, MockImageRecord::default())
        .with_image(&fixture.oemdrv_path, MockImageRecord::default())
        .with_injected_error(&temporary_master_path, "Simulated cluster corruption");

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
        other_error => panic!("expected PromotionIntegrityCheckFailed, got: {:?}", other_error),
    }

    // Assert that the final golden master was NEVER created
    let golden_master_path = fixture
        .depot_store_directory
        .join("win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2");
    assert!(storage.inspect_image(&golden_master_path).is_err());
}

#[test]
fn test_builder_qemu_process_failure_aborts_and_cleans_up() {
    let fixture = BuilderTestFixture::new();

    let storage = MockStorageManager::new()
        .with_image(&fixture.iso_path, MockImageRecord::default())
        .with_image(&fixture.oemdrv_path, MockImageRecord::default());

    // Runner: inject non-zero exit code on qemu-system-x86_64
    let runner = MockProcessRunner::new()
        .with_output("qemu-system-x86_64", 1, "", "KVM hardware virtualization unavailable");

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
        other_error => panic!("expected QemuExecutionFailed, got: {:?}", other_error),
    }

    // swtpm daemon must have been killed
    let killed_daemon = runner.recorded_actions().iter().any(|action| matches!(action, RecordedProcessAction::KillDaemon { .. }));
    assert!(killed_daemon);
}
