#![doc = include_str!("../README.md")]

mod backend;
mod f64_matrix;
pub mod surface;
mod svd;

pub use f64_matrix::{F64ColumnView, F64Matrix, F64MatrixView, F64RowView};
pub use svd::{PseudoinverseOptions, ReconstructionDiagnostics, SvdDecomposition, SvdOptions};

use media_core::{DetectError, Result};
use tensor_data::{F32Tensor, F32TensorView, TensorShape};
use vector_analysis_core::{cosine_similarity, dot, l2_norm, DenseVector};

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Checked dense matrix dimensions.
pub struct MatrixShape {
    /// Number of matrix rows.
    pub rows: usize,
    /// Number of matrix columns.
    pub cols: usize,
}

impl MatrixShape {
    /// Creates a shape with non-zero rows and columns.
    pub fn new(rows: usize, cols: usize) -> Result<Self> {
        let shape = Self { rows, cols };
        shape.validate()?;
        Ok(shape)
    }

    /// Verifies non-zero dimensions and element-count overflow safety.
    pub fn validate(self) -> Result<()> {
        if self.rows == 0 || self.cols == 0 {
            return Err(invalid_argument(
                "matrix rows and cols must be greater than zero",
            ));
        }
        let _ = self.element_count()?;
        Ok(())
    }

    /// Multiplies rows by columns and fails on `usize` overflow.
    pub fn element_count(self) -> Result<usize> {
        self.rows
            .checked_mul(self.cols)
            .ok_or_else(|| invalid_argument("matrix element count overflowed usize"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Memory interpretation for a matrix or matrix view.
pub enum MatrixLayout {
    /// Contiguous rows, where adjacent values advance across columns.
    RowMajor,
    /// Contiguous columns, used by transpose views without copying.
    ColumnMajor,
}

#[derive(Debug, Clone, PartialEq)]
/// Owned finite `f32` matrix stored in row-major order.
pub struct F32Matrix {
    shape: MatrixShape,
    layout: MatrixLayout,
    values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
/// Least-squares solution and residual diagnostics for a full-column-rank design.
pub struct LeastSquaresSolution {
    /// Fitted coefficients, one per design column.
    pub coefficients: Vec<f32>,
    /// Fitted target values.
    pub fitted: Vec<f32>,
    /// Residuals as `target - fitted`.
    pub residuals: Vec<f32>,
    /// Sum of squared residuals.
    pub residual_sum_squares: f32,
    /// Estimated matrix rank.
    pub rank: usize,
    /// Number of design rows.
    pub observations: usize,
    /// Number of design columns.
    pub predictors: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Deterministic solve diagnostics for dense finite matrices.
pub struct LinearSolveDiagnostics {
    /// Determinant when the matrix is square.
    pub determinant: Option<f32>,
    /// Tolerance-aware rank estimate.
    pub rank: usize,
    /// Ratio of smallest to largest LU diagonal pivot magnitude, when available.
    pub pivot_ratio: Option<f32>,
    /// Number of matrix rows.
    pub observations: usize,
    /// Number of matrix columns.
    pub predictors: usize,
}

impl F32Matrix {
    /// Creates a row-major matrix after shape and finite-value validation.
    pub fn new(shape: MatrixShape, values: Vec<f32>) -> Result<Self> {
        let matrix = Self {
            shape,
            layout: MatrixLayout::RowMajor,
            values,
        };
        matrix.validate()?;
        Ok(matrix)
    }

    /// Creates a row-major matrix filled with zeros.
    pub fn zeros(rows: usize, cols: usize) -> Result<Self> {
        let shape = MatrixShape::new(rows, cols)?;
        Self::new(shape, vec![0.0; shape.element_count()?])
    }

    /// Creates a row-major square identity matrix.
    pub fn identity(size: usize) -> Result<Self> {
        let shape = MatrixShape::new(size, size)?;
        let mut values = vec![0.0; shape.element_count()?];
        for index in 0..size {
            values[index * size + index] = 1.0;
        }
        Self::new(shape, values)
    }

    /// Builds a matrix from compile-time-sized row arrays.
    pub fn from_rows<const R: usize, const C: usize>(rows: [[f32; C]; R]) -> Result<Self> {
        let mut values = Vec::with_capacity(R * C);
        for row in rows {
            values.extend(row);
        }
        Self::new(MatrixShape::new(R, C)?, values)
    }

    /// Creates a square matrix with the supplied finite diagonal values.
    pub fn from_diag(diagonal: &[f32]) -> Result<Self> {
        if diagonal.is_empty() {
            return Err(invalid_argument("matrix diagonal must not be empty"));
        }
        if diagonal.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("matrix diagonal values must be finite"));
        }
        let mut matrix = Self::zeros(diagonal.len(), diagonal.len())?;
        for (index, value) in diagonal.iter().copied().enumerate() {
            matrix.values[index * diagonal.len() + index] = value;
        }
        Ok(matrix)
    }

    /// Returns the checked row and column dimensions.
    pub fn shape(&self) -> MatrixShape {
        self.shape
    }

    /// Returns the matrix storage layout.
    pub fn layout(&self) -> MatrixLayout {
        self.layout
    }

    /// Borrows the contiguous matrix values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Consumes the matrix and returns its contiguous values.
    pub fn into_values(self) -> Vec<f32> {
        self.values
    }

    /// Borrows this matrix as a view without copying values.
    pub fn as_view(&self) -> F32MatrixView<'_> {
        F32MatrixView {
            shape: self.shape,
            layout: self.layout,
            values: &self.values,
        }
    }

    /// Borrows one row, respecting the current matrix layout.
    pub fn row(&self, index: usize) -> Result<RowView<'_>> {
        self.as_view().row(index)
    }

    /// Borrows one column, respecting the current matrix layout.
    pub fn column(&self, index: usize) -> Result<ColumnView<'_>> {
        self.as_view().column(index)
    }

    /// Multiplies this matrix by `right`.
    pub fn matmul(&self, right: &F32MatrixView<'_>) -> Result<Self> {
        self.as_view().matmul(right)
    }

    /// Multiplies this matrix by a finite dense vector.
    pub fn matvec(&self, vector: &[f32]) -> Result<DenseVector> {
        self.as_view().matvec(vector)
    }

    /// Creates a transposed view without copying values.
    pub fn transpose_view(&self) -> F32MatrixView<'_> {
        self.as_view().transpose()
    }

    /// Returns a row-major owned transpose of this matrix.
    pub fn transpose_owned(&self) -> Result<Self> {
        self.as_view().transpose_owned()
    }

    /// Returns a row-major matrix whose rows have unit L2 norm.
    pub fn l2_normalize_rows(&self) -> Result<Self> {
        self.as_view().l2_normalize_rows()
    }

    /// Returns a row-major matrix whose columns have unit L2 norm.
    pub fn l2_normalize_columns(&self) -> Result<Self> {
        self.as_view().l2_normalize_columns()
    }

    /// Computes all pairwise row cosine similarities against `right`.
    pub fn pairwise_row_cosine(&self, right: &F32MatrixView<'_>) -> Result<Self> {
        self.as_view().pairwise_row_cosine(right)
    }

    /// Returns this matrix diagonal values.
    pub fn diagonal(&self) -> Result<Vec<f32>> {
        self.as_view().diagonal()
    }

    /// Computes `self * self^T`.
    pub fn gram_rows(&self) -> Result<Self> {
        self.as_view().gram_rows()
    }

    /// Computes `self^T * self`.
    pub fn gram_columns(&self) -> Result<Self> {
        self.as_view().gram_columns()
    }

