use std::fs::File;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

use onehost::storage::qemu_img::QemuImgStorage;
use onehost::storage::traits::{StorageError, StorageManager};

fn create_real_qcow2_image(path: &std::path::Path, size_megabytes: u64) {
    let size_argument = format!("{size_megabytes}M");
    let output = Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            path.to_str().expect("valid utf8 path"),
            &size_argument,
        ])
        .output()
        .expect("qemu-img must be available in test environment");

    assert!(
        output.status.success(),
        "failed to create test qcow2 image: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_real_qemu_img_inspect_base_image() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let base_image_path = temporary_directory.path().join("golden_master.qcow2");

    create_real_qcow2_image(&base_image_path, 10);

    let storage = QemuImgStorage::new();
    let inspection_info = storage
        .inspect_image(&base_image_path)
        .expect("inspection of real qcow2 image must succeed");

    assert_eq!(inspection_info.format, "qcow2");
    assert_eq!(inspection_info.virtual_size_bytes, 10 * 1024 * 1024);
    assert_eq!(inspection_info.backing_file, None);

    let check_result = storage.check_image(&base_image_path);
    assert!(check_result.is_ok());
}

#[test]
fn test_real_qemu_img_create_overlay_and_inspect_backing_file() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let base_image_path = temporary_directory.path().join("base.qcow2");
    let overlay_disk_path = temporary_directory.path().join("instance_overlay.qcow2");

    create_real_qcow2_image(&base_image_path, 20);

    let storage = QemuImgStorage::new();
    storage
        .create_cow_overlay(&base_image_path, &overlay_disk_path)
        .expect("overlay creation via real qemu-img must succeed");

    let overlay_inspection = storage
        .inspect_image(&overlay_disk_path)
        .expect("inspection of overlay image must succeed");

    assert_eq!(overlay_inspection.format, "qcow2");
    assert_eq!(overlay_inspection.virtual_size_bytes, 20 * 1024 * 1024);
    assert_eq!(
        overlay_inspection.backing_file,
        Some(base_image_path.clone())
    );

    let check_result = storage.check_image(&overlay_disk_path);
    assert!(check_result.is_ok());
}

#[test]
fn test_real_qemu_img_copy_base_image_and_rebase_overlay() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let depot_directory = temporary_directory.path().join("depot");
    let pool_directory = temporary_directory.path().join("pool");
    std::fs::create_dir_all(&depot_directory).expect("create depot dir");
    std::fs::create_dir_all(&pool_directory).expect("create pool dir");

    let original_master_path = depot_directory.join("master.qcow2");
    let cached_base_path = pool_directory.join("cached_base.qcow2");
    let overlay_disk_path = pool_directory.join("instance.qcow2");

    create_real_qcow2_image(&original_master_path, 15);

    let storage = QemuImgStorage::new();

    // 1. Copy base image to local pool cache and verify 0444 read-only permissions
    storage
        .copy_base_image(&original_master_path, &cached_base_path)
        .expect("copying base image must succeed");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&cached_base_path).expect("read metadata");
        let permissions_mode = metadata.permissions().mode() & 0o777;
        assert_eq!(permissions_mode, 0o444);
    }

    // 2. Create overlay pointing to depot master
    storage
        .create_cow_overlay(&original_master_path, &overlay_disk_path)
        .expect("create overlay pointing to depot master");

    let initial_inspection = storage
        .inspect_image(&overlay_disk_path)
        .expect("inspect initial overlay");
    assert_eq!(
        initial_inspection.backing_file,
        Some(original_master_path.clone())
    );

    // 3. Unsafe rebase pointing to local pool cached base
    storage
        .rebase_overlay(&overlay_disk_path, &cached_base_path, true)
        .expect("metadata rebase must succeed");

    let rebased_inspection = storage
        .inspect_image(&overlay_disk_path)
        .expect("inspect rebased overlay");
    assert_eq!(
        rebased_inspection.backing_file,
        Some(cached_base_path.clone())
    );
}

#[test]
fn test_real_qemu_img_convert_thin_backup_compressed() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let base_image_path = temporary_directory.path().join("base.qcow2");
    let overlay_disk_path = temporary_directory.path().join("instance.qcow2");
    let backup_archive_path = temporary_directory.path().join("backup_disk.qcow2");

    create_real_qcow2_image(&base_image_path, 10);

    let storage = QemuImgStorage::new();
    storage
        .create_cow_overlay(&base_image_path, &overlay_disk_path)
        .expect("create overlay");

    // Convert thin backup with compression and backing file reference
    storage
        .convert_thin_backup(
            &overlay_disk_path,
            &backup_archive_path,
            Some(&base_image_path),
            true,
        )
        .expect("convert thin backup must succeed");

    let backup_inspection = storage
        .inspect_image(&backup_archive_path)
        .expect("inspect backup archive");

    assert_eq!(backup_inspection.format, "qcow2");
    assert_eq!(backup_inspection.virtual_size_bytes, 10 * 1024 * 1024);
    assert_eq!(backup_inspection.backing_file, Some(base_image_path));
}

#[test]
fn test_real_qemu_img_check_image_detects_corruption() {
    let temporary_directory = tempdir().expect("temporary directory creation succeeds");
    let corrupt_image_path = temporary_directory.path().join("corrupted.qcow2");

    create_real_qcow2_image(&corrupt_image_path, 10);

    // Corrupt the QCOW2 image by overwriting internal header bytes
    let mut file = File::options()
        .write(true)
        .open(&corrupt_image_path)
        .expect("open file for corruption");
    let corrupt_payload = [0xFFu8; 1024];
    file.write_all(&corrupt_payload)
        .expect("write corruption bytes");
    drop(file);

    let storage = QemuImgStorage::new();
    let check_result = storage.check_image(&corrupt_image_path);

    assert!(check_result.is_err());
    match check_result.unwrap_err() {
        StorageError::ImageCorruptionDetected { path, details } => {
            assert_eq!(path, corrupt_image_path);
            assert!(!details.is_empty());
        }
        other_error => panic!("expected ImageCorruptionDetected, but got: {:?}", other_error),
    }
}
