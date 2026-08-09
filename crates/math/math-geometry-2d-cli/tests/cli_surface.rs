#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(math_geometry_2d_cli::LIBRARY_CRATE, "math-geometry-2d");
    let surface = math_geometry_2d_cli::package_surface();
    assert_eq!(surface.library, "moenarch-math-geometry-2d");
    assert!(!surface.operations.is_empty());
}

#[test]
fn cli_adapter_runs_new_operation() {
    let response = math_geometry_2d_cli::run_operation(
        "geometry.overlap",
        serde_json::json!({
            "left": {"x": 0.0, "y": 0.0, "width": 2.0, "height": 2.0},
            "right": {"x": 1.0, "y": 1.0, "width": 2.0, "height": 2.0}
        }),
    )
    .expect("run operation");
    assert!(response.value["iou"].as_f64().unwrap() > 0.0);
}
