//! Library-owned runtime surface for `math-geometry-2d`.

use runtime_core::{
    describe_surface_response, parse_surface_input, structured_operation_response,
    surface_operation, validate_max_items, OperationId, PackageSurface, RuntimeCapabilities,
    SurfaceError, SurfaceOperation, SurfaceRequest, SurfaceResponse,
};
use serde::Deserialize;

use crate::{Affine2, Bounds2f, LineSegment2, Point2f, Polygon2f, RectF32};

const MAX_VALUES: usize = 100_000;

/// Returns the package surface exposed by every transport wrapper.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            operation(
                "describe",
                "Describe package",
                "Shared 2D geometry contracts for multimodal image, video, and layout processing.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "geometry.bounds",
                "Point bounds",
                "Computes 2D bounds, dimensions, and center for finite points.",
                serde_json::json!({"points": [[0.0, 1.0], [2.0, 3.0]]}),
            ),
            operation(
                "geometry.transform",
                "Transform points",
                "Applies an affine transform to finite 2D points and returns transformed bounds.",
                serde_json::json!({
                    "points": [[1.0, 2.0]],
                    "transform": {"m11": 1.0, "m12": 0.0, "m21": 0.0, "m22": 1.0, "tx": 2.0, "ty": 3.0}
                }),
            ),
            operation(
                "geometry.intersections",
                "Rectangle intersections",
                "Checks every rectangle pair and returns intersection rectangles when present.",
                serde_json::json!({"rects": [{"x": 0.0, "y": 0.0, "width": 2.0, "height": 2.0}]}),
            ),
            operation(
                "geometry.overlap",
                "Rectangle overlap",
                "Computes intersection area, IoU, and directional overlap ratios for two rectangles.",
                serde_json::json!({"left": {"x": 0.0, "y": 0.0, "width": 2.0, "height": 2.0}, "right": {"x": 1.0, "y": 1.0, "width": 2.0, "height": 2.0}}),
            ),
            operation(
                "geometry.segmentIntersection",
                "Segment intersection",
                "Computes the point and segment parameters for two finite 2D segment intersections.",
                serde_json::json!({"left": {"start": [0.0, 0.0], "end": [2.0, 2.0]}, "right": {"start": [0.0, 2.0], "end": [2.0, 0.0]}}),
            ),
            operation(
                "geometry.polygonSummary",
                "Polygon summary",
                "Reports area, winding, centroid, and bounds for a finite 2D polygon.",
                serde_json::json!({"points": [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]]}),
            ),
        ],
    }
}

