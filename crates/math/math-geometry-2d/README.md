# math-geometry-2d

Shared 2D geometry contracts for multimodal image, video, and layout
processing. This crate is part of the Analytical Math Crates family.

## Highlights

- Checked pixel and normalized 2D primitives
- Rectangle intersection, union, and clamping helpers
- Rectangle IoU and directional overlap ratios
- Finite line-segment intersection parameters
- Affine transforms with compose and invert support
- Polygon area, centroid, winding, containment, and bounds helpers
- Normalized and pixel coordinate conversion helpers

## Example

```rust,no_run
use math_geometry_2d::{Affine2, NormalizedPoint2, RectU32, Size2u};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rect = RectU32::new(10, 12, 32, 24)?;
    let point = NormalizedPoint2::new(0.5, 0.5)?.to_pixel_point(Size2u::new(64, 48)?);
    let shifted = Affine2::translation(4.0, -2.0).apply_point(rect.center_f32());
    assert_eq!(point.x, 32);
    assert!(shifted.x > 0.0);
    Ok(())
}
```

## Package surface

Primary workflow: `geometry.bounds`.

Workflow operations:

- `geometry.bounds`: Computes 2D bounds, dimensions, and center for finite points.
- `geometry.transform`: Applies an affine transform to finite 2D points and returns transformed bounds.
- `geometry.intersections`: Checks every rectangle pair and returns intersection rectangles when present.
- `geometry.overlap`: Computes intersection area, IoU, and directional overlap ratios for two rectangles.
- `geometry.segmentIntersection`: Computes the point and segment parameters for two finite 2D segment intersections.
- `geometry.polygonSummary`: Reports area, winding, centroid, and bounds for a finite 2D polygon.

Debug operations:

- `describe`: inspect package metadata and runtime support.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-math-geometry-2d-cli -- run \
  --operation geometry.bounds \
  --json '{"points":[[0.0,1.0],[2.0,3.0]]}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `video-analysis-core`
- `image-analysis-processing`
- `video-analysis-posture`
