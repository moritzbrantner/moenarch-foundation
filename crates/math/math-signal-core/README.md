# math-signal-core

Shared signal-domain math for windows, frame strides, resampling, filtering, level transforms, and deterministic sequence alignment. This crate is part of the Analytical Math Crates family.

## Highlights

- Checked sample-rate and resampling descriptors
- Shared window functions and frame/hop sizing
- Interpolation helpers for signal-domain consumers
- Reusable FIR and biquad coefficient contracts
- Signal level summaries, centered FIR application, and peak normalization
- Bounded-memory dynamic time warping with an optional Sakoe-Chiba band and explicit work evidence

## Dynamic time warping

`dynamic_time_warping` aligns two non-empty finite scalar signals using absolute sample difference as the local cost. The implementation retains only two dynamic-programming rows and chooses the shorter input as the row width, so auxiliary memory is `O(min(n, m))` rather than `O(n*m)`.

`DtwConfig::window` optionally applies a Sakoe-Chiba radius. A radius smaller than the two input lengths' difference is rejected instead of being silently widened. The returned `DtwReport` exposes total and normalized distance, deterministic path length, evaluated-cell count, working-set size, and the effective window so resource use is evidence rather than hidden policy. The first slice computes alignment distance and evidence; it deliberately does not retain a full warping path, which would defeat the bounded-memory contract.

## Example

```rust,no_run
use math_signal_core::{
    dynamic_time_warping, BiquadDesign, DtwConfig, FrameStride, SampleRate, WindowFunction,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = FrameStride::new(1024, 256)?;
    let weights = WindowFunction::Hann.weights(4);
    let coeffs = BiquadDesign::LowPass.design(SampleRate::new(48_000)?, 1_000.0, 0.707)?;
    let alignment = dynamic_time_warping(
        &[0.0, 1.0, 2.0],
        &[0.0, 2.0],
        DtwConfig { window: Some(1) },
    )?;
    assert_eq!(spec.frame_count(2_048), 5);
    assert!(weights[1] > 0.5);
    assert_eq!(alignment.distance, 1.0);
    coeffs.validate()?;
    Ok(())
}
```

## Package surface

Primary workflow: `signal.frames`.

Workflow operations:

- `signal.frames`: Computes frame count and preview mean/RMS summaries for a finite mono sample buffer.
- `signal.align`: Computes bounded-memory dynamic-time-warping distance between two finite mono sample buffers, with an optional Sakoe-Chiba window.
- `signal.filterDesign`: Designs normalized biquad coefficients for supported filter kinds.
- `signal.levels`: Computes peak, RMS, mean, and DC offset for a finite mono sample buffer.
- `signal.filterApply`: Applies a centered FIR kernel to a finite mono sample buffer.
- `signal.normalizePeak`: Scales a finite mono sample buffer to a requested peak amplitude.

Debug operations:

- `describe`: inspect package metadata and runtime support.
- `signal.resamplePlan`: Returns output length and source-position preview indices for a sample-rate conversion.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-math-signal-core-cli -- run \
  --operation signal.frames \
  --json '{"frameSize":2,"hopSize":1,"samples":[0.0,1.0,0.0,-1.0]}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `audio-analysis-core`
- `audio-analysis-processing`
- `audio-analysis-fourier`
