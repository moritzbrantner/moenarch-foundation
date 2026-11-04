//! Differential coverage of canonical CSR×CSR composition against `sprs`
//! (dev-only oracle, Bank & Douglas SMMP).
//!
//! Both implementations accumulate each output entry in the same order (left
//! row entries in stored order, then right row entries in stored order), so
//! for canonical inputs the canonical Foundation result must equal the
//! `sprs` result *bit for bit* once `sprs`' explicitly stored cancellation
//! zeros are removed. Where Foundation accepts non-canonical CSR input
//! (unsorted or duplicate columns) the oracle needs canonical input, so the
//! comparison uses an explicit accumulation-order tolerance instead.
//!
//! Recorded `sprs` behaviour that differs from the Foundation contract:
//! * exact-zero cancellations remain as explicitly stored `0.0` entries;
//! * non-finite products and sums propagate as `inf`/`NaN` instead of failing;
//! * construction rejects unsorted or duplicate column indices;
//! * no candidate-product statistic exists, so it is computed here
//!   independently from the CSR structure rather than taken from `sprs`.

use math_sparse_data::{CooMatrix, CsrMatrix};
use sprs::CsMat;

/// Deterministic xorshift64* stream for reproducible sparse patterns.
struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    /// Non-zero value on a coarse grid so that cancellations can occur.
    fn value(&mut self) -> f32 {
        let magnitude = (self.below(16) + 1) as f32 * 0.25;
        if self.next() & 1 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }
}

fn canonical(rows: usize, cols: usize, entries: Vec<(usize, usize, f32)>) -> CsrMatrix {
    CooMatrix::new(rows, cols, entries)
        .expect("valid COO fixture")
        .to_csr()
        .expect("canonical CSR fixture")
}

/// Random matrix whose row `r` has `degree(r)` sampled (possibly duplicate)
/// columns; duplicates are combined by canonicalization.
fn generated(seed: u64, rows: usize, cols: usize, degree: impl Fn(usize) -> usize) -> CsrMatrix {
    let mut stream = Stream(seed);
    let mut entries = Vec::new();
    for row in 0..rows {
        for _ in 0..degree(row) {
            entries.push((row, stream.below(cols), stream.value()));
        }
    }
    canonical(rows, cols, entries)
}

fn to_sprs(matrix: &CsrMatrix) -> CsMat<f32> {
    let mut indptr = vec![0usize];
    let mut indices = Vec::new();
    let mut data = Vec::new();
    for row in matrix.rows_iter() {
        indices.extend_from_slice(row.indices());
        data.extend_from_slice(row.values());
        indptr.push(indices.len());
    }
    CsMat::try_new((matrix.rows(), matrix.cols()), indptr, indices, data)
        .map_err(|(_, _, _, error)| error)
        .expect("canonical Foundation CSR is valid sprs CSR")
}

/// `(row_offsets, column_indices, values)` with explicit zeros removed.
type Triplets = (Vec<usize>, Vec<usize>, Vec<f32>);

fn sprs_without_explicit_zeros(matrix: &CsMat<f32>) -> (Triplets, usize) {
    let mut offsets = vec![0usize];
    let mut indices = Vec::new();
    let mut values = Vec::new();
    let mut explicit_zeros = 0;
    for row in matrix.outer_iterator() {
        for (col, &value) in row.iter() {
            if value == 0.0 {
                explicit_zeros += 1;
            } else {
                indices.push(col);
                values.push(value);
            }
        }
        offsets.push(indices.len());
    }
    ((offsets, indices, values), explicit_zeros)
}

fn foundation_triplets(matrix: &CsrMatrix) -> Triplets {
    let mut offsets = vec![0usize];
    let mut indices = Vec::new();
    let mut values = Vec::new();
    for row in matrix.rows_iter() {
        indices.extend_from_slice(row.indices());
        values.extend_from_slice(row.values());
        offsets.push(indices.len());
    }
    (offsets, indices, values)
}

fn independent_candidate_products(left: &CsrMatrix, right: &CsrMatrix) -> usize {
    let right_nnz = right.row_nnz();
    left.rows_iter()
        .flat_map(|row| row.indices().to_vec())
        .map(|shared| right_nnz[shared])
        .sum()
}

