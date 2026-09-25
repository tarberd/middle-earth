use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

use crate::hypervisor::traits::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor,
    HypervisorError,
};

/// Production implementation of Hypervisor backed by the `virsh` command-line utility.
#[derive(Debug, Clone)]
pub struct VirshHypervisor {
    pub virsh_executable_path: PathBuf,
    pub connection_uri: Option<String>,
}

impl Default for VirshHypervisor {
    fn default() -> Self {
        Self {
            virsh_executable_path: PathBuf::from("virsh"),
            connection_uri: Some("qemu:///system".to_string()),
        }
    }
}

impl VirshHypervisor {
    /// Creates a VirshHypervisor with default path ("virsh") and connection URI ("qemu:///system").
    pub fn new() -> Self {
        Self::default()
    }

    /// Functional builder: configures custom virsh binary path.
    pub fn with_executable(self, executable: impl Into<PathBuf>) -> Self {
        Self {
            virsh_executable_path: executable.into(),
            ..self
        }
    }

    /// Functional builder: configures hypervisor connection URI (e.g. Some("qemu:///system") or None).
    pub fn with_connection_uri(self, uri: Option<String>) -> Self {
        Self {
            connection_uri: uri,
            ..self
        }
    }

    fn execute_command(&self, arguments: &[&str]) -> Result<String, HypervisorError> {
        let mut process = Command::new(&self.virsh_executable_path);

        if let Some(uri) = &self.connection_uri {
            process.arg("-c").arg(uri);
        }

        process.args(arguments);

        let output = process.output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
            let command_line = format!("{} {}", self.virsh_executable_path.display(), arguments.join(" "));

            if stderr_text.contains("failed to get domain") || stderr_text.contains("Domain not found") {
                let domain_name = arguments
                    .iter()
                    .copied()
                    .find(|argument| !argument.starts_with('-'))
                    .unwrap_or("unknown");
                Err(HypervisorError::DomainNotFound {
                    domain_name: domain_name.to_string(),
                })
            } else if stderr_text.contains("failed to get pool") || stderr_text.contains("Storage pool not found") {
                let pool_name = arguments
                    .iter()
                    .copied()
                    .find(|argument| !argument.starts_with('-'))
                    .unwrap_or("unknown");
                Err(HypervisorError::StoragePoolNotFound {
                    pool_name: pool_name.to_string(),
                })
            } else {
                Err(HypervisorError::CommandExecutionFailed {
                    command: command_line,
                    exit_code: output.status.code(),
                    stderr: stderr_text,
                })
            }
        }
    }
}

/// Pure parser: parses `virsh dominfo` stdout into a structured DomainInfo instance.
pub fn parse_dominfo_output(
    raw_output: &str,
    domain_name: &str,
) -> Result<DomainInfo, HypervisorError> {
    let initial_domain_info = DomainInfo {
        name: domain_name.to_string(),
        state: DomainState::Unknown,
        vcpu_count: None,
        memory_kib: None,
        autostart: false,
    };

    let parsed_info = raw_output
        .lines()
        .filter_map(|line| line.split_once(':'))
        .fold(initial_domain_info, |accumulator, (raw_key, raw_value)| {
            let key = raw_key.trim();
            let value = raw_value.trim();

            match key {
                "State" => DomainInfo {
                    state: DomainState::from_virsh_state(value),
                    ..accumulator
                },
                "CPU(s)" => DomainInfo {
                    vcpu_count: value.parse::<u32>().ok(),
                    ..accumulator
                },
                "Max memory" => DomainInfo {
                    memory_kib: value
                        .split_whitespace()
                        .next()
                        .and_then(|numeric_part| numeric_part.parse::<u64>().ok()),
                    ..accumulator
                },
                "Autostart" => DomainInfo {
                    autostart: value.eq_ignore_ascii_case("enable"),
                    ..accumulator
                },
                _ => accumulator,
            }
        });

    Ok(parsed_info)
}

/// Pure parser: parses `virsh domblklist <domain> --details` stdout into BlockDeviceInfo entries.
pub fn parse_domblklist_output(raw_output: &str) -> Result<Vec<BlockDeviceInfo>, HypervisorError> {
    Ok(raw_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Type") && !line.starts_with("----"))
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() >= 3 {
                let device_type = columns[1].to_string();
                let target_device = columns[2].to_string();
                let source_file = (columns.len() >= 4 && columns[3] != "-")
                    .then(|| PathBuf::from(columns[3]));

                Some(BlockDeviceInfo {
                    target_device,
                    source_file,
                    device_type,
                })
            } else {
                None
            }
        })
        .collect())
}

impl Hypervisor for VirshHypervisor {
    fn domain_info(&self, domain_name: &str) -> Result<DomainInfo, HypervisorError> {
        let raw_output = self.execute_command(&["dominfo", domain_name])?;
        parse_dominfo_output(&raw_output, domain_name)
    }

