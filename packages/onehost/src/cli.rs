use std::io::Write;
use std::path::{Path, PathBuf};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::backup::{BackupEngine, BackupError, BackupOptions, RestoreEngine, RestoreError, RestoreOptions};
use crate::config::loader::{ManifestLoadError, ManifestLoader};
use crate::config::model::OnehostManifest;
use crate::config::validation::ManifestValidationError;
use crate::hypervisor::traits::{DomainState, Hypervisor, HypervisorError};
use crate::image::builder::{ImageBuildOptions, ImageBuildOutcome, ImageBuilder, ImageBuilderError};
use crate::image::tag::{ImageTagParseError, ImageTagSpecification};
use crate::lifecycle::{
    ApplyOptions, DestroyOptions, DomainLifecycleApplier, DomainLifecycleDestroyer,
    DomainLifecyclePlanner, InstancePlanAction, LifecycleError, OnehostPlan,
};
use crate::process::traits::{ProcessError, ProcessRunner};
use crate::storage::traits::{StorageError, StorageManager};
use crate::xdg::XdgBaseDirectories;

/// Command-line interface definition for the onehost tool.
#[derive(Parser, Debug)]
#[command(
    name = "onehost",
    about = "Declarative Windows Libvirt/KVM lifecycle management and disaster recovery tool",
    version
)]
pub struct Cli {
    /// Path to the onehost.json declarative manifest
    #[arg(short, long, global = true, value_name = "PATH")]
    pub manifest: Option<PathBuf>,

    /// Emit machine-readable JSON output to stdout
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Supported subcommands for onehost.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Inspect live infrastructure and emit an execution plan against declared manifest
    Plan(PlanArgs),

    /// Reconcile live hypervisor and storage state to match declared manifest
    Apply(ApplyArgs),

    /// Gracefully shutdown and undefine a domain, optionally unlinking storage overlays
    Destroy(DestroyArgs),

    /// Perform a crash-consistent or live VSS-quiesced thin backup of an instance
    Backup(BackupArgs),

    /// Restore an instance from a staged backup directory with full disaster recovery
    Restore(RestoreArgs),

    /// Build and manage golden master QCOW2 images
    Image(ImageCommand),

