#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(math_sparse_data_cli::LIBRARY_CRATE, "math-sparse-data");
    let surface = math_sparse_data_cli::package_surface();
    assert_eq!(surface.library, "moenarch-math-sparse-data");
    assert!(!surface.operations.is_empty());
}

#[test]
fn cli_adapter_runs_new_operation() {
    let response = math_sparse_data_cli::run_operation(
        "sparse.matrixStats",
        serde_json::json!({
            "matrix": {
                "rows": 3,
                "cols": 4,
                "entries": [[0, 1, 2.0], [1, 3, 4.0], [2, 1, -1.0]]
            }
        }),
    )
    .expect("run operation");
    assert_eq!(response.value["nnz"], 3);
}
