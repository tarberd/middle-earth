use std::collections::HashMap;
use std::path::PathBuf;

use onehost::config::model::{
    FlavorConfiguration, OnehostManifest, StorageDirectoriesConfiguration,
};
use onehost::image::builder::{
    ImageBuildOptions, ImageBuildOutcome, ImageBuilder, ImageBuilderError,
};
use onehost::image::tag::{FlavorResolutionError, ImageTagSpecification};
use onehost::storage::QemuImgStorage;

pub struct BuilderTestFixture {
    pub temporary_directory: tempfile::TempDir,
    pub manifest: OnehostManifest,
    pub image_tag: ImageTagSpecification,
    pub iso_path: PathBuf,
    pub oemdrv_path: PathBuf,
    pub depot_store_directory: PathBuf,
    pub depot_iso_directory: PathBuf,
    pub scratch_directory: PathBuf,
    pub swtpm_script_path: PathBuf,
    pub qemu_script_path: PathBuf,
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

        let swtpm_script_path = base_path.join("fake-swtpm.sh");
        let qemu_script_path = base_path.join("fake-qemu.sh");

        let swtpm_content = "#!/bin/sh\nexec sleep 60\n";
        std::fs::write(&swtpm_script_path, swtpm_content).expect("write fake swtpm script");

        let qemu_content = "#!/bin/sh\nexit 0\n";
        std::fs::write(&qemu_script_path, qemu_content).expect("write fake qemu script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&swtpm_script_path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod swtpm script");
            std::fs::set_permissions(&qemu_script_path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod qemu script");
        }

        let storage_config = StorageDirectoriesConfiguration::new(
            depot_store_directory.clone(),
            depot_iso_directory.clone(),
            depot_backup_directory,
            nvram_directory,
            nvram_template,
            ovmf_code,
        );

        let extra_isos = HashMap::new();
        let content_hash = QemuImgStorage::new()
            .compute_content_hash(&oemdrv_path, &extra_isos)
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
            swtpm_script_path,
            qemu_script_path,
            content_hash,
        }
    }

    fn builder<'a>(&'a self, storage: &'a QemuImgStorage) -> ImageBuilder<'a> {
        ImageBuilder::new(storage)
            .with_swtpm_binary(&self.swtpm_script_path)
            .with_qemu_binary(&self.qemu_script_path)
    }

    fn golden_master_path(&self) -> PathBuf {
        self.depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2",
            self.content_hash
        ))
    }

    fn temporary_master_path(&self) -> PathBuf {
        self.depot_store_directory.join(format!(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-{}.qcow2.tmp",
            self.content_hash
        ))
    }

    fn scratch_disk_path(&self) -> PathBuf {
        self.scratch_directory.join("build-disk.qcow2")
    }
}

#[test]
fn test_builder_already_existing_master_returns_early_without_modifying_storage() {
    let fixture = BuilderTestFixture::new();
    let golden_master_path = fixture.golden_master_path();

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&golden_master_path, 1024 * 1024)
        .expect("create golden master disk");

    let initial_metadata = std::fs::metadata(&golden_master_path).expect("read metadata");
    let initial_modified = initial_metadata.modified().expect("read modified time");

    let builder = fixture.builder(&storage);
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

    // Verify existing master is retained and unmodified
    assert!(golden_master_path.exists());
    let current_metadata = std::fs::metadata(&golden_master_path).expect("read current metadata");
    assert_eq!(
        initial_modified,
        current_metadata.modified().expect("read current modified time")
    );

    // Verify scratch directory has no build artifacts
    assert!(!fixture.scratch_disk_path().exists());
    assert!(!fixture.temporary_master_path().exists());

    // Verify depot store contains only the initial golden master
    let depot_store_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert_eq!(depot_store_entries, vec![golden_master_path]);
}

#[test]
fn test_builder_missing_iso_fails_fast_with_typed_error() {
    let fixture = BuilderTestFixture::new();

    // Remove the ISO file from depot
    if fixture.iso_path.exists() {
        std::fs::remove_file(&fixture.iso_path).expect("remove iso file");
    }

    let storage = QemuImgStorage::new();
    let builder = fixture.builder(&storage);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must fail when ISO is missing");

    assert!(matches!(
        build_error,
        ImageBuilderError::IsoNotFound {
            ref iso_directory,
            ref release_tuple,
        } if *iso_directory == fixture.depot_iso_directory
            && release_tuple == "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us"
    ));

    // Verify no build artifacts or temporary files were created
    assert!(!fixture.golden_master_path().exists());
    assert!(!fixture.temporary_master_path().exists());
    assert!(!fixture.scratch_disk_path().exists());

    let depot_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert!(depot_entries.is_empty());
}

