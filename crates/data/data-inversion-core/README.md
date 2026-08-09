# data-inversion-core

Shared fidelity and inversion trace metadata for generated analysis outputs.

## Feature flags

- No optional feature flags today.

## Runtime Surface

- `inversion.trace` builds an inversion trace summary from JSON.
- `inversion.confidence` validates confidence values.
- `inversion.fidelity` returns the weaker of two fidelity values.

## Example

```rust,ignore
use data_inversion_core::{InformationFidelity, InversionTrace};

let trace = InversionTrace::new(InformationFidelity::Approximate)
    .with_assumption("interpolated from adjacent frames");

let _ = trace;
```

## Related crates

- `audio-analysis-synthesis`
- `image-analysis-synthesis`
- `text-generation`
