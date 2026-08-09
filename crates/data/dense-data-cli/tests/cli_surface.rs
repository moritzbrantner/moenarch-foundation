#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(dense_data_cli::LIBRARY_CRATE, "dense-data");
    let surface = dense_data_cli::package_surface();
    assert_eq!(surface.library, "moenarch-dense-data");
    assert!(!surface.operations.is_empty());
}
