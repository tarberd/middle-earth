use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use onehost::image::tag::{
    ContentAddressedImageResolver, FlavorDerivationMetadata, FlavorResolutionError,
    ImageTagParseError, ImageTagSpecification,
};

#[test]
fn test_parse_valid_canonical_release_tuple() {
    let raw_tag = "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass";
    let parsed_tag = ImageTagSpecification::from_str(raw_tag).expect("valid tag parse");

    assert_eq!(parsed_tag.uup_id, "ce863cb6-6967-4632-a567-eb965a6f2ea3");
    assert_eq!(parsed_tag.edition, "pro");
    assert_eq!(parsed_tag.language, "en-us");
    assert_eq!(parsed_tag.flavor, "looking-glass");
    assert_eq!(parsed_tag.to_string(), raw_tag);
}

#[test]
fn test_parse_normalizes_case_and_whitespace() {
    let raw_tag = "  CE863CB6-6967-4632-A567-EB965A6F2EA3 / Pro / EN-US / looking-glass ";
    let parsed_tag = ImageTagSpecification::from_str(raw_tag).expect("normalized tag parse");

    assert_eq!(parsed_tag.uup_id, "ce863cb6-6967-4632-a567-eb965a6f2ea3");
    assert_eq!(parsed_tag.edition, "pro");
    assert_eq!(parsed_tag.language, "en-us");
    assert_eq!(parsed_tag.flavor, "looking-glass");
    assert_eq!(
        parsed_tag.to_string(),
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass"
    );
}

#[test]
fn test_rejects_legacy_informal_build_strings() {
    // Legacy 3-part tag (os/build_version/flavor) must be rejected
    let segment_error =
        ImageTagSpecification::from_str("win11/26300.9457.pro.en-us/looking-glass")
            .expect_err("Expected InvalidSegmentCount error");
    assert_eq!(
        segment_error,
        ImageTagParseError::InvalidSegmentCount {
            raw_tag: "win11/26300.9457.pro.en-us/looking-glass".to_string(),
            expected_segments: 4,
            actual_segments: 3,
        }
    );

    // Informal 4-part tag with non-UUID in first segment must be rejected
    let uup_error = ImageTagSpecification::from_str("win11/pro/en-us/looking-glass")
        .expect_err("Expected InvalidUupId error");
    assert_eq!(
        uup_error,
        ImageTagParseError::InvalidUupId {
            raw_tag: "win11/pro/en-us/looking-glass".to_string(),
            uup_id: "win11".to_string(),
        }
    );
}

#[test]
fn test_parse_malformed_tags_are_strictly_rejected() {
    // Too few segments
    assert!(matches!(
        ImageTagSpecification::from_str("ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us"),
        Err(ImageTagParseError::InvalidSegmentCount {
            expected_segments: 4,
            actual_segments: 3,
            ..
        })
    ));

    // Too many segments
    assert!(matches!(
        ImageTagSpecification::from_str(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass/extra"
        ),
        Err(ImageTagParseError::InvalidSegmentCount {
            expected_segments: 4,
            actual_segments: 5,
            ..
        })
    ));

    // Empty segments
    assert!(matches!(
        ImageTagSpecification::from_str(
            "ce863cb6-6967-4632-a567-eb965a6f2ea3//en-us/looking-glass"
        ),
        Err(ImageTagParseError::EmptySegment {
            segment_index: 1,
            ..
        })
    ));
    assert!(matches!(
        ImageTagSpecification::from_str("ce863cb6-6967-4632-a567-eb965a6f2ea3/pro//looking-glass"),
        Err(ImageTagParseError::EmptySegment {
            segment_index: 2,
            ..
        })
    ));
    assert!(matches!(
        ImageTagSpecification::from_str("ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/"),
        Err(ImageTagParseError::EmptySegment {
            segment_index: 3,
            ..
        })
    ));
}

#[test]
fn test_serde_json_roundtrip_from_string() {
    let raw_tag = "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass";
    let tag = ImageTagSpecification::from_str(raw_tag).expect("parse tag");

    let json_serialized = serde_json::to_string(&tag).expect("serialize json");
    let tag_deserialized: ImageTagSpecification =
        serde_json::from_str(&json_serialized).expect("deserialize json");
    assert_eq!(tag, tag_deserialized);

    // Verify it serializes as a structured JSON object
    let parsed_json: serde_json::Value =
        serde_json::from_str(&json_serialized).expect("parse json value");
    assert!(parsed_json.is_object());
    assert_eq!(parsed_json["uup_id"], "ce863cb6-6967-4632-a567-eb965a6f2ea3");
    assert_eq!(parsed_json["edition"], "pro");
    assert_eq!(parsed_json["language"], "en-us");
    assert_eq!(parsed_json["flavor"], "looking-glass");

    // Also verify deserialization from 4-segment string JSON
    let string_json = format!("\"{}\"", raw_tag);
    let from_string: ImageTagSpecification =
        serde_json::from_str(&string_json).expect("deserialize string json");
    assert_eq!(from_string, tag);
}

#[test]
fn test_serde_json_deserialization_from_structured_object() {
    let structured_json = r#"{
        "uup_id": "ce863cb6-6967-4632-a567-eb965a6f2ea3",
        "edition": "pro",
        "language": "en-us",
        "flavor": "looking-glass"
    }"#;

    let tag: ImageTagSpecification =
        serde_json::from_str(structured_json).expect("deserialize structured json");
    assert_eq!(tag.uup_id, "ce863cb6-6967-4632-a567-eb965a6f2ea3");
    assert_eq!(tag.edition, "pro");
    assert_eq!(tag.language, "en-us");
    assert_eq!(tag.flavor, "looking-glass");

    // Invalid UUID in structured object is rejected
    let invalid_uuid_json = r#"{
        "uup_id": "not-a-valid-uuid",
        "edition": "pro",
        "language": "en-us",
        "flavor": "looking-glass"
    }"#;
    assert!(serde_json::from_str::<ImageTagSpecification>(invalid_uuid_json).is_err());
}

