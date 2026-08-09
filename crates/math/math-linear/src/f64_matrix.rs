use std::convert::TryFrom;

use crate::{invalid_argument, F32Matrix, MatrixLayout, MatrixShape};
use media_core::{DetectError, Result};

#[derive(Debug, Clone, PartialEq)]
/// Owned finite `f64` matrix stored in row-major order.
pub struct F64Matrix {
    shape: MatrixShape,
    layout: MatrixLayout,
    values: Vec<f64>,
}

impl F64Matrix {
    /// Creates a row-major matrix after shape and finite-value validation.
    pub fn new(shape: MatrixShape, values: Vec<f64>) -> Result<Self> {
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
    pub fn from_rows<const R: usize, const C: usize>(rows: [[f64; C]; R]) -> Result<Self> {
        let mut values = Vec::with_capacity(R * C);
        for row in rows {
            values.extend(row);
        }
        Self::new(MatrixShape::new(R, C)?, values)
    }

    /// Creates a square matrix with the supplied finite diagonal values.
    pub fn from_diag(diagonal: &[f64]) -> Result<Self> {
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
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Consumes the matrix and returns its contiguous values.
    pub fn into_values(self) -> Vec<f64> {
        self.values
    }

    /// Borrows this matrix as a view without copying values.
    pub fn as_view(&self) -> F64MatrixView<'_> {
        F64MatrixView {
            shape: self.shape,
            layout: self.layout,
            values: &self.values,
        }
    }

    /// Borrows one row, respecting the current matrix layout.
    pub fn row(&self, index: usize) -> Result<F64RowView<'_>> {
        self.as_view().row(index)
    }

    /// Borrows one column, respecting the current matrix layout.
    pub fn column(&self, index: usize) -> Result<F64ColumnView<'_>> {
        self.as_view().column(index)
    }

    /// Multiplies this matrix by `right`.
    pub fn matmul(&self, right: &F64MatrixView<'_>) -> Result<Self> {
        self.as_view().matmul(right)
    }

    /// Multiplies this matrix by a finite vector.
    pub fn matvec(&self, vector: &[f64]) -> Result<Vec<f64>> {
        self.as_view().matvec(vector)
    }

    /// Creates a transposed view without copying values.
    pub fn transpose_view(&self) -> F64MatrixView<'_> {
        self.as_view().transpose()
    }

    /// Returns a row-major owned transpose of this matrix.
    pub fn transpose_owned(&self) -> Result<Self> {
        self.as_view().transpose_owned()
    }

    /// Returns a row-major matrix with column means subtracted.
    pub fn center_columns(&self) -> Result<Self> {
        self.as_view().center_columns()
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
/// Borrowed finite `f64` matrix values with shape and layout metadata.
pub struct F64MatrixView<'a> {
    shape: MatrixShape,
    layout: MatrixLayout,
    values: &'a [f64],
}

impl<'a> F64MatrixView<'a> {
    /// Creates a row-major matrix view after validating shape and values.
    pub fn new(shape: MatrixShape, values: &'a [f64]) -> Result<Self> {
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
    pub fn values(&self) -> &'a [f64] {
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
    pub fn transpose_owned(&self) -> Result<F64Matrix> {
        self.transpose().into_owned()
    }

    /// Borrows one logical row from this view.
    pub fn row(self, index: usize) -> Result<F64RowView<'a>> {
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
        Ok(F64RowView {
            values: self.values,
            len: self.shape.cols,
            offset,
            stride,
        })
    }

