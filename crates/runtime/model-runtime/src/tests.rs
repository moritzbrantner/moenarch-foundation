use std::collections::BTreeMap;

use tempfile::tempdir;

use crate::{
    resolve_or_download_bundle_with_downloader, DownloadedModel, HuggingFaceModelSpec, ModelBundle,
    ModelBundleResolveOptions, ModelBundleStore, ModelDownloader, ModelFileRequest, ModelPreset,
    ModelRuntimeBackend, ModelSource, ModelSpec, ModelTask, RawPrediction,
};

#[derive(Debug, Clone)]
struct FakeDownloader {
    root: std::path::PathBuf,
}

impl ModelDownloader for FakeDownloader {
    fn download_model(&self, spec: &HuggingFaceModelSpec) -> crate::Result<DownloadedModel> {
        std::fs::create_dir_all(&self.root)?;
        let mut files = BTreeMap::new();
        for request in &spec.files {
            let remote_path = match request {
                ModelFileRequest::Required(path) | ModelFileRequest::Optional(path) => path.clone(),
                ModelFileRequest::FirstAvailable(paths) => paths[0].clone(),
            };
            let local = self.root.join(remote_path.replace('/', "_"));
            std::fs::write(&local, b"fake")?;
            files.insert(remote_path, local);
        }
        Ok(DownloadedModel {
            spec: spec.clone(),
            files,
        })
    }
}

#[test]
fn model_spec_records_generic_source_and_compat_fields() {
    let spec = ModelSpec::new("owner/model", ModelTask::TextEmbedding)
        .revision("v1")
        .file("config.json");

    assert_eq!(spec.repo_id_value(), Some("owner/model"));
    assert_eq!(spec.revision_value(), Some("v1"));
    assert_eq!(spec.source.kind(), "hugging_face");
    assert_eq!(spec.files, vec![ModelFileRequest::required("config.json")]);

    let local = ModelSpec::from_source(
        "local-detector",
        ModelTask::ObjectDetection,
        ModelSource::Custom("fixture".to_string()),
    );
    assert_eq!(local.repo_id_value(), None);
    assert_eq!(local.source.kind(), "fixture");
}

#[test]
fn model_bundle_store_materializes_generic_manifest() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("config.json");
    std::fs::write(&source, br#"{"model_type":"fixture"}"#).unwrap();

    let downloaded = DownloadedModel {
        spec: ModelSpec::new("owner/model", ModelTask::TextClassification)
            .name("fixture-model")
            .file("config.json"),
        files: BTreeMap::from([("config.json".to_string(), source)]),
    };

    let bundle = ModelBundleStore::new(temp.path().join("bundles"))
        .materialize(&downloaded)
        .unwrap();

    assert_eq!(bundle.manifest.name, "fixture-model");
    assert_eq!(bundle.manifest.task, ModelTask::TextClassification);
    assert!(bundle.manifest_path().exists());
    assert!(bundle.file_path("config.json").unwrap().exists());

    let loaded = ModelBundle::load(bundle.manifest_path()).unwrap();
    assert_eq!(loaded.manifest, bundle.manifest);
}

#[test]
fn model_presets_include_local_onnx_defaults() {
    let ids = ModelPreset::ALL
        .iter()
        .map(|preset| preset.as_str())
        .collect::<Vec<_>>();

    assert!(ids.contains(&"roberta-base-squad2-onnx"));
    assert!(ids.contains(&"vit-base-patch16-224-onnx"));
    assert!(ids.contains(&"vit-gpt2-image-captioning-onnx"));
    assert!(ids.contains(&"xenova-yolov8n-pose-onnx"));
}

