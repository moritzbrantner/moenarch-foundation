use std::cmp::Ordering;

use crate::{DetectError, Result, Timebase, Timestamp};

impl Timebase {
    /// Creates a validated positive timebase.
    pub fn try_new(num: i32, den: i32) -> Result<Self> {
        let value = Self::new(num, den);
        value.validate()?;
        Ok(value)
    }

    /// Validates that one tick represents a finite positive duration.
    pub fn validate(self) -> Result<()> {
        if self.num <= 0 || self.den <= 0 {
            return Err(DetectError::InvalidArgument(format!(
                "timebase numerator and denominator must be positive, got {}/{}",
                self.num, self.den
            )));
        }
        Ok(())
    }

    /// Returns seconds per tick after validating the timebase.
    pub fn checked_seconds_per_tick(self) -> Result<f64> {
        self.validate()?;
        Ok(self.seconds_per_tick())
    }
}

impl Timestamp {
    /// Creates a timestamp with a validated timebase.
    pub fn try_new(pts: i64, timebase: Timebase) -> Result<Self> {
        timebase.validate()?;
        Ok(Self::new(pts, timebase))
    }

    /// Validates the timestamp's timebase.
    pub fn validate(self) -> Result<()> {
        self.timebase.validate()
    }

    /// Returns the timestamp in seconds after validating its timebase.
    pub fn checked_seconds(self) -> Result<f64> {
        self.validate()?;
        Ok(self.seconds())
    }

    /// Compares two timestamps by media time, even when their timebases differ.
    ///
    /// The derived `Ord` implementation remains structural for compatibility.
    /// Use this method whenever chronological order matters.
    pub fn chronological_cmp(self, other: Self) -> Result<Ordering> {
        self.validate()?;
        other.validate()?;

        let left = i128::from(self.pts)
            .checked_mul(i128::from(self.timebase.num))
            .and_then(|value| value.checked_mul(i128::from(other.timebase.den)))
            .ok_or_else(|| time_overflow("chronological comparison overflowed"))?;
        let right = i128::from(other.pts)
            .checked_mul(i128::from(other.timebase.num))
            .and_then(|value| value.checked_mul(i128::from(self.timebase.den)))
            .ok_or_else(|| time_overflow("chronological comparison overflowed"))?;

        Ok(left.cmp(&right))
    }

    /// Returns whether two differently represented timestamps identify the same instant.
    pub fn same_instant(self, other: Self) -> Result<bool> {
        Ok(self.chronological_cmp(other)? == Ordering::Equal)
    }

    /// Rescales this timestamp to another timebase without losing precision.
    ///
    /// Returns an error when the destination timebase cannot represent the
    /// instant with an integral presentation timestamp.
    pub fn rescale_exact(self, timebase: Timebase) -> Result<Self> {
        self.validate()?;
        timebase.validate()?;

        let numerator = i128::from(self.pts)
            .checked_mul(i128::from(self.timebase.num))
            .and_then(|value| value.checked_mul(i128::from(timebase.den)))
            .ok_or_else(|| time_overflow("timestamp rescaling overflowed"))?;
        let denominator = i128::from(self.timebase.den)
            .checked_mul(i128::from(timebase.num))
            .ok_or_else(|| time_overflow("timestamp rescaling overflowed"))?;

        if numerator % denominator != 0 {
            return Err(DetectError::InvalidArgument(format!(
                "timestamp cannot be represented exactly in timebase {}/{}",
                timebase.num, timebase.den
            )));
        }

        let pts = i64::try_from(numerator / denominator).map_err(|_| {
            DetectError::InvalidArgument("rescaled timestamp is outside the i64 range".to_string())
        })?;
        Ok(Self::new(pts, timebase))
    }

    /// Adds two timestamps as exact rational media durations.
    ///
    /// The result preserves this timestamp's timebase when the other operand
    /// can be represented there exactly and the tick addition does not
    /// overflow. Otherwise the exact sum is reduced to a `1/den` timebase.
    /// The operation fails rather than rounding or converting through `f64`.
    pub fn checked_add_exact(self, other: Self) -> Result<Self> {
        self.validate()?;
        other.validate()?;

        if let Ok(other_in_self_base) = other.rescale_exact(self.timebase) {
            if let Some(pts) = self.pts.checked_add(other_in_self_base.pts) {
                return Ok(Self::new(pts, self.timebase));
            }
        }

        let left_numerator = i128::from(self.pts)
            .checked_mul(i128::from(self.timebase.num))
            .ok_or_else(|| time_overflow("timestamp addition overflowed"))?;
        let right_numerator = i128::from(other.pts)
            .checked_mul(i128::from(other.timebase.num))
            .ok_or_else(|| time_overflow("timestamp addition overflowed"))?;
        let left_denominator = i128::from(self.timebase.den);
        let right_denominator = i128::from(other.timebase.den);

        let numerator = left_numerator
            .checked_mul(right_denominator)
            .and_then(|left| {
                right_numerator
                    .checked_mul(left_denominator)
                    .and_then(|right| left.checked_add(right))
            })
            .ok_or_else(|| time_overflow("timestamp addition overflowed"))?;
        let denominator = left_denominator
            .checked_mul(right_denominator)
            .ok_or_else(|| time_overflow("timestamp addition overflowed"))?;
        let divisor = greatest_common_divisor(numerator, denominator);
        let reduced_numerator = numerator / divisor;
        let reduced_denominator = denominator / divisor;

        let pts = i64::try_from(reduced_numerator).map_err(|_| {
            DetectError::InvalidArgument("exact timestamp sum is outside the i64 range".to_string())
        })?;
        let den = i32::try_from(reduced_denominator).map_err(|_| {
            DetectError::InvalidArgument(
                "exact timestamp sum requires a timebase denominator outside the i32 range"
                    .to_string(),
            )
        })?;

        Ok(Self::new(pts, Timebase::try_new(1, den)?))
    }
}

