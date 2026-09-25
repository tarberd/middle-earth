use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;

use crate::storage::traits::{ImageInspectionInfo, StorageError, StorageManager};

/// Production implementation of StorageManager backed by the `qemu-img` command-line utility.
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
    let parsed: RawQemuImgInspection = serde_json::from_str(raw_json).map_err(|json_err| {
        StorageError::InspectionParseError {
            details: format!("Failed to parse qemu-img JSON: {json_err}"),
        }
    })?;

    Ok(ImageInspectionInfo {
        format: parsed.format,
        virtual_size_bytes: parsed.virtual_size,
        actual_size_bytes: parsed.actual_size.unwrap_or(0),
        backing_file: parsed.backing_filename.map(PathBuf::from),
    })
}

impl StorageManager for QemuImgStorage {
    fn create_cow_overlay(
        &self,
        backing_file_path: &Path,
        overlay_path: &Path,
    ) -> Result<(), StorageError> {
        if !backing_file_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: backing_file_path.to_path_buf(),
            });
        }

        let backing_file_str = backing_file_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Backing file path contains invalid UTF-8".to_string(),
            }
        })?;

        let overlay_str = overlay_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Overlay file path contains invalid UTF-8".to_string(),
            }
        })?;

        self.execute_command(&[
            "create",
            "-f",
            "qcow2",
            "-F",
            "qcow2",
            "-b",
            backing_file_str,
            overlay_str,
        ])?;

        Ok(())
    }

    fn rebase_overlay(
        &self,
        overlay_path: &Path,
        new_backing_file_path: &Path,
        unsafe_mode: bool,
    ) -> Result<(), StorageError> {
        if !overlay_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: overlay_path.to_path_buf(),
            });
        }

        let backing_file_str = new_backing_file_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "New backing file path contains invalid UTF-8".to_string(),
            }
        })?;

        let overlay_str = overlay_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Overlay path contains invalid UTF-8".to_string(),
            }
        })?;

        let mut arguments = vec!["rebase"];
        if unsafe_mode {
            arguments.push("-u");
        }
        arguments.extend_from_slice(&["-b", backing_file_str, "-F", "qcow2", overlay_str]);

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn inspect_image(&self, image_path: &Path) -> Result<ImageInspectionInfo, StorageError> {
        if !image_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            });
        }

        let image_str = image_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Image path contains invalid UTF-8".to_string(),
            }
        })?;

        let raw_json = self.execute_command(&["info", "--output=json", image_str])?;
        parse_qemu_img_info_json(&raw_json)
    }

    fn check_image(&self, image_path: &Path) -> Result<(), StorageError> {
        if !image_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            });
        }

        let image_str = image_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Image path contains invalid UTF-8".to_string(),
            }
        })?;

        if let Err(command_failure) = self.execute_command(&["check", image_str]) {
            return Err(StorageError::ImageCorruptionDetected {
                path: image_path.to_path_buf(),
                details: command_failure.to_string(),
            });
        }

        Ok(())
    }

    fn copy_base_image(
        &self,
        source_depot_path: &Path,
        destination_pool_path: &Path,
    ) -> Result<(), StorageError> {
        if !source_depot_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: source_depot_path.to_path_buf(),
            });
        }

        if let Some(destination_parent) = destination_pool_path.parent() {
            std::fs::create_dir_all(destination_parent)?;
        }

        std::fs::copy(source_depot_path, destination_pool_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let read_only_permissions = std::fs::Permissions::from_mode(0o444);
            std::fs::set_permissions(destination_pool_path, read_only_permissions)?;
        }

        Ok(())
    }

    fn convert_thin_backup(
        &self,
        source_disk_path: &Path,
        destination_archive_path: &Path,
        backing_file_path: Option<&Path>,
        compress: bool,
    ) -> Result<(), StorageError> {
        if !source_disk_path.exists() {
            return Err(StorageError::SourceFileNotFound {
                path: source_disk_path.to_path_buf(),
            });
        }

        let source_str = source_disk_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Source disk path contains invalid UTF-8".to_string(),
            }
        })?;

        let destination_str = destination_archive_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Destination archive path contains invalid UTF-8".to_string(),
            }
        })?;

        let mut arguments = vec!["convert", "-U", "-O", "qcow2"];
        if compress {
            arguments.push("-c");
        }

        let backing_str_holder;
        if let Some(backing_file) = backing_file_path {
            backing_str_holder = backing_file.to_str().ok_or_else(|| {
                StorageError::InspectionParseError {
                    details: "Backing file path contains invalid UTF-8".to_string(),
                }
            })?;
            arguments.extend_from_slice(&["-B", backing_str_holder, "-F", "qcow2"]);
        }

        arguments.push(source_str);
        arguments.push(destination_str);

        self.execute_command(&arguments)?;
        Ok(())
    }
}