    /// Borrows one logical column from this view.
    pub fn column(self, index: usize) -> Result<F64ColumnView<'a>> {
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
        Ok(F64ColumnView {
            values: self.values,
            len: self.shape.rows,
            offset,
            stride,
        })
    }

    /// Reads one value by logical row and column.
    pub fn get(self, row: usize, col: usize) -> Result<f64> {
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

    /// Multiplies this view by another matrix view.
    pub fn matmul(self, right: &F64MatrixView<'_>) -> Result<F64Matrix> {
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
        F64Matrix::new(shape, values)
    }

    /// Multiplies this view by a finite vector.
    pub fn matvec(self, vector: &[f64]) -> Result<Vec<f64>> {
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
            let row_values = self.row(row)?.as_slice();
            *output_value = row_values.iter().zip(vector).map(|(l, r)| l * r).sum();
        }
        Ok(output)
    }

    /// Computes the Frobenius norm.
    pub fn frobenius_norm(&self) -> Result<f64> {
        self.validate()?;
        let mut sum_squares = 0.0;
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                let value = self.get(row, col)?;
                sum_squares += value * value;
            }
        }
        if !sum_squares.is_finite() {
            return Err(invalid_argument(
                "matrix Frobenius norm produced a non-finite value",
            ));
        }
        Ok(sum_squares.sqrt())
    }

    /// Averages each logical column.
    pub fn column_means(&self) -> Result<Vec<f64>> {
        self.validate()?;
        (0..self.shape.cols)
            .map(|col| Ok(self.column(col)?.iter().sum::<f64>() / self.shape.rows as f64))
            .collect()
    }

    /// Returns a row-major matrix with column means subtracted.
    pub fn center_columns(&self) -> Result<F64Matrix> {
        let means = self.column_means()?;
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for (col, mean) in means.iter().enumerate().take(self.shape.cols) {
                values.push(self.get(row, col)? - mean);
            }
        }
        F64Matrix::new(self.shape, values)
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
    pub fn into_owned(self) -> Result<F64Matrix> {
        let mut values = Vec::with_capacity(self.shape.element_count()?);
        for row in 0..self.shape.rows {
            for col in 0..self.shape.cols {
                values.push(self.get(row, col)?);
            }
        }
        F64Matrix::new(self.shape, values)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Strided borrowed view over one logical f64 matrix row.
pub struct F64RowView<'a> {
    values: &'a [f64],
    len: usize,
    offset: usize,
    stride: usize,
}

impl<'a> F64RowView<'a> {
    /// Iterates over row values in logical column order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(|index| self.values[self.offset + index * self.stride])
    }

    /// Collects the possibly strided row into a contiguous vector.
    pub fn as_slice(&self) -> Vec<f64> {
        self.iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Strided borrowed view over one logical f64 matrix column.
pub struct F64ColumnView<'a> {
    values: &'a [f64],
    len: usize,
    offset: usize,
    stride: usize,
}

impl<'a> F64ColumnView<'a> {
    /// Iterates over column values in logical row order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(|index| self.values[self.offset + index * self.stride])
    }

    /// Collects the possibly strided column into a contiguous vector.
    pub fn as_slice(&self) -> Vec<f64> {
        self.iter().collect()
    }
}

impl TryFrom<&F32Matrix> for F64Matrix {
    type Error = DetectError;

    fn try_from(value: &F32Matrix) -> Result<Self> {
        F64Matrix::new(
            value.shape(),
            value.values().iter().map(|value| *value as f64).collect(),
        )
    }
}

impl TryFrom<&F64Matrix> for F32Matrix {
    type Error = DetectError;

    fn try_from(value: &F64Matrix) -> Result<Self> {
        let mut values = Vec::with_capacity(value.values().len());
        for value in value.values() {
            if !value.is_finite() || *value < f32::MIN as f64 || *value > f32::MAX as f64 {
                return Err(invalid_argument(
                    "f64-to-f32 matrix conversion requires finite in-range values",
                ));
            }
            values.push(*value as f32);
        }
        F32Matrix::new(value.shape(), values)
    }
}

impl TryFrom<F64Matrix> for F32Matrix {
    type Error = DetectError;

    fn try_from(value: F64Matrix) -> Result<Self> {
        F32Matrix::try_from(&value)
    }
}
