use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::config::model::OnehostManifest;
use crate::image::tag::{
    ContentAddressedImageResolver, FlavorDerivationMetadata, FlavorResolutionError,
    ImageTagParseError, ImageTagSpecification,
};
use crate::process::traits::{ProcessError, ProcessRunner};
use crate::storage::traits::{StorageError, StorageManager};

/// Configuration options controlling the image build process.
#[derive(Debug, Clone)]
pub struct ImageBuildOptions {
    pub force: bool,
    pub scratch_dir: Option<PathBuf>,
    pub iso_path: Option<PathBuf>,
    pub memory_mb: u32,
    pub cpu_count: u32,
}

impl Default for ImageBuildOptions {
    fn default() -> Self {
        Self {
            force: false,
            scratch_dir: None,
            iso_path: None,
            memory_mb: 8192,
            cpu_count: 8,
        }
    }
}

/// Outcome of an image build operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageBuildOutcome {
    AlreadyExists {
        golden_master_path: PathBuf,
    },
    Built {
        golden_master_path: PathBuf,
        virtual_size_bytes: u64,
        archive_size_bytes: u64,
    },
}

/// Enumerates errors that can occur during golden master image building.
#[derive(Debug, Error)]
pub enum ImageBuilderError {
    #[error("Image tag parsing error: {0}")]
    TagError(#[from] ImageTagParseError),

    #[error("Flavor resolution error: {0}")]
    FlavorError(#[from] FlavorResolutionError),

    #[error("Windows installation ISO not found in '{iso_directory}' for build version '{build_version}'")]
    IsoNotFound {
        iso_directory: PathBuf,
        build_version: String,
    },

    #[error("OEMDRV derivation path '{path}' does not exist on host filesystem")]
    OemdrvNotFound {
        path: PathBuf,
    },

    #[error("TPM emulator (swtpm) failed: {details}")]
    SwtpmError {
        details: String,
    },

    #[error("Headless QEMU installation VM failed with exit code {exit_code:?}: {stderr}")]
    QemuExecutionFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Integrity check failed on staged golden master '{path}': {details}")]
    PromotionIntegrityCheckFailed {
        path: PathBuf,
        details: String,
    },

    #[error("Storage error during image build: {0}")]
    StorageError(#[from] StorageError),

    #[error("Process execution error during image build: {0}")]
    ProcessError(#[from] ProcessError),

    #[error("I/O error during image build: {0}")]
    IoError(#[from] std::io::Error),
}

/// RAII scope guard ensuring ephemeral scratch build artifacts and temporary staging files
/// are cleaned up on failure, error, or panic.
struct BuildScratchGuard<'a, S: StorageManager> {
    storage: &'a S,
    scratch_disk_path: PathBuf,
    temporary_master_path: Option<PathBuf>,
    armed: bool,
}

impl<'a, S: StorageManager> BuildScratchGuard<'a, S> {
    fn new(storage: &'a S, scratch_disk_path: PathBuf) -> Self {
        Self {
            storage,
            scratch_disk_path,
            temporary_master_path: None,
            armed: true,
        }
    }

    fn set_temporary_master(&mut self, path: PathBuf) {
        self.temporary_master_path = Some(path);
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl<'a, S: StorageManager> Drop for BuildScratchGuard<'a, S> {
    fn drop(&mut self) {
        if self.armed {
            if let Some(ref temporary_master) = self.temporary_master_path
                && let Err(error) = self.storage.delete_image(temporary_master)
            {
                tracing::debug!(
                    path = %temporary_master.display(),
                    error = %error,
                    "Failed to delete temporary master during scratch cleanup guard drop"
                );
            }

            if let Err(error) = self.storage.delete_image(&self.scratch_disk_path) {
                tracing::debug!(
                    path = %self.scratch_disk_path.display(),
                    error = %error,
                    "Failed to delete scratch disk during scratch cleanup guard drop"
                );
            }
        }
    }
}

/// Orchestrates unattended installation of Windows golden master images using headless QEMU, swtpm, and OEMDRV media.
pub struct ImageBuilder<'a, S: StorageManager, P: ProcessRunner> {
    pub storage: &'a S,
    pub runner: &'a P,
}

impl<'a, S: StorageManager, P: ProcessRunner> ImageBuilder<'a, S, P> {
    /// Creates a new ImageBuilder instance.
    pub fn new(storage: &'a S, runner: &'a P) -> Self {
        Self { storage, runner }
    }

    /// Resolves the Windows installation ISO path.
    fn resolve_iso_path(
        &self,
        manifest: &OnehostManifest,
        image_tag: &ImageTagSpecification,
        options: &ImageBuildOptions,
    ) -> Result<PathBuf, ImageBuilderError> {
        if let Some(ref explicit_iso_path) = options.iso_path {
            let iso_exists = self.storage.inspect_image(explicit_iso_path).is_ok();
            (iso_exists)
                .then(|| explicit_iso_path.clone())
                .ok_or_else(|| ImageBuilderError::IsoNotFound {
                    iso_directory: explicit_iso_path.parent().unwrap_or(Path::new("")).to_path_buf(),
                    build_version: image_tag.build_version.clone(),
                })
        } else {
            let direct_iso_name = format!("win11-{}.iso", image_tag.build_version);
            let candidate_path = manifest.storage.depot_iso_dir.join(direct_iso_name);

            if self.storage.inspect_image(&candidate_path).is_ok() {
                Ok(candidate_path)
            } else {
                Err(ImageBuilderError::IsoNotFound {
                    iso_directory: manifest.storage.depot_iso_dir.clone(),
                    build_version: image_tag.build_version.clone(),
                })
            }
        }
    }

    /// Builds a content-addressed golden master image according to the specified image tag and options.
    pub fn build_image(
        &self,
        manifest: &OnehostManifest,
        image_tag: &ImageTagSpecification,
        options: &ImageBuildOptions,
    ) -> Result<ImageBuildOutcome, ImageBuilderError> {
        // 1. Resolve flavor derivation metadata and content hash
        let flavor_config = manifest
            .flavors
            .get(&image_tag.flavor_name)
            .ok_or_else(|| FlavorResolutionError::UnknownFlavor {
                requested_flavor: image_tag.flavor_name.clone(),
                available_flavors: manifest.flavors.keys().cloned().collect(),
            })?;

        let flavor_metadata = FlavorDerivationMetadata::new(
            flavor_config.oemdrv_path.clone(),
            flavor_config.hash.clone(),
        )?;

        let golden_master_filename = ContentAddressedImageResolver::resolve_golden_master_filename(
            image_tag,
            &flavor_metadata,
        );

        let destination_master_path = manifest
            .storage
            .depot_store_dir
            .join(&golden_master_filename);

        // 2. Preflight guardrail: check if golden master already exists (immutable store)
        let master_already_exists = self.storage.inspect_image(&destination_master_path).is_ok();
        if master_already_exists && !options.force {
            tracing::info!(
                path = %destination_master_path.display(),
                "Golden master already exists in depot store; skipping build"
            );
            Ok(ImageBuildOutcome::AlreadyExists {
                golden_master_path: destination_master_path,
            })
        } else {
            // 3. Locate required Windows installation ISO
            let win_iso_path = self.resolve_iso_path(manifest, image_tag, options)?;

            // 4. Verify OEMDRV path exists
            let oemdrv_path = &flavor_metadata.oemdrv_nix_store_path;
            let oemdrv_exists = self.storage.inspect_image(oemdrv_path).is_ok();
            oemdrv_exists
                .then_some(())
                .ok_or_else(|| ImageBuilderError::OemdrvNotFound {
                    path: oemdrv_path.clone(),
                })?;

            // 5. Allocate ephemeral scratch isolation directory
            let scratch_directory = match options.scratch_dir {
                Some(ref custom_directory) => custom_directory.clone(),
                None => {
                    let pid = std::process::id();
                    let tag_slug = format!(
                        "{}-{}-{}-{}",
                        image_tag.operating_system,
                        image_tag.build_version,
                        image_tag.flavor_name,
                        flavor_metadata.content_hash
                    );
                    std::env::temp_dir()
                        .join("onehost-build")
                        .join(format!("{tag_slug}-{pid}"))
                }
            };

            let scratch_disk_path = scratch_directory.join("build-disk.qcow2");
            let swtpm_directory = scratch_directory.join("swtpm");
            let swtpm_socket_path = swtpm_directory.join("swtpm-sock");
            let ovmf_vars_path = scratch_directory.join("ovmf-vars.fd");
            let monitor_socket_path = scratch_directory.join("monitor.sock");

            // Arm the scratch cleanup guard to guarantee removal of scratch disks on failure or panic
            let mut scratch_guard = BuildScratchGuard::new(self.storage, scratch_disk_path.clone());

            // 6. Create 64GB ephemeral build disk in scratch directory
            self.storage
                .create_empty_disk(&scratch_disk_path, 64 * 1024 * 1024 * 1024)?;

            // 7. Initialize OVMF NVRAM vars in scratch directory
            self.storage
                .initialize_nvram(&manifest.storage.nvram_template, &ovmf_vars_path)?;

            // 8. Spawn background swtpm TPM 2.0 emulator daemon
            let swtpm_state_arg = format!("dir={}", swtpm_directory.display());
            let swtpm_ctrl_arg = format!("type=unixio,path={}", swtpm_socket_path.display());

            let mut swtpm_handle = self.runner.spawn_daemon(
                "swtpm",
                &[
                    "socket",
                    "--tpmstate",
                    &swtpm_state_arg,
                    "--ctrl",
                    &swtpm_ctrl_arg,
                    "--tpm2",
                ],
            )?;

            // 9. Execute headless QEMU installation VM
            let smp_argument = format!(
                "{},sockets=1,cores={},threads=1",
                options.cpu_count, options.cpu_count
            );
            let memory_argument = options.memory_mb.to_string();
            let ovmf_code_argument = format!(
                "if=pflash,format=raw,readonly=on,file={}",
                manifest.storage.ovmf_code.display()
            );
            let ovmf_vars_argument = format!("if=pflash,format=raw,file={}", ovmf_vars_path.display());
            let chardev_argument = format!(
                "socket,id=chrtpm,path={}",
                swtpm_socket_path.display()
            );
            let build_drive_argument = format!(
                "file={},if=virtio,format=qcow2,cache=none",
                scratch_disk_path.display()
            );
            let win_iso_argument = format!("file={},media=cdrom,readonly=on", win_iso_path.display());
            let oemdrv_iso_argument = format!("file={},media=cdrom,readonly=on", oemdrv_path.display());
            let monitor_argument = format!("unix:{},server,nowait", monitor_socket_path.display());

            let qemu_arguments = [
                "-enable-kvm",
                "-cpu",
                "host,hv_relaxed,hv_spinlocks=0x1fff,hv_vapic,hv_time",
                "-smp",
                &smp_argument,
                "-m",
                &memory_argument,
                "-drive",
                &ovmf_code_argument,
                "-drive",
                &ovmf_vars_argument,
                "-chardev",
                &chardev_argument,
                "-tpmdev",
                "emulator,id=tpm0,chardev=chrtpm",
                "-device",
                "tpm-tis,tpmdev=tpm0",
                "-drive",
                &build_drive_argument,
                "-drive",
                &win_iso_argument,
                "-drive",
                &oemdrv_iso_argument,
                "-net",
                "nic,model=virtio",
                "-net",
                "user",
                "-monitor",
                &monitor_argument,
                "-nographic",
            ];

            tracing::info!(
                tag = %image_tag,
                "Launching headless QEMU builder VM for automated Windows installation"
            );

            let qemu_output = self.runner.execute("qemu-system-x86_64", &qemu_arguments)?;

            if let Err(kill_error) = swtpm_handle.kill() {
                tracing::debug!(error = %kill_error, "Failed to kill swtpm handle after QEMU execution");
            }

            (qemu_output.exit_code == 0)
                .then_some(())
                .ok_or(ImageBuilderError::QemuExecutionFailed {
                    exit_code: Some(qemu_output.exit_code),
                    stderr: qemu_output.stderr,
                })?;

            // 10. Atomic Master Promotion Sequence
            let temporary_master_filename = format!("{golden_master_filename}.tmp");
            let temporary_master_path = manifest
                .storage
                .depot_store_dir
                .join(temporary_master_filename);

            scratch_guard.set_temporary_master(temporary_master_path.clone());

            tracing::info!(
                destination = %temporary_master_path.display(),
                "Compressing and staging raw build disk to temporary golden master"
            );

            self.storage.convert_thin_backup(
                &scratch_disk_path,
                &temporary_master_path,
                None,
                true,
            )?;

            // Verify image integrity before promotion
            self.storage
                .check_image(&temporary_master_path)
                .map_err(|corruption_error| ImageBuilderError::PromotionIntegrityCheckFailed {
                    path: temporary_master_path.clone(),
                    details: corruption_error.to_string(),
                })?;

            // Atomically rename temporary staged image to final immutable golden master
            self.storage
                .move_file_safely(&temporary_master_path, &destination_master_path)?;

            // Apply strict read-only permissions (0444)
            self.storage.set_readonly(&destination_master_path)?;

            // Disarm cleanup guard and unlink scratch build disk
            scratch_guard.disarm();
            self.storage.delete_image(&scratch_disk_path)?;

            let final_inspection = self.storage.inspect_image(&destination_master_path)?;

            tracing::info!(
                path = %destination_master_path.display(),
                size_bytes = final_inspection.actual_size_bytes,
                "Golden master successfully created and promoted to depot store"
            );

            Ok(ImageBuildOutcome::Built {
                golden_master_path: destination_master_path,
                virtual_size_bytes: final_inspection.virtual_size_bytes,
                archive_size_bytes: final_inspection.actual_size_bytes,
            })
        }
    }
}
