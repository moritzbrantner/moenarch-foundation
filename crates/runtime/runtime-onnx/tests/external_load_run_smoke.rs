#![cfg(feature = "external-tests")]

use std::path::PathBuf;

use runtime_onnx::{
    OnnxDimension, OnnxF32Tensor, OnnxI32Tensor, OnnxI64Tensor, OnnxNamedTensor, OnnxRunner,
    OnnxSession, OnnxTensorElementType, OnnxTensorValue,
};

#[test]
#[ignore = "requires local ONNX Runtime dylib and model bundle"]
fn runtime_onnx_loads_and_runs_local_model() {
    let Some(model_path) = smoke_model_path() else {
        eprintln!(
            "skipping runtime-onnx external smoke because RUNTIME_ONNX_SMOKE_MODEL is missing"
        );
        return;
    };

    let mut session = OnnxSession::from_file_cpu_single_threaded_with_diagnostics(&model_path)
        .expect("load ONNX session");
    let metadata = session.metadata().expect("inspect ONNX session metadata");
    assert!(!metadata.inputs.is_empty(), "model has no inputs");
    assert!(!metadata.outputs.is_empty(), "model has no outputs");

    let inputs = metadata
        .inputs
        .iter()
        .map(dummy_input)
        .collect::<Result<Vec<_>, _>>()
        .expect("build dummy inputs");
    let outputs = session.run(inputs).expect("run ONNX session");
    assert!(!outputs.is_empty(), "model returned no outputs");
    for output in &outputs {
        assert_finite_output(output);
    }
}

fn smoke_model_path() -> Option<PathBuf> {
    let path = std::env::var_os("RUNTIME_ONNX_SMOKE_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                ".model-runtime/roberta-base-squad2-onnx/main/files/onnx/model_quantized.onnx",
            )
        });
    path.is_file().then_some(path)
}

fn dummy_input(info: &runtime_onnx::OnnxIoInfo) -> runtime_onnx::Result<OnnxNamedTensor> {
    let shape = dummy_shape(info);
    let len = shape.iter().product::<usize>();
    let name = info.name.clone();
    let element_type = info
        .element_type
        .unwrap_or_else(|| inferred_element_type(&name));
    let tensor = match element_type {
        OnnxTensorElementType::F32 => {
            OnnxTensorValue::F32(OnnxF32Tensor::new(shape, vec![0.0; len])?)
        }
        OnnxTensorElementType::I64 => {
            let fill = if name.contains("attention_mask") {
                1
            } else {
                0
            };
            OnnxTensorValue::I64(OnnxI64Tensor::new(shape, vec![fill; len])?)
        }
        OnnxTensorElementType::I32 => {
            OnnxTensorValue::I32(OnnxI32Tensor::new(shape, vec![0; len])?)
        }
        OnnxTensorElementType::U8 => {
            OnnxTensorValue::U8(runtime_onnx::OnnxU8Tensor::new(shape, vec![0; len])?)
        }
    };
    Ok(OnnxNamedTensor { name, tensor })
}

fn dummy_shape(info: &runtime_onnx::OnnxIoInfo) -> Vec<usize> {
    if info.dimensions.is_empty() {
        return vec![1];
    }
    info.dimensions
        .iter()
        .enumerate()
        .map(|(index, dim)| match dim {
            OnnxDimension::Fixed(value) if *value > 0 => *value,
            _ if index == 0 => 1,
            _ => 8,
        })
        .collect()
}

fn inferred_element_type(name: &str) -> OnnxTensorElementType {
    if name.contains("input_ids")
        || name.contains("attention_mask")
        || name.contains("token_type_ids")
    {
        OnnxTensorElementType::I64
    } else {
        OnnxTensorElementType::F32
    }
}

fn assert_finite_output(output: &OnnxNamedTensor) {
    if let OnnxTensorValue::F32(tensor) = &output.tensor {
        assert!(
            tensor.values.iter().all(|value| value.is_finite()),
            "output {} contains non-finite f32 values",
            output.name
        );
    }
}
