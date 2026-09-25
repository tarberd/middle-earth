use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

use onehost::storage::{
    execute_cross_device_streaming_move, parse_qemu_img_info_json, MockImageRecord,
    MockStorageManager, RecordedStorageAction, StorageError, StorageManager,
};

const SAMPLE_QEMU_IMG_INFO_JSON: &str = r#"
{
    "virtual-size": 68719476736,
    "filename": "/var/lib/libvirt/images/win11-gollum.qcow2",
    "format": "qcow2",
    "actual-size": 12845056,
    "format-specific": {
        "type": "qcow2",
        "data": {
            "compat": "1.1",
            "compression-type": "zlib"
        }
    },
    "dirty-flag": false,
    "backing-filename": "/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2",
    "backing-filename-format": "qcow2"
}
"#;

#[test]
fn test_execute_cross_device_streaming_move() {
    let source_directory = tempdir().expect("Failed to create source directory");
    let destination_directory = tempdir().expect("Failed to create destination directory");

    let source_path = source_directory.path().join("vm-disk.qcow2");
    let destination_path = destination_directory.path().join("vm-disk-moved.qcow2");

    let test_payload = b"QCOW2_TEST_IMAGE_HEADER_AND_CLUSTERS_REPRESENTATIVE_PAYLOAD";
    fs::write(&source_path, test_payload).expect("Failed to write test source payload");

    execute_cross_device_streaming_move(&source_path, &destination_path)
        .expect("Streaming move should succeed");

    assert!(!source_path.exists(), "Source file must be unlinked after successful move");
    assert!(destination_path.exists(), "Destination file must exist after move");

    let transferred_content = fs::read(&destination_path).expect("Failed to read moved file");
    assert_eq!(transferred_content, test_payload);
}

#[test]
fn test_execute_cross_device_streaming_move_nonexistent_source() {
    let destination_directory = tempdir().expect("Failed to create destination directory");
    let non_existent_source = Path::new("/tmp/non_existent_source_path_for_test.qcow2");
    let destination_path = destination_directory.path().join("destination.qcow2");

    let result = execute_cross_device_streaming_move(non_existent_source, &destination_path);
    assert!(matches!(result, Err(StorageError::SourceFileNotFound { .. })));
}

#[test]
fn test_execute_cross_device_streaming_move_nonexistent_destination_parent() {
    let source_directory = tempdir().expect("Failed to create source directory");
    let source_path = source_directory.path().join("source.qcow2");
    fs::write(&source_path, b"dummy payload").expect("Failed to write source file");

    let non_existent_destination = Path::new("/tmp/non_existent_parent_dir_xyz123/destination.qcow2");
    let result = execute_cross_device_streaming_move(&source_path, non_existent_destination);
    assert!(matches!(
        result,
        Err(StorageError::DestinationDirectoryNotFound { .. })
    ));
}

#[test]
fn test_mock_storage_manager_overlay_creation_and_rebase() {
    let storage = MockStorageManager::new();

    let backing_path = PathBuf::from("/var/lib/libvirt/images/win11-base.qcow2");
    let overlay_path = PathBuf::from("/var/lib/libvirt/images/win11-instance.qcow2");
    let new_backing_path = PathBuf::from("/data/kvm/libvirt/images/win11-base.qcow2");

    storage
        .create_cow_overlay(&backing_path, &overlay_path)
        .expect("Overlay creation should succeed");

    let inspected = storage.inspect_image(&overlay_path).expect("Inspection should succeed");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.backing_file, Some(backing_path.clone()));

    storage
        .rebase_overlay(&overlay_path, &new_backing_path, true)
        .expect("Unsafe rebase should succeed");

    let rebased_inspected = storage.inspect_image(&overlay_path).expect("Rebased inspection should succeed");
    assert_eq!(rebased_inspected.backing_file, Some(new_backing_path.clone()));

    let actions = storage.recorded_actions();
    assert!(actions.contains(&RecordedStorageAction::CreateCowOverlay {
        backing_file_path: backing_path,
        overlay_path: overlay_path.clone(),
    }));
    assert!(actions.contains(&RecordedStorageAction::RebaseOverlay {
        overlay_path,
        new_backing_file_path: new_backing_path,
        unsafe_mode: true,
    }));
}