    /// Returns a row-major matrix with column means subtracted.
    pub fn center_columns(&self) -> Result<Self> {
        self.as_view().center_columns()
    }

    /// Returns a row-major matrix with row means subtracted.
    pub fn center_rows(&self) -> Result<Self> {
        self.as_view().center_rows()
    }

    /// Decomposes this square matrix using LU factorization with partial pivoting.
    pub fn lu_decompose(&self) -> Result<LuDecomposition> {
        self.as_view().lu_decompose()
    }

    /// Decomposes this symmetric positive definite matrix using Cholesky factorization.
    pub fn cholesky_decompose(&self) -> Result<CholeskyDecomposition> {
        self.as_view().cholesky_decompose()
    }

    /// Decomposes this matrix with modified Gram-Schmidt QR factorization.
    pub fn qr_decompose(&self) -> Result<QrDecomposition> {
        self.as_view().qr_decompose()
    }

    /// Returns simple determinant and diagonal magnitude diagnostics.
    pub fn condition_estimate(&self) -> Result<MatrixConditionEstimate> {
        self.as_view().condition_estimate()
    }

    /// Returns a deterministic modified Gram-Schmidt rank estimate.
    pub fn rank_estimate(&self, tolerance: f32) -> Result<usize> {
        self.as_view().rank_estimate(tolerance)
    }

    /// Fits a full-column-rank least-squares model with QR factorization.
    pub fn least_squares(&self, target: &[f32], tolerance: f32) -> Result<LeastSquaresSolution> {
        self.as_view().least_squares(target, tolerance)
    }

    /// Returns deterministic linear solve diagnostics.
    pub fn solve_diagnostics(&self, tolerance: f32) -> Result<LinearSolveDiagnostics> {
        self.as_view().solve_diagnostics(tolerance)
    }

    /// Computes this square matrix determinant through LU decomposition.
    pub fn determinant(&self) -> Result<f32> {
        self.as_view().determinant()
    }

    /// Solves `A x = b` for a finite vector `b`.
    pub fn solve_vector(&self, b: &[f32]) -> Result<Vec<f32>> {
        self.as_view().solve_vector(b)
    }

    /// Solves `A X = B` for matrix `B`.
    pub fn solve_matrix(&self, b: &F32MatrixView<'_>) -> Result<Self> {
        self.as_view().solve_matrix(b)
    }

    /// Computes this square matrix inverse.
    pub fn inverse(&self) -> Result<Self> {
        self.as_view().inverse()
    }

    /// Verifies shape/value count agreement and rejects non-finite values.
    pub fn validate(&self) -> Result<()> {
        self.shape.validate()?;
        if self.values.len() != self.shape.element_count()? {
            return Err(invalid_argument(format!(
                "matrix shape expects {} values but matrix has {}",
                self.shape.element_count()?,
                self.values.len()
            )));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("matrix values must be finite"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Borrowed finite `f32` matrix values with shape and layout metadata.
pub struct F32MatrixView<'a> {
    shape: MatrixShape,
    layout: MatrixLayout,
    values: &'a [f32],
}

impl<'a> F32MatrixView<'a> {
    /// Creates a row-major matrix view after validating shape and values.
    pub fn new(shape: MatrixShape, values: &'a [f32]) -> Result<Self> {
        let view = Self {
            shape,
            layout: MatrixLayout::RowMajor,
            values,
        };
        view.validate()?;
        Ok(view)
    }

    /// Returns the checked row and column dimensions.
    pub fn shape(&self) -> MatrixShape {
        self.shape
    }

    /// Returns how the borrowed value slice is interpreted.
    pub fn layout(&self) -> MatrixLayout {
        self.layout
    }

    /// Borrows the underlying contiguous values.
    pub fn values(&self) -> &'a [f32] {
        self.values
    }

    /// Creates a transposed view by swapping dimensions and layout metadata.
    pub fn transpose(self) -> Self {
        Self {
            shape: MatrixShape {
                rows: self.shape.cols,
                cols: self.shape.rows,
            },
            layout: match self.layout {
                MatrixLayout::RowMajor => MatrixLayout::ColumnMajor,
                MatrixLayout::ColumnMajor => MatrixLayout::RowMajor,
            },
            values: self.values,
        }
    }

    /// Returns a row-major owned transpose of this view.
    pub fn transpose_owned(&self) -> Result<F32Matrix> {
        self.transpose().into_owned()
    }

    /// Borrows one logical row from this view.
    pub fn row(self, index: usize) -> Result<RowView<'a>> {
        if index >= self.shape.rows {
            return Err(invalid_argument(format!(
                "row index {index} is out of bounds for {} rows",
                self.shape.rows
            )));
        }
        let (offset, stride) = match self.layout {
            MatrixLayout::RowMajor => (index * self.shape.cols, 1),
            MatrixLayout::ColumnMajor => (index, self.shape.rows),
        };
        Ok(RowView {
            values: self.values,
            len: self.shape.cols,
            offset,
            stride,
        })
    }

    /// Borrows one logical column from this view.
    pub fn column(self, index: usize) -> Result<ColumnView<'a>> {
        if index >= self.shape.cols {
            return Err(invalid_argument(format!(
                "column index {index} is out of bounds for {} cols",
                self.shape.cols
            )));
        }
        let (offset, stride) = match self.layout {
            MatrixLayout::RowMajor => (index, self.shape.cols),
            MatrixLayout::ColumnMajor => (index * self.shape.rows, 1),
        };
        Ok(ColumnView {
            values: self.values,
            len: self.shape.rows,
            offset,
            stride,
        })
    }

    /// Reads one value by logical row and column.
    pub fn get(self, row: usize, col: usize) -> Result<f32> {
        if row >= self.shape.rows || col >= self.shape.cols {
            return Err(invalid_argument("matrix indices are out of bounds"));
        }
        let index = match self.layout {
            MatrixLayout::RowMajor => row * self.shape.cols + col,
            MatrixLayout::ColumnMajor => col * self.shape.rows + row,
        };
        Ok(self.values[index])
    }

    /// Returns whether this view is square.
    pub fn is_square(&self) -> bool {
        self.shape.rows == self.shape.cols
    }

    /// Adds two matrices with equal shape and returns a row-major matrix.
    pub fn add(&self, right: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.elementwise_binary(right, |left, right| left + right)
    }

    /// Subtracts two matrices with equal shape and returns a row-major matrix.
    pub fn sub(&self, right: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.elementwise_binary(right, |left, right| left - right)
    }

