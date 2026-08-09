use serde::{Deserialize, Serialize};

/// Stable identifier for a curated type in the package landscape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct LandscapeTypeId(pub String);

impl LandscapeTypeId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LandscapeTypeId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for LandscapeTypeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Stable identifier for a curated function in the package landscape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct LandscapeFunctionId(pub String);

impl LandscapeFunctionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LandscapeFunctionId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for LandscapeFunctionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Reference to a curated type without depending on the owning domain crate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeTypeRef {
    pub id: LandscapeTypeId,
    pub owner: String,
    pub rust_type: Option<String>,
    pub schema_ref: Option<String>,
}

impl LandscapeTypeRef {
    pub fn new(id: impl Into<LandscapeTypeId>, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            rust_type: None,
            schema_ref: None,
        }
    }

    pub fn rust_type(mut self, value: impl Into<String>) -> Self {
        self.rust_type = Some(value.into());
        self
    }

    pub fn schema_ref(mut self, value: impl Into<String>) -> Self {
        self.schema_ref = Some(value.into());
        self
    }
}

/// A curated function input or output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LandscapePort {
    pub name: String,
    #[serde(rename = "typeRef")]
    pub type_ref: LandscapeTypeRef,
    pub required: bool,
    pub cardinality: LandscapeCardinality,
}

impl LandscapePort {
    pub fn new(name: impl Into<String>, type_ref: LandscapeTypeRef) -> Self {
        Self {
            name: name.into(),
            type_ref,
            required: true,
            cardinality: LandscapeCardinality::One,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self.cardinality = LandscapeCardinality::Optional;
        self
    }

    pub fn many(mut self) -> Self {
        self.cardinality = LandscapeCardinality::Many;
        self
    }
}

/// Cardinality of a curated function input or output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LandscapeCardinality {
    One,
    Optional,
    Many,
}

/// Curated function metadata for one operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeFunction {
    pub id: LandscapeFunctionId,
    pub owner: String,
    pub inputs: Vec<LandscapePort>,
    pub outputs: Vec<LandscapePort>,
    pub stability: LandscapeStability,
}

impl LandscapeFunction {
    pub fn new(id: impl Into<LandscapeFunctionId>, owner: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            stability: LandscapeStability::Stable,
        }
    }

    pub fn input(mut self, port: LandscapePort) -> Self {
        self.inputs.push(port);
        self
    }

    pub fn output(mut self, port: LandscapePort) -> Self {
        self.outputs.push(port);
        self
    }

    pub fn stability(mut self, stability: LandscapeStability) -> Self {
        self.stability = stability;
        self
    }
}

/// Release stability for curated landscape metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LandscapeStability {
    Stable,
    Experimental,
    Internal,
}

/// Landscape metadata attached to one surface operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LandscapeOperationContract {
    pub function: LandscapeFunction,
}

impl LandscapeOperationContract {
    pub fn new(function: LandscapeFunction) -> Self {
        Self { function }
    }
}

/// Returns known package owners for curated landscape metadata.
pub fn known_owner_packages() -> &'static [&'static str] {
    well_known::known_owner_packages()
}

/// Validates a curated landscape contract without checking domain semantics.
pub fn validate_landscape_contract(contract: &LandscapeOperationContract) -> Result<(), String> {
    validate_landscape_function(&contract.function)
}

/// Validates one curated function declaration.
pub fn validate_landscape_function(function: &LandscapeFunction) -> Result<(), String> {
    if function.id.as_str().trim().is_empty() {
        return Err("curated function id must not be empty".to_string());
    }
    validate_owner("curated function", &function.owner)?;
    if !matches!(function.stability, LandscapeStability::Internal) {
        if function.inputs.is_empty() {
            return Err(format!(
                "curated function `{}` must declare at least one input",
                function.id.as_str()
            ));
        }
        if function.outputs.is_empty() {
            return Err(format!(
                "curated function `{}` must declare at least one output",
                function.id.as_str()
            ));
        }
    }
    for port in function.inputs.iter().chain(function.outputs.iter()) {
        validate_port(function.id.as_str(), port)?;
    }
    Ok(())
}