#[test]
fn test_mock_storage_manager_image_inspection_and_check() {
    let valid_path = PathBuf::from("/data/store/valid.qcow2");
    let corrupted_path = PathBuf::from("/data/store/corrupted.qcow2");

    let valid_record = MockImageRecord {
        format: "qcow2".to_string(),
        virtual_size_bytes: 68_719_476_736,
        actual_size_bytes: 15_000_000,
        backing_file: None,
        is_corrupted: false,
    };

    let corrupted_record = MockImageRecord {
        format: "qcow2".to_string(),
        virtual_size_bytes: 68_719_476_736,
        actual_size_bytes: 15_000_000,
        backing_file: None,
        is_corrupted: true,
    };

    let storage = MockStorageManager::new()
        .with_image(&valid_path, valid_record)
        .with_image(&corrupted_path, corrupted_record);

    assert!(storage.check_image(&valid_path).is_ok());

    let corruption_result = storage.check_image(&corrupted_path);
    assert!(matches!(
        corruption_result,
        Err(StorageError::ImageCorruptionDetected { .. })
    ));
}

#[test]
fn test_mock_storage_manager_copy_base_image_sets_readonly_permissions() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let depot_path = temp_workspace.path().join("depot-master.qcow2");
    let pool_path = temp_workspace.path().join("pool").join("cached-master.qcow2");

    fs::write(&depot_path, b"GOLDEN_MASTER_READONLY_BITS").expect("Failed to write golden master");

    let storage = MockStorageManager::new();
    storage
        .copy_base_image(&depot_path, &pool_path)
        .expect("Base image copy should succeed");

    assert!(pool_path.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&pool_path).expect("Failed to read metadata");
        let permissions_mode = metadata.permissions().mode() & 0o777;
        assert_eq!(permissions_mode, 0o444);
    }
}

#[test]
fn test_storage_manager_move_file_safely_same_filesystem() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let source_path = temp_workspace.path().join("source.qcow2");
    let destination_path = temp_workspace.path().join("destination.qcow2");

    fs::write(&source_path, b"DISK_DATA").expect("Failed to write source file");

    let storage = MockStorageManager::new();
    storage
        .move_file_safely(&source_path, &destination_path)
        .expect("Safe move on same filesystem should succeed");

    assert!(!source_path.exists());
    assert!(destination_path.exists());
    assert_eq!(fs::read(&destination_path).unwrap(), b"DISK_DATA");
}

#[test]
fn test_storage_manager_move_file_safely_simulated_cross_device_fallback() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let source_path = temp_workspace.path().join("cross-source.qcow2");
    let destination_path = temp_workspace.path().join("cross-destination.qcow2");

    fs::write(&source_path, b"CROSS_DEVICE_DATA").expect("Failed to write source file");

    let storage = MockStorageManager::new().with_simulated_cross_device_path(&source_path);

    storage
        .move_file_safely(&source_path, &destination_path)
        .expect("Cross device move fallback should succeed");

    assert!(!source_path.exists());
    assert!(destination_path.exists());
    assert_eq!(fs::read(&destination_path).unwrap(), b"CROSS_DEVICE_DATA");
}

#[test]
fn test_qemu_img_parse_inspection_json() {
    let inspection = parse_qemu_img_info_json(SAMPLE_QEMU_IMG_INFO_JSON)
        .expect("Failed to parse qemu-img JSON output");

    assert_eq!(inspection.format, "qcow2");
    assert_eq!(inspection.virtual_size_bytes, 68_719_476_736);
    assert_eq!(inspection.actual_size_bytes, 12_845_056);
    assert_eq!(
        inspection.backing_file,
        Some(PathBuf::from(
            "/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2"
        ))
    );
}

#[test]
fn test_mock_storage_manager_convert_thin_backup() {
    let storage = MockStorageManager::new();

    let source_disk = PathBuf::from("/var/lib/libvirt/images/win11-active.qcow2");
    let destination_archive = PathBuf::from("/data/depot/backup/archive.qcow2");
    let backing_file = PathBuf::from("/var/lib/libvirt/images/win11-base.qcow2");

    storage
        .convert_thin_backup(&source_disk, &destination_archive, Some(&backing_file), true)
        .expect("Thin backup conversion should succeed");

    let actions = storage.recorded_actions();
    assert!(actions.contains(&RecordedStorageAction::ConvertThinBackup {
        source_disk_path: source_disk,
        destination_archive_path: destination_archive,
        backing_file_path: Some(backing_file),
        compress: true,
    }));
}
