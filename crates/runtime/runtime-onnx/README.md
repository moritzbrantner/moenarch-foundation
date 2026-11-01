# runtime-onnx

Domain-neutral ONNX Runtime session and tensor helpers for the workspace.

This crate owns direct `ort` integration. Task crates keep model downloads,
preprocessing, decoding, labels, spans, boxes, captions, embeddings, faces, and
poses.

`inspect_model_metadata(path)` performs static ONNX protobuf inspection without
requiring the `onnxruntime` feature. It reads only graph input/output names,
tensor element type metadata, and tensor dimensions; it does not construct an
ONNX Runtime session or execute a model.

`inspect_model_graph_diagnostics(path)` is a smoke-test diagnostic helper. It
uses the same static parser to report graph op counts, domains, opsets,
initializer count, and node count without constructing an ONNX Runtime session.

`from_file_cpu_single_threaded(path)` is the conservative ORT construction path
for local validation of sensitive models. It uses the CPU execution provider,
disables graph optimization, limits intra/inter threads to one, disables
parallel execution, and disables memory pattern optimization.

`from_file_cpu_single_threaded_with_diagnostics(path)` has the same runtime
behavior. Set `ONNX_RUNTIME_LOAD_DIAGNOSTICS=1` to print internal load stages
to stderr. Optional diagnostic-only knobs:

- `ONNX_RUNTIME_LOAD_MODE=file|memory`
- `ONNX_RUNTIME_LOG_LEVEL=verbose|info|warning|error|fatal`
- `ONNX_RUNTIME_LOG_VERBOSITY=N`
- `ONNX_RUNTIME_DISABLE_SPINNING=1`
- `ONNX_RUNTIME_OPTIMIZED_MODEL_PATH=/ignored/path/model.optimized.onnx`

ONNX Runtime is loaded dynamically from `ORT_DYLIB_PATH`, else from
`libonnxruntime.so` (`.dylib`/`.dll`) on the loader path, and must be at least
the release the linked `ort` API level requires (1.24). A library that cannot
be loaded returns `OnnxRuntimeError::RuntimeUnavailable`; an older one returns
`OnnxRuntimeError::RuntimeTooOld` with the found and required versions. Before
`ort` 2.0.0-rc.15 either case deadlocked inside `ort::api()` instead.
