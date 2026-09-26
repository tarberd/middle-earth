use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::storage::traits::{ImageInspectionInfo, StorageError, StorageManager};

/// Records an individual storage management action for assertion in test suites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedStorageAction {
    CreateCowOverlay {
        backing_file_path: PathBuf,
        overlay_path: PathBuf,
    },
    RebaseOverlay {
        overlay_path: PathBuf,
        new_backing_file_path: PathBuf,
        unsafe_mode: bool,
    },
    InspectImage {
        image_path: PathBuf,
    },
    CheckImage {
        image_path: PathBuf,
    },
    CopyBaseImage {
        source_depot_path: PathBuf,
        destination_pool_path: PathBuf,
    },
    MoveFileSafely {
        source_path: PathBuf,
        destination_path: PathBuf,
    },
    ConvertThinBackup {
        source_disk_path: PathBuf,
        destination_archive_path: PathBuf,
        backing_file_path: Option<PathBuf>,
        compress: bool,
    },
    RestoreThinBackup {
        archive_path: PathBuf,
        destination_overlay_path: PathBuf,
        backing_file_path: Option<PathBuf>,
    },
    InitializeNvram {
        template_path: PathBuf,
        destination_nvram_path: PathBuf,
    },
    DeleteImage {
        image_path: PathBuf,
    },
    WriteFile {
        destination_path: PathBuf,
        content: String,
    },
    ReadFile {
        source_path: PathBuf,
    },
    CreateEmptyDisk {
        disk_path: PathBuf,
        size_bytes: u64,
    },
    SetReadonly {
        file_path: PathBuf,
    },
}

/// In-memory representation of an image tracked by MockStorageManager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockImageRecord {
    pub format: String,
    pub virtual_size_bytes: u64,
    pub actual_size_bytes: u64,
    pub backing_file: Option<PathBuf>,
    pub is_corrupted: bool,
}

impl Default for MockImageRecord {
    fn default() -> Self {
        Self {
            format: "qcow2".to_string(),
            virtual_size_bytes: 68_719_476_736, // 64 GiB default
            actual_size_bytes: 196_608,         // 192 KiB initial metadata
            backing_file: None,
            is_corrupted: false,
        }
    }
}

/// Internal in-memory state tracked by MockStorageManager.
#[derive(Debug, Default, Clone)]
pub struct MockStorageManagerState {
    pub images: HashMap<PathBuf, MockImageRecord>,
    pub recorded_actions: Vec<RecordedStorageAction>,
    pub simulate_cross_device_paths: HashSet<PathBuf>,
    pub copied_base_images: Vec<(PathBuf, PathBuf)>,
    pub initialized_nvrams: Vec<(PathBuf, PathBuf)>,
    pub injected_errors: HashMap<PathBuf, String>,
    pub files: HashMap<PathBuf, String>,
}

/// In-memory mock implementing the StorageManager trait for hermetic testing.
#[derive(Debug, Default, Clone)]
pub struct MockStorageManager {
    pub state: Arc<Mutex<MockStorageManagerState>>,
}

impl MockStorageManager {
    /// Creates an empty MockStorageManager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Functional builder: registers a virtual file with content at path.
    pub fn with_file(self, path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.files.insert(path.into(), content.into());
        }
        self
    }

    /// Functional builder: registers an image record at path.
    pub fn with_image(self, path: impl Into<PathBuf>, record: MockImageRecord) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.images.insert(path.into(), record);
        }
        self
    }

    /// Functional builder: marks a path to simulate cross-device filesystem boundary (EXDEV).
    pub fn with_simulated_cross_device_path(self, path: impl Into<PathBuf>) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.simulate_cross_device_paths.insert(path.into());
        }
        self
    }

    /// Functional builder: injects an error for a path.
    pub fn with_injected_error(
        self,
        path: impl Into<PathBuf>,
        error_details: impl Into<String>,
    ) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .injected_errors
                .insert(path.into(), error_details.into());
        }
        self
    }

    /// Returns a copy of all recorded actions executed on this mock.
    pub fn recorded_actions(&self) -> Vec<RecordedStorageAction> {
        self.state
            .lock()
            .map(|locked_state| locked_state.recorded_actions.clone())
            .unwrap_or_default()
    }

    /// Returns recorded NVRAM initializations.
    pub fn initialized_nvrams(&self) -> Vec<(PathBuf, PathBuf)> {
        self.state
            .lock()
            .map(|locked_state| locked_state.initialized_nvrams.clone())
            .unwrap_or_default()
    }
}