fn operation(
    id: &str,
    name: &str,
    description: &str,
    example_request: serde_json::Value,
) -> SurfaceOperation {
    let mut operation = surface_operation(id, name, description, example_request);
    if id == "geometry.transform" {
        runtime_core::attach_landscape_contract(
            &mut operation,
            runtime_core::landscape::LandscapeOperationContract::new(
                runtime_core::landscape::LandscapeFunction::new(
                    "geometry.transformPoints",
                    env!("CARGO_PKG_NAME"),
                )
                .input(
                    runtime_core::landscape::LandscapePort::new(
                        "points",
                        runtime_core::landscape::well_known::geometry_point2f(),
                    )
                    .many(),
                )
                .output(
                    runtime_core::landscape::LandscapePort::new(
                        "points",
                        runtime_core::landscape::well_known::geometry_point2f(),
                    )
                    .many(),
                ),
            ),
        );
    }
    operation
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let surface = package_surface();
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => return Ok(describe_surface_response(&surface, request)),
        "geometry.bounds" => bounds_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "geometry.transform" => transform_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "geometry.intersections" => intersections_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "geometry.overlap" => overlap_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "geometry.segmentIntersection" => segment_intersection_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        "geometry.polygonSummary" => polygon_summary_value(
            operation.as_str(),
            parse_surface_input(Some(operation.as_str()), request.input)?,
        )?,
        operation => {
            return Err(
                SurfaceError::unsupported_operation(operation, env!("CARGO_PKG_NAME"))
                    .to_error_string(),
            );
        }
    };
    Ok(structured_operation_response(&surface, operation, value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PointsRequest {
    points: Vec<[f32; 2]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformRequest {
    points: Vec<[f32; 2]>,
    transform: Affine2,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntersectionsRequest {
    rects: Vec<RectF32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlapRequest {
    left: RectF32,
    right: RectF32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentEndpointRequest {
    start: [f32; 2],
    end: [f32; 2],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentIntersectionRequest {
    left: SegmentEndpointRequest,
    right: SegmentEndpointRequest,
}

fn bounds_value(operation: &str, request: PointsRequest) -> Result<serde_json::Value, String> {
    let points = points_from_arrays(operation, request.points)?;
    bounds_json(operation, bounds_for_points(operation, &points)?)
}

fn transform_value(
    operation: &str,
    request: TransformRequest,
) -> Result<serde_json::Value, String> {
    request
        .transform
        .validate()
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let points = points_from_arrays(operation, request.points)?;
    let transformed = points
        .into_iter()
        .map(|point| request.transform.apply_point(point))
        .collect::<Vec<_>>();
    let bounds = bounds_for_points(operation, &transformed)?;
    Ok(serde_json::json!({
        "determinant": request.transform.determinant().map_err(|error| invalid_request(operation, error.to_string()))?,
        "points": transformed.iter().map(point_array).collect::<Vec<_>>(),
        "bounds": bounds_json(operation, bounds)?
    }))
}

fn intersections_value(
    operation: &str,
    request: IntersectionsRequest,
) -> Result<serde_json::Value, String> {
    validate_max_items(operation, "rects", request.rects.len(), MAX_VALUES)?;
    for rect in &request.rects {
        rect.validate()
            .map_err(|error| invalid_request(operation, error.to_string()))?;
    }
    let mut pairs = Vec::new();
    for left_index in 0..request.rects.len() {
        for right_index in (left_index + 1)..request.rects.len() {
            let left = request.rects[left_index];
            let right = request.rects[right_index];
            let intersection = left
                .intersection(right)
                .map_err(|error| invalid_request(operation, error.to_string()))?;
            pairs.push(serde_json::json!({
                "left": left_index,
                "right": right_index,
                "intersects": intersection.is_some(),
                "intersection": intersection.map(rect_json)
            }));
        }
    }
    Ok(serde_json::json!({
        "rectCount": request.rects.len(),
        "pairs": pairs
    }))
}

fn overlap_value(operation: &str, request: OverlapRequest) -> Result<serde_json::Value, String> {
    request
        .left
        .validate()
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    request
        .right
        .validate()
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let intersection = request
        .left
        .intersection(request.right)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    let intersection_area = intersection
        .map(|rect| rect.area())
        .transpose()
        .map_err(|error| invalid_request(operation, error.to_string()))?
        .unwrap_or(0.0);
    Ok(serde_json::json!({
        "intersects": intersection.is_some(),
        "intersection": intersection.map(rect_json),
        "intersectionArea": intersection_area,
        "iou": request.left.iou(request.right).map_err(|error| invalid_request(operation, error.to_string()))?,
        "leftOverlapRatio": request.left.overlap_ratio(request.right).map_err(|error| invalid_request(operation, error.to_string()))?,
        "rightOverlapRatio": request.right.overlap_ratio(request.left).map_err(|error| invalid_request(operation, error.to_string()))?
    }))
}

fn segment_intersection_value(
    operation: &str,
    request: SegmentIntersectionRequest,
) -> Result<serde_json::Value, String> {
    let left = segment_from_request(operation, request.left)?;
    let right = segment_from_request(operation, request.right)?;
    let intersection = left
        .intersection(right)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "intersects": intersection.is_some(),
        "intersection": intersection.map(|value| serde_json::json!({
            "point": point_array(&value.point),
            "leftT": value.left_t,
            "rightT": value.right_t
        }))
    }))
}

fn polygon_summary_value(
    operation: &str,
    request: PointsRequest,
) -> Result<serde_json::Value, String> {
    let polygon = Polygon2f::new(points_from_arrays(operation, request.points)?)
        .map_err(|error| invalid_request(operation, error.to_string()))?;
    Ok(serde_json::json!({
        "pointCount": polygon.points().len(),
        "area": polygon.area().map_err(|error| invalid_request(operation, error.to_string()))?,
        "signedArea": polygon.signed_area().map_err(|error| invalid_request(operation, error.to_string()))?,
        "clockwise": polygon.is_clockwise().map_err(|error| invalid_request(operation, error.to_string()))?,
        "centroid": point_array(&polygon.centroid().map_err(|error| invalid_request(operation, error.to_string()))?),
        "bounds": bounds_json(operation, polygon.bounds().map_err(|error| invalid_request(operation, error.to_string()))?)?
    }))
}

fn points_from_arrays(operation: &str, points: Vec<[f32; 2]>) -> Result<Vec<Point2f>, String> {
    if points.is_empty() {
        return Err(invalid_request(operation, "points must not be empty"));
    }
    validate_max_items(operation, "points", points.len(), MAX_VALUES)?;
    points
        .into_iter()
        .map(|point| Point2f::new(point[0], point[1]))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid_request(operation, error.to_string()))
}

fn bounds_for_points(operation: &str, points: &[Point2f]) -> Result<Bounds2f, String> {
    Bounds2f::from_points(points).map_err(|error| invalid_request(operation, error.to_string()))
}

fn bounds_json(operation: &str, bounds: Bounds2f) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "min": point_array(&bounds.min),
        "max": point_array(&bounds.max),
        "width": bounds.width(),
        "height": bounds.height(),
        "center": point_array(&bounds.center().map_err(|error| invalid_request(operation, error.to_string()))?)
    }))
}

