//! Library-owned runtime surface for `math-linear`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, PackageSurface, RuntimeCapabilities, SurfaceError, SurfaceRequest,
    SurfaceResponse,
};
use serde::Deserialize;
use tensor_data::F32Tensor;

use crate::{
    F32Matrix, F64Matrix, Kernel1d, MatrixShape, PseudoinverseOptions, SvdDecomposition, SvdOptions,
};

const MAX_VALUES: usize = 100_000;
const MAX_SVD_DIMENSION: usize = 512;

/// Describes the linear algebra operations exposed by transport wrappers.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            surface_operation(
                "describe",
                "Describe package",
                "Dense matrix and kernel contracts bridging tensor-data and vector-analysis-core.",
                serde_json::json!({"includeOperations": true}),
            ),
            surface_operation(
                "linear.matmul",
                "Matrix multiply",
                "Multiplies two finite f32 row-major matrices.",
                serde_json::json!({
                    "left": {"rows": 2, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0]},
                    "right": {"rows": 2, "cols": 1, "values": [5.0, 6.0]}
                }),
            ),
            surface_operation(
                "linear.transpose",
                "Matrix transpose",
                "Returns a row-major owned transpose of a finite f32 matrix.",
                serde_json::json!({
                    "matrix": {"rows": 2, "cols": 3, "values": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]}
                }),
            ),
            surface_operation(
                "linear.solve",
                "Linear solve",
                "Solves a square finite f32 matrix against a vector or matrix right-hand side.",
                serde_json::json!({
                    "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]},
                    "rhs": [1.0, 2.0]
                }),
            ),
            surface_operation(
                "linear.decompose",
                "LU decomposition",
                "Decomposes a square finite f32 matrix with partial-pivoted LU.",
                serde_json::json!({
                    "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]}
                }),
            ),
            surface_operation(
                "linear.inverse",
                "Matrix inverse",
                "Returns the inverse of a finite square f32 matrix.",
                serde_json::json!({
                    "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]}
                }),
            ),
            surface_operation(
                "linear.kernel1d",
                "1D kernel",
                "Validates and optionally normalizes a finite 1D f32 kernel.",
                serde_json::json!({"values": [0.25, 0.5, 0.25], "normalize": true}),
            ),
            surface_operation(
                "linear.tensorBridge",
                "Tensor matrix bridge",
                "Projects rank-2 tensor payloads to matrix shape or matrix-shaped payloads to tensor shape.",
                serde_json::json!({"shape": [2, 2], "values": [1.0, 2.0, 3.0, 4.0], "direction": "tensorToMatrix"}),
            ),
            surface_operation(
                "linear.gram",
                "Gram matrix",
                "Computes row or column Gram matrices for a finite f32 matrix.",
                serde_json::json!({"matrix": {"rows": 2, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0]}, "axis": "rows"}),
            ),
            surface_operation(
                "linear.cholesky",
                "Cholesky decomposition",
                "Factors a symmetric positive definite matrix into lower-triangular Cholesky form.",
                serde_json::json!({"matrix": {"rows": 2, "cols": 2, "values": [4.0, 2.0, 2.0, 3.0]}}),
            ),
            surface_operation(
                "linear.qr",
                "QR decomposition",
                "Factors a full-column-rank matrix with deterministic modified Gram-Schmidt QR.",
                serde_json::json!({"matrix": {"rows": 3, "cols": 2, "values": [1.0, 0.0, 1.0, 1.0, 0.0, 1.0]}}),
            ),
            surface_operation(
                "linear.center",
                "Center matrix",
                "Subtracts row or column means from a finite f32 matrix.",
                serde_json::json!({"matrix": {"rows": 2, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0]}, "axis": "columns"}),
            ),
            surface_operation(
                "linear.leastSquares",
                "Least squares",
                "Fits a full-column-rank QR least-squares model for a finite f32 design matrix.",
                serde_json::json!({
                    "matrix": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0]},
                    "target": [3.0, 5.0, 7.0],
                    "tolerance": 0.0
                }),
            ),
            surface_operation(
                "linear.svd",
                "SVD",
                "Computes compact singular values, rank, and condition diagnostics for a finite real matrix; thin factors are opt-in.",
                serde_json::json!({
                    "matrix": {"rows": 3, "cols": 2, "values": [1.0, 0.0, 1.0, 1.0, 0.0, 1.0]},
                    "precision": "f64",
                    "computeFactors": false
                }),
            ),
            surface_operation(
                "linear.pseudoinverse",
                "Pseudoinverse",
                "Computes a Moore-Penrose pseudoinverse from the pure Rust SVD path.",
                serde_json::json!({
                    "matrix": {"rows": 3, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]},
                    "precision": "f64"
                }),
            ),
            surface_operation(
                "linear.rank",
                "Numerical rank",
                "Computes singular-value based numerical rank for a finite real matrix.",
                serde_json::json!({
                    "matrix": {"rows": 3, "cols": 2, "values": [1.0, 2.0, 2.0, 4.0, 3.0, 6.0]},
                    "precision": "f64",
                    "tolerance": 1.0e-8
                }),
            ),
        ],
    }
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "linear.matmul" => matmul_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.transpose" => transpose_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.solve" => solve_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.decompose" => decompose_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.inverse" => inverse_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.kernel1d" => kernel1d_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.tensorBridge" => tensor_bridge_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.gram" => gram_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.cholesky" => cholesky_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.qr" => qr_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.center" => center_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.leastSquares" => least_squares_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.svd" => svd_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.pseudoinverse" => pseudoinverse_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        "linear.rank" => rank_value(parse_surface_input(
            Some(operation.as_str()),
            request.input,
        )?)?,
        operation => {
            return Err(
                SurfaceError::unsupported_operation(operation, env!("CARGO_PKG_NAME"))
                    .to_error_string(),
            );
        }
    };
    Ok(structured_operation_response(&surface, operation, value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatmulRequest {
    left: MatrixRequest,
    right: MatrixRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnaryMatrixRequest {
    matrix: MatrixRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatrixAxisRequest {
    matrix: MatrixRequest,
    #[serde(default = "default_columns_axis")]
    axis: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeastSquaresRequest {
    matrix: MatrixRequest,
    target: Vec<f32>,
    #[serde(default)]
    tolerance: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SolveRequest {
    matrix: MatrixRequest,
    #[serde(default)]
    rhs: Option<Vec<f32>>,
    #[serde(default)]
    rhs_matrix: Option<MatrixRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatrixRequest {
    rows: usize,
    cols: usize,
    values: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrecisionMatrixRequest {
    rows: usize,
    cols: usize,
    values: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SvdRequest {
    matrix: PrecisionMatrixRequest,
    #[serde(default = "default_f64_precision")]
    precision: String,
    #[serde(default)]
    tolerance: Option<f64>,
    #[serde(default)]
    max_sweeps: Option<usize>,
    #[serde(default)]
    compute_factors: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KernelRequest {
    values: Vec<f32>,
    #[serde(default)]
    normalize: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TensorBridgeRequest {
    shape: Vec<usize>,
    values: Vec<f32>,
    direction: String,
}

fn matmul_value(request: MatmulRequest) -> Result<serde_json::Value, String> {
    let left = matrix_from_request(request.left)?;
    let right = matrix_from_request(request.right)?;
    let product = left
        .matmul(&right.as_view())
        .map_err(|error| error.to_string())?;
    matrix_json(product)
}

fn transpose_value(request: UnaryMatrixRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let transpose = matrix
        .as_view()
        .transpose_owned()
        .map_err(|error| error.to_string())?;
    matrix_json(transpose)
}

fn solve_value(request: SolveRequest) -> Result<serde_json::Value, String> {
    if request.rhs.is_some() && request.rhs_matrix.is_some() {
        return Err("linear.solve accepts either rhs or rhsMatrix, not both".to_string());
    }
    if let Some(rhs) = request.rhs.as_ref() {
        validate_value_count(rhs.len())?;
    }
    if let Some(rhs_matrix) = request.rhs_matrix.as_ref() {
        validate_value_count(rhs_matrix.values.len())?;
    }
    let matrix = matrix_from_request(request.matrix)?;
    let decomposition = matrix
        .as_view()
        .lu_decompose()
        .map_err(|error| error.to_string())?;
    let determinant = decomposition
        .determinant()
        .map_err(|error| error.to_string())?;

    match (request.rhs, request.rhs_matrix) {
        (Some(_), Some(_)) => unreachable!("checked above"),
        (Some(rhs), None) => {
            let solution = decomposition
                .solve_vector(&rhs)
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "solution": solution,
                "determinant": determinant
            }))
        }
        (None, Some(rhs_matrix)) => {
            let rhs = matrix_from_request(rhs_matrix)?;
            let solution = decomposition
                .solve_matrix(&rhs.as_view())
                .map_err(|error| error.to_string())?;
            let shape = solution.shape();
            Ok(serde_json::json!({
                "solutionMatrix": {
                    "rows": shape.rows,
                    "cols": shape.cols,
                    "values": solution.values()
                },
                "determinant": determinant
            }))
        }
        (None, None) => Err("linear.solve requires rhs or rhsMatrix".to_string()),
    }
}

fn decompose_value(request: UnaryMatrixRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let decomposition = matrix
        .as_view()
        .lu_decompose()
        .map_err(|error| error.to_string())?;
    let determinant = decomposition
        .determinant()
        .map_err(|error| error.to_string())?;
    let lower = decomposition
        .lower_matrix()
        .map_err(|error| error.to_string())?;
    let upper = decomposition
        .upper_matrix()
        .map_err(|error| error.to_string())?;
    let shape = decomposition.shape();
    Ok(serde_json::json!({
        "method": "lu",
        "rows": shape.rows,
        "cols": shape.cols,
        "pivots": decomposition.pivots(),
        "swapCount": decomposition.swap_count(),
        "determinant": determinant,
        "lower": {
            "rows": lower.shape().rows,
            "cols": lower.shape().cols,
            "values": lower.values()
        },
        "upper": {
            "rows": upper.shape().rows,
            "cols": upper.shape().cols,
            "values": upper.values()
        }
    }))
}

fn inverse_value(request: UnaryMatrixRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let inverse = matrix.inverse().map_err(|error| error.to_string())?;
    matrix_json(inverse)
}

fn kernel1d_value(request: KernelRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    let kernel = Kernel1d::new(request.values).map_err(|error| error.to_string())?;
    let sum = kernel.values().iter().sum::<f32>();
    let mut value = serde_json::json!({
        "len": kernel.values().len(),
        "sum": sum,
        "center": kernel.values().len() / 2,
        "values": kernel.values()
    });
    if request.normalize {
        if sum.abs() <= f32::EPSILON {
            return Err("1D kernel sum must be non-zero to normalize".to_string());
        }
        value["normalizedValues"] = serde_json::json!(kernel
            .values()
            .iter()
            .map(|value| value / sum)
            .collect::<Vec<_>>());
    }
    Ok(value)
}

fn tensor_bridge_value(request: TensorBridgeRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.values.len())?;
    match request.direction.as_str() {
        "tensorToMatrix" => {
            let tensor = F32Tensor::from_dims(request.shape, request.values)
                .map_err(|error| error.to_string())?;
            let matrix = F32Matrix::try_from(&tensor).map_err(|error| error.to_string())?;
            let shape = matrix.shape();
            Ok(serde_json::json!({
                "direction": "tensorToMatrix",
                "shape": [shape.rows, shape.cols],
                "rows": shape.rows,
                "cols": shape.cols,
                "values": matrix.values()
            }))
        }
        "matrixToTensor" => {
            if request.shape.len() != 2 {
                return Err("matrixToTensor requires exactly two shape dimensions".to_string());
            }
            let matrix = F32Matrix::new(
                MatrixShape::new(request.shape[0], request.shape[1])
                    .map_err(|error| error.to_string())?,
                request.values,
            )
            .map_err(|error| error.to_string())?;
            let tensor = F32Tensor::try_from(&matrix).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "direction": "matrixToTensor",
                "shape": tensor.shape().dimensions(),
                "values": tensor.values()
            }))
        }
        direction => Err(format!("unsupported tensor bridge direction `{direction}`")),
    }
}

fn gram_value(request: MatrixAxisRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let gram = match request.axis.as_str() {
        "rows" => matrix.gram_rows(),
        "columns" | "cols" => matrix.gram_columns(),
        axis => return Err(format!("unsupported Gram axis `{axis}`")),
    }
    .map_err(|error| error.to_string())?;
    let mut value = matrix_json(gram)?;
    value["axis"] = serde_json::json!(request.axis);
    Ok(value)
}

fn cholesky_value(request: UnaryMatrixRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let decomposition = matrix
        .cholesky_decompose()
        .map_err(|error| error.to_string())?;
    let reconstructed = decomposition
        .lower
        .matmul(&decomposition.lower.transpose_view())
        .map_err(|error| error.to_string())?;
    let condition = matrix
        .condition_estimate()
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "method": "cholesky",
        "lower": matrix_projection(&decomposition.lower),
        "reconstructed": matrix_projection(&reconstructed),
        "condition": {
            "determinant": condition.determinant,
            "diagonalMinAbs": condition.diagonal_min_abs,
            "diagonalMaxAbs": condition.diagonal_max_abs
        }
    }))
}

fn qr_value(request: UnaryMatrixRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let decomposition = matrix.qr_decompose().map_err(|error| error.to_string())?;
    let reconstructed = decomposition
        .q
        .matmul(&decomposition.r.as_view())
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "method": "qr",
        "q": matrix_projection(&decomposition.q),
        "r": matrix_projection(&decomposition.r),
        "reconstructed": matrix_projection(&reconstructed)
    }))
}

