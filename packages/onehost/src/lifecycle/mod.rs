use thiserror::Error;

use crate::domain::diff::DomainXmlDiffError;
use crate::domain::template::DomainTemplateSynthesisError;
use crate::hypervisor::traits::HypervisorError;
use crate::image::tag::{FlavorResolutionError, ImageTagParseError};
use crate::storage::traits::StorageError;

pub mod applier;
pub mod destroyer;
pub mod planner;

pub use applier::{ApplyOptions, DomainLifecycleApplier};
pub use destroyer::{DestroyOptions, DomainLifecycleDestroyer};
pub use planner::{
    DanglingSnapshotAlert, DomainLifecyclePlanner, InstancePlanAction, OnehostPlan,
    TemplateResolver,
};

/// Enumerates errors that can occur during domain lifecycle planning, application, or destruction.
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("Lifecycle policy violation for '{instance_name}': {details}")]
    LifecyclePolicyViolation {
        instance_name: String,
        details: String,
    },

    #[error("Hypervisor error: {source}")]
    HypervisorError {
        #[from]
        source: HypervisorError,
    },

    #[error("Storage error: {source}")]
    StorageError {
        #[from]
        source: StorageError,
    },

    #[error("Domain synthesis error: {source}")]
    DomainSynthesisError {
        #[from]
        source: DomainTemplateSynthesisError,
    },

    #[error("Domain diff error: {source}")]
    DomainDiffError {
        #[from]
        source: DomainXmlDiffError,
    },

    #[error("Image tag parse error: {source}")]
    ImageTagParseError {
        #[from]
        source: ImageTagParseError,
    },

    #[error("Flavor resolution error: {source}")]
    FlavorResolutionError {
        #[from]
        source: FlavorResolutionError,
    },

    #[error("Configuration error: {details}")]
    ConfigurationError {
        details: String,
    },

    #[error("I/O error during lifecycle management: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}
