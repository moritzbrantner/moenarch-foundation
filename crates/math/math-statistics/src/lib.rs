#![doc = include_str!("../README.md")]

pub mod surface;
use math_linear::{F32Matrix, F32MatrixView, F64Matrix, MatrixShape, PseudoinverseOptions};
use media_core::{DetectError, Result};
use numbers_core::{quantile, quartiles, NumberRange, RunningStats};

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variance normalization to use for second-moment statistics.
pub enum VarianceMode {
    /// Divide by `n`.
    Population,
    /// Divide by `n - 1`.
    Sample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Method used to convert adjacent values into a derived series.
pub enum ChangeMethod {
    /// `next - previous`.
    Difference,
    /// `next / previous - 1.0`, requiring strictly positive values.
    Relative,
    /// `ln(next / previous)`, requiring strictly positive values.
    LogRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Method used to assign ranks to tied values.
pub enum RankMethod {
    /// Tied values receive the average of their one-based rank positions.
    Average,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Summary statistics for a finite scalar series.
pub struct SeriesSummary {
    /// Number of observations.
    pub count: usize,
    /// Arithmetic mean.
    pub mean: f64,
    /// Variance using the requested normalization.
    pub variance: f64,
    /// Square root of variance.
    pub std_dev: f64,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// First quartile.
    pub q1: f64,
    /// Median.
    pub median: f64,
    /// Third quartile.
    pub q3: f64,
    /// Interquartile range.
    pub iqr: f64,
    /// Fisher-Pearson moment skewness, when variance is non-zero.
    pub skewness: Option<f64>,
    /// Moment excess kurtosis, when variance is non-zero.
    pub excess_kurtosis: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Pairwise summary for two equally sized finite scalar series.
pub struct PairwiseSummary {
    /// Arithmetic mean of the left series.
    pub left_mean: f64,
    /// Arithmetic mean of the right series.
    pub right_mean: f64,
    /// Covariance using the requested normalization.
    pub covariance: f64,
    /// Pearson correlation.
    pub correlation: f64,
    /// Number of paired observations.
    pub observations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Tail-risk summary using empirical quantiles.
pub struct TailRisk {
    /// Confidence level in the exclusive range `(0, 1)`.
    pub confidence: f64,
    /// Return/value threshold at `1.0 - confidence`.
    pub threshold: f64,
    /// Positive loss amount at the requested confidence.
    pub value_at_risk: f64,
    /// Positive average tail loss at or below the threshold.
    pub conditional_value_at_risk: f64,
    /// Number of observations used.
    pub observations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Maximum drawdown details for a compounded path.
pub struct Drawdown {
    /// Positive drawdown depth as a fraction of prior peak equity.
    pub depth: f64,
    /// Index of the equity peak before the drawdown.
    pub peak_index: usize,
    /// Index of the equity trough.
    pub trough_index: usize,
    /// First index where equity recovered to the prior peak, if any.
    pub recovery_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Simple linear regression summary for paired scalar observations.
pub struct LinearRegressionSummary {
    /// Slope coefficient.
    pub slope: f64,
    /// Intercept coefficient.
    pub intercept: f64,
    /// Coefficient of determination.
    pub r_squared: f64,
    /// Residual standard error.
    pub residual_std_error: f64,
    /// Number of paired observations.
    pub observations: usize,
}

#[derive(Debug, Clone, PartialEq)]
/// Ordinary least-squares regression result for a dense design matrix.
pub struct OlsRegression {
    /// Fitted coefficients, one per design column.
    pub coefficients: Vec<f32>,
    /// Fitted target values.
    pub fitted: Vec<f32>,
    /// Residuals as `target - fitted`.
    pub residuals: Vec<f32>,
    /// Coefficient of determination.
    pub r_squared: f32,
    /// Number of observations.
    pub observations: usize,
    /// Number of predictors/design columns.
    pub predictors: usize,
}

#[derive(Debug, Clone, PartialEq)]
/// OLS regression result with residual and coefficient diagnostics.
pub struct OlsRegressionDiagnostics {
    /// Base OLS regression payload.
    pub regression: OlsRegression,
    /// Sum of squared residuals.
    pub residual_sum_squares: f32,
    /// Total target sum of squares.
    pub total_sum_squares: f32,
    /// Mean squared error when degrees of freedom are available.
    pub mean_squared_error: f32,
    /// Square root of mean squared error.
    pub root_mean_squared_error: f32,
    /// Adjusted coefficient of determination.
    pub adjusted_r_squared: f32,
    /// Residual degrees of freedom.
    pub degrees_of_freedom: usize,
    /// Per-coefficient standard errors when finite and positive.
    pub standard_errors: Vec<Option<f32>>,
    /// Per-coefficient t-statistics when standard errors are available.
    pub t_statistics: Vec<Option<f32>>,
}

#[derive(Debug, Clone, PartialEq)]
/// Deterministic ridge regression result from regularized normal equations.
pub struct RidgeRegression {
    /// Fitted coefficients, one per design column.
    pub coefficients: Vec<f32>,
    /// Fitted target values.
    pub fitted: Vec<f32>,
    /// Residuals as `target - fitted`.
    pub residuals: Vec<f32>,
    /// Coefficient of determination.
    pub r_squared: f32,
    /// Regularization strength.
    pub lambda: f32,
    /// Number of observations.
    pub observations: usize,
    /// Number of predictors/design columns.
    pub predictors: usize,
}

/// Returns summary statistics for a finite scalar series.
pub fn summarize_series(values: &[f64], mode: VarianceMode) -> Result<SeriesSummary> {
    validate_series(values, "series")?;
    let mean = mean(values)?;
    let variance = variance(values, mode)?;
    let std_dev = variance.sqrt();
    let quartiles = quartiles(values)?;
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (skewness, excess_kurtosis) = higher_moments(values, mean, std_dev);
    Ok(SeriesSummary {
        count: values.len(),
        mean,
        variance,
        std_dev,
        min,
        max,
        q1: quartiles.q1,
        median: quartiles.median,
        q3: quartiles.q3,
        iqr: quartiles.iqr,
        skewness,
        excess_kurtosis,
    })
}

/// Returns the arithmetic mean of a finite scalar series.
pub fn mean(values: &[f64]) -> Result<f64> {
    validate_series(values, "series")?;
    Ok(values.iter().sum::<f64>() / values.len() as f64)
}

/// Returns variance for a finite scalar series.
pub fn variance(values: &[f64], mode: VarianceMode) -> Result<f64> {
    validate_series(values, "series")?;
    let denom = variance_denominator(values.len(), mode)?;
    let mean = mean(values)?;
    Ok(values
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>()
        / denom)
}

/// Returns standard deviation for a finite scalar series.
pub fn std_dev(values: &[f64], mode: VarianceMode) -> Result<f64> {
    Ok(variance(values, mode)?.sqrt())
}

/// Returns covariance between two equally sized finite scalar series.
pub fn covariance(left: &[f64], right: &[f64], mode: VarianceMode) -> Result<f64> {
    validate_pair(left, right)?;
    let denom = variance_denominator(left.len(), mode)?;
    let left_mean = mean(left)?;
    let right_mean = mean(right)?;
    Ok(left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>()
        / denom)
}

/// Returns Pearson correlation between two equally sized finite scalar series.
pub fn correlation(left: &[f64], right: &[f64]) -> Result<f64> {
    let cov = covariance(left, right, VarianceMode::Sample)?;
    let left_std = std_dev(left, VarianceMode::Sample)?;
    let right_std = std_dev(right, VarianceMode::Sample)?;
    let denom = left_std * right_std;
    if denom <= f64::EPSILON {
        return Err(invalid_argument("correlation requires non-zero variance"));
    }
    Ok((cov / denom).clamp(-1.0, 1.0))
}

/// Returns pairwise mean, covariance, and correlation for two scalar series.
pub fn summarize_pair(left: &[f64], right: &[f64], mode: VarianceMode) -> Result<PairwiseSummary> {
    validate_pair(left, right)?;
    Ok(PairwiseSummary {
        left_mean: mean(left)?,
        right_mean: mean(right)?,
        covariance: covariance(left, right, mode)?,
        correlation: correlation(left, right)?,
        observations: left.len(),
    })
}

/// Converts adjacent finite values into differences, relative changes, or log-ratios.
pub fn changes(values: &[f64], method: ChangeMethod) -> Result<Vec<f64>> {
    validate_change_input(values, method)?;
    Ok(values
        .windows(2)
        .map(|window| match method {
            ChangeMethod::Difference => window[1] - window[0],
            ChangeMethod::Relative => window[1] / window[0] - 1.0,
            ChangeMethod::LogRatio => (window[1] / window[0]).ln(),
        })
        .collect())
}

/// Returns compounded cumulative return/change from period returns.
pub fn cumulative_compounded_return(values: &[f64]) -> Result<f64> {
    validate_compoundable(values)?;
    Ok(values
        .iter()
        .fold(1.0, |equity, value| equity * (1.0 + value))
        - 1.0)
}

/// Returns maximum drawdown for a compounded path.
pub fn max_drawdown(values: &[f64]) -> Result<Drawdown> {
    validate_compoundable(values)?;
    let mut equity = 1.0;
    let mut peak = 1.0;
    let mut peak_index = 0;
    let mut max_drawdown = Drawdown {
        depth: 0.0,
        peak_index: 0,
        trough_index: 0,
        recovery_index: Some(0),
    };
    let mut active_peak_index = 0;

    for (index, value) in values.iter().enumerate() {
        let equity_index = index + 1;
        equity *= 1.0 + value;
        if equity >= peak {
            peak = equity;
            peak_index = equity_index;
            active_peak_index = equity_index;
            if max_drawdown.recovery_index.is_none() && equity >= peak {
                max_drawdown.recovery_index = Some(equity_index);
            }
        }
        let depth = if peak > 0.0 {
            (peak - equity) / peak
        } else {
            0.0
        };
        if depth > max_drawdown.depth {
            max_drawdown = Drawdown {
                depth,
                peak_index: active_peak_index,
                trough_index: equity_index,
                recovery_index: None,
            };
        } else if max_drawdown.recovery_index.is_none()
            && peak_index > max_drawdown.trough_index
            && equity >= peak
        {
            max_drawdown.recovery_index = Some(equity_index);
        }
    }

    Ok(max_drawdown)
}

/// Returns empirical VaR/CVaR style tail risk as positive loss values.
pub fn tail_risk(values: &[f64], confidence: f64) -> Result<TailRisk> {
    validate_series(values, "tail risk series")?;
    if !confidence.is_finite() || confidence <= 0.0 || confidence >= 1.0 {
        return Err(invalid_argument(
            "confidence must be finite and between 0 and 1",
        ));
    }
    let threshold = quantile(values, 1.0 - confidence)?;
    let tail = values
        .iter()
        .copied()
        .filter(|value| *value <= threshold)
        .collect::<Vec<_>>();
    let tail_mean = tail.iter().sum::<f64>() / tail.len() as f64;
    Ok(TailRisk {
        confidence,
        threshold,
        value_at_risk: (-threshold).max(0.0),
        conditional_value_at_risk: (-tail_mean).max(0.0),
        observations: values.len(),
    })
}

/// Returns rolling arithmetic means for fixed-size windows.
pub fn rolling_mean(values: &[f64], window: usize) -> Result<Vec<f64>> {
    rolling_apply(values, window, mean)
}

/// Returns rolling standard deviations for fixed-size windows.
pub fn rolling_std_dev(values: &[f64], window: usize, mode: VarianceMode) -> Result<Vec<f64>> {
    rolling_apply(values, window, |window| std_dev(window, mode))
}

/// Returns rolling correlations for fixed-size paired windows.
pub fn rolling_correlation(left: &[f64], right: &[f64], window: usize) -> Result<Vec<f64>> {
    validate_pair(left, right)?;
    validate_window(window, left.len())?;
    let mut values = Vec::with_capacity(left.len() + 1 - window);
    for start in 0..=(left.len() - window) {
        values.push(correlation(
            &left[start..start + window],
            &right[start..start + window],
        )?);
    }
    Ok(values)
}

/// Returns rolling min/max ranges for fixed-size windows.
pub fn rolling_min_max(values: &[f64], window: usize) -> Result<Vec<NumberRange>> {
    validate_series(values, "series")?;
    validate_window(window, values.len())?;
    let mut ranges = Vec::with_capacity(values.len() + 1 - window);
    for start in 0..=(values.len() - window) {
        let slice = &values[start..start + window];
        let min = slice.iter().copied().fold(f64::INFINITY, f64::min);
        let max = slice.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        ranges.push(NumberRange::new(min, max)?);
    }
    Ok(ranges)
}

/// Returns z-scores using the requested variance normalization.
pub fn z_scores(values: &[f64], mode: VarianceMode) -> Result<Vec<f64>> {
    let mean = mean(values)?;
    let std = std_dev(values, mode)?;
    if std <= f64::EPSILON {
        return Err(invalid_argument("z-scores require non-zero variance"));
    }
    Ok(values.iter().map(|value| (value - mean) / std).collect())
}

/// Returns one-based average ranks for finite scalar values.
pub fn rank_values(values: &[f64]) -> Result<Vec<f64>> {
    validate_series(values, "series")?;
    let mut indexed = values
        .iter()
        .copied()
        .enumerate()
        .collect::<Vec<(usize, f64)>>();
    indexed.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut ranks = vec![0.0; values.len()];
    let mut start = 0;
    while start < indexed.len() {
        let mut end = start + 1;
        while end < indexed.len() && indexed[end].1 == indexed[start].1 {
            end += 1;
        }
        let average_rank = (start + 1 + end) as f64 / 2.0;
        for index in start..end {
            ranks[indexed[index].0] = average_rank;
        }
        start = end;
    }
    Ok(ranks)
}

/// Computes Spearman rank correlation using average ranks for ties.
pub fn spearman_correlation(left: &[f64], right: &[f64]) -> Result<f64> {
    validate_pair(left, right)?;
    correlation(&rank_values(left)?, &rank_values(right)?)
}

/// Fits `y = intercept + slope * x` for paired finite scalar observations.
pub fn simple_linear_regression(x: &[f64], y: &[f64]) -> Result<LinearRegressionSummary> {
    validate_pair(x, y)?;
    let x_mean = mean(x)?;
    let y_mean = mean(y)?;
    let ss_xx = x.iter().map(|value| (value - x_mean).powi(2)).sum::<f64>();
    if ss_xx <= f64::EPSILON {
        return Err(invalid_argument(
            "linear regression requires non-zero x variance",
        ));
    }
    let ss_xy = x
        .iter()
        .zip(y)
        .map(|(x_value, y_value)| (x_value - x_mean) * (y_value - y_mean))
        .sum::<f64>();
    let slope = ss_xy / ss_xx;
    let intercept = y_mean - slope * x_mean;
    let fitted = x
        .iter()
        .map(|value| intercept + slope * value)
        .collect::<Vec<_>>();
    let ss_res = y
        .iter()
        .zip(&fitted)
        .map(|(actual, fitted)| (actual - fitted).powi(2))
        .sum::<f64>();
    let ss_tot = y.iter().map(|value| (value - y_mean).powi(2)).sum::<f64>();
    let r_squared = if ss_tot <= f64::EPSILON {
        if ss_res <= f64::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        1.0 - ss_res / ss_tot
    };
    let residual_std_error = if x.len() > 2 {
        (ss_res / (x.len() - 2) as f64).sqrt()
    } else {
        0.0
    };
    Ok(LinearRegressionSummary {
        slope,
        intercept,
        r_squared,
        residual_std_error,
        observations: x.len(),
    })
}

/// Fits ordinary least squares using QR, falling back to normal equations.
pub fn ordinary_least_squares(design: &F32MatrixView<'_>, target: &[f32]) -> Result<OlsRegression> {
    design.validate()?;
    if target.len() != design.shape().rows {
        return Err(invalid_argument("OLS target length must match design rows"));
    }
    if target.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument("OLS target values must be finite"));
    }
    let coefficients = match design.qr_decompose() {
        Ok(qr) => solve_qr(&qr.q, &qr.r, target)?,
        Err(_) => {
            let design_f64 = F64Matrix::try_from(&design.into_owned()?)?;
            let target_f64 = target.iter().map(|value| *value as f64).collect::<Vec<_>>();
            let coefficients_f64 = design_f64
                .pseudoinverse(PseudoinverseOptions::default())?
                .matvec(&target_f64)?;
            let mut coefficients = Vec::with_capacity(coefficients_f64.len());
            for value in coefficients_f64 {
                if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
                    return Err(invalid_argument(
                        "OLS pseudoinverse produced f32-out-of-range coefficients",
                    ));
                }
                coefficients.push(value as f32);
            }
            coefficients
        }
    };
    let fitted = design.matvec(&coefficients)?.into_values();
    let residuals = target
        .iter()
        .zip(&fitted)
        .map(|(actual, fitted)| actual - fitted)
        .collect::<Vec<_>>();
    let mean_target = target.iter().sum::<f32>() / target.len() as f32;
    let ss_res = residuals.iter().map(|value| value * value).sum::<f32>();
    let ss_tot = target
        .iter()
        .map(|value| (value - mean_target).powi(2))
        .sum::<f32>();
    let r_squared = if ss_tot <= f32::EPSILON {
        if ss_res <= f32::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        1.0 - ss_res / ss_tot
    };
    Ok(OlsRegression {
        coefficients,
        fitted,
        residuals,
        r_squared,
        observations: design.shape().rows,
        predictors: design.shape().cols,
    })
}

/// Fits full-column-rank OLS and returns residual and coefficient diagnostics.
pub fn ordinary_least_squares_diagnostics(
    design: &F32MatrixView<'_>,
    target: &[f32],
    tolerance: f32,
) -> Result<OlsRegressionDiagnostics> {
    let solution = design.least_squares(target, tolerance)?;
    let residual_sum_squares = solution.residual_sum_squares;
    let observations = solution.observations;
    let predictors = solution.predictors;
    let mean_target = target.iter().sum::<f32>() / target.len() as f32;
    let total_sum_squares = target
        .iter()
        .map(|value| (value - mean_target).powi(2))
        .sum::<f32>();
    let r_squared = r_squared_from_sums(residual_sum_squares, total_sum_squares);
    let regression = OlsRegression {
        coefficients: solution.coefficients,
        fitted: solution.fitted,
        residuals: solution.residuals,
        r_squared,
        observations,
        predictors,
    };
    let effective_residual_sum_squares = if residual_sum_squares.abs() <= 1.0e-10 {
        0.0
    } else {
        residual_sum_squares
    };
    let degrees_of_freedom = regression
        .observations
        .saturating_sub(regression.predictors);
    let mean_squared_error = if degrees_of_freedom > 0 {
        effective_residual_sum_squares / degrees_of_freedom as f32
    } else {
        0.0
    };
    let root_mean_squared_error = mean_squared_error.sqrt();
    let adjusted_r_squared = if regression.observations > regression.predictors + 1 {
        1.0 - (1.0 - r_squared) * (regression.observations - 1) as f32
            / (regression.observations - regression.predictors - 1) as f32
    } else {
        r_squared
    };
    let (standard_errors, t_statistics) = if degrees_of_freedom == 0 {
        (
            vec![None; regression.predictors],
            vec![None; regression.predictors],
        )
    } else {
        regression_standard_errors(design, mean_squared_error, regression.predictors)?
            .map(|standard_errors| {
                let t_statistics = regression
                    .coefficients
                    .iter()
                    .zip(&standard_errors)
                    .map(|(coefficient, standard_error)| {
                        standard_error.and_then(|se| {
                            if se > 0.0 {
                                Some(coefficient / se)
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                (standard_errors, t_statistics)
            })
            .unwrap_or_else(|| {
                (
                    vec![None; regression.predictors],
                    vec![None; regression.predictors],
                )
            })
    };
    Ok(OlsRegressionDiagnostics {
        regression,
        residual_sum_squares: effective_residual_sum_squares,
        total_sum_squares,
        mean_squared_error,
        root_mean_squared_error,
        adjusted_r_squared,
        degrees_of_freedom,
        standard_errors,
        t_statistics,
    })
}

/// Fits deterministic ridge regression by solving `(X^T X + lambda I) beta = X^T y`.
pub fn ridge_regression(
    design: &F32MatrixView<'_>,
    target: &[f32],
    lambda: f32,
) -> Result<RidgeRegression> {
    design.validate()?;
    if target.len() != design.shape().rows {
        return Err(invalid_argument(
            "ridge regression target length must match design rows",
        ));
    }
    if target.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument(
            "ridge regression target values must be finite",
        ));
    }
    if !lambda.is_finite() || lambda < 0.0 {
        return Err(invalid_argument(
            "ridge regression lambda must be finite and non-negative",
        ));
    }
    let xtx = design.gram_columns()?;
    let mut values = xtx.values().to_vec();
    let predictors = design.shape().cols;
    for index in 0..predictors {
        values[index * predictors + index] += lambda;
    }
    let regularized = F32Matrix::new(MatrixShape::new(predictors, predictors)?, values)?;
    let xty = design.transpose().matvec(target)?.into_values();
    let coefficients = regularized.solve_vector(&xty)?;
    let fitted = design.matvec(&coefficients)?.into_values();
    let residuals = target
        .iter()
        .zip(&fitted)
        .map(|(actual, fitted)| actual - fitted)
        .collect::<Vec<_>>();
    let residual_sum_squares = residuals.iter().map(|value| value * value).sum::<f32>();
    let mean_target = target.iter().sum::<f32>() / target.len() as f32;
    let total_sum_squares = target
        .iter()
        .map(|value| (value - mean_target).powi(2))
        .sum::<f32>();
    Ok(RidgeRegression {
        coefficients,
        fitted,
        residuals,
        r_squared: r_squared_from_sums(residual_sum_squares, total_sum_squares),
        lambda,
        observations: design.shape().rows,
        predictors,
    })
}

/// Computes a correlation matrix treating rows as observations and columns as variables.
pub fn correlation_matrix_from_rows(matrix: &F32MatrixView<'_>) -> Result<F32Matrix> {
    matrix.validate()?;
    let cols = matrix.shape().cols;
    let columns = matrix_columns_as_f64(matrix)?;
    let mut values = vec![0.0; cols * cols];
    for row in 0..cols {
        for col in 0..cols {
            values[row * cols + col] = correlation(&columns[row], &columns[col])? as f32;
        }
    }
    F32Matrix::new(MatrixShape::new(cols, cols)?, values)
}

/// Computes a covariance matrix treating rows as observations and columns as variables.
pub fn covariance_matrix_from_rows(
    matrix: &F32MatrixView<'_>,
    mode: VarianceMode,
) -> Result<F32Matrix> {
    matrix.validate()?;
    let cols = matrix.shape().cols;
    let columns = matrix_columns_as_f64(matrix)?;
    let mut values = vec![0.0; cols * cols];
    for row in 0..cols {
        for col in 0..cols {
            values[row * cols + col] = covariance(&columns[row], &columns[col], mode)? as f32;
        }
    }
    F32Matrix::new(MatrixShape::new(cols, cols)?, values)
}

#[derive(Debug, Clone, PartialEq)]
/// One finite multivariate observation with a positive statistical weight.
pub struct WeightedObservation {
    /// Observation coordinates or feature values.
    pub values: Vec<f64>,
    /// Positive finite contribution to weighted statistics.
    pub weight: f64,
}

impl WeightedObservation {
    /// Creates an observation with weight `1.0`.
    pub fn new(values: impl Into<Vec<f64>>) -> Result<Self> {
        Self::weighted(values, 1.0)
    }

    /// Creates an observation with an explicit positive finite weight.
    pub fn weighted(values: impl Into<Vec<f64>>, weight: f64) -> Result<Self> {
        let observation = Self {
            values: values.into(),
            weight,
        };
        observation.validate()?;
        Ok(observation)
    }

    /// Verifies non-empty finite values and a positive finite weight.
    pub fn validate(&self) -> Result<()> {
        if self.values.is_empty() {
            return Err(invalid_argument(
                "weighted observation values must not be empty",
            ));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "weighted observation values must be finite",
            ));
        }
        if !self.weight.is_finite() || self.weight <= 0.0 {
            return Err(invalid_argument(
                "weighted observation weight must be finite and positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Population covariance result with means and weighted sample metadata.
pub struct CovarianceMatrix {
    /// Dense square covariance matrix in row-major order.
    pub matrix: F32Matrix,
    /// Per-dimension weighted means used to center the observations.
    pub means: Vec<f64>,
    /// Number of items represented by this value.
    pub count: u64,
    /// Sum of all observation weights.
    pub weight_sum: f64,
}

impl CovarianceMatrix {
    /// Converts covariance values to correlations using diagonal variances.
    pub fn correlation_matrix(&self) -> Result<F32Matrix> {
        let dims = self.matrix.shape().rows;
        let mut values = vec![0.0; dims * dims];
        for row in 0..dims {
            let var_row = self.matrix.values()[row * dims + row].max(0.0).sqrt();
            for col in 0..dims {
                let var_col = self.matrix.values()[col * dims + col].max(0.0).sqrt();
                let denom = (var_row * var_col).max(f32::EPSILON);
                values[row * dims + col] = self.matrix.values()[row * dims + col] / denom;
            }
        }
        F32Matrix::new(MatrixShape::new(dims, dims)?, values)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Streaming weighted population covariance accumulator.
pub struct RunningCovariance {
    dimensions: usize,
    count: u64,
    weight_sum: f64,
    mean: Vec<f64>,
    m2: Vec<f64>,
}

impl RunningCovariance {
    /// Creates an empty accumulator for fixed-width observations.
    pub fn new(dimensions: usize) -> Result<Self> {
        if dimensions == 0 {
            return Err(invalid_argument(
                "covariance dimensions must be greater than zero",
            ));
        }
        Ok(Self {
            dimensions,
            count: 0,
            weight_sum: 0.0,
            mean: vec![0.0; dimensions],
            m2: vec![0.0; dimensions * dimensions],
        })
    }

    /// Accumulates one unit-weight observation for each row in a matrix.
    pub fn from_matrix(matrix: &F32MatrixView<'_>) -> Result<Self> {
        let mut running = Self::new(matrix.shape().cols)?;
        for row in 0..matrix.shape().rows {
            let values = matrix
                .row(row)?
                .as_slice()
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>();
            running.push(WeightedObservation::new(values)?)?;
        }
        Ok(running)
    }

    /// Returns the required observation width.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Returns the number of observations pushed, independent of weights.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Adds one weighted observation using a numerically stable online update.
    pub fn push(&mut self, observation: WeightedObservation) -> Result<()> {
        observation.validate()?;
        if observation.values.len() != self.dimensions {
            return Err(invalid_argument(
                "covariance observation dimensions must match",
            ));
        }
        self.count += 1;

        let new_weight_sum = self.weight_sum + observation.weight;
        let delta = observation
            .values
            .iter()
            .zip(&self.mean)
            .map(|(value, mean)| value - mean)
            .collect::<Vec<_>>();
        let next_mean = self
            .mean
            .iter()
            .zip(&delta)
            .map(|(mean, delta)| mean + delta * observation.weight / new_weight_sum)
            .collect::<Vec<_>>();
        let delta2 = observation
            .values
            .iter()
            .zip(&next_mean)
            .map(|(value, mean)| value - mean)
            .collect::<Vec<_>>();

        for (row, delta_value) in delta.iter().enumerate().take(self.dimensions) {
            for (col, delta2_value) in delta2.iter().enumerate().take(self.dimensions) {
                self.m2[row * self.dimensions + col] +=
                    observation.weight * delta_value * delta2_value;
            }
        }
        self.weight_sum = new_weight_sum;
        self.mean = next_mean;
        Ok(())
    }

    /// Returns weighted population covariance and fails when no observations exist.
    pub fn covariance_matrix(&self) -> Result<CovarianceMatrix> {
        if self.weight_sum <= 0.0 {
            return Err(invalid_argument(
                "covariance requires at least one observation",
            ));
        }
        let values = self
            .m2
            .iter()
            .map(|value| (value / self.weight_sum) as f32)
            .collect::<Vec<_>>();
        Ok(CovarianceMatrix {
            matrix: F32Matrix::new(MatrixShape::new(self.dimensions, self.dimensions)?, values)?,
            means: self.mean.clone(),
            count: self.count,
            weight_sum: self.weight_sum,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Per-column z-score normalizer fitted from a dense matrix.
pub struct ZScoreNormalizer {
    means: Vec<f64>,
    std_devs: Vec<f64>,
}

impl ZScoreNormalizer {
    /// Fits per-column means and standard deviations from matrix rows.
    pub fn fit(matrix: &F32MatrixView<'_>) -> Result<Self> {
        let cols = matrix.shape().cols;
        let mut stats = (0..cols).map(|_| RunningStats::new()).collect::<Vec<_>>();
        for row in 0..matrix.shape().rows {
            let values = matrix.row(row)?.as_slice();
            for (stat, value) in stats.iter_mut().zip(values) {
                stat.push(value as f64);
            }
        }
        let means = stats
            .iter()
            .map(|stat| stat.summary().mean.unwrap_or(0.0))
            .collect::<Vec<_>>();
        let std_devs = stats
            .iter()
            .map(|stat| stat.summary().std_dev.unwrap_or(1.0).max(f64::EPSILON))
            .collect::<Vec<_>>();
        Ok(Self { means, std_devs })
    }

    /// Borrows the fitted per-column means.
    pub fn means(&self) -> &[f64] {
        &self.means
    }

    /// Borrows the fitted per-column standard deviations.
    pub fn std_devs(&self) -> &[f64] {
        &self.std_devs
    }

    /// Applies `(value - mean) / std_dev` to each matrix column.
    pub fn transform_matrix(&self, matrix: &F32MatrixView<'_>) -> Result<F32Matrix> {
        if matrix.shape().cols != self.means.len() {
            return Err(invalid_argument(
                "z-score normalizer dimensions must match matrix",
            ));
        }
        let mut values = Vec::with_capacity(matrix.values().len());
        for row in 0..matrix.shape().rows {
            let row_values = matrix.row(row)?.as_slice();
            for (index, value) in row_values.into_iter().enumerate() {
                values.push(((value as f64 - self.means[index]) / self.std_devs[index]) as f32);
            }
        }
        F32Matrix::new(matrix.shape(), values)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Per-column min/max normalizer fitted from a dense matrix.
pub struct MinMaxNormalizer {
    ranges: Vec<NumberRange>,
}

impl MinMaxNormalizer {
    /// Fits per-column finite minimum and maximum ranges.
    pub fn fit(matrix: &F32MatrixView<'_>) -> Result<Self> {
        let cols = matrix.shape().cols;
        let mut mins = vec![f64::INFINITY; cols];
        let mut maxs = vec![f64::NEG_INFINITY; cols];
        for row in 0..matrix.shape().rows {
            let row_values = matrix.row(row)?.as_slice();
            for (index, value) in row_values.into_iter().enumerate() {
                mins[index] = mins[index].min(value as f64);
                maxs[index] = maxs[index].max(value as f64);
            }
        }
        let mut ranges = Vec::with_capacity(cols);
        for index in 0..cols {
            ranges.push(NumberRange::new(mins[index], maxs[index])?);
        }
        Ok(Self { ranges })
    }

    /// Borrows fitted per-column ranges.
    pub fn ranges(&self) -> &[NumberRange] {
        &self.ranges
    }

    /// Maps each column into the unit interval using its fitted range.
    pub fn transform_matrix(&self, matrix: &F32MatrixView<'_>) -> Result<F32Matrix> {
        if matrix.shape().cols != self.ranges.len() {
            return Err(invalid_argument(
                "min-max normalizer dimensions must match matrix",
            ));
        }
        let mut values = Vec::with_capacity(matrix.values().len());
        for row in 0..matrix.shape().rows {
            let row_values = matrix.row(row)?.as_slice();
            for (index, value) in row_values.into_iter().enumerate() {
                values.push(self.ranges[index].normalize(value as f64)? as f32);
            }
        }
        F32Matrix::new(matrix.shape(), values)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Deterministic PCA-lite model for small and medium dense matrices.
pub struct PrincipalComponents {
    mean: Vec<f32>,
    components: F32Matrix,
    explained_variance: Vec<f32>,
}

impl PrincipalComponents {
    /// Fits principal components from the weighted population covariance matrix.
    pub fn fit(matrix: &F32MatrixView<'_>, component_count: usize) -> Result<Self> {
        if component_count == 0 {
            return Err(invalid_argument(
                "component_count must be greater than zero",
            ));
        }
        if component_count > matrix.shape().cols {
            return Err(invalid_argument(
                "component_count must not exceed matrix column count",
            ));
        }
        let covariance = RunningCovariance::from_matrix(matrix)?.covariance_matrix()?;
        let mut working = covariance.matrix.values().to_vec();
        let dims = matrix.shape().cols;
        let mut components = Vec::with_capacity(component_count * dims);
        let mut explained_variance = Vec::with_capacity(component_count);

        for _ in 0..component_count {
            let (eigenvalue, eigenvector) = power_iteration(&working, dims)?;
            explained_variance.push(eigenvalue.max(0.0));
            components.extend(eigenvector.iter().copied());
            for row in 0..dims {
                for col in 0..dims {
                    working[row * dims + col] -= eigenvalue * eigenvector[row] * eigenvector[col];
                }
            }
        }

        Ok(Self {
            mean: covariance.means.iter().map(|value| *value as f32).collect(),
            components: F32Matrix::new(MatrixShape::new(component_count, dims)?, components)?,
            explained_variance,
        })
    }

    /// Borrows component vectors as rows in descending extraction order.
    pub fn components(&self) -> &F32Matrix {
        &self.components
    }

    /// Borrows the per-column mean used to center inputs.
    pub fn mean(&self) -> &[f32] {
        &self.mean
    }

    /// Borrows the non-negative variance captured by each component.
    pub fn explained_variance(&self) -> &[f32] {
        &self.explained_variance
    }

    /// Centers the input matrix and projects rows onto the fitted components.
    pub fn transform(&self, matrix: &F32MatrixView<'_>) -> Result<F32Matrix> {
        if matrix.shape().cols != self.mean.len() {
            return Err(invalid_argument(
                "PCA transform dimensions must match input columns",
            ));
        }
        let mut centered = Vec::with_capacity(matrix.values().len());
        for row in 0..matrix.shape().rows {
            let row_values = matrix.row(row)?.as_slice();
            for (index, value) in row_values.into_iter().enumerate() {
                centered.push(value - self.mean[index]);
            }
        }
        let centered = F32Matrix::new(matrix.shape(), centered)?;
        centered.matmul(&self.components.as_view().transpose())
    }
}

fn power_iteration(values: &[f32], dims: usize) -> Result<(f32, Vec<f32>)> {
    let mut vector = vec![0.0; dims];
    vector[0] = 1.0;
    for _ in 0..32 {
        let mut next = vec![0.0; dims];
        for row in 0..dims {
            for col in 0..dims {
                next[row] += values[row * dims + col] * vector[col];
            }
        }
        let norm = next.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm <= f32::EPSILON {
            return Err(invalid_argument(
                "PCA power iteration encountered a zero norm vector",
            ));
        }
        for value in &mut next {
            *value /= norm;
        }
        vector = next;
    }
    let mut av = vec![0.0; dims];
    for row in 0..dims {
        for col in 0..dims {
            av[row] += values[row * dims + col] * vector[col];
        }
    }
    let eigenvalue = vector
        .iter()
        .zip(&av)
        .map(|(left, right)| left * right)
        .sum();
    Ok((eigenvalue, vector))
}

fn solve_qr(q: &F32Matrix, r: &F32Matrix, target: &[f32]) -> Result<Vec<f32>> {
    let q_t_y = q.as_view().transpose().matvec(target)?.into_values();
    let cols = r.shape().cols;
    let mut coefficients = vec![0.0; cols];
    for row in (0..cols).rev() {
        let mut sum = q_t_y[row];
        for (col, coefficient) in coefficients.iter().enumerate().take(cols).skip(row + 1) {
            sum -= r.as_view().get(row, col)? * coefficient;
        }
        let pivot = r.as_view().get(row, row)?;
        if pivot.abs() <= f32::EPSILON {
            return Err(invalid_argument("OLS QR solve encountered a zero pivot"));
        }
        coefficients[row] = sum / pivot;
    }
    Ok(coefficients)
}

fn regression_standard_errors(
    design: &F32MatrixView<'_>,
    mean_squared_error: f32,
    predictors: usize,
) -> Result<Option<Vec<Option<f32>>>> {
    if mean_squared_error <= 0.0 || !mean_squared_error.is_finite() {
        return Ok(Some(vec![None; predictors]));
    }
    let inverse = match design.gram_columns()?.inverse() {
        Ok(inverse) => inverse,
        Err(_) => return Ok(None),
    };
    let standard_errors = (0..predictors)
        .map(|index| {
            let variance = inverse.values()[index * predictors + index] * mean_squared_error;
            if variance.is_finite() && variance > 0.0 {
                Some(variance.sqrt())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    Ok(Some(standard_errors))
}

fn matrix_columns_as_f64(matrix: &F32MatrixView<'_>) -> Result<Vec<Vec<f64>>> {
    (0..matrix.shape().cols)
        .map(|col| {
            Ok(matrix
                .column(col)?
                .as_slice()
                .into_iter()
                .map(|value| value as f64)
                .collect::<Vec<_>>())
        })
        .collect()
}

fn r_squared_from_sums(residual_sum_squares: f32, total_sum_squares: f32) -> f32 {
    if total_sum_squares <= f32::EPSILON {
        if residual_sum_squares <= f32::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        1.0 - residual_sum_squares / total_sum_squares
    }
}

fn higher_moments(values: &[f64], mean: f64, std_dev: f64) -> (Option<f64>, Option<f64>) {
    if std_dev <= f64::EPSILON {
        return (None, None);
    }
    let n = values.len() as f64;
    let skewness = values
        .iter()
        .map(|value| ((value - mean) / std_dev).powi(3))
        .sum::<f64>()
        / n;
    let excess_kurtosis = values
        .iter()
        .map(|value| ((value - mean) / std_dev).powi(4))
        .sum::<f64>()
        / n
        - 3.0;
    (Some(skewness), Some(excess_kurtosis))
}

fn rolling_apply(
    values: &[f64],
    window: usize,
    mut operation: impl FnMut(&[f64]) -> Result<f64>,
) -> Result<Vec<f64>> {
    validate_series(values, "series")?;
    validate_window(window, values.len())?;
    let mut results = Vec::with_capacity(values.len() + 1 - window);
    for start in 0..=(values.len() - window) {
        results.push(operation(&values[start..start + window])?);
    }
    Ok(results)
}

fn validate_series(values: &[f64], name: &str) -> Result<()> {
    if values.is_empty() {
        return Err(invalid_argument(format!("{name} must not be empty")));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument(format!("{name} values must be finite")));
    }
    Ok(())
}

fn validate_pair(left: &[f64], right: &[f64]) -> Result<()> {
    validate_series(left, "left series")?;
    validate_series(right, "right series")?;
    if left.len() != right.len() {
        return Err(invalid_argument("series lengths must match"));
    }
    Ok(())
}

fn validate_change_input(values: &[f64], method: ChangeMethod) -> Result<()> {
    if values.len() < 2 {
        return Err(invalid_argument(
            "change series must contain at least two values",
        ));
    }
    validate_series(values, "change series")?;
    if matches!(method, ChangeMethod::Relative | ChangeMethod::LogRatio)
        && values.iter().any(|value| *value <= 0.0)
    {
        return Err(invalid_argument(
            "relative and log-ratio changes require positive values",
        ));
    }
    Ok(())
}

fn validate_compoundable(values: &[f64]) -> Result<()> {
    validate_series(values, "compound series")?;
    if values.iter().any(|value| *value < -1.0) {
        return Err(invalid_argument(
            "compound values must be greater than or equal to -1",
        ));
    }
    Ok(())
}

fn validate_window(window: usize, len: usize) -> Result<()> {
    if window == 0 {
        return Err(invalid_argument("rolling window must be greater than zero"));
    }
    if window > len {
        return Err(invalid_argument(
            "rolling window must not exceed series length",
        ));
    }
    Ok(())
}

fn variance_denominator(len: usize, mode: VarianceMode) -> Result<f64> {
    match mode {
        VarianceMode::Population => Ok(len as f64),
        VarianceMode::Sample => {
            if len < 2 {
                return Err(invalid_argument(
                    "sample variance requires at least two observations",
                ));
            }
            Ok((len - 1) as f64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1.0e-10,
            "expected {left} to be close to {right}"
        );
    }

    #[test]
    fn scalar_series_reports_sample_and_population_moments() {
        let values = [1.0, 2.0, 3.0, 4.0];
        assert_close(mean(&values).unwrap(), 2.5);
        assert_close(variance(&values, VarianceMode::Population).unwrap(), 1.25);
        assert_close(
            variance(&values, VarianceMode::Sample).unwrap(),
            1.6666666666666667,
        );
        let summary = summarize_series(&values, VarianceMode::Sample).unwrap();
        assert_eq!(summary.count, 4);
        assert_close(summary.median, 2.5);
        assert!(summary.skewness.unwrap().abs() < 1.0e-10);
    }

    #[test]
    fn scalar_series_rejects_invalid_inputs() {
        assert!(mean(&[]).is_err());
        assert!(mean(&[1.0, f64::NAN]).is_err());
        assert!(std_dev(&[1.0], VarianceMode::Sample).is_err());
        assert!(correlation(&[1.0, 1.0], &[2.0, 3.0]).is_err());
        assert!(covariance(&[1.0, 2.0], &[1.0], VarianceMode::Sample).is_err());
    }

    #[test]
    fn changes_support_difference_relative_and_log_ratio() {
        let values = [100.0, 110.0, 99.0];
        assert_eq!(
            changes(&values, ChangeMethod::Difference).unwrap(),
            vec![10.0, -11.0]
        );
        let relative = changes(&values, ChangeMethod::Relative).unwrap();
        assert_close(relative[0], 0.1);
        assert_close(relative[1], -0.1);
        let logs = changes(&values, ChangeMethod::LogRatio).unwrap();
        assert_close(logs.iter().sum::<f64>(), (99.0_f64 / 100.0).ln());
        assert!(changes(&[1.0], ChangeMethod::Difference).is_err());
        assert!(changes(&[1.0, 0.0], ChangeMethod::Relative).is_err());
        assert!(changes(&[1.0, -1.0], ChangeMethod::LogRatio).is_err());
    }

    #[test]
    fn computes_pairwise_tail_drawdown_rolling_and_z_scores() {
        let returns = [0.1, -0.2, 0.05, 0.3, -0.1];
        let pair = summarize_pair(&returns, &returns, VarianceMode::Sample).unwrap();
        assert_close(pair.correlation, 1.0);
        assert_eq!(pair.observations, 5);

        let risk = tail_risk(&returns, 0.8).unwrap();
        assert_eq!(risk.observations, 5);
        assert!(risk.value_at_risk >= 0.0);
        assert!(risk.conditional_value_at_risk >= risk.value_at_risk);

        let drawdown = max_drawdown(&returns).unwrap();
        assert_close(drawdown.depth, 0.2);
        assert_eq!(drawdown.peak_index, 1);
        assert_eq!(drawdown.trough_index, 2);
        assert_eq!(drawdown.recovery_index, Some(4));

        assert_eq!(rolling_mean(&returns, 3).unwrap().len(), 3);
        assert_eq!(
            rolling_std_dev(&returns, 3, VarianceMode::Sample)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(rolling_min_max(&returns, 3).unwrap().len(), 3);
        assert!(rolling_mean(&returns, 0).is_err());
        assert!(rolling_mean(&returns, 6).is_err());
        for value in rolling_correlation(&returns, &returns, 3).unwrap() {
            assert_close(value, 1.0);
        }

        let scores = z_scores(&[1.0, 2.0, 3.0], VarianceMode::Population).unwrap();
        assert_close(mean(&scores).unwrap(), 0.0);
        assert_close(std_dev(&scores, VarianceMode::Population).unwrap(), 1.0);
    }

    #[test]
    fn streaming_covariance_matches_batch_covariance() {
        let matrix = F32Matrix::from_rows([[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]).unwrap();
        let streaming = RunningCovariance::from_matrix(&matrix.as_view())
            .unwrap()
            .covariance_matrix()
            .unwrap();
        assert_eq!(streaming.matrix.shape().rows, 2);
        assert!(streaming.matrix.values()[0] > 0.0);
    }

    #[test]
    fn normalizers_produce_expected_ranges() {
        let matrix = F32Matrix::from_rows([[0.0, 1.0], [2.0, 3.0]]).unwrap();
        let z = ZScoreNormalizer::fit(&matrix.as_view()).unwrap();
        let min_max = MinMaxNormalizer::fit(&matrix.as_view()).unwrap();
        let normalized = min_max.transform_matrix(&matrix.as_view()).unwrap();
        assert_eq!(normalized.values(), &[0.0, 0.0, 1.0, 1.0]);
        assert_eq!(z.means(), &[1.0, 2.0]);
    }

    #[test]
    fn pca_reports_components_and_variance() {
        let matrix = F32Matrix::from_rows([[1.0, 1.0], [2.0, 2.0], [3.0, 3.0]]).unwrap();
        let pca = PrincipalComponents::fit(&matrix.as_view(), 1).unwrap();
        assert_eq!(pca.components().shape().rows, 1);
        assert_eq!(pca.explained_variance().len(), 1);
        assert!(pca.explained_variance()[0] >= 0.0);
    }

    #[test]
    fn ranks_and_spearman_handle_ties() {
        assert_eq!(
            rank_values(&[3.0, 1.0, 1.0, 2.0]).unwrap(),
            vec![4.0, 1.5, 1.5, 3.0]
        );
        assert_close(
            spearman_correlation(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap(),
            1.0,
        );
    }

    #[test]
    fn simple_regression_fits_line() {
        let summary = simple_linear_regression(&[1.0, 2.0, 3.0], &[3.0, 5.0, 7.0]).unwrap();
        assert_close(summary.slope, 2.0);
        assert_close(summary.intercept, 1.0);
        assert_close(summary.r_squared, 1.0);
        assert_eq!(summary.observations, 3);
        assert!(simple_linear_regression(&[1.0, 1.0], &[2.0, 3.0]).is_err());
    }

    #[test]
    fn ordinary_least_squares_fits_dense_design() {
        let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0]]).unwrap();
        let regression = ordinary_least_squares(&design.as_view(), &[3.0, 5.0, 7.0]).unwrap();
        assert!((regression.coefficients[0] - 1.0).abs() < 1.0e-4);
        assert!((regression.coefficients[1] - 2.0).abs() < 1.0e-4);
        assert!((regression.r_squared - 1.0).abs() < 1.0e-4);
        assert_eq!(regression.observations, 3);
        assert_eq!(regression.predictors, 2);
    }

    #[test]
    fn ols_diagnostics_report_coefficients_and_standard_errors() {
        let design =
            F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0], [1.0, 4.0], [1.0, 5.0]])
                .unwrap();
        let diagnostics =
            ordinary_least_squares_diagnostics(&design.as_view(), &[3.1, 4.9, 7.2, 8.8, 11.1], 0.0)
                .unwrap();

        assert!((diagnostics.regression.coefficients[0] - 1.0).abs() < 0.25);
        assert!((diagnostics.regression.coefficients[1] - 2.0).abs() < 0.1);
        assert!(diagnostics.root_mean_squared_error < 0.25);
        assert_eq!(diagnostics.degrees_of_freedom, 3);
        assert!(diagnostics.standard_errors.iter().all(Option::is_some));
        assert!(diagnostics.t_statistics.iter().all(Option::is_some));
    }

    #[test]
    fn ols_diagnostics_handle_exact_fit() {
        let design =
            F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0], [1.0, 4.0]]).unwrap();
        let diagnostics =
            ordinary_least_squares_diagnostics(&design.as_view(), &[3.0, 5.0, 7.0, 9.0], 0.0)
                .unwrap();

        assert!(diagnostics.residual_sum_squares < 1.0e-8);
        assert!(diagnostics.root_mean_squared_error < 1.0e-4);
        assert_eq!(diagnostics.standard_errors, vec![None, None]);
    }

    #[test]
    fn diagnostics_reject_invalid_inputs() {
        let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0]]).unwrap();

        assert!(ordinary_least_squares_diagnostics(&design.as_view(), &[1.0, 2.0], 0.0).is_err());
        assert!(
            ordinary_least_squares_diagnostics(&design.as_view(), &[1.0, f32::NAN, 3.0], 0.0)
                .is_err()
        );
    }

    #[test]
    fn ridge_regression_handles_rank_deficient_design() {
        let design =
            F32Matrix::from_rows([[1.0, 2.0], [2.0, 4.0], [3.0, 6.0], [4.0, 8.0]]).unwrap();
        let regression = ridge_regression(&design.as_view(), &[1.0, 2.0, 3.0, 4.0], 1.0).unwrap();

        assert_eq!(regression.coefficients.len(), 2);
        assert_eq!(regression.fitted.len(), 4);
        assert!(regression.r_squared.is_finite());
    }

    #[test]
    fn ridge_rejects_invalid_lambda() {
        let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0]]).unwrap();

        assert!(ridge_regression(&design.as_view(), &[3.0, 5.0, 7.0], -1.0).is_err());
        assert!(ridge_regression(&design.as_view(), &[3.0, 5.0, 7.0], f32::NAN).is_err());
    }

    #[test]
    fn row_correlation_and_covariance_matrices_have_expected_shape() {
        let matrix = F32Matrix::from_rows([[1.0, 2.0], [2.0, 4.0], [3.0, 6.0]]).unwrap();
        let correlation = correlation_matrix_from_rows(&matrix.as_view()).unwrap();
        let covariance =
            covariance_matrix_from_rows(&matrix.as_view(), VarianceMode::Sample).unwrap();

        assert_eq!(correlation.shape().rows, 2);
        assert_eq!(correlation.shape().cols, 2);
        assert!((correlation.as_view().get(0, 0).unwrap() - 1.0).abs() < 1.0e-6);
        assert!((correlation.as_view().get(1, 1).unwrap() - 1.0).abs() < 1.0e-6);
        assert!(
            (correlation.as_view().get(0, 1).unwrap() - correlation.as_view().get(1, 0).unwrap())
                .abs()
                < 1.0e-6
        );
        assert_eq!(covariance.shape().rows, 2);
        assert_eq!(covariance.shape().cols, 2);
    }
}
