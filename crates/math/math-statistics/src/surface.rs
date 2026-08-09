//! Library-owned runtime surface for `math-statistics`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, PackageSurface, RuntimeCapabilities, SurfaceError, SurfaceRequest,
    SurfaceResponse,
};
use serde::Deserialize;

use math_linear::{F64Matrix, MatrixShape, PseudoinverseOptions, SvdOptions};

use crate::{
    changes, rank_values, rolling_correlation, rolling_mean, rolling_min_max, rolling_std_dev,
    simple_linear_regression, spearman_correlation, summarize_pair, summarize_series, tail_risk,
    z_scores, ChangeMethod, VarianceMode,
};

const MAX_VALUES: usize = 100_000;

/// Describes the statistics operations exposed by transport wrappers.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            surface_operation("describe", "Describe package", "Shared statistics for scalar series, paired series, dense matrix inputs, and streaming observations.", serde_json::json!({"includeOperations": true})),
            surface_operation("stats.series.describe", "Describe series", "Computes summary statistics for a finite scalar series.", serde_json::json!({"values": [1.0, 2.0, 3.0, 4.0], "varianceMode": "sample"})),
            surface_operation("stats.series.changes", "Series changes", "Converts adjacent values into differences, relative changes, or log-ratios.", serde_json::json!({"values": [100.0, 102.0, 99.0, 105.0], "method": "relative"})),
            surface_operation("stats.series.compare", "Compare series", "Computes pairwise covariance and correlation for two finite scalar series.", serde_json::json!({"left": [0.01, -0.02, 0.03], "right": [0.0, -0.01, 0.02], "varianceMode": "sample"})),
            surface_operation("stats.series.rolling", "Rolling statistics", "Computes rolling mean, standard deviation, ranges, and optional paired correlations.", serde_json::json!({"values": [0.1, -0.2, 0.05, 0.3], "window": 2, "varianceMode": "sample"})),
            surface_operation("stats.series.tailRisk", "Tail risk", "Computes empirical VaR/CVaR style tail-risk statistics.", serde_json::json!({"values": [0.02, -0.01, 0.03, -0.02, 0.01], "confidence": 0.8})),
            surface_operation("stats.series.zScores", "Z-scores", "Standardizes a finite scalar series into z-scores.", serde_json::json!({"values": [1.0, 2.0, 3.0], "varianceMode": "population"})),
            surface_operation("stats.normalize", "Normalize matrix", "Applies z-score or min-max column normalization to a finite matrix; matrix workflows default to f64.", serde_json::json!({"matrix": {"rows": 2, "cols": 2, "values": [0.0, 1.0, 2.0, 3.0]}, "method": "minMax", "precision": "f64"})),
            surface_operation("stats.covariance", "Covariance matrix", "Computes covariance and optional correlation for matrix rows as observations with f64 defaults.", serde_json::json!({"matrix": {"rows": 3, "cols": 2, "values": [1.0, 0.0, 0.0, 1.0, 1.0, 1.0]}, "correlation": true, "precision": "f64"})),
            surface_operation("stats.pca", "PCA", "Fits centered-data SVD PCA components and optionally transforms rows into component space.", serde_json::json!({"matrix": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]}, "componentCount": 1, "transform": true, "precision": "f64"})),
            surface_operation("stats.series.rankCorrelation", "Rank correlation", "Computes average ranks and Spearman correlation for finite paired scalar series.", serde_json::json!({"left": [1.0, 2.0, 2.0, 4.0], "right": [10.0, 20.0, 20.0, 40.0]})),
            surface_operation("stats.regression.linear", "Linear regression", "Fits a simple y = intercept + slope * x regression for paired finite scalar observations.", serde_json::json!({"x": [1.0, 2.0, 3.0], "y": [3.0, 5.0, 7.0]})),
            surface_operation("stats.regression.ols", "OLS regression", "Fits ordinary least squares from a finite dense design matrix and target vector, using SVD pseudoinverse for rank-deficient designs.", serde_json::json!({"design": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0]}, "target": [3.0, 5.0, 7.0], "precision": "f64"})),
            surface_operation("stats.regression.diagnostics", "Regression diagnostics", "Fits strict full-column-rank OLS and returns residual, adjusted R-squared, standard-error, and t-statistic diagnostics.", serde_json::json!({"design": {"rows": 4, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0, 1.0, 4.0]}, "target": [3.0, 5.0, 7.0, 9.0], "tolerance": 0.0, "precision": "f64"})),
        ],
    }
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "stats.series.describe" => series_describe_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.changes" => series_changes_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.compare" => series_compare_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.rolling" => series_rolling_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.tailRisk" => series_tail_risk_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.zScores" => series_z_scores_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.normalize" => normalize_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.covariance" => covariance_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.pca" => pca_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.series.rankCorrelation" => rank_correlation_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.regression.linear" => regression_linear_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.regression.ols" => regression_ols_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "stats.regression.diagnostics" => regression_diagnostics_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        operation => {
            return Err(
                SurfaceError::unsupported_operation(operation, env!("CARGO_PKG_NAME"))
                    .to_error_string(),
            )
        }
    };
    Ok(structured_operation_response(&surface, operation, value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatrixRequest {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeriesRequest {
    values: Vec<f64>,
    #[serde(default = "default_variance_mode")]
    variance_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangesRequest {
    values: Vec<f64>,
    #[serde(default = "default_change_method")]
    method: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompareRequest {
    left: Vec<f64>,
    right: Vec<f64>,
    #[serde(default = "default_variance_mode")]
    variance_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RollingRequest {
    values: Vec<f64>,
    window: usize,
    #[serde(default = "default_variance_mode")]
    variance_mode: String,
    #[serde(default)]
    right: Option<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TailRiskRequest {
    values: Vec<f64>,
    #[serde(default = "default_confidence")]
    confidence: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NormalizeRequest {
    matrix: MatrixRequest,
    method: String,
    #[serde(default = "default_f64_precision")]
    precision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CovarianceRequest {
    matrix: MatrixRequest,
    #[serde(default)]
    correlation: bool,
    #[serde(default = "default_f64_precision")]
    precision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PcaRequest {
    matrix: MatrixRequest,
    component_count: usize,
    #[serde(default)]
    transform: bool,
    #[serde(default = "default_f64_precision")]
    precision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RankCorrelationRequest {
    left: Vec<f64>,
    right: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinearRegressionRequest {
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OlsRequest {
    design: MatrixRequest,
    target: Vec<f64>,
    #[serde(default)]
    tolerance: f64,
    #[serde(default = "default_f64_precision")]
    precision: String,
}

fn series_describe_value(request: SeriesRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    let mode = parse_variance_mode(&request.variance_mode)?;
    let summary = summarize_series(&request.values, mode).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "count": summary.count,
        "mean": summary.mean,
        "variance": summary.variance,
        "stdDev": summary.std_dev,
        "min": summary.min,
        "max": summary.max,
        "q1": summary.q1,
        "median": summary.median,
        "q3": summary.q3,
        "iqr": summary.iqr,
        "skewness": summary.skewness,
        "excessKurtosis": summary.excess_kurtosis
    }))
}

fn series_changes_value(request: ChangesRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    let method = parse_change_method(&request.method)?;
    let values = changes(&request.values, method).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "method": request.method,
        "values": values
    }))
}

fn series_compare_value(request: CompareRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.left.len())?;
    validate_value_count(request.right.len())?;
    let mode = parse_variance_mode(&request.variance_mode)?;
    let summary =
        summarize_pair(&request.left, &request.right, mode).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "leftMean": summary.left_mean,
        "rightMean": summary.right_mean,
        "covariance": summary.covariance,
        "correlation": summary.correlation,
        "observations": summary.observations
    }))
}

fn series_rolling_value(request: RollingRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    if let Some(right) = &request.right {
        validate_value_count(right.len())?;
    }
    let mode = parse_variance_mode(&request.variance_mode)?;
    let mut value = serde_json::json!({
        "window": request.window,
        "mean": rolling_mean(&request.values, request.window).map_err(|error| error.to_string())?,
        "stdDev": rolling_std_dev(&request.values, request.window, mode).map_err(|error| error.to_string())?,
        "minMax": rolling_min_max(&request.values, request.window)
            .map_err(|error| error.to_string())?
            .iter()
            .map(|range| serde_json::json!({"min": range.min, "max": range.max}))
            .collect::<Vec<_>>()
    });
    if let Some(right) = request.right {
        value["correlation"] =
            serde_json::json!(rolling_correlation(&request.values, &right, request.window)
                .map_err(|error| error.to_string())?);
    }
    Ok(value)
}

fn series_tail_risk_value(request: TailRiskRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    let risk = tail_risk(&request.values, request.confidence).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "confidence": risk.confidence,
        "threshold": risk.threshold,
        "valueAtRisk": risk.value_at_risk,
        "conditionalValueAtRisk": risk.conditional_value_at_risk,
        "observations": risk.observations
    }))
}

