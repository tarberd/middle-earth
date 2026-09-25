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
    ImageCorruptionDetected {
        path: PathBuf,
        details: String,
    },

    #[error("I/O error during storage operation: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
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

    let destination_parent = destination_path.parent().ok_or_else(|| {
        StorageError::DestinationDirectoryNotFound {
            path: destination_path.to_path_buf(),
        }
    })?;

    destination_parent
        .exists()
        .then_some(())
        .ok_or_else(|| StorageError::DestinationDirectoryNotFound {
            path: destination_parent.to_path_buf(),
        })?;

    let file_name = destination_path
        .file_name()
        .and_then(|raw_name| raw_name.to_str())
        .unwrap_or("file");

    let temporary_staging_path = destination_parent.join(format!(".{file_name}.transfer_{}", std::process::id()));

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
                let is_cross_device = error.kind() == io::ErrorKind::CrossesDevices
                    || error.raw_os_error() == Some(18); // 18 is EXDEV on Linux

                if is_cross_device {
                    execute_cross_device_streaming_move(source_path, destination_path)
                } else {
                    Err(StorageError::IoError { source: error })
                }
            }
        }
    }

    /// Converts and compresses a frozen overlay disk to a standalone or thin backup archive.
    fn convert_thin_backup(
        &self,
        source_disk_path: &Path,
        destination_archive_path: &Path,
        backing_file_path: Option<&Path>,
        compress: bool,
    ) -> Result<(), StorageError>;

    /// Deletes an image file from storage if it exists.
    fn delete_image(&self, image_path: &Path) -> Result<(), StorageError> {
        if image_path.exists() {
            fs::remove_file(image_path)?;
        }
        Ok(())
    }
}
