use std::path::{Path, PathBuf};
use onehost::config::loader::{ManifestLoadError, ManifestLoader};
use onehost::config::model::ImageChangePolicy;
use onehost::config::validation::ManifestValidationError;

const VALID_MANIFEST_JSON: &str = r#"{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
  "version": "1.0",
  "storage": {
    "depot_store_dir": "/data/depot/virtualization/libvirt/store",
    "depot_iso_dir": "/data/depot/virtualization/libvirt/iso",
    "depot_backup_dir": "/data/depot/virtualization/libvirt/backup",
    "nvram_dir": "/var/lib/libvirt/qemu/nvram",
    "nvram_template": "/run/libvirt/nix-ovmf/edk2-i386-vars.fd",
    "ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd",
    "default_pool": "default"
  },
  "flavors": {
    "looking-glass": {
      "oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass",
      "hash": "ba6eafb7"
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e12345678-win11-template.xml",
      "image": "win11/26300.9457.pro.en-us/looking-glass",
      "pool": "default",
      "autostart": false,
      "lifecycle": {
        "on_image_change": "protect",
        "prevent_destroy": false
      }
    }
  }
}"#;

#[test]
fn test_valid_manifest_deserialization_and_validation() {
    let manifest = ManifestLoader::load_from_json_string(VALID_MANIFEST_JSON).unwrap();

    assert_eq!(manifest.version, "1.0");
    assert_eq!(
        manifest.storage.depot_store_dir,
        Path::new("/data/depot/virtualization/libvirt/store")
    );
    assert_eq!(
        manifest.storage.default_pool.as_deref(),
        Some("default")
    );

    let flavor = manifest.flavors.get("looking-glass").unwrap();
    assert_eq!(flavor.hash, "ba6eafb7");
    assert_eq!(
        flavor.oemdrv_path,
        Path::new("/nix/store/ba6eafb712345678-oemdrv-looking-glass")
    );

    let instance = manifest.instances.get("win11-gollum").unwrap();
    assert_eq!(instance.uuid, "e5a7d620-8931-4bf6-98ec-7e44a30e8c45");
    assert_eq!(instance.image.flavor_name, "looking-glass");
    assert_eq!(instance.pool.as_deref(), Some("default"));
    assert_eq!(instance.effective_pool(&manifest.storage).unwrap(), "default");
    assert!(!instance.autostart);
    assert_eq!(
        instance.lifecycle.on_image_change,
        ImageChangePolicy::Protect
    );
    assert!(!instance.lifecycle.prevent_destroy);
}

#[test]
fn test_manifest_version_must_be_exact() {
    let invalid_version_json = VALID_MANIFEST_JSON.replace(r#""version": "1.0""#, r#""version": "2.0""#);
    match ManifestLoader::load_from_json_string(&invalid_version_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::UnsupportedManifestVersion {
                supported_version,
                actual_version,
            },
        }) => {
            assert_eq!(supported_version, "1.0");
            assert_eq!(actual_version, "2.0");
        }
        other => panic!("Expected UnsupportedManifestVersion error, got {:?}", other),
    }
}

#[test]
fn test_relative_paths_in_storage_configuration_are_rejected() {
    let relative_path_json = VALID_MANIFEST_JSON.replace(
        r#""/data/depot/virtualization/libvirt/store""#,
        r#""relative/path/to/store""#,
    );
    match ManifestLoader::load_from_json_string(&relative_path_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::RelativePathNotAllowed { field_name, path },
        }) => {
            assert_eq!(field_name, "storage.depot_store_dir");
            assert_eq!(path, PathBuf::from("relative/path/to/store"));
        }
        other => panic!("Expected RelativePathNotAllowed error, got {:?}", other),
    }
}

#[test]
fn test_relative_paths_in_flavor_configuration_are_rejected() {
    let relative_flavor_json = VALID_MANIFEST_JSON.replace(
        r#""/nix/store/ba6eafb712345678-oemdrv-looking-glass""#,
        r#""relative/oemdrv/path""#,
    );
    match ManifestLoader::load_from_json_string(&relative_flavor_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::RelativePathNotAllowed { field_name, path },
        }) => {
            assert_eq!(field_name, "flavors.looking-glass.oemdrv_path");
            assert_eq!(path, PathBuf::from("relative/oemdrv/path"));
        }
        other => panic!("Expected RelativePathNotAllowed error, got {:?}", other),
    }
}

