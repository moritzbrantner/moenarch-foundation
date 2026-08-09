#![doc = include_str!("../README.md")]
#![allow(deprecated)]

mod bundles;
mod conformance;
mod download;
pub mod jobs;
mod predictions;
mod presets;
mod spec;
pub mod surface;

pub use bundles::*;
pub use conformance::*;
pub use download::*;
pub use jobs::{
    plan_model_access, plan_model_bundle, ModelAccessJobRequest, ModelAccessPlan, ModelBundlePlan,
    ModelBundlePlanFile, ModelJobInput, ModelJobKind,
};
pub use predictions::*;
pub use presets::*;
pub use spec::*;

use std::fmt;

/// Result type used by generic model runtime infrastructure.
pub type Result<T> = std::result::Result<T, ModelRuntimeError>;

/// Error type used by generic model runtime infrastructure.
#[derive(Debug)]
pub enum ModelRuntimeError {
    /// The supplied argument or model metadata was invalid.
    InvalidArgument(String),
    /// A filesystem or network source failed.
    Source(String),
    /// A filesystem operation failed.
    Io(std::io::Error),
}

impl fmt::Display for ModelRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "invalid argument: {message}"),
            Self::Source(message) => write!(formatter, "model source error: {message}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ModelRuntimeError {}

impl From<std::io::Error> for ModelRuntimeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests;
