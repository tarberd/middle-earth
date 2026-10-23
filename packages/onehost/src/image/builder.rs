use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

use crate::config::model::OnehostManifest;
use crate::image::tag::{
    ContentAddressedImageResolver, FlavorDerivationMetadata, FlavorResolutionError,
    ImageTagParseError, ImageTagSpecification,
};
use crate::storage::{QemuImgStorage, StorageError};

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

    #[error(
        "Windows installation ISO not found in '{iso_directory}' for release tuple '{release_tuple}'"
    )]
    IsoNotFound {
        iso_directory: PathBuf,
        release_tuple: String,
    },

    #[error("OEMDRV derivation path '{path}' does not exist on host filesystem")]
    OemdrvNotFound { path: PathBuf },

    #[error("TPM emulator (swtpm) failed: {details}")]
    SwtpmError { details: String },

    #[error("Headless QEMU installation VM failed with exit code {exit_code:?}: {stderr}")]
    QemuExecutionFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Integrity check failed on staged golden master '{path}': {details}")]
    PromotionIntegrityCheckFailed { path: PathBuf, details: String },

    #[error("Storage error during image build: {0}")]
    StorageError(#[from] StorageError),

    #[error("I/O error during image build: {0}")]
    IoError(#[from] std::io::Error),
}


/// Orchestrates unattended installation of Windows golden master images using headless QEMU, swtpm, and OEMDRV media.
#[derive(Debug, Clone)]
pub struct ImageBuilder<'a> {
    pub storage: &'a QemuImgStorage,
    pub swtpm_binary_path: PathBuf,
    pub qemu_binary_path: PathBuf,
}

impl<'a> ImageBuilder<'a> {
    /// Creates a new ImageBuilder instance.
    pub fn new(storage: &'a QemuImgStorage) -> Self {
        Self {
            storage,
            swtpm_binary_path: PathBuf::from("swtpm"),
            qemu_binary_path: PathBuf::from("qemu-system-x86_64"),
        }
    }

    /// Functional builder: configures custom swtpm binary path.
    pub fn with_swtpm_binary(self, binary: impl Into<PathBuf>) -> Self {
        Self {
            swtpm_binary_path: binary.into(),
            ..self
        }
    }

    /// Functional builder: configures custom qemu binary path.
    pub fn with_qemu_binary(self, binary: impl Into<PathBuf>) -> Self {
        Self {
            qemu_binary_path: binary.into(),
            ..self
        }
    }

    /// Resolves the Windows installation ISO path.
    fn resolve_iso_path(
        &self,
        manifest: &OnehostManifest,
        image_tag: &ImageTagSpecification,
        options: &ImageBuildOptions,
    ) -> Result<PathBuf, ImageBuilderError> {
        let release_tuple = image_tag.release_tuple();
        if let Some(ref explicit_iso_path) = options.iso_path {
            let iso_exists = self.storage.inspect_image(explicit_iso_path).is_ok();
            (iso_exists)
                .then(|| explicit_iso_path.clone())
                .ok_or_else(|| ImageBuilderError::IsoNotFound {
                    iso_directory: explicit_iso_path
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_path_buf(),
                    release_tuple,
                })
        } else {
            let candidate_path = ContentAddressedImageResolver::resolve_depot_iso_path(
                &manifest.storage.depot_iso_dir,
                image_tag,
            );

            if self.storage.inspect_image(&candidate_path).is_ok() {
                Ok(candidate_path)
            } else {
                Err(ImageBuilderError::IsoNotFound {
                    iso_directory: manifest.storage.depot_iso_dir.clone(),
                    release_tuple,
                })
            }
        }
    }

