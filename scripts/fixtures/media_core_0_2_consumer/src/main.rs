//! Registry-shaped consumer of the issue #143 media-core 0.2 wave.
//!
//! It proves the candidate closure resolves to one media-core identity and
//! that a downstream match on the now `#[non_exhaustive]` `DetectError`
//! compiles only with a wildcard arm.

use data_inversion_core as _;
use dense_data as _;
use graph_analysis_core as _;
use math_geometry_2d as _;
use math_linear as _;
use math_signal_core as _;
use math_sparse_data as _;
use math_statistics as _;
use media_core::DetectError;
use tensor_data as _;
use vector_analysis_core as _;
use vector_analysis_index as _;

fn classify(error: &DetectError) -> &'static str {
    match error {
        DetectError::ModelRequired { .. } => "model-required",
        DetectError::InvalidArgument(_) => "invalid-argument",
        _ => "other",
    }
}

/// Re-exported errors keep media-core's single type identity.
fn audio_error_is_media_error(error: audio_contracts::DetectError) -> DetectError {
    error
}

/// Dependent crates' results carry the same media-core error type.
fn numbers_result_is_media_result(
    result: media_core::Result<numbers_core::NumberRange>,
) -> Result<numbers_core::NumberRange, DetectError> {
    result
}

fn main() {
    let error = audio_error_is_media_error(DetectError::ModelRequired {
        capability: "named-entity recognition".into(),
        hint: "install the NER model bundle".into(),
    });
    assert_eq!(classify(&error), "model-required");
    let _ = numbers_result_is_media_result(numbers_core::NumberRange::new(0.0, 1.0));
}
