use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::config::model::OnehostManifest;
use crate::config::validation::{validate_manifest, ManifestValidationError};
use crate::xdg::{XdgBaseDirectories, XdgDirectoryResolutionError};

/// Enumerates errors that can occur when loading and validating a manifest file.
#[derive(Debug, Error)]
pub enum ManifestLoadError {
    #[error("Failed to read manifest file from '{path}': {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Manifest file not found at '{path}'")]
    ManifestNotFound { path: PathBuf },

    #[error("Failed to parse manifest JSON: {source}")]
    JsonDeserializationError { source: serde_json::Error },

    #[error("Manifest validation failed: {source}")]
    ValidationError {
        #[from]
        source: ManifestValidationError,
    },

    #[error("Failed to resolve XDG configuration path: {source}")]
    XdgResolutionError {
        #[from]
        source: XdgDirectoryResolutionError,
    },
}

/// Provides facilities for loading, parsing, and validating `onehost.json` manifests.
pub struct ManifestLoader;

impl ManifestLoader {
    /// Loads, parses, and strictly validates a manifest from a filesystem path.
    pub fn load_from_path(manifest_path: &Path) -> Result<OnehostManifest, ManifestLoadError> {
        manifest_path
            .exists()
            .then_some(())
            .ok_or_else(|| ManifestLoadError::ManifestNotFound {
                path: manifest_path.to_path_buf(),
            })?;

        let json_content = fs::read_to_string(manifest_path).map_err(|source| {
            ManifestLoadError::IoError {
                path: manifest_path.to_path_buf(),
                source,
            }
        })?;

        Self::load_from_json_string(&json_content)
    }

    /// Parses and strictly validates a manifest from a raw JSON string.
    pub fn load_from_json_string(json_content: &str) -> Result<OnehostManifest, ManifestLoadError> {
        let manifest: OnehostManifest = serde_json::from_str(json_content)
            .map_err(|source| ManifestLoadError::JsonDeserializationError { source })?;

        validate_manifest(&manifest)?;

        Ok(manifest)
    }

    /// Resolves the effective manifest path, preferring an explicit CLI argument if provided,
    /// or falling back to `$XDG_CONFIG_HOME/onehost/onehost.json`.
    pub fn resolve_manifest_path(
        xdg_directories: &XdgBaseDirectories,
        explicit_cli_path: Option<&Path>,
    ) -> Result<PathBuf, ManifestLoadError> {
        explicit_cli_path
            .map(|path| Ok(path.to_path_buf()))
            .unwrap_or_else(|| {
                let configuration_directory = xdg_directories.onehost_configuration_directory()?;
                Ok(configuration_directory.join("onehost.json"))
            })
    }
}