    /// Executes QEMU installation VM with background swtpm emulator, ensuring swtpm child process is killed and reaped.
    fn execute_qemu_with_swtpm(
        &self,
        swtpm_directory: &Path,
        swtpm_socket_path: &Path,
        qemu_arguments: &[&str],
    ) -> Result<(), ImageBuilderError> {
        std::fs::create_dir_all(swtpm_directory)?;
        let swtpm_state_arg = format!("dir={}", swtpm_directory.display());
        let swtpm_ctrl_arg = format!("type=unixio,path={}", swtpm_socket_path.display());

        let mut swtpm_child = Command::new(&self.swtpm_binary_path)
            .args([
                "socket",
                "--tpmstate",
                &swtpm_state_arg,
                "--ctrl",
                &swtpm_ctrl_arg,
                "--tpm2",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|io_error| ImageBuilderError::SwtpmError {
                details: format!(
                    "Failed to spawn swtpm binary '{}': {io_error}",
                    self.swtpm_binary_path.display()
                ),
            })?;

        let qemu_result = Command::new(&self.qemu_binary_path)
            .args(qemu_arguments)
            .output();

        if let Err(kill_error) = swtpm_child.kill() {
            tracing::debug!(
                error = %kill_error,
                "Failed to kill swtpm child process"
            );
        }
        if let Err(wait_error) = swtpm_child.wait() {
            tracing::debug!(
                error = %wait_error,
                "Failed to wait for swtpm child process"
            );
        }

        let qemu_output = qemu_result.map_err(ImageBuilderError::IoError)?;

        (qemu_output.status.success())
            .then_some(())
            .ok_or_else(|| ImageBuilderError::QemuExecutionFailed {
                exit_code: qemu_output.status.code(),
                stderr: String::from_utf8_lossy(&qemu_output.stderr).to_string(),
            })
    }

    /// Converts scratch disk to temporary staged golden master and verifies image integrity.
    fn stage_and_verify_temporary_master(
        &self,
        scratch_disk_path: &Path,
        temporary_master_path: &Path,
    ) -> Result<(), ImageBuilderError> {
        self.storage
            .convert_thin_backup(scratch_disk_path, temporary_master_path, None, true)?;

        self.storage
            .check_image(temporary_master_path)
            .map_err(|corruption_error| ImageBuilderError::PromotionIntegrityCheckFailed {
                path: temporary_master_path.to_path_buf(),
                details: corruption_error.to_string(),
            })
    }

    /// Builds a content-addressed golden master image according to the specified image tag and options.
    pub fn build_image(
        &self,
        manifest: &OnehostManifest,
        image_tag: &ImageTagSpecification,
        options: &ImageBuildOptions,
    ) -> Result<ImageBuildOutcome, ImageBuilderError> {
        // 1. Resolve flavor derivation metadata and content hash
        let flavor_config = manifest.flavors.get(&image_tag.flavor).ok_or_else(|| {
            FlavorResolutionError::UnknownFlavor {
                requested_flavor: image_tag.flavor.clone(),
                available_flavors: manifest.flavors.keys().cloned().collect(),
            }
        })?;

        flavor_config
            .oemdrv_path
            .exists()
            .then_some(())
            .ok_or_else(|| ImageBuilderError::OemdrvNotFound {
                path: flavor_config.oemdrv_path.clone(),
            })?;

        let content_hash = self
            .storage
            .compute_content_hash(&flavor_config.oemdrv_path, &flavor_config.extra_isos)?;

        let flavor_metadata =
            FlavorDerivationMetadata::new(flavor_config.oemdrv_path.clone(), content_hash)?;

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

            let oemdrv_path = &flavor_metadata.oemdrv_nix_store_path;

            // 4. Allocate ephemeral scratch isolation directory
            let scratch_directory = match options.scratch_dir {
                Some(ref custom_directory) => custom_directory.clone(),
                None => {
                    let pid = std::process::id();
                    let tag_slug = format!(
                        "{}-{}-{}-{}-{}",
                        image_tag.uup_id,
                        image_tag.edition,
                        image_tag.language,
                        image_tag.flavor,
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

            std::fs::create_dir_all(&scratch_directory)?;

            // 5. Create 64GB ephemeral build disk in scratch directory
            self.storage
                .create_empty_disk(&scratch_disk_path, 64 * 1024 * 1024 * 1024)?;

            let smp_argument = format!(
                "{},sockets=1,cores={},threads=1",
                options.cpu_count, options.cpu_count
            );
            let memory_argument = options.memory_mb.to_string();
            let ovmf_code_argument = format!(
                "if=pflash,format=raw,readonly=on,file={}",
                manifest.storage.ovmf_code.display()
            );
            let ovmf_vars_argument =
                format!("if=pflash,format=raw,file={}", ovmf_vars_path.display());
            let chardev_argument = format!("socket,id=chrtpm,path={}", swtpm_socket_path.display());
            let build_drive_argument = format!(
                "file={},if=virtio,format=qcow2,cache=none",
                scratch_disk_path.display()
            );
            let win_iso_argument =
                format!("file={},media=cdrom,readonly=on", win_iso_path.display());
            let oemdrv_iso_argument =
                format!("file={},media=cdrom,readonly=on", oemdrv_path.display());
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

            let temporary_master_filename = format!("{golden_master_filename}.tmp");
            let temporary_master_path = manifest
                .storage
                .depot_store_dir
                .join(temporary_master_filename);

            let build_steps_result = (|| -> Result<(), ImageBuilderError> {
                // 6. Initialize OVMF NVRAM vars in scratch directory
                self.storage
                    .initialize_nvram(&manifest.storage.nvram_template, &ovmf_vars_path)?;

                // 7. Execute headless QEMU installation VM with TPM emulator
                tracing::info!(
                    tag = %image_tag,
                    "Launching headless QEMU builder VM for automated Windows installation"
                );
                self.execute_qemu_with_swtpm(
                    &swtpm_directory,
                    &swtpm_socket_path,
                    &qemu_arguments,
                )?;

                // 8. Atomic Master Promotion Sequence
                tracing::info!(
                    destination = %temporary_master_path.display(),
                    "Compressing and staging raw build disk to temporary golden master"
                );

                self.stage_and_verify_temporary_master(
                    &scratch_disk_path,
                    &temporary_master_path,
                )
                .inspect_err(|staging_error| {
                    tracing::debug!(
                        path = %temporary_master_path.display(),
                        error = %staging_error,
                        "Temporary master staging or integrity check failed"
                    );
                    if let Err(cleanup_error) = self.storage.delete_image(&temporary_master_path) {
                        tracing::debug!(
                            path = %temporary_master_path.display(),
                            error = %cleanup_error,
                            "Failed to delete temporary master during cleanup"
                        );
                    }
                })?;

                // Atomically rename temporary staged image to final immutable golden master
                self.storage
                    .move_file_safely(&temporary_master_path, &destination_master_path)?;

                // Apply strict read-only permissions (0444)
                self.storage.set_readonly(&destination_master_path)?;

                Ok(())
            })();

            build_steps_result.inspect_err(|build_error| {
                tracing::debug!(
                    path = %scratch_disk_path.display(),
                    error = %build_error,
                    "Build pipeline failed; unlinking scratch build disk"
                );
                if let Err(cleanup_error) = self.storage.delete_image(&scratch_disk_path) {
                    tracing::debug!(
                        path = %scratch_disk_path.display(),
                        error = %cleanup_error,
                        "Failed to delete scratch disk during cleanup"
                    );
                }
            })?;

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
