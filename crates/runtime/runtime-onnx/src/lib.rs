#![doc = include_str!("../README.md")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "onnxruntime")]
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OnnxRuntimeError {
    #[error("ONNX Runtime support is unavailable in this build")]
    Unavailable,
    #[error("invalid ONNX runtime argument: {0}")]
    InvalidArgument(String),
    #[error("invalid ONNX tensor shape: {0}")]
    InvalidTensorShape(String),
    #[error("unsupported ONNX tensor type: {0}")]
    UnsupportedTensorType(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("ONNX Runtime source error: {0}")]
    Source(String),
}

pub type Result<T> = std::result::Result<T, OnnxRuntimeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnnxTensorElementType {
    F32,
    I64,
    I32,
    U8,
}

pub trait OnnxTensorElement: Clone + std::fmt::Debug + Send + Sync + 'static {
    fn validate_values(_values: &[Self]) -> Result<()> {
        Ok(())
    }
}

impl OnnxTensorElement for f32 {
    fn validate_values(values: &[Self]) -> Result<()> {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(OnnxRuntimeError::InvalidArgument(
                "f32 tensor values must be finite".to_string(),
            ));
        }
        Ok(())
    }
}

impl OnnxTensorElement for i64 {}
impl OnnxTensorElement for i32 {}
impl OnnxTensorElement for u8 {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnnxTensor<T> {
    pub shape: Vec<usize>,
    pub values: Vec<T>,
}

impl<T: OnnxTensorElement> OnnxTensor<T> {
    pub fn new(shape: Vec<usize>, values: Vec<T>) -> Result<Self> {
        let expected = element_count(&shape)?;
        if expected != values.len() {
            return Err(OnnxRuntimeError::InvalidTensorShape(format!(
                "shape {shape:?} expects {expected} value(s), got {}",
                values.len()
            )));
        }
        T::validate_values(&values)?;
        Ok(Self { shape, values })
    }
}

pub type OnnxF32Tensor = OnnxTensor<f32>;
pub type OnnxI64Tensor = OnnxTensor<i64>;
pub type OnnxI32Tensor = OnnxTensor<i32>;
pub type OnnxU8Tensor = OnnxTensor<u8>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OnnxTensorValue {
    F32(OnnxF32Tensor),
    I64(OnnxI64Tensor),
    I32(OnnxI32Tensor),
    U8(OnnxU8Tensor),
}

impl OnnxTensorValue {
    pub fn element_type(&self) -> OnnxTensorElementType {
        match self {
            Self::F32(_) => OnnxTensorElementType::F32,
            Self::I64(_) => OnnxTensorElementType::I64,
            Self::I32(_) => OnnxTensorElementType::I32,
            Self::U8(_) => OnnxTensorElementType::U8,
        }
    }

