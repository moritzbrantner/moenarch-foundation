//! Exact base-10 decimal values and explicit rounding contracts.
//!
//! This module intentionally supplies mechanics only. Currency, money, ledger,
//! tax, interest, and portfolio semantics belong to consumers above foundation.

use std::{fmt, str::FromStr};

use rust_decimal::{prelude::ToPrimitive, Decimal, RoundingStrategy};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Errors from exact decimal parsing, arithmetic, rounding, and conversion.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecimalError {
    /// Text was not a representable base-10 decimal literal.
    #[error("invalid exact decimal literal: {0}")]
    Parse(String),
    /// A requested decimal scale exceeds the supported fixed scale range.
    #[error("decimal scale {scale} exceeds maximum supported scale {maximum}")]
    InvalidScale { scale: u32, maximum: u32 },
    /// An exact decimal operation exceeded the fixed representation range.
    #[error("decimal arithmetic overflow")]
    Overflow,
    /// Division by zero was requested.
    #[error("decimal division by zero")]
    DivisionByZero,
    /// A float conversion received NaN or infinity.
    #[error("floating-point input must be finite")]
    NonFiniteFloat,
    /// A conversion to an integer would discard a fractional part or overflow.
    #[error("decimal is not representable as an exact i128")]
    NotExactInteger,
}

/// Explicit rounding behavior for [`ExactDecimal::round_to_scale`] and division.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundingMode {
    /// Resolve midpoint ties to the nearest even final digit.
    MidpointNearestEven,
    /// Resolve midpoint ties away from zero.
    MidpointAwayFromZero,
    /// Resolve midpoint ties toward zero.
    MidpointTowardZero,
    /// Always truncate toward zero.
    TowardZero,
    /// Always round away from zero.
    AwayFromZero,
    /// Always round toward negative infinity.
    TowardNegativeInfinity,
    /// Always round toward positive infinity.
    TowardPositiveInfinity,
}

impl RoundingMode {
    fn strategy(self) -> RoundingStrategy {
        match self {
            Self::MidpointNearestEven => RoundingStrategy::MidpointNearestEven,
            Self::MidpointAwayFromZero => RoundingStrategy::MidpointAwayFromZero,
            Self::MidpointTowardZero => RoundingStrategy::MidpointTowardZero,
            Self::TowardZero => RoundingStrategy::ToZero,
            Self::AwayFromZero => RoundingStrategy::AwayFromZero,
            Self::TowardNegativeInfinity => RoundingStrategy::ToNegativeInfinity,
            Self::TowardPositiveInfinity => RoundingStrategy::ToPositiveInfinity,
        }
    }
}

/// A finite exact base-10 decimal backed by a checked fixed-scale representation.
///
/// JSON serialization is always a decimal string, never a JSON number, so an
/// external floating-point parser cannot silently change the stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExactDecimal(Decimal);

impl ExactDecimal {
    /// Largest fractional scale representable by this foundation contract.
    pub const MAX_SCALE: u32 = Decimal::MAX_SCALE;
    /// Exact zero with scale zero.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Parses a strict base-10 literal without float intermediates or rounding.
    pub fn parse(value: &str) -> Result<Self, DecimalError> {
        Decimal::from_str_exact(value)
            .map(Self)
            .map_err(|error| DecimalError::Parse(error.to_string()))
    }

    /// Creates a value from an integer mantissa and explicit decimal scale.
    pub fn from_i128_scaled(value: i128, scale: u32) -> Result<Self, DecimalError> {
        ensure_scale(scale)?;
        Ok(Self(Decimal::from_i128_with_scale(value, scale)))
    }

    /// Converts a finite f64 explicitly, retaining the float's represented digits.
    ///
    /// This is deliberately not a `From<f64>` implementation: `0.1_f64` becomes
    /// its binary approximation rather than the source literal `"0.1"`.
    pub fn from_f64_retain(value: f64) -> Result<Self, DecimalError> {
        if !value.is_finite() {
            return Err(DecimalError::NonFiniteFloat);
        }
        Decimal::from_f64_retain(value)
            .map(Self)
            .ok_or(DecimalError::Overflow)
    }

    /// Returns a canonical decimal string preserving this value's scale.
    pub fn to_decimal_string(self) -> String {
        self.0.to_string()
    }

    /// Returns the stored fractional scale.
    pub const fn scale(self) -> u32 {
        self.0.scale()
    }

