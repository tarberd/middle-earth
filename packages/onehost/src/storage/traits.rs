use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Structured inspection information for a disk image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInspectionInfo {
    pub format: String,
    pub virtual_size_bytes: u64,
    pub actual_size_bytes: u64,
    pub backing_file: Option<PathBuf>,
}

/// Enumerates errors that can occur during storage management operations.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Source file does not exist at '{path}'")]
    SourceFileNotFound { path: PathBuf },

    #[error("Destination parent directory does not exist at '{path}'")]
    DestinationDirectoryNotFound { path: PathBuf },

    #[error("Failed to parse qemu-img JSON output: {details}")]
    InspectionParseError { details: String },

    #[error("qemu-img command '{command}' failed with exit code {exit_code:?}: {stderr}")]
    CommandExecutionFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Image integrity check failed for '{path}': {details}")]
    ImageCorruptionDetected { path: PathBuf, details: String },

    #[error("I/O error during storage operation: {source}")]
    IoError {
        #[from]
        source: io::Error,
    },
}

/// Performs safe cross-device file move via streaming copy, fsync, atomic rename, and source removal.
pub fn execute_cross_device_streaming_move(
    source_path: &Path,
    destination_path: &Path,
) -> Result<(), StorageError> {
    source_path
        .exists()
        .then_some(())
        .ok_or_else(|| StorageError::SourceFileNotFound {
            path: source_path.to_path_buf(),
        })?;

    let destination_parent =
        destination_path
            .parent()
            .ok_or_else(|| StorageError::DestinationDirectoryNotFound {
                path: destination_path.to_path_buf(),
            })?;

    destination_parent.exists().then_some(()).ok_or_else(|| {
        StorageError::DestinationDirectoryNotFound {
            path: destination_parent.to_path_buf(),
        }
    })?;

    let file_name = destination_path
        .file_name()
        .and_then(|raw_name| raw_name.to_str())
        .unwrap_or("file");

    let temporary_staging_path =
        destination_parent.join(format!(".{file_name}.transfer_{}", std::process::id()));

    // Streaming copy from source to temporary staging file
    let mut source_file = File::open(source_path)?;
    let mut staging_file = File::create(&temporary_staging_path)?;

    io::copy(&mut source_file, &mut staging_file)?;

    // Ensure all bytes and metadata hit physical storage
    staging_file.sync_all()?;
    drop(staging_file);
    drop(source_file);

    // Atomically rename staging file to final destination on the same target filesystem
    fs::rename(&temporary_staging_path, destination_path)?;

    // Safely remove the source file
    fs::remove_file(source_path)?;

    Ok(())
}

/// Safely moves a file, using `std::fs::rename` with fallback to streaming copy, fsync, and atomic rename
/// when encountering cross-device filesystem boundary (`EXDEV`).
pub fn execute_safe_file_move(
    source_path: &Path,
    destination_path: &Path,
) -> Result<(), StorageError> {
    source_path
        .exists()
        .then_some(())
        .ok_or_else(|| StorageError::SourceFileNotFound {
            path: source_path.to_path_buf(),
        })?;

    match fs::rename(source_path, destination_path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let is_cross_device =
                error.kind() == io::ErrorKind::CrossesDevices || error.raw_os_error() == Some(18); // 18 is EXDEV on Linux

            if is_cross_device {
                execute_cross_device_streaming_move(source_path, destination_path)
            } else {
                Err(StorageError::IoError { source: error })
            }
        }
    }
}

/// Trait abstracting image manipulation and storage management.
pub trait StorageManager: Send + Sync {
    /// Creates a QCOW2 copy-on-write overlay backed by `backing_file_path`.
    fn create_cow_overlay(
        &self,
        backing_file_path: &Path,
        overlay_path: &Path,
    ) -> Result<(), StorageError>;

    /// Rebases an existing overlay to a new backing file path.
    /// If `unsafe_mode` is true, performs fast metadata-only rebase (`-u`), essential for pool relocation.
    fn rebase_overlay(
        &self,
        overlay_path: &Path,
        new_backing_file_path: &Path,
        unsafe_mode: bool,
    ) -> Result<(), StorageError>;

