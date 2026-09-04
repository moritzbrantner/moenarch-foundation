use math_signal_core::{
    apply_fir_mono, db_to_linear, linear_to_db, resample_mono, signal_levels, FirKernel1d,
    InterpolationMode, SampleRate,
};

const VALUES: [f32; 3] = [-1.0, 0.0, 1.0];

fn exhaustive_signals(len: usize) -> Vec<Vec<f32>> {
    let count = VALUES.len().pow(len as u32);
    (0..count)
        .map(|mut encoded| {
            let mut signal = Vec::with_capacity(len);
            for _ in 0..len {
                signal.push(VALUES[encoded % VALUES.len()]);
                encoded /= VALUES.len();
            }
            signal
        })
        .collect()
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1.0e-5,
        "expected {expected}, got {actual}"
    );
}

fn reference_fir(samples: &[f32], coefficients: &[f32]) -> Vec<f32> {
    let center = coefficients.len() / 2;
    (0..samples.len())
        .map(|index| {
            coefficients
                .iter()
                .enumerate()
                .filter_map(|(tap, coefficient)| {
                    let sample_index = index as isize + tap as isize - center as isize;
                    (0..samples.len() as isize)
                        .contains(&sample_index)
                        .then(|| samples[sample_index as usize] * coefficient)
                })
                .sum()
        })
        .collect()
}

#[test]
fn signal_levels_match_reference_statistics_exhaustively() {
    for len in 1..=6 {
        for samples in exhaustive_signals(len) {
            let levels = signal_levels(&samples).unwrap();
            let peak = samples.iter().map(|sample| sample.abs()).fold(0.0, f32::max);
            let mean = samples.iter().sum::<f32>() / samples.len() as f32;
            let rms = (samples.iter().map(|sample| sample * sample).sum::<f32>()
                / samples.len() as f32)
                .sqrt();

            assert_eq!(levels.count, samples.len());
            assert_close(levels.peak, peak);
            assert_close(levels.mean, mean);
            assert_close(levels.dc_offset, mean);
            assert_close(levels.rms, rms);
        }
    }
}

#[test]
fn centered_fir_matches_direct_reference_convolution_exhaustively() {
    let coefficient_sets = [
        vec![1.0],
        vec![0.5, 0.5],
        vec![0.25, 0.5, 0.25],
        vec![-0.25, 0.0, 0.5, 0.0, -0.25],
    ];

    for coefficients in coefficient_sets {
        let kernel = FirKernel1d::new(coefficients.clone()).unwrap();
        for len in 1..=5 {
            for samples in exhaustive_signals(len) {
                let actual = apply_fir_mono(&samples, &kernel).unwrap();
                let expected = reference_fir(&samples, &coefficients);
                assert_eq!(actual.len(), expected.len());
                for (actual, expected) in actual.into_iter().zip(expected) {
                    assert_close(actual, expected);
                }
            }
        }
    }
}

#[test]
fn equal_rate_resampling_is_an_exact_identity() {
    let rate = SampleRate::new(48_000).unwrap();
    for mode in [InterpolationMode::Nearest, InterpolationMode::Linear] {
        for len in 1..=6 {
            for samples in exhaustive_signals(len) {
                assert_eq!(resample_mono(&samples, rate, rate, mode).unwrap(), samples);
            }
        }
    }
}

#[test]
fn decibel_conversion_round_trips_representative_amplitudes() {
    for linear in [0.001, 0.01, 0.1, 0.5, 1.0, 2.0, 10.0, 100.0] {
        let db = linear_to_db(linear).unwrap();
        assert_close(db_to_linear(db).unwrap(), linear);
    }
}