fn validate_port(function_id: &str, port: &LandscapePort) -> Result<(), String> {
    if port.name.trim().is_empty() {
        return Err(format!(
            "curated function `{function_id}` has a port with an empty name"
        ));
    }
    if port.type_ref.id.as_str().trim().is_empty() {
        return Err(format!(
            "curated function `{function_id}` port `{}` has an empty type id",
            port.name
        ));
    }
    validate_owner(
        &format!("curated function `{function_id}` port `{}`", port.name),
        &port.type_ref.owner,
    )
}

fn validate_owner(context: &str, owner: &str) -> Result<(), String> {
    if owner.trim().is_empty() {
        return Err(format!("{context} owner must not be empty"));
    }
    if !known_owner_packages().contains(&owner) {
        return Err(format!("{context} owner `{owner}` is not known"));
    }
    Ok(())
}

/// Well-known curated type references for foundational contract owners.
pub mod well_known {
    use super::{LandscapeTypeId, LandscapeTypeRef};

    pub const OWNER_RUNTIME_CORE: &str = "moenarch-runtime-core";
    pub const OWNER_TEXT_CORE: &str = "moenarch-text-core";
    pub const OWNER_TEXT_TRANSCRIPTS: &str = "moenarch-text-transcripts";
    pub const OWNER_TEXT_ANALYSIS: &str = "moenarch-text-analysis";
    pub const OWNER_TEXT_RETRIEVAL: &str = "moenarch-text-retrieval";
    pub const OWNER_IMAGE_ANALYSIS_CORE: &str = "moenarch-image-analysis-core";
    pub const OWNER_IMAGE_ANALYSIS_DETECTION: &str = "moenarch-image-analysis-detection";
    pub const OWNER_AUDIO_ANALYSIS_CORE: &str = "moenarch-audio-analysis-core";
    pub const OWNER_AUDIO_ANALYSIS_TRANSCRIPTION: &str = "moenarch-audio-analysis-transcription";
    pub const OWNER_VISION_CORE: &str = "moenarch-vision-core";
    pub const OWNER_VECTOR_ANALYSIS_CORE: &str = "moenarch-vector-analysis-core";
    pub const OWNER_TENSOR_DATA: &str = "moenarch-tensor-data";
    pub const OWNER_NUMBERS_CORE: &str = "moenarch-numbers-core";
    pub const OWNER_MATH_GEOMETRY_2D: &str = "moenarch-math-geometry-2d";
    pub const OWNER_VIDEO_ANALYSIS_CORE: &str = "moenarch-video-analysis-core";
    pub const OWNER_VIDEO_ANALYSIS_DETECTORS: &str = "moenarch-video-analysis-detectors";
    pub const OWNER_VIDEO_ANALYSIS_OUTPUT: &str = "moenarch-video-analysis-output";
    pub const OWNER_VIDEO_ANALYSIS_RECONSTRUCTION: &str = "moenarch-video-analysis-reconstruction";
    pub const OWNER_VIDEO_ANALYSIS_SFM: &str = "moenarch-video-analysis-sfm";
    pub const OWNER_VIDEO_ANALYSIS_RADIANCE_FIELDS: &str =
        "moenarch-video-analysis-radiance-fields";
    pub const OWNER_VIDEO_ANALYSIS_RADIANCE_PIPELINE: &str =
        "moenarch-video-analysis-radiance-pipeline";

