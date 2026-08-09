#[test]
fn package_endpoint_reports_wrapped_library() {
    let response = math_sparse_data_server::response_for("GET", "/api/package", "");
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains("math-sparse-data"));
}

#[test]
fn run_endpoint_calls_library_surface() {
    let response = math_sparse_data_server::response_for(
        "POST",
        "/api/run",
        r#"{"operation":"describe","input":{"includeOperations":true}}"#,
    );
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains(r#""operation""#));
}

#[test]
fn run_endpoint_calls_new_operation() {
    let response = math_sparse_data_server::response_for(
        "POST",
        "/api/run",
        r#"{"operation":"sparse.matrixStats","input":{"matrix":{"rows":3,"cols":4,"entries":[[0,1,2.0],[1,3,4.0],[2,1,-1.0]]}}}"#,
    );
    assert_eq!(response.status_code, 200);
    assert!(response.body.contains(r#""nnz":3"#));
}
