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
        Decimal::try_from_i128_with_scale(value, scale)
            .map(Self)
            .map_err(|_| DecimalError::Overflow)
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
        checked_add_sub(self.0, rhs.0, false).map(Self)
    }

    /// Subtracts two exact decimals, failing rather than wrapping on overflow.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, DecimalError> {
        checked_add_sub(self.0, rhs.0, true).map(Self)
    }

    /// Multiplies two exact decimals, failing if the exact product cannot be
    /// represented without reducing precision or scale.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, DecimalError> {
        let (left, left_scale) = normalized_parts(self.0);
        let (right, right_scale) = normalized_parts(rhs.0);
        if left == 0 || right == 0 {
            return Ok(Self::ZERO);
        }
        let scale = left_scale
            .checked_add(right_scale)
            .ok_or(DecimalError::Overflow)?;
        exact_product(left, right, scale).map(Self)
    }

    /// Divides then rounds to the caller-selected scale and mode.
    ///
    /// The exact coefficient ratio is rounded once, directly at the requested
    /// scale, so no intermediate decimal rounding can move it across a midpoint.
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
        exact_div_round(self.0, rhs.0, scale, mode).map(Self)
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

fn checked_add_sub(left: Decimal, right: Decimal, subtract: bool) -> Result<Decimal, DecimalError> {
    let scale = left.scale().max(right.scale());
    let left_digits = scaled_magnitude(left.mantissa(), left.scale(), scale);
    let right_digits = scaled_magnitude(right.mantissa(), right.scale(), scale);
    let left_negative = left.is_sign_negative();
    let right_negative = right.is_sign_negative() ^ subtract;

    let (digits, negative) = if left_negative == right_negative {
        (add_magnitudes(&left_digits, &right_digits), left_negative)
    } else {
        match compare_magnitudes(&left_digits, &right_digits) {
            std::cmp::Ordering::Greater => (
                subtract_magnitudes(&left_digits, &right_digits),
                left_negative,
            ),
            std::cmp::Ordering::Less => (
                subtract_magnitudes(&right_digits, &left_digits),
                right_negative,
            ),
            std::cmp::Ordering::Equal => (vec![0], false),
        }
    };
    decimal_from_signed_digits(digits, negative, scale)
}

fn scaled_magnitude(mantissa: i128, current_scale: u32, target_scale: u32) -> Vec<u8> {
    let mut digits = decimal_digits_little_endian(mantissa.unsigned_abs());
    digits.splice(
        0..0,
        std::iter::repeat_n(0, (target_scale - current_scale) as usize),
    );
    digits
}

fn normalized_parts(value: Decimal) -> (i128, u32) {
    let mut mantissa = value.mantissa();
    let mut scale = value.scale();
    while scale > 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        scale -= 1;
    }
    (mantissa, scale)
}

fn decimal_from_signed_digits(
    mut digits: Vec<u8>,
    negative: bool,
    mut scale: u32,
) -> Result<Decimal, DecimalError> {
    while magnitude_exceeds_decimal_max(&digits) {
        if scale == 0 || digits.first() != Some(&0) {
            return Err(DecimalError::Overflow);
        }
        digits.remove(0);
        scale -= 1;
    }
    let coefficient = digits.iter().rev().try_fold(0_i128, |value, digit| {
        value.checked_mul(10)?.checked_add(i128::from(*digit))
    });
    let coefficient = coefficient.ok_or(DecimalError::Overflow)?;
    let signed = if negative { -coefficient } else { coefficient };
    Decimal::try_from_i128_with_scale(signed, scale).map_err(|_| DecimalError::Overflow)
}

fn add_magnitudes(left: &[u8], right: &[u8]) -> Vec<u8> {
    let length = left.len().max(right.len());
    let mut result = Vec::with_capacity(length + 1);
    let mut carry = 0_u8;
    for index in 0..length {
        let sum =
            left.get(index).copied().unwrap_or(0) + right.get(index).copied().unwrap_or(0) + carry;
        result.push(sum % 10);
        carry = sum / 10;
    }
    if carry != 0 {
        result.push(carry);
    }
    result
}

fn compare_magnitudes(left: &[u8], right: &[u8]) -> std::cmp::Ordering {
    let left_length = significant_length(left);
    let right_length = significant_length(right);
    left_length.cmp(&right_length).then_with(|| {
        left[..left_length]
            .iter()
            .rev()
            .cmp(right[..right_length].iter().rev())
    })
}