impl StorageManager for MockStorageManager {
    fn create_cow_overlay(
        &self,
        backing_file_path: &Path,
        overlay_path: &Path,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::CreateCowOverlay {
                backing_file_path: backing_file_path.to_path_buf(),
                overlay_path: overlay_path.to_path_buf(),
            });

        if let Some(error_details) = locked_state.injected_errors.get(overlay_path) {
            Err(StorageError::CommandExecutionFailed {
                command: format!("qemu-img create -f qcow2 {}", overlay_path.display()),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let record = MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 196_608,
                backing_file: Some(backing_file_path.to_path_buf()),
                is_corrupted: false,
            };

            locked_state.images.insert(overlay_path.to_path_buf(), record);
            Ok(())
        }
    }

    fn rebase_overlay(
        &self,
        overlay_path: &Path,
        new_backing_file_path: &Path,
        unsafe_mode: bool,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::RebaseOverlay {
                overlay_path: overlay_path.to_path_buf(),
                new_backing_file_path: new_backing_file_path.to_path_buf(),
                unsafe_mode,
            });

        if let Some(error_details) = locked_state.injected_errors.get(overlay_path) {
            Err(StorageError::CommandExecutionFailed {
                command: format!("qemu-img rebase {}", overlay_path.display()),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let image_record = locked_state
                .images
                .get_mut(overlay_path)
                .ok_or_else(|| StorageError::SourceFileNotFound {
                    path: overlay_path.to_path_buf(),
                })?;

            image_record.backing_file = Some(new_backing_file_path.to_path_buf());
            Ok(())
        }
    }

    fn inspect_image(&self, image_path: &Path) -> Result<ImageInspectionInfo, StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::InspectImage {
                image_path: image_path.to_path_buf(),
            });

