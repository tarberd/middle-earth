use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use onehost::image::tag::{
    ContentAddressedImageResolver, FlavorDerivationMetadata, FlavorResolutionError,
    ImageTagParseError, ImageTagSpecification,
};

#[test]
fn test_parse_valid_image_tag() {
    let raw_tag = "win11/26300.9457.pro.en-us/looking-glass";
    let parsed_tag = ImageTagSpecification::from_str(raw_tag).unwrap();

    assert_eq!(parsed_tag.operating_system, "win11");
    assert_eq!(parsed_tag.build_version, "26300.9457.pro.en-us");
    assert_eq!(parsed_tag.flavor_name, "looking-glass");
    assert_eq!(parsed_tag.to_string(), raw_tag);
}

#[test]
fn test_parse_malformed_tags_are_strictly_rejected() {
    // Legacy 4-part tag with manual revision must be rejected (NO backwards compatibility)
    match ImageTagSpecification::from_str("win11/26300.9457.pro.en-us/looking-glass/v2") {
        Err(ImageTagParseError::InvalidSegmentCount {
            raw_tag,
            expected_segments,
            actual_segments,
        }) => {
            assert_eq!(raw_tag, "win11/26300.9457.pro.en-us/looking-glass/v2");
            assert_eq!(expected_segments, 3);
            assert_eq!(actual_segments, 4);
        }
        other => panic!("Expected InvalidSegmentCount error, got {:?}", other),
    }

    // Too few segments
    assert!(matches!(
        ImageTagSpecification::from_str("win11/26300"),
        Err(ImageTagParseError::InvalidSegmentCount { .. })
    ));
    assert!(matches!(
        ImageTagSpecification::from_str("win11"),
        Err(ImageTagParseError::InvalidSegmentCount { .. })
    ));

    // Empty segments
    assert!(matches!(
        ImageTagSpecification::from_str("win11//looking-glass"),
        Err(ImageTagParseError::EmptySegment { segment_index: 1, .. })
    ));
    assert!(matches!(
        ImageTagSpecification::from_str("/26300/looking-glass"),
        Err(ImageTagParseError::EmptySegment { segment_index: 0, .. })
    ));
    assert!(matches!(
        ImageTagSpecification::from_str("win11/26300/"),
        Err(ImageTagParseError::EmptySegment { segment_index: 2, .. })
    ));
}

#[test]
fn test_serde_json_roundtrip_for_image_tag() {
    let raw_tag = "win11/26300.9457.pro.en-us/looking-glass";
    let tag = ImageTagSpecification::from_str(raw_tag).unwrap();

    let json_serialized = serde_json::to_string(&tag).unwrap();
    assert_eq!(json_serialized, format!("\"{}\"", raw_tag));

    let tag_deserialized: ImageTagSpecification = serde_json::from_str(&json_serialized).unwrap();
    assert_eq!(tag, tag_deserialized);
}

#[test]
fn test_resolve_golden_master_artifact_paths() {
    let tag = ImageTagSpecification::from_str("win11/26300.9457.pro.en-us/looking-glass").unwrap();
    let flavor_metadata = FlavorDerivationMetadata::new(
        PathBuf::from("/nix/store/ba6eafb712345678-oemdrv-looking-glass"),
        "ba6eafb7".to_string(),
    )
    .unwrap();

    let filename = ContentAddressedImageResolver::resolve_golden_master_filename(&tag, &flavor_metadata);
    assert_eq!(
        filename,
        "win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2"
    );

    let depot_store_directory = Path::new("/data/depot/virtualization/libvirt/store");
    let depot_path = ContentAddressedImageResolver::resolve_depot_master_path(
        depot_store_directory,
        &tag,
        &flavor_metadata,
    );
    assert_eq!(
        depot_path,
        PathBuf::from("/data/depot/virtualization/libvirt/store/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2")
    );

    let storage_pool_directory = Path::new("/var/lib/libvirt/images");
    let pool_base_path = ContentAddressedImageResolver::resolve_storage_pool_base_path(
        storage_pool_directory,
        &tag,
        &flavor_metadata,
    );
    assert_eq!(
        pool_base_path,
        PathBuf::from("/var/lib/libvirt/images/win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2")
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
            "ba6eafb7".to_string(),
        )
        .unwrap(),
    )]
    .into_iter()
    .collect();

    let valid_tag = ImageTagSpecification::from_str("win11/26300.9457.pro.en-us/looking-glass").unwrap();
    let resolved_metadata = ContentAddressedImageResolver::lookup_flavor(&valid_tag, &flavor_registry).unwrap();
    assert_eq!(resolved_metadata.content_hash, "ba6eafb7");

    let unknown_flavor_tag = ImageTagSpecification::from_str("win11/26300.9457.pro.en-us/unregistered").unwrap();
    match ContentAddressedImageResolver::lookup_flavor(&unknown_flavor_tag, &flavor_registry) {
        Err(FlavorResolutionError::UnknownFlavor {
            requested_flavor,
            available_flavors,
        }) => {
            assert_eq!(requested_flavor, "unregistered");
            assert_eq!(available_flavors, vec!["looking-glass".to_string()]);
        }
        other => panic!("Expected UnknownFlavor error, got {:?}", other),
    }
}

#[test]
fn test_flavor_derivation_metadata_validation() {
    // Relative nix store path is rejected
    let relative_path_result = FlavorDerivationMetadata::new(
        PathBuf::from("relative/nix/store/path"),
        "ba6eafb7".to_string(),
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
