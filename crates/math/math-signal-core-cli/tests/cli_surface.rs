#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(math_signal_core_cli::LIBRARY_CRATE, "math-signal-core");
    let surface = math_signal_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-math-signal-core");
    assert!(!surface.operations.is_empty());
}

#[test]
fn cli_adapter_runs_new_operation() {
    let response = math_signal_core_cli::run_operation(
        "signal.levels",
        serde_json::json!({"samples": [0.0, 0.5, -1.0]}),
    )
    .expect("run operation");
    assert_eq!(response.value["count"], 3);
}
