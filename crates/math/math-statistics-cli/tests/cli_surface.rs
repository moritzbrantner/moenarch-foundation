#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(math_statistics_cli::LIBRARY_CRATE, "math-statistics");
    let surface = math_statistics_cli::package_surface();
    assert_eq!(surface.library, "moenarch-math-statistics");
    assert!(!surface.operations.is_empty());
}

#[test]
fn cli_adapter_runs_new_operation() {
    let response = math_statistics_cli::run_operation(
        "stats.regression.diagnostics",
        serde_json::json!({
            "design": {"rows": 4, "cols": 2, "values": [1.0, 1.0, 1.0, 2.0, 1.0, 3.0, 1.0, 4.0]},
            "target": [3.0, 5.0, 7.0, 9.0],
            "tolerance": 0.0
        }),
    )
    .expect("run operation");
    assert_eq!(response.value["degreesOfFreedom"], 2);
}

#[test]
fn cli_adapter_runs_rank_deficient_ols() {
    let response = math_statistics_cli::run_operation(
        "stats.regression.ols",
        serde_json::json!({
            "design": {"rows": 4, "cols": 2, "values": [1.0, 2.0, 2.0, 4.0, 3.0, 6.0, 4.0, 8.0]},
            "target": [1.0, 2.0, 3.0, 4.0]
        }),
    )
    .expect("run operation");
    assert_eq!(response.value["precision"], "f64");
}
