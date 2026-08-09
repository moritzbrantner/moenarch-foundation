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

When using the `ort` `load-dynamic` feature, local smoke execution may require
an explicit `ORT_DYLIB_PATH`. On the 2026-06-10 smoke host, leaving
`ORT_DYLIB_PATH` unset hung before `onnxSessionBuilder=ok`, while pointing it
at the local `.audio-tools/whisperx-venv` ONNX Runtime 1.26.0 dylib allowed
both file and memory session commits to pass.