    pub const RUNTIME_SURFACE_REQUEST: &str = "runtime.surfaceRequest";
    pub const RUNTIME_SURFACE_RESPONSE: &str = "runtime.surfaceResponse";
    pub const TEXT_DOCUMENT: &str = "text.document";
    pub const TEXT_SEGMENT: &str = "text.segment";
    pub const TEXT_TRANSCRIPT_SEGMENT: &str = "text.transcriptSegment";
    pub const TEXT_ANALYSIS_REPORT: &str = "text.analysisReport";
    pub const TEXT_RETRIEVAL_QUERY: &str = "text.retrievalQuery";
    pub const TEXT_SEARCH_RESULT: &str = "text.searchResult";
    pub const IMAGE_IMAGE: &str = "image.image";
    pub const IMAGE_DETECTION_REQUEST: &str = "image.detectionRequest";
    pub const AUDIO_FRAME: &str = "audio.frame";
    pub const AUDIO_SOURCE: &str = "audio.source";
    pub const AUDIO_TRANSCRIPTION_CONFIG: &str = "audio.transcriptionConfig";
    pub const VISION_DETECTION: &str = "vision.detection";
    pub const VISION_EMBEDDING: &str = "vision.embedding";
    pub const VECTOR_VECTOR: &str = "vector.vector";
    pub const TENSOR_F32_TENSOR: &str = "tensor.f32Tensor";
    pub const NUMBERS_SUMMARY: &str = "numbers.summary";
    pub const GEOMETRY_RECT_U32: &str = "geometry.rectU32";
    pub const GEOMETRY_POINT2F: &str = "geometry.point2f";
    pub const VIDEO_TIMECODE: &str = "video.timecode";
    pub const VIDEO_FRAME: &str = "video.frame";
    pub const VIDEO_SCENE: &str = "video.scene";
    pub const VIDEO_SCENE_LIST: &str = "video.sceneList";
    pub const VIDEO_DETECTOR_CONFIG: &str = "video.detectorConfig";
    pub const VIDEO_CAMERA: &str = "video.camera";
    pub const VIDEO_CAMERA_PATH: &str = "video.cameraPath";
    pub const VIDEO_RECONSTRUCTION: &str = "video.reconstruction";
    pub const VIDEO_SFM_MATCH_PLAN: &str = "video.sfmMatchPlan";
    pub const VIDEO_RADIANCE_ASSET: &str = "video.radianceAsset";

    pub fn runtime_surface_request() -> LandscapeTypeRef {
        type_ref(
            RUNTIME_SURFACE_REQUEST,
            OWNER_RUNTIME_CORE,
            "runtime_core::SurfaceRequest",
        )
    }

    pub fn runtime_surface_response() -> LandscapeTypeRef {
        type_ref(
            RUNTIME_SURFACE_RESPONSE,
            OWNER_RUNTIME_CORE,
            "runtime_core::SurfaceResponse",
        )
    }

    pub fn text_document() -> LandscapeTypeRef {
        type_ref(
            TEXT_DOCUMENT,
            OWNER_TEXT_CORE,
            "text_core::TextDocumentContract",
        )
    }

    pub fn text_segment() -> LandscapeTypeRef {
        type_ref(
            TEXT_SEGMENT,
            OWNER_TEXT_CORE,
            "text_core::TextSegmentContract",
        )
    }

    pub fn text_transcript_segment() -> LandscapeTypeRef {
        type_ref(
            TEXT_TRANSCRIPT_SEGMENT,
            OWNER_TEXT_TRANSCRIPTS,
            "text_transcripts::TranscriptSegmentContract",
        )
    }

    pub fn text_analysis_report() -> LandscapeTypeRef {
        type_ref(
            TEXT_ANALYSIS_REPORT,
            OWNER_TEXT_ANALYSIS,
            "text_analysis::DocumentAnalysisReport",
        )
    }

    pub fn text_retrieval_query() -> LandscapeTypeRef {
        type_ref(
            TEXT_RETRIEVAL_QUERY,
            OWNER_TEXT_RETRIEVAL,
            "text_retrieval::SearchQuery",
        )
    }

    pub fn text_search_result() -> LandscapeTypeRef {
        type_ref(
            TEXT_SEARCH_RESULT,
            OWNER_TEXT_RETRIEVAL,
            "text_retrieval::SearchResult",
        )
    }

    pub fn image_image() -> LandscapeTypeRef {
        type_ref(
            IMAGE_IMAGE,
            OWNER_IMAGE_ANALYSIS_CORE,
            "image_analysis_core::OwnedImage",
        )
    }

    pub fn image_detection_request() -> LandscapeTypeRef {
        type_ref(
            IMAGE_DETECTION_REQUEST,
            OWNER_IMAGE_ANALYSIS_DETECTION,
            "image_analysis_detection::ImageDetectionRequest",
        )
    }

