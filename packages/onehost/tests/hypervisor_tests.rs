use std::path::PathBuf;

use onehost::hypervisor::{
    BlockDeviceInfo, DomainState, HypervisorError, parse_domblklist_output, parse_dominfo_output,
    parse_pool_target_path,
};

const SAMPLE_POOL_XML: &str = r#"
<pool type='dir'>
  <name>default</name>
  <uuid>4f5a6b7c-8d9e-0f1a-2b3c-4d5e6f7a8b9c</uuid>
  <capacity unit='bytes'>1000204886016</capacity>
  <allocation unit='bytes'>450123890688</allocation>
  <available unit='bytes'>550080995328</available>
  <source>
  </source>
  <target>
    <path>/var/lib/libvirt/images</path>
    <permissions>
      <mode>0755</mode>
      <owner>0</owner>
      <group>0</group>
    </permissions>
  </target>
</pool>
"#;

const SAMPLE_VIRSH_DOMINFO_OUTPUT: &str = r#"
Id:             -
Name:           win11-gollum
UUID:           e5a7d620-8931-4bf6-98ec-7e44a30e8c45
OS Type:        hvm
State:          shut off
CPU(s):         8
Max memory:     16777216 KiB
Used memory:    16777216 KiB
Persistent:     yes
Autostart:      enable
Managed save:   no
Security model: none
Security DOI:   0
"#;

const SAMPLE_VIRSH_DOMBLKLIST_OUTPUT: &str = r#"
 Type   Device   Target   Source
------------------------------------------------
 file   disk     sda      /var/lib/libvirt/images/win11-gollum.qcow2
 file   cdrom    sdb      -
 file   disk     sdc      /data/kvm/data.qcow2
"#;

#[test]
fn test_parse_pool_target_path_success() {
    let target_path = parse_pool_target_path(SAMPLE_POOL_XML).expect("Failed to parse pool XML");
    assert_eq!(target_path, PathBuf::from("/var/lib/libvirt/images"));
}

#[test]
fn test_parse_pool_target_path_missing_path() {
    let malformed_xml = "<pool type='dir'><name>no-target</name></pool>";
    let result = parse_pool_target_path(malformed_xml);
    assert!(matches!(
        result,
        Err(HypervisorError::MissingPoolPathElement { .. })
    ));
}

#[test]
fn test_domain_state_parsing() {
    assert_eq!(
        DomainState::from_virsh_state("running"),
        DomainState::Running
    );
    assert_eq!(
        DomainState::from_virsh_state("shut off"),
        DomainState::Shutoff
    );
    assert_eq!(
        DomainState::from_virsh_state("shutoff"),
        DomainState::Shutoff
    );
    assert_eq!(DomainState::from_virsh_state("paused"), DomainState::Paused);
    assert_eq!(
        DomainState::from_virsh_state("crashed"),
        DomainState::Crashed
    );
    assert_eq!(
        DomainState::from_virsh_state("blocked"),
        DomainState::Blocked
    );
    assert_eq!(
        DomainState::from_virsh_state("pmsuspended"),
        DomainState::Pmsuspended
    );
    assert_eq!(
        DomainState::from_virsh_state("something-else"),
        DomainState::Unknown
    );
}

#[test]
fn test_virsh_parsing_dominfo() {
    let parsed_info = parse_dominfo_output(SAMPLE_VIRSH_DOMINFO_OUTPUT, "win11-gollum")
        .expect("Failed to parse dominfo output");

    assert_eq!(parsed_info.name, "win11-gollum");
    assert_eq!(parsed_info.state, DomainState::Shutoff);
    assert_eq!(parsed_info.vcpu_count, Some(8));
    assert_eq!(parsed_info.memory_kib, Some(16_777_216));
    assert!(parsed_info.autostart);
}

#[test]
fn test_virsh_parsing_domblklist() {
    let block_devices = parse_domblklist_output(SAMPLE_VIRSH_DOMBLKLIST_OUTPUT)
        .expect("Failed to parse domblklist output");

    assert_eq!(block_devices.len(), 3);

    assert_eq!(
        block_devices[0],
        BlockDeviceInfo {
            target_device: "sda".to_string(),
            source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
            device_type: "disk".to_string(),
        }
    );

    assert_eq!(
        block_devices[1],
        BlockDeviceInfo {
            target_device: "sdb".to_string(),
            source_file: None,
            device_type: "cdrom".to_string(),
        }
    );

    assert_eq!(
        block_devices[2],
        BlockDeviceInfo {
            target_device: "sdc".to_string(),
            source_file: Some(PathBuf::from("/data/kvm/data.qcow2")),
            device_type: "disk".to_string(),
        }
    );
}