/// Asserts exact agreement and returns the number of explicit zeros `sprs`
/// retained.
fn assert_matches_sprs(left: &CsrMatrix, right: &CsrMatrix) -> usize {
    let (product, stats) = left.mul_csr_with_stats(right).expect("finite product");
    let oracle = &to_sprs(left) * &to_sprs(right);
    assert_eq!(oracle.shape(), (product.rows(), product.cols()));
    let (expected, explicit_zeros) = sprs_without_explicit_zeros(&oracle);
    let actual = foundation_triplets(&product);
    assert_eq!(actual.0, expected.0, "row offsets differ from sprs");
    assert_eq!(actual.1, expected.1, "column order differs from sprs");
    let actual_bits: Vec<u32> = actual.2.iter().map(|value| value.to_bits()).collect();
    let expected_bits: Vec<u32> = expected.2.iter().map(|value| value.to_bits()).collect();
    assert_eq!(actual_bits, expected_bits, "values are not bit-identical");
    assert_eq!(stats.output_nnz, oracle.nnz() - explicit_zeros);
    assert_eq!(
        stats.candidate_products,
        independent_candidate_products(left, right)
    );
    explicit_zeros
}

#[test]
fn tiny_rectangular_and_identity_products_match_sprs() {
    let left = canonical(
        3,
        4,
        vec![(0, 0, 2.0), (0, 3, -1.0), (1, 1, 3.0), (2, 2, 4.0)],
    );
    let right = canonical(
        4,
        2,
        vec![(0, 0, 5.0), (1, 1, 2.0), (2, 0, -2.0), (3, 1, 7.0)],
    );
    assert_eq!(assert_matches_sprs(&left, &right), 0);

    let identity = canonical(4, 4, (0..4).map(|index| (index, index, 1.0)).collect());
    assert_matches_sprs(&left, &identity);
    assert_matches_sprs(&identity, &right);

    let one = canonical(1, 1, vec![(0, 0, -3.5)]);
    assert_matches_sprs(&one, &one);
}

#[test]
fn empty_rows_columns_and_all_zero_products_match_sprs() {
    let left = canonical(3, 5, vec![(0, 4, 1.0), (2, 0, -2.0)]);
    let empty = canonical(5, 7, vec![]);
    assert_matches_sprs(&left, &empty);
    let disjoint = canonical(5, 2, vec![(1, 0, 3.0), (3, 1, 4.0)]);
    let product = left.mul_csr(&disjoint).unwrap();
    assert_eq!(product.row_nnz(), vec![0, 0, 0]);
    assert_matches_sprs(&left, &disjoint);
    assert_matches_sprs(&empty.transpose().unwrap(), &empty);
}

#[test]
fn cancellation_is_removed_while_sprs_keeps_explicit_zeros() {
    let left = canonical(2, 2, vec![(0, 0, 1.0), (0, 1, 1.0), (1, 0, 0.5)]);
    let right = canonical(2, 2, vec![(0, 0, 2.0), (1, 0, -2.0), (1, 1, 3.0)]);
    let explicit_zeros = assert_matches_sprs(&left, &right);
    assert_eq!(explicit_zeros, 1, "sprs stores the cancelled (0, 0) entry");
    let product = left.mul_csr(&right).unwrap();
    assert_eq!(
        product.to_coo().unwrap().entries(),
        &[(0, 1, 3.0), (1, 0, 1.0)]
    );
}

#[test]
fn generated_adverse_sparsity_patterns_match_sprs_bit_for_bit() {
    let mut explicit_zeros = 0;
    // Highly sparse square.
    let a = generated(1, 400, 400, |_| 2);
    let b = generated(2, 400, 400, |_| 2);
    explicit_zeros += assert_matches_sprs(&a, &b);
    // Thin left times wide right.
    let thin = generated(3, 4, 600, |_| 40);
    let wide = generated(4, 600, 50_000, |_| 6);
    explicit_zeros += assert_matches_sprs(&thin, &wide);
    // Dense-ish rows on a small column domain (many collisions/cancellations).
    let dense = generated(5, 64, 48, |_| 40);
    let dense_right = generated(6, 48, 32, |_| 24);
    explicit_zeros += assert_matches_sprs(&dense, &dense_right);
    // Skewed degree: a few hub rows with large fan-out.
    let skew = |row: usize| if row.is_multiple_of(50) { 300 } else { 2 };
    let hubs = generated(7, 500, 500, skew);
    let hubs_right = generated(8, 500, 500, skew);
    explicit_zeros += assert_matches_sprs(&hubs, &hubs_right);
    // Tall times short-wide.
    let tall = generated(9, 2_000, 3, |_| 2);
    let short = generated(10, 3, 900, |_| 300);
    explicit_zeros += assert_matches_sprs(&tall, &short);
    assert!(
        explicit_zeros > 0,
        "the coarse value grid must exercise sprs explicit-zero retention"
    );
}