    /// Converts to f64 only through this named, potentially lossy boundary.
    pub fn to_f64_lossy(self) -> Result<f64, DecimalError> {
        self.0.to_f64().ok_or(DecimalError::Overflow)
    }

    /// Converts to i128 only when the value is integral and representable.
    pub fn to_i128_exact(self) -> Result<i128, DecimalError> {
        if !self.0.fract().is_zero() {
            return Err(DecimalError::NotExactInteger);
        }
        self.0.to_i128().ok_or(DecimalError::NotExactInteger)
    }

    /// Adds two exact decimals, failing rather than wrapping on overflow.
    pub fn checked_add(self, rhs: Self) -> Result<Self, DecimalError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(DecimalError::Overflow)
    }

    /// Subtracts two exact decimals, failing rather than wrapping on overflow.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, DecimalError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(DecimalError::Overflow)
    }

    /// Multiplies two exact decimals, failing rather than wrapping on overflow.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, DecimalError> {
        self.0
            .checked_mul(rhs.0)
            .map(Self)
            .ok_or(DecimalError::Overflow)
    }

    /// Divides then rounds to the caller-selected scale and mode.
    ///
    /// Repeating fractions are bounded by the underlying 28-digit decimal
    /// representation before this explicit final rounding step.
    pub fn checked_div_round(
        self,
        rhs: Self,
        scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, DecimalError> {
        ensure_scale(scale)?;
        if rhs.0.is_zero() {
            return Err(DecimalError::DivisionByZero);
        }
        self.0
            .checked_div(rhs.0)
            .map(|value| Self(value.round_dp_with_strategy(scale, mode.strategy())))
            .ok_or(DecimalError::Overflow)
    }

    /// Rounds to an explicit scale and named mode.
    pub fn round_to_scale(self, scale: u32, mode: RoundingMode) -> Result<Self, DecimalError> {
        ensure_scale(scale)?;
        Ok(Self(self.0.round_dp_with_strategy(scale, mode.strategy())))
    }
}

impl fmt::Display for ExactDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ExactDecimal {
    type Err = DecimalError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for ExactDecimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_decimal_string())
    }
}

impl<'de> Deserialize<'de> for ExactDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

fn ensure_scale(scale: u32) -> Result<(), DecimalError> {
    (scale <= ExactDecimal::MAX_SCALE)
        .then_some(())
        .ok_or(DecimalError::InvalidScale {
            scale,
            maximum: ExactDecimal::MAX_SCALE,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn decimal_fraction_and_repeated_cents_are_exact() {
        let value = ExactDecimal::parse("0.1")
            .unwrap()
            .checked_add(ExactDecimal::parse("0.2").unwrap())
            .unwrap();
        assert_eq!(value.to_decimal_string(), "0.3");
        let cent = ExactDecimal::parse("0.01").unwrap();
        let total = (0..100)
            .try_fold(ExactDecimal::ZERO, |sum, _| sum.checked_add(cent))
            .unwrap();
        assert_eq!(total.to_decimal_string(), "1.00");
    }

    #[test]
    fn rounding_and_conversion_boundaries_are_explicit() {
        let value = ExactDecimal::parse("2.5").unwrap();
        assert_eq!(
            value
                .round_to_scale(0, RoundingMode::MidpointNearestEven)
                .unwrap()
                .to_decimal_string(),
            "2"
        );
        assert_eq!(
            value
                .round_to_scale(0, RoundingMode::MidpointAwayFromZero)
                .unwrap()
                .to_decimal_string(),
            "3"
        );
        assert!(ExactDecimal::parse("1.1").unwrap().to_i128_exact().is_err());
        assert!(ExactDecimal::from_f64_retain(f64::NAN).is_err());
        assert!(ExactDecimal::parse("1")
            .unwrap()
            .checked_div_round(ExactDecimal::ZERO, 2, RoundingMode::MidpointNearestEven)
            .is_err());
    }

    proptest! {
        #[test]
        fn parse_serialize_and_add_subtract_round_trip(mantissa in -1_000_000_i64..=1_000_000, scale in 0_u32..=6) {
            let value = ExactDecimal::from_i128_scaled(mantissa.into(), scale).unwrap();
            let serialized = serde_json::to_string(&value).unwrap();
            let recovered: ExactDecimal = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(recovered, value);
            let zero = value.checked_sub(value).unwrap();
            prop_assert_eq!(zero, ExactDecimal::ZERO);
        }
    }
}