/// A half-open media-time range `[start, end)` with exact rational endpoints.
///
/// Endpoints may use different valid timebases. Construction and queries use
/// exact integer arithmetic for ordering rather than floating-point seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaRange {
    /// Inclusive start timestamp.
    pub start: Timestamp,
    /// Exclusive end timestamp.
    pub end: Timestamp,
}

impl MediaRange {
    /// Creates a validated half-open media range.
    pub fn new(start: Timestamp, end: Timestamp) -> Result<Self> {
        let value = Self { start, end };
        value.validate()?;
        Ok(value)
    }

    /// Validates both endpoints and their chronological ordering.
    pub fn validate(self) -> Result<()> {
        if self.start.chronological_cmp(self.end)? == Ordering::Greater {
            return Err(DetectError::InvalidArgument(
                "media range end must not precede start".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns whether this range has zero duration.
    pub fn is_empty(self) -> Result<bool> {
        Ok(self.start.chronological_cmp(self.end)? == Ordering::Equal)
    }

    /// Returns the range duration in seconds for presentation or approximate analysis.
    pub fn duration_seconds(self) -> Result<f64> {
        self.validate()?;
        Ok(self.end.checked_seconds()? - self.start.checked_seconds()?)
    }

    /// Returns whether the timestamp lies inside this half-open range.
    pub fn contains(self, timestamp: Timestamp) -> Result<bool> {
        self.validate()?;
        timestamp.validate()?;
        Ok(
            self.start.chronological_cmp(timestamp)? != Ordering::Greater
                && timestamp.chronological_cmp(self.end)? == Ordering::Less,
        )
    }

    /// Returns whether two half-open ranges overlap.
    pub fn overlaps(self, other: Self) -> Result<bool> {
        self.validate()?;
        other.validate()?;
        Ok(self.start.chronological_cmp(other.end)? == Ordering::Less
            && other.start.chronological_cmp(self.end)? == Ordering::Less)
    }
}

fn greatest_common_divisor(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn time_overflow(message: &str) -> DetectError {
    DetectError::InvalidArgument(message.to_string())
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::MediaRange;
    use crate::{Timebase, Timestamp};

    #[test]
    fn validated_timebases_reject_zero_and_negative_tick_durations() {
        assert!(Timebase::try_new(1, 1_000).is_ok());
        assert!(Timebase::try_new(1, 0).is_err());
        assert!(Timebase::try_new(-1, 1_000).is_err());
    }

    #[test]
    fn timestamps_compare_chronologically_across_timebases() {
        let one_second = Timestamp::new(1, Timebase::new(1, 1));
        let nine_hundred_ms = Timestamp::new(900, Timebase::new(1, 1_000));
        let thousand_ms = Timestamp::new(1_000, Timebase::new(1, 1_000));

        assert_eq!(
            nine_hundred_ms.chronological_cmp(one_second).unwrap(),
            Ordering::Less
        );
        assert!(one_second.same_instant(thousand_ms).unwrap());
    }

    #[test]
    fn exact_rescaling_preserves_instants_and_rejects_rounding() {
        let timestamp = Timestamp::new(24, Timebase::new(1, 24));
        let milliseconds = timestamp.rescale_exact(Timebase::new(1, 1_000)).unwrap();
        assert_eq!(milliseconds.pts, 1_000);
        assert!(timestamp.rescale_exact(Timebase::new(1, 25)).is_ok());

        let one_frame = Timestamp::new(1, Timebase::new(1, 24));
        assert!(one_frame.rescale_exact(Timebase::new(1, 1_000)).is_err());
    }

    #[test]
    fn exact_addition_preserves_existing_base_when_possible() {
        let presentation = Timestamp::new(9, Timebase::new(1, 10));
        let duration = Timestamp::new(100, Timebase::new(1, 1_000));

        assert_eq!(
            presentation.checked_add_exact(duration).unwrap(),
            Timestamp::new(10, Timebase::new(1, 10))
        );
    }

    #[test]
    fn exact_addition_finds_reduced_common_base_without_rounding() {
        assert_eq!(
            Timestamp::new(1, Timebase::new(1, 6))
                .checked_add_exact(Timestamp::new(1, Timebase::new(1, 4)))
                .unwrap(),
            Timestamp::new(5, Timebase::new(1, 12))
        );
    }

    #[test]
    fn exact_addition_recovers_from_direct_tick_overflow() {
        assert_eq!(
            Timestamp::new(i64::MAX, Timebase::new(1, 1_000))
                .checked_add_exact(Timestamp::new(1, Timebase::new(1, 1_000)))
                .unwrap(),
            Timestamp::new(1_i64 << 60, Timebase::new(1, 125))
        );
    }

    #[test]
    fn media_ranges_are_half_open_and_cross_timebase_safe() {
        let range = MediaRange::new(
            Timestamp::new(500, Timebase::new(1, 1_000)),
            Timestamp::new(2, Timebase::new(1, 1)),
        )
        .unwrap();

        assert_eq!(range.duration_seconds().unwrap(), 1.5);
        assert!(range
            .contains(Timestamp::new(1_999, Timebase::new(1, 1_000)))
            .unwrap());
        assert!(!range
            .contains(Timestamp::new(2, Timebase::new(1, 1)))
            .unwrap());
        assert!(range
            .overlaps(
                MediaRange::new(
                    Timestamp::new(1_500, Timebase::new(1, 1_000)),
                    Timestamp::new(2_500, Timebase::new(1, 1_000)),
                )
                .unwrap(),
            )
            .unwrap());
    }
}