    pub fn shape(&self) -> &[usize] {
        match self {
            Self::F32(tensor) => &tensor.shape,
            Self::I64(tensor) => &tensor.shape,
            Self::I32(tensor) => &tensor.shape,
            Self::U8(tensor) => &tensor.shape,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnnxNamedTensor {
    pub name: String,
    pub tensor: OnnxTensorValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnnxDimension {
    Fixed(usize),
    Symbolic(String),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxIoInfo {
    pub name: String,
    pub element_type: Option<OnnxTensorElementType>,
    pub dimensions: Vec<OnnxDimension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OnnxSessionMetadata {
    pub inputs: Vec<OnnxIoInfo>,
    pub outputs: Vec<OnnxIoInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxSessionOptions {
    pub graph_optimization: OnnxGraphOptimization,
    pub execution_provider: OnnxExecutionProvider,
}

impl Default for OnnxSessionOptions {
    fn default() -> Self {
        Self {
            graph_optimization: OnnxGraphOptimization::Default,
            execution_provider: OnnxExecutionProvider::Cpu,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnnxExecutionProvider {
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnnxGraphOptimization {
    Default,
    Disable,
}

pub trait OnnxRunner {
    fn metadata(&self) -> Result<OnnxSessionMetadata>;
    fn run(&mut self, inputs: Vec<OnnxNamedTensor>) -> Result<Vec<OnnxNamedTensor>>;
}

#[derive(Debug)]
pub struct OnnxSession {
    #[cfg(feature = "onnxruntime")]
    session: Mutex<ort::session::Session>,
}

impl OnnxSession {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_file_with_options(path, OnnxSessionOptions::default())
    }

    pub fn from_file_cpu_single_threaded(path: impl AsRef<Path>) -> Result<Self> {
        from_file_cpu_single_threaded(path)
    }

    pub fn from_file_cpu_single_threaded_with_diagnostics(path: impl AsRef<Path>) -> Result<Self> {
        from_file_cpu_single_threaded_with_diagnostics(path)
    }

    pub fn from_file_with_options(
        path: impl AsRef<Path>,
        options: OnnxSessionOptions,
    ) -> Result<Self> {
        from_file_impl(
            path.as_ref(),
            options,
            false,
            SessionLoadDiagnostics::from_env()?,
        )
    }
}

pub fn from_file_cpu_single_threaded(path: impl AsRef<Path>) -> Result<OnnxSession> {
    from_file_impl(
        path.as_ref(),
        OnnxSessionOptions {
            graph_optimization: OnnxGraphOptimization::Disable,
            execution_provider: OnnxExecutionProvider::Cpu,
        },
        true,
        SessionLoadDiagnostics::from_env()?,
    )
}

pub fn from_file_cpu_single_threaded_with_diagnostics(
    path: impl AsRef<Path>,
) -> Result<OnnxSession> {
    from_file_impl(
        path.as_ref(),
        OnnxSessionOptions {
            graph_optimization: OnnxGraphOptimization::Disable,
            execution_provider: OnnxExecutionProvider::Cpu,
        },
        true,
        SessionLoadDiagnostics::from_env()?,
    )
}

pub fn inspect_model_metadata(path: impl AsRef<Path>) -> Result<OnnxSessionMetadata> {
    let bytes = std::fs::read(path)?;
    parse_model_metadata(&bytes)
}

#[doc(hidden)]
pub fn inspect_model_graph_diagnostics(path: impl AsRef<Path>) -> Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    Ok(parse_model_graph_diagnostics(&bytes)?.to_lines())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnnxRuntimeLoadMode {
    File,
    Memory,
}

impl OnnxRuntimeLoadMode {
    #[cfg_attr(not(feature = "onnxruntime"), allow(dead_code))]
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Memory => "memory",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionLoadDiagnostics {
    enabled: bool,
    load_mode: OnnxRuntimeLoadMode,
    disable_spinning: bool,
    optimized_model_path: Option<PathBuf>,
    #[cfg(feature = "onnxruntime")]
    log_level: Option<ort::logging::LogLevel>,
    #[cfg(not(feature = "onnxruntime"))]
    log_level: Option<()>,
    log_verbosity: Option<i32>,
}

impl SessionLoadDiagnostics {
    fn from_env() -> Result<Self> {
        let enabled = session_load_diagnostics_enabled();
        Self::from_env_values(
            enabled,
            std::env::var("ONNX_RUNTIME_LOAD_MODE").ok(),
            std::env::var("ONNX_RUNTIME_LOG_LEVEL").ok(),
            std::env::var("ONNX_RUNTIME_LOG_VERBOSITY").ok(),
            std::env::var("ONNX_RUNTIME_DISABLE_SPINNING").ok(),
            std::env::var_os("ONNX_RUNTIME_OPTIMIZED_MODEL_PATH").map(PathBuf::from),
        )
    }

    fn from_env_values(
        enabled: bool,
        load_mode: Option<String>,
        log_level: Option<String>,
        log_verbosity: Option<String>,
        disable_spinning: Option<String>,
        optimized_model_path: Option<PathBuf>,
    ) -> Result<Self> {
        let load_mode = parse_load_mode(load_mode.as_deref())?;
        let disable_spinning = disable_spinning.as_deref() == Some("1");
        let optimized_model_path = if enabled { optimized_model_path } else { None };
        Ok(Self {
            enabled,
            load_mode,
            disable_spinning,
            optimized_model_path,
            log_level: parse_diagnostic_log_level(enabled, log_level.as_deref())?,
            log_verbosity: parse_diagnostic_log_verbosity(enabled, log_verbosity.as_deref())?,
        })
    }
}

fn session_load_diagnostics_enabled() -> bool {
    std::env::var("ONNX_RUNTIME_LOAD_DIAGNOSTICS").as_deref() == Ok("1")
}

#[cfg_attr(not(feature = "onnxruntime"), allow(dead_code))]
fn diagnostic_stage(enabled: bool, stage: &str) {
    if diagnostic_stage_line(enabled, stage).is_some() {
        eprintln!("{stage}");
    }
}

#[cfg_attr(not(any(feature = "onnxruntime", test)), allow(dead_code))]
fn diagnostic_stage_line(enabled: bool, stage: &str) -> Option<&str> {
    enabled.then_some(stage)
}

fn parse_load_mode(value: Option<&str>) -> Result<OnnxRuntimeLoadMode> {
    match value.unwrap_or("file") {
        "file" => Ok(OnnxRuntimeLoadMode::File),
        "memory" => Ok(OnnxRuntimeLoadMode::Memory),
        other => Err(OnnxRuntimeError::InvalidArgument(format!(
            "ONNX_RUNTIME_LOAD_MODE must be `file` or `memory`, got `{other}`"
        ))),
    }
}

#[cfg(feature = "onnxruntime")]
fn parse_diagnostic_log_level(
    enabled: bool,
    value: Option<&str>,
) -> Result<Option<ort::logging::LogLevel>> {
    if !enabled {
        return Ok(None);
    }
    let level = match value.unwrap_or("verbose") {
        "verbose" => ort::logging::LogLevel::Verbose,
        "info" => ort::logging::LogLevel::Info,
        "warning" => ort::logging::LogLevel::Warning,
        "error" => ort::logging::LogLevel::Error,
        "fatal" => ort::logging::LogLevel::Fatal,
        other => {
            return Err(OnnxRuntimeError::InvalidArgument(format!(
                "ONNX_RUNTIME_LOG_LEVEL must be verbose, info, warning, error, or fatal; got `{other}`"
            )));
        }
    };
    Ok(Some(level))
}

#[cfg(not(feature = "onnxruntime"))]
fn parse_diagnostic_log_level(enabled: bool, value: Option<&str>) -> Result<Option<()>> {
    if !enabled {
        return Ok(None);
    }
    match value.unwrap_or("verbose") {
        "verbose" | "info" | "warning" | "error" | "fatal" => Ok(Some(())),
        other => Err(OnnxRuntimeError::InvalidArgument(format!(
            "ONNX_RUNTIME_LOG_LEVEL must be verbose, info, warning, error, or fatal; got `{other}`"
        ))),
    }
}

fn parse_diagnostic_log_verbosity(enabled: bool, value: Option<&str>) -> Result<Option<i32>> {
    if !enabled {
        return Ok(None);
    }
    let value = value.unwrap_or("4").parse::<i32>().map_err(|error| {
        OnnxRuntimeError::InvalidArgument(format!(
            "ONNX_RUNTIME_LOG_VERBOSITY must be an integer: {error}"
        ))
    })?;
    Ok(Some(value))
}

#[cfg(not(feature = "onnxruntime"))]
fn from_file_impl(
    _path: &Path,
    _options: OnnxSessionOptions,
    _single_threaded: bool,
    _diagnostics: SessionLoadDiagnostics,
) -> Result<OnnxSession> {
    Err(OnnxRuntimeError::Unavailable)
}

#[cfg(feature = "onnxruntime")]
fn from_file_impl(
    path: &Path,
    options: OnnxSessionOptions,
    single_threaded: bool,
    diagnostics: SessionLoadDiagnostics,
) -> Result<OnnxSession> {
    if !path.is_file() {
        return Err(OnnxRuntimeError::InvalidArgument(format!(
            "ONNX model file `{}` does not exist",
            path.display()
        )));
    }

    diagnostic_stage(diagnostics.enabled, "onnxSessionBuilder=begin");
    let mut builder = ort::session::Session::builder().map_err(ort_error)?;
    diagnostic_stage(diagnostics.enabled, "onnxSessionBuilder=ok");
    if let Some(level) = diagnostics.log_level {
        builder = builder.with_log_level(level).map_err(ort_error)?;
    }
    if let Some(verbosity) = diagnostics.log_verbosity {
        builder = builder.with_log_verbosity(verbosity).map_err(ort_error)?;
    }
    if diagnostics.enabled {
        builder = builder
            .with_log_id("video-analysis-speaker-embedding")
            .map_err(ort_error)?;
    }
    if let Some(path) = &diagnostics.optimized_model_path {
        builder = builder.with_optimized_model_path(path).map_err(ort_error)?;
    }
    let builder = match options.graph_optimization {
        OnnxGraphOptimization::Default => builder,
        OnnxGraphOptimization::Disable => {
            let builder = builder
                .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)
                .map_err(ort_error)?;
            diagnostic_stage(diagnostics.enabled, "onnxSessionGraphOptimization=disabled");
            builder
        }
    };
    let mut builder = match options.execution_provider {
        OnnxExecutionProvider::Cpu => {
            let builder = builder
                .with_no_environment_execution_providers()
                .map_err(ort_error)?;
            let builder = builder
                .with_execution_providers([ort::ep::CPUExecutionProvider::default().build()])
                .map_err(ort_error)?;
            diagnostic_stage(diagnostics.enabled, "onnxSessionExecutionProviders=cpu");
            builder
        }
    };
    if single_threaded {
        builder = builder.with_intra_threads(1).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionIntraThreads=1");
        builder = builder.with_inter_threads(1).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionInterThreads=1");
        builder = builder.with_parallel_execution(false).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionParallelExecution=false");
        builder = builder.with_memory_pattern(false).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionMemoryPattern=false");
    }
    if diagnostics.disable_spinning {
        builder = builder.with_intra_op_spinning(false).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionIntraOpSpinning=false");
        builder = builder.with_inter_op_spinning(false).map_err(ort_error)?;
        diagnostic_stage(diagnostics.enabled, "onnxSessionInterOpSpinning=false");
    }
    diagnostic_stage(
        diagnostics.enabled,
        &format!("onnxLoadMode={}", diagnostics.load_mode.as_str()),
    );
    diagnostic_stage(diagnostics.enabled, "onnxSessionCommit=begin");
    let session = match diagnostics.load_mode {
        OnnxRuntimeLoadMode::File => builder.commit_from_file(path).map_err(ort_error)?,
        OnnxRuntimeLoadMode::Memory => {
            let bytes = std::fs::read(path)?;
            builder.commit_from_memory(&bytes).map_err(ort_error)?
        }
    };
    diagnostic_stage(diagnostics.enabled, "onnxSessionCommit=ok");
    Ok(OnnxSession {
        session: Mutex::new(session),
    })
}

fn parse_model_metadata(bytes: &[u8]) -> Result<OnnxSessionMetadata> {
    Ok(parse_model(bytes)?.metadata)
}

fn parse_model_graph_diagnostics(bytes: &[u8]) -> Result<OnnxGraphDiagnostics> {
    Ok(parse_model(bytes)?.graph_diagnostics)
}

struct ParsedModel {
    metadata: OnnxSessionMetadata,
    graph_diagnostics: OnnxGraphDiagnostics,
}

fn parse_model(bytes: &[u8]) -> Result<ParsedModel> {
    let mut reader = ProtoReader::new(bytes);
    let mut graph = None;
    let mut opsets = BTreeMap::new();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (7, WIRE_LEN) => graph = Some(parse_graph(reader.read_len_bytes()?)?),
            (8, WIRE_LEN) => {
                let (domain, version) = parse_opset_import(reader.read_len_bytes()?)?;
                let domain = display_domain(&domain);
                let version = version
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                opsets.insert(domain, version);
            }
            _ => reader.skip_value(wire_type)?,
        }
    }
    let mut graph =
        graph.ok_or_else(|| malformed_proto("ONNX ModelProto is missing graph field"))?;
    graph.graph_diagnostics.opsets = opsets;
    Ok(graph)
}

fn parse_graph(bytes: &[u8]) -> Result<ParsedModel> {
    let mut reader = ProtoReader::new(bytes);
    let mut metadata = OnnxSessionMetadata::default();
    let mut graph_diagnostics = OnnxGraphDiagnostics::default();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) => {
                let node = parse_node(reader.read_len_bytes()?)?;
                graph_diagnostics.node_count += 1;
                *graph_diagnostics
                    .op_counts
                    .entry(empty_to_unknown(node.op_type))
                    .or_default() += 1;
                *graph_diagnostics
                    .domain_counts
                    .entry(display_domain(&node.domain))
                    .or_default() += 1;
            }
            (5, WIRE_LEN) => {
                parse_tensor_initializer(reader.read_len_bytes()?)?;
                graph_diagnostics.initializer_count += 1;
            }
            (11, WIRE_LEN) => metadata
                .inputs
                .push(parse_value_info(reader.read_len_bytes()?)?),
            (12, WIRE_LEN) => metadata
                .outputs
                .push(parse_value_info(reader.read_len_bytes()?)?),
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(ParsedModel {
        metadata,
        graph_diagnostics,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct OnnxGraphDiagnostics {
    op_counts: BTreeMap<String, usize>,
    domain_counts: BTreeMap<String, usize>,
    opsets: BTreeMap<String, String>,
    initializer_count: usize,
    node_count: usize,
}

impl OnnxGraphDiagnostics {
    fn to_lines(&self) -> Vec<String> {
        vec![
            format!("onnxGraphOps={}", format_counts(&self.op_counts)),
            format!("onnxGraphDomains={}", format_counts(&self.domain_counts)),
            format!("onnxGraphOpsets={}", format_key_values(&self.opsets)),
            format!("onnxGraphInitializerCount={}", self.initializer_count),
            format!("onnxGraphNodeCount={}", self.node_count),
        ]
    }
}

#[derive(Default)]
struct ParsedNode {
    op_type: String,
    domain: String,
}

fn parse_node(bytes: &[u8]) -> Result<ParsedNode> {
    let mut reader = ProtoReader::new(bytes);
    let mut node = ParsedNode::default();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) | (2, WIRE_LEN) => {
                reader.read_string()?;
            }
            (4, WIRE_LEN) => node.op_type = reader.read_string()?,
            (7, WIRE_LEN) => node.domain = reader.read_string()?,
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(node)
}

fn parse_opset_import(bytes: &[u8]) -> Result<(String, Option<i64>)> {
    let mut reader = ProtoReader::new(bytes);
    let mut domain = String::new();
    let mut version = None;
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) => domain = reader.read_string()?,
            (2, WIRE_VARINT) => {
                version = Some(
                    i64::try_from(reader.read_varint()?)
                        .map_err(|_| malformed_proto("ONNX opset version does not fit in i64"))?,
                );
            }
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok((domain, version))
}

fn parse_tensor_initializer(bytes: &[u8]) -> Result<()> {
    let mut reader = ProtoReader::new(bytes);
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_VARINT) => {
                i64::try_from(reader.read_varint()?).map_err(|_| {
                    malformed_proto("ONNX initializer dimension does not fit in i64")
                })?;
            }
            (2, WIRE_VARINT) => {
                reader.read_varint()?;
            }
            (8, WIRE_LEN) => {
                reader.read_string()?;
            }
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(())
}

fn empty_to_unknown(value: String) -> String {
    if value.is_empty() {
        "<unknown>".to_string()
    } else {
        value
    }
}

fn display_domain(value: &str) -> String {
    if value.is_empty() {
        "<default>".to_string()
    } else {
        value.to_string()
    }
}

fn format_counts(values: &BTreeMap<String, usize>) -> String {
    if values.is_empty() {
        return "<none>".to_string();
    }
    values
        .iter()
        .map(|(name, count)| format!("{name}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_key_values(values: &BTreeMap<String, String>) -> String {
    if values.is_empty() {
        return "<none>".to_string();
    }
    values
        .iter()
        .map(|(name, value)| format!("{name}:{value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_value_info(bytes: &[u8]) -> Result<OnnxIoInfo> {
    let mut reader = ProtoReader::new(bytes);
    let mut name = String::new();
    let mut element_type = None;
    let mut dimensions = Vec::new();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) => name = reader.read_string()?,
            (2, WIRE_LEN) => {
                let tensor = parse_type_proto(reader.read_len_bytes()?)?;
                element_type = tensor.element_type;
                dimensions = tensor.dimensions;
            }
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(OnnxIoInfo {
        name,
        element_type,
        dimensions,
    })
}

#[derive(Default)]
struct ParsedTensorType {
    element_type: Option<OnnxTensorElementType>,
    dimensions: Vec<OnnxDimension>,
}

fn parse_type_proto(bytes: &[u8]) -> Result<ParsedTensorType> {
    let mut reader = ProtoReader::new(bytes);
    let mut tensor = ParsedTensorType::default();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) => tensor = parse_tensor_type(reader.read_len_bytes()?)?,
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(tensor)
}

fn parse_tensor_type(bytes: &[u8]) -> Result<ParsedTensorType> {
    let mut reader = ProtoReader::new(bytes);
    let mut tensor = ParsedTensorType::default();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_VARINT) => {
                tensor.element_type = match reader.read_varint()? {
                    1 => Some(OnnxTensorElementType::F32),
                    _ => None,
                };
            }
            (2, WIRE_LEN) => tensor.dimensions = parse_tensor_shape(reader.read_len_bytes()?)?,
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(tensor)
}

fn parse_tensor_shape(bytes: &[u8]) -> Result<Vec<OnnxDimension>> {
    let mut reader = ProtoReader::new(bytes);
    let mut dimensions = Vec::new();
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_LEN) => dimensions.push(parse_dimension(reader.read_len_bytes()?)?),
            _ => reader.skip_value(wire_type)?,
        }
    }
    Ok(dimensions)
}

fn parse_dimension(bytes: &[u8]) -> Result<OnnxDimension> {
    let mut reader = ProtoReader::new(bytes);
    let mut fixed = None;
    let mut symbolic = None;
    while let Some((field, wire_type)) = reader.read_key()? {
        match (field, wire_type) {
            (1, WIRE_VARINT) => {
                fixed =
                    Some(usize::try_from(reader.read_varint()?).map_err(|_| {
                        malformed_proto("ONNX tensor dimension does not fit in usize")
                    })?);
            }
            (2, WIRE_LEN) => symbolic = Some(reader.read_string()?),
            _ => reader.skip_value(wire_type)?,
        }
    }
    if let Some(value) = symbolic.filter(|value| !value.is_empty()) {
        Ok(OnnxDimension::Symbolic(value))
    } else if let Some(value) = fixed {
        Ok(OnnxDimension::Fixed(value))
    } else {
        Ok(OnnxDimension::Unknown)
    }
}

const WIRE_VARINT: u8 = 0;
const WIRE_FIXED64: u8 = 1;
const WIRE_LEN: u8 = 2;
const WIRE_FIXED32: u8 = 5;

struct ProtoReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ProtoReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_key(&mut self) -> Result<Option<(u32, u8)>> {
        if self.offset == self.bytes.len() {
            return Ok(None);
        }
        let key = self.read_varint()?;
        let field = (key >> 3) as u32;
        let wire_type = (key & 0x07) as u8;
        if field == 0 {
            return Err(malformed_proto("protobuf field number must be non-zero"));
        }
        Ok(Some((field, wire_type)))
    }

    fn read_varint(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = *self
                .bytes
                .get(self.offset)
                .ok_or_else(|| malformed_proto("truncated protobuf varint"))?;
            self.offset += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(malformed_proto("protobuf varint exceeds 64 bits"))
    }

    fn read_len_bytes(&mut self) -> Result<&'a [u8]> {
        let len = usize::try_from(self.read_varint()?)
            .map_err(|_| malformed_proto("protobuf length does not fit in usize"))?;
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| malformed_proto("protobuf length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| malformed_proto("truncated length-delimited protobuf field"))?;
        self.offset = end;
        Ok(value)
    }

    fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_len_bytes()?;
        std::str::from_utf8(bytes)
            .map(|value| value.to_string())
            .map_err(|_| malformed_proto("protobuf string is not valid UTF-8"))
    }

    fn skip_value(&mut self, wire_type: u8) -> Result<()> {
        match wire_type {
            WIRE_VARINT => {
                self.read_varint()?;
                Ok(())
            }
            WIRE_FIXED64 => self.skip_bytes(8),
            WIRE_LEN => {
                self.read_len_bytes()?;
                Ok(())
            }
            WIRE_FIXED32 => self.skip_bytes(4),
            _ => Err(malformed_proto("unsupported protobuf wire type")),
        }
    }

    fn skip_bytes(&mut self, len: usize) -> Result<()> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| malformed_proto("protobuf fixed-width field overflow"))?;
        if end > self.bytes.len() {
            return Err(malformed_proto("truncated fixed-width protobuf field"));
        }
        self.offset = end;
        Ok(())
    }
}

fn malformed_proto(message: impl Into<String>) -> OnnxRuntimeError {
    OnnxRuntimeError::InvalidArgument(format!("malformed ONNX protobuf: {}", message.into()))
}

#[cfg(not(feature = "onnxruntime"))]
impl OnnxRunner for OnnxSession {
    fn metadata(&self) -> Result<OnnxSessionMetadata> {
        Err(OnnxRuntimeError::Unavailable)
    }

