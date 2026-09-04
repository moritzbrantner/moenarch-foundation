use math_sparse_data::SparseVector;

const VALUES: [f32; 3] = [-1.0, 0.0, 1.0];

fn exhaustive_dense_vectors(dimensions: usize) -> Vec<Vec<f32>> {
    let count = VALUES.len().pow(dimensions as u32);
    (0..count)
        .map(|mut encoded| {
            let mut values = Vec::with_capacity(dimensions);
            for _ in 0..dimensions {
                values.push(VALUES[encoded % VALUES.len()]);
                encoded /= VALUES.len();
            }
            values
        })
        .collect()
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1.0e-5,
        "expected {expected}, got {actual}"
    );
}

fn assert_slice_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert_close(actual, expected);
    }
}

#[test]
fn sparse_dense_round_trip_covers_small_domains_exhaustively() {
    for dimensions in 1..=6 {
        for dense in exhaustive_dense_vectors(dimensions) {
            let sparse = SparseVector::from_dense(&dense).unwrap();
            assert_eq!(sparse.dimensions(), dimensions);
            assert_slice_close(&sparse.to_dense(), &dense);
            assert_eq!(sparse.nnz(), dense.iter().filter(|value| **value != 0.0).count());
        }
    }
}

#[test]
fn sparse_binary_operations_match_dense_reference_exhaustively() {
    let vectors = exhaustive_dense_vectors(4);
    for left_dense in &vectors {
        let left = SparseVector::from_dense(left_dense).unwrap();
        for right_dense in &vectors {
            let right = SparseVector::from_dense(right_dense).unwrap();

            let expected_dot = left_dense
                .iter()
                .zip(right_dense)
                .map(|(left, right)| left * right)
                .sum::<f32>();
            assert_close(left.dot(&right).unwrap(), expected_dot);

            let expected_sum = left_dense
                .iter()
                .zip(right_dense)
                .map(|(left, right)| left + right)
                .collect::<Vec<_>>();
            assert_slice_close(&left.add(&right).unwrap().to_dense(), &expected_sum);

            let expected_hadamard = left_dense
                .iter()
                .zip(right_dense)
                .map(|(left, right)| left * right)
                .collect::<Vec<_>>();
            assert_slice_close(
                &left.hadamard(&right).unwrap().to_dense(),
                &expected_hadamard,
            );
        }
    }
}

#[test]
fn sparse_norms_scaling_and_cosine_match_dense_reference() {
    for dense in exhaustive_dense_vectors(5) {
        let sparse = SparseVector::from_dense(&dense).unwrap();
        let expected_l1 = dense.iter().map(|value| value.abs()).sum::<f32>();
        let expected_l2 = dense.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert_close(sparse.l1_norm().unwrap(), expected_l1);
        assert_close(sparse.l2_norm().unwrap(), expected_l2);

        for factor in [-2.0, -0.5, 0.0, 0.5, 2.0] {
            let expected = dense.iter().map(|value| value * factor).collect::<Vec<_>>();
            assert_slice_close(&sparse.scale(factor).unwrap().to_dense(), &expected);
        }

        if expected_l2 > f32::EPSILON {
            let normalized = sparse.normalize_l2().unwrap();
            assert_close(normalized.l2_norm().unwrap(), 1.0);
            assert_close(sparse.cosine_similarity(&sparse).unwrap(), 1.0);
        }
    }
}

#[test]
fn canonicalization_is_idempotent_and_combines_duplicate_indices() {
    let sparse = SparseVector::new(
        4,
        vec![2, 0, 2, 1, 0],
        vec![3.0, 1.0, -1.0, 4.0, 2.0],
    )
    .unwrap();
    let canonical = sparse.canonicalized().unwrap();
    assert_eq!(canonical.indices(), &[0, 1, 2]);
    assert_slice_close(canonical.values(), &[3.0, 4.0, 2.0]);
    assert_slice_close(&canonical.to_dense(), &[3.0, 4.0, 2.0, 0.0]);
    assert_eq!(canonical.canonicalized().unwrap(), canonical);
}