#[test]
fn test_structured_image_deserialization_rejects_unknown_fields() {
    let structured_json_with_unknown = r#"{
        "uup_id": "ce863cb6-6967-4632-a567-eb965a6f2ea3",
        "edition": "pro",
        "language": "en-us",
        "flavor": "looking-glass",
        "unexpected_field": 123
    }"#;
    assert!(serde_json::from_str::<ImageTagSpecification>(structured_json_with_unknown).is_err());
}

#[test]
fn test_parse_rejects_invalid_bcp47_language_tags() {
    let invalid_tags = [
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/123/looking-glass",
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/invalid_tag!/looking-glass",
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/e/looking-glass",
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/english-unitedstates/looking-glass",
    ];
    let all_invalid = invalid_tags.iter().all(|tag_str| {
        matches!(
            ImageTagSpecification::from_str(tag_str),
            Err(ImageTagParseError::InvalidLanguage { .. })
        )
    });
    assert!(all_invalid, "Expected InvalidLanguage error for all invalid tags");
}

#[test]
fn test_resolve_golden_master_and_iso_artifact_paths() {
    let tag = ImageTagSpecification::from_str(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
    )
    .expect("parse valid tag");
    let flavor_metadata = FlavorDerivationMetadata::new(
        PathBuf::from("/nix/store/ba6eafb712345678-oemdrv-looking-glass"),
        "ba6eafb71234".to_string(),
    )
    .expect("valid flavor metadata");

    let master_filename =
        ContentAddressedImageResolver::resolve_golden_master_filename(&tag, &flavor_metadata);
    assert_eq!(
        master_filename,
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
    );

    let iso_filename = ContentAddressedImageResolver::resolve_iso_filename(&tag);
    assert_eq!(
        iso_filename,
        "ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us.iso"
    );

    let depot_store_directory = Path::new("/data/depot/virtualization/libvirt/store");
    let depot_path = ContentAddressedImageResolver::resolve_depot_master_path(
        depot_store_directory,
        &tag,
        &flavor_metadata,
    );
    assert_eq!(
        depot_path,
        PathBuf::from(
            "/data/depot/virtualization/libvirt/store/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
        )
    );

    let depot_iso_directory = Path::new("/data/depot/virtualization/libvirt/iso");
    let depot_iso_path =
        ContentAddressedImageResolver::resolve_depot_iso_path(depot_iso_directory, &tag);
    assert_eq!(
        depot_iso_path,
        PathBuf::from(
            "/data/depot/virtualization/libvirt/iso/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us.iso"
        )
    );

    let storage_pool_directory = Path::new("/var/lib/libvirt/images");
    let pool_base_path = ContentAddressedImageResolver::resolve_storage_pool_base_path(
        storage_pool_directory,
        &tag,
        &flavor_metadata,
    );
    assert_eq!(
        pool_base_path,
        PathBuf::from(
            "/var/lib/libvirt/images/ce863cb6-6967-4632-a567-eb965a6f2ea3-pro-en-us-looking-glass-ba6eafb71234.qcow2"
        )
    );

    let instance_overlay_path = ContentAddressedImageResolver::resolve_instance_overlay_path(
        storage_pool_directory,
        "win11-gollum",
    );
    assert_eq!(
        instance_overlay_path,
        PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")
    );
}

#[test]
fn test_resolve_with_flavor_registry() {
    let flavor_registry: HashMap<String, FlavorDerivationMetadata> = [(
        "looking-glass".to_string(),
        FlavorDerivationMetadata::new(
            PathBuf::from("/nix/store/ba6eafb712345678-oemdrv-looking-glass"),
            "ba6eafb71234".to_string(),
        )
        .expect("valid metadata"),
    )]
    .into_iter()
    .collect();

    let valid_tag = ImageTagSpecification::from_str(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/looking-glass",
    )
    .expect("parse valid tag");
    let resolved_metadata =
        ContentAddressedImageResolver::lookup_flavor(&valid_tag, &flavor_registry)
            .expect("lookup valid flavor");
    assert_eq!(resolved_metadata.content_hash, "ba6eafb71234");

    let unknown_flavor_tag = ImageTagSpecification::from_str(
        "ce863cb6-6967-4632-a567-eb965a6f2ea3/pro/en-us/unregistered",
    )
    .expect("parse unknown flavor tag");
    let resolution_error =
        ContentAddressedImageResolver::lookup_flavor(&unknown_flavor_tag, &flavor_registry)
            .expect_err("Expected UnknownFlavor error");
    assert_eq!(
        resolution_error,
        FlavorResolutionError::UnknownFlavor {
            requested_flavor: "unregistered".to_string(),
            available_flavors: vec!["looking-glass".to_string()],
        }
    );
}

#[test]
fn test_flavor_derivation_metadata_validation() {
    // Relative nix store path is rejected
    let relative_path_result = FlavorDerivationMetadata::new(
        PathBuf::from("relative/nix/store/path"),
        "ba6eafb71234".to_string(),
    );
    assert!(matches!(
        relative_path_result,
        Err(FlavorResolutionError::RelativeNixStorePathNotAllowed { .. })
    ));

    // Empty content hash is rejected
    let empty_hash_result = FlavorDerivationMetadata::new(
        PathBuf::from("/nix/store/ba6eafb712345678-oemdrv-looking-glass"),
        "".to_string(),
    );
    assert!(matches!(
        empty_hash_result,
        Err(FlavorResolutionError::EmptyContentHash)
    ));
}