fn center_value(request: MatrixAxisRequest) -> Result<serde_json::Value, String> {
    let matrix = matrix_from_request(request.matrix)?;
    let centered = match request.axis.as_str() {
        "rows" => matrix.center_rows(),
        "columns" | "cols" => matrix.center_columns(),
        axis => return Err(format!("unsupported center axis `{axis}`")),
    }
    .map_err(|error| error.to_string())?;
    let mut value = matrix_json(centered)?;
    value["axis"] = serde_json::json!(request.axis);
    Ok(value)
}

fn least_squares_value(request: LeastSquaresRequest) -> Result<serde_json::Value, String> {
    validate_value_count(request.target.len())?;
    let matrix = matrix_from_request(request.matrix)?;
    let solution = matrix
        .least_squares(&request.target, request.tolerance)
        .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "coefficients": solution.coefficients,
        "fitted": solution.fitted,
        "residuals": solution.residuals,
        "residualSumSquares": solution.residual_sum_squares,
        "rank": solution.rank,
        "observations": solution.observations,
        "predictors": solution.predictors
    }))
}

fn svd_value(request: SvdRequest) -> Result<serde_json::Value, String> {
    let compute_factors = request.compute_factors;
    let matrix = precision_matrix_from_request(request.matrix, &request.precision)?;
    let svd = matrix
        .svd(SvdOptions {
            tolerance: request.tolerance,
            max_sweeps: request.max_sweeps,
            compute_factors,
            max_dimension: Some(MAX_SVD_DIMENSION),
        })
        .map_err(surface_invalid_request)?;
    Ok(svd_json(svd, compute_factors, &request.precision))
}

