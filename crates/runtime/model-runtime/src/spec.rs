use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ModelPreset;

/// Documentation URL for the cuda-oxide compiler/runtime stack.
pub const CUDA_OXIDE_BOOK_URL: &str = "https://nvlabs.github.io/cuda-oxide/index.html";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Variants describing model task.
pub enum ModelTask {
    /// The object detection variant.
    ObjectDetection,
    /// The pose estimation2d variant.
    PoseEstimation2d,
    /// The pose lifting3d variant.
    PoseLifting3d,
    /// The image classification variant.
    ImageClassification,
    /// The image segmentation variant.
    ImageSegmentation,
    /// The image embedding variant.
    ImageEmbedding,
    /// The face detection variant.
    FaceDetection,
    /// The face embedding variant.
    FaceEmbedding,
    /// The OCR variant.
    Ocr,
    /// The audio classification variant.
    AudioClassification,
    /// The audio event detection variant.
    AudioEventDetection,
    /// The audio embedding variant.
    AudioEmbedding,
    /// The speech recognition variant.
    SpeechRecognition,
    /// The speaker diarization variant.
    SpeakerDiarization,
    /// The source separation variant.
    SourceSeparation,
    /// The audio generation variant.
    AudioGeneration,
    /// The speaker-conditioned text-to-speech variant.
    SpeakerConditionedTts,
    /// The text classification variant.
    TextClassification,
    /// The token classification variant.
    TokenClassification,
    /// The zero shot classification variant.
    ZeroShotClassification,
    /// The text embedding variant.
    TextEmbedding,
    /// The summarization variant.
    Summarization,
    /// The reranking variant.
    Reranking,
    /// The question answering variant.
    QuestionAnswering,
    /// The multimodal embedding variant.
    MultimodalEmbedding,
    /// The custom variant.
    Custom(String),
}

