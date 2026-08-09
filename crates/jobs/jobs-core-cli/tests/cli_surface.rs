#[test]
fn cli_adapter_reports_wrapped_library() {
    assert_eq!(jobs_core_cli::LIBRARY_CRATE, "jobs-core");
    let surface = jobs_core_cli::package_surface();
    assert_eq!(surface.library, "moenarch-jobs-core");
    assert!(!surface.operations.is_empty());
}
