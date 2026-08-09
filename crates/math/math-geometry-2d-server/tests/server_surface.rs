#[test]
fn package_endpoint_reports_wrapped_library() {
    let response = math_geometry_2d_server::response_for("GET", "/api/package", "");
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("math-geometry-2d"));
}

#[test]
fn run_endpoint_calls_library_surface() {
    let response = math_geometry_2d_server::response_for(
        "POST",
        "/api/run",
        r#"{"operation":"describe","input":{"includeOperations":true}}"#,
    );
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains(r#""operation""#));
}

#[test]
fn run_endpoint_calls_new_operation() {
    let response = math_geometry_2d_server::response_for(
        "POST",
        "/api/run",
        r#"{"operation":"geometry.overlap","input":{"left":{"x":0.0,"y":0.0,"width":2.0,"height":2.0},"right":{"x":1.0,"y":1.0,"width":2.0,"height":2.0}}}"#,
    );
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains(r#""iou""#));
}
