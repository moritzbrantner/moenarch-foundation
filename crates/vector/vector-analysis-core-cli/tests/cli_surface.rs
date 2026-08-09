#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(
        vector_analysis_core_cli::LIBRARY_CRATE,
        "vector-analysis-core"
    );
    let surface = vector_analysis_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-vector-analysis-core");
    assert!(!surface.operations.is_empty());
}