    fn run(&mut self, _inputs: Vec<OnnxNamedTensor>) -> Result<Vec<OnnxNamedTensor>> {
        Err(OnnxRuntimeError::Unavailable)
    }
}

#[cfg(feature = "onnxruntime")]
impl OnnxRunner for OnnxSession {
    fn metadata(&self) -> Result<OnnxSessionMetadata> {
        let session = self
            .session
            .lock()
            .map_err(|_| OnnxRuntimeError::Source("ONNX session mutex was poisoned".to_string()))?;
        Ok(session_metadata(&session))
    }

    fn run(&mut self, inputs: Vec<OnnxNamedTensor>) -> Result<Vec<OnnxNamedTensor>> {
        use std::borrow::Cow;

        use ort::session::SessionInputValue;
        use ort::value::Tensor;

        let mut ort_inputs = Vec::<(Cow<'_, str>, SessionInputValue<'_>)>::new();
        for input in inputs {
            let name = Cow::from(input.name);
            let value: SessionInputValue<'_> = match input.tensor {
                OnnxTensorValue::F32(tensor) => {
                    Tensor::<f32>::from_array((shape_to_i64(&tensor.shape)?, tensor.values))
                        .map_err(ort_error)?
                        .into()
                }
                OnnxTensorValue::I64(tensor) => {
                    Tensor::<i64>::from_array((shape_to_i64(&tensor.shape)?, tensor.values))
                        .map_err(ort_error)?
                        .into()
                }
                OnnxTensorValue::I32(tensor) => {
                    Tensor::<i32>::from_array((shape_to_i64(&tensor.shape)?, tensor.values))
                        .map_err(ort_error)?
                        .into()
                }
                OnnxTensorValue::U8(tensor) => {
                    Tensor::<u8>::from_array((shape_to_i64(&tensor.shape)?, tensor.values))
                        .map_err(ort_error)?
                        .into()
                }
            };
            ort_inputs.push((name, value));
        }

