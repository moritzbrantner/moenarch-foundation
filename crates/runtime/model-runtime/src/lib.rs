#![doc = include_str!("../README.md")]
#![allow(deprecated)]

mod access;
#[cfg(not(target_arch = "wasm32"))]
mod bundles;
mod conformance;
#[cfg(not(target_arch = "wasm32"))]
mod download;
mod predictions;
mod presets;
mod spec;
pub mod surface;

pub use access::*;
#[cfg(not(target_arch = "wasm32"))]
pub use bundles::*;
pub use conformance::*;
#[cfg(not(target_arch = "wasm32"))]
pub use download::*;
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
    /// Cooperative cancellation was requested.
    Cancelled,
}

impl fmt::Display for ModelRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "invalid argument: {message}"),
            Self::Source(message) => write!(formatter, "model source error: {message}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Cancelled => write!(formatter, "model operation cancelled"),
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
