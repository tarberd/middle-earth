use std::path::{Path, PathBuf};

use onehost::hypervisor::{
    BlockDeviceInfo, DiskSnapshotSpecification, DomainInfo, DomainState, Hypervisor,
    HypervisorError,
};
use onehost_test_support::FakeHypervisor;

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

const SECOND_POOL_XML: &str = r#"
<pool type='dir'>
  <name>backup-pool</name>
  <target>
    <path>/mnt/backups</path>
  </target>
</pool>
"#;

#[test]
fn test_fake_hypervisor_lifecycle_and_state_inspection() {
    let initial_info = DomainInfo {
        name: "test-vm".to_string(),
        state: DomainState::Shutoff,
        vcpu_count: Some(4),
        memory_kib: Some(8_388_608),
        autostart: true,
    };
    let initial_xml = "<domain type='kvm'><name>test-vm</name></domain>";

    let fake = FakeHypervisor::new().with_domain(initial_info, initial_xml);

    // Verify initial snapshot state
    let snapshot = fake.snapshot();
    assert!(snapshot.domains.contains_key("test-vm"));
    assert!(!snapshot.domains.contains_key("nonexistent-vm"));
    let mut domain_names: Vec<String> = snapshot.domains.keys().cloned().collect();
    domain_names.sort();
    assert_eq!(domain_names, vec!["test-vm".to_string()]);
    assert_eq!(
        snapshot
            .domains
            .get("test-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );
    assert_eq!(
        snapshot
            .domains
            .get("nonexistent-vm")
            .map(|entry| entry.info.state),
        None
    );
    assert_eq!(
        snapshot
            .domains
            .get("test-vm")
            .map(|entry| entry.xml.as_str()),
        Some(initial_xml)
    );
    assert_eq!(
        snapshot
            .domains
            .get("nonexistent-vm")
            .map(|entry| entry.xml.as_str()),
        None
    );
    assert_eq!(
        snapshot
            .domains
            .get("test-vm")
            .map(|entry| entry.info.autostart),
        Some(true)
    );
    assert_eq!(
        snapshot
            .domains
            .get("nonexistent-vm")
            .map(|entry| entry.info.autostart),
        None
    );

    // Boot VM and observe state transition in snapshot
    fake.start_domain("test-vm")
        .expect("start_domain should succeed");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("test-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );

    let queried_info = fake
        .domain_info("test-vm")
        .expect("domain_info should succeed");
    assert_eq!(queried_info.state, DomainState::Running);

    // Shutdown VM and observe state transition in snapshot
    fake.shutdown_domain("test-vm")
        .expect("shutdown_domain should succeed");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("test-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );

    // Destroy VM from running state
    fake.start_domain("test-vm")
        .expect("restart should succeed");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("test-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );
    fake.destroy_domain("test-vm")
        .expect("destroy_domain should succeed");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("test-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );

    // Toggle autostart and observe snapshot
    fake.set_autostart("test-vm", false)
        .expect("set_autostart should succeed");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("test-vm")
            .map(|entry| entry.info.autostart),
        Some(false)
    );

    // Undefine domain and observe cleanup in snapshot
    fake.undefine_domain("test-vm", true)
        .expect("undefine_domain should succeed");
    let after_undefine_snapshot = fake.snapshot();
    assert!(!after_undefine_snapshot.domains.contains_key("test-vm"));
    assert!(after_undefine_snapshot.domains.is_empty());
}

#[test]
fn test_fake_hypervisor_redefine_preserves_running_state() {
    let fake = FakeHypervisor::new();
    let initial_xml = "<domain type='kvm'><name>running-vm</name></domain>";
    fake.define_domain(initial_xml).expect("define new domain");

    let initial_snapshot = fake.snapshot();
    assert_eq!(
        initial_snapshot
            .domains
            .get("running-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );

    fake.start_domain("running-vm").expect("start running-vm");
    assert_eq!(
        fake.snapshot()
            .domains
            .get("running-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );

    let updated_xml = "<domain type='kvm'><name>running-vm</name><vcpu>4</vcpu></domain>";
    fake.define_domain(updated_xml).expect("redefine domain");

    let updated_snapshot = fake.snapshot();
    assert_eq!(
        updated_snapshot
            .domains
            .get("running-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );
    assert_eq!(
        updated_snapshot
            .domains
            .get("running-vm")
            .map(|entry| entry.xml.as_str()),
        Some(updated_xml)
    );
}

#[test]
fn test_fake_hypervisor_undefine_running_domain_returns_error() {
    let fake = FakeHypervisor::new();
    let domain_xml = "<domain type='kvm'><name>active-vm</name></domain>";
    fake.define_domain(domain_xml).expect("define domain");
    fake.start_domain("active-vm").expect("start domain");

    // Attempting to undefine running domain must return HypervisorError
    let error = fake
        .undefine_domain("active-vm", false)
        .expect_err("undefine running domain must fail");
    assert!(matches!(
        error,
        HypervisorError::CommandExecutionFailed { ref stderr, .. }
            if stderr.contains("Cannot undefine running domain")
    ));

    // Domain must still exist and remain running in snapshot
    let snapshot = fake.snapshot();
    assert!(snapshot.domains.contains_key("active-vm"));
    assert_eq!(
        snapshot
            .domains
            .get("active-vm")
            .map(|entry| entry.info.state),
        Some(DomainState::Running)
    );

    // After shutdown, undefine succeeds
    fake.shutdown_domain("active-vm").expect("shutdown domain");
    fake.undefine_domain("active-vm", false)
        .expect("undefine shutoff domain");
    assert!(!fake.snapshot().domains.contains_key("active-vm"));
}

#[test]
fn test_fake_hypervisor_define_domain_and_block_device_extraction() {
    let fake = FakeHypervisor::new();

    let domain_xml = r#"
<domain type='kvm'>
  <name>win-workstation</name>
  <devices>
    <disk type='file' device='disk'>
      <source file='/var/lib/libvirt/images/win-workstation.qcow2'/>
      <target dev='sda' bus='virtio'/>
    </disk>
    <disk type='file' device='cdrom'>
      <target dev='sdb' bus='sata'/>
    </disk>
  </devices>
</domain>
"#;

    fake.define_domain(domain_xml)
        .expect("define_domain should succeed");

    let snapshot = fake.snapshot();
    assert!(snapshot.domains.contains_key("win-workstation"));
    assert_eq!(
        snapshot
            .domains
            .get("win-workstation")
            .map(|entry| entry.info.state),
        Some(DomainState::Shutoff)
    );
    assert_eq!(
        snapshot
            .domains
            .get("win-workstation")
            .map(|entry| entry.info.autostart),
        Some(false)
    );

    let devices = &snapshot
        .domains
        .get("win-workstation")
        .expect("block devices should exist")
        .block_devices;
    assert_eq!(devices.len(), 2);
    assert_eq!(
        devices[0],
        BlockDeviceInfo {
            target_device: "sda".to_string(),
            source_file: Some(PathBuf::from(
                "/var/lib/libvirt/images/win-workstation.qcow2"
            )),
            device_type: "disk".to_string(),
        }
    );
    assert_eq!(
        devices[1],
        BlockDeviceInfo {
            target_device: "sdb".to_string(),
            source_file: None,
            device_type: "cdrom".to_string(),
        }
    );

    let queried_devices = fake
        .list_block_devices("win-workstation")
        .expect("list_block_devices should succeed");
    assert_eq!(&queried_devices, devices);

    let dumped_xml = fake
        .dump_xml("win-workstation", true)
        .expect("dump_xml should succeed");
    assert_eq!(dumped_xml, domain_xml);
}

#[test]
fn test_fake_hypervisor_explicit_block_devices_builder() {
    let custom_devices = vec![BlockDeviceInfo {
        target_device: "vda".to_string(),
        source_file: Some(PathBuf::from("/srv/storage/custom.qcow2")),
        device_type: "disk".to_string(),
    }];

    let domain_info = DomainInfo {
        name: "custom-vm".to_string(),
        state: DomainState::Shutoff,
        vcpu_count: None,
        memory_kib: None,
        autostart: false,
    };

    let fake = FakeHypervisor::new()
        .with_domain(domain_info, "<domain><name>custom-vm</name></domain>")
        .with_block_devices("custom-vm", custom_devices.clone());

    let snapshot = fake.snapshot();
    assert_eq!(
        snapshot
            .domains
            .get("custom-vm")
            .map(|entry| &entry.block_devices),
        Some(&custom_devices)
    );
    assert_eq!(
        snapshot
            .domains
            .get("nonexistent-vm")
            .map(|entry| &entry.block_devices),
        None
    );
}

#[test]
fn test_fake_hypervisor_storage_pool_inspection_and_resolution() {
    let fake = FakeHypervisor::new()
        .with_pool_xml("default", SAMPLE_POOL_XML)
        .with_pool_xml("backup-pool", SECOND_POOL_XML);

    let snapshot = fake.snapshot();
    assert!(snapshot.storage_pools.contains_key("default"));
    assert!(snapshot.storage_pools.contains_key("backup-pool"));
    assert!(!snapshot.storage_pools.contains_key("unknown-pool"));

    let mut pools: Vec<String> = snapshot.storage_pools.keys().cloned().collect();
    pools.sort();
    assert_eq!(
        pools,
        vec!["backup-pool".to_string(), "default".to_string()]
    );
    assert_eq!(
        snapshot.storage_pools.get("default").map(|s| s.as_str()),
        Some(SAMPLE_POOL_XML)
    );
    assert_eq!(snapshot.storage_pools.get("unknown-pool"), None);

    // Hypervisor trait pool operations
    let dumped_xml = fake
        .pool_dump_xml("default")
        .expect("pool_dump_xml should succeed");
    assert_eq!(dumped_xml, SAMPLE_POOL_XML);

    fake.pool_refresh("default")
        .expect("pool_refresh should succeed");

    let resolved_path = fake
        .resolve_pool_path("default")
        .expect("resolve_pool_path should succeed");
    assert_eq!(resolved_path, PathBuf::from("/var/lib/libvirt/images"));

    let resolved_name = fake
        .resolve_pool_name_by_path(Path::new("/var/lib/libvirt/images"))
        .expect("resolve_pool_name_by_path should succeed");
    assert_eq!(resolved_name, Some("default".to_string()));

    let unknown_pool_name = fake
        .resolve_pool_name_by_path(Path::new("/nonexistent/path"))
        .expect("resolve should return None for unknown path");
    assert_eq!(unknown_pool_name, None);
}

#[test]
fn test_fake_hypervisor_snapshots_and_blockcommit() {
    let temp_dir = tempfile::tempdir().expect("tempdir should succeed");
    let snapshot_file_path = temp_dir.path().join("snap1.qcow2");
    let base_file_path = PathBuf::from("/var/lib/base.qcow2");

    let domain_info = DomainInfo {
        name: "snap-vm".to_string(),
        state: DomainState::Running,
        vcpu_count: Some(2),
        memory_kib: Some(4_194_304),
        autostart: true,
    };

    let initial_devices = vec![BlockDeviceInfo {
        target_device: "sda".to_string(),
        device_type: "disk".to_string(),
        source_file: Some(base_file_path.clone()),
    }];

    let fake = FakeHypervisor::new()
        .with_domain(domain_info, "<domain><name>snap-vm</name></domain>")
        .with_block_devices("snap-vm", initial_devices);

    let disk_spec = DiskSnapshotSpecification {
        target_device: "sda".to_string(),
        snapshot_file_path: snapshot_file_path.clone(),
    };

    fake.create_snapshot("snap-vm", "daily-backup", &[disk_spec], false)
        .expect("create_snapshot should succeed");

    assert!(snapshot_file_path.exists());
    let snapshot_after_create = fake.snapshot();
    let devices_after_snapshot = &snapshot_after_create
        .domains
        .get("snap-vm")
        .expect("domain entry should exist")
        .block_devices;
    assert_eq!(
        devices_after_snapshot,
        &vec![BlockDeviceInfo {
            target_device: "sda".to_string(),
            device_type: "disk".to_string(),
            source_file: Some(snapshot_file_path.clone()),
        }]
    );

    fake.blockcommit(
        "snap-vm",
        "sda",
        Some(&base_file_path),
        Some(&snapshot_file_path),
        true,
        true,
    )
    .expect("blockcommit should succeed");

    let snapshot_after_commit = fake.snapshot();
    let devices_after_commit = &snapshot_after_commit
        .domains
        .get("snap-vm")
        .expect("domain entry should exist")
        .block_devices;
    assert_eq!(
        devices_after_commit,
        &vec![BlockDeviceInfo {
            target_device: "sda".to_string(),
            device_type: "disk".to_string(),
            source_file: Some(base_file_path),
        }]
    );
}

#[test]
fn test_fake_hypervisor_undefine_domain_cleans_nvram_file() {
    let temp_dir = tempfile::tempdir().expect("tempdir should succeed");
    let nvram_path = temp_dir.path().join("nvram-vm_VARS.fd");
    std::fs::write(&nvram_path, b"dummy nvram").expect("write dummy nvram");
    assert!(nvram_path.exists());

    let domain_xml = format!(
        "<domain type='kvm'><name>nvram-vm</name><os><nvram>{}</nvram></os></domain>",
        nvram_path.display()
    );

    let fake = FakeHypervisor::new();
    fake.define_domain(&domain_xml)
        .expect("define_domain should succeed");
    assert!(fake.snapshot().domains.contains_key("nvram-vm"));

    fake.undefine_domain("nvram-vm", true)
        .expect("undefine_domain with cleanup_nvram should succeed");
    assert!(!fake.snapshot().domains.contains_key("nvram-vm"));
    assert!(!nvram_path.exists());
}

#[test]
fn test_fake_hypervisor_injected_errors_and_failures() {
    let domain_info = DomainInfo {
        name: "failing-vm".to_string(),
        state: DomainState::Shutoff,
        vcpu_count: None,
        memory_kib: None,
        autostart: false,
    };

    let fake = FakeHypervisor::new()
        .with_domain(domain_info, "<domain><name>failing-vm</name></domain>")
        .with_injected_domain_error("failing-vm", "Simulated domain failure")
        .with_injected_pool_error("failing-pool", "Simulated pool failure")
        .with_quiesce_failure(true);

    // Domain queries should fail with injected error
    assert!(matches!(
        fake.domain_info("failing-vm"),
        Err(HypervisorError::CommandExecutionFailed { stderr, .. }) if stderr.contains("Simulated domain failure")
    ));
    assert!(matches!(
        fake.dump_xml("failing-vm", true),
        Err(HypervisorError::CommandExecutionFailed { command, stderr, .. })
            if command.contains("--inactive") && stderr.contains("Simulated domain failure")
    ));
    assert!(matches!(
        fake.dump_xml("failing-vm", false),
        Err(HypervisorError::CommandExecutionFailed { command, stderr, .. })
            if !command.contains("--inactive") && stderr.contains("Simulated domain failure")
    ));
    assert!(matches!(
        fake.undefine_domain("failing-vm", true),
        Err(HypervisorError::CommandExecutionFailed { command, stderr, .. })
            if command.contains("--nvram") && stderr.contains("Simulated domain failure")
    ));
    assert!(matches!(
        fake.undefine_domain("failing-vm", false),
        Err(HypervisorError::CommandExecutionFailed { command, stderr, .. })
            if !command.contains("--nvram") && stderr.contains("Simulated domain failure")
    ));

    // Pool queries should fail with injected error
    assert!(matches!(
        fake.pool_dump_xml("failing-pool"),
        Err(HypervisorError::CommandExecutionFailed { stderr, .. }) if stderr.contains("Simulated pool failure")
    ));
    assert!(matches!(
        fake.pool_refresh("failing-pool"),
        Err(HypervisorError::CommandExecutionFailed { stderr, .. }) if stderr.contains("Simulated pool failure")
    ));

    // Quiesce failure
    let disk_spec = DiskSnapshotSpecification {
        target_device: "sda".to_string(),
        snapshot_file_path: PathBuf::from("/tmp/dummy.snap"),
    };
    assert!(matches!(
        fake.create_snapshot("failing-vm", "snap", &[disk_spec], true),
        Err(HypervisorError::CommandExecutionFailed { stderr, .. }) if stderr.contains("Guest agent is not responding")
    ));
}

#[test]
fn test_fake_hypervisor_domain_not_found_errors() {
    let fake = FakeHypervisor::new();

    assert!(matches!(
        fake.domain_info("nonexistent"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.dump_xml("nonexistent", false),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.undefine_domain("nonexistent", false),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.start_domain("nonexistent"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.shutdown_domain("nonexistent"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.destroy_domain("nonexistent"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.list_block_devices("nonexistent"),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.set_autostart("nonexistent", true),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.blockcommit("nonexistent", "sda", None, None, false, false),
        Err(HypervisorError::DomainNotFound { .. })
    ));
    assert!(matches!(
        fake.create_snapshot("nonexistent", "snap", &[], false),
        Err(HypervisorError::DomainNotFound { .. })
    ));

    // Storage pool not found errors
    assert!(matches!(
        fake.pool_dump_xml("nonexistent"),
        Err(HypervisorError::StoragePoolNotFound { .. })
    ));
    assert!(matches!(
        fake.pool_refresh("nonexistent"),
        Err(HypervisorError::StoragePoolNotFound { .. })
    ));
}

#[test]
fn test_fake_hypervisor_define_domain_validation_errors() {
    let fake = FakeHypervisor::new();

    let malformed_xml = "not xml";
    assert!(matches!(
        fake.define_domain(malformed_xml),
        Err(HypervisorError::OutputParseError { .. })
    ));

    let missing_name_xml = "<domain type='kvm'><uuid>1234</uuid></domain>";
    assert!(matches!(
        fake.define_domain(missing_name_xml),
        Err(HypervisorError::OutputParseError { .. })
    ));
}