        let mut session = self
            .session
            .lock()
            .map_err(|_| OnnxRuntimeError::Source("ONNX session mutex was poisoned".to_string()))?;
        let outputs = session.run(ort_inputs).map_err(ort_error)?;
        let mut named = Vec::with_capacity(outputs.len());
        for (name, value) in outputs {
            named.push(OnnxNamedTensor {
                name: name.to_string(),
                tensor: extract_tensor_value(&value)?,
            });
        }
        Ok(named)
    }
}

pub fn input_name(metadata: &OnnxSessionMetadata, index: usize) -> Result<&str> {
    metadata
        .inputs
        .get(index)
        .map(|input| input.name.as_str())
        .ok_or_else(|| OnnxRuntimeError::InvalidArgument(format!("missing ONNX input #{index}")))
}

pub fn output_name(metadata: &OnnxSessionMetadata, index: usize) -> Result<&str> {
    metadata
        .outputs
        .get(index)
        .map(|output| output.name.as_str())
        .ok_or_else(|| OnnxRuntimeError::InvalidArgument(format!("missing ONNX output #{index}")))
}

pub fn first_f32_output(outputs: &[OnnxNamedTensor]) -> Result<&OnnxF32Tensor> {
    f32_output_by_name_or_index(outputs, "", 0)
}

pub fn f32_output_by_name_or_index<'a>(
    outputs: &'a [OnnxNamedTensor],
    name: &str,
    index: usize,
) -> Result<&'a OnnxF32Tensor> {
    match output_by_name_or_index(outputs, name, index)? {
        OnnxTensorValue::F32(tensor) => Ok(tensor),
        other => Err(output_type_error(
            "f32",
            name,
            index,
            outputs,
            other.element_type(),
        )),
    }
}