impl ModelTask {
    /// Returns default label.
    pub fn default_label(&self) -> &'static str {
        match self {
            Self::ObjectDetection => "object",
            Self::PoseEstimation2d => "pose_2d",
            Self::PoseLifting3d => "pose_3d",
            Self::ImageClassification => "scene",
            Self::ImageSegmentation => "mask",
            Self::ImageEmbedding => "image_embedding",
            Self::FaceDetection => "face",
            Self::FaceEmbedding => "face_embedding",
            Self::Ocr => "ocr",
            Self::AudioClassification => "audio_class",
            Self::AudioEventDetection => "audio_event",
            Self::AudioEmbedding => "audio_embedding",
            Self::SpeechRecognition => "speech",
            Self::SpeakerDiarization => "speaker",
            Self::SourceSeparation => "stem",
            Self::AudioGeneration => "audio_generation",
            Self::SpeakerConditionedTts => "speaker_conditioned_tts",
            Self::TextClassification => "semantic",
            Self::TokenClassification => "token",
            Self::ZeroShotClassification => "zero_shot",
            Self::TextEmbedding => "embedding",
            Self::Summarization => "summary",
            Self::Reranking => "reranking",
            Self::QuestionAnswering => "question_answering",
            Self::MultimodalEmbedding => "multimodal_embedding",
            Self::Custom(_) => "custom",
        }
    }

    /// Returns a stable protocol string for this task.
    pub fn as_protocol_str(&self) -> &str {
        match self {
            Self::ObjectDetection => "object_detection",
            Self::PoseEstimation2d => "pose_estimation_2d",
            Self::PoseLifting3d => "pose_lifting_3d",
            Self::ImageClassification => "image_classification",
            Self::ImageSegmentation => "image_segmentation",
            Self::ImageEmbedding => "image_embedding",
            Self::FaceDetection => "face_detection",
            Self::FaceEmbedding => "face_embedding",
            Self::Ocr => "ocr",
            Self::AudioClassification => "audio_classification",
            Self::AudioEventDetection => "audio_event_detection",
            Self::AudioEmbedding => "audio_embedding",
            Self::SpeechRecognition => "speech_recognition",
            Self::SpeakerDiarization => "speaker_diarization",
            Self::SourceSeparation => "source_separation",
            Self::AudioGeneration => "audio_generation",
            Self::SpeakerConditionedTts => "speaker_conditioned_tts",
            Self::TextClassification => "text_classification",
            Self::TokenClassification => "token_classification",
            Self::ZeroShotClassification => "zero_shot_classification",
            Self::TextEmbedding => "text_embedding",
            Self::Summarization => "summarization",
            Self::Reranking => "reranking",
            Self::QuestionAnswering => "question_answering",
            Self::MultimodalEmbedding => "multimodal_embedding",
            Self::Custom(kind) => kind.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Variants describing model file request.
pub enum ModelFileRequest {
    /// The required variant.
    Required(String),
    /// The optional variant.
    Optional(String),
    /// The first available variant.
    FirstAvailable(Vec<String>),
}

impl ModelFileRequest {
    /// Returns required.
    pub fn required(path: impl Into<String>) -> Self {
        Self::Required(path.into())
    }

    /// Returns optional.
    pub fn optional(path: impl Into<String>) -> Self {
        Self::Optional(path.into())
    }

    /// Returns first available.
    pub fn first_available(paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::FirstAvailable(paths.into_iter().map(Into::into).collect())
    }
}

/// Model source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModelSource {
    /// Hugging Face model repository.
    HuggingFace { repo_id: String, revision: String },
    /// Local model path.
    LocalPath { path: PathBuf },
    /// External command model runner.
    ExternalCommand { command: PathBuf },
    /// ComfyUI inventory root.
    ComfyUiInventory { root: PathBuf },
    /// Caller-defined source.
    Custom(String),
}

impl ModelSource {
    /// Returns a stable source kind.
    pub fn kind(&self) -> &str {
        match self {
            Self::HuggingFace { .. } => "hugging_face",
            Self::LocalPath { .. } => "local_path",
            Self::ExternalCommand { .. } => "external_command",
            Self::ComfyUiInventory { .. } => "comfyui_inventory",
            Self::Custom(kind) => kind.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Generic model spec.
pub struct ModelSpec {
    /// Human-readable name for this value.
    pub name: String,
    /// The task value.
    pub task: ModelTask,
    /// The model source value.
    pub source: ModelSource,
    /// The files value.
    pub files: Vec<ModelFileRequest>,
    /// Caller-defined model metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    /// Deprecated compatibility field for Hugging Face model identifiers.
    #[serde(default)]
    #[deprecated(note = "use ModelSpec::source or ModelSpec::repo_id() instead")]
    pub repo_id: String,
    /// Deprecated compatibility field for Hugging Face revisions.
    #[serde(default)]
    #[deprecated(note = "use ModelSpec::source or ModelSpec::revision() instead")]
    pub revision: String,
}

/// Hugging Face-oriented compatibility alias for callers migrating to `ModelSpec`.
pub type HuggingFaceModelSpec = ModelSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Variants describing model runtime backend.
pub enum ModelRuntimeBackend {
    /// The CPU variant.
    Cpu,
    /// The ONNX variant.
    Onnx,
    /// The candle variant.
    Candle,
    /// The cuda oxide variant.
    CudaOxide,
    /// The whisper.cpp variant.
    WhisperCpp,
    /// The Demucs variant.
    Demucs,
    /// The OpenCV variant.
    OpenCv,
    /// The ComfyUI variant.
    ComfyUi,
    /// The external variant.
    External,
    /// The heuristic variant.
    Heuristic,
    /// Imported caller predictions.
    Imported,
    /// The custom variant.
    Custom(String),
}

impl ModelRuntimeBackend {
    /// Borrows this value as a str.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cpu => "cpu",
            Self::Onnx => "onnx",
            Self::Candle => "candle",
            Self::CudaOxide => "cuda_oxide",
            Self::WhisperCpp => "whisper_cpp",
            Self::Demucs => "demucs",
            Self::OpenCv => "opencv",
            Self::ComfyUi => "comfyui",
            Self::External => "external",
            Self::Heuristic => "heuristic",
            Self::Imported => "imported",
            Self::Custom(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// General runtime preference for domain APIs.
pub enum RuntimePreference {
    /// Choose the best available runtime.
    Auto,
    /// Prefer native execution.
    Native,
    /// Prefer external execution.
    External,
    /// Force heuristic execution.
    Heuristic,
    /// Use imported predictions.
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
/// General fallback behavior for model-backed operations.
pub enum FallbackPolicy {
    /// Return a typed error.
    #[default]
    Error,
    /// Use a fast deterministic fallback.
    FastFallback,
    /// Use a heuristic fallback.
    HeuristicFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Advanced generic runtime selection.
pub struct ModelRuntimeSelection {
    /// Optional concrete model spec.
    #[serde(default)]
    pub model: Option<ModelSpec>,
    /// Optional preferred backend.
    #[serde(default)]
    pub backend: Option<ModelRuntimeBackend>,
    /// Optional bundle directory.
    #[serde(default)]
    pub bundle_dir: Option<PathBuf>,
    /// Fallback policy.
    #[serde(default)]
    pub fallback_policy: FallbackPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for cuda-oxide runtime config.
pub struct CudaOxideRuntimeConfig {
    /// The CUDA device index value.
    pub device_index: u32,
    /// Optional target SM architecture, such as `sm_80`.
    pub target_sm: Option<String>,
    /// Cargo subcommand used to build and run cuda-oxide kernels.
    pub cargo_oxide_command: String,
    /// The cuda-oxide documentation URL used for operator setup.
    pub documentation_url: String,
}

impl Default for CudaOxideRuntimeConfig {
    fn default() -> Self {
        Self {
            device_index: 0,
            target_sm: None,
            cargo_oxide_command: "cargo oxide".to_string(),
            documentation_url: CUDA_OXIDE_BOOK_URL.to_string(),
        }
    }
}

impl CudaOxideRuntimeConfig {
    /// Creates a new value.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns device index.
    pub fn device_index(mut self, value: u32) -> Self {
        self.device_index = value;
        self
    }

    /// Returns target SM.
    pub fn target_sm(mut self, value: impl Into<String>) -> Self {
        self.target_sm = Some(value.into());
        self
    }

    /// Returns cargo oxide command.
    pub fn cargo_oxide_command(mut self, value: impl Into<String>) -> Self {
        self.cargo_oxide_command = value.into();
        self
    }

    /// Returns runtime attributes for prediction metadata and traces.
    pub fn attributes(&self) -> BTreeMap<String, String> {
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "runtime.backend".to_string(),
            ModelRuntimeBackend::CudaOxide.as_str().to_string(),
        );
        attributes.insert(
            "runtime.cuda.device_index".to_string(),
            self.device_index.to_string(),
        );
        attributes.insert(
            "runtime.cuda_oxide.command".to_string(),
            self.cargo_oxide_command.clone(),
        );
        attributes.insert(
            "runtime.cuda_oxide.docs".to_string(),
            self.documentation_url.clone(),
        );
        if let Some(target_sm) = &self.target_sm {
            attributes.insert("runtime.cuda.target_sm".to_string(), target_sm.clone());
        }
        attributes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for cuda-oxide model plan.
pub struct CudaOxideModelPlan {
    /// The model spec value.
    pub spec: ModelSpec,
    /// The runtime value.
    pub runtime: CudaOxideRuntimeConfig,
    /// The cuda-oxide module name value.
    pub module_name: String,
    /// The kernel names value.
    pub kernel_names: Vec<String>,
}

impl CudaOxideModelPlan {
    /// Creates a new value.
    pub fn new(
        spec: ModelSpec,
        module_name: impl Into<String>,
        kernel_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            spec,
            runtime: CudaOxideRuntimeConfig::default(),
            module_name: module_name.into(),
            kernel_names: kernel_names.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns runtime.
    pub fn runtime(mut self, runtime: CudaOxideRuntimeConfig) -> Self {
        self.runtime = runtime;
        self
    }

    /// Returns attributes for prediction metadata and traces.
    pub fn attributes(&self) -> BTreeMap<String, String> {
        let mut attributes = self.runtime.attributes();
        attributes.insert(
            "runtime.cuda_oxide.module".to_string(),
            self.module_name.clone(),
        );
        if !self.kernel_names.is_empty() {
            attributes.insert(
                "runtime.cuda_oxide.kernels".to_string(),
                self.kernel_names.join(","),
            );
        }
        attributes
    }
}

#[allow(deprecated)]
impl ModelSpec {
    /// Creates a Hugging Face-backed model spec.
    pub fn new(repo_id: impl Into<String>, task: ModelTask) -> Self {
        let repo_id = repo_id.into();
        let revision = "main".to_string();
        Self {
            name: repo_id.clone(),
            task,
            source: ModelSource::HuggingFace {
                repo_id: repo_id.clone(),
                revision: revision.clone(),
            },
            files: Vec::new(),
            metadata: BTreeMap::new(),
            repo_id,
            revision,
        }
    }

    /// Creates a model spec from explicit source.
    pub fn from_source(name: impl Into<String>, task: ModelTask, source: ModelSource) -> Self {
        let name = name.into();
        let (repo_id, revision) = match &source {
            ModelSource::HuggingFace { repo_id, revision } => (repo_id.clone(), revision.clone()),
            _ => (String::new(), String::new()),
        };
        Self {
            name,
            task,
            source,
            files: Vec::new(),
            metadata: BTreeMap::new(),
            repo_id,
            revision,
        }
    }

    /// Returns the Hugging Face repo id when this spec has one.
    pub fn repo_id_value(&self) -> Option<&str> {
        match &self.source {
            ModelSource::HuggingFace { repo_id, .. } => Some(repo_id.as_str()),
            _ if !self.repo_id.is_empty() => Some(self.repo_id.as_str()),
            _ => None,
        }
    }

    /// Returns the revision when this spec has one.
    pub fn revision_value(&self) -> Option<&str> {
        match &self.source {
            ModelSource::HuggingFace { revision, .. } => Some(revision.as_str()),
            _ if !self.revision.is_empty() => Some(self.revision.as_str()),
            _ => None,
        }
    }

    /// Returns a filesystem-safe model name segment.
    pub fn safe_name(&self) -> String {
        self.name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Builds this value from preset.
    pub fn from_preset(preset: ModelPreset) -> Self {
        preset.spec()
    }

    /// Returns name.
    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.name = value.into();
        self
    }

    /// Returns revision.
    pub fn revision(mut self, value: impl Into<String>) -> Self {
        let revision = value.into();
        if let ModelSource::HuggingFace {
            revision: source_revision,
            ..
        } = &mut self.source
        {
            *source_revision = revision.clone();
        }
        self.revision = revision;
        self
    }

    /// Returns file.
    pub fn file(mut self, path: impl Into<String>) -> Self {
        self.files.push(ModelFileRequest::required(path));
        self
    }

    /// Returns optional file.
    pub fn optional_file(mut self, path: impl Into<String>) -> Self {
        self.files.push(ModelFileRequest::optional(path));
        self
    }

    /// Returns first available file.
    pub fn first_available_file(
        mut self,
        paths: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.files.push(ModelFileRequest::first_available(paths));
        self
    }

    /// Builds a cuda-oxide runtime plan for this model spec.
    pub fn cuda_oxide_plan(
        self,
        module_name: impl Into<String>,
        kernel_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> CudaOxideModelPlan {
        CudaOxideModelPlan::new(self, module_name, kernel_names)
    }
}
