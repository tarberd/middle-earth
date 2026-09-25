use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Enumerates all errors that can occur during XDG base directory resolution.
#[derive(Debug, Error)]
pub enum XdgDirectoryResolutionError {
    #[error("Environment variable '{variable_name}' must be an absolute path, but got relative path: {provided_path}")]
    RelativePathNotAllowed {
        variable_name: String,
        provided_path: PathBuf,
    },

    #[error("Cannot resolve fallback for '{fallback_target}': the $HOME environment variable is not set")]
    MissingHomeDirectory { fallback_target: String },

    #[error("XDG_RUNTIME_DIR environment variable is not set and no secure user runtime directory could be determined")]
    MissingRuntimeDirectory,

    #[error("Failed to create application directory at '{path}': {source}")]
    DirectoryCreationFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Type alias for environment lookup functions facilitating hermetic unit testing.
pub type EnvironmentLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Resolves standard XDG Base Directories according to the FreeDesktop.org specification,
/// providing application-scoped subdirectories for `onehost` with zero implicit or ambiguous defaults.
pub struct XdgBaseDirectories {
    environment_lookup: EnvironmentLookup,
}

impl XdgBaseDirectories {
    /// Constructs a resolver that reads directly from the live process environment.
    pub fn from_process_environment() -> Self {
        Self {
            environment_lookup: Box::new(|variable_name| std::env::var(variable_name).ok()),
        }
    }

    /// Constructs a resolver using a custom environment lookup closure, facilitating hermetic unit testing.
    pub fn from_environment_lookup<F>(lookup_function: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            environment_lookup: Box::new(lookup_function),
        }
    }

    /// Resolves `$XDG_CONFIG_HOME`, falling back to `$HOME/.config` if unset.
    pub fn configuration_home(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.resolve_directory_with_fallback("XDG_CONFIG_HOME", ".config")
    }

    /// Resolves `$XDG_CACHE_HOME`, falling back to `$HOME/.cache` if unset.
    pub fn cache_home(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.resolve_directory_with_fallback("XDG_CACHE_HOME", ".cache")
    }

    /// Resolves `$XDG_DATA_HOME`, falling back to `$HOME/.local/share` if unset.
    pub fn data_home(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.resolve_directory_with_fallback("XDG_DATA_HOME", ".local/share")
    }

    /// Resolves `$XDG_STATE_HOME`, falling back to `$HOME/.local/state` if unset.
    pub fn state_home(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.resolve_directory_with_fallback("XDG_STATE_HOME", ".local/state")
    }

    /// Resolves `$XDG_RUNTIME_DIR`. Fails explicitly if unset to adhere to zero-default safety.
    pub fn runtime_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        (self.environment_lookup)("XDG_RUNTIME_DIR")
            .ok_or(XdgDirectoryResolutionError::MissingRuntimeDirectory)
            .and_then(|raw_value| validate_absolute_path("XDG_RUNTIME_DIR", raw_value))
    }

    /// Returns the application configuration directory: `$XDG_CONFIG_HOME/onehost`.
    pub fn onehost_configuration_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.configuration_home().map(|directory| directory.join("onehost"))
    }

    /// Returns the application cache directory: `$XDG_CACHE_HOME/onehost`.
    pub fn onehost_cache_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.cache_home().map(|directory| directory.join("onehost"))
    }

    /// Returns the application data directory: `$XDG_DATA_HOME/onehost`.
    pub fn onehost_data_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.data_home().map(|directory| directory.join("onehost"))
    }

    /// Returns the application state directory: `$XDG_STATE_HOME/onehost`.
    pub fn onehost_state_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.state_home().map(|directory| directory.join("onehost"))
    }

    /// Returns the application runtime directory: `$XDG_RUNTIME_DIR/onehost`.
    pub fn onehost_runtime_directory(&self) -> Result<PathBuf, XdgDirectoryResolutionError> {
        self.runtime_directory().map(|directory| directory.join("onehost"))
    }

    /// Creates all application directories on the filesystem if they do not already exist.
    pub fn ensure_application_directories_exist(&self) -> Result<(), XdgDirectoryResolutionError> {
        [
            self.onehost_configuration_directory()?,
            self.onehost_cache_directory()?,
            self.onehost_data_directory()?,
            self.onehost_state_directory()?,
            self.onehost_runtime_directory()?,
        ]
        .into_iter()
        .filter(|directory| !directory.exists())
        .try_for_each(|directory| {
            fs::create_dir_all(&directory).map_err(|source| {
                XdgDirectoryResolutionError::DirectoryCreationFailed {
                    path: directory,
                    source,
                }
            })
        })
    }

    fn resolve_directory_with_fallback(
        &self,
        environment_variable_name: &str,
        fallback_relative_path: &str,
    ) -> Result<PathBuf, XdgDirectoryResolutionError> {
        (self.environment_lookup)(environment_variable_name)
            .map(|raw_value| validate_absolute_path(environment_variable_name, raw_value))
            .unwrap_or_else(|| {
                (self.environment_lookup)("HOME")
                    .ok_or_else(|| XdgDirectoryResolutionError::MissingHomeDirectory {
                        fallback_target: fallback_relative_path.to_string(),
                    })
                    .and_then(|home_value| validate_absolute_path("HOME", home_value))
                    .map(|home_path| home_path.join(fallback_relative_path))
            })
    }
}

fn validate_absolute_path(
    variable_name: &str,
    raw_value: String,
) -> Result<PathBuf, XdgDirectoryResolutionError> {
    let path = PathBuf::from(raw_value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(XdgDirectoryResolutionError::RelativePathNotAllowed {
            variable_name: variable_name.to_string(),
            provided_path: path,
        })
    }
}
