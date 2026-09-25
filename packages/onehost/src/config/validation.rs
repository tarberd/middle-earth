use std::path::PathBuf;
use thiserror::Error;

use crate::config::model::OnehostManifest;

/// Enumerates validation failures for a declared `onehost.json` manifest.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestValidationError {
    #[error(
        "Unsupported manifest version '{actual_version}': this version of onehost strictly requires version '{supported_version}'"
    )]
    UnsupportedManifestVersion {
        supported_version: String,
        actual_version: String,
    },

    #[error("Field '{field_name}' must be an absolute path, but got relative path: {path}")]
    RelativePathNotAllowed {
        field_name: String,
        path: PathBuf,
    },

    #[error("Field '{field_name}' must not be empty")]
    EmptyFieldNotAllowed { field_name: String },

    #[error(
        "Instance '{instance_name}' declares invalid RFC-4122 UUID '{raw_uuid}'"
    )]
    InvalidInstanceUuid {
        instance_name: String,
        raw_uuid: String,
    },

    #[error(
        "Instance '{instance_name}' references unknown flavor '{referenced_flavor}'. Available flavors: {available_flavors:?}"
    )]
    ReferencedFlavorNotFound {
        instance_name: String,
        referenced_flavor: String,
        available_flavors: Vec<String>,
    },

    #[error(
        "Instance '{instance_name}' has no storage pool specified and storage.default_pool is unset (zero implicit defaults)"
    )]
    MissingStoragePool { instance_name: String },
}

/// Validates an in-memory `OnehostManifest` according to strict declarative integrity invariants.
pub fn validate_manifest(manifest: &OnehostManifest) -> Result<(), ManifestValidationError> {
    // 1. Version enforcement (NO backwards compatibility)
    (manifest.version == "1.0")
        .then_some(())
        .ok_or_else(|| ManifestValidationError::UnsupportedManifestVersion {
            supported_version: "1.0".to_string(),
            actual_version: manifest.version.clone(),
        })?;

    // 2. Storage directory absolute path validation
    let storage = &manifest.storage;
    validate_absolute_path("storage.depot_store_dir", &storage.depot_store_dir)?;
    validate_absolute_path("storage.depot_iso_dir", &storage.depot_iso_dir)?;
    validate_absolute_path("storage.depot_backup_dir", &storage.depot_backup_dir)?;
    validate_absolute_path("storage.nvram_dir", &storage.nvram_dir)?;
    validate_absolute_path("storage.nvram_template", &storage.nvram_template)?;
    validate_absolute_path("storage.ovmf_code", &storage.ovmf_code)?;

    // 3. Flavor validation
    manifest.flavors.iter().try_for_each(|(flavor_name, flavor_config)| {
        validate_absolute_path(
            &format!("flavors.{flavor_name}.oemdrv_path"),
            &flavor_config.oemdrv_path,
        )?;
        (!flavor_config.hash.trim().is_empty())
            .then_some(())
            .ok_or_else(|| ManifestValidationError::EmptyFieldNotAllowed {
                field_name: format!("flavors.{flavor_name}.hash"),
            })
    })?;

    // 4. Instance validation
    let available_flavors: Vec<String> = manifest
        .flavors
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    manifest.instances.iter().try_for_each(|(instance_name, instance_config)| {
        (!instance_name.trim().is_empty())
            .then_some(())
            .ok_or_else(|| ManifestValidationError::EmptyFieldNotAllowed {
                field_name: "instance name key".to_string(),
            })?;

        // Validate UUID syntax (8-4-4-4-12 hex characters)
        validate_rfc4122_uuid(instance_name, &instance_config.uuid)?;

        // Validate template XML path
        validate_absolute_path(
            &format!("instances.{instance_name}.template_xml"),
            &instance_config.template_xml,
        )?;

        // Validate image flavor existence in flavors map
        let referenced_flavor = &instance_config.image.flavor_name;
        manifest.flavors.contains_key(referenced_flavor)
            .then_some(())
            .ok_or_else(|| ManifestValidationError::ReferencedFlavorNotFound {
                instance_name: instance_name.clone(),
                referenced_flavor: referenced_flavor.clone(),
                available_flavors: available_flavors.clone(),
            })?;

        // Validate storage pool resolution (zero implicit defaults)
        instance_config
            .effective_pool(storage)
            .map_err(|_| ManifestValidationError::MissingStoragePool {
                instance_name: instance_name.clone(),
            })?;

        Ok(())
    })?;

    Ok(())
}

fn validate_absolute_path(
    field_name: &str,
    path: &std::path::Path,
) -> Result<(), ManifestValidationError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ManifestValidationError::RelativePathNotAllowed {
            field_name: field_name.to_string(),
            path: path.to_path_buf(),
        })
    }
}

fn validate_rfc4122_uuid(
    instance_name: &str,
    raw_uuid: &str,
) -> Result<(), ManifestValidationError> {
    let uuid_segments: Vec<&str> = raw_uuid.split('-').collect();
    let is_valid = uuid_segments.len() == 5
        && uuid_segments[0].len() == 8
        && uuid_segments[1].len() == 4
        && uuid_segments[2].len() == 4
        && uuid_segments[3].len() == 4
        && uuid_segments[4].len() == 12
        && uuid_segments
            .iter()
            .all(|segment| segment.chars().all(|character| character.is_ascii_hexdigit()));

    if is_valid {
        Ok(())
    } else {
        Err(ManifestValidationError::InvalidInstanceUuid {
            instance_name: instance_name.to_string(),
            raw_uuid: raw_uuid.to_string(),
        })
    }
}
