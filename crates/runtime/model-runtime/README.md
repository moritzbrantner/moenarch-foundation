# model-runtime

Generic model infrastructure for the workspace.

This crate owns model identity, sources, bundle materialization, downloads,
model-specific artifact metadata, and job helpers. Generic artifact storage and
validation live in `moritzbrantner-jobs-core`; model bundles and Hugging Face download logic
stay here. Domain crates should hide this layer behind operations such as object
detection, transcription, OCR, segmentation, embeddings, and classification.

## Runtime Surface

- `model.executionPlan` validates a `ModelAccessJobRequest` and returns a pure
  job/access plan with runtime execution metadata.
- `model.bundlePlan` plans bundle manifest layout and artifact references
  without downloading or materializing files.
- `model.jobManifest` projects a planned model access job into a deterministic
  `JobManifest`.
- `model.presets` lists preset ids and model spec summaries.
- `model.spec` validates a model spec and returns safe names, task, source,
  requested files, and revision.

Default surface operations are deterministic and planning-only. They do not
download models, spawn background jobs, run native inference, execute external
commands, write files, or access the network.

## Local ONNX Defaults

`moritzbrantner-model-runtime` owns materialization for the server-only local
model workflows used by text QA, image classification, and image captioning.
The current ONNX-first presets are:

- `roberta-base-squad2-onnx`:
  `onnx-community/roberta-base-squad2-ONNX`.
- `vit-base-patch16-224-onnx`: `Xenova/vit-base-patch16-224`.
- `vit-gpt2-image-captioning-onnx`:
  `Xenova/vit-gpt2-image-captioning`.

Use `resolve_or_download_bundle` when a workflow should load an existing
bundle first and auto-download missing files into `.model-runtime` on native
server/CLI runtimes. WASM callers should keep using validation/import paths.
