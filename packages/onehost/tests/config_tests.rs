use onehost::config::loader::{ManifestLoadError, ManifestLoader};
use onehost::config::model::InstanceUuid;
use onehost::config::validation::ManifestValidationError;
use std::path::{Path, PathBuf};

const VALID_MANIFEST_JSON: &str = r#"{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
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
      "extra_isos": {
        "virtio": "/nix/store/1111111111111111-virtio-win.iso"
      }
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e12345678-win11-template.xml",
      "image": "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
      "pool": "default"
    }
  }
}"#;

#[test]
fn test_valid_manifest_deserialization_and_validation() {
    let manifest = ManifestLoader::load_from_json_string(VALID_MANIFEST_JSON)
        .expect("valid manifest should parse");

    assert_eq!(manifest.version, "1.0");
    assert_eq!(
        manifest.storage.depot_store_dir,
        Path::new("/data/depot/virtualization/libvirt/store")
    );

    let flavor = manifest
        .flavors
        .get("looking-glass")
        .expect("looking-glass flavor");
    assert_eq!(
        flavor.oemdrv_path,
        Path::new("/nix/store/ba6eafb712345678-oemdrv-looking-glass")
    );
    assert_eq!(
        flavor.extra_isos.get("virtio").expect("virtio extra iso"),
        Path::new("/nix/store/1111111111111111-virtio-win.iso")
    );

    let instance = manifest.instances.get("win11-gollum").expect("instance");
    assert_eq!(instance.uuid, "e5a7d620-8931-4bf6-98ec-7e44a30e8c45");
    assert_eq!(instance.image.flavor, "looking-glass");
    assert_eq!(
        instance.image.uup_id,
        "ce863cb6-6967-4632-a567-eb965a6f2ea3"
    );
    assert_eq!(instance.image.edition, "pro");
    assert_eq!(instance.image.language, "en-us");
    assert_eq!(instance.pool, "default");
    assert_eq!(
        instance.effective_pool("win11-gollum").expect("effective pool"),
        "default"
    );
}

#[test]
fn test_manifest_supports_structured_image_object() {
    let structured_image_manifest_json = VALID_MANIFEST_JSON.replace(
        r#""image": "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass""#,
        r#""image": {
        "uup_id": "ce863cb6-6967-4632-a567-eb965a6f2ea3",
        "edition": "pro",
        "language": "en-us",
        "flavor": "looking-glass"
      }"#,
    );

    let manifest = ManifestLoader::load_from_json_string(&structured_image_manifest_json)
        .expect("Manifest with structured image object should parse successfully");

    let instance = manifest.instances.get("win11-gollum").expect("instance");
    assert_eq!(instance.image.uup_id, "ce863cb6-6967-4632-a567-eb965a6f2ea3");
    assert_eq!(instance.image.edition, "pro");
    assert_eq!(instance.image.language, "en-us");
    assert_eq!(instance.image.flavor, "looking-glass");

    // Serialization of manifest serializes image as structured object
    let serialized_json = serde_json::to_string(&manifest).expect("manifest should serialize");
    assert!(
        serialized_json.contains(r#""uup_id":"ce863cb6-6967-4632-a567-eb965a6f2ea3""#),
        "Serialized manifest should contain structured image fields"
    );
}

#[test]
fn test_manifest_rejects_legacy_hash_field_in_flavor() {
    let legacy_hash_json = VALID_MANIFEST_JSON.replace(
        r#""oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass","#,
        r#""oemdrv_path": "/nix/store/ba6eafb712345678-oemdrv-looking-glass", "hash": "ba6eafb7","#,
    );
    match ManifestLoader::load_from_json_string(&legacy_hash_json) {
        Err(ManifestLoadError::JsonDeserializationError { source }) => {
            assert!(
                source.to_string().contains("unknown field `hash`"),
                "Expected unknown field hash error, got: {source}"
            );
        }
        other => {
            unreachable!("Expected JsonDeserializationError for legacy hash, got {other:?}");
        }
    }
}

