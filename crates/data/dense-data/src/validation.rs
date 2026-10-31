//! Validation shared by the dense data modules.

use media_core::{DetectError, Result};

use crate::DensePoint;

pub(crate) fn validate_point_set(points: &[DensePoint]) -> Result<usize> {
    if points.is_empty() {
        return Err(invalid_argument("dense point set must not be empty"));
    }
    let dimensions = points[0].dimensions();
    for point in points {
        point.validate()?;
        if point.dimensions() != dimensions {
            return Err(invalid_argument(
                "dense point set coordinates must have the same dimensions",
            ));
        }
    }
    Ok(dimensions)
}

pub(crate) fn validate_coordinates(coordinates: &[f64], name: &str) -> Result<()> {
    if coordinates.is_empty() {
        return Err(invalid_argument(format!("{name} must not be empty")));
    }
    if coordinates.iter().any(|coordinate| !coordinate.is_finite()) {
        return Err(invalid_argument(format!("{name} must be finite")));
    }
    Ok(())
}

pub(crate) fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}
