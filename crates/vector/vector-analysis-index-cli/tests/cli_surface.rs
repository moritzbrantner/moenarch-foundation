#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(
        vector_analysis_index_cli::LIBRARY_CRATE,
        "vector-analysis-index"
    );
    let surface = vector_analysis_index_cli::package_surface();
    assert_eq!(surface.library, "moenarch-vector-analysis-index");
    assert!(!surface.operations.is_empty());
}
