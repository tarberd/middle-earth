use std::path::PathBuf;

use crate::hypervisor::traits::Hypervisor;

/// Information tracking an individual active snapshot device during live backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSnapshotInfo {
    pub target_device: String,
    pub base_path: PathBuf,
    pub snapshot_path: PathBuf,
}

/// RAII scope guard guaranteeing that active disk snapshots are collapsed via `blockcommit`
/// if backup fails, panics, or is interrupted, preventing dangling `.snap` chains.
pub struct SnapshotCleanupGuard<'a, H: Hypervisor> {
    hypervisor: &'a H,
    domain_name: String,
    snapshots: Vec<ActiveSnapshotInfo>,
    armed: bool,
}

impl<'a, H: Hypervisor> SnapshotCleanupGuard<'a, H> {
    /// Creates and arms a new snapshot cleanup guard.
    pub fn new(
        hypervisor: &'a H,
        domain_name: impl Into<String>,
        snapshots: Vec<ActiveSnapshotInfo>,
    ) -> Self {
        Self {
            hypervisor,
            domain_name: domain_name.into(),
            snapshots,
            armed: true,
        }
    }

    /// Disarms the guard when snapshots have been cleanly committed and unlinked.
    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// Returns whether the guard is currently armed.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Returns the active snapshots protected by this guard.
    pub fn snapshots(&self) -> &[ActiveSnapshotInfo] {
        &self.snapshots
    }
}

impl<'a, H: Hypervisor> Drop for SnapshotCleanupGuard<'a, H> {
    fn drop(&mut self) {
        if self.armed {
            tracing::warn!(
                domain_name = %self.domain_name,
                "Snapshot cleanup guard triggered; executing rollback blockcommit on active snapshots"
            );
            self.snapshots.iter().for_each(|snapshot| {
                let blockcommit_result = self.hypervisor.blockcommit(
                    &self.domain_name,
                    &snapshot.target_device,
                    Some(&snapshot.base_path),
                    Some(&snapshot.snapshot_path),
                    true,
                    true,
                );

                if let Err(error) = blockcommit_result {
                    tracing::error!(
                        domain_name = %self.domain_name,
                        target_device = %snapshot.target_device,
                        error = %error,
                        "Failed to execute rollback blockcommit during snapshot cleanup guard drop"
                    );
                }
            });
        }
    }
}