fn pseudoinverse_value(request: SvdRequest) -> Result<serde_json::Value, String> {
    let matrix = precision_matrix_from_request(request.matrix, &request.precision)?;
    let inverse = matrix
        .pseudoinverse(PseudoinverseOptions {
            tolerance: request.tolerance,
            max_sweeps: request.max_sweeps,
            max_dimension: Some(MAX_SVD_DIMENSION),
        })
        .map_err(surface_invalid_request)?;
    let mut value = matrix_json_f64(&inverse);
    value["precision"] = serde_json::json!(request.precision);
    Ok(value)
}

fn rank_value(request: SvdRequest) -> Result<serde_json::Value, String> {
    let matrix = precision_matrix_from_request(request.matrix, &request.precision)?;
    let rank = matrix
        .svd(SvdOptions {
            tolerance: request.tolerance,
            max_sweeps: request.max_sweeps,
            compute_factors: false,
            max_dimension: Some(MAX_SVD_DIMENSION),
        })
        .map_err(surface_invalid_request)?;
    Ok(serde_json::json!({
        "precision": request.precision,
        "rank": rank.rank,
        "singularValues": rank.singular_values,
        "conditionEstimate": rank.condition_estimate,
        "tolerance": rank.tolerance,
        "sweeps": rank.sweeps
    }))
}

