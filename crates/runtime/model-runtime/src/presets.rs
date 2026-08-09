use std::str::FromStr;

use crate::{HuggingFaceModelSpec, ModelRuntimeError, ModelTask, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing model preset.
pub enum ModelPreset {
    /// The detr resnet50 variant.
    DetrResnet50,
    /// The yolos tiny variant.
    YolosTiny,
    /// The distilbert sst2 variant.
    DistilbertSst2,
    /// The bert base ner variant.
    BertBaseNer,
    /// The mini lm l6 v2 variant.
    MiniLmL6V2,
    /// The xenova distilbert sst2 ONNX variant.
    XenovaDistilbertSst2Onnx,
    /// The xenova mini lm l6 v2 ONNX variant.
    XenovaMiniLmL6V2Onnx,
    /// The all mpnet base v2 ONNX variant.
    XenovaAllMpnetBaseV2Onnx,
    /// The twitter roberta sentiment ONNX variant.
    XenovaTwitterRobertaSentimentOnnx,
    /// The BART MNLI zero-shot ONNX variant.
    XenovaBartLargeMnliOnnx,
    /// The BART CNN summarization ONNX variant.
    XenovaBartLargeCnnOnnx,
    /// The MS MARCO MiniLM reranker ONNX variant.
    XenovaMsMarcoMiniLmL6V2Onnx,
    /// The RoBERTa SQuAD2 QA ONNX variant.
    XenovaRobertaBaseSquad2Onnx,
    /// The onnx-community RoBERTa SQuAD2 QA ONNX variant.
    OnnxCommunityRobertaBaseSquad2,
    /// The Xenova ViT base patch16 224 image classification ONNX variant.
    XenovaVitBasePatch16_224Onnx,
    /// The Xenova ViT-GPT2 image captioning ONNX variant.
    XenovaVitGpt2ImageCaptioningOnnx,
    /// The Xenova TrOCR printed text ONNX variant.
    XenovaTrocrBasePrintedOnnx,
    /// The Xenova TrOCR handwritten text ONNX variant.
    XenovaTrocrBaseHandwrittenOnnx,
    /// The xenova DETR ResNet-50 ONNX variant.
    XenovaDetrResnet50Onnx,
    /// The Xenova YOLOv8n pose ONNX variant.
    XenovaYolov8nPoseOnnx,
    /// The AST AudioSet classification variant.
    AstAudioset,
    /// The Xenova AST AudioSet ONNX variant.
    XenovaAstAudiosetOnnx,
    /// The CLAP audio embedding variant.
    ClapHtsatUnfused,
    /// The Whisper tiny English ASR variant.
    WhisperTinyEn,
    /// The Whisper large-v3 ASR variant.
    WhisperLargeV3,
    /// The Whisper large-v3-turbo ASR variant.
    WhisperLargeV3Turbo,
    /// The wav2vec2 base ASR variant.
    Wav2Vec2Base960h,
    /// The pyannote speaker diarization variant.
    PyannoteSpeakerDiarization31,
    /// The Demucs music separation variant.
    DemucsMusicSeparation,
    /// The MusicGen small generation variant.
    MusicgenSmall,
    /// The F5-TTS v1 base speaker-conditioned TTS variant.
    F5TtsV1Base,
    /// The F5-TTS base speaker-conditioned TTS variant.
    F5TtsBase,
    /// The E2-TTS base speaker-conditioned TTS variant.
    E2TtsBase,
    /// The Vocos 24 kHz mel vocoder variant.
    VocosMel24Khz,
}

impl ModelPreset {
    /// Constant for all.
    pub const ALL: &'static [Self] = &[
        Self::DetrResnet50,
        Self::YolosTiny,
        Self::DistilbertSst2,
        Self::BertBaseNer,
        Self::MiniLmL6V2,
        Self::XenovaDistilbertSst2Onnx,
        Self::XenovaMiniLmL6V2Onnx,
        Self::XenovaAllMpnetBaseV2Onnx,
        Self::XenovaTwitterRobertaSentimentOnnx,
        Self::XenovaBartLargeMnliOnnx,
        Self::XenovaBartLargeCnnOnnx,
        Self::XenovaMsMarcoMiniLmL6V2Onnx,
        Self::XenovaRobertaBaseSquad2Onnx,
        Self::OnnxCommunityRobertaBaseSquad2,
        Self::XenovaVitBasePatch16_224Onnx,
        Self::XenovaVitGpt2ImageCaptioningOnnx,
        Self::XenovaTrocrBasePrintedOnnx,
        Self::XenovaTrocrBaseHandwrittenOnnx,
        Self::XenovaDetrResnet50Onnx,
        Self::XenovaYolov8nPoseOnnx,
        Self::AstAudioset,
        Self::XenovaAstAudiosetOnnx,
        Self::ClapHtsatUnfused,
        Self::WhisperTinyEn,
        Self::WhisperLargeV3,
        Self::WhisperLargeV3Turbo,
        Self::Wav2Vec2Base960h,
        Self::PyannoteSpeakerDiarization31,
        Self::DemucsMusicSeparation,
        Self::MusicgenSmall,
        Self::F5TtsV1Base,
        Self::F5TtsBase,
        Self::E2TtsBase,
        Self::VocosMel24Khz,
    ];

    /// Borrows this value as a str.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DetrResnet50 => "detr-resnet-50",
            Self::YolosTiny => "yolos-tiny",
            Self::DistilbertSst2 => "distilbert-sst2",
            Self::BertBaseNer => "bert-base-ner",
            Self::MiniLmL6V2 => "minilm-l6-v2",
            Self::XenovaDistilbertSst2Onnx => "xenova-distilbert-sst2-onnx",
            Self::XenovaMiniLmL6V2Onnx => "xenova-minilm-l6-v2-onnx",
            Self::XenovaAllMpnetBaseV2Onnx => "xenova-all-mpnet-base-v2-onnx",
            Self::XenovaTwitterRobertaSentimentOnnx => {
                "xenova-twitter-roberta-sentiment-latest-onnx"
            }
            Self::XenovaBartLargeMnliOnnx => "xenova-bart-large-mnli-onnx",
            Self::XenovaBartLargeCnnOnnx => "xenova-bart-large-cnn-onnx",
            Self::XenovaMsMarcoMiniLmL6V2Onnx => "xenova-ms-marco-minilm-l6-v2-onnx",
            Self::XenovaRobertaBaseSquad2Onnx => "xenova-roberta-base-squad2-onnx",
            Self::OnnxCommunityRobertaBaseSquad2 => "roberta-base-squad2-onnx",
            Self::XenovaVitBasePatch16_224Onnx => "vit-base-patch16-224-onnx",
            Self::XenovaVitGpt2ImageCaptioningOnnx => "vit-gpt2-image-captioning-onnx",
            Self::XenovaTrocrBasePrintedOnnx => "trocr-base-printed-onnx",
            Self::XenovaTrocrBaseHandwrittenOnnx => "trocr-base-handwritten-onnx",
            Self::XenovaDetrResnet50Onnx => "xenova-detr-resnet-50-onnx",
            Self::XenovaYolov8nPoseOnnx => "xenova-yolov8n-pose-onnx",
            Self::AstAudioset => "ast-audioset",
            Self::XenovaAstAudiosetOnnx => "xenova-ast-audioset-onnx",
            Self::ClapHtsatUnfused => "clap-htsat-unfused",
            Self::WhisperTinyEn => "whisper-tiny-en",
            Self::WhisperLargeV3 => "whisper-large-v3",
            Self::WhisperLargeV3Turbo => "whisper-large-v3-turbo",
            Self::Wav2Vec2Base960h => "wav2vec2-base-960h",
            Self::PyannoteSpeakerDiarization31 => "pyannote-speaker-diarization-3-1",
            Self::DemucsMusicSeparation => "demucs-music-separation",
            Self::MusicgenSmall => "musicgen-small",
            Self::F5TtsV1Base => "f5-tts-v1-base",
            Self::F5TtsBase => "f5-tts-base",
            Self::E2TtsBase => "e2-tts-base",
            Self::VocosMel24Khz => "vocos-mel-24khz",
        }
    }

    /// Returns spec.
    pub fn spec(self) -> HuggingFaceModelSpec {
        match self {
            Self::DetrResnet50 => {
                HuggingFaceModelSpec::new("facebook/detr-resnet-50", ModelTask::ObjectDetection)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::YolosTiny => {
                HuggingFaceModelSpec::new("hustvl/yolos-tiny", ModelTask::ObjectDetection)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::DistilbertSst2 => HuggingFaceModelSpec::new(
                "distilbert-base-uncased-finetuned-sst-2-english",
                ModelTask::TextClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer_config.json")
            .file("vocab.txt")
            .first_available_file(["model.safetensors", "pytorch_model.bin"]),
            Self::BertBaseNer => {
                HuggingFaceModelSpec::new("dslim/bert-base-NER", ModelTask::TokenClassification)
                    .name(self.as_str())
                    .file("config.json")
                    .file("tokenizer_config.json")
                    .file("vocab.txt")
                    .optional_file("tokenizer.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::MiniLmL6V2 => HuggingFaceModelSpec::new(
                "sentence-transformers/all-MiniLM-L6-v2",
                ModelTask::TextEmbedding,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .file("vocab.txt")
            .file("modules.json")
            .optional_file("sentence_bert_config.json")
            .first_available_file(["model.safetensors", "pytorch_model.bin"]),
            Self::XenovaDistilbertSst2Onnx => HuggingFaceModelSpec::new(
                "Xenova/distilbert-base-uncased-finetuned-sst-2-english",
                ModelTask::TextClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .first_available_file([
                "onnx/model.onnx",
                "onnx/model_quantized.onnx",
                "onnx/model_int8.onnx",
            ]),
            Self::XenovaMiniLmL6V2Onnx => {
                HuggingFaceModelSpec::new("Xenova/all-MiniLM-L6-v2", ModelTask::TextEmbedding)
                    .name(self.as_str())
                    .file("config.json")
                    .file("tokenizer.json")
                    .file("tokenizer_config.json")
                    .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"])
            }
            Self::XenovaAllMpnetBaseV2Onnx => {
                HuggingFaceModelSpec::new("Xenova/all-mpnet-base-v2", ModelTask::TextEmbedding)
                    .name(self.as_str())
                    .file("config.json")
                    .file("tokenizer.json")
                    .file("tokenizer_config.json")
                    .optional_file("modules.json")
                    .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"])
            }
            Self::XenovaTwitterRobertaSentimentOnnx => HuggingFaceModelSpec::new(
                "Xenova/twitter-roberta-base-sentiment-latest",
                ModelTask::TextClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"]),
            Self::XenovaBartLargeMnliOnnx => HuggingFaceModelSpec::new(
                "Xenova/bart-large-mnli",
                ModelTask::ZeroShotClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .first_available_file([
                "onnx/model_quantized.onnx",
                "onnx/encoder_model.onnx",
                "onnx/model.onnx",
            ]),
            Self::XenovaBartLargeCnnOnnx => {
                HuggingFaceModelSpec::new("Xenova/bart-large-cnn", ModelTask::Summarization)
                    .name(self.as_str())
                    .file("config.json")
                    .file("tokenizer.json")
                    .file("tokenizer_config.json")
                    .first_available_file([
                        "onnx/encoder_model.onnx",
                        "onnx/model.onnx",
                        "onnx/model_quantized.onnx",
                    ])
            }
            Self::XenovaMsMarcoMiniLmL6V2Onnx => {
                HuggingFaceModelSpec::new("Xenova/ms-marco-MiniLM-L-6-v2", ModelTask::Reranking)
                    .name(self.as_str())
                    .file("config.json")
                    .file("tokenizer.json")
                    .file("tokenizer_config.json")
                    .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"])
            }
            Self::XenovaRobertaBaseSquad2Onnx => HuggingFaceModelSpec::new(
                "Xenova/roberta-base-squad2",
                ModelTask::QuestionAnswering,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"]),
            Self::OnnxCommunityRobertaBaseSquad2 => HuggingFaceModelSpec::new(
                "onnx-community/roberta-base-squad2-ONNX",
                ModelTask::QuestionAnswering,
            )
            .name(self.as_str())
            .file("config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .optional_file("vocab.json")
            .optional_file("merges.txt")
            .optional_file("special_tokens_map.json")
            .first_available_file([
                "onnx/model_quantized.onnx",
                "onnx/model.onnx",
                "onnx/model_uint8.onnx",
            ]),
            Self::XenovaVitBasePatch16_224Onnx => HuggingFaceModelSpec::new(
                "Xenova/vit-base-patch16-224",
                ModelTask::ImageClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("preprocessor_config.json")
            .first_available_file(["onnx/model_quantized.onnx", "onnx/model.onnx"]),
            Self::XenovaVitGpt2ImageCaptioningOnnx => HuggingFaceModelSpec::new(
                "Xenova/vit-gpt2-image-captioning",
                ModelTask::Custom("image_captioning".to_string()),
            )
            .name(self.as_str())
            .file("config.json")
            .file("generation_config.json")
            .file("preprocessor_config.json")
            .file("tokenizer.json")
            .file("tokenizer_config.json")
            .file("vocab.json")
            .file("merges.txt")
            .first_available_file([
                "onnx/encoder_model_quantized.onnx",
                "onnx/encoder_model.onnx",
            ])
            .first_available_file([
                "onnx/decoder_model_quantized.onnx",
                "onnx/decoder_model.onnx",
            ]),
            Self::XenovaTrocrBasePrintedOnnx | Self::XenovaTrocrBaseHandwrittenOnnx => {
                let repo_id = match self {
                    Self::XenovaTrocrBasePrintedOnnx => "Xenova/trocr-base-printed",
                    Self::XenovaTrocrBaseHandwrittenOnnx => "Xenova/trocr-base-handwritten",
                    _ => unreachable!(),
                };
                HuggingFaceModelSpec::new(
                    repo_id,
                    ModelTask::Custom("optical_character_recognition".to_string()),
                )
                .name(self.as_str())
                .file("config.json")
                .file("preprocessor_config.json")
                .file("tokenizer.json")
                .optional_file("generation_config.json")
                .optional_file("tokenizer_config.json")
                .optional_file("vocab.json")
                .optional_file("merges.txt")
                .first_available_file([
                    "onnx/encoder_model_quantized.onnx",
                    "onnx/encoder_model.onnx",
                    "onnx/encoder_model_fp16.onnx",
                ])
                .first_available_file([
                    "onnx/decoder_model_quantized.onnx",
                    "onnx/decoder_model.onnx",
                    "onnx/decoder_model_fp16.onnx",
                ])
            }
            Self::XenovaDetrResnet50Onnx => {
                HuggingFaceModelSpec::new("Xenova/detr-resnet-50", ModelTask::ObjectDetection)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"])
            }
            Self::XenovaYolov8nPoseOnnx => {
                HuggingFaceModelSpec::new("Xenova/yolov8n-pose", ModelTask::PoseEstimation2d)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .first_available_file([
                        "onnx/model_quantized.onnx",
                        "onnx/model_int8.onnx",
                        "onnx/model.onnx",
                    ])
            }
            Self::AstAudioset => HuggingFaceModelSpec::new(
                "MIT/ast-finetuned-audioset-10-10-0.4593",
                ModelTask::AudioClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("preprocessor_config.json")
            .first_available_file(["model.safetensors", "pytorch_model.bin"]),
            Self::XenovaAstAudiosetOnnx => HuggingFaceModelSpec::new(
                "Xenova/ast-finetuned-audioset-10-10-0.4593",
                ModelTask::AudioClassification,
            )
            .name(self.as_str())
            .file("config.json")
            .file("preprocessor_config.json")
            .first_available_file(["onnx/model.onnx", "onnx/model_quantized.onnx"]),
            Self::ClapHtsatUnfused => {
                HuggingFaceModelSpec::new("laion/clap-htsat-unfused", ModelTask::AudioEmbedding)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .optional_file("tokenizer.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::WhisperTinyEn => {
                HuggingFaceModelSpec::new("openai/whisper-tiny.en", ModelTask::SpeechRecognition)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .file("tokenizer.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::WhisperLargeV3 => {
                let mut spec = HuggingFaceModelSpec::new(
                    "openai/whisper-large-v3",
                    ModelTask::SpeechRecognition,
                )
                .name(self.as_str())
                .file("config.json")
                .file("generation_config.json")
                .file("tokenizer.json")
                .file("preprocessor_config.json")
                .file("model.safetensors");
                spec.metadata
                    .insert("backend".to_string(), "candle".to_string());
                spec
            }
            Self::WhisperLargeV3Turbo => {
                let mut spec = HuggingFaceModelSpec::new(
                    "openai/whisper-large-v3-turbo",
                    ModelTask::SpeechRecognition,
                )
                .name(self.as_str())
                .file("config.json")
                .file("generation_config.json")
                .file("tokenizer.json")
                .file("preprocessor_config.json")
                .file("model.safetensors");
                spec.metadata
                    .insert("backend".to_string(), "candle".to_string());
                spec
            }
            Self::Wav2Vec2Base960h => {
                let mut spec = HuggingFaceModelSpec::new(
                    "facebook/wav2vec2-base-960h",
                    ModelTask::Custom("forced_alignment".to_string()),
                )
                .name(self.as_str())
                .file("config.json")
                .file("preprocessor_config.json")
                .first_available_file(["tokenizer.json", "vocab.json"])
                .file("model.safetensors");
                spec.metadata
                    .insert("backend".to_string(), "candle".to_string());
                spec
            }
            Self::PyannoteSpeakerDiarization31 => {
                let mut spec = HuggingFaceModelSpec::new(
                    "pyannote/speaker-diarization-3.1",
                    ModelTask::SpeakerDiarization,
                )
                .name(self.as_str())
                .file("config.yaml")
                .optional_file("pytorch_model.bin");
                spec.metadata
                    .insert("backend".to_string(), "plan-only".to_string());
                spec
            }
            Self::DemucsMusicSeparation => {
                HuggingFaceModelSpec::new("facebook/demucs", ModelTask::SourceSeparation)
                    .name(self.as_str())
                    .file("config.json")
                    .first_available_file(["pytorch_model.bin", "model.safetensors"])
            }
            Self::MusicgenSmall => {
                HuggingFaceModelSpec::new("facebook/musicgen-small", ModelTask::AudioGeneration)
                    .name(self.as_str())
                    .file("config.json")
                    .file("preprocessor_config.json")
                    .file("tokenizer.json")
                    .first_available_file(["model.safetensors", "pytorch_model.bin"])
            }
            Self::F5TtsV1Base => tts_preset(
                HuggingFaceModelSpec::new("SWivid/F5-TTS", ModelTask::SpeakerConditionedTts)
                    .name(self.as_str())
                    .file("F5TTS_v1_Base/model_1250000.safetensors")
                    .file("F5TTS_v1_Base/vocab.txt"),
                "f5-tts",
                "F5-TTS v1 base",
                "cc-by-nc-4.0",
                "Creative Commons Attribution Non Commercial 4.0",
                "https://creativecommons.org/licenses/by-nc/4.0/",
            ),
            Self::F5TtsBase => tts_preset(
                HuggingFaceModelSpec::new("SWivid/F5-TTS", ModelTask::SpeakerConditionedTts)
                    .name(self.as_str())
                    .file("F5TTS_Base/model_1200000.safetensors")
                    .file("F5TTS_Base/vocab.txt"),
                "f5-tts",
                "F5-TTS base",
                "cc-by-nc-4.0",
                "Creative Commons Attribution Non Commercial 4.0",
                "https://creativecommons.org/licenses/by-nc/4.0/",
            ),
            Self::E2TtsBase => tts_preset(
                HuggingFaceModelSpec::new("SWivid/E2-TTS", ModelTask::SpeakerConditionedTts)
                    .name(self.as_str())
                    .file("E2TTS_Base/model_1200000.safetensors"),
                "e2-tts",
                "E2-TTS base",
                "cc-by-nc-4.0",
                "Creative Commons Attribution Non Commercial 4.0",
                "https://creativecommons.org/licenses/by-nc/4.0/",
            ),
            Self::VocosMel24Khz => tts_preset(
                HuggingFaceModelSpec::new("charactr/vocos-mel-24khz", ModelTask::AudioGeneration)
                    .name(self.as_str())
                    .file("config.yaml")
                    .file("pytorch_model.bin"),
                "vocos",
                "Vocos mel 24 kHz",
                "mit",
                "MIT",
                "https://opensource.org/license/mit/",
            ),
        }
    }
}

fn tts_preset(
    mut spec: HuggingFaceModelSpec,
    family: &str,
    display_name: &str,
    license: &str,
    license_name: &str,
    license_url: &str,
) -> HuggingFaceModelSpec {
    spec.metadata
        .insert("modelFamily".to_string(), family.to_string());
    spec.metadata
        .insert("displayName".to_string(), display_name.to_string());
    spec.metadata
        .insert("license".to_string(), license.to_string());
    spec.metadata
        .insert("licenseName".to_string(), license_name.to_string());
    spec.metadata
        .insert("licenseUrl".to_string(), license_url.to_string());
    spec.metadata
        .insert("licenseScope".to_string(), "model".to_string());
    spec.metadata
        .insert("explicitOptIn".to_string(), "true".to_string());
    spec
}

impl FromStr for ModelPreset {
    type Err = ModelRuntimeError;

    fn from_str(input: &str) -> Result<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|preset| preset.as_str() == input)
            .ok_or_else(|| {
                ModelRuntimeError::InvalidArgument(format!(
                    "unknown model preset `{input}`; expected one of {}",
                    Self::ALL
                        .iter()
                        .map(|preset| preset.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }
}