fn segment_from_request(
    operation: &str,
    request: SegmentEndpointRequest,
) -> Result<LineSegment2, String> {
    LineSegment2::new(
        Point2f::new(request.start[0], request.start[1])
            .map_err(|error| invalid_request(operation, error.to_string()))?,
        Point2f::new(request.end[0], request.end[1])
            .map_err(|error| invalid_request(operation, error.to_string()))?,
    )
    .map_err(|error| invalid_request(operation, error.to_string()))
}

fn point_array(point: &Point2f) -> [f32; 2] {
    [point.x, point.y]
}

fn rect_json(rect: RectF32) -> serde_json::Value {
    serde_json::json!({
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height
    })
}

fn invalid_request(operation: &str, message: impl Into<String>) -> String {
    SurfaceError::invalid_request(Some(OperationId::new(operation)), message).to_error_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_reports_dimensions_and_center() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("geometry.bounds"),
            input: serde_json::json!({"points": [[0.0, 1.0], [2.0, 5.0]]}),
        })
        .expect("bounds operation");

        assert_eq!(response.value["min"], serde_json::json!([0.0, 1.0]));
        assert_eq!(response.value["max"], serde_json::json!([2.0, 5.0]));
        assert_eq!(response.value["center"], serde_json::json!([1.0, 3.0]));
    }

    #[test]
    fn transform_applies_affine() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("geometry.transform"),
            input: serde_json::json!({
                "points": [[1.0, 2.0]],
                "transform": {"m11": 2.0, "m12": 0.0, "m21": 0.0, "m22": 3.0, "tx": 1.0, "ty": -1.0}
            }),
        })
        .expect("transform operation");

        assert_eq!(response.value["determinant"], 6.0);
        assert_eq!(response.value["points"], serde_json::json!([[3.0, 5.0]]));
    }

    #[test]
    fn intersections_reports_pairs() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("geometry.intersections"),
            input: serde_json::json!({"rects": [
                {"x": 0.0, "y": 0.0, "width": 2.0, "height": 2.0},
                {"x": 1.0, "y": 1.0, "width": 2.0, "height": 2.0}
            ]}),
        })
        .expect("intersections operation");

        assert_eq!(response.value["rectCount"], 2);
        assert_eq!(response.value["pairs"][0]["intersects"], true);
    }

    #[test]
    fn overlap_segment_and_polygon_operations_run() {
        for operation in [
            "geometry.overlap",
            "geometry.segmentIntersection",
            "geometry.polygonSummary",
        ] {
            let surface_operation = package_surface()
                .operations
                .into_iter()
                .find(|candidate| candidate.id.as_str() == operation)
                .expect("operation metadata");
            let response = run_surface_operation(SurfaceRequest {
                operation: surface_operation.id,
                input: surface_operation.example_request,
            })
            .unwrap_or_else(|error| panic!("{operation} failed: {error}"));
            assert!(response.value.is_object());
        }
    }
}
