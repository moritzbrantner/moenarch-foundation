use media_core::{DetectError, Result};

/// Configuration for dynamic time warping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DtwConfig {
    /// Optional Sakoe-Chiba radius around the diagonal.
    ///
    /// A radius smaller than the input-length difference is rejected instead
    /// of being widened implicitly.
    pub window: Option<usize>,
}

/// Evidence returned by a dynamic-time-warping comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DtwReport {
    /// Minimum accumulated absolute-difference cost.
    pub distance: f64,
    /// Distance divided by the deterministic chosen path length.
    pub normalized_distance: f64,
    /// Number of aligned cells on the chosen minimum-cost path.
    pub path_length: usize,
    /// Number of dynamic-programming cells evaluated.
    pub cells_evaluated: usize,
    /// Maximum number of DP cells retained at once.
    pub working_set_cells: usize,
    /// Effective Sakoe-Chiba radius used by the computation.
    pub effective_window: usize,
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    cost: f64,
    steps: usize,
}

impl Cell {
    const UNREACHABLE: Self = Self {
        cost: f64::INFINITY,
        steps: usize::MAX,
    };

    const ORIGIN: Self = Self {
        cost: 0.0,
        steps: 0,
    };
}

/// Computes deterministic dynamic time warping over finite scalar signals.
///
/// Local distance is absolute sample difference. The implementation retains
/// only two DP rows, choosing the shorter input as the row width, so auxiliary
/// memory is `O(min(n, m))`. `DtwConfig::window` optionally applies a
/// Sakoe-Chiba band and therefore bounds evaluated work to the cells inside the
/// band. Equal-cost predecessor paths prefer fewer steps, then diagonal, up,
/// and left order.
pub fn dynamic_time_warping(left: &[f32], right: &[f32], config: DtwConfig) -> Result<DtwReport> {
    validate_signal("left", left)?;
    validate_signal("right", right)?;

    let original_difference = left.len().abs_diff(right.len());
    if config
        .window
        .is_some_and(|window| window < original_difference)
    {
        return Err(invalid_argument(format!(
            "DTW window must be at least the input-length difference ({original_difference})"
        )));
    }

    let (rows, columns) = if left.len() >= right.len() {
        (left, right)
    } else {
        (right, left)
    };
    let effective_window = config.window.unwrap_or(rows.len().max(columns.len()));

    let mut previous = vec![Cell::UNREACHABLE; columns.len() + 1];
    let mut current = vec![Cell::UNREACHABLE; columns.len() + 1];
    previous[0] = Cell::ORIGIN;
    let mut cells_evaluated = 0_usize;

    for row in 1..=rows.len() {
        current.fill(Cell::UNREACHABLE);
        let start = row.saturating_sub(effective_window).max(1);
        let end = row.saturating_add(effective_window).min(columns.len());

        for column in start..=end {
            let predecessor =
                best_predecessor(previous[column - 1], previous[column], current[column - 1]);
            if !predecessor.cost.is_finite() {
                continue;
            }

            let local = (f64::from(rows[row - 1]) - f64::from(columns[column - 1])).abs();
            current[column] = Cell {
                cost: predecessor.cost + local,
                steps: predecessor.steps + 1,
            };
            cells_evaluated += 1;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    let result = previous[columns.len()];
    if !result.cost.is_finite() || result.steps == 0 {
        return Err(invalid_argument(
            "DTW alignment is unreachable under the requested window",
        ));
    }

    Ok(DtwReport {
        distance: result.cost,
        normalized_distance: result.cost / result.steps as f64,
        path_length: result.steps,
        cells_evaluated,
        working_set_cells: 2 * (columns.len() + 1),
        effective_window,
    })
}

fn best_predecessor(diagonal: Cell, up: Cell, left: Cell) -> Cell {
    let mut best = diagonal;
    for candidate in [up, left] {
        if candidate.cost < best.cost
            || (candidate.cost == best.cost && candidate.steps < best.steps)
        {
            best = candidate;
        }
    }
    best
}

fn validate_signal(name: &str, samples: &[f32]) -> Result<()> {
    if samples.is_empty() {
        return Err(invalid_argument(format!(
            "DTW {name} signal must not be empty"
        )));
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid_argument(format!(
            "DTW {name} signal must contain only finite samples"
        )));
    }
    Ok(())
}

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_signals_have_zero_distance() {
        let report =
            dynamic_time_warping(&[0.0, 1.0, 2.0], &[0.0, 1.0, 2.0], DtwConfig::default()).unwrap();

        assert_eq!(report.distance, 0.0);
        assert_eq!(report.normalized_distance, 0.0);
        assert_eq!(report.path_length, 3);
        assert_eq!(report.working_set_cells, 8);
    }

    #[test]
    fn unequal_lengths_align_with_expected_cost() {
        let report =
            dynamic_time_warping(&[0.0, 1.0, 2.0], &[0.0, 2.0], DtwConfig::default()).unwrap();

        assert_eq!(report.distance, 1.0);
        assert!(report.path_length >= 3);
    }

    #[test]
    fn sufficient_band_matches_unbounded_result() {
        let left = [0.0, 1.0, 1.5, 3.0];
        let right = [0.0, 1.0, 3.0];
        let unbounded = dynamic_time_warping(&left, &right, DtwConfig::default()).unwrap();
        let banded = dynamic_time_warping(&left, &right, DtwConfig { window: Some(2) }).unwrap();

        assert_eq!(banded.distance, unbounded.distance);
        assert!(banded.cells_evaluated <= unbounded.cells_evaluated);
    }

    #[test]
    fn too_narrow_band_is_rejected_instead_of_widened() {
        let error =
            dynamic_time_warping(&[0.0, 1.0, 2.0, 3.0], &[0.0], DtwConfig { window: Some(2) })
                .unwrap_err();

        assert!(matches!(error, DetectError::InvalidArgument(_)));
    }

    #[test]
    fn rejects_empty_and_non_finite_inputs() {
        assert!(dynamic_time_warping(&[], &[0.0], DtwConfig::default()).is_err());
        assert!(dynamic_time_warping(&[f32::NAN], &[0.0], DtwConfig::default()).is_err());
    }
}
