use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

use onehost::storage::{
    QemuImgStorage, StorageError, StorageManager, execute_compute_content_hash,
    execute_cross_device_streaming_move, execute_safe_file_move, parse_qemu_img_info_json,
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

    assert!(
        !source_path.exists(),
        "Source file must be unlinked after successful move"
    );
    assert!(
        destination_path.exists(),
        "Destination file must exist after move"
    );

    let transferred_content = fs::read(&destination_path).expect("Failed to read moved file");
    assert_eq!(transferred_content, test_payload);
}

#[test]
fn test_execute_cross_device_streaming_move_nonexistent_source() {
    let destination_directory = tempdir().expect("Failed to create destination directory");
    let non_existent_source = Path::new("/tmp/non_existent_source_path_for_test.qcow2");
    let destination_path = destination_directory.path().join("destination.qcow2");

    let result = execute_cross_device_streaming_move(non_existent_source, &destination_path);
    assert!(matches!(
        result,
        Err(StorageError::SourceFileNotFound { .. })
    ));
}

#[test]
fn test_execute_cross_device_streaming_move_nonexistent_destination_parent() {
    let source_directory = tempdir().expect("Failed to create source directory");
    let source_path = source_directory.path().join("source.qcow2");
    fs::write(&source_path, b"dummy payload").expect("Failed to write source file");

    let non_existent_destination =
        Path::new("/tmp/non_existent_parent_dir_xyz123/destination.qcow2");
    let result = execute_cross_device_streaming_move(&source_path, non_existent_destination);
    assert!(matches!(
        result,
        Err(StorageError::DestinationDirectoryNotFound { .. })
    ));
}

#[test]
fn test_qemu_img_storage_overlay_creation_and_rebase() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let backing_path = temp_workspace.path().join("win11-base.qcow2");
    let overlay_path = temp_workspace.path().join("win11-instance.qcow2");
    let new_backing_path = temp_workspace.path().join("new-win11-base.qcow2");

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&backing_path, 20 * 1024 * 1024)
        .expect("Base disk creation should succeed");
    storage
        .create_empty_disk(&new_backing_path, 20 * 1024 * 1024)
        .expect("New base disk creation should succeed");

    storage
        .create_cow_overlay(&backing_path, &overlay_path)
        .expect("Overlay creation should succeed");

    assert!(overlay_path.exists());
    let inspected = storage
        .inspect_image(&overlay_path)
        .expect("Inspection should succeed");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.backing_file, Some(backing_path));

    storage
        .rebase_overlay(&overlay_path, &new_backing_path, true)
        .expect("Unsafe rebase should succeed");

    let rebased_inspected = storage
        .inspect_image(&overlay_path)
        .expect("Rebased inspection should succeed");
    assert_eq!(rebased_inspected.backing_file, Some(new_backing_path));
}

#[test]
fn test_qemu_img_storage_image_inspection_and_check() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let valid_path = temp_workspace.path().join("valid.qcow2");
    let corrupted_path = temp_workspace.path().join("corrupted.qcow2");

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&valid_path, 10 * 1024 * 1024)
        .expect("Failed to create valid disk");
    storage
        .create_empty_disk(&corrupted_path, 10 * 1024 * 1024)
        .expect("Failed to create disk for corruption");

    let inspected = storage
        .inspect_image(&valid_path)
        .expect("Inspection should succeed");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.virtual_size_bytes, 10 * 1024 * 1024);
    assert_eq!(inspected.backing_file, None);

    assert!(storage.check_image(&valid_path).is_ok());

    // Corrupt QCOW2 header bytes
    let mut file = fs::File::options()
        .write(true)
        .open(&corrupted_path)
        .expect("Failed to open file for corruption");
    file.write_all(&[0xFFu8; 1024])
        .expect("Failed to write corrupt bytes");
    drop(file);

    let corruption_result = storage.check_image(&corrupted_path);
    assert!(matches!(
        corruption_result,
        Err(StorageError::ImageCorruptionDetected { .. })
    ));
    if let Err(StorageError::ImageCorruptionDetected { path, details }) = corruption_result {
        assert_eq!(path, corrupted_path);
        assert!(!details.is_empty());
    }
}