#[test]
fn tts_presets_are_explicit_speaker_conditioned_and_carry_license_metadata() {
    let f5_v1 = ModelPreset::F5TtsV1Base.spec();
    assert_eq!(f5_v1.repo_id, "SWivid/F5-TTS");
    assert_eq!(f5_v1.task, ModelTask::SpeakerConditionedTts);
    assert!(f5_v1.files.contains(&ModelFileRequest::required(
        "F5TTS_v1_Base/model_1250000.safetensors"
    )));
    assert!(f5_v1
        .files
        .contains(&ModelFileRequest::required("F5TTS_v1_Base/vocab.txt")));
    assert_eq!(f5_v1.metadata["license"], "cc-by-nc-4.0");
    assert_eq!(f5_v1.metadata["licenseScope"], "model");
    assert_eq!(f5_v1.metadata["explicitOptIn"], "true");

    let f5_base = ModelPreset::F5TtsBase.spec();
    assert_eq!(f5_base.repo_id, "SWivid/F5-TTS");
    assert_eq!(f5_base.task, ModelTask::SpeakerConditionedTts);
    assert!(f5_base.files.contains(&ModelFileRequest::required(
        "F5TTS_Base/model_1200000.safetensors"
    )));
    assert!(f5_base
        .files
        .contains(&ModelFileRequest::required("F5TTS_Base/vocab.txt")));
    assert_eq!(f5_base.metadata["license"], "cc-by-nc-4.0");

    let e2 = ModelPreset::E2TtsBase.spec();
    assert_eq!(e2.repo_id, "SWivid/E2-TTS");
    assert_eq!(e2.task, ModelTask::SpeakerConditionedTts);
    assert!(e2.files.contains(&ModelFileRequest::required(
        "E2TTS_Base/model_1200000.safetensors"
    )));
    assert_eq!(e2.metadata["license"], "cc-by-nc-4.0");

    let vocos = ModelPreset::VocosMel24Khz.spec();
    assert_eq!(vocos.repo_id, "charactr/vocos-mel-24khz");
    assert_eq!(vocos.task, ModelTask::AudioGeneration);
    assert!(vocos
        .files
        .contains(&ModelFileRequest::required("config.yaml")));
    assert!(vocos
        .files
        .contains(&ModelFileRequest::required("pytorch_model.bin")));
    assert_eq!(vocos.metadata["license"], "mit");
}

#[test]
fn yolov8n_pose_onnx_preset_uses_pose_bundle_layout() {
    let preset = ModelPreset::XenovaYolov8nPoseOnnx;
    let spec = preset.spec();

    assert_eq!(preset.as_str(), "xenova-yolov8n-pose-onnx");
    assert_eq!(spec.repo_id, "Xenova/yolov8n-pose");
    assert_eq!(spec.task, ModelTask::PoseEstimation2d);
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("config.json")));
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("preprocessor_config.json")));
    assert!(spec.files.contains(&ModelFileRequest::first_available([
        "onnx/model_quantized.onnx",
        "onnx/model_int8.onnx",
        "onnx/model.onnx",
    ])));
}

#[test]
fn wav2vec2_base_preset_accepts_vocab_json_tokenizer_layout() {
    let spec = ModelPreset::Wav2Vec2Base960h.spec();

    assert_eq!(spec.repo_id, "facebook/wav2vec2-base-960h");
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("config.json")));
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("preprocessor_config.json")));
    assert!(spec.files.contains(&ModelFileRequest::first_available([
        "tokenizer.json",
        "vocab.json"
    ])));
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("model.safetensors")));
}

#[test]
fn distilbert_sst2_preset_uses_wordpiece_vocab_layout() {
    let spec = ModelPreset::DistilbertSst2.spec();

    assert_eq!(
        spec.repo_id,
        "distilbert-base-uncased-finetuned-sst-2-english"
    );
    assert_eq!(spec.name, "distilbert-sst2");
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("config.json")));
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("tokenizer_config.json")));
    assert!(spec
        .files
        .contains(&ModelFileRequest::required("vocab.txt")));
    assert!(!spec
        .files
        .contains(&ModelFileRequest::required("tokenizer.json")));
    assert!(spec.files.contains(&ModelFileRequest::first_available([
        "model.safetensors",
        "pytorch_model.bin"
    ])));
}

#[test]
fn bart_mnli_onnx_preset_prefers_quantized_smoke_artifact() {
    let spec = ModelPreset::XenovaBartLargeMnliOnnx.spec();

    assert_eq!(spec.repo_id, "Xenova/bart-large-mnli");
    assert!(spec.files.contains(&ModelFileRequest::first_available([
        "onnx/model_quantized.onnx",
        "onnx/encoder_model.onnx",
        "onnx/model.onnx"
    ])));
}

#[test]
fn resolver_loads_existing_bundle_before_download() {
    let temp = tempdir().unwrap();
    let spec = ModelPreset::OnnxCommunityRobertaBaseSquad2.spec();
    let store =
        ModelBundleStore::new(temp.path().join("bundles")).model_downloader(FakeDownloader {
            root: temp.path().join("cache"),
        });
    let existing = store.download(&spec).unwrap();

    let bundle = resolve_or_download_bundle_with_downloader(
        &spec,
        &ModelBundleResolveOptions {
            bundle_root: temp.path().join("bundles"),
            auto_download: true,
            ..ModelBundleResolveOptions::default()
        },
        FakeDownloader {
            root: temp.path().join("unused-cache"),
        },
    )
    .unwrap();

    assert_eq!(bundle.manifest_path(), existing.manifest_path());
}

#[test]
fn resolver_reports_missing_bundle_when_download_disabled() {
    let temp = tempdir().unwrap();
    let spec = ModelPreset::XenovaVitBasePatch16_224Onnx.spec();

    let error = resolve_or_download_bundle_with_downloader(
        &spec,
        &ModelBundleResolveOptions {
            bundle_root: temp.path().join("bundles"),
            auto_download: false,
            ..ModelBundleResolveOptions::default()
        },
        FakeDownloader {
            root: temp.path().join("cache"),
        },
    )
    .expect_err("missing bundle");

    assert!(error.to_string().contains("autoDownload is false"));
    assert!(error.to_string().contains(spec.name.as_str()));
}