#[test]
fn test_manifest_rejects_legacy_default_pool_in_storage() {
    let legacy_default_pool_json = VALID_MANIFEST_JSON.replace(
        r#""ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd""#,
        r#""ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd", "default_pool": "default""#,
    );
    match ManifestLoader::load_from_json_string(&legacy_default_pool_json) {
        Err(ManifestLoadError::JsonDeserializationError { source }) => {
            assert!(
                source.to_string().contains("unknown field `default_pool`"),
                "Expected unknown field default_pool error, got: {source}"
            );
        }
        other => {
            unreachable!(
                "Expected JsonDeserializationError for legacy default_pool, got {other:?}"
            );
        }
    }
}

#[test]
fn test_manifest_rejects_legacy_autostart_in_instance() {
    let legacy_autostart_json = VALID_MANIFEST_JSON.replace(
        r#""pool": "default""#,
        r#""pool": "default", "autostart": true"#,
    );
    match ManifestLoader::load_from_json_string(&legacy_autostart_json) {
        Err(ManifestLoadError::JsonDeserializationError { source }) => {
            assert!(
                source.to_string().contains("unknown field `autostart`"),
                "Expected unknown field autostart error, got: {source}"
            );
        }
        other => {
            unreachable!("Expected JsonDeserializationError for legacy autostart, got {other:?}");
        }
    }
}

#[test]
fn test_manifest_rejects_legacy_lifecycle_block_in_instance() {
    let legacy_lifecycle_json = VALID_MANIFEST_JSON.replace(
        r#""pool": "default""#,
        r#""pool": "default", "lifecycle": { "prevent_destroy": false, "on_image_change": "protect" }"#,
    );
    match ManifestLoader::load_from_json_string(&legacy_lifecycle_json) {
        Err(ManifestLoadError::JsonDeserializationError { source }) => {
            assert!(
                source.to_string().contains("unknown field `lifecycle`"),
                "Expected unknown field lifecycle error, got: {source}"
            );
        }
        other => {
            unreachable!("Expected JsonDeserializationError for legacy lifecycle, got {other:?}");
        }
    }
}

#[test]
fn test_instance_requires_explicit_pool() {
    // Missing pool field in instance is rejected by serde
    let missing_pool_json = VALID_MANIFEST_JSON.replace(",\n      \"pool\": \"default\"", "");
    match ManifestLoader::load_from_json_string(&missing_pool_json) {
        Err(ManifestLoadError::JsonDeserializationError { source }) => {
            assert!(
                source.to_string().contains("missing field `pool`"),
                "Expected missing field pool error, got: {source}"
            );
        }
        other => {
            unreachable!("Expected JsonDeserializationError for missing pool, got {other:?}");
        }
    }

    // Empty pool string is rejected by validate_manifest
    let empty_pool_json = VALID_MANIFEST_JSON.replace(r#""pool": "default""#, r#""pool": "   ""#);
    match ManifestLoader::load_from_json_string(&empty_pool_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::MissingStoragePool { instance_name },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
        }
        other => unreachable!("Expected MissingStoragePool error, got {other:?}"),
    }
}

