#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(math_linear_cli::LIBRARY_CRATE, "math-linear");
    let surface = math_linear_cli::package_surface();
    assert_eq!(surface.library, "moenarch-math-linear");
    assert!(!surface.operations.is_empty());
}

#[test]
fn cli_adapter_runs_new_operation() {
    let response = math_linear_cli::run_operation(
        "linear.leastSquares",
        serde_json::json!({
            "matrix": {"rows": 3, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0]},
            "target": [3.0, 5.0, 7.0],
            "tolerance": 0.0
        }),
    )
    .expect("run operation");
    assert_eq!(response.value["rank"], 2);
}

#[test]
fn cli_adapter_runs_svd_operation() {
    let response = math_linear_cli::run_operation(
        "linear.svd",
        serde_json::json!({
            "matrix": {"rows": 2, "cols": 2, "values": [1.0, 0.0, 0.0, 2.0]},
            "computeFactors": true
        }),
    )
    .expect("run operation");
    assert_eq!(response.value["rank"], 2);
    assert_eq!(response.value["u"]["rows"], 2);
}