fn series_z_scores_value(request: SeriesRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    let mode = parse_variance_mode(&request.variance_mode)?;
    let values = z_scores(&request.values, mode).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "values": values
    }))
}

fn normalize_value(request: NormalizeRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix, &request.precision)?;
    match request.method.as_str() {
        "zScore" => {
            let means = matrix
                .as_view()
                .column_means()
                .map_err(surface_invalid_request)?;
            let std_devs = column_std_devs(&matrix, &means)?;
            let normalized = normalize_z_score(&matrix, &means, &std_devs)?;
            Ok(serde_json::json!({
                "precision": request.precision,
                "method": "zScore",
                "rows": normalized.shape().rows,
                "cols": normalized.shape().cols,
                "values": normalized.values(),
                "means": means,
                "stdDevs": std_devs
            }))
        }
        "minMax" => {
            let ranges = column_ranges(&matrix)?;
            let normalized = normalize_min_max(&matrix, &ranges)?;
            Ok(serde_json::json!({
                "precision": request.precision,
                "method": "minMax",
                "rows": normalized.shape().rows,
                "cols": normalized.shape().cols,
                "values": normalized.values(),
                "ranges": ranges.iter().map(|(min, max)| serde_json::json!({"min": min, "max": max})).collect::<Vec<_>>()
            }))
        }
        method => Err(format!("unsupported normalization method `{method}`")),
    }
}