#[test]
fn test_manifest_version_must_be_exact() {
    let invalid_version_json =
        VALID_MANIFEST_JSON.replace(r#""version": "1.0""#, r#""version": "2.0""#);
    match ManifestLoader::load_from_json_string(&invalid_version_json) {
        Err(ManifestLoadError::ValidationError {
            source:
                ManifestValidationError::UnsupportedManifestVersion {
                    supported_version,
                    actual_version,
                },
        }) => {
            assert_eq!(supported_version, "1.0");
            assert_eq!(actual_version, "2.0");
        }
        other => unreachable!("Expected UnsupportedManifestVersion error, got {other:?}"),
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
        other => unreachable!("Expected RelativePathNotAllowed error, got {other:?}"),
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
        other => unreachable!("Expected RelativePathNotAllowed error, got {other:?}"),
    }

    // Relative path in extra_isos
    let relative_extra_iso_json = VALID_MANIFEST_JSON.replace(
        r#""/nix/store/1111111111111111-virtio-win.iso""#,
        r#""relative/virtio.iso""#,
    );
    match ManifestLoader::load_from_json_string(&relative_extra_iso_json) {
        Err(ManifestLoadError::ValidationError {
            source: ManifestValidationError::RelativePathNotAllowed { field_name, path },
        }) => {
            assert_eq!(field_name, "flavors.looking-glass.extra_isos.virtio");
            assert_eq!(path, PathBuf::from("relative/virtio.iso"));
        }
        other => unreachable!("Expected RelativePathNotAllowed error, got {other:?}"),
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
        other => unreachable!("Expected RelativePathNotAllowed error, got {other:?}"),
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
            source:
                ManifestValidationError::InvalidInstanceUuid {
                    instance_name,
                    raw_uuid,
                },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(raw_uuid, "not-a-valid-uuid");
        }
        other => unreachable!("Expected InvalidInstanceUuid error, got {other:?}"),
    }
}

#[test]
fn test_unregistered_flavor_reference_is_rejected() {
    let unregistered_flavor_json = VALID_MANIFEST_JSON.replace(
        r#""ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass""#,
        r#""ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/unregistered-flavor""#,
    );
    match ManifestLoader::load_from_json_string(&unregistered_flavor_json) {
        Err(ManifestLoadError::ValidationError {
            source:
                ManifestValidationError::ReferencedFlavorNotFound {
                    instance_name,
                    referenced_flavor,
                    available_flavors,
                },
        }) => {
            assert_eq!(instance_name, "win11-gollum");
            assert_eq!(referenced_flavor, "unregistered-flavor");
            assert_eq!(available_flavors, vec!["looking-glass".to_string()]);
        }
        other => unreachable!("Expected ReferencedFlavorNotFound error, got {other:?}"),
    }
}

#[test]
fn test_load_manifest_from_filesystem_file() {
    let temporary_directory = tempfile::tempdir().expect("tempdir");
    let manifest_file_path = temporary_directory.path().join("onehost.json");
    std::fs::write(&manifest_file_path, VALID_MANIFEST_JSON).expect("write manifest");

    let manifest = ManifestLoader::load_from_path(&manifest_file_path).expect("load manifest");
    assert_eq!(manifest.version, "1.0");
    assert!(manifest.instances.contains_key("win11-gollum"));
}

#[test]
fn test_born_valid_instance_uuid_parsing_and_invariants() {
    let valid_uuid_literal = "e5a7d620-8931-4bf6-98ec-7e44a30e8c45";
    let instance_uuid = InstanceUuid::parse(valid_uuid_literal).expect("Valid UUID should parse");

    assert_eq!(instance_uuid.as_str(), valid_uuid_literal);
    assert_eq!(instance_uuid, valid_uuid_literal);
    assert_eq!(valid_uuid_literal, instance_uuid);
    assert_eq!(&*instance_uuid, valid_uuid_literal);
    assert_eq!(format!("{instance_uuid}"), valid_uuid_literal);

    // Invalid length / segments
    assert!(InstanceUuid::parse("not-a-uuid").is_err());
    assert!(InstanceUuid::parse("e5a7d620-8931-4bf6-98ec").is_err());
    assert!(InstanceUuid::parse("e5a7d620-8931-4bf6-98ec-7e44a30e8c45-extra").is_err());
    // Invalid non-hex characters
    assert!(InstanceUuid::parse("g5a7d620-8931-4bf6-98ec-7e44a30e8c45").is_err());
    assert!(InstanceUuid::parse("e5a7d620-8931-4bf6-98ec-7e44a30e8c4z").is_err());

    // TryFrom and FromStr
    let from_str_uuid: InstanceUuid = valid_uuid_literal.parse().expect("FromStr must succeed");
    assert_eq!(instance_uuid, from_str_uuid);

    let try_from_uuid = InstanceUuid::try_from(valid_uuid_literal).expect("TryFrom must succeed");
    assert_eq!(instance_uuid, try_from_uuid);
}
