use std::path::{Path, PathBuf};

use onehost::hypervisor::{
    parse_domblklist_output, parse_dominfo_output, parse_pool_target_path, BlockDeviceInfo,
    DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor, HypervisorError,
    MockHypervisor, RecordedHypervisorAction,
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
    assert_eq!(DomainState::from_virsh_state("running"), DomainState::Running);
    assert_eq!(DomainState::from_virsh_state("shut off"), DomainState::Shutoff);
    assert_eq!(DomainState::from_virsh_state("shutoff"), DomainState::Shutoff);
    assert_eq!(DomainState::from_virsh_state("paused"), DomainState::Paused);
    assert_eq!(DomainState::from_virsh_state("crashed"), DomainState::Crashed);
    assert_eq!(DomainState::from_virsh_state("blocked"), DomainState::Blocked);
    assert_eq!(DomainState::from_virsh_state("pmsuspended"), DomainState::Pmsuspended);
    assert_eq!(DomainState::from_virsh_state("something-else"), DomainState::Unknown);
}

#[test]
fn test_mock_hypervisor_domain_lifecycle() {
    let initial_info = DomainInfo {
        name: "win11-gollum".to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(8),
        memory_kib: Some(16_777_216),
        autostart: true,
    };

    let domain_xml = "<domain type='kvm'><name>win11-gollum</name></domain>";

    let hypervisor = MockHypervisor::new().with_domain(initial_info, domain_xml);

    // Verify initial query
    let queried_info = hypervisor.domain_info("win11-gollum").expect("Domain should exist");
    assert_eq!(queried_info.state, DomainState::Shutoff);
    assert!(queried_info.autostart);

    // Boot VM
    hypervisor.start_domain("win11-gollum").expect("Failed to start domain");
    assert_eq!(
        hypervisor.domain_info("win11-gollum").unwrap().state,
        DomainState::Running
    );

    // Shutdown VM
    hypervisor.shutdown_domain("win11-gollum").expect("Failed to shutdown domain");
    assert_eq!(
        hypervisor.domain_info("win11-gollum").unwrap().state,
        DomainState::Shutoff
    );

    // Destroy VM
    hypervisor.start_domain("win11-gollum").expect("Failed to restart domain");
    hypervisor.destroy_domain("win11-gollum").expect("Failed to destroy domain");
    assert_eq!(
        hypervisor.domain_info("win11-gollum").unwrap().state,
        DomainState::Shutoff
    );

    // Set autostart
    hypervisor.set_autostart("win11-gollum", false).expect("Failed to set autostart");
    assert!(!hypervisor.domain_info("win11-gollum").unwrap().autostart);

    // Verify recorded action log
    let actions = hypervisor.recorded_actions();
    assert!(actions.contains(&RecordedHypervisorAction::DomainInfoRequested {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(actions.contains(&RecordedHypervisorAction::StartDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(actions.contains(&RecordedHypervisorAction::ShutdownDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(actions.contains(&RecordedHypervisorAction::DestroyDomain {
        domain_name: "win11-gollum".to_string(),
    }));
    assert!(actions.contains(&RecordedHypervisorAction::SetAutostart {
        domain_name: "win11-gollum".to_string(),
        autostart: false,
    }));
}

#[test]
fn test_mock_hypervisor_define_and_undefine() {
    let hypervisor = MockHypervisor::new();

    let new_domain_xml = "<domain type='kvm'><name>win11-fresh</name><uuid>12345678-1234-1234-1234-123456789abc</uuid></domain>";

    hypervisor.define_domain(new_domain_xml).expect("Failed to define domain");

    let info = hypervisor.domain_info("win11-fresh").expect("Domain should be defined");
    assert_eq!(info.name, "win11-fresh");
    assert_eq!(info.state, DomainState::Shutoff);

    let dumped_xml = hypervisor.dump_xml("win11-fresh", false).expect("Failed to dump XML");
    assert_eq!(dumped_xml, new_domain_xml);

    hypervisor.undefine_domain("win11-fresh", true).expect("Failed to undefine domain");
    assert!(matches!(
        hypervisor.domain_info("win11-fresh"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
}

#[test]
fn test_mock_hypervisor_snapshots_and_blockcommit() {
    let domain_info = DomainInfo {
        name: "win11-snap".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: false,
    };

    let hypervisor = MockHypervisor::new().with_domain(domain_info, "<domain><name>win11-snap</name></domain>");

    let disk_specifications = vec![
        DiskSnapshotSpecification {
            target_device: "sda".to_string(),
            snapshot_file_path: PathBuf::from("/var/lib/libvirt/images/win11-snap.sda.snap"),
        },
    ];

    hypervisor
        .create_snapshot("win11-snap", "backup-2026", &disk_specifications, true)
        .expect("Failed to create snapshot");

    hypervisor
        .blockcommit(
            "win11-snap",
            "sda",
            Some(Path::new("/var/lib/libvirt/images/win11-snap.qcow2")),
            Some(Path::new("/var/lib/libvirt/images/win11-snap.sda.snap")),
            true,
            true,
        )
        .expect("Failed to blockcommit");

    let actions = hypervisor.recorded_actions();
    assert!(actions.contains(&RecordedHypervisorAction::CreateSnapshot {
        domain_name: "win11-snap".to_string(),
        snapshot_name: "backup-2026".to_string(),
        disk_specifications,
        quiesce: true,
    }));
    assert!(actions.contains(&RecordedHypervisorAction::Blockcommit {
        domain_name: "win11-snap".to_string(),
        target_device: "sda".to_string(),
        base_path: Some(PathBuf::from("/var/lib/libvirt/images/win11-snap.qcow2")),
        top_path: Some(PathBuf::from("/var/lib/libvirt/images/win11-snap.sda.snap")),
        active: true,
        pivot: true,
    }));
}

#[test]
fn test_mock_hypervisor_pool_resolution_and_refresh() {
    let hypervisor = MockHypervisor::new().with_pool_xml("default", SAMPLE_POOL_XML);

    let resolved_path = hypervisor.resolve_pool_path("default").expect("Failed to resolve pool path");
    assert_eq!(resolved_path, PathBuf::from("/var/lib/libvirt/images"));

    hypervisor.pool_refresh("default").expect("Failed to refresh pool");

    let actions = hypervisor.recorded_actions();
    assert!(actions.contains(&RecordedHypervisorAction::PoolDumpXml {
        pool_name: "default".to_string(),
    }));
    assert!(actions.contains(&RecordedHypervisorAction::PoolRefresh {
        pool_name: "default".to_string(),
    }));
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

    assert_eq!(block_devices[0], BlockDeviceInfo {
        target_device: "sda".to_string(),
        source_file: Some(PathBuf::from("/var/lib/libvirt/images/win11-gollum.qcow2")),
        device_type: "disk".to_string(),
    });

    assert_eq!(block_devices[1], BlockDeviceInfo {
        target_device: "sdb".to_string(),
        source_file: None,
        device_type: "cdrom".to_string(),
    });

    assert_eq!(block_devices[2], BlockDeviceInfo {
        target_device: "sdc".to_string(),
        source_file: Some(PathBuf::from("/data/kvm/data.qcow2")),
        device_type: "disk".to_string(),
    });
}

#[test]
fn test_mock_hypervisor_injected_errors() {
    let hypervisor = MockHypervisor::new()
        .with_injected_domain_error("faulty-vm", "Simulated QEMU hypervisor error")
        .with_injected_pool_error("faulty-pool", "Simulated pool storage error");

    let domain_result = hypervisor.domain_info("faulty-vm");
    assert!(matches!(
        domain_result,
        Err(HypervisorError::CommandExecutionFailed { .. })
    ));

    let pool_result = hypervisor.pool_refresh("faulty-pool");
    assert!(matches!(
        pool_result,
        Err(HypervisorError::CommandExecutionFailed { .. })
    ));
}
