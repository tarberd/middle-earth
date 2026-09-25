use onehost::domain::DomainXmlElement;

const COMPLEX_DOMAIN_XML: &str = r#"<domain type="kvm" xmlns:qemu="http://libvirt.org/schemas/domain/qemu/1.0" xmlns:onehost="https://middle-earth.internal/onehost">
  <name>win11-gollum</name>
  <uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>
  <memory unit="KiB">16777216</memory>
  <vcpu placement="static">16</vcpu>
  <os>
    <type arch="x86_64" machine="q35">hvm</type>
    <loader readonly="yes" type="pflash">/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template="/run/libvirt/nix-ovmf/edk2-i386-vars.fd">/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd</nvram>
  </os>
  <devices>
    <emulator>/usr/bin/qemu-system-x86_64</emulator>
    <disk device="disk" onehost:role="os-disk" type="file">
      <target bus="sata" dev="sda"/>
      <driver name="qemu" type="qcow2"/>
      <source file="/var/lib/libvirt/images/win11-gollum.qcow2"/>
    </disk>
    <interface type="bridge">
      <source bridge="br0"/>
      <model type="virtio"/>
    </interface>
  </devices>
</domain>"#;

#[test]
fn test_round_trip_ast_invariance_complex_domain_xml() {
    let original_ast = DomainXmlElement::parse(COMPLEX_DOMAIN_XML)
        .expect("Failed to parse original complex domain XML");

    let serialized_xml = original_ast.to_xml_string(0);
    let reparsed_ast = DomainXmlElement::parse(&serialized_xml)
        .expect("Failed to re-parse serialized domain XML");

    assert_eq!(
        original_ast, reparsed_ast,
        "AST round-trip must preserve complete structural and attribute invariance"
    );
}

#[test]
fn test_empty_element_self_closing_formatting() {
    let empty_target = DomainXmlElement::new("target")
        .with_attribute("bus", "sata")
        .with_attribute("dev", "sda");

    let formatted = empty_target.to_xml_string(0);
    assert_eq!(formatted, "<target bus=\"sata\" dev=\"sda\"/>\n");

    let parsed = DomainXmlElement::parse(&formatted).expect("Failed to parse empty element");
    assert_eq!(empty_target, parsed);
}

#[test]
fn test_text_element_formatting() {
    let domain_name = DomainXmlElement::with_text_node("name", "win11-gollum");

    let formatted = domain_name.to_xml_string(0);
    assert_eq!(formatted, "<name>win11-gollum</name>\n");

    let parsed = DomainXmlElement::parse(&formatted).expect("Failed to parse text element");
    assert_eq!(domain_name, parsed);
}

#[test]
fn test_destructuring_and_zero_copy_move_semantics() {
    let element = DomainXmlElement::new("disk")
        .with_attribute("type", "file")
        .with_attribute("device", "disk")
        .with_child(DomainXmlElement::with_text_node("driver", "qemu"));

    let DomainXmlElement {
        tag_name,
        attributes,
        children,
        text_content,
    } = element;

    assert_eq!(tag_name, "disk");
    assert_eq!(attributes.len(), 2);
    assert_eq!(children.len(), 1);
    assert!(text_content.is_none());
}

#[test]
fn test_builder_methods_functional_immutability() {
    let base = DomainXmlElement::new("domain")
        .with_attribute("type", "kvm")
        .with_child(DomainXmlElement::with_text_node("name", "initial-name"));

    // Verify with_replaced_text
    let renamed = base.clone().transform_children(|child| {
        if child.tag_name == "name" {
            Some(child.with_replaced_text("renamed-vm"))
        } else {
            Some(child)
        }
    });

    assert_eq!(
        base.find_child_by_tag("name").and_then(|name_node| name_node.text_content.as_deref()),
        Some("initial-name")
    );
    assert_eq!(
        renamed.find_child_by_tag("name").and_then(|name_node| name_node.text_content.as_deref()),
        Some("renamed-vm")
    );

    // Verify without_attribute
    let without_attr = base.clone().without_attribute("type");
    assert_eq!(base.get_attribute("type"), Some("kvm"));
    assert_eq!(without_attr.get_attribute("type"), None);

    // Verify without_child_tag_recursive
    let without_name = base.without_child_tag_recursive("name");
    assert!(!without_name.has_child("name"));
}

#[test]
fn test_malformed_xml_edge_cases() {
    // 1. Empty content
    assert!(DomainXmlElement::parse("").is_err());
    assert!(DomainXmlElement::parse("   \n\t  ").is_err());

    // 2. Unclosed root
    assert!(DomainXmlElement::parse("<domain><name>test</name>").is_err());

    // 3. Mismatched closing tag
    assert!(DomainXmlElement::parse("<domain><name>test</other></domain>").is_err());

    // 4. Multiple roots
    assert!(DomainXmlElement::parse("<domain/><domain/>").is_err());

    // 5. Unexpected text after root
    assert!(DomainXmlElement::parse("<domain/>trailing text").is_err());
}