#[test]
fn resolver_fake_downloader_materializes_expected_manifest() {
    let temp = tempdir().unwrap();
    let spec = ModelPreset::XenovaVitGpt2ImageCaptioningOnnx.spec();

    let bundle = resolve_or_download_bundle_with_downloader(
        &spec,
        &ModelBundleResolveOptions {
            bundle_root: temp.path().join("bundles"),
            auto_download: true,
            ..ModelBundleResolveOptions::default()
        },
        FakeDownloader {
            root: temp.path().join("cache"),
        },
    )
    .unwrap();

    assert_eq!(bundle.manifest.repo_id, "Xenova/vit-gpt2-image-captioning");
    assert!(bundle.file_path("config.json").unwrap().exists());
    assert!(bundle
        .file_path("onnx/encoder_model_quantized.onnx")
        .unwrap()
        .exists());
    assert!(bundle
        .file_path("onnx/decoder_model_quantized.onnx")
        .unwrap()
        .exists());
}

#[cfg(feature = "jobs")]
#[test]
fn model_bundle_exports_generic_artifact_metadata() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("tokenizer.json");
    std::fs::write(&source, br#"{"tokenizer":"fixture"}"#).unwrap();

    let downloaded = DownloadedModel {
        spec: ModelSpec::new("owner/model", ModelTask::TextEmbedding)
            .name("fixture-model")
            .revision("v1")
            .file("tokenizer.json"),
        files: BTreeMap::from([("tokenizer.json".to_string(), source)]),
    };

    let bundle = ModelBundleStore::new(temp.path().join("bundles"))
        .materialize(&downloaded)
        .unwrap();
    let artifacts = bundle.artifact_refs();

    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].metadata["model.repoId"], "owner/model");
    assert_eq!(artifacts[0].metadata["model.revision"], "v1");
    assert_eq!(artifacts[0].metadata["model.task"], "text_embedding");
    assert_eq!(artifacts[0].metadata["model.fileRole"], "tokenizer");
}

#[test]
fn blue_green_prediction_check_remains_generic() {
    let green = vec![RawPrediction::label("positive", 0.9)];
    let blue = vec![RawPrediction::label("positive", 0.90001)];

    let report = crate::compare_blue_green_predictions(
        &green,
        &blue,
        crate::BlueGreenPredictionTestOptions {
            max_score_delta: 0.001,
            compare_regions: true,
        },
    )
    .unwrap();

    assert_eq!(report.compared_predictions, 1);
    assert!(ModelRuntimeBackend::Onnx.as_str().contains("onnx"));
}

#[cfg(feature = "jobs")]
#[test]
fn model_job_spec_records_standard_metadata() {
    let spec = ModelSpec::new("owner/model", ModelTask::TextClassification).revision("v1");
    let job = crate::jobs::model_job_spec(
        "job-1",
        crate::jobs::ModelJobKind::Download,
        &spec,
        ModelRuntimeBackend::Onnx,
    )
    .unwrap();

    assert_eq!(job.kind.as_deref(), Some("model-download"));
    assert_eq!(job.metadata["model.name"], "owner/model");
    assert_eq!(job.metadata["model.task"], "text_classification");
    assert_eq!(job.metadata["model.runtime"], "onnx");
    assert_eq!(job.metadata["model.revision"], "v1");
    assert_eq!(job.metadata["model.repoId"], "owner/model");
}

#[cfg(feature = "jobs")]
#[test]
fn model_access_job_request_records_standard_metadata() {
    use crate::jobs::{run_model_job_inline_for_tests, ModelAccessJobRequest, ModelJobKind};

    let request = ModelAccessJobRequest {
        id: Some("inline-model-job".to_string()),
        kind: ModelJobKind::Inference,
        spec: ModelSpec::new("owner/model", ModelTask::TextClassification).revision("v1"),
        backend: ModelRuntimeBackend::Heuristic,
        inputs: vec![crate::jobs::ModelJobInput::Json(
            serde_json::json!({"text": "hello"}),
        )],
        output_artifact_prefix: Some("prediction".to_string()),
        metadata: BTreeMap::from([("caller".to_string(), "test".to_string())]),
    };

    let result = run_model_job_inline_for_tests(request).unwrap();
    assert_eq!(result.job_id.as_str(), "inline-model-job");
    assert_eq!(result.kind, ModelJobKind::Inference);
    assert_eq!(result.backend, ModelRuntimeBackend::Heuristic);
    assert_eq!(result.output.unwrap()["inline"], true);
}
