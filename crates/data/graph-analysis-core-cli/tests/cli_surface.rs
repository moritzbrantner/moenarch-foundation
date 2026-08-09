#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(
        graph_analysis_core_cli::LIBRARY_CRATE,
        "graph-analysis-core"
    );
    let surface = graph_analysis_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-graph-analysis-core");
    assert!(!surface.operations.is_empty());
}
