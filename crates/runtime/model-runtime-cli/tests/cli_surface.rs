#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(model_runtime_cli::LIBRARY_CRATE, "model-runtime");
    let surface = model_runtime_cli::package_surface();
    assert_eq!(surface.library, "moenarch-model-runtime");
    assert!(!surface.operations.is_empty());
}