fn matrix_from_request(request: MatrixRequest) -> Result<F32Matrix, String> {
    validate_value_count(request.values.len())?;
    F32Matrix::new(
        MatrixShape::new(request.rows, request.cols).map_err(|error| error.to_string())?,
        request.values,
    )
    .map_err(|error| error.to_string())
}

fn precision_matrix_from_request(
    request: PrecisionMatrixRequest,
    precision: &str,
) -> Result<F64Matrix, String> {
    validate_value_count(request.values.len())?;
    if request.rows.max(request.cols) > MAX_SVD_DIMENSION {
        return Err(format!(
            "invalid request: SVD-class operations require max(rows, cols) <= {MAX_SVD_DIMENSION}"
        ));
    }
    let shape = MatrixShape::new(request.rows, request.cols).map_err(surface_invalid_request)?;
    match precision {
        "f64" => F64Matrix::new(shape, request.values).map_err(surface_invalid_request),
        "f32" => {
            let mut values = Vec::with_capacity(request.values.len());
            for value in request.values {
                if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
                    return Err(
                        "invalid request: precision f32 requires finite in-range values"
                            .to_string(),
                    );
                }
                values.push(value as f32);
            }
            let matrix = F32Matrix::new(shape, values).map_err(surface_invalid_request)?;
            F64Matrix::try_from(&matrix).map_err(surface_invalid_request)
        }
        precision => Err(format!(
            "invalid request: unsupported precision `{precision}`; expected `f32` or `f64`"
        )),
    }
}