#[test]
fn transposition_and_gram_composition_match_sprs() {
    let a = generated(11, 120, 90, |row| 1 + row % 7);
    let b = generated(12, 90, 150, |row| 1 + row % 5);

    // (A B)^T = B^T A^T: compare the transposed composition exactly.
    let a_t = a.transpose().unwrap();
    let b_t = b.transpose().unwrap();
    assert_matches_sprs(&b_t, &a_t);
    let oracle_transposed = &to_sprs(&b_t) * &to_sprs(&a_t);
    let transposed_view = (&to_sprs(&a) * &to_sprs(&b)).transpose_view().to_csr();
    // Different accumulation order: compare entries within tolerance.
    let tolerance = 1.0e-4;
    let dense_left = oracle_transposed.to_dense();
    let dense_right = transposed_view.to_dense();
    for (left, right) in dense_left.iter().zip(dense_right.iter()) {
        assert!((left - right).abs() <= tolerance, "{left} != {right}");
    }

    // Row Gram A A^T and column Gram A^T A.
    let rows = a.gram_rows_sparse().unwrap();
    let oracle_rows = &to_sprs(&a) * &to_sprs(&a_t);
    assert_eq!(
        foundation_triplets(&rows),
        sprs_without_explicit_zeros(&oracle_rows).0
    );
    let columns = a.gram_columns_sparse().unwrap();
    let oracle_columns = &to_sprs(&a_t) * &to_sprs(&a);
    assert_eq!(
        foundation_triplets(&columns),
        sprs_without_explicit_zeros(&oracle_columns).0
    );
}

#[test]
fn non_canonical_foundation_input_matches_canonicalized_oracle_within_tolerance() {
    // Foundation CSR may store unsorted and duplicate columns; sprs rejects
    // them, so the oracle receives the canonicalized equivalent.
    let raw = CsrMatrix::new(
        2,
        3,
        vec![0, 4, 6],
        vec![2, 0, 2, 1, 1, 1],
        vec![0.1, 1.0, 0.2, 3.0, -0.7, 0.7],
    )
    .unwrap();
    let mut indptr = vec![0usize];
    let mut indices = Vec::new();
    let mut data = Vec::new();
    for row in raw.rows_iter() {
        indices.extend_from_slice(row.indices());
        data.extend_from_slice(row.values());
        indptr.push(indices.len());
    }
    assert!(
        CsMat::try_new((2, 3), indptr, indices, data).is_err(),
        "sprs rejects unsorted/duplicate CSR columns"
    );

    let right = generated(13, 3, 5, |_| 3);
    let product = raw.mul_csr(&right).unwrap();
    let canonical_left = raw.to_coo().unwrap().to_csr().unwrap();
    let oracle = &to_sprs(&canonical_left) * &to_sprs(&right);
    let expected = oracle.to_dense();
    let actual = product.to_dense_matrix().unwrap();
    for row in 0..2 {
        for col in 0..5 {
            let actual = actual.as_view().get(row, col).unwrap();
            let expected = expected[[row, col]];
            assert!(
                (actual - expected).abs() <= 1.0e-5 * (1.0 + expected.abs()),
                "({row}, {col}): {actual} != {expected}"
            );
        }
    }
}

#[test]
fn non_finite_results_fail_in_foundation_but_propagate_in_sprs() {
    let huge = canonical(1, 2, vec![(0, 0, f32::MAX), (0, 1, f32::MAX)]);
    let twos = canonical(2, 1, vec![(0, 0, 2.0), (1, 0, 1.0)]);
    assert!(huge.mul_csr(&twos).is_err(), "product overflow is rejected");
    let ones = canonical(2, 1, vec![(0, 0, 1.0), (1, 0, 1.0)]);
    assert!(
        huge.mul_csr(&ones).is_err(),
        "accumulated overflow is rejected"
    );

    let oracle = &to_sprs(&huge) * &to_sprs(&twos);
    assert!(oracle.data().iter().any(|value| value.is_infinite()));
    let oracle = &to_sprs(&huge) * &to_sprs(&ones);
    assert!(oracle.data().iter().any(|value| value.is_infinite()));
}