fn subtract_magnitudes(larger: &[u8], smaller: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(larger.len());
    let mut borrow = 0_i8;
    for (index, larger_digit) in larger.iter().enumerate() {
        let mut difference =
            *larger_digit as i8 - smaller.get(index).copied().unwrap_or(0) as i8 - borrow;
        if difference < 0 {
            difference += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        result.push(difference as u8);
    }
    result.truncate(significant_length(&result));
    result
}

fn significant_length(digits: &[u8]) -> usize {
    digits
        .iter()
        .rposition(|digit| *digit != 0)
        .map_or(1, |index| index + 1)
}

fn magnitude_exceeds_decimal_max(digits: &[u8]) -> bool {
    let maximum = decimal_digits_little_endian(Decimal::MAX.mantissa().unsigned_abs());
    compare_magnitudes(digits, &maximum) == std::cmp::Ordering::Greater
}

fn exact_product(left: i128, right: i128, mut scale: u32) -> Result<Decimal, DecimalError> {
    let left_digits = decimal_digits_little_endian(left.unsigned_abs());
    let right_digits = decimal_digits_little_endian(right.unsigned_abs());
    let mut product = vec![0_u16; left_digits.len() + right_digits.len()];

    for (left_index, left_digit) in left_digits.iter().enumerate() {
        for (right_index, right_digit) in right_digits.iter().enumerate() {
            product[left_index + right_index] += u16::from(*left_digit) * u16::from(*right_digit);
        }
    }
    for index in 0..product.len() - 1 {
        let carry = product[index] / 10;
        product[index] %= 10;
        product[index + 1] += carry;
    }
    while product.last() == Some(&0) {
        product.pop();
    }
    while scale > 0 && product.first() == Some(&0) {
        product.remove(0);
        scale -= 1;
    }
    if scale > ExactDecimal::MAX_SCALE {
        return Err(DecimalError::Overflow);
    }

    let coefficient = product.iter().rev().try_fold(0_u128, |value, digit| {
        value.checked_mul(10)?.checked_add(u128::from(*digit))
    });
    let coefficient = coefficient
        .filter(|value| *value <= Decimal::MAX.mantissa().unsigned_abs())
        .ok_or(DecimalError::Overflow)?;
    let coefficient = i128::try_from(coefficient).map_err(|_| DecimalError::Overflow)?;
    let signed = if (left < 0) ^ (right < 0) {
        -coefficient
    } else {
        coefficient
    };
    Decimal::try_from_i128_with_scale(signed, scale).map_err(|_| DecimalError::Overflow)
}

fn decimal_digits_little_endian(mut value: u128) -> Vec<u8> {
    let mut digits = Vec::new();
    if value == 0 {
        return vec![0];
    }
    while value > 0 {
        digits.push((value % 10) as u8);
        value /= 10;
    }
    digits
}

fn exact_div_round(
    left: Decimal,
    right: Decimal,
    target_scale: u32,
    mode: RoundingMode,
) -> Result<Decimal, DecimalError> {
    let negative = left.is_sign_negative() ^ right.is_sign_negative();
    let numerator = left.mantissa().unsigned_abs();
    if numerator == 0 {
        return Decimal::try_from_i128_with_scale(0, target_scale)
            .map_err(|_| DecimalError::Overflow);
    }
    let divisor = right.mantissa().unsigned_abs();
    let exponent = i64::from(right.scale()) + i64::from(target_scale) - i64::from(left.scale());

    let (coefficient, remainder, half_comparison) = if exponent >= 0 {
        divide_decimal_digits(numerator, exponent as u32, divisor)?
    } else {
        let denominator = 10_u128
            .checked_pow((-exponent) as u32)
            .and_then(|factor| divisor.checked_mul(factor));
        match denominator {
            Some(denominator) => {
                let quotient = numerator / denominator;
                let remainder = numerator % denominator;
                (quotient, remainder, compare_to_half(remainder, denominator))
            }
            None => (0, numerator, std::cmp::Ordering::Less),
        }
    };
    let increment = should_increment(coefficient, remainder, half_comparison, negative, mode);
    let coefficient = coefficient
        .checked_add(u128::from(increment))
        .filter(|value| *value <= Decimal::MAX.mantissa().unsigned_abs())
        .ok_or(DecimalError::Overflow)?;
    let coefficient = i128::try_from(coefficient).map_err(|_| DecimalError::Overflow)?;
    let signed = if negative { -coefficient } else { coefficient };
    Decimal::try_from_i128_with_scale(signed, target_scale).map_err(|_| DecimalError::Overflow)
}

fn divide_decimal_digits(
    numerator: u128,
    appended_zeroes: u32,
    divisor: u128,
) -> Result<(u128, u128, std::cmp::Ordering), DecimalError> {
    let mut digits: Vec<u8> = numerator
        .to_string()
        .bytes()
        .map(|byte| byte - b'0')
        .collect();
    digits.extend(std::iter::repeat_n(0, appended_zeroes as usize));

    let mut quotient = 0_u128;
    let mut remainder = 0_u128;
    for digit in digits {
        remainder = remainder
            .checked_mul(10)
            .and_then(|value| value.checked_add(u128::from(digit)))
            .ok_or(DecimalError::Overflow)?;
        let quotient_digit = remainder / divisor;
        remainder %= divisor;
        quotient = quotient
            .checked_mul(10)
            .and_then(|value| value.checked_add(quotient_digit))
            .ok_or(DecimalError::Overflow)?;
    }
    Ok((quotient, remainder, compare_to_half(remainder, divisor)))
}

fn compare_to_half(remainder: u128, divisor: u128) -> std::cmp::Ordering {
    remainder.cmp(&(divisor / 2)).then_with(|| {
        if divisor.is_multiple_of(2) {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        }
    })
}

fn should_increment(
    coefficient: u128,
    remainder: u128,
    half_comparison: std::cmp::Ordering,
    negative: bool,
    mode: RoundingMode,
) -> bool {
    use std::cmp::Ordering::{Equal, Greater};

    match mode {
        RoundingMode::MidpointNearestEven => {
            half_comparison == Greater || (half_comparison == Equal && coefficient % 2 == 1)
        }
        RoundingMode::MidpointAwayFromZero => matches!(half_comparison, Equal | Greater),
        RoundingMode::MidpointTowardZero => half_comparison == Greater,
        RoundingMode::TowardZero => false,
        RoundingMode::AwayFromZero => remainder != 0,
        RoundingMode::TowardNegativeInfinity => negative && remainder != 0,
        RoundingMode::TowardPositiveInfinity => !negative && remainder != 0,
    }
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
        let cases = [
            (RoundingMode::MidpointNearestEven, "2", "-2"),
            (RoundingMode::MidpointAwayFromZero, "3", "-3"),
            (RoundingMode::MidpointTowardZero, "2", "-2"),
            (RoundingMode::TowardZero, "2", "-2"),
            (RoundingMode::AwayFromZero, "3", "-3"),
            (RoundingMode::TowardNegativeInfinity, "2", "-3"),
            (RoundingMode::TowardPositiveInfinity, "3", "-2"),
        ];
        for (mode, positive, negative) in cases {
            assert_eq!(
                ExactDecimal::parse("2.5")
                    .unwrap()
                    .round_to_scale(0, mode)
                    .unwrap()
                    .to_decimal_string(),
                positive
            );
            assert_eq!(
                ExactDecimal::parse("-2.5")
                    .unwrap()
                    .round_to_scale(0, mode)
                    .unwrap()
                    .to_decimal_string(),
                negative
            );
        }
        assert!(ExactDecimal::parse("1.1").unwrap().to_i128_exact().is_err());
        assert!(ExactDecimal::from_f64_retain(f64::NAN).is_err());
        assert!(ExactDecimal::from_i128_scaled(i128::MAX, 0).is_err());
        assert!(ExactDecimal::from_i128_scaled(1, ExactDecimal::MAX_SCALE + 1).is_err());
        assert_eq!(
            ExactDecimal::from_i128_scaled(1, ExactDecimal::MAX_SCALE)
                .unwrap()
                .to_decimal_string(),
            "0.0000000000000000000000000001"
        );
        assert!(ExactDecimal::parse("1")
            .unwrap()
            .checked_div_round(ExactDecimal::ZERO, 2, RoundingMode::MidpointNearestEven)
            .is_err());
        assert_eq!(
            ExactDecimal::parse("2")
                .unwrap()
                .checked_div_round(
                    ExactDecimal::parse("3").unwrap(),
                    2,
                    RoundingMode::MidpointNearestEven,
                )
                .unwrap()
                .to_decimal_string(),
            "0.67"
        );
        let division_midpoints = [
            (RoundingMode::MidpointNearestEven, "2", "-2"),
            (RoundingMode::MidpointAwayFromZero, "3", "-3"),
            (RoundingMode::MidpointTowardZero, "2", "-2"),
            (RoundingMode::TowardZero, "2", "-2"),
            (RoundingMode::AwayFromZero, "3", "-3"),
            (RoundingMode::TowardNegativeInfinity, "2", "-3"),
            (RoundingMode::TowardPositiveInfinity, "3", "-2"),
        ];
        for (mode, positive, negative) in division_midpoints {
            assert_eq!(
                ExactDecimal::parse("5")
                    .unwrap()
                    .checked_div_round(ExactDecimal::parse("2").unwrap(), 0, mode)
                    .unwrap()
                    .to_decimal_string(),
                positive
            );
            assert_eq!(
                ExactDecimal::parse("-5")
                    .unwrap()
                    .checked_div_round(ExactDecimal::parse("2").unwrap(), 0, mode)
                    .unwrap()
                    .to_decimal_string(),
                negative
            );
        }
        assert_eq!(
            ExactDecimal::parse("1")
                .unwrap()
                .checked_div_round(
                    ExactDecimal::parse("2").unwrap(),
                    2,
                    RoundingMode::MidpointNearestEven,
                )
                .unwrap()
                .to_decimal_string(),
            "0.50"
        );
        let large_odd_denominator = ExactDecimal::parse("79228162514264337593543950335").unwrap();
        let below_half = ExactDecimal::parse("39614081257132168796771975167").unwrap();
        let above_half = ExactDecimal::parse("39614081257132168796771975168").unwrap();
        assert_eq!(
            below_half
                .checked_div_round(large_odd_denominator, 0, RoundingMode::MidpointAwayFromZero,)
                .unwrap()
                .to_decimal_string(),
            "0"
        );
        assert_eq!(
            above_half
                .checked_div_round(large_odd_denominator, 0, RoundingMode::MidpointTowardZero,)
                .unwrap()
                .to_decimal_string(),
            "1"
        );
        let tiny = ExactDecimal::parse("0.0000000000000000000000000001").unwrap();
        assert_eq!(
            tiny.checked_div_round(large_odd_denominator, 0, RoundingMode::AwayFromZero,)
                .unwrap()
                .to_decimal_string(),
            "1"
        );
    }

    #[test]
    fn exact_arithmetic_rejects_implicit_precision_loss() {
        let maximum = ExactDecimal::parse("79228162514264337593543950335").unwrap();
        let quantum = ExactDecimal::parse("0.0000000000000000000000000001").unwrap();
        let scaled_zero = ExactDecimal::parse("0.0000000000000000000000000000").unwrap();
        assert_eq!(maximum.checked_add(scaled_zero).unwrap(), maximum);
        assert_eq!(maximum.checked_sub(scaled_zero).unwrap(), maximum);
        assert_eq!(
            ExactDecimal::parse("0.00")
                .unwrap()
                .checked_add(ExactDecimal::parse("1.0").unwrap())
                .unwrap()
                .to_decimal_string(),
            "1.00"
        );
        assert_eq!(
            maximum
                .checked_add(ExactDecimal::parse("-1.0").unwrap())
                .unwrap(),
            ExactDecimal::parse("79228162514264337593543950334").unwrap()
        );
        let high_scale_one = ExactDecimal::parse("1.0000000000000000000000000000").unwrap();
        assert_eq!(
            maximum
                .checked_add(ExactDecimal::parse("-1.0000000000000000000000000000").unwrap(),)
                .unwrap(),
            ExactDecimal::parse("79228162514264337593543950334").unwrap()
        );
        assert_eq!(
            maximum.checked_sub(high_scale_one).unwrap(),
            ExactDecimal::parse("79228162514264337593543950334").unwrap()
        );
        assert_eq!(
            ExactDecimal::parse("1.20")
                .unwrap()
                .checked_add(ExactDecimal::parse("2.3").unwrap())
                .unwrap()
                .to_decimal_string(),
            "3.50"
        );
        assert_eq!(
            maximum.checked_add(ExactDecimal::parse("1").unwrap()),
            Err(DecimalError::Overflow)
        );
        assert_eq!(maximum.checked_add(quantum), Err(DecimalError::Overflow));
        assert_eq!(
            maximum.checked_sub(ExactDecimal::parse("-1").unwrap()),
            Err(DecimalError::Overflow)
        );
        assert_eq!(
            ExactDecimal::parse("0.00000000000000000001")
                .unwrap()
                .checked_mul(ExactDecimal::parse("0.00000000000000000001").unwrap()),
            Err(DecimalError::Overflow)
        );
        assert_eq!(
            ExactDecimal::parse("1.2300")
                .unwrap()
                .checked_mul(ExactDecimal::parse("2.0").unwrap())
                .unwrap()
                .to_decimal_string(),
            "2.46"
        );
        assert_eq!(
            ExactDecimal::parse("1.2300")
                .unwrap()
                .checked_mul(ExactDecimal::ZERO)
                .unwrap()
                .to_decimal_string(),
            "0"
        );
        assert_eq!(
            ExactDecimal::parse("0.00000000000000000004")
                .unwrap()
                .checked_mul(ExactDecimal::parse("0.000000025").unwrap())
                .unwrap()
                .to_decimal_string(),
            "0.000000000000000000000000001"
        );
    }

    #[test]
    fn serialization_shape_and_rounding_mode_names_are_stable() {
        let scaled = ExactDecimal::parse("1.2300").unwrap();
        assert_eq!(serde_json::to_string(&scaled).unwrap(), r#""1.2300""#);
        let recovered: ExactDecimal = serde_json::from_str(r#""1.2300""#).unwrap();
        assert_eq!(recovered.scale(), 4);
        assert_eq!(recovered.to_decimal_string(), "1.2300");
        assert!(serde_json::from_str::<ExactDecimal>("1.2300").is_err());

        let modes = [
            (
                RoundingMode::MidpointNearestEven,
                r#""MidpointNearestEven""#,
            ),
            (
                RoundingMode::MidpointAwayFromZero,
                r#""MidpointAwayFromZero""#,
            ),
            (RoundingMode::MidpointTowardZero, r#""MidpointTowardZero""#),
            (RoundingMode::TowardZero, r#""TowardZero""#),
            (RoundingMode::AwayFromZero, r#""AwayFromZero""#),
            (
                RoundingMode::TowardNegativeInfinity,
                r#""TowardNegativeInfinity""#,
            ),
            (
                RoundingMode::TowardPositiveInfinity,
                r#""TowardPositiveInfinity""#,
            ),
        ];
        for (mode, expected) in modes {
            assert_eq!(serde_json::to_string(&mode).unwrap(), expected);
        }
    }

    proptest! {
        #[test]
        fn parse_serialize_and_add_subtract_round_trip(
            mantissa in -1_000_000_i64..=1_000_000,
            other_mantissa in -1_000_000_i64..=1_000_000,
            scale in 0_u32..=6,
        ) {
            let value = ExactDecimal::from_i128_scaled(mantissa.into(), scale).unwrap();
            let other = ExactDecimal::from_i128_scaled(other_mantissa.into(), scale).unwrap();
            let serialized = serde_json::to_string(&value).unwrap();
            let recovered: ExactDecimal = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(recovered, value);
            prop_assert_eq!(recovered.scale(), scale);
            let zero = value.checked_sub(value).unwrap();
            prop_assert_eq!(zero, ExactDecimal::ZERO);
            let sum = value.checked_add(other).unwrap();
            prop_assert_eq!(sum.checked_sub(other).unwrap(), value);
        }

        #[test]
        fn multiplication_is_exact_for_representable_products(
            left in -1_000_000_i64..=1_000_000,
            right in -1_000_000_i64..=1_000_000,
            left_scale in 0_u32..=6,
            right_scale in 0_u32..=6,
        ) {
            let left_value = ExactDecimal::from_i128_scaled(left.into(), left_scale).unwrap();
            let right_value = ExactDecimal::from_i128_scaled(right.into(), right_scale).unwrap();
            let actual = left_value.checked_mul(right_value).unwrap();
            let expected = ExactDecimal::from_i128_scaled(
                i128::from(left) * i128::from(right),
                left_scale + right_scale,
            ).unwrap();
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn cross_scale_addition_matches_an_integer_reference(
            left in -1_000_000_i64..=1_000_000,
            right in -1_000_000_i64..=1_000_000,
            left_scale in 0_u32..=8,
            right_scale in 0_u32..=8,
        ) {
            let left_value = ExactDecimal::from_i128_scaled(left.into(), left_scale).unwrap();
            let right_value = ExactDecimal::from_i128_scaled(right.into(), right_scale).unwrap();
            let target_scale = left_scale.max(right_scale);
            let left_aligned = i128::from(left) * 10_i128.pow(target_scale - left_scale);
            let right_aligned = i128::from(right) * 10_i128.pow(target_scale - right_scale);
            let expected = ExactDecimal::from_i128_scaled(left_aligned + right_aligned, target_scale).unwrap();
            let actual = left_value.checked_add(right_value).unwrap();
            prop_assert_eq!(actual, expected);
            prop_assert_eq!(actual.scale(), target_scale);
        }

        #[test]
        fn lossless_integer_conversion_round_trips(
            value in -9_000_000_000_000_000_i64..=9_000_000_000_000_000,
        ) {
            let decimal = ExactDecimal::from_i128_scaled(i128::from(value), 0).unwrap();
            prop_assert_eq!(decimal.to_i128_exact().unwrap(), i128::from(value));
            let float = decimal.to_f64_lossy().unwrap();
            prop_assert_eq!(float, value as f64);
            prop_assert_eq!(ExactDecimal::from_f64_retain(float).unwrap(), decimal);
        }

        #[test]
        fn division_obeys_rounding_invariants(
            numerator in -1_000_000_i64..=1_000_000,
            denominator in 1_u64..=1_000_000,
            target_scale in 0_u32..=6,
        ) {
            let left = ExactDecimal::from_i128_scaled(i128::from(numerator), 0).unwrap();
            let right = ExactDecimal::from_i128_scaled(i128::from(denominator), 0).unwrap();
            let scaled_numerator = i128::from(numerator).unsigned_abs() * 10_u128.pow(target_scale);
            let floor = scaled_numerator / u128::from(denominator);
            let remainder = scaled_numerator % u128::from(denominator);
            let ceiling = floor + u128::from(remainder != 0);
            let negative = numerator < 0;
            let modes = [
                RoundingMode::MidpointNearestEven,
                RoundingMode::MidpointAwayFromZero,
                RoundingMode::MidpointTowardZero,
                RoundingMode::TowardZero,
                RoundingMode::AwayFromZero,
                RoundingMode::TowardNegativeInfinity,
                RoundingMode::TowardPositiveInfinity,
            ];
            for mode in modes {
                let actual = left.checked_div_round(right, target_scale, mode).unwrap();
                let magnitude = actual.0.mantissa().unsigned_abs();
                prop_assert_eq!(actual.scale(), target_scale);
                prop_assert!(magnitude == floor || magnitude == ceiling);
                match mode {
                    RoundingMode::TowardZero => prop_assert_eq!(magnitude, floor),
                    RoundingMode::AwayFromZero => prop_assert_eq!(magnitude, ceiling),
                    RoundingMode::TowardNegativeInfinity if negative => {
                        prop_assert_eq!(magnitude, ceiling);
                    }
                    RoundingMode::TowardNegativeInfinity => prop_assert_eq!(magnitude, floor),
                    RoundingMode::TowardPositiveInfinity if negative => {
                        prop_assert_eq!(magnitude, floor);
                    }
                    RoundingMode::TowardPositiveInfinity => prop_assert_eq!(magnitude, ceiling),
                    _ => {}
                }
                if remainder == 0 {
                    prop_assert_eq!(magnitude, floor);
                }
            }
        }

        #[test]
        fn coefficient_boundaries_preserve_value_and_scale(
            offset in 0_u64..=1_000_000,
            scale in 0_u32..=ExactDecimal::MAX_SCALE,
            negative in any::<bool>(),
        ) {
            let magnitude = Decimal::MAX.mantissa() - i128::from(offset);
            let mantissa = if negative { -magnitude } else { magnitude };
            let value = ExactDecimal::from_i128_scaled(mantissa, scale).unwrap();
            prop_assert_eq!(value.0.mantissa(), mantissa);
            prop_assert_eq!(value.scale(), scale);
            let serialized = serde_json::to_string(&value).unwrap();
            let recovered: ExactDecimal = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(recovered.0.mantissa(), mantissa);
            prop_assert_eq!(recovered.scale(), scale);
        }
    }
}
