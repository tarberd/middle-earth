use onehost::domain::diff::{
    DomainXmlDiffError, DomainXmlDifference, DomainXmlDiffer,
};

const BASE_SYNTHESIZED_XML: &str = r#"<domain type='kvm' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
  <name>win11-gollum</name>
  <uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>
  <memory unit='KiB'>16777216</memory>
  <currentMemory unit='KiB'>16777216</currentMemory>
  <vcpu placement='static'>16</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-9.1'>hvm</type>
    <loader readonly='yes' type='pflash'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd'>/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd</nvram>
  </os>
  <devices>
    <emulator>/run/libvirt/nix-emulators/qemu-system-x86_64</emulator>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/var/lib/libvirt/images/win11-gollum.qcow2'/>
      <target dev='sda' bus='sata'/>
    </disk>
    <interface type='bridge'>
      <source bridge='br-public-hosts'/>
      <model type='virtio'/>
    </interface>
  </devices>
  <qemu:commandline>
    <qemu:arg value='-device'/>
    <qemu:arg value='ivshmem-plain,memdev=looking-glass'/>
  </qemu:commandline>
</domain>"#;

#[test]
fn test_identical_domain_xmls_have_no_drift() {
    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, BASE_SYNTHESIZED_XML)
            .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_runtime_volatile_domain_id_and_aliases_are_normalized() {
    let live_xml_with_runtime_volatiles = r#"<domain type='kvm' id='42' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
  <name>win11-gollum</name>
  <uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>
  <memory unit='KiB'>16777216</memory>
  <currentMemory unit='KiB'>16777216</currentMemory>
  <vcpu placement='static'>16</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-9.1'>hvm</type>
    <loader readonly='yes' type='pflash'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd'>/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd</nvram>
  </os>
  <devices>
    <emulator>/run/libvirt/nix-emulators/qemu-system-x86_64</emulator>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/var/lib/libvirt/images/win11-gollum.qcow2'/>
      <target dev='sda' bus='sata'/>
      <alias name='sata0-0-0'/>
    </disk>
    <interface type='bridge'>
      <source bridge='br-public-hosts'/>
      <model type='virtio'/>
      <alias name='net0'/>
    </interface>
  </devices>
  <qemu:commandline>
    <qemu:arg value='-device'/>
    <qemu:arg value='ivshmem-plain,memdev=looking-glass'/>
  </qemu:commandline>
</domain>"#;

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, live_xml_with_runtime_volatiles)
            .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_auto_generated_pci_address_is_ignored_when_not_in_synthesized() {
    let live_xml_with_auto_address = r#"<domain type='kvm' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
  <name>win11-gollum</name>
  <uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>
  <memory unit='KiB'>16777216</memory>
  <currentMemory unit='KiB'>16777216</currentMemory>
  <vcpu placement='static'>16</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-9.1'>hvm</type>
    <loader readonly='yes' type='pflash'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd'>/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd</nvram>
  </os>
  <devices>
    <emulator>/run/libvirt/nix-emulators/qemu-system-x86_64</emulator>
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/var/lib/libvirt/images/win11-gollum.qcow2'/>
      <target dev='sda' bus='sata'/>
      <address type='drive' controller='0' bus='0' target='0' unit='0'/>
    </disk>
    <interface type='bridge'>
      <source bridge='br-public-hosts'/>
      <model type='virtio'/>
      <address type='pci' domain='0x0000' bus='0x01' slot='0x00' function='0x0'/>
    </interface>
  </devices>
  <qemu:commandline>
    <qemu:arg value='-device'/>
    <qemu:arg value='ivshmem-plain,memdev=looking-glass'/>
  </qemu:commandline>
</domain>"#;

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, live_xml_with_auto_address)
            .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_explicit_pci_address_drift_is_detected_when_in_synthesized() {
    let synthesized_with_address = BASE_SYNTHESIZED_XML.replace(
        "<target dev='sda' bus='sata'/>",
        "<target dev='sda' bus='sata'/>\n      <address type='drive' controller='0' bus='0' target='0' unit='0'/>",
    );

    let live_with_different_address = BASE_SYNTHESIZED_XML.replace(
        "<target dev='sda' bus='sata'/>",
        "<target dev='sda' bus='sata'/>\n      <address type='drive' controller='0' bus='0' target='0' unit='1'/>",
    );

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(&synthesized_with_address, &live_with_different_address)
            .expect("Diff comparison failed");

    assert!(diff_result.has_drift);
    let has_unit_difference = diff_result.differences.iter().any(|difference| match difference {
        DomainXmlDifference::AttributeMismatch {
            attribute_name,
            expected,
            actual,
            ..
        } => attribute_name == "unit" && expected == "0" && actual == "1",
        _ => false,
    });
    assert!(has_unit_difference);
}