pub fn f32_output_by_preferred_name_or_index<'a>(
    outputs: &'a [OnnxNamedTensor],
    preferred_names: &[&str],
    index: usize,
) -> Result<&'a OnnxF32Tensor> {
    match output_by_preferred_name_or_index(outputs, preferred_names, index)? {
        OnnxTensorValue::F32(tensor) => Ok(tensor),
        other => Err(output_type_error(
            "f32",
            &preferred_names.join("|"),
            index,
            outputs,
            other.element_type(),
        )),
    }
}

pub fn i64_output_by_name_or_index<'a>(
    outputs: &'a [OnnxNamedTensor],
    name: &str,
    index: usize,
) -> Result<&'a OnnxI64Tensor> {
    match output_by_name_or_index(outputs, name, index)? {
        OnnxTensorValue::I64(tensor) => Ok(tensor),
        other => Err(output_type_error(
            "i64",
            name,
            index,
            outputs,
            other.element_type(),
        )),
    }
}

pub fn single_f32_input(
    name: impl Into<String>,
    shape: Vec<usize>,
    values: Vec<f32>,
) -> Result<OnnxNamedTensor> {
    Ok(OnnxNamedTensor {
        name: name.into(),
        tensor: OnnxTensorValue::F32(OnnxTensor::new(shape, values)?),
    })
}

pub fn single_i64_input(
    name: impl Into<String>,
    shape: Vec<usize>,
    values: Vec<i64>,
) -> Result<OnnxNamedTensor> {
    Ok(OnnxNamedTensor {
        name: name.into(),
        tensor: OnnxTensorValue::I64(OnnxTensor::new(shape, values)?),
    })
}

fn output_by_name_or_index<'a>(
    outputs: &'a [OnnxNamedTensor],
    name: &str,
    index: usize,
) -> Result<&'a OnnxTensorValue> {
    if !name.is_empty() {
        if let Some(output) = outputs.iter().find(|output| output.name == name) {
            return Ok(&output.tensor);
        }
    }
    outputs
        .get(index)
        .map(|output| &output.tensor)
        .ok_or_else(|| {
            OnnxRuntimeError::InvalidArgument(format!(
                "missing ONNX output `{name}` and fallback index #{index}; available outputs: {}",
                available_output_names(outputs)
            ))
        })
}

fn output_by_preferred_name_or_index<'a>(
    outputs: &'a [OnnxNamedTensor],
    preferred_names: &[&str],
    index: usize,
) -> Result<&'a OnnxTensorValue> {
    let preferred = outputs.iter().find(|output| {
        preferred_names.iter().any(|name| {
            output
                .name
                .to_ascii_lowercase()
                .contains(&name.to_ascii_lowercase())
        })
    });
    preferred
        .or_else(|| outputs.get(index))
        .map(|output| &output.tensor)
        .ok_or_else(|| {
            OnnxRuntimeError::InvalidArgument(format!(
                "missing ONNX output matching {:?} and fallback index #{index}; available outputs: {}",
                preferred_names,
                available_output_names(outputs)
            ))
        })
}

fn output_type_error(
    expected: &str,
    requested: &str,
    fallback_index: usize,
    outputs: &[OnnxNamedTensor],
    actual: OnnxTensorElementType,
) -> OnnxRuntimeError {
    OnnxRuntimeError::UnsupportedTensorType(format!(
        "expected {expected} output `{requested}` with fallback index #{fallback_index}, got {actual:?}; available outputs: {}",
        available_output_names(outputs)
    ))
}

