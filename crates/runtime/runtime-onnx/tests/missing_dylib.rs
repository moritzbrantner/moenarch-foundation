#![cfg(feature = "onnxruntime")]

// Its own test binary: the ONNX Runtime dylib is loaded once per process.
#[test]
fn missing_onnx_runtime_dylib_is_an_error_not_a_hang() {
    let dir = std::env::temp_dir().join(format!("runtime-onnx-missing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("model.onnx");
    std::fs::write(&model, b"not a model").unwrap();
    std::env::set_var("ORT_DYLIB_PATH", dir.join("libonnxruntime-missing.so"));

    let Err(error) = runtime_onnx::OnnxSession::from_file(&model) else {
        panic!("loading without an ONNX Runtime dylib must fail");
    };

    assert!(
        matches!(
            error,
            runtime_onnx::OnnxRuntimeError::RuntimeUnavailable { .. }
        ),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("ONNX Runtime unavailable"), "{message}");
    assert!(message.contains("ORT_DYLIB_PATH"), "{message}");
    std::fs::remove_dir_all(&dir).ok();
}