#[test]
fn test_auto_generated_mac_is_ignored_when_not_in_synthesized() {
    let live_xml_with_auto_mac = BASE_SYNTHESIZED_XML.replace(
        "<model type='virtio'/>",
        "<mac address='52:54:00:12:34:56'/>\n      <model type='virtio'/>",
    );

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, &live_xml_with_auto_mac)
            .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_explicit_mac_drift_is_detected_when_in_synthesized() {
    let synthesized_with_mac = BASE_SYNTHESIZED_XML.replace(
        "<model type='virtio'/>",
        "<mac address='52:54:00:aa:bb:cc'/>\n      <model type='virtio'/>",
    );

    let live_with_different_mac = BASE_SYNTHESIZED_XML.replace(
        "<model type='virtio'/>",
        "<mac address='52:54:00:12:34:56'/>\n      <model type='virtio'/>",
    );

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(&synthesized_with_mac, &live_with_different_mac)
            .expect("Diff comparison failed");

    assert!(diff_result.has_drift);
    let has_mac_difference = diff_result.differences.iter().any(|difference| match difference {
        DomainXmlDifference::AttributeMismatch {
            attribute_name,
            expected,
            actual,
            ..
        } => attribute_name == "address" && expected == "52:54:00:aa:bb:cc" && actual == "52:54:00:12:34:56",
        _ => false,
    });
    assert!(has_mac_difference);
}

#[test]
fn test_dynamic_seclabel_is_normalized_out() {
    let live_xml_with_seclabel = BASE_SYNTHESIZED_XML.replace(
        "</domain>",
        "  <seclabel type='dynamic' model='dac' relabel='yes'/>\n</domain>",
    );

    let diff_result =
        DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, &live_xml_with_seclabel)
            .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_vcpu_and_memory_drift_are_detected() {
    let synthesized_with_more_resources = BASE_SYNTHESIZED_XML
        .replace("<vcpu placement='static'>16</vcpu>", "<vcpu placement='static'>32</vcpu>")
        .replace("<memory unit='KiB'>16777216</memory>", "<memory unit='KiB'>33554432</memory>");

    let diff_result = DomainXmlDiffer::compare_domain_xmls(
        &synthesized_with_more_resources,
        BASE_SYNTHESIZED_XML,
    )
    .expect("Diff comparison failed");

    assert!(diff_result.has_drift);

    let vcpu_drift = diff_result.differences.iter().any(|difference| match difference {
        DomainXmlDifference::TextContentMismatch {
            element_path,
            expected,
            actual,
        } => element_path == "/domain/vcpu" && expected == "32" && actual == "16",
        _ => false,
    });
    assert!(vcpu_drift);

    let memory_drift = diff_result.differences.iter().any(|difference| match difference {
        DomainXmlDifference::TextContentMismatch {
            element_path,
            expected,
            actual,
        } => element_path == "/domain/memory" && expected == "33554432" && actual == "16777216",
        _ => false,
    });
    assert!(memory_drift);
}

#[test]
fn test_added_and_removed_disks_are_detected() {
    let synthesized_with_second_disk = BASE_SYNTHESIZED_XML.replace(
        "</devices>",
        r#"    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/data/games/data.qcow2'/>
      <target dev='sdb' bus='virtio'/>
    </disk>
  </devices>"#,
    );

    // 1. Missing disk in live
    let diff_result_missing = DomainXmlDiffer::compare_domain_xmls(
        &synthesized_with_second_disk,
        BASE_SYNTHESIZED_XML,
    )
    .expect("Diff comparison failed");

    assert!(diff_result_missing.has_drift);
    let reports_missing_disk = diff_result_missing.differences.iter().any(|difference| match difference {
        DomainXmlDifference::MissingChildElement { parent_path, expected_child } => {
            parent_path == "/domain/devices" && expected_child.contains("dev=\"sdb\"")
        }
        _ => false,
    });
    assert!(reports_missing_disk);

    // 2. Unexpected disk in live
    let diff_result_unexpected = DomainXmlDiffer::compare_domain_xmls(
        BASE_SYNTHESIZED_XML,
        &synthesized_with_second_disk,
    )
    .expect("Diff comparison failed");

    assert!(diff_result_unexpected.has_drift);
    let reports_unexpected_disk = diff_result_unexpected.differences.iter().any(|difference| match difference {
        DomainXmlDifference::UnexpectedChildElement { parent_path, actual_child } => {
            parent_path == "/domain/devices" && actual_child.contains("dev=\"sdb\"")
        }
        _ => false,
    });
    assert!(reports_unexpected_disk);
}

#[test]
fn test_attribute_ordering_and_quote_differences_are_normalized() {
    let reordered_attributes_xml = BASE_SYNTHESIZED_XML.replace(
        "<disk type='file' device='disk'>",
        "<disk device=\"disk\" type=\"file\">",
    );

    let diff_result = DomainXmlDiffer::compare_domain_xmls(
        BASE_SYNTHESIZED_XML,
        &reordered_attributes_xml,
    )
    .expect("Diff comparison failed");

    assert!(!diff_result.has_drift);
    assert!(diff_result.differences.is_empty());
}

#[test]
fn test_malformed_xml_returns_error() {
    let malformed_xml = "<domain><name>unclosed";

    let result = DomainXmlDiffer::compare_domain_xmls(BASE_SYNTHESIZED_XML, malformed_xml);
    assert!(matches!(result, Err(DomainXmlDiffError::LiveXmlParseError(_))));

    let result_synthesized = DomainXmlDiffer::compare_domain_xmls(malformed_xml, BASE_SYNTHESIZED_XML);
    assert!(matches!(result_synthesized, Err(DomainXmlDiffError::SynthesizedXmlParseError(_))));
}