#[test]
fn test_builder_unregistered_flavor_fails_fast() {
    let fixture = BuilderTestFixture::new();

    let storage = QemuImgStorage::new();
    let unknown_tag: ImageTagSpecification =
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/nonexistent-flavor"
            .parse()
            .expect("image tag parsing");

    let builder = fixture.builder(&storage);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &unknown_tag, &options)
        .expect_err("must fail on unregistered flavor");

    assert!(matches!(
        build_error,
        ImageBuilderError::FlavorError(FlavorResolutionError::UnknownFlavor {
            ref requested_flavor,
            ..
        }) if requested_flavor == "nonexistent-flavor"
    ));

    // Verify no build artifacts or temporary files were created
    assert!(!fixture.golden_master_path().exists());
    assert!(!fixture.temporary_master_path().exists());
    assert!(!fixture.scratch_disk_path().exists());

    let depot_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert!(depot_entries.is_empty());
}

#[test]
fn test_builder_missing_oemdrv_fails_fast() {
    let fixture = BuilderTestFixture::new();

    // Remove OEMDRV file
    if fixture.oemdrv_path.exists() {
        std::fs::remove_file(&fixture.oemdrv_path).expect("remove oemdrv file");
    }

    let storage = QemuImgStorage::new();
    let builder = fixture.builder(&storage);
    let options = ImageBuildOptions::default();

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must fail when OEMDRV is missing");

    assert!(matches!(
        build_error,
        ImageBuilderError::OemdrvNotFound { ref path } if *path == fixture.oemdrv_path
    ));

    // Verify no build artifacts or temporary files were created
    assert!(!fixture.golden_master_path().exists());
    assert!(!fixture.temporary_master_path().exists());
    assert!(!fixture.scratch_disk_path().exists());

    let depot_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert!(depot_entries.is_empty());
}

#[test]
fn test_builder_atomic_promotion_sequence_and_scratch_cleanup() {
    let fixture = BuilderTestFixture::new();

    let storage = QemuImgStorage::new();
    let builder = fixture.builder(&storage);
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

    assert!(matches!(
        outcome,
        ImageBuildOutcome::Built {
            ref golden_master_path,
            virtual_size_bytes,
            archive_size_bytes,
        } if *golden_master_path == expected_master_path
            && virtual_size_bytes == 64 * 1024 * 1024 * 1024
            && archive_size_bytes > 0
    ));

    // 1. Golden master promoted and exists on filesystem
    assert!(expected_master_path.exists());

    // 2. Validate image structure, format, and size via QemuImgStorage inspection
    let inspected = storage
        .inspect_image(&expected_master_path)
        .expect("inspect golden master");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.virtual_size_bytes, 64 * 1024 * 1024 * 1024);
    assert!(inspected.backing_file.is_none());
    assert!(inspected.actual_size_bytes > 0);

    // 3. Verify image integrity with qemu-img check
    storage
        .check_image(&expected_master_path)
        .expect("promoted golden master integrity check");

    // 4. Strict read-only permissions (0444) on promoted golden master
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(&expected_master_path)
            .expect("metadata")
            .permissions();
        assert_eq!(permissions.mode() & 0o777, 0o444);
    }

    // 5. Scratch disk and temporary staging files cleaned up
    assert!(!fixture.scratch_disk_path().exists());
    assert!(!fixture.temporary_master_path().exists());

    // 6. Depot store contains only the final promoted master
    let depot_store_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert_eq!(depot_store_entries, vec![expected_master_path]);
}

#[test]
fn test_builder_force_flag_rebuilds_and_overwrites_existing_master() {
    let fixture = BuilderTestFixture::new();
    let golden_master_path = fixture.golden_master_path();

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&golden_master_path, 1024 * 1024)
        .expect("create initial small golden master");

    let initial_inspection = storage
        .inspect_image(&golden_master_path)
        .expect("inspect initial disk");
    assert_eq!(initial_inspection.virtual_size_bytes, 1024 * 1024);

    let builder = fixture.builder(&storage);
    let options = ImageBuildOptions {
        force: true,
        scratch_dir: Some(fixture.scratch_directory.clone()),
        iso_path: None,
        memory_mb: 8192,
        cpu_count: 8,
    };

    let outcome = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect("forced rebuild succeeds");

    assert!(matches!(
        outcome,
        ImageBuildOutcome::Built {
            golden_master_path: ref outcome_path,
            virtual_size_bytes,
            archive_size_bytes,
        } if *outcome_path == golden_master_path
            && virtual_size_bytes == 64 * 1024 * 1024 * 1024
            && archive_size_bytes > 0
    ));

    // Verify rebuilt golden master replaced the old disk with the 64GB image
    assert!(golden_master_path.exists());
    let inspected = storage
        .inspect_image(&golden_master_path)
        .expect("inspect rebuilt golden master");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.virtual_size_bytes, 64 * 1024 * 1024 * 1024);
    assert!(inspected.backing_file.is_none());

    // Verify readonly permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::metadata(&golden_master_path)
            .expect("metadata")
            .permissions();
        assert_eq!(permissions.mode() & 0o777, 0o444);
    }

    // Verify scratch disk and temporary files cleaned up
    assert!(!fixture.scratch_disk_path().exists());
    assert!(!fixture.temporary_master_path().exists());

    let depot_store_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert_eq!(depot_store_entries, vec![golden_master_path]);
}

