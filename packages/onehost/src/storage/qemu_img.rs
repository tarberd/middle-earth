use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
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

/// Concrete QEMU disk image storage manager backed by the `qemu-img` command-line utility.
#[derive(Debug, Clone)]
pub struct QemuImgStorage {
    pub qemu_img_executable_path: PathBuf,
}

impl Default for QemuImgStorage {
    fn default() -> Self {
        Self {
            qemu_img_executable_path: PathBuf::from("qemu-img"),
        }
    }
}

impl QemuImgStorage {
    /// Creates a QemuImgStorage with default executable path ("qemu-img").
    pub fn new() -> Self {
        Self::default()
    }

    /// Functional builder: sets custom qemu-img executable path.
    pub fn with_executable(self, executable: impl Into<PathBuf>) -> Self {
        Self {
            qemu_img_executable_path: executable.into(),
        }
    }

    fn execute_command(&self, arguments: &[&str]) -> Result<String, StorageError> {
        let output = Command::new(&self.qemu_img_executable_path)
            .args(arguments)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
            let command_line = format!(
                "{} {}",
                self.qemu_img_executable_path.display(),
                arguments.join(" ")
            );

            Err(StorageError::CommandExecutionFailed {
                command: command_line,
                exit_code: output.status.code(),
                stderr: stderr_text,
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawQemuImgInspection {
    format: String,
    #[serde(rename = "virtual-size")]
    virtual_size: u64,
    #[serde(rename = "actual-size")]
    actual_size: Option<u64>,
    #[serde(rename = "backing-filename")]
    backing_filename: Option<String>,
}

/// Pure parser: parses `qemu-img info --output=json` output into ImageInspectionInfo.
pub fn parse_qemu_img_info_json(raw_json: &str) -> Result<ImageInspectionInfo, StorageError> {
    let parsed: RawQemuImgInspection = serde_json::from_str(raw_json).map_err(|json_error| {
        StorageError::InspectionParseError {
            details: format!("Failed to parse qemu-img JSON: {json_error}"),
        }
    })?;

    Ok(ImageInspectionInfo {
        format: parsed.format,
        virtual_size_bytes: parsed.virtual_size,
        actual_size_bytes: parsed.actual_size.unwrap_or(0),
        backing_file: parsed.backing_filename.map(PathBuf::from),
    })
}

impl QemuImgStorage {
    /// Creates a QCOW2 copy-on-write overlay backed by `backing_file_path`.
    pub fn create_cow_overlay(
        &self,
        backing_file_path: &Path,
        overlay_path: &Path,
    ) -> Result<(), StorageError> {
        backing_file_path.exists().then_some(()).ok_or_else(|| {
            StorageError::SourceFileNotFound {
                path: backing_file_path.to_path_buf(),
            }
        })?;

        if let Some(destination_parent) = overlay_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        let rendered_backing_path =
            backing_file_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Backing file path contains invalid UTF-8".to_string(),
                })?;

        let rendered_overlay_path =
            overlay_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Overlay file path contains invalid UTF-8".to_string(),
                })?;

        self.execute_command(&[
            "create",
            "-f",
            "qcow2",
            "-F",
            "qcow2",
            "-b",
            rendered_backing_path,
            rendered_overlay_path,
        ])?;

        Ok(())
    }