fn covariance_value(request: CovarianceRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix, &request.precision)?;
    let means = matrix
        .as_view()
        .column_means()
        .map_err(surface_invalid_request)?;
    let covariance = covariance_matrix_f64(&matrix, &means, VarianceMode::Population)?;
    let mut value = serde_json::json!({
        "precision": request.precision,
        "count": matrix.shape().rows,
        "weightSum": matrix.shape().rows as f64,
        "means": means,
        "covariance": matrix_projection(&covariance)
    });
    if request.correlation {
        value["correlation"] = matrix_projection(&correlation_matrix_f64(&covariance)?);
    }
    Ok(value)
}

fn pca_value(request: PcaRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix, &request.precision)?;
    let pca = pca_svd(&matrix, request.component_count)?;
    let mut value = serde_json::json!({
        "precision": request.precision,
        "mean": pca.mean(),
        "components": matrix_projection(pca.components()),
        "explainedVariance": pca.explained_variance(),
        "explainedVarianceRatio": pca.explained_variance_ratio(),
        "singularValues": pca.singular_values()
    });
    if request.transform {
        let transformed = pca.transform(&matrix)?;
        value["transformed"] = matrix_projection(&transformed);
    }
    Ok(value)
}

fn rank_correlation_value(request: RankCorrelationRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.left.len())?;
    validate_value_count(request.right.len())?;
    Ok(serde_json::json!({
        "leftRanks": rank_values(&request.left).map_err(|error| error.to_string())?,
        "rightRanks": rank_values(&request.right).map_err(|error| error.to_string())?,
        "spearmanCorrelation": spearman_correlation(&request.left, &request.right).map_err(|error| error.to_string())?,
        "observations": request.left.len()
    }))
}

fn regression_linear_value(request: LinearRegressionRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.x.len())?;
    validate_value_count(request.y.len())?;
    let summary =
        simple_linear_regression(&request.x, &request.y).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "slope": summary.slope,
        "intercept": summary.intercept,
        "rSquared": summary.r_squared,
        "residualStdError": summary.residual_std_error,
        "observations": summary.observations
    }))
}

