use crate::{invalid_argument, F32MatrixView, LuDecomposition, MatrixShape};
use media_core::Result;

pub(crate) fn pivot_tolerance(values: &[f32]) -> f32 {
    let max_abs = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f32, f32::max);
    (max_abs * 1.0e-6).max(1.0e-7)
}

pub(crate) fn lu_decompose(matrix: F32MatrixView<'_>) -> Result<LuDecomposition> {
    let shape = matrix.shape();
    if shape.rows != shape.cols {
        return Err(invalid_argument(
            "LU decomposition requires a square matrix",
        ));
    }

    let size = shape.rows;
    let mut lu = Vec::with_capacity(shape.element_count()?);
    for row in 0..size {
        for col in 0..size {
            lu.push(matrix.get(row, col)?);
        }
    }

    let tolerance = pivot_tolerance(&lu);
    let mut pivots = (0..size).collect::<Vec<_>>();
    let mut swap_count = 0;

    for col in 0..size {
        let mut pivot_row = col;
        let mut pivot_abs = lu[col * size + col].abs();
        for row in col + 1..size {
            let candidate = lu[row * size + col].abs();
            if candidate > pivot_abs {
                pivot_abs = candidate;
                pivot_row = row;
            }
        }

        if pivot_abs <= tolerance {
            return Err(invalid_argument("matrix is singular or near-singular"));
        }

        if pivot_row != col {
            for inner_col in 0..size {
                lu.swap(col * size + inner_col, pivot_row * size + inner_col);
            }
            pivots.swap(col, pivot_row);
            swap_count += 1;
        }

        let pivot = lu[col * size + col];
        for row in col + 1..size {
            let factor_index = row * size + col;
            lu[factor_index] /= pivot;
            let factor = lu[factor_index];
            for inner_col in col + 1..size {
                lu[row * size + inner_col] -= factor * lu[col * size + inner_col];
            }
        }
    }

    if lu.iter().any(|value| !value.is_finite()) {
        return Err(invalid_argument(
            "LU decomposition produced non-finite values",
        ));
    }

    Ok(LuDecomposition::new(
        MatrixShape::new(size, size)?,
        lu,
        pivots,
        swap_count,
    ))
}