    /// Rebases an existing overlay to a new backing file path.
    pub fn rebase_overlay(
        &self,
        overlay_path: &Path,
        new_backing_file_path: &Path,
        unsafe_mode: bool,
    ) -> Result<(), StorageError> {
        overlay_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: overlay_path.to_path_buf(),
            })?;

        let rendered_backing_path =
            new_backing_file_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "New backing file path contains invalid UTF-8".to_string(),
                })?;

        let rendered_overlay_path =
            overlay_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Overlay path contains invalid UTF-8".to_string(),
                })?;

        let unsafe_flag = unsafe_mode.then_some("-u");
        let arguments: Vec<&str> = ["rebase"]
            .into_iter()
            .chain(unsafe_flag)
            .chain([
                "-b",
                rendered_backing_path,
                "-F",
                "qcow2",
                rendered_overlay_path,
            ])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    /// Inspects an image file using `qemu-img info`.
    pub fn inspect_image(&self, image_path: &Path) -> Result<ImageInspectionInfo, StorageError> {
        image_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            })?;

        let rendered_image_path =
            image_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Image path contains invalid UTF-8".to_string(),
                })?;

        let raw_json = self.execute_command(&["info", "--output=json", rendered_image_path])?;
        parse_qemu_img_info_json(&raw_json)
    }

    /// Verifies image integrity using `qemu-img check`.
    pub fn check_image(&self, image_path: &Path) -> Result<(), StorageError> {
        image_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            })?;

        let rendered_image_path =
            image_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Image path contains invalid UTF-8".to_string(),
                })?;

        self.execute_command(&["check", rendered_image_path])
            .map(|_| ())
            .map_err(|command_failure| StorageError::ImageCorruptionDetected {
                path: image_path.to_path_buf(),
                details: command_failure.to_string(),
            })
    }

    /// Copies a golden master image from depot to storage pool with `0444` read-only permissions.
    pub fn copy_base_image(
        &self,
        source_depot_path: &Path,
        destination_pool_path: &Path,
    ) -> Result<(), StorageError> {
        source_depot_path.exists().then_some(()).ok_or_else(|| {
            StorageError::SourceFileNotFound {
                path: source_depot_path.to_path_buf(),
            }
        })?;

        if let Some(destination_parent) = destination_pool_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        fs::copy(source_depot_path, destination_pool_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let read_only_permissions = fs::Permissions::from_mode(0o444);
            fs::set_permissions(destination_pool_path, read_only_permissions)?;
        }

        Ok(())
    }

    /// Converts and compresses a frozen overlay disk to a standalone or thin backup archive.
    pub fn convert_thin_backup(
        &self,
        source_disk_path: &Path,
        destination_archive_path: &Path,
        backing_file_path: Option<&Path>,
        compress: bool,
    ) -> Result<(), StorageError> {
        source_disk_path.exists().then_some(()).ok_or_else(|| {
            StorageError::SourceFileNotFound {
                path: source_disk_path.to_path_buf(),
            }
        })?;

        if let Some(destination_parent) = destination_archive_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        let rendered_source_path =
            source_disk_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Source disk path contains invalid UTF-8".to_string(),
                })?;

        let rendered_destination_path = destination_archive_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Destination archive path contains invalid UTF-8".to_string(),
            }
        })?;

        let compress_flag = compress.then_some("-c");
        let rendered_backing_file = backing_file_path
            .map(|backing_file| {
                backing_file
                    .to_str()
                    .ok_or_else(|| StorageError::InspectionParseError {
                        details: "Backing file path contains invalid UTF-8".to_string(),
                    })
            })
            .transpose()?;

        let backing_flag_arguments =
            rendered_backing_file.map(|rendered_backing| ["-B", rendered_backing, "-F", "qcow2"]);

        let arguments: Vec<&str> = ["convert", "-U", "-O", "qcow2"]
            .into_iter()
            .chain(compress_flag)
            .chain(backing_flag_arguments.into_iter().flatten())
            .chain([rendered_source_path, rendered_destination_path])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    /// Restores a thin backup archive to a destination overlay disk backed by the designated base image.
    pub fn restore_thin_backup(
        &self,
        archive_path: &Path,
        destination_overlay_path: &Path,
        backing_file_path: Option<&Path>,
    ) -> Result<(), StorageError> {
        archive_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: archive_path.to_path_buf(),
            })?;

        if let Some(destination_parent) = destination_overlay_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        let rendered_archive_path =
            archive_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Archive path contains invalid UTF-8".to_string(),
                })?;

        let rendered_destination_path = destination_overlay_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Destination overlay path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_backing_file = backing_file_path
            .map(|backing_file| {
                backing_file
                    .to_str()
                    .ok_or_else(|| StorageError::InspectionParseError {
                        details: "Backing file path contains invalid UTF-8".to_string(),
                    })
            })
            .transpose()?;

        let backing_flag_arguments =
            rendered_backing_file.map(|rendered_backing| ["-B", rendered_backing, "-F", "qcow2"]);

        let arguments: Vec<&str> = ["convert", "-O", "qcow2"]
            .into_iter()
            .chain(backing_flag_arguments.into_iter().flatten())
            .chain([rendered_archive_path, rendered_destination_path])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    /// Safely moves a file, using `std::fs::rename` with fallback to streaming copy, fsync, and atomic rename.
    pub fn move_file_safely(
        &self,
        source_path: &Path,
        destination_path: &Path,
    ) -> Result<(), StorageError> {
        execute_safe_file_move(source_path, destination_path)
    }

    /// Initializes a fresh NVRAM vars file by copying from the template.
    pub fn initialize_nvram(
        &self,
        template_path: &Path,
        destination_nvram_path: &Path,
    ) -> Result<(), StorageError> {
        template_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: template_path.to_path_buf(),
            })?;

        if let Some(destination_parent) = destination_nvram_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        fs::copy(template_path, destination_nvram_path)?;
        Ok(())
    }

    /// Deletes an image file from storage if it exists.
    pub fn delete_image(&self, image_path: &Path) -> Result<(), StorageError> {
        if image_path.exists() {
            fs::remove_file(image_path)?;
        }
        Ok(())
    }

    /// Writes text content to a destination file, creating parent directories if needed.
    pub fn write_file(&self, destination_path: &Path, content: &str) -> Result<(), StorageError> {
        if let Some(destination_parent) = destination_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }
        fs::write(destination_path, content)?;
        Ok(())
    }

    /// Reads text content from a source file.
    pub fn read_file(&self, source_path: &Path) -> Result<String, StorageError> {
        source_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: source_path.to_path_buf(),
            })?;
        let content = fs::read_to_string(source_path)?;
        Ok(content)
    }

    /// Creates a fresh empty QCOW2 disk of the specified size in bytes.
    pub fn create_empty_disk(&self, disk_path: &Path, size_bytes: u64) -> Result<(), StorageError> {
        if let Some(destination_parent) = disk_path.parent() {
            fs::create_dir_all(destination_parent)?;
        }

        let rendered_disk_path =
            disk_path
                .to_str()
                .ok_or_else(|| StorageError::InspectionParseError {
                    details: "Disk path contains invalid UTF-8".to_string(),
                })?;

        let size_argument = size_bytes.to_string();

        self.execute_command(&["create", "-f", "qcow2", rendered_disk_path, &size_argument])?;

        Ok(())
    }

    /// Sets strict read-only permissions (0444) on a file.
    pub fn set_readonly(&self, file_path: &Path) -> Result<(), StorageError> {
        file_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: file_path.to_path_buf(),
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let read_only_permissions = fs::Permissions::from_mode(0o444);
            fs::set_permissions(file_path, read_only_permissions)?;
        }

        Ok(())
    }

    /// Computes the dynamic SHA-256 Content Hash across OEMDRV and alphabetically sorted extra ISOs,
    /// returning the first 12 hexadecimal characters.
    pub fn compute_content_hash(
        &self,
        oemdrv_path: &Path,
        extra_isos: &std::collections::HashMap<String, PathBuf>,
    ) -> Result<String, StorageError> {
        execute_compute_content_hash(oemdrv_path, extra_isos)
    }
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

/// Computes a streaming SHA-256 Content Hash across OEMDRV and alphabetically sorted extra ISOs,
/// returning the first 12 hexadecimal characters.
pub fn execute_compute_content_hash(
    oemdrv_path: &Path,
    extra_isos: &std::collections::HashMap<String, PathBuf>,
) -> Result<String, StorageError> {
    use sha2::{Digest, Sha256};

    let hasher = stream_file_to_hasher(Sha256::new(), oemdrv_path)?;

    let sorted_isos: std::collections::BTreeMap<&str, &PathBuf> = extra_isos
        .iter()
        .map(|(key, path)| (key.as_str(), path))
        .collect();

    let final_hasher = sorted_isos
        .values()
        .try_fold(hasher, |accumulated_hasher, iso_path| {
            stream_file_to_hasher(accumulated_hasher, iso_path)
        })?;

    let full_hex = format!("{:x}", Digest::finalize(final_hasher));
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