fn regression_ols_value(request: OlsRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.target.len())?;
    let design = matrix_from_request(request.design, &request.precision)?;
    let regression = ordinary_least_squares_f64(&design, &request.target, request.tolerance)?;
    Ok(serde_json::json!({
        "precision": request.precision,
        "coefficients": regression.coefficients,
        "fitted": regression.fitted,
        "residuals": regression.residuals,
        "rSquared": regression.r_squared,
        "observations": regression.observations,
        "predictors": regression.predictors
    }))
}

fn regression_diagnostics_value(request: OlsRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.target.len())?;
    let design = matrix_from_request(request.design, &request.precision)?;
    let diagnostics =
        ordinary_least_squares_diagnostics_f64(&design, &request.target, request.tolerance)?;
    Ok(serde_json::json!({
        "precision": request.precision,
        "coefficients": diagnostics.regression.coefficients,
        "rSquared": diagnostics.regression.r_squared,
        "adjustedRSquared": diagnostics.adjusted_r_squared,
        "rootMeanSquaredError": diagnostics.root_mean_squared_error,
        "degreesOfFreedom": diagnostics.degrees_of_freedom,
        "standardErrors": diagnostics.standard_errors,
        "tStatistics": diagnostics.t_statistics
    }))
}

fn matrix_from_request(request: MatrixRequest, precision: &str) -> Result<F64Matrix, String> {
    validate_value_count(request.values.len())?;
    let shape = MatrixShape::new(request.rows, request.cols).map_err(surface_invalid_request)?;
    match precision {
        "f64" => F64Matrix::new(shape, request.values).map_err(surface_invalid_request),
        "f32" => {
            let mut promoted = Vec::with_capacity(request.values.len());
            for value in request.values {
                if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
                    return Err(
                        "invalid request: precision f32 requires finite in-range values"
                            .to_string(),
                    );
                }
                promoted.push(value as f32 as f64);
            }
            F64Matrix::new(shape, promoted).map_err(surface_invalid_request)
        }
        precision => Err(format!(
            "invalid request: unsupported precision `{precision}`; expected `f32` or `f64`"
        )),
    }
}

fn matrix_projection(matrix: &F64Matrix) -> serde_json::Value {
    let shape = matrix.shape();
    serde_json::json!({
        "rows": shape.rows,
        "cols": shape.cols,
        "values": matrix.values()
    })
}

