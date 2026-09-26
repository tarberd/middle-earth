use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;

use crate::storage::traits::{
    execute_safe_file_move, ImageInspectionInfo, StorageError, StorageManager,
};

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

impl StorageManager for QemuImgStorage {
    fn create_cow_overlay(
        &self,
        backing_file_path: &Path,
        overlay_path: &Path,
    ) -> Result<(), StorageError> {
        backing_file_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: backing_file_path.to_path_buf(),
            })?;

        if let Some(destination_parent) = overlay_path.parent() {
            std::fs::create_dir_all(destination_parent)?;
        }

        let rendered_backing_path = backing_file_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Backing file path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_overlay_path = overlay_path.to_str().ok_or_else(|| {
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
            rendered_backing_path,
            rendered_overlay_path,
        ])?;

        Ok(())
    }

    fn rebase_overlay(
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

        let rendered_backing_path = new_backing_file_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "New backing file path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_overlay_path = overlay_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Overlay path contains invalid UTF-8".to_string(),
            }
        })?;

        let unsafe_flag = unsafe_mode.then_some("-u");
        let arguments: Vec<&str> = ["rebase"]
            .into_iter()
            .chain(unsafe_flag)
            .chain(["-b", rendered_backing_path, "-F", "qcow2", rendered_overlay_path])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn inspect_image(&self, image_path: &Path) -> Result<ImageInspectionInfo, StorageError> {
        image_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            })?;

        let rendered_image_path = image_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Image path contains invalid UTF-8".to_string(),
            }
        })?;

        let raw_json = self.execute_command(&["info", "--output=json", rendered_image_path])?;
        parse_qemu_img_info_json(&raw_json)
    }

    fn check_image(&self, image_path: &Path) -> Result<(), StorageError> {
        image_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            })?;

        let rendered_image_path = image_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Image path contains invalid UTF-8".to_string(),
            }
        })?;

        self.execute_command(&["check", rendered_image_path])
            .map(|_| ())
            .map_err(|command_failure| StorageError::ImageCorruptionDetected {
                path: image_path.to_path_buf(),
                details: command_failure.to_string(),
            })
    }

    fn copy_base_image(
        &self,
        source_depot_path: &Path,
        destination_pool_path: &Path,
    ) -> Result<(), StorageError> {
        source_depot_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: source_depot_path.to_path_buf(),
            })?;

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
        source_disk_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: source_disk_path.to_path_buf(),
            })?;

        if let Some(destination_parent) = destination_archive_path.parent() {
            std::fs::create_dir_all(destination_parent)?;
        }

        let rendered_source_path = source_disk_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Source disk path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_destination_path = destination_archive_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Destination archive path contains invalid UTF-8".to_string(),
            }
        })?;

        let compress_flag = compress.then_some("-c");
        let rendered_backing_file = backing_file_path
            .map(|backing_file| {
                backing_file.to_str().ok_or_else(|| StorageError::InspectionParseError {
                    details: "Backing file path contains invalid UTF-8".to_string(),
                })
            })
            .transpose()?;

        let backing_flag_arguments = rendered_backing_file
            .map(|rendered_backing| ["-B", rendered_backing, "-F", "qcow2"]);

        let arguments: Vec<&str> = ["convert", "-U", "-O", "qcow2"]
            .into_iter()
            .chain(compress_flag)
            .chain(backing_flag_arguments.into_iter().flatten())
            .chain([rendered_source_path, rendered_destination_path])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn restore_thin_backup(
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
            std::fs::create_dir_all(destination_parent)?;
        }

        let rendered_archive_path = archive_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Archive path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_destination_path = destination_overlay_path.to_str().ok_or_else(|| {
            StorageError::InspectionParseError {
                details: "Destination overlay path contains invalid UTF-8".to_string(),
            }
        })?;

        let rendered_backing_file = backing_file_path
            .map(|backing_file| {
                backing_file.to_str().ok_or_else(|| StorageError::InspectionParseError {
                    details: "Backing file path contains invalid UTF-8".to_string(),
                })
            })
            .transpose()?;

        let backing_flag_arguments = rendered_backing_file
            .map(|rendered_backing| ["-B", rendered_backing, "-F", "qcow2"]);

        let arguments: Vec<&str> = ["convert", "-O", "qcow2"]
            .into_iter()
            .chain(backing_flag_arguments.into_iter().flatten())
            .chain([rendered_archive_path, rendered_destination_path])
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn move_file_safely(
        &self,
        source_path: &Path,
        destination_path: &Path,
    ) -> Result<(), StorageError> {
        execute_safe_file_move(source_path, destination_path)
    }

    fn initialize_nvram(
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
            std::fs::create_dir_all(destination_parent)?;
        }

        std::fs::copy(template_path, destination_nvram_path)?;
        Ok(())
    }

    fn delete_image(&self, image_path: &Path) -> Result<(), StorageError> {
        if image_path.exists() {
            std::fs::remove_file(image_path)?;
        }
        Ok(())
    }

    fn write_file(&self, destination_path: &Path, content: &str) -> Result<(), StorageError> {
        if let Some(destination_parent) = destination_path.parent() {
            std::fs::create_dir_all(destination_parent)?;
        }
        std::fs::write(destination_path, content)?;
        Ok(())
    }

    fn read_file(&self, source_path: &Path) -> Result<String, StorageError> {
        source_path
            .exists()
            .then_some(())
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: source_path.to_path_buf(),
            })?;
        let content = std::fs::read_to_string(source_path)?;
        Ok(content)
    }
}