#[test]
fn test_relative_paths_in_instance_template_are_rejected() {
    let relative_template_json = VALID_MANIFEST_JSON.replace(
        r#""/nix/store/7q8w9e12345678-win11-template.xml""#,
        r#""relative/template.xml""#,
    );
    match ManifestLoader::load_from_json_string(&relative_template_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::RelativePathNotAllowed { field_name, path },
        }) => {
            assert_eq!(field_name, "instances.win11-gollum.template_xml");
            assert_eq!(path, PathBuf::from("relative/template.xml"));
        }
        other => panic!("Expected RelativePathNotAllowed error, got {:?}", other),
    }
}

#[test]
fn test_invalid_uuid_in_instance_is_rejected() {
    let invalid_uuid_json = VALID_MANIFEST_JSON.replace(
        r#""e5a7d620-8931-4bf6-98ec-7e44a30e8c45""#,
        r#""not-a-valid-uuid""#,
    );
    match ManifestLoader::load_from_json_string(&invalid_uuid_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::InvalidInstanceUuid {
                instance_name,
                raw_uuid,
            },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(raw_uuid, "not-a-valid-uuid");
        }
        other => panic!("Expected InvalidInstanceUuid error, got {:?}", other),
    }
}

#[test]
fn test_unregistered_flavor_reference_is_rejected() {
    let unregistered_flavor_json = VALID_MANIFEST_JSON.replace(
        r#""win11/26300.9457.pro.en-us/looking-glass""#,
        r#""win11/26300.9457.pro.en-us/unregistered-flavor""#,
    );
    match ManifestLoader::load_from_json_string(&unregistered_flavor_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::ReferencedFlavorNotFound {
                instance_name,
                referenced_flavor,
                available_flavors,
            },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(referenced_flavor, "unregistered-flavor");
            assert_eq!(available_flavors, vec!["looking-glass".to_string()]);
        }
        other => panic!("Expected ReferencedFlavorNotFound error, got {:?}", other),
    }
}

#[test]
fn test_missing_storage_pool_is_rejected_when_no_default_exists() {
    // Remove "default_pool" and set instance pool to null/empty
    let no_pool_json = r#"{
  "version": "1.0",
  "storage": {
    "depot_store_dir": "/data/depot/virtualization/libvirt/store",
    "depot_iso_dir": "/data/depot/virtualization/libvirt/iso",
    "depot_backup_dir": "/data/depot/virtualization/libvirt/backup",
    "nvram_dir": "/var/lib/libvirt/qemu/nvram",
    "nvram_template": "/run/libvirt/nix-ovmf/edk2-i386-vars.fd",
    "ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd"
  },
  "flavors": {
    "looking-glass": {
      "oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass",
      "hash": "ba6eafb7"
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e12345678-win11-template.xml",
      "image": "win11/26300.9457.pro.en-us/looking-glass",
      "autostart": false,
      "lifecycle": {
        "on_image_change": "protect",
        "prevent_destroy": false
      }
    }
  }
}"#;

    match ManifestLoader::load_from_json_string(no_pool_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::MissingStoragePool { instance_name },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other => panic!("Expected MissingStoragePool error, got {:?}", other),
    }
}

#[test]
fn test_effective_pool_resolution_hierarchy() {
    let manifest = ManifestLoader::load_from_json_string(VALID_MANIFEST_JSON).unwrap();
    let instance = manifest.instances.get("win11-gollum").unwrap();
    assert_eq!(instance.effective_pool(&manifest.storage).unwrap(), "default");

    // When instance has an explicit pool override, it takes precedence
    let custom_instance = instance.clone().with_pool(Some("fast-nvme".to_string()));
    assert_eq!(custom_instance.effective_pool(&manifest.storage).unwrap(), "fast-nvme");
}

#[test]
fn test_load_manifest_from_filesystem_file() {
    let temporary_directory = tempfile::tempdir().unwrap();
    let manifest_file_path = temporary_directory.path().join("onehost.json");
    std::fs::write(&manifest_file_path, VALID_MANIFEST_JSON).unwrap();

    let manifest = ManifestLoader::load_from_path(&manifest_file_path).unwrap();
    assert_eq!(manifest.version, "1.0");
    assert!(manifest.instances.contains_key("win11-gollum"));
}