fn available_output_names(outputs: &[OnnxNamedTensor]) -> String {
    if outputs.is_empty() {
        return "<none>".to_string();
    }
    outputs
        .iter()
        .map(|output| {
            format!(
                "{}:{:?}{:?}",
                output.name,
                output.tensor.element_type(),
                output.tensor.shape()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn element_count(shape: &[usize]) -> Result<usize> {
    shape.iter().try_fold(1usize, |acc, dim| {
        acc.checked_mul(*dim).ok_or_else(|| {
            OnnxRuntimeError::InvalidTensorShape(format!(
                "shape {shape:?} element count overflowed"
            ))
        })
    })
}

#[cfg(feature = "onnxruntime")]
fn session_metadata(session: &ort::session::Session) -> OnnxSessionMetadata {
    OnnxSessionMetadata {
        inputs: session.inputs().iter().map(io_info).collect(),
        outputs: session.outputs().iter().map(io_info).collect(),
    }
}

#[cfg(feature = "onnxruntime")]
fn io_info(outlet: &ort::value::Outlet) -> OnnxIoInfo {
    match outlet.dtype() {
        ort::value::ValueType::Tensor {
            ty,
            shape,
            dimension_symbols,
        } => OnnxIoInfo {
            name: outlet.name().to_string(),
            element_type: element_type_from_ort(*ty),
            dimensions: shape
                .iter()
                .enumerate()
                .map(|(index, dim)| {
                    if *dim >= 0 {
                        OnnxDimension::Fixed(*dim as usize)
                    } else {
                        dimension_symbols
                            .get(index)
                            .filter(|symbol| !symbol.is_empty())
                            .cloned()
                            .map(OnnxDimension::Symbolic)
                            .unwrap_or(OnnxDimension::Unknown)
                    }
                })
                .collect(),
        },
        _ => OnnxIoInfo {
            name: outlet.name().to_string(),
            element_type: None,
            dimensions: Vec::new(),
        },
    }
}

#[cfg(feature = "onnxruntime")]
fn extract_tensor_value(value: &ort::value::DynValue) -> Result<OnnxTensorValue> {
    match value.dtype() {
        ort::value::ValueType::Tensor { ty, .. } => match ty {
            ort::value::TensorElementType::Float32 => {
                let (shape, values) = value.try_extract_tensor::<f32>().map_err(ort_error)?;
                Ok(OnnxTensorValue::F32(OnnxTensor::new(
                    shape_from_ort(shape)?,
                    values.to_vec(),
                )?))
            }
            ort::value::TensorElementType::Int64 => {
                let (shape, values) = value.try_extract_tensor::<i64>().map_err(ort_error)?;
                Ok(OnnxTensorValue::I64(OnnxTensor::new(
                    shape_from_ort(shape)?,
                    values.to_vec(),
                )?))
            }
            ort::value::TensorElementType::Int32 => {
                let (shape, values) = value.try_extract_tensor::<i32>().map_err(ort_error)?;
                Ok(OnnxTensorValue::I32(OnnxTensor::new(
                    shape_from_ort(shape)?,
                    values.to_vec(),
                )?))
            }
            ort::value::TensorElementType::Uint8 => {
                let (shape, values) = value.try_extract_tensor::<u8>().map_err(ort_error)?;
                Ok(OnnxTensorValue::U8(OnnxTensor::new(
                    shape_from_ort(shape)?,
                    values.to_vec(),
                )?))
            }
            other => Err(OnnxRuntimeError::UnsupportedTensorType(format!("{other}"))),
        },
        other => Err(OnnxRuntimeError::UnsupportedTensorType(format!(
            "{other:?}"
        ))),
    }
}

#[cfg(feature = "onnxruntime")]
fn shape_from_ort(shape: &ort::value::Shape) -> Result<Vec<usize>> {
    shape
        .iter()
        .map(|dim| {
            usize::try_from(*dim).map_err(|_| {
                OnnxRuntimeError::InvalidTensorShape(format!(
                    "ONNX output shape contains negative dimension {dim}"
                ))
            })
        })
        .collect()
}

#[cfg(feature = "onnxruntime")]
fn shape_to_i64(shape: &[usize]) -> Result<Vec<i64>> {
    shape
        .iter()
        .map(|dim| {
            i64::try_from(*dim).map_err(|_| {
                OnnxRuntimeError::InvalidTensorShape(format!(
                    "ONNX input shape dimension {dim} does not fit in i64"
                ))
            })
        })
        .collect()
}

#[cfg(feature = "onnxruntime")]
fn element_type_from_ort(ty: ort::value::TensorElementType) -> Option<OnnxTensorElementType> {
    match ty {
        ort::value::TensorElementType::Float32 => Some(OnnxTensorElementType::F32),
        ort::value::TensorElementType::Int64 => Some(OnnxTensorElementType::I64),
        ort::value::TensorElementType::Int32 => Some(OnnxTensorElementType::I32),
        ort::value::TensorElementType::Uint8 => Some(OnnxTensorElementType::U8),
        _ => None,
    }
}

#[cfg(feature = "onnxruntime")]
fn ort_error<T>(error: ort::Error<T>) -> OnnxRuntimeError {
    OnnxRuntimeError::Source(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_shape_validation_rejects_mismatched_element_counts() {
        let err = OnnxF32Tensor::new(vec![2, 2], vec![1.0, 2.0, 3.0]).unwrap_err();
        assert!(matches!(err, OnnxRuntimeError::InvalidTensorShape(_)));
    }

    #[test]
    fn f32_tensor_validation_rejects_nan_and_inf() {
        assert!(OnnxF32Tensor::new(vec![1], vec![f32::NAN]).is_err());
        assert!(OnnxF32Tensor::new(vec![1], vec![f32::INFINITY]).is_err());
    }

    #[test]
    fn named_tensor_lookup_by_name_and_index_is_deterministic() {
        let first = single_f32_input("first", vec![1], vec![1.0]).unwrap();
        let second = single_f32_input("second", vec![1], vec![2.0]).unwrap();
        let outputs = vec![first, second];
        assert_eq!(
            f32_output_by_name_or_index(&outputs, "second", 0)
                .unwrap()
                .values,
            vec![2.0]
        );
        assert_eq!(
            f32_output_by_name_or_index(&outputs, "missing", 0)
                .unwrap()
                .values,
            vec![1.0]
        );
    }

    #[test]
    fn preferred_output_lookup_selects_named_f32_before_fallback() {
        let first = single_f32_input("last_hidden_state", vec![1], vec![1.0]).unwrap();
        let second = single_f32_input("image_embeds", vec![1], vec![2.0]).unwrap();
        let outputs = vec![first, second];
        assert_eq!(
            f32_output_by_preferred_name_or_index(&outputs, &["image_embeds"], 0)
                .unwrap()
                .values,
            vec![2.0]
        );
    }

    #[test]
    fn preferred_output_lookup_falls_back_to_index() {
        let first = single_f32_input("first", vec![1], vec![1.0]).unwrap();
        let second = single_f32_input("second", vec![1], vec![2.0]).unwrap();
        let outputs = vec![first, second];
        assert_eq!(
            f32_output_by_preferred_name_or_index(&outputs, &["missing"], 1)
                .unwrap()
                .values,
            vec![2.0]
        );
    }

    #[test]
    fn missing_output_error_reports_available_outputs() {
        let outputs = vec![single_f32_input("scores", vec![1], vec![1.0]).unwrap()];
        let err = f32_output_by_preferred_name_or_index(&outputs, &["logits"], 3).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("logits"));
        assert!(message.contains("#3"));
        assert!(message.contains("scores"));
        assert!(message.contains("F32"));
    }

    #[test]
    fn unsupported_output_dtype_returns_typed_error_through_helpers() {
        let outputs = vec![single_i64_input("ids", vec![1], vec![42]).unwrap()];
        let err = f32_output_by_name_or_index(&outputs, "ids", 0).unwrap_err();
        let message = err.to_string();
        assert!(matches!(err, OnnxRuntimeError::UnsupportedTensorType(_)));
        assert!(message.contains("I64"));
        assert!(message.contains("ids"));
    }

    #[test]
    fn static_metadata_parser_reads_feature_input_and_embedding_output() {
        let bytes = model_proto(graph_proto(
            vec![value_info(
                "feats",
                1,
                vec![
                    TestDim::Symbolic("B"),
                    TestDim::Symbolic("T"),
                    TestDim::Fixed(80),
                ],
            )],
            vec![value_info(
                "embs",
                1,
                vec![TestDim::Symbolic("B"), TestDim::Fixed(256)],
            )],
        ));

        let metadata = parse_model_metadata(&bytes).unwrap();

        assert_eq!(metadata.inputs[0].name, "feats");
        assert_eq!(
            metadata.inputs[0].element_type,
            Some(OnnxTensorElementType::F32)
        );
        assert_eq!(
            metadata.inputs[0].dimensions,
            vec![
                OnnxDimension::Symbolic("B".to_string()),
                OnnxDimension::Symbolic("T".to_string()),
                OnnxDimension::Fixed(80),
            ]
        );
        assert_eq!(metadata.outputs[0].name, "embs");
        assert_eq!(
            metadata.outputs[0].dimensions,
            vec![
                OnnxDimension::Symbolic("B".to_string()),
                OnnxDimension::Fixed(256),
            ]
        );
    }

    #[test]
    fn static_metadata_parser_handles_unknown_and_unsupported_types() {
        let bytes = model_proto(graph_proto(
            vec![value_info(
                "tokens",
                7,
                vec![TestDim::Fixed(1), TestDim::Unknown],
            )],
            vec![],
        ));

        let metadata = parse_model_metadata(&bytes).unwrap();

        assert_eq!(metadata.inputs[0].element_type, None);
        assert_eq!(
            metadata.inputs[0].dimensions,
            vec![OnnxDimension::Fixed(1), OnnxDimension::Unknown]
        );
    }

    #[test]
    fn static_metadata_parser_rejects_malformed_protobuf() {
        let err = parse_model_metadata(&[0x3a, 0x80]).unwrap_err();
        assert!(matches!(err, OnnxRuntimeError::InvalidArgument(_)));
        assert!(err.to_string().contains("malformed ONNX protobuf"));
    }

    #[test]
    fn static_graph_parser_aggregates_ops_and_domains() {
        let bytes = model_proto_with_opsets(
            graph_proto_full(
                vec![],
                vec![],
                vec![
                    node_proto("", "Gemm", vec!["x"], vec!["y"]),
                    node_proto("", "Gemm", vec!["y"], vec!["z"]),
                    node_proto("ai.onnx.ml", "Scaler", vec!["z"], vec!["out"]),
                ],
                vec![tensor_initializer("weights", vec![2, 2], 1)],
            ),
            vec![opset_import("", 17), opset_import("ai.onnx.ml", 3)],
        );

        let diagnostics = parse_model_graph_diagnostics(&bytes).unwrap();

        assert_eq!(diagnostics.node_count, 3);
        assert_eq!(diagnostics.initializer_count, 1);
        assert_eq!(diagnostics.op_counts.get("Gemm"), Some(&2));
        assert_eq!(diagnostics.op_counts.get("Scaler"), Some(&1));
        assert_eq!(diagnostics.domain_counts.get("<default>"), Some(&2));
        assert_eq!(diagnostics.domain_counts.get("ai.onnx.ml"), Some(&1));
        assert_eq!(
            diagnostics.opsets.get("<default>").map(String::as_str),
            Some("17")
        );
        assert_eq!(
            diagnostics.opsets.get("ai.onnx.ml").map(String::as_str),
            Some("3")
        );
    }

    #[test]
    fn static_graph_parser_reports_default_domain() {
        let bytes = model_proto(graph_proto_full(
            vec![],
            vec![],
            vec![node_proto("", "Relu", vec!["x"], vec!["y"])],
            vec![],
        ));

        let lines = parse_model_graph_diagnostics(&bytes).unwrap().to_lines();

        assert!(lines.iter().any(|line| line == "onnxGraphOps=Relu:1"));
        assert!(lines
            .iter()
            .any(|line| line == "onnxGraphDomains=<default>:1"));
        assert!(lines
            .iter()
            .any(|line| line == "onnxGraphInitializerCount=0"));
        assert!(lines.iter().any(|line| line == "onnxGraphNodeCount=1"));
    }

    #[test]
    fn static_graph_parser_tolerates_unsupported_initializer_tensor_type() {
        let bytes = model_proto(graph_proto_full(
            vec![],
            vec![],
            vec![],
            vec![tensor_initializer("opaque", vec![1], 99)],
        ));

        let diagnostics = parse_model_graph_diagnostics(&bytes).unwrap();

        assert_eq!(diagnostics.initializer_count, 1);
        assert_eq!(diagnostics.node_count, 0);
    }

    #[test]
    fn static_graph_parser_rejects_malformed_nested_node() {
        let mut malformed_node = varint((4 << 3) | u64::from(WIRE_LEN));
        malformed_node.push(0x80);
        let bytes = model_proto(graph_proto_full(
            vec![],
            vec![],
            vec![malformed_node],
            vec![],
        ));

        let err = parse_model_graph_diagnostics(&bytes).unwrap_err();

        assert!(matches!(err, OnnxRuntimeError::InvalidArgument(_)));
        assert!(err.to_string().contains("malformed ONNX protobuf"));
    }

    #[test]
    fn diagnostics_env_defaults_are_silent_file_load() {
        let config =
            SessionLoadDiagnostics::from_env_values(false, None, None, None, None, None).unwrap();

        assert!(!config.enabled);
        assert_eq!(config.load_mode, OnnxRuntimeLoadMode::File);
        assert!(config.log_level.is_none());
        assert!(config.log_verbosity.is_none());
        assert_eq!(diagnostic_stage_line(false, "hidden"), None);
    }

    #[test]
    fn diagnostics_env_accepts_memory_load_mode() {
        let config = SessionLoadDiagnostics::from_env_values(
            true,
            Some("memory".to_string()),
            None,
            None,
            Some("1".to_string()),
            Some(PathBuf::from("/ignored/model.optimized.onnx")),
        )
        .unwrap();

        assert!(config.enabled);
        assert_eq!(config.load_mode, OnnxRuntimeLoadMode::Memory);
        assert!(config.disable_spinning);
        assert!(config.log_level.is_some());
        assert_eq!(config.log_verbosity, Some(4));
        assert!(config.optimized_model_path.is_some());
        assert_eq!(diagnostic_stage_line(true, "visible"), Some("visible"));
    }

    #[test]
    fn diagnostics_env_rejects_invalid_log_level() {
        let err = SessionLoadDiagnostics::from_env_values(
            true,
            None,
            Some("debug".to_string()),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert!(matches!(err, OnnxRuntimeError::InvalidArgument(_)));
        assert!(err.to_string().contains("ONNX_RUNTIME_LOG_LEVEL"));
    }

    #[test]
    fn diagnostics_env_rejects_invalid_verbosity() {
        let err = SessionLoadDiagnostics::from_env_values(
            true,
            None,
            None,
            Some("loud".to_string()),
            None,
            None,
        )
        .unwrap_err();

        assert!(matches!(err, OnnxRuntimeError::InvalidArgument(_)));
        assert!(err.to_string().contains("ONNX_RUNTIME_LOG_VERBOSITY"));
    }

    #[cfg(not(feature = "onnxruntime"))]
    #[test]
    fn no_feature_session_construction_returns_unavailable() {
        let err = OnnxSession::from_file("missing.onnx").unwrap_err();
        assert!(matches!(err, OnnxRuntimeError::Unavailable));
    }

    #[cfg(feature = "onnxruntime")]
    #[test]
    fn missing_model_path_returns_typed_error() {
        let err = OnnxSession::from_file("missing.onnx").unwrap_err();
        assert!(matches!(err, OnnxRuntimeError::InvalidArgument(_)));
    }

    enum TestDim<'a> {
        Fixed(u64),
        Symbolic(&'a str),
        Unknown,
    }

    fn model_proto(graph: Vec<u8>) -> Vec<u8> {
        len_field(7, graph)
    }

    fn model_proto_with_opsets(graph: Vec<u8>, opsets: Vec<Vec<u8>>) -> Vec<u8> {
        let mut bytes = model_proto(graph);
        for opset in opsets {
            bytes.extend(len_field(8, opset));
        }
        bytes
    }

    fn graph_proto(inputs: Vec<Vec<u8>>, outputs: Vec<Vec<u8>>) -> Vec<u8> {
        graph_proto_full(inputs, outputs, vec![], vec![])
    }

    fn graph_proto_full(
        inputs: Vec<Vec<u8>>,
        outputs: Vec<Vec<u8>>,
        nodes: Vec<Vec<u8>>,
        initializers: Vec<Vec<u8>>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        for node in nodes {
            bytes.extend(len_field(1, node));
        }
        for initializer in initializers {
            bytes.extend(len_field(5, initializer));
        }
        for input in inputs {
            bytes.extend(len_field(11, input));
        }
        for output in outputs {
            bytes.extend(len_field(12, output));
        }
        bytes
    }

    fn node_proto(domain: &str, op_type: &str, inputs: Vec<&str>, outputs: Vec<&str>) -> Vec<u8> {
        let mut bytes = Vec::new();
        for input in inputs {
            bytes.extend(len_field(1, input.as_bytes().to_vec()));
        }
        for output in outputs {
            bytes.extend(len_field(2, output.as_bytes().to_vec()));
        }
        bytes.extend(len_field(4, op_type.as_bytes().to_vec()));
        if !domain.is_empty() {
            bytes.extend(len_field(7, domain.as_bytes().to_vec()));
        }
        bytes
    }

    fn tensor_initializer(name: &str, dims: Vec<u64>, data_type: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        for dim in dims {
            bytes.extend(varint_field(1, dim));
        }
        bytes.extend(varint_field(2, data_type));
        bytes.extend(len_field(8, name.as_bytes().to_vec()));
        bytes
    }

    fn opset_import(domain: &str, version: u64) -> Vec<u8> {
        let mut bytes = len_field(1, domain.as_bytes().to_vec());
        bytes.extend(varint_field(2, version));
        bytes
    }

    fn value_info(name: &str, elem_type: u64, dims: Vec<TestDim<'_>>) -> Vec<u8> {
        let mut shape = Vec::new();
        for dim in dims {
            shape.extend(len_field(
                1,
                match dim {
                    TestDim::Fixed(value) => varint_field(1, value),
                    TestDim::Symbolic(value) => len_field(2, value.as_bytes().to_vec()),
                    TestDim::Unknown => Vec::new(),
                },
            ));
        }
        let mut tensor_type = varint_field(1, elem_type);
        tensor_type.extend(len_field(2, shape));
        let type_proto = len_field(1, tensor_type);
        let mut value_info = len_field(1, name.as_bytes().to_vec());
        value_info.extend(len_field(2, type_proto));
        value_info
    }

    fn len_field(field: u64, value: Vec<u8>) -> Vec<u8> {
        let mut bytes = varint((field << 3) | u64::from(WIRE_LEN));
        bytes.extend(varint(value.len() as u64));
        bytes.extend(value);
        bytes
    }

    fn varint_field(field: u64, value: u64) -> Vec<u8> {
        let mut bytes = varint((field << 3) | u64::from(WIRE_VARINT));
        bytes.extend(varint(value));
        bytes
    }

    fn varint(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if value == 0 {
                return bytes;
            }
        }
    }
}
