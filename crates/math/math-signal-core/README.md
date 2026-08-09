# math-signal-core

Shared signal-domain math for windows, frame strides, resampling, and biquad
design. This crate is part of the Analytical Math Crates family.

## Highlights

- Checked sample-rate and resampling descriptors
- Shared window functions and frame/hop sizing
- Interpolation helpers for signal-domain consumers
- Reusable FIR and biquad coefficient contracts
- Signal level summaries, centered FIR application, and peak normalization

## Example

```rust,no_run
use math_signal_core::{BiquadDesign, FrameStride, SampleRate, WindowFunction};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = FrameStride::new(1024, 256)?;
    let weights = WindowFunction::Hann.weights(4);
    let coeffs = BiquadDesign::LowPass.design(SampleRate::new(48_000)?, 1_000.0, 0.707)?;
    assert_eq!(spec.frame_count(2_048), 5);
    assert!(weights[1] > 0.5);
    coeffs.validate()?;
    Ok(())
}
```

## Package surface

Primary workflow: `signal.frames`.

Workflow operations:

- `signal.frames`: Computes frame count and preview mean/RMS summaries for a finite mono sample buffer.
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
