use std::path::Path;

use onehost::domain::template::{
    DomainTemplateEngine, DomainTemplateInjectionParameters, DomainTemplateSynthesisError,
};

const SAMPLE_TEMPLATE_XML: &str = r#"<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
  <name>TEMPLATE-PLACEHOLDER</name>
  <uuid>00000000-0000-0000-0000-000000000000</uuid>
  <memory unit='KiB'>16777216</memory>
  <currentMemory unit='KiB'>16777216</currentMemory>
  <vcpu placement='static'>16</vcpu>
  <os>
    <type arch='x86_64' machine='pc-q35-9.1'>hvm</type>
    <loader readonly='yes' type='pflash'>/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd</loader>
    <nvram template='/run/libvirt/nix-ovmf/edk2-i386-vars.fd'>/placeholder/nvram.fd</nvram>
  </os>
  <devices>
    <emulator>/run/libvirt/nix-emulators/qemu-system-x86_64</emulator>
    <!-- Managed OS Disk -->
    <disk type='file' device='disk' onehost:role='os-disk'>
      <target dev='sda' bus='sata'/>
      <address type='drive' controller='0' bus='0' target='0' unit='0'/>
    </disk>
    <!-- Unmanaged Secondary Data Disk -->
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='/data/games/data.qcow2'/>
      <target dev='sdb' bus='virtio'/>
    </disk>
    <!-- Passthrough device -->
    <hostdev mode='subsystem' type='pci' managed='yes'>
      <driver name='vfio'/>
      <source>
        <address domain='0x0000' bus='0x07' slot='0x00' function='0x1'/>
      </source>
    </hostdev>
  </devices>
  <qemu:commandline>
    <qemu:arg value='-device'/>
    <qemu:arg value='ivshmem-plain,memdev=looking-glass'/>
  </qemu:commandline>
</domain>"#;

#[test]
fn test_synthesize_domain_xml_with_single_os_disk() {
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: Some(Path::new("/run/libvirt/nix-ovmf/edk2-i386-vars.fd")),
        ovmf_code_path: Some(Path::new("/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd")),
    };

    let synthesized_xml = DomainTemplateEngine::synthesize_concrete_domain_xml(
        SAMPLE_TEMPLATE_XML,
        &parameters,
    )
    .unwrap();

    // 1. Instance identity injected
    assert!(synthesized_xml.contains("<name>win11-gollum</name>"));
    assert!(!synthesized_xml.contains("TEMPLATE-PLACEHOLDER"));
    assert!(synthesized_xml.contains("<uuid>e5a7d620-8931-4bf6-98ec-7e44a30e8c45</uuid>"));
    assert!(!synthesized_xml.contains("00000000-0000-0000-0000-000000000000"));

    // 2. NVRAM path injected
    assert!(synthesized_xml.contains("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"));

    // 3. Managed OS Disk injected with driver and source
    assert!(synthesized_xml.contains("<driver name='qemu' type='qcow2'/>") || synthesized_xml.contains("<driver name=\"qemu\" type=\"qcow2\"/>"));
    assert!(synthesized_xml.contains("/var/lib/libvirt/images/win11-gollum.qcow2"));
    assert!(synthesized_xml.contains("dev='sda'") || synthesized_xml.contains("dev=\"sda\""));
    assert!(synthesized_xml.contains("bus='sata'") || synthesized_xml.contains("bus=\"sata\""));

    // 4. Custom namespace and role attributes stripped cleanly
    assert!(!synthesized_xml.contains("xmlns:onehost"));
    assert!(!synthesized_xml.contains("onehost:role"));
    assert!(!synthesized_xml.contains("os-disk"));

    // 5. Unmanaged data disk untouched
    assert!(synthesized_xml.contains("/data/games/data.qcow2"));
    assert!(synthesized_xml.contains("dev='sdb'") || synthesized_xml.contains("dev=\"sdb\""));

    // 6. Passthrough hostdev and Looking Glass arguments preserved
    assert!(synthesized_xml.contains("bus='0x07'") || synthesized_xml.contains("bus=\"0x07\""));
    assert!(synthesized_xml.contains("ivshmem-plain,memdev=looking-glass"));
}

#[test]
fn test_fails_when_no_disk_has_onehost_role() {
    let xml_without_role = SAMPLE_TEMPLATE_XML.replace("onehost:role='os-disk'", "");
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(&xml_without_role, &parameters);
    assert_eq!(result, Err(DomainTemplateSynthesisError::MissingDesignatedOsDisk));
}

#[test]
fn test_fails_when_multiple_disks_have_onehost_role() {
    // Tag both sda and sdb with onehost:role='os-disk'
    let xml_with_duplicate_role = SAMPLE_TEMPLATE_XML.replace(
        "<disk type='file' device='disk'>",
        "<disk type='file' device='disk' onehost:role='os-disk'>",
    );
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(&xml_with_duplicate_role, &parameters);
    assert_eq!(
        result,
        Err(DomainTemplateSynthesisError::MultipleDesignatedOsDisksFound { count: 2 })
    );
}

#[test]
fn test_fails_on_malformed_xml() {
    let malformed_xml = "<domain><name>broken</domain>";
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(malformed_xml, &parameters);
    assert!(matches!(result, Err(DomainTemplateSynthesisError::XmlParseError(_))));
}

#[test]
fn test_fails_if_root_is_not_domain() {
    let non_domain_xml = "<virtual-machine><name>test</name></virtual-machine>";
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(non_domain_xml, &parameters);
    assert_eq!(result, Err(DomainTemplateSynthesisError::MissingRootDomainElement));
}

#[test]
fn test_fails_when_name_element_missing() {
    let xml_without_name = SAMPLE_TEMPLATE_XML.replace("<name>TEMPLATE-PLACEHOLDER</name>", "");
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(&xml_without_name, &parameters);
    assert_eq!(result, Err(DomainTemplateSynthesisError::MissingNameElement));
}

#[test]
fn test_fails_when_uuid_element_missing() {
    let xml_without_uuid = SAMPLE_TEMPLATE_XML.replace("<uuid>00000000-0000-0000-0000-000000000000</uuid>", "");
    let parameters = DomainTemplateInjectionParameters {
        instance_name: "win11-gollum",
        instance_uuid: "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
        overlay_disk_path: Path::new("/var/lib/libvirt/images/win11-gollum.qcow2"),
        instance_nvram_path: Path::new("/var/lib/libvirt/qemu/nvram/win11-gollum_VARS.fd"),
        nvram_template_path: None,
        ovmf_code_path: None,
    };

    let result = DomainTemplateEngine::synthesize_concrete_domain_xml(&xml_without_uuid, &parameters);
    assert_eq!(result, Err(DomainTemplateSynthesisError::MissingUuidElement));
}