    pub fn audio_frame() -> LandscapeTypeRef {
        type_ref(
            AUDIO_FRAME,
            OWNER_AUDIO_ANALYSIS_CORE,
            "video_analysis_core::OwnedAudioFrame",
        )
    }

    pub fn audio_source() -> LandscapeTypeRef {
        type_ref(
            AUDIO_SOURCE,
            OWNER_AUDIO_ANALYSIS_TRANSCRIPTION,
            "audio_analysis_transcription::TranscriptionSource",
        )
    }

    pub fn audio_transcription_config() -> LandscapeTypeRef {
        type_ref(
            AUDIO_TRANSCRIPTION_CONFIG,
            OWNER_AUDIO_ANALYSIS_TRANSCRIPTION,
            "audio_analysis_transcription::TranscriptionPipelineRequest",
        )
    }

    pub fn vision_detection() -> LandscapeTypeRef {
        type_ref(
            VISION_DETECTION,
            OWNER_VISION_CORE,
            "vision_core::VisualDetection",
        )
    }

    pub fn vision_embedding() -> LandscapeTypeRef {
        type_ref(
            VISION_EMBEDDING,
            OWNER_VISION_CORE,
            "vision_core::VisualEmbedding",
        )
    }

    pub fn vector_vector() -> LandscapeTypeRef {
        type_ref(
            VECTOR_VECTOR,
            OWNER_VECTOR_ANALYSIS_CORE,
            "vector_analysis_core::DenseVector",
        )
    }

    pub fn tensor_f32_tensor() -> LandscapeTypeRef {
        type_ref(
            TENSOR_F32_TENSOR,
            OWNER_TENSOR_DATA,
            "tensor_data::F32Tensor",
        )
    }

    pub fn numbers_summary() -> LandscapeTypeRef {
        type_ref(
            NUMBERS_SUMMARY,
            OWNER_NUMBERS_CORE,
            "numbers_core::NumberSummary",
        )
    }

    pub fn geometry_rect_u32() -> LandscapeTypeRef {
        type_ref(
            GEOMETRY_RECT_U32,
            OWNER_MATH_GEOMETRY_2D,
            "math_geometry_2d::RectU32",
        )
    }

    pub fn geometry_point2f() -> LandscapeTypeRef {
        type_ref(
            GEOMETRY_POINT2F,
            OWNER_MATH_GEOMETRY_2D,
            "math_geometry_2d::Point2f",
        )
    }

    pub fn video_timecode() -> LandscapeTypeRef {
        type_ref(
            VIDEO_TIMECODE,
            OWNER_VIDEO_ANALYSIS_CORE,
            "video_analysis_core::FrameTimecode",
        )
    }

    pub fn video_frame() -> LandscapeTypeRef {
        type_ref(
            VIDEO_FRAME,
            OWNER_VIDEO_ANALYSIS_CORE,
            "video_analysis_core::VideoFrame",
        )
    }

    pub fn video_scene() -> LandscapeTypeRef {
        type_ref(
            VIDEO_SCENE,
            OWNER_VIDEO_ANALYSIS_CORE,
            "video_analysis_core::Scene",
        )
    }

    pub fn video_scene_list() -> LandscapeTypeRef {
        type_ref(
            VIDEO_SCENE_LIST,
            OWNER_VIDEO_ANALYSIS_OUTPUT,
            "video_analysis_output::SceneList",
        )
    }

    pub fn video_detector_config() -> LandscapeTypeRef {
        type_ref(
            VIDEO_DETECTOR_CONFIG,
            OWNER_VIDEO_ANALYSIS_DETECTORS,
            "video_analysis_detectors::WeightedCompositeDetector",
        )
    }

    pub fn video_camera() -> LandscapeTypeRef {
        type_ref(
            VIDEO_CAMERA,
            OWNER_VIDEO_ANALYSIS_RADIANCE_FIELDS,
            "video_analysis_radiance_fields::CameraView",
        )
    }

