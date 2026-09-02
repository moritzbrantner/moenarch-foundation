//! Deterministic sparse matrix composition without dense intermediates.

use std::collections::BTreeMap;

use super::{invalid_argument, CsrMatrix, Result};

/// Observable structural work performed by sparse matrix multiplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SparseProductStats {
    /// Number of scalar products considered from compatible stored entries.
    pub candidate_products: usize,
    /// Number of canonical non-zero entries stored in the result.
    pub output_nnz: usize,
}

impl CsrMatrix {
    /// Multiplies two CSR matrices and returns a canonical CSR result.
    ///
    /// Each output row uses a row-local sparse accumulator keyed by output
    /// column. The implementation never materializes a dense output matrix or
    /// a dense `right.cols()` row accumulator.
    pub fn mul_csr(&self, right: &Self) -> Result<Self> {
        self.mul_csr_with_stats(right).map(|(matrix, _)| matrix)
    }

    /// Multiplies two CSR matrices and reports deterministic structural work.
    pub fn mul_csr_with_stats(&self, right: &Self) -> Result<(Self, SparseProductStats)> {
        self.validate()?;
        right.validate()?;
        if self.cols != right.rows {
            return Err(invalid_argument(
                "sparse matrix dimensions are incompatible for multiplication",
            ));
        }

        let mut row_offsets = Vec::with_capacity(self.rows + 1);
        let mut column_indices = Vec::new();
        let mut values = Vec::new();
        let mut candidate_products = 0usize;
        row_offsets.push(0);

        for row in 0..self.rows {
            let mut accumulator = BTreeMap::<usize, f32>::new();
            for left_entry in self.row_offsets[row]..self.row_offsets[row + 1] {
                let shared = self.column_indices[left_entry];
                let left_value = self.values[left_entry];
                for right_entry in right.row_offsets[shared]..right.row_offsets[shared + 1] {
                    candidate_products = candidate_products.checked_add(1).ok_or_else(|| {
                        invalid_argument("sparse multiplication candidate count overflowed usize")
                    })?;
                    let output_col = right.column_indices[right_entry];
                    let contribution = left_value * right.values[right_entry];
                    if !contribution.is_finite() {
                        return Err(invalid_argument(
                            "sparse multiplication produced a non-finite contribution",
                        ));
                    }
                    let accumulated = accumulator.entry(output_col).or_insert(0.0);
                    *accumulated += contribution;
                    if !accumulated.is_finite() {
                        return Err(invalid_argument(
                            "sparse multiplication produced a non-finite accumulated value",
                        ));
                    }
                }
            }

            for (col, value) in accumulator {
                if value != 0.0 {
                    column_indices.push(col);
                    values.push(value);
                }
            }
            row_offsets.push(values.len());
        }

        let output_nnz = values.len();
        let matrix = Self::new(self.rows, right.cols, row_offsets, column_indices, values)?;
        Ok((
            matrix,
            SparseProductStats {
                candidate_products,
                output_nnz,
            },
        ))
    }

    /// Returns the sparse column Gram matrix `A^T A`.
    pub fn gram_columns_sparse(&self) -> Result<Self> {
        self.transpose()?.mul_csr(self)
    }

