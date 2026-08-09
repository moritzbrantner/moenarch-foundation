use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Backend-neutral raw model prediction used for runtime conformance checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawPrediction {
    /// Optional prediction kind.
    pub kind: Option<String>,
    /// Optional label.
    pub label: Option<String>,
    /// Optional text payload.
    pub text: Option<String>,
    /// Optional confidence score.
    pub score: Option<f32>,
    /// Arbitrary model attributes.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl RawPrediction {
    /// Creates a label prediction.
    pub fn label(label: impl Into<String>, score: f32) -> Self {
        Self {
            label: Some(label.into()),
            score: Some(score),
            ..Self::default()
        }
    }
}