        if let Some(error_details) = locked_state.injected_errors.get(image_path) {
            Err(StorageError::InspectionParseError {
                details: error_details.clone(),
            })
        } else {
            let image_record = locked_state
                .images
                .get(image_path)
                .ok_or_else(|| StorageError::SourceFileNotFound {
                    path: image_path.to_path_buf(),
                })?;

            Ok(ImageInspectionInfo {
                format: image_record.format.clone(),
                virtual_size_bytes: image_record.virtual_size_bytes,
                actual_size_bytes: image_record.actual_size_bytes,
                backing_file: image_record.backing_file.clone(),
            })
        }
    }

    fn check_image(&self, image_path: &Path) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::CheckImage {
                image_path: image_path.to_path_buf(),
            });

        let image_record = locked_state
            .images
            .get(image_path)
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: image_path.to_path_buf(),
            })?;

        if let Some(error_details) = locked_state.injected_errors.get(image_path) {
            Err(StorageError::ImageCorruptionDetected {
                path: image_path.to_path_buf(),
                details: error_details.clone(),
            })
        } else if image_record.is_corrupted {
            Err(StorageError::ImageCorruptionDetected {
                path: image_path.to_path_buf(),
                details: "Simulated corruption in disk image".to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn copy_base_image(
        &self,
        source_depot_path: &Path,
        destination_pool_path: &Path,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::CopyBaseImage {
                source_depot_path: source_depot_path.to_path_buf(),
                destination_pool_path: destination_pool_path.to_path_buf(),
            });

        locked_state
            .copied_base_images
            .push((source_depot_path.to_path_buf(), destination_pool_path.to_path_buf()));

        if let Some(source_record) = locked_state.images.get(source_depot_path).cloned() {
            locked_state
                .images
                .insert(destination_pool_path.to_path_buf(), source_record);
        }

        Ok(())
    }

    fn move_file_safely(
        &self,
        source_path: &Path,
        destination_path: &Path,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::MoveFileSafely {
                source_path: source_path.to_path_buf(),
                destination_path: destination_path.to_path_buf(),
            });

        let maybe_moved_record = locked_state.images.remove(source_path);
        if let Some(moved_record) = maybe_moved_record {
            locked_state
                .images
                .insert(destination_path.to_path_buf(), moved_record);
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
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::ConvertThinBackup {
                source_disk_path: source_disk_path.to_path_buf(),
                destination_archive_path: destination_archive_path.to_path_buf(),
                backing_file_path: backing_file_path.map(Path::to_path_buf),
                compress,
            });

        if let Some(error_details) = locked_state.injected_errors.get(source_disk_path) {
            Err(StorageError::CommandExecutionFailed {
                command: format!("qemu-img convert {}", source_disk_path.display()),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let archive_record = MockImageRecord {
                format: "qcow2".to_string(),
                virtual_size_bytes: 68_719_476_736,
                actual_size_bytes: 50_000,
                backing_file: backing_file_path.map(Path::to_path_buf),
                is_corrupted: false,
            };

            locked_state
                .images
                .insert(destination_archive_path.to_path_buf(), archive_record);
            Ok(())
        }
    }

    fn restore_thin_backup(
        &self,
        archive_path: &Path,
        destination_overlay_path: &Path,
        backing_file_path: Option<&Path>,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::RestoreThinBackup {
                archive_path: archive_path.to_path_buf(),
                destination_overlay_path: destination_overlay_path.to_path_buf(),
                backing_file_path: backing_file_path.map(Path::to_path_buf),
            });

        let restored_record = MockImageRecord {
            format: "qcow2".to_string(),
            virtual_size_bytes: 68_719_476_736,
            actual_size_bytes: 196_608,
            backing_file: backing_file_path.map(Path::to_path_buf),
            is_corrupted: false,
        };

        locked_state
            .images
            .insert(destination_overlay_path.to_path_buf(), restored_record);
        Ok(())
    }

    fn initialize_nvram(
        &self,
        template_path: &Path,
        destination_nvram_path: &Path,
    ) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::InitializeNvram {
                template_path: template_path.to_path_buf(),
                destination_nvram_path: destination_nvram_path.to_path_buf(),
            });

        locked_state
            .initialized_nvrams
            .push((template_path.to_path_buf(), destination_nvram_path.to_path_buf()));

        Ok(())
    }

    fn delete_image(&self, image_path: &Path) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::DeleteImage {
                image_path: image_path.to_path_buf(),
            });

        locked_state.images.remove(image_path);
        Ok(())
    }

    fn write_file(&self, destination_path: &Path, content: &str) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::WriteFile {
                destination_path: destination_path.to_path_buf(),
                content: content.to_string(),
            });

        locked_state
            .files
            .insert(destination_path.to_path_buf(), content.to_string());
        Ok(())
    }

    fn read_file(&self, source_path: &Path) -> Result<String, StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::ReadFile {
                source_path: source_path.to_path_buf(),
            });

        locked_state
            .files
            .get(source_path)
            .cloned()
            .ok_or_else(|| StorageError::SourceFileNotFound {
                path: source_path.to_path_buf(),
            })
    }

    fn create_empty_disk(&self, disk_path: &Path, size_bytes: u64) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::CreateEmptyDisk {
                disk_path: disk_path.to_path_buf(),
                size_bytes,
            });

        let image_record = MockImageRecord {
            format: "qcow2".to_string(),
            virtual_size_bytes: size_bytes,
            actual_size_bytes: 196_608,
            backing_file: None,
            is_corrupted: false,
        };

        locked_state
            .images
            .insert(disk_path.to_path_buf(), image_record);

        Ok(())
    }

    fn set_readonly(&self, file_path: &Path) -> Result<(), StorageError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| std::io::Error::other(poison_error.to_string()))?;

        locked_state
            .recorded_actions
            .push(RecordedStorageAction::SetReadonly {
                file_path: file_path.to_path_buf(),
            });

        Ok(())
    }
}