#[test]
fn test_qemu_img_storage_copy_base_image_sets_readonly_permissions() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let depot_path = temp_workspace.path().join("depot-master.qcow2");
    let pool_path = temp_workspace
        .path()
        .join("pool")
        .join("cached-master.qcow2");

    fs::write(&depot_path, b"GOLDEN_MASTER_READONLY_BITS").expect("Failed to write golden master");

    let storage = QemuImgStorage::new();
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
fn test_qemu_img_initialize_nvram() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let template_path = temp_workspace.path().join("template_VARS.fd");
    let destination_nvram = temp_workspace
        .path()
        .join("nvram_dir")
        .join("instance_VARS.fd");

    fs::write(&template_path, b"NVRAM_TEMPLATE_BITS").expect("Failed to write template NVRAM");

    let storage = QemuImgStorage::new();
    storage
        .initialize_nvram(&template_path, &destination_nvram)
        .expect("NVRAM initialization should succeed");

    assert!(destination_nvram.exists());
    assert_eq!(
        fs::read(&destination_nvram).expect("NVRAM destination should be readable"),
        b"NVRAM_TEMPLATE_BITS"
    );
}

#[test]
fn test_storage_manager_move_file_safely_same_filesystem() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let source_path = temp_workspace.path().join("source.qcow2");
    let destination_path = temp_workspace.path().join("destination.qcow2");

    fs::write(&source_path, b"DISK_DATA").expect("Failed to write source file");

    let storage = QemuImgStorage::new();
    storage
        .move_file_safely(&source_path, &destination_path)
        .expect("Safe move on same filesystem should succeed");

    assert!(!source_path.exists());
    assert!(destination_path.exists());
    assert_eq!(
        fs::read(&destination_path).expect("Destination file should be readable"),
        b"DISK_DATA"
    );
}

#[test]
fn test_execute_safe_file_move_success() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let source_path = temp_workspace.path().join("source_direct.qcow2");
    let destination_path = temp_workspace.path().join("destination_direct.qcow2");

    fs::write(&source_path, b"DIRECT_SAFE_MOVE_DATA").expect("Failed to write source file");

    execute_safe_file_move(&source_path, &destination_path)
        .expect("Direct execute_safe_file_move should succeed");

    assert!(!source_path.exists());
    assert!(destination_path.exists());
    assert_eq!(
        fs::read(&destination_path).expect("Moved destination file should be readable"),
        b"DIRECT_SAFE_MOVE_DATA"
    );
}

#[test]
fn test_qemu_img_storage_convert_thin_backup() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let base_disk = temp_workspace.path().join("win11-base.qcow2");
    let source_disk = temp_workspace.path().join("win11-active.qcow2");
    let destination_archive = temp_workspace.path().join("archive.qcow2");

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&base_disk, 10 * 1024 * 1024)
        .expect("Base disk creation should succeed");
    storage
        .create_cow_overlay(&base_disk, &source_disk)
        .expect("Overlay creation should succeed");

    storage
        .convert_thin_backup(
            &source_disk,
            &destination_archive,
            Some(&base_disk),
            true,
        )
        .expect("Thin backup conversion should succeed");

    assert!(destination_archive.exists());
    let inspected = storage
        .inspect_image(&destination_archive)
        .expect("Inspection should succeed");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.virtual_size_bytes, 10 * 1024 * 1024);
    assert_eq!(inspected.backing_file, Some(base_disk));
}

#[test]
fn test_qemu_img_storage_restore_thin_backup_and_delete_image() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let depot_base = temp_workspace.path().join("base.qcow2");
    let overlay_path = temp_workspace.path().join("overlay.qcow2");
    let backup_archive = temp_workspace.path().join("backup.qcow2");
    let restored_overlay = temp_workspace.path().join("restored.qcow2");

    let storage = QemuImgStorage::new();
    storage
        .create_empty_disk(&depot_base, 10 * 1024 * 1024)
        .expect("Base disk creation should succeed");

    storage
        .create_cow_overlay(&depot_base, &overlay_path)
        .expect("Overlay creation should succeed");

    storage
        .convert_thin_backup(&overlay_path, &backup_archive, Some(&depot_base), true)
        .expect("Thin backup conversion should succeed");

    // Restore thin backup
    storage
        .restore_thin_backup(&backup_archive, &restored_overlay, Some(&depot_base))
        .expect("Restore thin backup should succeed");

    assert!(restored_overlay.exists());
    let inspected = storage
        .inspect_image(&restored_overlay)
        .expect("Inspect restored overlay should succeed");
    assert_eq!(inspected.format, "qcow2");
    assert_eq!(inspected.virtual_size_bytes, 10 * 1024 * 1024);
    assert_eq!(inspected.backing_file, Some(depot_base));

    // Delete image
    storage
        .delete_image(&restored_overlay)
        .expect("Delete image should succeed");
    assert!(!restored_overlay.exists());
    assert!(storage.inspect_image(&restored_overlay).is_err());

    // Idempotent delete on already removed file
    storage
        .delete_image(&restored_overlay)
        .expect("Idempotent delete should succeed");
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
fn test_qemu_img_storage_write_read_file_and_set_readonly() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let file_path = temp_workspace.path().join("config").join("metadata.txt");

    let storage = QemuImgStorage::new();
    storage
        .write_file(&file_path, "INSTANCE_METADATA")
        .expect("write_file should succeed");

    assert!(file_path.exists());
    let content = storage
        .read_file(&file_path)
        .expect("read_file should succeed");
    assert_eq!(content, "INSTANCE_METADATA");

    storage
        .set_readonly(&file_path)
        .expect("set_readonly should succeed");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&file_path).expect("read metadata");
        let permissions_mode = metadata.permissions().mode() & 0o777;
        assert_eq!(permissions_mode, 0o444);
    }
}

