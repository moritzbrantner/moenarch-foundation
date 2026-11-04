//! Registry-shaped consumer of the issue #149 runtime-onnx 0.1.2 release.
//!
//! It proves that a consumer of the exact candidate gets a typed
//! `RuntimeUnavailable` error, not a hang, when no ONNX Runtime dylib can be
//! loaded. Published 0.1.1 (ort 2.0.0-rc.12) deadlocked here instead.

use runtime_onnx::{OnnxRuntimeError, OnnxSession};

fn main() {
    let dir = std::env::temp_dir().join(format!(
        "runtime-onnx-0-1-2-consumer-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch directory");
    let model = dir.join("model.onnx");
    std::fs::write(&model, b"not a model").expect("write placeholder model");
    std::env::set_var("ORT_DYLIB_PATH", dir.join("libonnxruntime-missing.so"));

    let result = OnnxSession::from_file(&model);
    std::fs::remove_dir_all(&dir).ok();

    let Err(error) = result else {
        panic!("loading without an ONNX Runtime dylib must fail");
    };
    assert!(
        matches!(error, OnnxRuntimeError::RuntimeUnavailable { .. }),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("ORT_DYLIB_PATH"), "{message}");
}