    /// Returns the sparse row Gram matrix `A A^T`.
    pub fn gram_rows_sparse(&self) -> Result<Self> {
        self.mul_csr(&self.transpose()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CooMatrix;

    fn csr(rows: usize, cols: usize, entries: &[(usize, usize, f32)]) -> CsrMatrix {
        CooMatrix::new(rows, cols, entries.to_vec())
            .unwrap()
            .to_csr()
            .unwrap()
    }

    fn assert_dense_close(left: &CsrMatrix, right: &CsrMatrix) {
        let left = left.to_dense_matrix().unwrap();
        let right = right.to_dense_matrix().unwrap();
        assert_eq!(left.shape(), right.shape());
        for (left, right) in left.values().iter().zip(right.values()) {
            assert!((left - right).abs() < 1.0e-5, "{left} != {right}");
        }
    }

    fn dense_product(left: &CsrMatrix, right: &CsrMatrix) -> CsrMatrix {
        let left_dense = left.to_dense_matrix().unwrap();
        let right_dense = right.to_dense_matrix().unwrap();
        let product = left_dense.matmul(&right_dense.as_view()).unwrap();
        let entries = (0..product.shape().rows)
            .flat_map(|row| {
                let product = &product;
                (0..product.shape().cols).filter_map(move |col| {
                    let value = product.as_view().get(row, col).unwrap();
                    (value != 0.0).then_some((row, col, value))
                })
            })
            .collect::<Vec<_>>();
        csr(product.shape().rows, product.shape().cols, &entries)
    }

    #[test]
    fn csr_product_matches_dense_reference() {
        let left = csr(3, 4, &[(0, 0, 2.0), (0, 3, -1.0), (1, 1, 3.0), (2, 2, 4.0)]);
        let right = csr(4, 2, &[(0, 0, 5.0), (1, 1, 2.0), (2, 0, -2.0), (3, 1, 7.0)]);
        let sparse = left.mul_csr(&right).unwrap();
        assert_dense_close(&sparse, &dense_product(&left, &right));
    }

    #[test]
    fn product_is_canonical_and_reports_structural_work() {
        let left = CsrMatrix::new(1, 2, vec![0, 3], vec![1, 0, 0], vec![1.0, 2.0, -2.0]).unwrap();
        let right = csr(2, 3, &[(0, 2, 3.0), (1, 1, 4.0)]);
        let (product, stats) = left.mul_csr_with_stats(&right).unwrap();
        assert_eq!(product.to_coo().unwrap().entries(), &[(0, 1, 4.0)]);
        assert_eq!(stats.candidate_products, 3);
        assert_eq!(stats.output_nnz, 1);
    }

    #[test]
    fn cancellation_elides_exact_zero_entries() {
        let left = csr(1, 2, &[(0, 0, 1.0), (0, 1, 1.0)]);
        let right = csr(2, 1, &[(0, 0, 2.0), (1, 0, -2.0)]);
        let product = left.mul_csr(&right).unwrap();
        assert_eq!(product.to_coo().unwrap().entries(), &[]);
    }

    #[test]
    fn identity_zero_and_empty_rows_are_preserved() {
        let matrix = csr(3, 3, &[(0, 2, 4.0), (2, 0, -1.0)]);
        let identity = csr(3, 3, &[(0, 0, 1.0), (1, 1, 1.0), (2, 2, 1.0)]);
        assert_eq!(matrix.mul_csr(&identity).unwrap(), matrix);
        assert_eq!(identity.mul_csr(&matrix).unwrap(), matrix);

        let zero = csr(3, 2, &[]);
        let zero_product = matrix.mul_csr(&zero).unwrap();
        assert_eq!(zero_product.rows(), 3);
        assert_eq!(zero_product.cols(), 2);
        assert_eq!(zero_product.to_coo().unwrap().entries(), &[]);
        assert_eq!(zero_product.row_nnz(), vec![0, 0, 0]);
    }

    #[test]
    fn bounded_associativity_matches_dense_oracle() {
        let a = csr(2, 3, &[(0, 0, 1.0), (0, 2, 2.0), (1, 1, 3.0)]);
        let b = csr(3, 2, &[(0, 1, 4.0), (1, 0, -1.0), (2, 1, 5.0)]);
        let c = csr(2, 2, &[(0, 0, 2.0), (1, 1, -3.0)]);
        let left = a.mul_csr(&b).unwrap().mul_csr(&c).unwrap();
        let right = a.mul_csr(&b.mul_csr(&c).unwrap()).unwrap();
        assert_dense_close(&left, &right);
    }

    #[test]
    fn sparse_gram_helpers_match_dense_composition() {
        let matrix = csr(3, 4, &[(0, 0, 1.0), (0, 3, 2.0), (1, 1, -1.0), (2, 3, 4.0)]);
        let columns = matrix.gram_columns_sparse().unwrap();
        let rows = matrix.gram_rows_sparse().unwrap();
        assert_dense_close(
            &columns,
            &matrix.transpose().unwrap().mul_csr(&matrix).unwrap(),
        );
        assert_dense_close(
            &rows,
            &matrix.mul_csr(&matrix.transpose().unwrap()).unwrap(),
        );
    }

    #[test]
    fn rejects_shape_mismatch_and_non_finite_products() {
        let left = csr(2, 3, &[(0, 0, 1.0)]);
        let wrong = csr(2, 2, &[(0, 0, 1.0)]);
        assert!(left.mul_csr(&wrong).is_err());

        let huge_left = csr(1, 1, &[(0, 0, f32::MAX)]);
        let huge_right = csr(1, 1, &[(0, 0, 2.0)]);
        assert!(huge_left.mul_csr(&huge_right).is_err());
    }
}