#[test]
fn test_execute_compute_content_hash_streaming_and_deterministic_order() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let oemdrv_path = temp_dir.path().join("oemdrv.iso");
    let virtio_path = temp_dir.path().join("virtio.iso");
    let tools_path = temp_dir.path().join("tools.iso");

    fs::write(&oemdrv_path, b"OEMDRV_PAYLOAD_CONTENT_DATA").expect("Failed to write oemdrv");
    fs::write(&virtio_path, b"VIRTIO_DRIVER_DATA").expect("Failed to write virtio");
    fs::write(&tools_path, b"GUEST_TOOLS_DATA").expect("Failed to write tools");

    // Order 1: virtio then tools
    let mut extra_isos_1 = HashMap::new();
    extra_isos_1.insert("virtio".to_string(), virtio_path.clone());
    extra_isos_1.insert("tools".to_string(), tools_path.clone());

    // Order 2: tools then virtio
    let mut extra_isos_2 = HashMap::new();
    extra_isos_2.insert("tools".to_string(), tools_path.clone());
    extra_isos_2.insert("virtio".to_string(), virtio_path.clone());

    let hash_1 = execute_compute_content_hash(&oemdrv_path, &extra_isos_1)
        .expect("Content hash calculation should succeed");
    let hash_2 = execute_compute_content_hash(&oemdrv_path, &extra_isos_2)
        .expect("Content hash calculation should succeed");

    assert_eq!(
        hash_1.len(),
        12,
        "Content hash must be exactly 12 characters"
    );
    assert_eq!(
        hash_1, hash_2,
        "Hash calculation must be deterministic regardless of map insertion order"
    );

    // Verify changing content changes hash
    let modified_oemdrv_path = temp_dir.path().join("oemdrv_modified.iso");
    fs::write(&modified_oemdrv_path, b"DIFFERENT_OEMDRV_PAYLOAD")
        .expect("Failed to write modified oemdrv");
    let hash_3 = execute_compute_content_hash(&modified_oemdrv_path, &extra_isos_1)
        .expect("Content hash calculation should succeed");
    assert_ne!(
        hash_1, hash_3,
        "Different content must produce different hash"
    );

    // Test with QemuImgStorage delegates cleanly
    let qemu_storage = QemuImgStorage::new();
    let qemu_hash = qemu_storage
        .compute_content_hash(&oemdrv_path, &extra_isos_1)
        .expect("QemuImgStorage compute_content_hash should succeed");
    assert_eq!(hash_1, qemu_hash);
}

#[test]
fn test_qemu_img_storage_compute_content_hash_empty_extra_isos() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let oemdrv_path = temp_dir.path().join("oemdrv.iso");
    fs::write(&oemdrv_path, b"OEMDRV_STANDALONE_PAYLOAD").expect("Failed to write oemdrv");

    let extra_isos = HashMap::new();
    let storage = QemuImgStorage::new();
    let hash = storage
        .compute_content_hash(&oemdrv_path, &extra_isos)
        .expect("compute_content_hash should succeed");

    assert_eq!(hash.len(), 12);
}

#[test]
fn test_qemu_img_storage_source_file_not_found_errors() {
    let temp_workspace = tempdir().expect("Failed to create temporary workspace");
    let nonexistent_file = temp_workspace.path().join("nonexistent.qcow2");
    let destination_file = temp_workspace.path().join("destination.qcow2");

    let storage = QemuImgStorage::new();

    assert!(matches!(
        storage.inspect_image(&nonexistent_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.check_image(&nonexistent_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.create_cow_overlay(&nonexistent_file, &destination_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.rebase_overlay(&nonexistent_file, &destination_file, true),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.copy_base_image(&nonexistent_file, &destination_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.convert_thin_backup(&nonexistent_file, &destination_file, None, false),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.restore_thin_backup(&nonexistent_file, &destination_file, None),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.initialize_nvram(&nonexistent_file, &destination_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.read_file(&nonexistent_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
    assert!(matches!(
        storage.set_readonly(&nonexistent_file),
        Err(StorageError::SourceFileNotFound { .. })
    ));
}
