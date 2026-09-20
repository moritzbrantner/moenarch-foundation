use media_core::Result;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::invalid_argument;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
/// Inclusive finite numeric range used for normalization and histograms.
pub struct NumberRange {
    /// Inclusive lower bound.
    pub min: f64,
    /// Inclusive upper bound.
    pub max: f64,
}

impl NumberRange {
    /// Creates a finite range whose lower bound is not greater than its upper bound.
    pub fn new(min: f64, max: f64) -> Result<Self> {
        let range = Self { min, max };
        range.validate()?;
        Ok(range)
    }

    pub(crate) fn validate(self) -> Result<()> {
        if !self.min.is_finite() || !self.max.is_finite() {
            return Err(invalid_argument("number range bounds must be finite"));
        }
        if self.min > self.max {
            return Err(invalid_argument("number range min must not exceed max"));
        }
        Ok(())
    }

    /// Clamps a value into this range.
    pub fn clamp(self, value: f64) -> Result<f64> {
        self.validate()?;
        if !value.is_finite() {
            return Err(invalid_argument("range value must be finite"));
        }
        Ok(value.clamp(self.min, self.max))
    }

    /// Clamps and normalizes a finite value into `0.0..=1.0`.
    pub fn normalize(self, value: f64) -> Result<f64> {
        let value = self.clamp(value)?;
        if self.min == self.max {
            return Ok(0.0);
        }
        Ok((value - self.min) / (self.max - self.min))
    }

    /// Maps a finite normalized value back into this range.
    pub fn denormalize(self, value: f64) -> Result<f64> {
        self.validate()?;
        if !value.is_finite() {
            return Err(invalid_argument("normalized value must be finite"));
        }
        if self.min == self.max {
            return Ok(self.min);
        }
        Ok(self.min + value * (self.max - self.min))
    }
}

#[derive(Deserialize)]
#[serde(rename = "NumberRange")]
struct RangeWire {
    min: f64,
    max: f64,
}

impl<'de> Deserialize<'de> for NumberRange {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let wire = RangeWire::deserialize(deserializer)?;
        Self::new(wire.min, wire.max).map_err(D::Error::custom)
    }
}