    fn dump_xml(&self, domain_name: &str, inactive: bool) -> Result<String, HypervisorError> {
        let inactive_flag = inactive.then_some("--inactive");
        let arguments: Vec<&str> = ["dumpxml"]
            .into_iter()
            .chain(inactive_flag)
            .chain([domain_name])
            .collect();
        self.execute_command(&arguments)
    }

    fn define_domain(&self, domain_xml: &str) -> Result<(), HypervisorError> {
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(domain_xml.as_bytes())?;
        temp_file.flush()?;

        let temp_path = temp_file.path().to_str().ok_or_else(|| {
            HypervisorError::OutputParseError {
                details: "Temporary domain XML file path is invalid UTF-8".to_string(),
            }
        })?;

        self.execute_command(&["define", temp_path])?;
        Ok(())
    }

    fn undefine_domain(
        &self,
        domain_name: &str,
        cleanup_nvram: bool,
    ) -> Result<(), HypervisorError> {
        let nvram_flag = cleanup_nvram.then_some("--nvram");
        let arguments: Vec<&str> = ["undefine", domain_name]
            .into_iter()
            .chain(nvram_flag)
            .collect();
        self.execute_command(&arguments)?;
        Ok(())
    }

    fn start_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        self.execute_command(&["start", domain_name])?;
        Ok(())
    }

    fn shutdown_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        self.execute_command(&["shutdown", domain_name])?;
        Ok(())
    }

    fn destroy_domain(&self, domain_name: &str) -> Result<(), HypervisorError> {
        self.execute_command(&["destroy", domain_name])?;
        Ok(())
    }

    fn list_block_devices(
        &self,
        domain_name: &str,
    ) -> Result<Vec<BlockDeviceInfo>, HypervisorError> {
        let raw_output = self.execute_command(&["domblklist", domain_name, "--details"])?;
        parse_domblklist_output(&raw_output)
    }

    fn create_snapshot(
        &self,
        domain_name: &str,
        snapshot_name: &str,
        disk_specifications: &[DiskSnapshotSpecification],
        quiesce: bool,
    ) -> Result<(), HypervisorError> {
        let formatted_disk_specifications: Vec<String> = disk_specifications
            .iter()
            .map(|specification| {
                format!(
                    "{},file={}",
                    specification.target_device,
                    specification.snapshot_file_path.display()
                )
            })
            .collect();

        let base_arguments = [
            "snapshot-create-as",
            "--domain",
            domain_name,
            "--name",
            snapshot_name,
            "--disk-only",
            "--atomic",
            "--no-metadata",
        ];

        let diskspec_flag_arguments: Vec<&str> = formatted_disk_specifications
            .iter()
            .flat_map(|diskspec| ["--diskspec", diskspec.as_str()])
            .collect();

        let quiesce_flag: Option<&str> = quiesce.then_some("--quiesce");

        let arguments: Vec<&str> = base_arguments
            .into_iter()
            .chain(diskspec_flag_arguments)
            .chain(quiesce_flag)
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn blockcommit(
        &self,
        domain_name: &str,
        target_device: &str,
        base_path: Option<&Path>,
        top_path: Option<&Path>,
        active: bool,
        pivot: bool,
    ) -> Result<(), HypervisorError> {
        let rendered_base_path = base_path.map(|path| path.display().to_string());
        let rendered_top_path = top_path.map(|path| path.display().to_string());

        let base_flag: Option<[&str; 2]> = rendered_base_path
            .as_deref()
            .map(|base| ["--base", base]);
        let top_flag: Option<[&str; 2]> = rendered_top_path
            .as_deref()
            .map(|top| ["--top", top]);
        let active_flag: Option<&str> = active.then_some("--active");
        let pivot_flag: Option<&str> = pivot.then_some("--pivot");

        let arguments: Vec<&str> = ["blockcommit", domain_name, target_device]
            .into_iter()
            .chain(base_flag.into_iter().flatten())
            .chain(top_flag.into_iter().flatten())
            .chain(active_flag)
            .chain(pivot_flag)
            .collect();

        self.execute_command(&arguments)?;
        Ok(())
    }

    fn pool_dump_xml(&self, pool_name: &str) -> Result<String, HypervisorError> {
        self.execute_command(&["pool-dumpxml", pool_name])
    }

    fn pool_refresh(&self, pool_name: &str) -> Result<(), HypervisorError> {
        self.execute_command(&["pool-refresh", pool_name])?;
        Ok(())
    }

    fn set_autostart(&self, domain_name: &str, autostart: bool) -> Result<(), HypervisorError> {
        let disable_flag = (!autostart).then_some("--disable");
        let arguments: Vec<&str> = ["autostart", domain_name]
            .into_iter()
            .chain(disable_flag)
            .collect();
        self.execute_command(&arguments)?;
        Ok(())
    }

    fn list_storage_pools(&self) -> Result<Vec<String>, HypervisorError> {
        let raw_output = self.execute_command(&["pool-list", "--all", "--name"])?;
        Ok(raw_output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect())
    }
}