fn matrix_json(matrix: F32Matrix) -> Result<serde_json::Value, String> {
    let shape = matrix.shape();
    Ok(serde_json::json!({
        "rows": shape.rows,
        "cols": shape.cols,
        "values": matrix.values()
    }))
}

fn matrix_json_f64(matrix: &F64Matrix) -> serde_json::Value {
    let shape = matrix.shape();
    serde_json::json!({
        "rows": shape.rows,
        "cols": shape.cols,
        "values": matrix.values()
    })
}

fn matrix_projection(matrix: &F32Matrix) -> serde_json::Value {
    let shape = matrix.shape();
    serde_json::json!({
        "rows": shape.rows,
        "cols": shape.cols,
        "values": matrix.values()
    })
}

fn svd_json(svd: SvdDecomposition, include_factors: bool, precision: &str) -> serde_json::Value {
    let mut value = serde_json::json!({
        "precision": precision,
        "singularValues": svd.singular_values,
        "rank": svd.rank,
        "conditionEstimate": svd.condition_estimate,
        "sweeps": svd.sweeps,
        "tolerance": svd.tolerance,
        "reconstruction": {
            "residualFrobenius": svd.reconstruction.residual_frobenius,
            "relativeResidual": svd.reconstruction.relative_residual,
            "maxAbsDiff": svd.reconstruction.max_abs_diff
        }
    });
    if include_factors {
        if let Some(u) = svd.u.as_ref() {
            value["u"] = matrix_json_f64(u);
        }
        if let Some(vt) = svd.vt.as_ref() {
            value["vt"] = matrix_json_f64(vt);
        }
    }
    value
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

fn default_columns_axis() -> String {
    "columns".to_string()
}

fn default_f64_precision() -> String {
    "f64".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::OperationId;

    fn assert_close(left: f32, right: f32) {
        assert!((left - right).abs() < 1.0e-4, "expected {left} ≈ {right}");
    }

    fn f32_array(value: &serde_json::Value) -> Vec<f32> {
        serde_json::from_value(value.clone()).expect("f32 array")
    }

    #[test]
    fn matmul_returns_product() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.matmul"),
            input: serde_json::json!({
                "left": {"rows": 2, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0]},
                "right": {"rows": 2, "cols": 1, "values": [5.0, 6.0]}
            }),
        })
        .expect("matmul operation");

        assert_eq!(response.value["rows"], 2);
        assert_eq!(response.value["cols"], 1);
        assert_eq!(response.value["values"], serde_json::json!([17.0, 39.0]));
    }

    #[test]
    fn kernel_normalizes_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.kernel1d"),
            input: serde_json::json!({"values": [1.0, 1.0, 2.0], "normalize": true}),
        })
        .expect("kernel operation");

        assert_eq!(response.value["len"], 3);
        assert_eq!(response.value["center"], 1);
        assert_eq!(
            response.value["normalizedValues"],
            serde_json::json!([0.25, 0.25, 0.5])
        );
    }

    #[test]
    fn tensor_bridge_reports_matrix_shape() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.tensorBridge"),
            input: serde_json::json!({"shape": [2, 2], "values": [1.0, 2.0, 3.0, 4.0], "direction": "tensorToMatrix"}),
        })
        .expect("tensor bridge operation");

        assert_eq!(response.value["rows"], 2);
        assert_eq!(response.value["cols"], 2);
    }

    #[test]
    fn transpose_returns_expected_shape_and_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.transpose"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 3, "values": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]}
            }),
        })
        .expect("transpose operation");

        assert_eq!(response.value["rows"], 3);
        assert_eq!(response.value["cols"], 2);
        assert_eq!(
            response.value["values"],
            serde_json::json!([1.0, 4.0, 2.0, 5.0, 3.0, 6.0])
        );
    }

    #[test]
    fn solve_with_vector_rhs_returns_solution() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.solve"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]},
                "rhs": [1.0, 2.0]
            }),
        })
        .expect("solve operation");

        let solution = f32_array(&response.value["solution"]);
        assert_close(solution[0], 0.2);
        assert_close(solution[1], 0.6);
        assert_close(
            serde_json::from_value(response.value["determinant"].clone()).unwrap(),
            5.0,
        );
    }

    #[test]
    fn svd_operations_default_to_f64_and_compact_output() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.svd"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 2, "values": [1.0, 0.0, 0.0, 2.0]}
            }),
        })
        .expect("svd operation");

        assert_eq!(response.value["precision"], "f64");
        assert_eq!(response.value["rank"], 2);
        assert!(response.value["u"].is_null());
        assert!(
            response.value["singularValues"].as_array().unwrap()[0]
                .as_f64()
                .unwrap()
                >= 2.0
        );
    }

    #[test]
    fn svd_can_return_thin_factors() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.svd"),
            input: serde_json::json!({
                "matrix": {"rows": 3, "cols": 2, "values": [1.0, 0.0, 1.0, 1.0, 0.0, 1.0]},
                "computeFactors": true,
                "precision": "f32"
            }),
        })
        .expect("svd operation");

        assert_eq!(response.value["u"]["rows"], 3);
        assert_eq!(response.value["vt"]["cols"], 2);
    }

    #[test]
    fn pseudoinverse_and_rank_operations_run() {
        let pinv = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.pseudoinverse"),
            input: serde_json::json!({
                "matrix": {"rows": 3, "cols": 2, "values": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]}
            }),
        })
        .expect("pseudoinverse operation");
        assert_eq!(pinv.value["rows"], 2);
        assert_eq!(pinv.value["cols"], 3);

        let rank = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.rank"),
            input: serde_json::json!({
                "matrix": {"rows": 3, "cols": 2, "values": [1.0, 2.0, 2.0, 4.0, 3.0, 6.0]},
                "tolerance": 1.0e-8
            }),
        })
        .expect("rank operation");
        assert_eq!(rank.value["rank"], 1);
    }

    #[test]
    fn solve_with_matrix_rhs_returns_inverse_like_result() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.solve"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]},
                "rhsMatrix": {"rows": 2, "cols": 2, "values": [1.0, 0.0, 0.0, 1.0]}
            }),
        })
        .expect("solve operation");

        assert_eq!(response.value["solutionMatrix"]["rows"], 2);
        assert_eq!(response.value["solutionMatrix"]["cols"], 2);
        let values = f32_array(&response.value["solutionMatrix"]["values"]);
        assert_close(values[0], 0.6);
        assert_close(values[1], -0.2);
        assert_close(values[2], -0.2);
        assert_close(values[3], 0.4);
    }

    #[test]
    fn decompose_returns_lower_and_upper_objects() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.decompose"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]}
            }),
        })
        .expect("decompose operation");

        assert_eq!(response.value["method"], "lu");
        assert_eq!(response.value["lower"]["rows"], 2);
        assert_eq!(response.value["upper"]["cols"], 2);
        assert!(response.value["lower"]["values"].is_array());
        assert!(response.value["upper"]["values"].is_array());
    }

    #[test]
    fn inverse_returns_expected_values() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.inverse"),
            input: serde_json::json!({
                "matrix": {"rows": 2, "cols": 2, "values": [2.0, 1.0, 1.0, 3.0]}
            }),
        })
        .expect("inverse operation");

        let values = f32_array(&response.value["values"]);
        assert_close(values[0], 0.6);
        assert_close(values[1], -0.2);
        assert_close(values[2], -0.2);
        assert_close(values[3], 0.4);
    }

    #[test]
    fn new_linear_operations_run() {
        for operation in [
            "linear.gram",
            "linear.cholesky",
            "linear.qr",
            "linear.center",
            "linear.leastSquares",
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
    fn least_squares_returns_coefficients() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("linear.leastSquares"),
            input: serde_json::json!({
                "matrix": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0]},
                "target": [3.0, 5.0, 7.0],
                "tolerance": 0.0
            }),
        })
        .expect("least squares");
        let coefficients = f32_array(&response.value["coefficients"]);
        assert_close(coefficients[0], 1.0);
        assert_close(coefficients[1], 2.0);
        assert_eq!(response.value["rank"], 2);
    }
}