    /// Scales every matrix value by a finite factor.
    pub fn scale(&self, factor: f32) -> Result<F32Matrix> {
        self.validate()?;
        if !factor.is_finite() {
            return Err(invalid_argument("matrix scale factor must be finite"));
        }
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                values.push(self.get(row, col)? * factor);
            }
        }
        F32Matrix::new(self.shape, values)
    }

    /// Computes the Frobenius norm.
    pub fn frobenius_norm(&self) -> Result<f32> {
        self.validate()?;
        let sum_squares = self.values.iter().map(|value| value * value).sum::<f32>();
        if !sum_squares.is_finite() {
            return Err(invalid_argument(
                "matrix Frobenius norm produced a non-finite value",
            ));
        }
        Ok(sum_squares.sqrt())
    }

    /// Computes the trace of a square matrix.
    pub fn trace(&self) -> Result<f32> {
        self.validate()?;
        self.require_square("matrix trace")?;
        let mut trace = 0.0;
        for index in 0..self.shape.rows {
            trace += self.get(index, index)?;
        }
        if !trace.is_finite() {
            return Err(invalid_argument("matrix trace produced a non-finite value"));
        }
        Ok(trace)
    }

    /// Multiplies this view by another matrix view.
    pub fn matmul(self, right: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.validate()?;
        right.validate()?;
        if self.shape.cols != right.shape.rows {
            return Err(invalid_argument("matrix multiply shapes are incompatible"));
        }
        let shape = MatrixShape::new(self.shape.rows, right.shape.cols)?;
        let mut values = vec![0.0; shape.element_count()?];
        for row in 0..self.shape.rows {
            for col in 0..right.shape.cols {
                let mut acc = 0.0;
                for inner in 0..self.shape.cols {
                    acc += self.get(row, inner)? * right.get(inner, col)?;
                }
                values[row * shape.cols + col] = acc;
            }
        }
        F32Matrix::new(shape, values)
    }

    /// Multiplies this view by a finite dense vector.
    pub fn matvec(self, vector: &[f32]) -> Result<DenseVector> {
        self.validate()?;
        if vector.len() != self.shape.cols {
            return Err(invalid_argument(
                "matrix/vector dimensions are incompatible",
            ));
        }
        if vector.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("matrix/vector values must be finite"));
        }
        let mut output = vec![0.0; self.shape.rows];
        for (row, output_value) in output.iter_mut().enumerate() {
            *output_value = dot(self.row(row)?.as_slice().as_slice(), vector)?;
        }
        DenseVector::new(output)
    }

    /// Sums each logical row.
    pub fn row_sums(self) -> Result<Vec<f32>> {
        (0..self.shape.rows)
            .map(|index| Ok(self.row(index)?.iter().sum()))
            .collect()
    }

    /// Sums each logical column.
    pub fn column_sums(self) -> Result<Vec<f32>> {
        (0..self.shape.cols)
            .map(|index| Ok(self.column(index)?.iter().sum()))
            .collect()
    }

    /// Averages each logical row.
    pub fn row_means(&self) -> Result<Vec<f32>> {
        self.validate()?;
        (0..self.shape.rows)
            .map(|index| Ok(self.row(index)?.iter().sum::<f32>() / self.shape.cols as f32))
            .collect()
    }

    /// Averages each logical column.
    pub fn column_means(&self) -> Result<Vec<f32>> {
        self.validate()?;
        (0..self.shape.cols)
            .map(|index| Ok(self.column(index)?.iter().sum::<f32>() / self.shape.rows as f32))
            .collect()
    }

    /// Returns this matrix diagonal values.
    pub fn diagonal(&self) -> Result<Vec<f32>> {
        self.validate()?;
        let len = self.shape.rows.min(self.shape.cols);
        (0..len).map(|index| self.get(index, index)).collect()
    }

    /// Computes `self * self^T`.
    pub fn gram_rows(&self) -> Result<F32Matrix> {
        self.matmul(&self.transpose())
    }

    /// Computes `self^T * self`.
    pub fn gram_columns(&self) -> Result<F32Matrix> {
        self.transpose().matmul(self)
    }

    /// Returns a row-major matrix with column means subtracted.
    pub fn center_columns(&self) -> Result<F32Matrix> {
        let means = self.column_means()?;
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for (col, mean) in means.iter().enumerate().take(self.shape.cols) {
                values.push(self.get(row, col)? - mean);
            }
        }
        F32Matrix::new(self.shape, values)
    }

    /// Returns a row-major matrix with row means subtracted.
    pub fn center_rows(&self) -> Result<F32Matrix> {
        let means = self.row_means()?;
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for (row, mean) in means.iter().enumerate().take(self.shape.rows) {
            for col in 0..self.shape.cols {
                values.push(self.get(row, col)? - mean);
            }
        }
        F32Matrix::new(self.shape, values)
    }

    /// Returns a row-major matrix whose rows have unit L2 norm.
    pub fn l2_normalize_rows(self) -> Result<F32Matrix> {
        let mut values = Vec::with_capacity(self.values.len());
        for row in 0..self.shape.rows {
            let row_view = self.row(row)?;
            let vector = DenseVector::new(row_view.as_slice())?.l2_normalized()?;
            values.extend(vector.into_values());
        }
        F32Matrix::new(self.shape, values)
    }

    /// Returns a row-major matrix whose columns have unit L2 norm.
    pub fn l2_normalize_columns(self) -> Result<F32Matrix> {
        let mut values = vec![0.0; self.shape.element_count()?];
        let normalized = (0..self.shape.cols)
            .map(|col| DenseVector::new(self.column(col)?.as_slice())?.l2_normalized())
            .collect::<Result<Vec<_>>>()?;
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                values[row * self.shape.cols + col] = normalized[col].as_slice()[row];
            }
        }
        F32Matrix::new(self.shape, values)
    }

    /// Computes the dot product for every pair of rows in two matrices.
    pub fn pairwise_row_dot(self, right: &F32MatrixView<'_>) -> Result<F32Matrix> {
        if self.shape.cols != right.shape.cols {
            return Err(invalid_argument(
                "row pairwise dot requires equal column counts",
            ));
        }
        let shape = MatrixShape::new(self.shape.rows, right.shape.rows)?;
        let mut values = vec![0.0; shape.element_count()?];
        for row in 0..self.shape.rows {
            let left = self.row(row)?;
            for other_row in 0..right.shape.rows {
                let right_row = right.row(other_row)?;
                values[row * shape.cols + other_row] =
                    dot(left.as_slice().as_slice(), right_row.as_slice().as_slice())?;
            }
        }
        F32Matrix::new(shape, values)
    }

    /// Computes cosine similarity for every pair of rows in two matrices.
    pub fn pairwise_row_cosine(self, right: &F32MatrixView<'_>) -> Result<F32Matrix> {
        if self.shape.cols != right.shape.cols {
            return Err(invalid_argument(
                "row pairwise cosine requires equal column counts",
            ));
        }
        let shape = MatrixShape::new(self.shape.rows, right.shape.rows)?;
        let mut values = vec![0.0; shape.element_count()?];
        for row in 0..self.shape.rows {
            let left = self.row(row)?;
            for other_row in 0..right.shape.rows {
                let right_row = right.row(other_row)?;
                values[row * shape.cols + other_row] =
                    cosine_similarity(left.as_slice().as_slice(), right_row.as_slice().as_slice())?;
            }
        }
        F32Matrix::new(shape, values)
    }

    /// Decomposes this square matrix using LU factorization with partial pivoting.
    pub fn lu_decompose(&self) -> Result<LuDecomposition> {
        self.validate()?;
        self.require_square("LU decomposition")?;
        backend::pure::lu_decompose(*self)
    }

    /// Decomposes this symmetric positive definite matrix using Cholesky factorization.
    pub fn cholesky_decompose(&self) -> Result<CholeskyDecomposition> {
        self.validate()?;
        self.require_square("Cholesky decomposition")?;
        let size = self.shape.rows;
        let mut lower = vec![0.0; self.shape.element_count()?];
        for row in 0..size {
            for col in 0..=row {
                let mut sum = self.get(row, col)?;
                for inner in 0..col {
                    sum -= lower[row * size + inner] * lower[col * size + inner];
                }
                if row == col {
                    if sum <= f32::EPSILON {
                        return Err(invalid_argument(
                            "Cholesky decomposition requires a positive definite matrix",
                        ));
                    }
                    lower[row * size + col] = sum.sqrt();
                } else {
                    let pivot = lower[col * size + col];
                    if pivot.abs() <= f32::EPSILON {
                        return Err(invalid_argument(
                            "Cholesky decomposition encountered a zero pivot",
                        ));
                    }
                    lower[row * size + col] = sum / pivot;
                }
            }
        }
        Ok(CholeskyDecomposition {
            lower: F32Matrix::new(self.shape, lower)?,
        })
    }

    /// Decomposes this matrix with modified Gram-Schmidt QR factorization.
    pub fn qr_decompose(&self) -> Result<QrDecomposition> {
        self.validate()?;
        if self.shape.rows < self.shape.cols {
            return Err(invalid_argument("QR decomposition requires rows >= cols"));
        }
        let rows = self.shape.rows;
        let cols = self.shape.cols;
        let mut q_columns = Vec::<Vec<f32>>::with_capacity(cols);
        let mut r_values = vec![0.0; cols * cols];
        for col in 0..cols {
            let mut v = self.column(col)?.as_slice();
            for prev in 0..col {
                let q = &q_columns[prev];
                let r = dot(q, &v)?;
                r_values[prev * cols + col] = r;
                for row in 0..rows {
                    v[row] -= r * q[row];
                }
            }
            let norm = l2_norm(&v)?;
            if norm <= f32::EPSILON {
                return Err(invalid_argument(
                    "QR decomposition requires full column rank",
                ));
            }
            r_values[col * cols + col] = norm;
            for value in &mut v {
                *value /= norm;
            }
            q_columns.push(v);
        }
        let mut q_values = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                q_values[row * cols + col] = q_columns[col][row];
            }
        }
        Ok(QrDecomposition {
            q: F32Matrix::new(MatrixShape::new(rows, cols)?, q_values)?,
            r: F32Matrix::new(MatrixShape::new(cols, cols)?, r_values)?,
        })
    }

    /// Returns simple determinant and diagonal magnitude diagnostics.
    pub fn condition_estimate(&self) -> Result<MatrixConditionEstimate> {
        self.validate()?;
        self.require_square("condition estimate")?;
        let decomposition = self.lu_decompose()?;
        let diagonal = decomposition.upper_matrix()?.diagonal()?;
        let mut diagonal_min_abs = f32::INFINITY;
        let mut diagonal_max_abs = 0.0_f32;
        for value in diagonal {
            diagonal_min_abs = diagonal_min_abs.min(value.abs());
            diagonal_max_abs = diagonal_max_abs.max(value.abs());
        }
        Ok(MatrixConditionEstimate {
            determinant: decomposition.determinant()?,
            diagonal_min_abs,
            diagonal_max_abs,
        })
    }

    /// Returns a deterministic modified Gram-Schmidt rank estimate.
    pub fn rank_estimate(&self, tolerance: f32) -> Result<usize> {
        self.validate()?;
        let tolerance = resolve_rank_tolerance(self, tolerance)?;
        let rows = self.shape.rows;
        let cols = self.shape.cols;
        let mut basis = Vec::<Vec<f32>>::with_capacity(rows.min(cols));
        for col in 0..cols {
            let mut residual = self.column(col)?.as_slice();
            for q in &basis {
                let projection = dot(q, &residual)?;
                for row in 0..rows {
                    residual[row] -= projection * q[row];
                }
            }
            let norm = l2_norm(&residual)?;
            if norm > tolerance {
                for value in &mut residual {
                    *value /= norm;
                }
                basis.push(residual);
            }
        }
        Ok(basis.len())
    }

    /// Fits a full-column-rank least-squares model with QR factorization.
    pub fn least_squares(&self, target: &[f32], tolerance: f32) -> Result<LeastSquaresSolution> {
        self.validate()?;
        if target.len() != self.shape.rows {
            return Err(invalid_argument(
                "least-squares target length must match matrix rows",
            ));
        }
        if target.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "least-squares target values must be finite",
            ));
        }
        let rank = self.rank_estimate(tolerance)?;
        if rank < self.shape.cols {
            return Err(invalid_argument(
                "least-squares requires a full-column-rank matrix",
            ));
        }
        let qr = self.qr_decompose()?;
        let coefficients = qr.solve_least_squares(target)?;
        let fitted = self.matvec(&coefficients)?.into_values();
        let residuals = target
            .iter()
            .zip(&fitted)
            .map(|(actual, fitted)| actual - fitted)
            .collect::<Vec<_>>();
        let residual_sum_squares = residuals.iter().map(|value| value * value).sum::<f32>();
        if !residual_sum_squares.is_finite() {
            return Err(invalid_argument(
                "least-squares residual sum of squares is non-finite",
            ));
        }
        Ok(LeastSquaresSolution {
            coefficients,
            fitted,
            residuals,
            residual_sum_squares,
            rank,
            observations: self.shape.rows,
            predictors: self.shape.cols,
        })
    }

    /// Returns deterministic linear solve diagnostics.
    pub fn solve_diagnostics(&self, tolerance: f32) -> Result<LinearSolveDiagnostics> {
        self.validate()?;
        let rank = self.rank_estimate(tolerance)?;
        let (determinant, pivot_ratio) = if self.is_square() {
            let condition = self.condition_estimate()?;
            (Some(condition.determinant), condition.pivot_ratio())
        } else {
            (None, None)
        };
        Ok(LinearSolveDiagnostics {
            determinant,
            rank,
            pivot_ratio,
            observations: self.shape.rows,
            predictors: self.shape.cols,
        })
    }

    /// Computes this square matrix determinant through LU decomposition.
    pub fn determinant(&self) -> Result<f32> {
        self.lu_decompose()?.determinant()
    }

    /// Solves `A x = b` for a finite vector `b`.
    pub fn solve_vector(&self, b: &[f32]) -> Result<Vec<f32>> {
        self.lu_decompose()?.solve_vector(b)
    }

    /// Solves `A X = B` for matrix `B`.
    pub fn solve_matrix(&self, b: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.lu_decompose()?.solve_matrix(b)
    }

    /// Computes this square matrix inverse.
    pub fn inverse(&self) -> Result<F32Matrix> {
        let identity = F32Matrix::identity(self.shape.rows)?;
        self.solve_matrix(&identity.as_view())
    }

    /// Verifies shape/value count agreement and rejects non-finite values.
    pub fn validate(self) -> Result<()> {
        self.shape.validate()?;
        if self.values.len() != self.shape.element_count()? {
            return Err(invalid_argument(format!(
                "matrix shape expects {} values but matrix view has {}",
                self.shape.element_count()?,
                self.values.len()
            )));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("matrix values must be finite"));
        }
        Ok(())
    }

    /// Copies this view into an owned row-major matrix.
    pub fn into_owned(self) -> Result<F32Matrix> {
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                values.push(self.get(row, col)?);
            }
        }
        F32Matrix::new(self.shape, values)
    }

    fn elementwise_binary(
        &self,
        right: &F32MatrixView<'_>,
        op: impl Fn(f32, f32) -> f32,
    ) -> Result<F32Matrix> {
        self.validate()?;
        right.validate()?;
        if self.shape != right.shape {
            return Err(invalid_argument("matrix shapes are incompatible"));
        }
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                values.push(op(self.get(row, col)?, right.get(row, col)?));
            }
        }
        F32Matrix::new(self.shape, values)
    }

    fn require_square(&self, operation: &str) -> Result<()> {
        if !self.is_square() {
            return Err(invalid_argument(format!(
                "{operation} requires a square matrix"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Matrix triangle selector for decomposed matrices.
pub enum MatrixTriangle {
    /// Unit diagonal lower triangular factor.
    Lower,
    /// Upper triangular factor.
    Upper,
}

#[derive(Debug, Clone, PartialEq)]
/// LU decomposition with partial pivoting for a finite square `f32` matrix.
pub struct LuDecomposition {
    shape: MatrixShape,
    lu: Vec<f32>,
    pivots: Vec<usize>,
    swap_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
/// Cholesky decomposition `A = L * L^T`.
pub struct CholeskyDecomposition {
    /// Lower triangular factor.
    pub lower: F32Matrix,
}

#[derive(Debug, Clone, PartialEq)]
/// Thin QR decomposition `A = Q * R`.
pub struct QrDecomposition {
    /// Matrix with orthonormal columns.
    pub q: F32Matrix,
    /// Upper triangular factor.
    pub r: F32Matrix,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Simple square-matrix determinant and diagonal magnitude diagnostics.
pub struct MatrixConditionEstimate {
    /// Determinant computed through LU decomposition.
    pub determinant: f32,
    /// Smallest absolute upper-triangular diagonal entry.
    pub diagonal_min_abs: f32,
    /// Largest absolute upper-triangular diagonal entry.
    pub diagonal_max_abs: f32,
}

impl LuDecomposition {
    pub(crate) fn new(
        shape: MatrixShape,
        lu: Vec<f32>,
        pivots: Vec<usize>,
        swap_count: usize,
    ) -> Self {
        Self {
            shape,
            lu,
            pivots,
            swap_count,
        }
    }

    /// Returns the square decomposition shape.
    pub fn shape(&self) -> MatrixShape {
        self.shape
    }

    /// Returns the row permutation after partial pivoting.
    pub fn pivots(&self) -> &[usize] {
        &self.pivots
    }

    /// Returns the number of pivot row swaps performed.
    pub fn swap_count(&self) -> usize {
        self.swap_count
    }

    /// Computes the determinant from the upper-triangular diagonal and swap parity.
    pub fn determinant(&self) -> Result<f32> {
        self.validate()?;
        let size = self.shape.rows;
        let mut determinant = if self.swap_count.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        for index in 0..size {
            determinant *= self.lu[index * size + index];
        }
        if !determinant.is_finite() {
            return Err(invalid_argument(
                "matrix determinant produced a non-finite value",
            ));
        }
        Ok(determinant)
    }

    /// Solves `A x = b` for a finite vector `b`.
    pub fn solve_vector(&self, b: &[f32]) -> Result<Vec<f32>> {
        self.validate()?;
        let size = self.shape.rows;
        if b.len() != size {
            return Err(invalid_argument(
                "linear solve vector length is incompatible",
            ));
        }
        if b.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("linear solve values must be finite"));
        }

        let mut y = vec![0.0; size];
        for row in 0..size {
            let mut sum = b[self.pivots[row]];
            for (col, value) in y.iter().enumerate().take(row) {
                sum -= self.lu[row * size + col] * value;
            }
            y[row] = sum;
        }

        let tolerance = backend::pure::pivot_tolerance(&self.lu);
        let mut x = vec![0.0; size];
        for row in (0..size).rev() {
            let mut sum = y[row];
            for (col, value) in x.iter().enumerate().take(size).skip(row + 1) {
                sum -= self.lu[row * size + col] * value;
            }
            let pivot = self.lu[row * size + row];
            if pivot.abs() <= tolerance {
                return Err(invalid_argument("matrix is singular or near-singular"));
            }
            x[row] = sum / pivot;
        }
        if x.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("linear solve produced non-finite values"));
        }
        Ok(x)
    }

    /// Solves `A X = B` for a finite matrix `B`.
    pub fn solve_matrix(&self, b: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.validate()?;
        b.validate()?;
        if b.shape.rows != self.shape.rows {
            return Err(invalid_argument(
                "linear solve matrix rows are incompatible",
            ));
        }
        let shape = MatrixShape::new(self.shape.rows, b.shape.cols)?;
        let mut values = vec![0.0; shape.element_count()?];
        for col in 0..b.shape.cols {
            let rhs = b.column(col)?.as_slice();
            let solution = self.solve_vector(&rhs)?;
            for row in 0..shape.rows {
                values[row * shape.cols + col] = solution[row];
            }
        }
        F32Matrix::new(shape, values)
    }

    /// Extracts the unit diagonal lower triangular factor.
    pub fn lower_matrix(&self) -> Result<F32Matrix> {
        self.triangle_matrix(MatrixTriangle::Lower)
    }

    /// Extracts the upper triangular factor.
    pub fn upper_matrix(&self) -> Result<F32Matrix> {
        self.triangle_matrix(MatrixTriangle::Upper)
    }

    fn triangle_matrix(&self, triangle: MatrixTriangle) -> Result<F32Matrix> {
        self.validate()?;
        let size = self.shape.rows;
        let mut values = vec![0.0; self.shape.element_count()?];
        for row in 0..size {
            for col in 0..size {
                values[row * size + col] = match triangle {
                    MatrixTriangle::Lower if row > col => self.lu[row * size + col],
                    MatrixTriangle::Lower if row == col => 1.0,
                    MatrixTriangle::Upper if row <= col => self.lu[row * size + col],
                    _ => 0.0,
                };
            }
        }
        F32Matrix::new(self.shape, values)
    }

    fn validate(&self) -> Result<()> {
        self.shape.validate()?;
        if self.shape.rows != self.shape.cols {
            return Err(invalid_argument(
                "LU decomposition requires a square matrix",
            ));
        }
        if self.lu.len() != self.shape.element_count()? {
            return Err(invalid_argument(
                "LU decomposition values do not match shape",
            ));
        }
        if self.pivots.len() != self.shape.rows {
            return Err(invalid_argument(
                "LU decomposition pivots do not match shape",
            ));
        }
        if self.lu.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("LU decomposition values must be finite"));
        }
        Ok(())
    }
}

impl CholeskyDecomposition {
    /// Solves `A x = b` using `A = L * L^T`.
    pub fn solve_vector(&self, b: &[f32]) -> Result<Vec<f32>> {
        self.lower.validate()?;
        let shape = self.lower.shape();
        if shape.rows != shape.cols {
            return Err(invalid_argument(
                "Cholesky solve requires a square lower factor",
            ));
        }
        if b.len() != shape.rows {
            return Err(invalid_argument(
                "Cholesky solve vector length is incompatible",
            ));
        }
        if b.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("Cholesky solve values must be finite"));
        }
        let size = shape.rows;
        let lower = self.lower.as_view();
        let mut y = vec![0.0; size];
        for row in 0..size {
            let mut sum = b[row];
            for (col, value) in y.iter().enumerate().take(row) {
                sum -= lower.get(row, col)? * value;
            }
            let pivot = lower.get(row, row)?;
            if pivot <= f32::EPSILON {
                return Err(invalid_argument(
                    "Cholesky solve requires positive diagonal pivots",
                ));
            }
            y[row] = sum / pivot;
        }
        let mut x = vec![0.0; size];
        for row in (0..size).rev() {
            let mut sum = y[row];
            for (col, value) in x.iter().enumerate().take(size).skip(row + 1) {
                sum -= lower.get(col, row)? * value;
            }
            let pivot = lower.get(row, row)?;
            if pivot <= f32::EPSILON {
                return Err(invalid_argument(
                    "Cholesky solve requires positive diagonal pivots",
                ));
            }
            x[row] = sum / pivot;
        }
        if x.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "Cholesky solve produced non-finite values",
            ));
        }
        Ok(x)
    }

    /// Solves `A X = B` using `A = L * L^T`.
    pub fn solve_matrix(&self, b: &F32MatrixView<'_>) -> Result<F32Matrix> {
        self.lower.validate()?;
        b.validate()?;
        let shape = self.lower.shape();
        if b.shape.rows != shape.rows {
            return Err(invalid_argument(
                "Cholesky solve matrix rows are incompatible",
            ));
        }
        let output_shape = MatrixShape::new(shape.rows, b.shape.cols)?;
        let mut values = vec![0.0; output_shape.element_count()?];
        for col in 0..b.shape.cols {
            let rhs = b.column(col)?.as_slice();
            let solution = self.solve_vector(&rhs)?;
            for row in 0..output_shape.rows {
                values[row * output_shape.cols + col] = solution[row];
            }
        }
        F32Matrix::new(output_shape, values)
    }

    /// Computes `ln(det(A))` from `A = L * L^T`.
    pub fn log_determinant(&self) -> Result<f32> {
        self.lower.validate()?;
        let shape = self.lower.shape();
        if shape.rows != shape.cols {
            return Err(invalid_argument(
                "Cholesky log determinant requires a square lower factor",
            ));
        }
        let mut sum = 0.0;
        for index in 0..shape.rows {
            let diagonal = self.lower.as_view().get(index, index)?;
            if diagonal <= 0.0 {
                return Err(invalid_argument(
                    "Cholesky log determinant requires positive diagonal entries",
                ));
            }
            sum += diagonal.ln();
        }
        let log_determinant = 2.0 * sum;
        if !log_determinant.is_finite() {
            return Err(invalid_argument(
                "Cholesky log determinant produced a non-finite value",
            ));
        }
        Ok(log_determinant)
    }
}