#[test]
fn test_builder_corruption_check_failure_aborts_promotion_and_cleans_up_tmp() {
    let fixture = BuilderTestFixture::new();
    let temporary_master_path = fixture.temporary_master_path();

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
    let builder = fixture.builder(&storage);
    let options = ImageBuildOptions {
        scratch_dir: Some(fixture.scratch_directory.clone()),
        ..Default::default()
    };

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must abort on corruption detection");

    assert!(matches!(
        build_error,
        ImageBuilderError::PromotionIntegrityCheckFailed { ref path, ref details }
            if *path == temporary_master_path && details.contains("Simulated cluster corruption")
    ));

    // Assert that the final golden master was NEVER created
    assert!(!fixture.golden_master_path().exists());
    // Staging temporary master must be unlinked
    assert!(!temporary_master_path.exists());
    // Scratch disk must be cleaned up
    assert!(!fixture.scratch_disk_path().exists());

    // Depot store directory has no leftover files
    let depot_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert!(depot_entries.is_empty());
}

#[test]
fn test_builder_qemu_process_failure_aborts_and_cleans_up() {
    let fixture = BuilderTestFixture::new();

    let failing_qemu_script = fixture.scratch_directory.join("failing-qemu.sh");
    let script_content = "#!/bin/sh\necho \"KVM hardware virtualization unavailable\" >&2\nexit 1\n";
    std::fs::write(&failing_qemu_script, script_content).expect("write failing qemu script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&failing_qemu_script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod failing qemu script");
    }

    let storage = QemuImgStorage::new();
    let builder = fixture.builder(&storage).with_qemu_binary(&failing_qemu_script);
    let options = ImageBuildOptions {
        scratch_dir: Some(fixture.scratch_directory.clone()),
        ..Default::default()
    };

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must abort on QEMU failure");

    assert!(matches!(
        build_error,
        ImageBuilderError::QemuExecutionFailed {
            exit_code: Some(1),
            ref stderr,
        } if stderr.contains("KVM hardware virtualization unavailable")
    ));

    // Verify observable filesystem state on failure:
    // 1. Final golden master was NEVER created
    assert!(!fixture.golden_master_path().exists());

    // 2. Temporary staged master was NEVER created / does not exist
    assert!(!fixture.temporary_master_path().exists());

    // 3. Scratch disk must be deleted on failure
    assert!(!fixture.scratch_disk_path().exists());

    // 4. Depot store directory remains completely empty
    let depot_entries: Vec<PathBuf> = std::fs::read_dir(&fixture.depot_store_directory)
        .expect("read depot store directory")
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    assert!(depot_entries.is_empty());
}

#[test]
fn test_builder_swtpm_process_failure_aborts_and_cleans_up() {
    let fixture = BuilderTestFixture::new();
    let storage = QemuImgStorage::new();

    let nonexistent_swtpm = fixture.scratch_directory.join("nonexistent-swtpm");
    let builder = fixture.builder(&storage).with_swtpm_binary(&nonexistent_swtpm);
    let options = ImageBuildOptions {
        scratch_dir: Some(fixture.scratch_directory.clone()),
        ..Default::default()
    };

    let build_error = builder
        .build_image(&fixture.manifest, &fixture.image_tag, &options)
        .expect_err("must abort on swtpm failure");

    assert!(matches!(
        build_error,
        ImageBuilderError::SwtpmError { ref details }
            if details.contains("Failed to spawn swtpm binary")
    ));

    // Verify filesystem state:
    assert!(!fixture.golden_master_path().exists());
    assert!(!fixture.temporary_master_path().exists());
    assert!(!fixture.scratch_disk_path().exists());
}