#[derive(Debug, Clone, PartialEq)]
struct OlsRegressionF64 {
    coefficients: Vec<f64>,
    fitted: Vec<f64>,
    residuals: Vec<f64>,
    r_squared: f64,
    observations: usize,
    predictors: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct OlsRegressionDiagnosticsF64 {
    regression: OlsRegressionF64,
    root_mean_squared_error: f64,
    adjusted_r_squared: f64,
    degrees_of_freedom: usize,
    standard_errors: Vec<Option<f64>>,
    t_statistics: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
struct PcaSvdModel {
    mean: Vec<f64>,
    components: F64Matrix,
    explained_variance: Vec<f64>,
    explained_variance_ratio: Vec<f64>,
    singular_values: Vec<f64>,
}

impl PcaSvdModel {
    fn mean(&self) -> &[f64] {
        &self.mean
    }

    fn components(&self) -> &F64Matrix {
        &self.components
    }

    fn explained_variance(&self) -> &[f64] {
        &self.explained_variance
    }

    fn explained_variance_ratio(&self) -> &[f64] {
        &self.explained_variance_ratio
    }

    fn singular_values(&self) -> &[f64] {
        &self.singular_values
    }

    fn transform(&self, matrix: &F64Matrix) -> Result<F64Matrix, String> {
        if matrix.shape().cols != self.mean.len() {
            return Err(
                "invalid request: PCA transform dimensions must match input columns".to_string(),
            );
        }
        let mut centered = Vec::with_capacity(matrix.values().len());
        for row in 0..matrix.shape().rows {
            for (col, mean) in self.mean.iter().enumerate() {
                centered.push(
                    matrix
                        .as_view()
                        .get(row, col)
                        .map_err(surface_invalid_request)?
                        - mean,
                );
            }
        }
        let centered = F64Matrix::new(matrix.shape(), centered).map_err(surface_invalid_request)?;
        centered
            .matmul(&self.components.as_view().transpose())
            .map_err(surface_invalid_request)
    }
}

fn column_std_devs(matrix: &F64Matrix, means: &[f64]) -> Result<Vec<f64>, String> {
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let mut values = vec![0.0; cols];
    for row in 0..rows {
        for col in 0..cols {
            let centered = matrix
                .as_view()
                .get(row, col)
                .map_err(surface_invalid_request)?
                - means[col];
            values[col] += centered * centered;
        }
    }
    for value in &mut values {
        *value = (*value / rows as f64).sqrt().max(f64::EPSILON);
    }
    Ok(values)
}

fn normalize_z_score(
    matrix: &F64Matrix,
    means: &[f64],
    std_devs: &[f64],
) -> Result<F64Matrix, String> {
    let mut values = Vec::with_capacity(matrix.values().len());
    for row in 0..matrix.shape().rows {
        for col in 0..matrix.shape().cols {
            values.push(
                (matrix
                    .as_view()
                    .get(row, col)
                    .map_err(surface_invalid_request)?
                    - means[col])
                    / std_devs[col],
            );
        }
    }
    F64Matrix::new(matrix.shape(), values).map_err(surface_invalid_request)
}

fn column_ranges(matrix: &F64Matrix) -> Result<Vec<(f64, f64)>, String> {
    let cols = matrix.shape().cols;
    let mut ranges = vec![(f64::INFINITY, f64::NEG_INFINITY); cols];
    for row in 0..matrix.shape().rows {
        for (col, range) in ranges.iter_mut().enumerate() {
            let value = matrix
                .as_view()
                .get(row, col)
                .map_err(surface_invalid_request)?;
            range.0 = range.0.min(value);
            range.1 = range.1.max(value);
        }
    }
    Ok(ranges)
}

fn normalize_min_max(matrix: &F64Matrix, ranges: &[(f64, f64)]) -> Result<F64Matrix, String> {
    let mut values = Vec::with_capacity(matrix.values().len());
    for row in 0..matrix.shape().rows {
        for (col, (min, max)) in ranges.iter().enumerate() {
            let denom = (max - min).max(f64::EPSILON);
            values.push(
                (matrix
                    .as_view()
                    .get(row, col)
                    .map_err(surface_invalid_request)?
                    - min)
                    / denom,
            );
        }
    }
    F64Matrix::new(matrix.shape(), values).map_err(surface_invalid_request)
}

fn covariance_matrix_f64(
    matrix: &F64Matrix,
    means: &[f64],
    mode: VarianceMode,
) -> Result<F64Matrix, String> {
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let denom = match mode {
        VarianceMode::Population => rows as f64,
        VarianceMode::Sample if rows > 1 => (rows - 1) as f64,
        VarianceMode::Sample => {
            return Err("invalid request: sample covariance requires at least two rows".to_string())
        }
    };
    let mut values = vec![0.0; cols * cols];
    for row_var in 0..cols {
        for col_var in 0..cols {
            let mut sum = 0.0;
            for obs in 0..rows {
                sum += (matrix
                    .as_view()
                    .get(obs, row_var)
                    .map_err(surface_invalid_request)?
                    - means[row_var])
                    * (matrix
                        .as_view()
                        .get(obs, col_var)
                        .map_err(surface_invalid_request)?
                        - means[col_var]);
            }
            values[row_var * cols + col_var] = sum / denom;
        }
    }
    F64Matrix::new(
        MatrixShape::new(cols, cols).map_err(surface_invalid_request)?,
        values,
    )
    .map_err(surface_invalid_request)
}

fn correlation_matrix_f64(covariance: &F64Matrix) -> Result<F64Matrix, String> {
    let dims = covariance.shape().rows;
    let mut values = vec![0.0; dims * dims];
    for row in 0..dims {
        let row_std = covariance
            .as_view()
            .get(row, row)
            .map_err(surface_invalid_request)?
            .max(0.0)
            .sqrt();
        for col in 0..dims {
            let col_std = covariance
                .as_view()
                .get(col, col)
                .map_err(surface_invalid_request)?
                .max(0.0)
                .sqrt();
            let denom = (row_std * col_std).max(f64::EPSILON);
            values[row * dims + col] = covariance
                .as_view()
                .get(row, col)
                .map_err(surface_invalid_request)?
                / denom;
        }
    }
    F64Matrix::new(
        MatrixShape::new(dims, dims).map_err(surface_invalid_request)?,
        values,
    )
    .map_err(surface_invalid_request)
}

fn pca_svd(matrix: &F64Matrix, component_count: usize) -> Result<PcaSvdModel, String> {
    if component_count == 0 || component_count > matrix.shape().cols {
        return Err("invalid request: componentCount must be greater than zero and not exceed matrix column count".to_string());
    }
    let mean = matrix
        .as_view()
        .column_means()
        .map_err(surface_invalid_request)?;
    let centered = matrix.center_columns().map_err(surface_invalid_request)?;
    let svd = centered
        .svd(SvdOptions {
            compute_factors: true,
            ..SvdOptions::default()
        })
        .map_err(surface_invalid_request)?;
    let vt = svd
        .vt
        .as_ref()
        .ok_or_else(|| "invalid request: PCA SVD factors missing".to_string())?;
    let cols = matrix.shape().cols;
    let rows = matrix.shape().rows;
    let mut component_values = Vec::with_capacity(component_count * cols);
    for component in 0..component_count {
        for col in 0..cols {
            component_values.push(
                vt.as_view()
                    .get(component, col)
                    .map_err(surface_invalid_request)?,
            );
        }
    }
    let denom = rows.saturating_sub(1).max(1) as f64;
    let explained_variance = svd
        .singular_values
        .iter()
        .take(component_count)
        .map(|value| value * value / denom)
        .collect::<Vec<_>>();
    let total_variance = svd
        .singular_values
        .iter()
        .map(|value| value * value / denom)
        .sum::<f64>()
        .max(f64::EPSILON);
    let explained_variance_ratio = explained_variance
        .iter()
        .map(|value| value / total_variance)
        .collect::<Vec<_>>();
    Ok(PcaSvdModel {
        mean,
        components: F64Matrix::new(
            MatrixShape::new(component_count, cols).map_err(surface_invalid_request)?,
            component_values,
        )
        .map_err(surface_invalid_request)?,
        explained_variance,
        explained_variance_ratio,
        singular_values: svd
            .singular_values
            .into_iter()
            .take(component_count)
            .collect(),
    })
}

fn ordinary_least_squares_f64(
    design: &F64Matrix,
    target: &[f64],
    tolerance: f64,
) -> Result<OlsRegressionF64, String> {
    validate_ols_inputs(design, target, tolerance)?;
    let coefficients = design
        .pseudoinverse(PseudoinverseOptions {
            tolerance: (tolerance > 0.0).then_some(tolerance),
            ..PseudoinverseOptions::default()
        })
        .map_err(surface_invalid_request)?
        .matvec(target)
        .map_err(surface_invalid_request)?;
    ols_from_coefficients(design, target, coefficients)
}

fn ordinary_least_squares_diagnostics_f64(
    design: &F64Matrix,
    target: &[f64],
    tolerance: f64,
) -> Result<OlsRegressionDiagnosticsF64, String> {
    validate_ols_inputs(design, target, tolerance)?;
    let rank = design
        .numerical_rank((tolerance > 0.0).then_some(tolerance))
        .map_err(surface_invalid_request)?;
    if rank < design.shape().cols {
        return Err(
            "invalid request: regression diagnostics requires an identifiable full-rank design"
                .to_string(),
        );
    }
    let regression = ordinary_least_squares_f64(design, target, tolerance)?;
    let residual_sum_squares = regression
        .residuals
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    let degrees_of_freedom = regression
        .observations
        .saturating_sub(regression.predictors);
    let mean_squared_error = if degrees_of_freedom > 0 {
        residual_sum_squares / degrees_of_freedom as f64
    } else {
        0.0
    };
    let root_mean_squared_error = mean_squared_error.sqrt();
    let adjusted_r_squared = if regression.observations > regression.predictors + 1 {
        1.0 - (1.0 - regression.r_squared) * (regression.observations - 1) as f64
            / (regression.observations - regression.predictors - 1) as f64
    } else {
        regression.r_squared
    };
    let inverse = design
        .transpose_owned()
        .map_err(surface_invalid_request)?
        .matmul(&design.as_view())
        .map_err(surface_invalid_request)?
        .pseudoinverse(PseudoinverseOptions::default())
        .map_err(surface_invalid_request)?;
    let standard_errors = (0..regression.predictors)
        .map(|index| {
            let variance =
                inverse.values()[index * regression.predictors + index] * mean_squared_error;
            (variance.is_finite() && variance > 0.0).then(|| variance.sqrt())
        })
        .collect::<Vec<_>>();
    let t_statistics = regression
        .coefficients
        .iter()
        .zip(&standard_errors)
        .map(|(coefficient, standard_error)| {
            standard_error.and_then(|se| (se > 0.0).then_some(coefficient / se))
        })
        .collect::<Vec<_>>();
    Ok(OlsRegressionDiagnosticsF64 {
        regression,
        root_mean_squared_error,
        adjusted_r_squared,
        degrees_of_freedom,
        standard_errors,
        t_statistics,
    })
}

fn validate_ols_inputs(design: &F64Matrix, target: &[f64], tolerance: f64) -> Result<(), String> {
    if target.len() != design.shape().rows {
        return Err("invalid request: OLS target length must match design rows".to_string());
    }
    if target.iter().any(|value| !value.is_finite()) {
        return Err("invalid request: OLS target values must be finite".to_string());
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err("invalid request: OLS tolerance must be finite and non-negative".to_string());
    }
    Ok(())
}

fn ols_from_coefficients(
    design: &F64Matrix,
    target: &[f64],
    coefficients: Vec<f64>,
) -> Result<OlsRegressionF64, String> {
    let fitted = design
        .as_view()
        .matvec(&coefficients)
        .map_err(surface_invalid_request)?;
    let residuals = target
        .iter()
        .zip(&fitted)
        .map(|(actual, fitted)| actual - fitted)
        .collect::<Vec<_>>();
    let mean_target = target.iter().sum::<f64>() / target.len() as f64;
    let ss_res = residuals.iter().map(|value| value * value).sum::<f64>();
    let ss_tot = target
        .iter()
        .map(|value| (value - mean_target).powi(2))
        .sum::<f64>();
    let r_squared = if ss_tot <= f64::EPSILON {
        if ss_res <= f64::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        1.0 - ss_res / ss_tot
    };
    Ok(OlsRegressionF64 {
        coefficients,
        fitted,
        residuals,
        r_squared,
        observations: design.shape().rows,
        predictors: design.shape().cols,
    })
}

fn validate_value_count(count: usize) -> Result<(), String> {
    if count > MAX_VALUES {
        return Err(format!("values must not exceed {MAX_VALUES}"));
    }
    Ok(())
}

fn surface_invalid_request(error: impl std::fmt::Display) -> String {
    format!("invalid request: {error}")
}

fn parse_variance_mode(value: &str) -> Result<VarianceMode, String> {
    match value {
        "population" => Ok(VarianceMode::Population),
        "sample" => Ok(VarianceMode::Sample),
        mode => Err(format!("unsupported variance mode `{mode}`")),
    }
}

fn parse_change_method(value: &str) -> Result<ChangeMethod, String> {
    match value {
        "difference" => Ok(ChangeMethod::Difference),
        "relative" => Ok(ChangeMethod::Relative),
        "logRatio" | "log-ratio" | "log" => Ok(ChangeMethod::LogRatio),
        method => Err(format!("unsupported change method `{method}`")),
    }
}

fn default_variance_mode() -> String {
    "sample".to_string()
}

fn default_change_method() -> String {
    "difference".to_string()
}

fn default_confidence() -> f64 {
    0.95
}

fn default_f64_precision() -> String {
    "f64".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::OperationId;

    #[test]
    fn surface_lists_series_operations() {
        let ids = package_surface()
            .operations
            .iter()
            .map(|operation| operation.id.as_str().to_string())
            .collect::<Vec<_>>();
        for id in [
            "stats.series.describe",
            "stats.series.changes",
            "stats.series.compare",
            "stats.series.rolling",
            "stats.series.tailRisk",
            "stats.series.zScores",
        ] {
            assert!(ids.contains(&id.to_string()), "missing {id}");
        }
    }

    #[test]
    fn series_operations_run() {
        for operation in package_surface()
            .operations
            .into_iter()
            .filter(|operation| operation.id.as_str().starts_with("stats.series."))
        {
            let response = run_surface_operation(SurfaceRequest {
                operation: operation.id,
                input: operation.example_request,
            })
            .unwrap_or_else(|error| panic!("{} failed: {error}", operation.name));
            assert!(response.value.is_object());
        }
    }

    #[test]
    fn normalizes_min_max() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.normalize"),
            input: serde_json::json!({"matrix": {"rows": 2, "cols": 2, "values": [0.0, 1.0, 2.0, 3.0]}, "method": "minMax"}),
        }).expect("normalize");
        assert_eq!(
            response.value["values"],
            serde_json::json!([0.0, 0.0, 1.0, 1.0])
        );
    }

    #[test]
    fn covariance_can_include_correlation() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.covariance"),
            input: serde_json::json!({"matrix": {"rows": 3, "cols": 2, "values": [1.0, 0.0, 0.0, 1.0, 1.0, 1.0]}, "correlation": true}),
        }).expect("covariance");
        assert_eq!(response.value["count"], 3);
        assert!(response.value["correlation"].is_object());
    }

    #[test]
    fn pca_returns_components() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.pca"),
            input: serde_json::json!({"matrix": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 2.0, 2.0, 3.0, 3.0]}, "componentCount": 1, "transform": true}),
        }).expect("pca");
        assert_eq!(response.value["components"]["rows"], 1);
        assert!(response.value["transformed"].is_object());
    }

    #[test]
    fn regression_operations_run() {
        for operation in [
            "stats.series.rankCorrelation",
            "stats.regression.linear",
            "stats.regression.ols",
            "stats.regression.diagnostics",
        ] {
            let surface_operation = package_surface()
                .operations
                .into_iter()
                .find(|candidate| candidate.id.as_str() == operation)
                .expect("operation metadata");
            let response = run_surface_operation(SurfaceRequest {
                operation: surface_operation.id,
                input: surface_operation.example_request,
            })
            .unwrap_or_else(|error| panic!("{operation} failed: {error}"));
            assert!(response.value.is_object());
        }
    }

    #[test]
    fn regression_diagnostics_return_expected_payload() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.regression.diagnostics"),
            input: serde_json::json!({
                "design": {"rows": 4, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0, 1.0, 4.0]},
                "target": [3.0, 5.0, 7.0, 9.0],
                "tolerance": 0.0
            }),
        })
        .expect("diagnostics");
        assert_eq!(response.value["coefficients"].as_array().unwrap().len(), 2);
        assert_eq!(response.value["degreesOfFreedom"], 2);
        assert!(response.value["standardErrors"].is_array());
        assert!(response.value["tStatistics"].is_array());
    }

    #[test]
    fn ols_uses_pseudoinverse_but_diagnostics_stay_strict() {
        let ols = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.regression.ols"),
            input: serde_json::json!({
                "design": {"rows": 4, "cols": 2, "values": [1.0, 2.0, 2.0, 4.0, 3.0, 6.0, 4.0, 8.0]},
                "target": [1.0, 2.0, 3.0, 4.0]
            }),
        })
        .expect("rank-deficient ols");
        assert_eq!(ols.value["precision"], "f64");
        assert_eq!(ols.value["coefficients"].as_array().unwrap().len(), 2);

        let diagnostics = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("stats.regression.diagnostics"),
            input: serde_json::json!({
                "design": {"rows": 4, "cols": 2, "values": [1.0, 2.0, 2.0, 4.0, 3.0, 6.0, 4.0, 8.0]},
                "target": [1.0, 2.0, 3.0, 4.0]
            }),
        });
        assert!(diagnostics.is_err());
    }
}