    pub fn video_camera_path() -> LandscapeTypeRef {
        type_ref(
            VIDEO_CAMERA_PATH,
            OWNER_VIDEO_ANALYSIS_RADIANCE_FIELDS,
            "video_analysis_radiance_fields::CameraPath",
        )
    }

    pub fn video_reconstruction() -> LandscapeTypeRef {
        type_ref(
            VIDEO_RECONSTRUCTION,
            OWNER_VIDEO_ANALYSIS_RECONSTRUCTION,
            "video_analysis_reconstruction::Reconstruction",
        )
    }

    pub fn video_sfm_match_plan() -> LandscapeTypeRef {
        type_ref(
            VIDEO_SFM_MATCH_PLAN,
            OWNER_VIDEO_ANALYSIS_SFM,
            "video_analysis_sfm::SfmRequest",
        )
    }

    pub fn video_radiance_asset() -> LandscapeTypeRef {
        type_ref(
            VIDEO_RADIANCE_ASSET,
            OWNER_VIDEO_ANALYSIS_RADIANCE_PIPELINE,
            "video_analysis_radiance_pipeline::RadianceAsset",
        )
    }

    pub fn known_owner_packages() -> &'static [&'static str] {
        &[
            OWNER_RUNTIME_CORE,
            OWNER_TEXT_CORE,
            OWNER_TEXT_TRANSCRIPTS,
            OWNER_TEXT_ANALYSIS,
            OWNER_TEXT_RETRIEVAL,
            OWNER_IMAGE_ANALYSIS_CORE,
            OWNER_IMAGE_ANALYSIS_DETECTION,
            OWNER_AUDIO_ANALYSIS_CORE,
            OWNER_AUDIO_ANALYSIS_TRANSCRIPTION,
            OWNER_VISION_CORE,
            OWNER_VECTOR_ANALYSIS_CORE,
            OWNER_TENSOR_DATA,
            OWNER_NUMBERS_CORE,
            OWNER_MATH_GEOMETRY_2D,
            OWNER_VIDEO_ANALYSIS_CORE,
            OWNER_VIDEO_ANALYSIS_DETECTORS,
            OWNER_VIDEO_ANALYSIS_OUTPUT,
            OWNER_VIDEO_ANALYSIS_RECONSTRUCTION,
            OWNER_VIDEO_ANALYSIS_SFM,
            OWNER_VIDEO_ANALYSIS_RADIANCE_FIELDS,
            OWNER_VIDEO_ANALYSIS_RADIANCE_PIPELINE,
        ]
    }

    pub fn known_type_ids() -> &'static [&'static str] {
        &[
            RUNTIME_SURFACE_REQUEST,
            RUNTIME_SURFACE_RESPONSE,
            TEXT_DOCUMENT,
            TEXT_SEGMENT,
            TEXT_TRANSCRIPT_SEGMENT,
            TEXT_ANALYSIS_REPORT,
            TEXT_RETRIEVAL_QUERY,
            TEXT_SEARCH_RESULT,
            IMAGE_IMAGE,
            IMAGE_DETECTION_REQUEST,
            AUDIO_FRAME,
            AUDIO_SOURCE,
            AUDIO_TRANSCRIPTION_CONFIG,
            VISION_DETECTION,
            VISION_EMBEDDING,
            VECTOR_VECTOR,
            TENSOR_F32_TENSOR,
            NUMBERS_SUMMARY,
            GEOMETRY_RECT_U32,
            GEOMETRY_POINT2F,
            VIDEO_TIMECODE,
            VIDEO_FRAME,
            VIDEO_SCENE,
            VIDEO_SCENE_LIST,
            VIDEO_DETECTOR_CONFIG,
            VIDEO_CAMERA,
            VIDEO_CAMERA_PATH,
            VIDEO_RECONSTRUCTION,
            VIDEO_SFM_MATCH_PLAN,
            VIDEO_RADIANCE_ASSET,
        ]
    }

    fn type_ref(
        id: &'static str,
        owner: &'static str,
        rust_type: &'static str,
    ) -> LandscapeTypeRef {
        LandscapeTypeRef::new(LandscapeTypeId::new(id), owner).rust_type(rust_type)
    }
}