    /// Query and display real-time operational status across declared instances and storage
    Status(StatusArgs),
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct PlanArgs {
    /// Optional target instance name filter
    #[arg(short = 'i', long)]
    pub instance: Option<String>,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct ApplyArgs {
    /// Optional target instance name filter
    #[arg(short = 'i', long)]
    pub instance: Option<String>,

    /// Permit recreation of instance overlays when base image drift occurs
    #[arg(long)]
    pub allow_recreate: bool,

    /// Proceed with state mutations without interactive confirmation
    #[arg(short = 'y', long)]
    pub auto_approve: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct DestroyArgs {
    /// Target instance name to deprovision
    pub instance: String,

    /// Delete the instance copy-on-write overlay disk from storage pool
    #[arg(long)]
    pub delete_disk: bool,

    /// Immediately force power off (virsh destroy) instead of graceful ACPI shutdown
    #[arg(short, long)]
    pub force: bool,

    /// Override prevent_destroy guardrail policy if declared on the instance
    #[arg(long)]
    pub allow_destroy_protected: bool,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub auto_approve: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct BackupArgs {
    /// Instance name to backup
    pub instance: String,

    /// Destination directory for backup artifacts
    #[arg(short, long)]
    pub target_dir: Option<PathBuf>,

    /// Skip guest VSS quiescing and perform crash-consistent snapshot
    #[arg(long)]
    pub crash_consistent: bool,

    /// Disable zlib compression on exported thin backup archives
    #[arg(long)]
    pub no_compress: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct RestoreArgs {
    /// Staged backup directory containing manifest.json, domain.xml, and disk archives
    pub backup_dir: PathBuf,

    /// Optional target storage pool override
    #[arg(short, long)]
    pub pool: Option<String>,

    /// Allow restoring over an already defined hypervisor domain
    #[arg(long)]
    pub allow_overwrite: bool,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct ImageCommand {
    #[command(subcommand)]
    pub action: ImageSubcommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ImageSubcommands {
    /// Build a golden master QCOW2 image from official ISO and OEMDRV derivation
    Build(ImageBuildArgs),
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct ImageBuildArgs {
    /// Image tag specification (<os>/<version>/<flavor>)
    pub tag: String,

    /// Force rebuild even if golden master already exists in depot store
    #[arg(short, long)]
    pub force: bool,

    /// Custom scratch directory for build artifacts
    #[arg(long)]
    pub scratch_dir: Option<PathBuf>,

    /// Explicit path to Windows installation ISO
    #[arg(long)]
    pub iso: Option<PathBuf>,

    /// VM RAM allocation in megabytes (default: 8192)
    #[arg(long, default_value_t = 8192)]
    pub memory: u32,

    /// VM vCPU core count (default: 8)
    #[arg(long, default_value_t = 8)]
    pub cpus: u32,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct StatusArgs {
    /// Optional target instance name filter
    #[arg(short = 'i', long)]
    pub instance: Option<String>,
}

/// Structured report of an individual instance's live operational status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceStatusReport {
    pub instance_name: String,
    pub domain_state: Option<String>,
    pub vcpus: Option<u32>,
    pub memory_kib: Option<u64>,
    pub declared_pool: String,
    pub overlay_path: PathBuf,
    pub overlay_exists: bool,
    pub virtual_size_bytes: Option<u64>,
    pub actual_size_bytes: Option<u64>,
    pub backing_file: Option<PathBuf>,
}

/// Enumerates errors that can occur during CLI command execution.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Manifest file not found at '{path}'. Please specify --manifest <PATH> or create onehost.json")]
    ManifestNotFound { path: PathBuf },

    #[error("Failed to load manifest: {0}")]
    ManifestLoadError(#[from] ManifestLoadError),

    #[error("Manifest validation error: {0}")]
    ManifestValidationError(#[from] ManifestValidationError),

    #[error("Lifecycle planning error: {0}")]
    LifecycleError(#[from] LifecycleError),

    #[error("Backup error: {0}")]
    BackupError(#[from] BackupError),

    #[error("Restore error: {0}")]
    RestoreError(#[from] RestoreError),

    #[error("Image builder error: {0}")]
    ImageBuilderError(#[from] ImageBuilderError),

    #[error("Hypervisor error: {0}")]
    HypervisorError(#[from] HypervisorError),

    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    #[error("Process error: {0}")]
    ProcessError(#[from] ProcessError),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Image tag parsing error: {0}")]
    TagError(#[from] ImageTagParseError),
}

/// Resolves the effective manifest path from explicit argument, current directory, or XDG config directory.
pub fn resolve_manifest_path(explicit_path: Option<&Path>) -> Result<PathBuf, CliError> {
    match explicit_path {
        Some(path) => (path.exists())
            .then(|| path.to_path_buf())
            .ok_or_else(|| CliError::ManifestNotFound {
                path: path.to_path_buf(),
            }),
        None => {
            let current_dir_manifest = PathBuf::from("onehost.json");
            if current_dir_manifest.exists() {
                Ok(current_dir_manifest)
            } else {
                let xdg_directories = XdgBaseDirectories::from_process_environment();
                let xdg_manifest = xdg_directories
                    .onehost_configuration_directory()
                    .map(|configuration_directory| configuration_directory.join("onehost.json"))
                    .map_err(|_| CliError::ManifestNotFound {
                        path: current_dir_manifest.clone(),
                    })?;

                (xdg_manifest.exists())
                    .then_some(xdg_manifest)
                    .ok_or(CliError::ManifestNotFound {
                        path: current_dir_manifest,
                    })
            }
        }
    }
}

/// Formats a human-readable OpenTofu-style execution plan diff.
pub fn format_plan_human_readable(plan: &OnehostPlan) -> String {
    let actions_output = plan
        .actions
        .iter()
        .map(|action| match action {
            InstancePlanAction::Create {
                instance_name,
                target_pool,
                golden_master_path,
                ..
            } => {
                format!(
                    "  + create instance '{instance_name}' in pool '{target_pool}' (base: {})",
                    golden_master_path.display()
                )
            }
            InstancePlanAction::UpdateDomainXml {
                instance_name,
                diff_summary,
                ..
            } => {
                format!("  ~ update instance '{instance_name}' domain XML\n{diff_summary}")
            }
            InstancePlanAction::Recreate {
                instance_name,
                reason,
                ..
            } => {
                format!("  ! recreate instance '{instance_name}': {reason}")
            }
            InstancePlanAction::RelocateStoragePool {
                instance_name,
                source_pool,
                target_pool,
                ..
            } => {
                format!("  > relocate instance '{instance_name}' storage from '{source_pool}' -> '{target_pool}'")
            }
            InstancePlanAction::Delete { instance_name, .. } => {
                format!("  - destroy instance '{instance_name}'")
            }
            InstancePlanAction::NoOp { instance_name } => {
                format!("  <= no-op instance '{instance_name}' (matches declared state)")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let count_creates = plan
        .actions
        .iter()
        .filter(|action| matches!(action, InstancePlanAction::Create { .. }))
        .count();
    let count_updates = plan
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action,
                InstancePlanAction::UpdateDomainXml { .. }
                    | InstancePlanAction::RelocateStoragePool { .. }
            )
        })
        .count();
    let count_recreates = plan
        .actions
        .iter()
        .filter(|action| matches!(action, InstancePlanAction::Recreate { .. }))
        .count();
    let count_destroys = plan
        .actions
        .iter()
        .filter(|action| matches!(action, InstancePlanAction::Delete { .. }))
        .count();

    format!(
        "Onehost Execution Plan\n\n{}\n\nPlan: {} to add, {} to change, {} to recreate, {} to destroy.",
        actions_output, count_creates, count_updates, count_recreates, count_destroys
    )
}

/// Collects live status reports across all instances declared in the manifest.
pub fn collect_status_reports<H: Hypervisor, S: StorageManager>(
    manifest: &OnehostManifest,
    hypervisor: &H,
    storage: &S,
    filter_instance: Option<&str>,
) -> Result<Vec<InstanceStatusReport>, CliError> {
    manifest
        .instances
        .iter()
        .filter(|(instance_name, _instance_configuration)| {
            filter_instance.is_none_or(|target_name| instance_name.as_str() == target_name)
        })
        .map(|(instance_name, instance_configuration)| {
            let domain_info = hypervisor.domain_info(instance_name).ok();
            let effective_pool = instance_configuration.effective_pool(&manifest.storage)?;
            let pool_directory = hypervisor.resolve_pool_path(effective_pool)?;
            let overlay_path = pool_directory.join(format!("{instance_name}.qcow2"));

            let inspection = storage.inspect_image(&overlay_path).ok();
            let overlay_exists = inspection.is_some();

            let domain_state = domain_info.as_ref().map(|info| match info.state {
                DomainState::Running => "running".to_string(),
                DomainState::Shutoff => "shutoff".to_string(),
                DomainState::Paused => "paused".to_string(),
                DomainState::Crashed => "crashed".to_string(),
                DomainState::Blocked => "blocked".to_string(),
                DomainState::Pmsuspended => "pmsuspended".to_string(),
                DomainState::Unknown => "unknown".to_string(),
            });

            let vcpus = domain_info.as_ref().and_then(|info| info.vcpu_count);
            let memory_kib = domain_info.as_ref().and_then(|info| info.memory_kib);
            let virtual_size_bytes = inspection.as_ref().map(|info| info.virtual_size_bytes);
            let actual_size_bytes = inspection.as_ref().map(|info| info.actual_size_bytes);
            let backing_file = inspection.and_then(|info| info.backing_file);

            Ok(InstanceStatusReport {
                instance_name: instance_name.clone(),
                domain_state,
                vcpus,
                memory_kib,
                declared_pool: effective_pool.to_string(),
                overlay_path,
                overlay_exists,
                virtual_size_bytes,
                actual_size_bytes,
                backing_file,
            })
        })
        .collect()
}

/// Formats a human-readable table of instance status reports.
pub fn format_status_table(reports: &[InstanceStatusReport]) -> String {
    let header = format!(
        "{:<16} {:<10} {:<6} {:<10} {:<10} {:<10} {}",
        "INSTANCE", "STATE", "VCPUS", "MEM(MiB)", "POOL", "DISK", "BACKING"
    );
    let separator = "-".repeat(80);

    let rows = reports
        .iter()
        .map(|report| {
            let state = report.domain_state.as_deref().unwrap_or("undefined");
            let vcpus = report
                .vcpus
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".to_string());
            let memory_mib = report
                .memory_kib
                .map(|kibibytes| (kibibytes / 1024).to_string())
                .unwrap_or_else(|| "-".to_string());
            let disk_size = report
                .actual_size_bytes
                .map(|bytes| format!("{:.1}M", bytes as f64 / 1024.0 / 1024.0))
                .unwrap_or_else(|| "-".to_string());
            let backing = report
                .backing_file
                .as_ref()
                .and_then(|file_path| file_path.file_name())
                .and_then(|file_name| file_name.to_str())
                .unwrap_or("-");

            format!(
                "{:<16} {:<10} {:<6} {:<10} {:<10} {:<10} {}",
                report.instance_name,
                state,
                vcpus,
                memory_mib,
                report.declared_pool,
                disk_size,
                backing
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}\n{}\n{}", header, separator, rows)
}

/// Dispatches a parsed CLI command to the appropriate imperative lifecycle shell.
pub fn dispatch<H: Hypervisor, S: StorageManager, P: ProcessRunner, W: Write>(
    cli: Cli,
    hypervisor: &H,
    storage: &S,
    runner: &P,
    output: &mut W,
) -> Result<i32, CliError> {
    match cli.command {
        Commands::Plan(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref())?;
            let manifest = ManifestLoader::load_from_path(&manifest_path)?;

            let planner = DomainLifecyclePlanner::new(hypervisor, storage);
            let mut plan = planner.plan(&manifest)?;

            if let Some(ref target_name) = args.instance {
                plan.actions
                    .retain(|action| action.instance_name() == target_name);
            }

            if cli.json {
                let serialized = serde_json::to_string_pretty(&plan)?;
                writeln!(output, "{serialized}")?;
            } else {
                let formatted = format_plan_human_readable(&plan);
                writeln!(output, "{formatted}")?;
            }

            Ok(0)
        }

        Commands::Apply(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref())?;
            let manifest = ManifestLoader::load_from_path(&manifest_path)?;

            let planner = DomainLifecyclePlanner::new(hypervisor, storage);
            let mut plan = planner.plan(&manifest)?;

            if let Some(ref target_name) = args.instance {
                plan.actions
                    .retain(|action| action.instance_name() == target_name);
            }

            let apply_options = ApplyOptions {
                allow_recreate: args.allow_recreate,
                ..Default::default()
            };

            let applier = DomainLifecycleApplier::new(hypervisor, storage);
            applier.apply(&plan, &apply_options)?;

            writeln!(output, "Apply complete! All planned actions successfully reconciled.")?;

            Ok(0)
        }

        Commands::Destroy(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref())?;
            let manifest = ManifestLoader::load_from_path(&manifest_path)?;

            let destroyer = DomainLifecycleDestroyer::new(hypervisor, storage);
            let destroy_options = DestroyOptions {
                delete_disk: args.delete_disk,
                force: args.force,
                allow_destroy_protected: args.allow_destroy_protected,
            };

            destroyer.destroy(&args.instance, &manifest, &destroy_options)?;

            writeln!(
                output,
                "Instance '{}' successfully deprovisioned (delete_disk: {}).",
                args.instance, args.delete_disk
            )?;

            Ok(0)
        }

        Commands::Backup(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref()).ok();
            let manifest = match manifest_path {
                Some(ref path) => Some(ManifestLoader::load_from_path(path)?),
                None => None,
            };

            let backup_options = BackupOptions {
                quiesce: !args.crash_consistent,
                compress: !args.no_compress,
                timestamp: None,
            };

            let target_directory = match args.target_dir {
                Some(explicit_dir) => explicit_dir,
                None => {
                    let base_depot_backups = manifest
                        .as_ref()
                        .map(|item| item.storage.depot_backup_dir.clone())
                        .unwrap_or_else(|| PathBuf::from("/data/depot/backups"));

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_secs().to_string())
                        .unwrap_or_else(|_| "backup".to_string());

                    base_depot_backups.join(&args.instance).join(timestamp)
                }
            };

            let backup_engine = BackupEngine::new(hypervisor, storage);
            let manifest_result = backup_engine.backup_instance(
                &args.instance,
                &target_directory,
                &backup_options,
                manifest.as_ref(),
            )?;

            if cli.json {
                let serialized = serde_json::to_string_pretty(&manifest_result)?;
                writeln!(output, "{serialized}")?;
            } else {
                writeln!(
                    output,
                    "Backup complete! Staged {} disk(s) in: {}",
                    manifest_result.disks.len(),
                    target_directory.display()
                )?;
            }

            Ok(0)
        }

        Commands::Restore(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref()).ok();
            let manifest = match manifest_path {
                Some(ref path) => Some(ManifestLoader::load_from_path(path)?),
                None => None,
            };

            let restore_options = RestoreOptions {
                target_pool: args.pool,
                depot_store_dir: None,
                nvram_dir: None,
                allow_overwrite: args.allow_overwrite,
            };

            let restore_engine = RestoreEngine::new(hypervisor, storage);
            restore_engine.restore_instance(&args.backup_dir, &restore_options, manifest.as_ref())?;

            writeln!(
                output,
                "Restore complete! Instance successfully restored from: {}",
                args.backup_dir.display()
            )?;

            Ok(0)
        }

        Commands::Image(ImageCommand { action }) => match action {
            ImageSubcommands::Build(args) => {
                let manifest_path = resolve_manifest_path(cli.manifest.as_deref())?;
                let manifest = ManifestLoader::load_from_path(&manifest_path)?;

                let image_tag: ImageTagSpecification = args.tag.parse()?;
                let build_options = ImageBuildOptions {
                    force: args.force,
                    scratch_dir: args.scratch_dir,
                    iso_path: args.iso,
                    memory_mb: args.memory,
                    cpu_count: args.cpus,
                };

                let builder = ImageBuilder::new(storage, runner);
                let outcome = builder.build_image(&manifest, &image_tag, &build_options)?;

                match outcome {
                    ImageBuildOutcome::AlreadyExists { golden_master_path } => {
                        writeln!(
                            output,
                            "Golden master already exists at: {}",
                            golden_master_path.display()
                        )?;
                    }
                    ImageBuildOutcome::Built {
                        golden_master_path,
                        virtual_size_bytes,
                        archive_size_bytes,
                    } => {
                        writeln!(
                            output,
                            "Golden master successfully created!\nPath: {}\nVirtual Size: {} GiB\nArchive Size: {} MiB",
                            golden_master_path.display(),
                            virtual_size_bytes / 1024 / 1024 / 1024,
                            archive_size_bytes / 1024 / 1024
                        )?;
                    }
                }

                Ok(0)
            }
        },

        Commands::Status(args) => {
            let manifest_path = resolve_manifest_path(cli.manifest.as_deref())?;
            let manifest = ManifestLoader::load_from_path(&manifest_path)?;

            let reports = collect_status_reports(&manifest, hypervisor, storage, args.instance.as_deref())?;

            if cli.json {
                let serialized = serde_json::to_string_pretty(&reports)?;
                writeln!(output, "{serialized}")?;
            } else {
                let formatted = format_status_table(&reports);
                writeln!(output, "{formatted}")?;
            }

            Ok(0)
        }
    }
}