impl QrDecomposition {
    /// Solves a full-column-rank least-squares target with this QR factorization.
    pub fn solve_least_squares(&self, target: &[f32]) -> Result<Vec<f32>> {
        self.q.validate()?;
        self.r.validate()?;
        if target.len() != self.q.shape().rows {
            return Err(invalid_argument(
                "QR least-squares target length must match Q rows",
            ));
        }
        if target.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "QR least-squares target values must be finite",
            ));
        }
        if self.r.shape().rows != self.r.shape().cols || self.r.shape().cols != self.q.shape().cols
        {
            return Err(invalid_argument("QR least-squares shapes are incompatible"));
        }
        let q_t_y = self.q.as_view().transpose().matvec(target)?.into_values();
        let cols = self.r.shape().cols;
        let mut coefficients = vec![0.0; cols];
        for row in (0..cols).rev() {
            let mut sum = q_t_y[row];
            for (col, coefficient) in coefficients.iter().enumerate().take(cols).skip(row + 1) {
                sum -= self.r.as_view().get(row, col)? * coefficient;
            }
            let pivot = self.r.as_view().get(row, row)?;
            if pivot.abs() <= f32::EPSILON {
                return Err(invalid_argument(
                    "QR least-squares encountered a zero pivot",
                ));
            }
            coefficients[row] = sum / pivot;
        }
        if coefficients.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument(
                "QR least-squares produced non-finite coefficients",
            ));
        }
        Ok(coefficients)
    }
}

