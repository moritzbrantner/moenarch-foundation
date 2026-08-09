#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(
        data_inversion_core_cli::LIBRARY_CRATE,
        "data-inversion-core"
    );
    let surface = data_inversion_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-data-inversion-core");
    assert!(!surface.operations.is_empty());
}
