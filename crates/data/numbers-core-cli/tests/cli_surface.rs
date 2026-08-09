#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(numbers_core_cli::LIBRARY_CRATE, "numbers-core");
    let surface = numbers_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-numbers-core");
    assert!(!surface.operations.is_empty());
}
