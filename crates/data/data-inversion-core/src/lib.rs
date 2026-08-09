#![doc = include_str!("../README.md")]

pub mod surface;
use media_core::{DetectError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing information fidelity.
pub enum InformationFidelity {
    /// The exact variant.
    Exact,
    /// The preserved variant.
    Preserved,
    /// The quantized variant.
    Quantized,
    /// The estimated variant.
    Estimated,
    /// The interpolated variant.
    Interpolated,
    /// The heuristic variant.
    Heuristic,
    /// The placeholder variant.
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing inversion method.
pub enum InversionMethod {
    /// The preserved variant.
    Preserved,
    /// The defaulted variant.
    Defaulted,
    /// The quantized variant.
    Quantized,
    /// The inferred variant.
    Inferred,
    /// The interpolated variant.
    Interpolated,
    /// The template variant.
    Template,
    /// The omitted variant.
    Omitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data type for inversion note.
pub struct InversionNote {
    /// The field value.
    pub field: String,
    /// The method value.
    pub method: InversionMethod,
    /// The message value.
    pub message: String,
}

impl InversionNote {
    /// Creates a new value.
    pub fn new(
        field: impl Into<String>,
        method: InversionMethod,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            method,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for inversion trace.
pub struct InversionTrace {
    /// The source type value.
    pub source_type: String,
    /// The target type value.
    pub target_type: String,
    /// The fidelity value.
    pub fidelity: InformationFidelity,
    /// Confidence score for this value.
    pub confidence: f32,
    /// The assumptions value.
    pub assumptions: Vec<String>,
    /// The notes value.
    pub notes: Vec<InversionNote>,
}

impl InversionTrace {
    /// Creates a new value.
    pub fn new(
        source_type: impl Into<String>,
        target_type: impl Into<String>,
        fidelity: InformationFidelity,
    ) -> Self {
        Self {
            source_type: source_type.into(),
            target_type: target_type.into(),
            fidelity,
            confidence: default_confidence(fidelity),
            assumptions: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Returns confidence.
    pub fn confidence(mut self, confidence: f32) -> Result<Self> {
        validate_confidence(confidence)?;
        self.confidence = confidence;
        Ok(self)
    }

    /// Returns assumption.
    pub fn assumption(mut self, assumption: impl Into<String>) -> Self {
        self.assumptions.push(assumption.into());
        self
    }

    /// Returns note.
    pub fn note(
        mut self,
        field: impl Into<String>,
        method: InversionMethod,
        message: impl Into<String>,
    ) -> Self {
        self.notes.push(InversionNote::new(field, method, message));
        self
    }

    /// Returns merge notes.
    pub fn merge_notes(&mut self, other: &InversionTrace) {
        self.assumptions.extend(other.assumptions.iter().cloned());
        self.notes.extend(other.notes.iter().cloned());
        self.confidence = self.confidence.min(other.confidence);
        self.fidelity = weaker_fidelity(self.fidelity, other.fidelity);
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for generated.
pub struct Generated<T> {
    /// The value value.
    pub value: T,
    /// The trace value.
    pub trace: InversionTrace,
}

impl<T> Generated<T> {
    /// Creates a new value.
    pub fn new(value: T, trace: InversionTrace) -> Self {
        Self { value, trace }
    }

    /// Returns map.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Generated<U> {
        Generated {
            value: f(self.value),
            trace: self.trace,
        }
    }
}

/// Validates confidence.
pub fn validate_confidence(confidence: f32) -> Result<()> {
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(DetectError::InvalidArgument(
            "inversion confidence must be finite and between 0 and 1".to_string(),
        ));
    }
    Ok(())
}

/// Returns weaker fidelity.
pub fn weaker_fidelity(
    left: InformationFidelity,
    right: InformationFidelity,
) -> InformationFidelity {
    if fidelity_rank(left) >= fidelity_rank(right) {
        left
    } else {
        right
    }
}

fn fidelity_rank(fidelity: InformationFidelity) -> u8 {
    match fidelity {
        InformationFidelity::Exact => 0,
        InformationFidelity::Preserved => 1,
        InformationFidelity::Quantized => 2,
        InformationFidelity::Estimated => 3,
        InformationFidelity::Interpolated => 4,
        InformationFidelity::Heuristic => 5,
        InformationFidelity::Placeholder => 6,
    }
}

fn default_confidence(fidelity: InformationFidelity) -> f32 {
    match fidelity {
        InformationFidelity::Exact => 1.0,
        InformationFidelity::Preserved => 0.95,
        InformationFidelity::Quantized => 0.8,
        InformationFidelity::Estimated => 0.65,
        InformationFidelity::Interpolated => 0.55,
        InformationFidelity::Heuristic => 0.35,
        InformationFidelity::Placeholder => 0.15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weaker_fidelity_keeps_more_lossy_value() {
        assert_eq!(
            weaker_fidelity(
                InformationFidelity::Preserved,
                InformationFidelity::Heuristic
            ),
            InformationFidelity::Heuristic
        );
    }

    #[test]
    fn rejects_invalid_confidence() {
        assert!(validate_confidence(1.1).is_err());
        assert!(validate_confidence(f32::NAN).is_err());
    }
}