    /// Inspects an image file using `qemu-img info`.
    fn inspect_image(&self, image_path: &Path) -> Result<ImageInspectionInfo, StorageError>;

    /// Verifies image integrity using `qemu-img check`.
    fn check_image(&self, image_path: &Path) -> Result<(), StorageError>;

    /// Copies a golden master image from depot to storage pool with `0444` read-only permissions.
    fn copy_base_image(
        &self,
        source_depot_path: &Path,
        destination_pool_path: &Path,
    ) -> Result<(), StorageError>;

    /// Safely moves a file, using `std::fs::rename` with fallback to streaming copy, fsync, and atomic rename
    /// when encountering cross-device filesystem boundary (`EXDEV`).
    fn move_file_safely(
        &self,
        source_path: &Path,
        destination_path: &Path,
    ) -> Result<(), StorageError>;

    /// Converts and compresses a frozen overlay disk to a standalone or thin backup archive.
    fn convert_thin_backup(
        &self,
        source_disk_path: &Path,
        destination_archive_path: &Path,
        backing_file_path: Option<&Path>,
        compress: bool,
    ) -> Result<(), StorageError>;

    /// Restores a thin backup archive to a destination overlay disk backed by the designated base image.
    fn restore_thin_backup(
        &self,
        archive_path: &Path,
        destination_overlay_path: &Path,
        backing_file_path: Option<&Path>,
    ) -> Result<(), StorageError>;

    /// Initializes a fresh NVRAM vars file by copying from the template.
    fn initialize_nvram(
        &self,
        template_path: &Path,
        destination_nvram_path: &Path,
    ) -> Result<(), StorageError>;

    /// Deletes an image file from storage if it exists.
    fn delete_image(&self, image_path: &Path) -> Result<(), StorageError>;

    /// Writes text content to a destination file, creating parent directories if needed.
    fn write_file(&self, destination_path: &Path, content: &str) -> Result<(), StorageError>;

    /// Reads text content from a source file.
    fn read_file(&self, source_path: &Path) -> Result<String, StorageError>;

    /// Creates a fresh empty QCOW2 disk of the specified size in bytes.
    fn create_empty_disk(&self, disk_path: &Path, size_bytes: u64) -> Result<(), StorageError>;

    /// Sets strict read-only permissions (0444) on a file.
    fn set_readonly(&self, file_path: &Path) -> Result<(), StorageError>;

    /// Computes the dynamic SHA-256 Content Hash across OEMDRV and alphabetically sorted extra ISOs,
    /// returning the first 12 hexadecimal characters.
    fn compute_content_hash(
        &self,
        oemdrv_path: &Path,
        extra_isos: &std::collections::HashMap<String, PathBuf>,
    ) -> Result<String, StorageError>;
}

/// Computes a streaming SHA-256 Content Hash across OEMDRV and alphabetically sorted extra ISOs,
/// returning the first 12 hexadecimal characters.
pub fn execute_compute_content_hash(
    oemdrv_path: &Path,
    extra_isos: &std::collections::HashMap<String, PathBuf>,
) -> Result<String, StorageError> {
    use sha2::{Digest, Sha256};

    let initial_hasher = stream_file_to_hasher(Sha256::new(), oemdrv_path)?;

    let sorted_isos: std::collections::BTreeMap<&str, &PathBuf> = extra_isos
        .iter()
        .map(|(key, path)| (key.as_str(), path))
        .collect();

    let final_hasher = sorted_isos
        .into_values()
        .try_fold(initial_hasher, |hasher, iso_path| {
            stream_file_to_hasher(hasher, iso_path)
        })?;

    let full_hex = format!("{:x}", final_hasher.finalize());
    Ok(full_hex[..12].to_string())
}

fn stream_file_to_hasher(
    mut hasher: sha2::Sha256,
    path: &Path,
) -> Result<sha2::Sha256, StorageError> {
    path.exists()
        .then_some(())
        .ok_or_else(|| StorageError::SourceFileNotFound {
            path: path.to_path_buf(),
        })?;

    let mut file = File::open(path)?;
    io::copy(&mut file, &mut hasher)?;
    Ok(hasher)
}