impl MatrixConditionEstimate {
    /// Returns `min_abs_pivot / max_abs_pivot` when a non-zero pivot range is available.
    pub fn pivot_ratio(&self) -> Option<f32> {
        if self.diagonal_max_abs > 0.0 && self.diagonal_min_abs.is_finite() {
            Some(self.diagonal_min_abs / self.diagonal_max_abs)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Strided borrowed view over one logical matrix row.
pub struct RowView<'a> {
    values: &'a [f32],
    len: usize,
    offset: usize,
    stride: usize,
}

impl<'a> RowView<'a> {
    /// Returns the number of values in the row.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the row has no values.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterates over row values in logical column order.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        (0..self.len).map(|index| self.values[self.offset + index * self.stride])
    }

    /// Collects the possibly strided row into a contiguous vector.
    pub fn as_slice(&self) -> Vec<f32> {
        self.iter().collect()
    }

    /// Copies the row into a validated dense vector.
    pub fn to_dense_vector(&self) -> Result<DenseVector> {
        DenseVector::new(self.as_slice())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Strided borrowed view over one logical matrix column.
pub struct ColumnView<'a> {
    values: &'a [f32],
    len: usize,
    offset: usize,
    stride: usize,
}

impl<'a> ColumnView<'a> {
    /// Returns the number of values in the column.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the column has no values.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterates over column values in logical row order.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        (0..self.len).map(|index| self.values[self.offset + index * self.stride])
    }

    /// Collects the possibly strided column into a contiguous vector.
    pub fn as_slice(&self) -> Vec<f32> {
        self.iter().collect()
    }

    /// Copies the column into a validated dense vector.
    pub fn to_dense_vector(&self) -> Result<DenseVector> {
        DenseVector::new(self.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Finite 2D convolution kernel stored in row-major order.
pub struct Kernel2d {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl Kernel2d {
    /// Creates a kernel with non-zero dimensions and matching finite values.
    pub fn new(width: usize, height: usize, values: impl Into<Vec<f32>>) -> Result<Self> {
        let kernel = Self {
            width,
            height,
            values: values.into(),
        };
        kernel.validate()?;
        Ok(kernel)
    }

    /// Returns the number of columns in the kernel.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the number of rows in the kernel.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Borrows row-major kernel values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Verifies dimensions, value count, and finite values.
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(invalid_argument(
                "kernel width and height must be greater than zero",
            ));
        }
        if self.values.len()
            != self
                .width
                .checked_mul(self.height)
                .ok_or_else(|| invalid_argument("kernel element count overflowed usize"))?
        {
            return Err(invalid_argument("kernel dimensions do not match values"));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("kernel values must be finite"));
        }
        Ok(())
    }

    /// Creates a 3x3 identity kernel with a `1.0` center coefficient.
    pub fn identity_3x3() -> Self {
        Self {
            width: 3,
            height: 3,
            values: vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Creates a standard 3x3 sharpen kernel.
    pub fn sharpen_3x3() -> Self {
        Self {
            width: 3,
            height: 3,
            values: vec![0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
        }
    }

    /// Creates a standard 3x3 edge-detection kernel.
    pub fn edge_3x3() -> Self {
        Self {
            width: 3,
            height: 3,
            values: vec![-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
        }
    }

    /// Creates an unnormalized 3x3 box blur kernel.
    pub fn blur_3x3() -> Self {
        Self {
            width: 3,
            height: 3,
            values: vec![1.0; 9],
        }
    }

    /// Copies a 3x3 kernel into a fixed-size row-major array.
    pub fn as_array_3x3(&self) -> Result<[f32; 9]> {
        if self.width != 3 || self.height != 3 {
            return Err(invalid_argument("kernel is not 3x3"));
        }
        Ok(self
            .values
            .clone()
            .try_into()
            .expect("kernel length is validated"))
    }
}

impl From<[f32; 9]> for Kernel2d {
    fn from(value: [f32; 9]) -> Self {
        Self {
            width: 3,
            height: 3,
            values: value.to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Finite 1D convolution kernel.
pub struct Kernel1d {
    values: Vec<f32>,
}

impl Kernel1d {
    /// Creates a non-empty kernel and rejects non-finite values.
    pub fn new(values: impl Into<Vec<f32>>) -> Result<Self> {
        let kernel = Self {
            values: values.into(),
        };
        kernel.validate()?;
        Ok(kernel)
    }

    /// Borrows kernel coefficients in storage order.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Verifies that the kernel is non-empty and finite.
    pub fn validate(&self) -> Result<()> {
        if self.values.is_empty() {
            return Err(invalid_argument("1D kernel must not be empty"));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("1D kernel values must be finite"));
        }
        Ok(())
    }
}

impl TryFrom<&F32Tensor> for F32Matrix {
    type Error = DetectError;

    fn try_from(value: &F32Tensor) -> Result<Self> {
        if value.shape().rank() != 2 {
            return Err(invalid_argument(
                "tensor-to-matrix conversion requires rank 2",
            ));
        }
        let dims = value.shape().dimensions();
        Self::new(MatrixShape::new(dims[0], dims[1])?, value.values().to_vec())
    }
}

impl TryFrom<F32Tensor> for F32Matrix {
    type Error = DetectError;

    fn try_from(value: F32Tensor) -> Result<Self> {
        if value.shape().rank() != 2 {
            return Err(invalid_argument(
                "tensor-to-matrix conversion requires rank 2",
            ));
        }
        let dims = value.shape().dimensions().to_vec();
        Self::new(MatrixShape::new(dims[0], dims[1])?, value.into_values())
    }
}

impl<'a> TryFrom<F32TensorView<'a>> for F32MatrixView<'a> {
    type Error = DetectError;

    fn try_from(value: F32TensorView<'a>) -> Result<Self> {
        if value.shape().rank() != 2 {
            return Err(invalid_argument(
                "tensor view to matrix view conversion requires rank 2",
            ));
        }
        let dims = value.shape().dimensions();
        Self::new(MatrixShape::new(dims[0], dims[1])?, value.values())
    }
}

impl TryFrom<&F32Matrix> for F32Tensor {
    type Error = DetectError;

    fn try_from(value: &F32Matrix) -> Result<Self> {
        F32Tensor::new(
            TensorShape::new([value.shape.rows, value.shape.cols])?,
            value.values.clone(),
        )
    }
}

impl TryFrom<RowView<'_>> for DenseVector {
    type Error = DetectError;

    fn try_from(value: RowView<'_>) -> Result<Self> {
        DenseVector::new(value.as_slice())
    }
}

impl TryFrom<ColumnView<'_>> for DenseVector {
    type Error = DetectError;

    fn try_from(value: ColumnView<'_>) -> Result<Self> {
        DenseVector::new(value.as_slice())
    }
}

impl TryFrom<&DenseVector> for F32Matrix {
    type Error = DetectError;

    fn try_from(value: &DenseVector) -> Result<Self> {
        F32Matrix::new(
            MatrixShape::new(1, value.dimensions())?,
            value.as_slice().to_vec(),
        )
    }
}

fn resolve_rank_tolerance(matrix: &F32MatrixView<'_>, tolerance: f32) -> Result<f32> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(invalid_argument(
            "rank tolerance must be finite and non-negative",
        ));
    }
    if tolerance > 0.0 {
        return Ok(tolerance);
    }
    let mut max_column_l2_norm = 0.0_f32;
    for col in 0..matrix.shape.cols {
        max_column_l2_norm = max_column_l2_norm.max(l2_norm(&matrix.column(col)?.as_slice())?);
    }
    Ok(f32::EPSILON * matrix.shape.rows.max(matrix.shape.cols) as f32 * max_column_l2_norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f32, right: f32) {
        assert!((left - right).abs() < 1.0e-4, "expected {left} ≈ {right}");
    }

    #[test]
    fn validates_shapes_and_stride_backed_views() {
        assert!(MatrixShape::new(0, 2).is_err());
        let matrix = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();
        assert_eq!(
            matrix.transpose_view().row(0).unwrap().as_slice(),
            vec![1.0, 3.0]
        );
    }

    #[test]
    fn matrix_multiply_matches_expected_values() {
        let left = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();
        let right = F32Matrix::from_rows([[2.0, 0.0], [1.0, 2.0]]).unwrap();
        let product = left.matmul(&right.as_view()).unwrap();
        assert_eq!(product.values(), &[4.0, 4.0, 10.0, 8.0]);
    }

    #[test]
    fn pairwise_row_cosine_and_kernel_helpers_work() {
        let matrix = F32Matrix::from_rows([[1.0, 0.0], [0.0, 1.0]]).unwrap();
        let cosine = matrix.pairwise_row_cosine(&matrix.as_view()).unwrap();
        assert_eq!(cosine.values(), &[1.0, 0.0, 0.0, 1.0]);
        assert_eq!(Kernel2d::sharpen_3x3().as_array_3x3().unwrap()[4], 5.0);
    }

    #[test]
    fn tensor_and_vector_bridges_round_trip() {
        let tensor = F32Tensor::from_dims([2, 2], vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let matrix = F32Matrix::try_from(&tensor).unwrap();
        assert_eq!(
            matrix.row(1).unwrap().to_dense_vector().unwrap().as_slice(),
            &[3.0, 4.0]
        );
        let tensor_round_trip = F32Tensor::try_from(&matrix).unwrap();
        assert_eq!(tensor_round_trip.values(), tensor.values());
    }

    #[test]
    fn identity_matrix_has_unit_diagonal() {
        let matrix = F32Matrix::identity(3).unwrap();

        assert_eq!(
            matrix.values(),
            &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn transpose_owned_round_trip_restores_original() {
        let matrix = F32Matrix::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]).unwrap();
        let transposed = matrix.as_view().transpose_owned().unwrap();
        assert_eq!(transposed.shape(), MatrixShape { rows: 3, cols: 2 });
        assert_eq!(transposed.values(), &[1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);

        let round_trip = transposed.as_view().transpose_owned().unwrap();
        assert_eq!(round_trip, matrix);
    }

    #[test]
    fn add_sub_and_scale_produce_expected_values() {
        let left = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();
        let right = F32Matrix::from_rows([[5.0, 6.0], [7.0, 8.0]]).unwrap();

        let added = left.as_view().add(&right.as_view()).unwrap();
        assert_eq!(added.values(), &[6.0, 8.0, 10.0, 12.0]);

        let subtracted = right.as_view().sub(&left.as_view()).unwrap();
        assert_eq!(subtracted.values(), &[4.0, 4.0, 4.0, 4.0]);

        let scaled = left.as_view().scale(0.5).unwrap();
        assert_eq!(scaled.values(), &[0.5, 1.0, 1.5, 2.0]);
        assert!(left.as_view().scale(f32::NAN).is_err());
    }

    #[test]
    fn frobenius_norm_and_means_are_correct() {
        let matrix = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();

        assert_close(matrix.as_view().frobenius_norm().unwrap(), 30.0_f32.sqrt());
        assert_eq!(matrix.as_view().row_means().unwrap(), vec![1.5, 3.5]);
        assert_eq!(matrix.as_view().column_means().unwrap(), vec![2.0, 3.0]);
    }

    #[test]
    fn trace_requires_square_matrix() {
        let matrix = F32Matrix::from_rows([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]).unwrap();

        assert!(matrix.as_view().trace().is_err());
    }

    #[test]
    fn lu_determinant_works_for_small_matrices() {
        let matrix_2x2 = F32Matrix::from_rows([[2.0, 1.0], [1.0, 3.0]]).unwrap();
        assert_close(matrix_2x2.determinant().unwrap(), 5.0);

        let matrix_3x3 =
            F32Matrix::from_rows([[6.0, 1.0, 1.0], [4.0, -2.0, 5.0], [2.0, 8.0, 7.0]]).unwrap();
        assert_close(matrix_3x3.determinant().unwrap(), -306.0);
    }

    #[test]
    fn solving_vector_returns_expected_values() {
        let matrix = F32Matrix::from_rows([[2.0, 1.0], [1.0, 3.0]]).unwrap();
        let solution = matrix.solve_vector(&[1.0, 2.0]).unwrap();

        assert_close(solution[0], 0.2);
        assert_close(solution[1], 0.6);
    }

    #[test]
    fn matrix_inverse_multiplies_to_identity() {
        let matrix = F32Matrix::from_rows([[4.0, 7.0], [2.0, 6.0]]).unwrap();
        let inverse = matrix.inverse().unwrap();
        let product = matrix.matmul(&inverse.as_view()).unwrap();

        assert_close(product.as_view().get(0, 0).unwrap(), 1.0);
        assert_close(product.as_view().get(0, 1).unwrap(), 0.0);
        assert_close(product.as_view().get(1, 0).unwrap(), 0.0);
        assert_close(product.as_view().get(1, 1).unwrap(), 1.0);
    }

    #[test]
    fn singular_matrix_returns_error() {
        let matrix = F32Matrix::from_rows([[1.0, 2.0], [2.0, 4.0]]).unwrap();

        assert!(matrix.lu_decompose().is_err());
        assert!(matrix.inverse().is_err());
    }

    #[test]
    fn pivoting_handles_zero_initial_pivot() {
        let matrix = F32Matrix::from_rows([[0.0, 2.0], [1.0, 3.0]]).unwrap();
        let decomposition = matrix.lu_decompose().unwrap();
        let solution = decomposition.solve_vector(&[4.0, 5.0]).unwrap();

        assert_eq!(decomposition.swap_count(), 1);
        assert_close(decomposition.determinant().unwrap(), -2.0);
        assert_close(solution[0], -1.0);
        assert_close(solution[1], 2.0);
    }

    #[test]
    fn diagonal_centering_and_gram_helpers_work() {
        let diag = F32Matrix::from_diag(&[2.0, 3.0]).unwrap();
        assert_eq!(diag.values(), &[2.0, 0.0, 0.0, 3.0]);
        assert_eq!(diag.diagonal().unwrap(), vec![2.0, 3.0]);

        let matrix = F32Matrix::from_rows([[1.0, 2.0], [3.0, 4.0]]).unwrap();
        assert_eq!(
            matrix.gram_rows().unwrap().values(),
            &[5.0, 11.0, 11.0, 25.0]
        );
        assert_eq!(
            matrix.gram_columns().unwrap().values(),
            &[10.0, 14.0, 14.0, 20.0]
        );
        assert_eq!(
            matrix.center_columns().unwrap().values(),
            &[-1.0, -1.0, 1.0, 1.0]
        );
        assert_eq!(
            matrix.center_rows().unwrap().values(),
            &[-0.5, 0.5, -0.5, 0.5]
        );
    }

    #[test]
    fn cholesky_reconstructs_spd_matrix() {
        let matrix = F32Matrix::from_rows([[4.0, 2.0], [2.0, 3.0]]).unwrap();
        let decomposition = matrix.cholesky_decompose().unwrap();
        let reconstructed = decomposition
            .lower
            .matmul(&decomposition.lower.transpose_view())
            .unwrap();
        for (actual, expected) in reconstructed.values().iter().zip(matrix.values()) {
            assert_close(*actual, *expected);
        }
    }

    #[test]
    fn qr_reconstructs_and_q_is_orthogonal() {
        let matrix = F32Matrix::from_rows([[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
        let decomposition = matrix.qr_decompose().unwrap();
        let reconstructed = decomposition.q.matmul(&decomposition.r.as_view()).unwrap();
        for (actual, expected) in reconstructed.values().iter().zip(matrix.values()) {
            assert_close(*actual, *expected);
        }
        let q_t_q = decomposition.q.as_view().gram_columns().unwrap();
        assert_close(q_t_q.as_view().get(0, 0).unwrap(), 1.0);
        assert_close(q_t_q.as_view().get(1, 1).unwrap(), 1.0);
        assert_close(q_t_q.as_view().get(0, 1).unwrap(), 0.0);
    }

    #[test]
    fn qr_least_squares_recovers_exact_coefficients() {
        let design =
            F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0], [1.0, 4.0]]).unwrap();
        let solution = design.least_squares(&[3.0, 5.0, 7.0, 9.0], 0.0).unwrap();

        assert_close(solution.coefficients[0], 1.0);
        assert_close(solution.coefficients[1], 2.0);
        assert!(solution.residual_sum_squares < 1.0e-8);
        assert_eq!(solution.rank, 2);
        assert_eq!(solution.observations, 4);
        assert_eq!(solution.predictors, 2);
    }

    #[test]
    fn least_squares_rejects_rank_deficient_design() {
        let design =
            F32Matrix::from_rows([[1.0, 2.0], [2.0, 4.0], [3.0, 6.0], [4.0, 8.0]]).unwrap();

        assert!(design.least_squares(&[1.0, 2.0, 3.0, 4.0], 0.0).is_err());
    }

    #[test]
    fn rank_estimate_handles_full_and_dependent_columns() {
        let full_rank = F32Matrix::from_rows([[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
        let dependent =
            F32Matrix::from_rows([[1.0, 2.0], [2.0, 4.0], [3.0, 6.0], [4.0, 8.0]]).unwrap();

        assert_eq!(full_rank.rank_estimate(0.0).unwrap(), 2);
        assert_eq!(dependent.rank_estimate(0.0).unwrap(), 1);
    }

    #[test]
    fn cholesky_solve_reconstructs_rhs() {
        let matrix = F32Matrix::from_rows([[4.0, 2.0], [2.0, 3.0]]).unwrap();
        let decomposition = matrix.cholesky_decompose().unwrap();
        let solution = decomposition.solve_vector(&[6.0, 5.0]).unwrap();
        let reconstructed = matrix.matvec(&solution).unwrap().into_values();

        assert_close(reconstructed[0], 6.0);
        assert_close(reconstructed[1], 5.0);
    }

    #[test]
    fn cholesky_log_determinant_matches_determinant() {
        let matrix = F32Matrix::from_rows([[4.0, 2.0], [2.0, 3.0]]).unwrap();
        let decomposition = matrix.cholesky_decompose().unwrap();
        let log_determinant = decomposition.log_determinant().unwrap();

        assert_close(log_determinant.exp(), matrix.determinant().unwrap());
    }

    #[test]
    fn least_squares_rejects_invalid_inputs() {
        let design = F32Matrix::from_rows([[1.0, 1.0], [1.0, 2.0], [1.0, 3.0]]).unwrap();

        assert!(design.least_squares(&[1.0, 2.0], 0.0).is_err());
        assert!(design.least_squares(&[1.0, f32::NAN, 3.0], 0.0).is_err());
        assert!(design.least_squares(&[1.0, 2.0, 3.0], -1.0).is_err());
        assert!(design
            .least_squares(&[1.0, 2.0, 3.0], f32::INFINITY)
            .is_err());
    }

    #[test]
    fn condition_estimate_reports_diagonal_range() {
        let matrix = F32Matrix::from_rows([[2.0, 1.0], [1.0, 3.0]]).unwrap();
        let estimate = matrix.condition_estimate().unwrap();
        assert_close(estimate.determinant, 5.0);
        assert!(estimate.diagonal_min_abs > 0.0);
        assert!(estimate.diagonal_max_abs >= estimate.diagonal_min_abs);
        assert!(estimate.pivot_ratio().unwrap() > 0.0);
    }
}
