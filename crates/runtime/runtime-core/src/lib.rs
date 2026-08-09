use serde::{Deserialize, Serialize};
use std::fmt;

pub mod landscape;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct DiagnosticCode(pub String);

impl DiagnosticCode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DiagnosticCode {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for DiagnosticCode {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub message: String,
    pub source: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            source: None,
            help: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub native: bool,
    pub server: bool,
    pub wasm: bool,
    pub mobile: MobileCapability,
    pub requirements: Vec<RuntimeRequirement>,
    pub max_recommended_input_bytes: Option<u64>,
}

impl RuntimeCapabilities {
    pub fn pure_rust() -> Self {
        Self {
            native: true,
            server: true,
            wasm: true,
            mobile: MobileCapability::Wasm,
            requirements: Vec::new(),
            max_recommended_input_bytes: None,
        }
    }

    pub fn with_max_recommended_input_bytes(mut self, bytes: u64) -> Self {
        self.max_recommended_input_bytes = Some(bytes);
        self
    }

    pub fn with_requirement(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.requirements.push(RuntimeRequirement {
            name: name.into(),
            description: Some(description.into()),
            required,
        });
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MobileCapability {
    Native,
    Wasm,
    ApiOnly,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRequirement {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct OperationId(pub String);

impl OperationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for OperationId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for OperationId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationMetadata {
    pub id: OperationId,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub capabilities: RuntimeCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageSurface {
    pub library: String,
    pub version: String,
    pub operations: Vec<SurfaceOperation>,
    pub capabilities: RuntimeCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceOperationCuration {
    pub role: SurfaceOperationRole,
    pub primary: bool,
    pub sort_order: u16,
}

impl SurfaceOperationCuration {
    pub fn workflow(sort_order: u16) -> Self {
        Self {
            role: SurfaceOperationRole::Workflow,
            primary: false,
            sort_order,
        }
    }

    pub fn debug(sort_order: u16) -> Self {
        Self {
            role: SurfaceOperationRole::Debug,
            primary: false,
            sort_order,
        }
    }

    pub fn support(sort_order: u16) -> Self {
        Self {
            role: SurfaceOperationRole::Support,
            primary: false,
            sort_order,
        }
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn from_operation_id(operation_id: &str) -> Self {
        if operation_id == "describe" {
            Self::debug(900)
        } else {
            match operation_category(operation_id) {
                "debug" => Self::debug(900),
                "support" => Self::support(500),
                _ => Self::workflow(100),
            }
        }
    }

    fn legacy_category(&self) -> &'static str {
        self.role.legacy_category()
    }
}

impl Default for SurfaceOperationCuration {
    fn default() -> Self {
        Self::workflow(100)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceOperationRole {
    Workflow,
    Debug,
    Support,
}

impl SurfaceOperationRole {
    fn legacy_category(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::Debug => "debug",
            Self::Support => "support",
        }
    }

    fn from_legacy_category(value: &str) -> Option<Self> {
        match value {
            "workflow" => Some(Self::Workflow),
            "debug" => Some(Self::Debug),
            "support" => Some(Self::Support),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceOperation {
    pub id: OperationId,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub curation: SurfaceOperationCuration,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub example_request: serde_json::Value,
    pub wasm_supported: bool,
    pub server_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceExecutionMode {
    InMemory,
    PlannedJob,
    BackgroundJob,
    ExternalCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceSideEffect {
    None,
    ReadsFiles,
    WritesFiles,
    Network,
    ExternalProcess,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceArtifactExpectation {
    pub id: String,
    pub kind: String,
    pub media_type: String,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceExecutionPlan {
    pub operation: OperationId,
    pub mode: SurfaceExecutionMode,
    pub side_effects: Vec<SurfaceSideEffect>,
    pub cancellable: bool,
    pub progress_unit: Option<String>,
    pub expected_artifacts: Vec<SurfaceArtifactExpectation>,
    pub requirements: Vec<RuntimeRequirement>,
    pub max_recommended_input_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRuntimeContext {
    pub runtime: SurfaceRuntimeKind,
    pub side_effects: SurfaceSideEffectPolicy,
    pub storage: SurfaceStorageContext,
    pub model: SurfaceModelContext,
}

impl SurfaceRuntimeContext {
    pub fn no_side_effects(runtime: SurfaceRuntimeKind) -> Self {
        Self {
            runtime,
            side_effects: SurfaceSideEffectPolicy::none(),
            storage: SurfaceStorageContext::default(),
            model: SurfaceModelContext::plan_only(),
        }
    }

    pub fn compatibility_no_side_effects() -> Self {
        Self::no_side_effects(SurfaceRuntimeKind::Unknown)
    }

    pub fn allows_model_auto_setup(&self) -> bool {
        self.model.auto_setup
            && self.side_effects.allow_reads
            && self.side_effects.allow_writes
            && self.side_effects.allow_network
            && self.storage.model_root.is_some()
            && self.runtime.can_setup_models()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceRuntimeKind {
    NativeCli,
    NativeServer,
    Wasm,
    Browser,
    Mobile,
    Unknown,
}

impl SurfaceRuntimeKind {
    pub fn can_setup_models(&self) -> bool {
        matches!(self, Self::NativeCli | Self::NativeServer)
    }

    pub fn can_run_external_processes(&self) -> bool {
        matches!(self, Self::NativeCli | Self::NativeServer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSideEffectPolicy {
    pub allow_reads: bool,
    pub allow_writes: bool,
    pub allow_network: bool,
    pub allow_external_process: bool,
    pub max_download_bytes: Option<u64>,
}

impl SurfaceSideEffectPolicy {
    pub fn none() -> Self {
        Self {
            allow_reads: false,
            allow_writes: false,
            allow_network: false,
            allow_external_process: false,
            max_download_bytes: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceStorageContext {
    pub cache_root: Option<String>,
    pub artifact_root: Option<String>,
    pub model_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceModelContext {
    pub auto_setup: bool,
    pub preference: SurfaceModelExecutionPreference,
}

impl SurfaceModelContext {
    pub fn plan_only() -> Self {
        Self {
            auto_setup: false,
            preference: SurfaceModelExecutionPreference::PlanOnly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceModelExecutionPreference {
    ModelFirst,
    PlanOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceRequest {
    pub operation: OperationId,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceResponse {
    pub operation: OperationId,
    pub value: serde_json::Value,
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceError {
    pub code: String,
    pub message: String,
    pub operation: Option<OperationId>,
    pub details: serde_json::Value,
}

impl SurfaceError {
    pub fn invalid_request(
        operation: Option<impl Into<OperationId>>,
        message: impl Into<String>,
    ) -> Self {
        Self::new("invalid_request", operation, message, serde_json::json!({}))
    }

    pub fn unsupported_operation(
        operation: impl Into<OperationId>,
        package: impl Into<String>,
    ) -> Self {
        let operation = operation.into();
        let package = package.into();
        Self::new(
            "unsupported_operation",
            Some(operation.clone()),
            format!(
                "unsupported operation `{}` for {}",
                operation.as_str(),
                package
            ),
            serde_json::json!({"package": package}),
        )
    }

    pub fn unsupported_value(
        operation: Option<impl Into<OperationId>>,
        field: impl Into<String>,
        value: impl Into<String>,
        allowed: &[&str],
    ) -> Self {
        let field = field.into();
        let value = value.into();
        Self::new(
            "unsupported_value",
            operation,
            format!("unsupported value `{value}` for `{field}`"),
            serde_json::json!({
                "field": field,
                "value": value,
                "allowed": allowed
            }),
        )
    }

    pub fn resource_limit(
        operation: Option<impl Into<OperationId>>,
        field: impl Into<String>,
        limit: usize,
        actual: usize,
    ) -> Self {
        let field = field.into();
        Self::new(
            "resource_limit",
            operation,
            format!("`{field}` exceeds the maximum supported size of {limit}"),
            serde_json::json!({
                "field": field,
                "limit": limit,
                "actual": actual
            }),
        )
    }

    pub fn cancelled(operation: impl Into<OperationId>, message: impl Into<String>) -> Self {
        Self::new("cancelled", Some(operation), message, serde_json::json!({}))
    }

    pub fn execution_failed(
        operation: impl Into<OperationId>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::new("execution_failed", Some(operation), message, details)
    }

    pub fn artifact_error(
        operation: impl Into<OperationId>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::new("artifact_error", Some(operation), message, details)
    }

    pub fn missing_dependency(
        operation: Option<impl Into<OperationId>>,
        dependency: impl Into<String>,
        setup: impl Into<String>,
    ) -> Self {
        let dependency = dependency.into();
        let setup = setup.into();
        Self::new(
            "missing_dependency",
            operation,
            format!("missing required dependency `{dependency}`"),
            serde_json::json!({
                "dependency": dependency,
                "setup": setup
            }),
        )
    }

    pub fn permission_denied(
        operation: Option<impl Into<OperationId>>,
        permission: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let permission = permission.into();
        Self::new(
            "permission_denied",
            operation,
            message,
            serde_json::json!({
                "permission": permission
            }),
        )
    }

    pub fn new(
        code: impl Into<String>,
        operation: Option<impl Into<OperationId>>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            operation: operation.map(Into::into),
            details,
        }
    }

    pub fn to_error_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.message.clone())
    }
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SurfaceError {}

pub fn parse_surface_error(error: &str) -> Option<SurfaceError> {
    serde_json::from_str(error).ok()
}

pub fn parse_surface_input<T: for<'de> Deserialize<'de>>(
    operation: Option<&str>,
    input: serde_json::Value,
) -> Result<T, String> {
    serde_json::from_value(input).map_err(|error| {
        SurfaceError::invalid_request(
            operation.map(OperationId::new),
            format!("invalid request: {error}"),
        )
        .to_error_string()
    })
}

pub fn require_non_empty<T>(operation: &str, field: &str, values: &[T]) -> Result<(), String> {
    if values.is_empty() {
        Err(SurfaceError::invalid_request(
            Some(OperationId::new(operation)),
            format!("invalid request: {field} must not be empty"),
        )
        .to_error_string())
    } else {
        Ok(())
    }
}

pub fn validate_max_items(
    operation: &str,
    field: &str,
    actual: usize,
    limit: usize,
) -> Result<(), String> {
    if actual > limit {
        Err(
            SurfaceError::resource_limit(Some(OperationId::new(operation)), field, limit, actual)
                .to_error_string(),
        )
    } else {
        Ok(())
    }
}

pub fn validate_matching_lengths(
    operation: &str,
    left_field: &str,
    left_len: usize,
    right_field: &str,
    right_len: usize,
) -> Result<(), String> {
    if left_len != right_len {
        Err(SurfaceError::invalid_request(
            Some(OperationId::new(operation)),
            format!(
                "invalid request: `{left_field}` length {left_len} must match `{right_field}` length {right_len}"
            ),
        )
        .to_error_string())
    } else {
        Ok(())
    }
}

/// Builds the standard package-surface operation metadata used by library
/// crates and transport adapters.
pub fn surface_operation(
    id: impl Into<String>,
    name: impl Into<String>,
    description: impl Into<String>,
    example_request: serde_json::Value,
) -> SurfaceOperation {
    let id = id.into();
    let curation = SurfaceOperationCuration::from_operation_id(&id);
    let mut operation = SurfaceOperation {
        id: OperationId::new(id.clone()),
        name: name.into(),
        description: Some(description.into()),
        input_schema: surface_input_schema(&id, &example_request),
        output_schema: surface_output_schema(&id),
        curation: curation.clone(),
        example_request,
        wasm_supported: true,
        server_supported: true,
    };
    set_surface_operation_curation(&mut operation, curation);
    operation
}

pub fn surface_operation_with_execution_plan(
    id: impl Into<String>,
    name: impl Into<String>,
    description: impl Into<String>,
    example_request: serde_json::Value,
    execution_plan: SurfaceExecutionPlan,
) -> SurfaceOperation {
    let mut operation = surface_operation(id, name, description, example_request);
    let execution_plan = surface_execution_plan_value(&execution_plan);
    insert_schema_extension(
        &mut operation.input_schema,
        "xExecutionPlan",
        execution_plan.clone(),
    );
    insert_schema_extension(
        &mut operation.output_schema,
        "xExecutionPlan",
        execution_plan,
    );
    operation
}

pub fn primary_workflow_operation(
    id: impl Into<String>,
    name: impl Into<String>,
    description: impl Into<String>,
    example_request: serde_json::Value,
    execution_plan: Option<SurfaceExecutionPlan>,
    lower_contracts: &[&str],
) -> SurfaceOperation {
    let mut operation = if let Some(plan) = execution_plan {
        surface_operation_with_execution_plan(id, name, description, example_request, plan)
    } else {
        surface_operation(id, name, description, example_request)
    };
    set_surface_operation_curation(
        &mut operation,
        SurfaceOperationCuration::workflow(0).primary(),
    );
    if !lower_contracts.is_empty() {
        let proof = serde_json::json!({
            "policy": "primary workflow proves compatibility with lower crate contracts",
            "crates": lower_contracts
        });
        insert_schema_extension(
            &mut operation.input_schema,
            "xLowerContractProof",
            proof.clone(),
        );
        insert_schema_extension(&mut operation.output_schema, "xLowerContractProof", proof);
    }
    operation
}

/// Adds a schema extension to both input and output schemas for an operation.
///
/// Runtime surfaces use `x*` schema fields for additive metadata such as
/// operation category, compatibility/deprecation state, or replacement hints.
pub fn add_surface_operation_schema_extension(
    operation: &mut SurfaceOperation,
    key: impl Into<String>,
    value: serde_json::Value,
) {
    let key = key.into();
    if key == "xOperationCategory" {
        if let Some(category) = value
            .as_str()
            .and_then(SurfaceOperationRole::from_legacy_category)
        {
            operation.curation.role = category;
        }
    }
    insert_schema_extension(&mut operation.input_schema, &key, value.clone());
    insert_schema_extension(&mut operation.output_schema, &key, value);
}

pub fn set_surface_operation_curation(
    operation: &mut SurfaceOperation,
    curation: SurfaceOperationCuration,
) {
    let category = serde_json::json!(curation.legacy_category());
    operation.curation = curation;
    insert_schema_extension(
        &mut operation.input_schema,
        "xOperationCategory",
        category.clone(),
    );
    insert_schema_extension(&mut operation.output_schema, "xOperationCategory", category);
}

pub fn landscape_operation_contract_value(
    contract: &landscape::LandscapeOperationContract,
) -> serde_json::Value {
    serde_json::to_value(contract).unwrap_or_else(|_| serde_json::json!({}))
}

pub fn surface_operation_with_landscape(
    id: impl Into<String>,
    name: impl Into<String>,
    description: impl Into<String>,
    example_request: serde_json::Value,
    contract: landscape::LandscapeOperationContract,
) -> SurfaceOperation {
    let mut operation = surface_operation(id, name, description, example_request);
    attach_landscape_contract(&mut operation, contract);
    operation
}

pub fn attach_landscape_contract(
    operation: &mut SurfaceOperation,
    contract: landscape::LandscapeOperationContract,
) {
    let landscape = landscape_operation_contract_value(&contract);
    insert_schema_extension(&mut operation.input_schema, "xLandscape", landscape.clone());
    insert_schema_extension(&mut operation.output_schema, "xLandscape", landscape);
}

pub fn surface_execution_plan_value(plan: &SurfaceExecutionPlan) -> serde_json::Value {
    serde_json::to_value(plan).unwrap_or_else(|_| serde_json::json!({}))
}

fn insert_schema_extension(schema: &mut serde_json::Value, key: &str, value: serde_json::Value) {
    if let serde_json::Value::Object(object) = schema {
        object.insert(key.to_string(), value);
    }
}

pub fn surface_input_schema(
    operation: &str,
    example_request: &serde_json::Value,
) -> serde_json::Value {
    let properties = match example_request {
        serde_json::Value::Object(object) => object
            .iter()
            .map(|(key, value)| (key.clone(), infer_schema_for_value(key, value)))
            .collect::<serde_json::Map<_, _>>(),
        _ => serde_json::Map::new(),
    };
    let required = required_fields_for_operation(operation, example_request);
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
        "xOperationCategory": operation_category(operation),
        "xReleaseStability": "stable",
        "xContractPolicy": "additiveOnly",
        "xErrorShape": {
            "code": "string",
            "message": "string",
            "operation": "string|null",
            "details": "object"
        },
        "xResourceLimits": {
            "maxRecommendedInputBytes": 1048576,
            "largePayloadBehavior": "reject or deterministically truncate by operation-specific limit"
        }
    })
}

pub fn surface_output_schema(operation: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["operation", "title", "message", "summary", "result"],
        "properties": {
            "operation": {"type": "string", "const": operation},
            "title": {"type": "string", "minLength": 1},
            "message": {"type": "string", "minLength": 1},
            "summary": {"type": "object"},
            "result": {}
        },
        "additionalProperties": true
    })
}

pub fn operation_category(operation: &str) -> &'static str {
    match operation {
        "describe"
        | "analysis.describe"
        | "classification.models"
        | "classification.schema"
        | "embeddings.backends"
        | "index.open"
        | "index.inspect"
        | "qa.models" => "debug",
        "index.removeDocuments" | "runtime.softmax" => "support",
        _ => "workflow",
    }
}

fn required_fields_for_operation(
    operation: &str,
    example_request: &serde_json::Value,
) -> Vec<String> {
    if operation == "describe" || operation.ends_with(".models") || operation.ends_with(".describe")
    {
        return Vec::new();
    }
    let optional = [
        "dimensions",
        "embedding",
        "id",
        "includeNearDuplicates",
        "includePunctuation",
        "includeSemanticNeighbors",
        "keywordLimit",
        "linguistics",
        "lowercase",
        "maxAlternatives",
        "maxTokens",
        "minTokensForDecision",
        "mode",
        "model",
        "n",
        "ngramSizes",
        "normalizeWhitespace",
        "options",
        "order",
        "profile",
        "previewLimit",
        "seed",
        "sentenceLevel",
        "shingleSizes",
        "streamId",
        "summarySentences",
        "topK",
        "truncation",
    ];
    match example_request {
        serde_json::Value::Object(object) => object
            .keys()
            .filter(|key| !optional.contains(&key.as_str()))
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

fn infer_schema_for_value(key: &str, value: &serde_json::Value) -> serde_json::Value {
    let mut schema = match value {
        serde_json::Value::Bool(_) => serde_json::json!({"type": "boolean"}),
        serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => {
            serde_json::json!({"type": "integer", "minimum": 0})
        }
        serde_json::Value::Number(_) => serde_json::json!({"type": "number"}),
        serde_json::Value::String(_) => serde_json::json!({"type": "string", "minLength": 1}),
        serde_json::Value::Array(values) => {
            let item_schema = values
                .first()
                .map(|value| infer_schema_for_value("item", value))
                .unwrap_or_else(|| serde_json::json!({}));
            serde_json::json!({"type": "array", "items": item_schema, "minItems": 1})
        }
        serde_json::Value::Object(object) => serde_json::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": object
                .iter()
                .map(|(key, value)| (key.clone(), infer_schema_for_value(key, value)))
                .collect::<serde_json::Map<_, _>>()
        }),
        serde_json::Value::Null => serde_json::json!({}),
    };
    if matches!(
        key,
        "topK" | "top_k" | "maxTokens" | "max_tokens" | "order" | "dimensions" | "n"
    ) {
        if let serde_json::Value::Object(object) = &mut schema {
            object.insert("minimum".to_string(), serde_json::json!(1));
            object.insert("maximum".to_string(), serde_json::json!(4096));
        }
    }
    schema
}

/// Builds the standard `describe` response without changing the shared
/// `SurfaceResponse` JSON shape.
pub fn describe_surface_response(
    surface: &PackageSurface,
    request: SurfaceRequest,
) -> SurfaceResponse {
    let result = serde_json::json!({
        "library": &surface.library,
        "version": &surface.version,
        "operationCount": surface.operations.len(),
        "operations": surface
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        "input": request.input
    });
    structured_surface_response(
        request.operation,
        "Package surface metadata",
        format!(
            "{} exposes {} package-surface operations.",
            surface.library,
            surface.operations.len()
        ),
        serde_json::json!({
            "operationCount": surface.operations.len(),
            "runtime": {
                "wasm": surface.capabilities.wasm,
                "server": surface.capabilities.server,
                "native": surface.capabilities.native
            }
        }),
        result,
    )
}

/// Builds a successful surface response with empty diagnostics and artifacts.
pub fn surface_response(operation: OperationId, value: serde_json::Value) -> SurfaceResponse {
    let title = operation.as_str().to_string();
    let message = format!("Ran package-surface operation `{}`.", operation.as_str());
    let value = ensure_structured_surface_value(&operation, title, message, value);
    SurfaceResponse {
        operation,
        value,
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
    }
}

/// Builds a package-surface value with standard human-readable metadata while
/// preserving object fields from the concrete operation result at the top level.
pub fn structured_surface_value(
    operation: &OperationId,
    title: impl Into<String>,
    message: impl Into<String>,
    summary: serde_json::Value,
    result: serde_json::Value,
) -> serde_json::Value {
    let mut object = match &result {
        serde_json::Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    object.insert("title".to_string(), serde_json::Value::String(title.into()));
    object.insert(
        "operation".to_string(),
        serde_json::Value::String(operation.as_str().to_string()),
    );
    object.insert(
        "message".to_string(),
        serde_json::Value::String(message.into()),
    );
    object.insert("summary".to_string(), summary);
    object.insert("result".to_string(), result);
    serde_json::Value::Object(object)
}

/// Adds the common package-surface UI fields to a result value when they are
/// missing, while preserving every existing top-level domain field.
pub fn ensure_structured_surface_value(
    operation: &OperationId,
    title: impl Into<String>,
    message: impl Into<String>,
    value: serde_json::Value,
) -> serde_json::Value {
    let result = value.clone();
    let mut object = match value {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    object
        .entry("operation".to_string())
        .or_insert_with(|| serde_json::Value::String(operation.as_str().to_string()));
    object
        .entry("title".to_string())
        .or_insert_with(|| serde_json::Value::String(title.into()));
    object
        .entry("message".to_string())
        .or_insert_with(|| serde_json::Value::String(message.into()));
    object
        .entry("summary".to_string())
        .or_insert_with(|| operation_summary(&result));
    object.entry("result".to_string()).or_insert(result);
    serde_json::Value::Object(object)
}

/// Builds a successful package-surface response using `structured_surface_value`.
pub fn structured_surface_response(
    operation: OperationId,
    title: impl Into<String>,
    message: impl Into<String>,
    summary: serde_json::Value,
    result: serde_json::Value,
) -> SurfaceResponse {
    let value = structured_surface_value(&operation, title, message, summary, result);
    surface_response(operation, value)
}

/// Builds a structured response for an operation listed in a package surface.
///
/// This keeps the concrete operation result at the top level for compatibility,
/// while adding the common `title`, `message`, `summary`, and `result` fields
/// expected by package-surface UIs.
pub fn structured_operation_response(
    surface: &PackageSurface,
    operation: OperationId,
    result: serde_json::Value,
) -> SurfaceResponse {
    let metadata = surface
        .operations
        .iter()
        .find(|candidate| candidate.id.as_str() == operation.as_str());
    let title = metadata
        .map(|operation| operation.name.clone())
        .unwrap_or_else(|| operation.as_str().to_string());
    let message = metadata
        .and_then(|operation| operation.description.clone())
        .unwrap_or_else(|| format!("Ran package-surface operation `{}`.", operation.as_str()));
    let summary = operation_summary(&result);
    structured_surface_response(operation, title, message, summary, result)
}

fn operation_summary(result: &serde_json::Value) -> serde_json::Value {
    match result {
        serde_json::Value::Object(object) => {
            let mut summary = serde_json::Map::new();
            summary.insert("status".to_string(), serde_json::json!("ok"));
            for key in [
                "count",
                "width",
                "height",
                "format",
                "pixelFormat",
                "dimensions",
                "operationCount",
            ] {
                if let Some(value) = object.get(key) {
                    summary.insert(key.to_string(), value.clone());
                }
            }
            if let Some((key, value)) = object
                .iter()
                .find(|(_, value)| matches!(value, serde_json::Value::Array(_)))
            {
                summary.insert(
                    format!("{key}Count"),
                    serde_json::json!(value.as_array().map(Vec::len).unwrap_or(0)),
                );
            }
            serde_json::Value::Object(summary)
        }
        serde_json::Value::Array(values) => {
            serde_json::json!({"status": "ok", "count": values.len()})
        }
        _ => serde_json::json!({"status": "ok"}),
    }
}

/// Shared helpers for thin package-surface CLI adapters.
pub mod cli {
    use std::fs;
    use std::io::{self, Read};

    use super::{
        ensure_structured_surface_value, OperationId, PackageSurface, SurfaceRequest,
        SurfaceResponse,
    };

    /// Static package metadata for one CLI adapter.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CliAdapterMetadata {
        pub library_crate: &'static str,
        pub surface_kind: &'static str,
        pub library_import: &'static str,
        pub server_package: &'static str,
        pub app_package: &'static str,
        pub wasm_package: &'static str,
    }

    /// Builds the standard CLI adapter metadata payload.
    pub fn package_metadata_json(metadata: CliAdapterMetadata, surface: PackageSurface) -> String {
        serde_json::json!({
            "package": format!("{}-cli", metadata.library_crate),
            "surface": metadata.surface_kind,
            "library": metadata.library_crate,
            "libraryImport": metadata.library_import,
            "serverPackage": metadata.server_package,
            "appPackage": metadata.app_package,
            "wasmPackage": metadata.wasm_package,
            "operations": surface.operations
        })
        .to_string()
    }

    /// Builds the standard CLI command schema payload.
    pub fn command_schema_json() -> String {
        serde_json::json!({
            "commands": [
                {"name": "info", "description": "Print package and adapter metadata."},
                {"name": "schema", "description": "Print the CLI command schema."},
                {"name": "operations", "description": "Print library operations."},
                {"name": "run", "description": "Run one library-owned operation."}
            ]
        })
        .to_string()
    }

    /// Reads a JSON request from `--json`, `--file`, or stdin.
    pub fn read_json_input(
        json: Option<String>,
        file: Option<String>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let input = if let Some(json) = json {
            json
        } else if let Some(file) = file {
            fs::read_to_string(file)?
        } else {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            if buffer.trim().is_empty() {
                "{}".to_string()
            } else {
                buffer
            }
        };
        Ok(serde_json::from_str(&input)?)
    }

    /// Runs an operation through a library-owned surface and adds standard
    /// package-surface value fields if an older surface omitted them.
    pub fn run_wrapped_operation(
        operation: &str,
        input: serde_json::Value,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> Result<SurfaceResponse, String> {
        let mut response = runner(SurfaceRequest {
            operation: OperationId::new(operation),
            input,
        })?;
        let value = std::mem::take(&mut response.value);
        response.value = ensure_structured_surface_value(
            &response.operation,
            operation.to_string(),
            format!("Ran package-surface operation `{}`.", operation),
            value,
        );
        Ok(response)
    }
}

/// Shared helpers for local package-surface HTTP adapters.
pub mod server {
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};

    use super::{
        parse_surface_error, Diagnostic, DiagnosticSeverity, OperationId, PackageSurface,
        SurfaceRequest, SurfaceResponse,
    };

    /// Static package metadata for one server adapter.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ServerAdapterMetadata {
        pub library_crate: &'static str,
        pub surface_kind: &'static str,
        pub library_import: &'static str,
        pub cli_package: &'static str,
        pub app_package: &'static str,
        pub wasm_package: &'static str,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HttpResponse {
        pub status_code: u16,
        pub reason: &'static str,
        pub content_type: &'static str,
        pub body: String,
    }

    /// Serves the standard local package-surface HTTP API.
    pub fn serve(
        addr: &str,
        metadata: ServerAdapterMetadata,
        surface_provider: fn() -> PackageSurface,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        for stream in listener.incoming() {
            handle_stream(stream?, metadata, surface_provider, runner)?;
        }
        Ok(())
    }

    /// Returns the standard response for one HTTP request.
    pub fn response_for(
        method: &str,
        path: &str,
        body: &str,
        metadata: ServerAdapterMetadata,
        surface_provider: fn() -> PackageSurface,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> HttpResponse {
        match (method, path) {
            ("OPTIONS", _) => HttpResponse {
                status_code: 204,
                reason: "No Content",
                content_type: "application/json",
                body: String::new(),
            },
            ("GET", "/health") => json_response(
                200,
                "OK",
                serde_json::json!({
                    "ok": true,
                    "package": format!("{}-server", metadata.library_crate),
                    "library": metadata.library_crate
                }),
            ),
            ("GET", "/api/package") => json_response(
                200,
                "OK",
                package_metadata_value(metadata, surface_provider()),
            ),
            ("GET", "/api/schema") => {
                json_response(200, "OK", schema_value(metadata, surface_provider()))
            }
            ("GET", "/api/operations") => {
                json_response(200, "OK", serde_json::json!(surface_provider().operations))
            }
            ("POST", "/api/run") => run_response(body, metadata, runner),
            ("POST", path) if path.starts_with("/api/") => {
                let operation = path.trim_start_matches("/api/");
                run_request(
                    SurfaceRequest {
                        operation: OperationId::new(operation),
                        input: parse_json_or_empty(body),
                    },
                    metadata,
                    runner,
                )
            }
            _ => json_response(
                404,
                "Not Found",
                serde_json::json!({
                    "error": "not found",
                    "path": path
                }),
            ),
        }
    }

    /// Builds the standard server adapter metadata payload.
    pub fn package_metadata_json(
        metadata: ServerAdapterMetadata,
        surface: PackageSurface,
    ) -> String {
        package_metadata_value(metadata, surface).to_string()
    }

    fn package_metadata_value(
        metadata: ServerAdapterMetadata,
        surface: PackageSurface,
    ) -> serde_json::Value {
        serde_json::json!({
            "package": format!("{}-server", metadata.library_crate),
            "surface": metadata.surface_kind,
            "library": metadata.library_crate,
            "libraryImport": metadata.library_import,
            "cliPackage": metadata.cli_package,
            "appPackage": metadata.app_package,
            "wasmPackage": metadata.wasm_package,
            "endpoints": [
                "GET /health",
                "GET /api/package",
                "GET /api/schema",
                "GET /api/operations",
                "POST /api/run",
                "POST /api/<operation-id>"
            ],
            "runtimeMetadata": {
                "candleDevice": serde_json::Value::Null
            },
            "operations": surface.operations
        })
    }

    fn schema_value(metadata: ServerAdapterMetadata, surface: PackageSurface) -> serde_json::Value {
        let operations = surface
            .operations
            .into_iter()
            .map(|operation| {
                let path = format!("/api/{}", operation.id.as_str());
                (
                    path,
                    serde_json::json!({
                        "post": {
                            "summary": operation.name,
                            "description": operation.description,
                            "requestBody": operation.input_schema,
                            "responses": {"200": operation.output_schema}
                        }
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();

        serde_json::json!({
            "openapi": "3.1.0",
            "info": {
                "title": format!("{} API", metadata.library_crate),
                "version": env!("CARGO_PKG_VERSION")
            },
            "paths": operations
        })
    }

    fn run_response(
        body: &str,
        metadata: ServerAdapterMetadata,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> HttpResponse {
        let payload = match serde_json::from_str::<serde_json::Value>(body) {
            Ok(value) => value,
            Err(error) => {
                return diagnostic_response(
                    400,
                    "Bad Request",
                    "invalid_request",
                    &format!("invalid JSON: {error}"),
                    metadata,
                );
            }
        };
        let operation = payload
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("describe")
            .to_string();
        let input = payload
            .get("input")
            .cloned()
            .unwrap_or_else(|| payload.clone());
        run_request(
            SurfaceRequest {
                operation: OperationId::new(operation),
                input,
            },
            metadata,
            runner,
        )
    }

    fn run_request(
        request: SurfaceRequest,
        metadata: ServerAdapterMetadata,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> HttpResponse {
        match runner(request) {
            Ok(response) => json_response(200, "OK", serde_json::json!(response)),
            Err(error) => {
                diagnostic_response(400, "Bad Request", "operation_failed", &error, metadata)
            }
        }
    }

    fn diagnostic_response(
        status_code: u16,
        reason: &'static str,
        code: &str,
        message: &str,
        metadata: ServerAdapterMetadata,
    ) -> HttpResponse {
        let parsed = parse_surface_error(message);
        let diagnostic_code = parsed
            .as_ref()
            .map(|error| error.code.as_str())
            .unwrap_or(code);
        let diagnostic_message = parsed
            .as_ref()
            .map(|error| error.message.as_str())
            .unwrap_or(message);
        let details = parsed
            .as_ref()
            .map(|error| error.details.clone())
            .unwrap_or_else(|| serde_json::json!({}));
        json_response(
            status_code,
            reason,
            serde_json::json!({
                "diagnostics": [Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    code: diagnostic_code.into(),
                    message: diagnostic_message.to_string(),
                    source: Some(format!("{}-server", metadata.library_crate)),
                    help: None,
                }],
                "error": {
                    "code": diagnostic_code,
                    "message": diagnostic_message,
                    "details": details
                }
            }),
        )
    }

    fn parse_json_or_empty(body: &str) -> serde_json::Value {
        if body.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(body).unwrap_or_else(|_| serde_json::json!({"raw": body}))
        }
    }

    fn handle_stream(
        mut stream: TcpStream,
        metadata: ServerAdapterMetadata,
        surface_provider: fn() -> PackageSurface,
        runner: fn(SurfaceRequest) -> Result<SurfaceResponse, String>,
    ) -> io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            let trimmed = header.trim_end();
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
        }

        let mut body = vec![0; content_length];
        if content_length > 0 {
            reader.read_exact(&mut body)?;
        }
        let body = String::from_utf8_lossy(&body);

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET");
        let path = parts.next().unwrap_or("/");
        let response = response_for(method, path, &body, metadata, surface_provider, runner);
        write_response(&mut stream, response)
    }

    fn json_response(
        status_code: u16,
        reason: &'static str,
        value: serde_json::Value,
    ) -> HttpResponse {
        HttpResponse {
            status_code,
            reason,
            content_type: "application/json",
            body: value.to_string(),
        }
    }

    fn write_response(stream: &mut TcpStream, response: HttpResponse) -> io::Result<()> {
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nConnection: close\r\n\r\n{}",
            response.status_code,
            response.reason,
            response.content_type,
            response.body.len(),
            response.body
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct JobId(pub String);

impl JobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for JobId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for JobId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct ArtifactId(pub String);

impl ArtifactId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ArtifactId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ArtifactId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_uses_camel_case_json() {
        let diagnostic = Diagnostic::new(DiagnosticSeverity::Warning, "demo.warning", "check");
        let json = serde_json::to_string(&diagnostic).expect("serialize diagnostic");

        assert!(json.contains("\"severity\":\"warning\""));
        assert!(json.contains("\"code\":\"demo.warning\""));
    }

    #[test]
    fn pure_rust_capabilities_allow_wasm_and_server() {
        let capabilities = RuntimeCapabilities::pure_rust();

        assert!(capabilities.native);
        assert!(capabilities.server);
        assert!(capabilities.wasm);
        assert_eq!(capabilities.mobile, MobileCapability::Wasm);
    }

    #[test]
    fn capability_builders_preserve_pure_rust_defaults() {
        let capabilities = RuntimeCapabilities::pure_rust()
            .with_max_recommended_input_bytes(1024)
            .with_requirement("fixture", "test fixture input", false);

        assert!(capabilities.native);
        assert!(capabilities.server);
        assert!(capabilities.wasm);
        assert_eq!(capabilities.max_recommended_input_bytes, Some(1024));
        assert_eq!(capabilities.requirements[0].name, "fixture");
        assert!(!capabilities.requirements[0].required);
    }

    #[test]
    fn package_surface_uses_camel_case_json() {
        let surface = PackageSurface {
            library: "demo-core".to_string(),
            version: "0.1.0".to_string(),
            capabilities: RuntimeCapabilities::pure_rust(),
            operations: vec![SurfaceOperation {
                id: OperationId::new("describe"),
                name: "Describe".to_string(),
                description: Some("Describe package surface".to_string()),
                curation: SurfaceOperationCuration::from_operation_id("describe"),
                input_schema: serde_json::json!({"type": "object"}),
                output_schema: serde_json::json!({"type": "object"}),
                example_request: serde_json::json!({}),
                wasm_supported: true,
                server_supported: true,
            }],
        };

        let json = serde_json::to_string(&surface).expect("serialize surface");

        assert!(json.contains("\"inputSchema\""));
        assert!(json.contains("\"curation\""));
        assert!(json.contains("\"exampleRequest\""));
        assert!(json.contains("\"wasmSupported\":true"));
    }

    #[test]
    fn surface_operation_curation_serializes_and_deserializes() {
        let operation = surface_operation(
            "demo.run",
            "Run demo",
            "Run a demo workflow",
            serde_json::json!({"text": "hello"}),
        );

        let value = serde_json::to_value(&operation).expect("serialize operation");
        assert_eq!(value["curation"]["role"], "workflow");
        assert_eq!(value["curation"]["primary"], false);
        assert_eq!(value["curation"]["sortOrder"], 100);

        let round_trip: SurfaceOperation =
            serde_json::from_value(value).expect("deserialize operation");
        assert_eq!(round_trip.curation, SurfaceOperationCuration::workflow(100));
    }

    #[test]
    fn surface_operation_deserializes_without_curation() {
        let json = serde_json::json!({
            "id": "describe",
            "name": "Describe",
            "description": "Describe package surface",
            "inputSchema": {"type": "object", "xOperationCategory": "debug"},
            "outputSchema": {"type": "object", "xOperationCategory": "debug"},
            "exampleRequest": {},
            "wasmSupported": true,
            "serverSupported": true
        });

        let operation: SurfaceOperation =
            serde_json::from_value(json).expect("deserialize old operation JSON");

        assert_eq!(operation.id.as_str(), "describe");
        assert_eq!(operation.curation, SurfaceOperationCuration::default());
    }

    #[test]
    fn set_surface_operation_curation_syncs_legacy_schema_category() {
        let mut operation = surface_operation(
            "demo.inspect",
            "Inspect demo",
            "Inspect demo inputs",
            serde_json::json!({}),
        );

        set_surface_operation_curation(&mut operation, SurfaceOperationCuration::debug(750));

        assert_eq!(operation.curation, SurfaceOperationCuration::debug(750));
        assert_eq!(operation.input_schema["xOperationCategory"], "debug");
        assert_eq!(operation.output_schema["xOperationCategory"], "debug");
    }

    #[test]
    fn surface_helpers_preserve_standard_response_shape() {
        let surface = PackageSurface {
            library: "demo".to_string(),
            version: "0.1.0".to_string(),
            capabilities: RuntimeCapabilities::pure_rust(),
            operations: vec![surface_operation(
                "describe",
                "Describe",
                "Describe demo package",
                serde_json::json!({"includeOperations": true}),
            )],
        };
        let response = describe_surface_response(
            &surface,
            SurfaceRequest {
                operation: OperationId::new("describe"),
                input: serde_json::json!({"includeOperations": true}),
            },
        );

        assert_eq!(response.operation.as_str(), "describe");
        assert_eq!(response.value["library"], "demo");
        assert_eq!(response.value["operationCount"], 1);
        assert_eq!(response.diagnostics, Vec::new());
        assert_eq!(response.artifacts, Vec::<serde_json::Value>::new());
    }

    #[test]
    fn surface_operation_declares_release_contract_schema() {
        let operation = surface_operation(
            "demo.run",
            "Run demo",
            "Run a demo workflow",
            serde_json::json!({"text": "hello", "topK": 3}),
        );

        assert_eq!(operation.input_schema["additionalProperties"], false);
        assert_eq!(operation.input_schema["xOperationCategory"], "workflow");
        assert_eq!(operation.input_schema["xReleaseStability"], "stable");
        assert_eq!(
            operation.input_schema["required"],
            serde_json::json!(["text"])
        );
        assert_eq!(operation.input_schema["properties"]["topK"]["minimum"], 1);
        assert_eq!(operation.output_schema["required"][0], "operation");
    }

    #[test]
    fn surface_operation_schema_extension_updates_both_schemas() {
        let mut operation = surface_operation(
            "demo.compat",
            "Run compatibility demo",
            "Run a compatibility helper",
            serde_json::json!({"text": "hello"}),
        );

        add_surface_operation_schema_extension(
            &mut operation,
            "xReplacementOperation",
            serde_json::json!("demo.run"),
        );

        assert_eq!(operation.input_schema["xReplacementOperation"], "demo.run");
        assert_eq!(operation.output_schema["xReplacementOperation"], "demo.run");
    }

    #[test]
    fn typed_surface_errors_roundtrip_for_transport_adapters() {
        let error = SurfaceError::unsupported_operation("demo.missing", "demo-package");
        let serialized = error.to_error_string();
        let parsed = parse_surface_error(&serialized).expect("typed surface error");

        assert_eq!(parsed.code, "unsupported_operation");
        assert_eq!(parsed.operation.unwrap().as_str(), "demo.missing");
        assert!(parsed.message.contains("unsupported operation"));
    }

    #[test]
    fn surface_error_is_standard_error_type() {
        let error = SurfaceError::invalid_request(Some("demo.run"), "invalid request: missing id");
        let boxed: Box<dyn std::error::Error> = Box::new(error.clone());

        assert_eq!(boxed.to_string(), "invalid request: missing id");
        assert_eq!(error.to_string(), "invalid request: missing id");
    }

    #[test]
    fn validation_helpers_return_typed_errors() {
        let limit = validate_max_items("demo.run", "values", 3, 2).expect_err("limit error");
        let parsed = parse_surface_error(&limit).expect("typed resource error");
        assert_eq!(parsed.code, "resource_limit");
        assert_eq!(parsed.details["field"], "values");
        assert_eq!(parsed.details["actual"], 3);

        let length =
            validate_matching_lengths("demo.run", "left", 2, "right", 3).expect_err("length");
        let parsed = parse_surface_error(&length).expect("typed length error");
        assert_eq!(parsed.code, "invalid_request");
        assert!(parsed.message.contains("left"));
        assert!(parsed.message.contains("right"));
    }

    #[test]
    fn execution_plan_serializes_to_schema_extension() {
        let plan = SurfaceExecutionPlan {
            operation: OperationId::new("demo.run"),
            mode: SurfaceExecutionMode::PlannedJob,
            side_effects: vec![SurfaceSideEffect::None],
            cancellable: true,
            progress_unit: Some("items".to_string()),
            expected_artifacts: vec![SurfaceArtifactExpectation {
                id: "report".to_string(),
                kind: "json".to_string(),
                media_type: "application/json".to_string(),
                required: true,
                description: Some("Structured report".to_string()),
            }],
            requirements: vec![RuntimeRequirement {
                name: "runtime-core".to_string(),
                description: Some("Pure Rust planner".to_string()),
                required: true,
            }],
            max_recommended_input_bytes: Some(1024),
        };

        let operation = surface_operation_with_execution_plan(
            "demo.run",
            "Run demo",
            "Build a deterministic demo plan",
            serde_json::json!({"items": [1]}),
            plan,
        );

        assert_eq!(
            operation.input_schema["xExecutionPlan"]["mode"],
            serde_json::json!("plannedJob")
        );
        assert_eq!(
            operation.output_schema["xExecutionPlan"]["expectedArtifacts"][0]["id"],
            serde_json::json!("report")
        );
        assert_eq!(
            operation.input_schema["xExecutionPlan"]["sideEffects"],
            serde_json::json!(["none"])
        );
    }

    #[test]
    fn runtime_context_serializes_and_tracks_no_side_effect_defaults() {
        let context = SurfaceRuntimeContext::no_side_effects(SurfaceRuntimeKind::Wasm);
        assert!(!context.side_effects.allow_reads);
        assert!(!context.side_effects.allow_writes);
        assert!(!context.side_effects.allow_network);
        assert_eq!(
            context.model.preference,
            SurfaceModelExecutionPreference::PlanOnly
        );
        assert!(!context.allows_model_auto_setup());

        let value = serde_json::to_value(&context).expect("serialize context");
        assert_eq!(value["runtime"], "wasm");
        assert_eq!(value["sideEffects"]["allowWrites"], false);
        assert_eq!(value["storage"]["modelRoot"], serde_json::Value::Null);

        let round_trip: SurfaceRuntimeContext =
            serde_json::from_value(value).expect("deserialize context");
        assert_eq!(round_trip, context);
    }

    #[test]
    fn primary_workflow_operation_attaches_strict_metadata_and_lower_contracts() {
        let operation = primary_workflow_operation(
            "demo.workflow",
            "Run workflow",
            "Runs the primary workflow.",
            serde_json::json!({"input": "value"}),
            None,
            &["moenarch-runtime-core", "moenarch-video-analysis-core"],
        );

        assert_eq!(
            operation.input_schema["additionalProperties"],
            serde_json::json!(false)
        );
        assert_eq!(operation.input_schema["xOperationCategory"], "workflow");
        assert_eq!(operation.input_schema["xReleaseStability"], "stable");
        assert_eq!(operation.input_schema["xContractPolicy"], "additiveOnly");
        assert_eq!(
            operation.input_schema["xErrorShape"]["code"],
            serde_json::json!("string")
        );
        assert_eq!(
            operation.input_schema["xLowerContractProof"]["crates"][1],
            "moenarch-video-analysis-core"
        );
    }

    #[test]
    fn landscape_contract_serializes_to_schema_extension() {
        let contract = landscape::LandscapeOperationContract::new(
            landscape::LandscapeFunction::new("demo.curated", "moenarch-runtime-core")
                .input(landscape::LandscapePort::new(
                    "request",
                    landscape::well_known::runtime_surface_request(),
                ))
                .output(landscape::LandscapePort::new(
                    "response",
                    landscape::well_known::runtime_surface_response(),
                )),
        );

        let operation = surface_operation_with_landscape(
            "demo.run",
            "Run demo",
            "Run a curated demo workflow",
            serde_json::json!({"request": {"operation": "demo.run", "input": {}}}),
            contract,
        );

        assert_eq!(operation.id.as_str(), "demo.run");
        assert_eq!(
            operation.example_request["request"]["operation"],
            "demo.run"
        );
        assert!(operation.wasm_supported);
        assert!(operation.server_supported);
        assert_eq!(
            operation.input_schema["xLandscape"]["function"]["id"],
            serde_json::json!("demo.curated")
        );
        assert_eq!(
            operation.input_schema["xLandscape"]["function"]["inputs"][0]["typeRef"]["id"],
            serde_json::json!("runtime.surfaceRequest")
        );
        assert_eq!(
            operation.output_schema["xLandscape"]["function"]["outputs"][0]["typeRef"]["rustType"],
            serde_json::json!("runtime_core::SurfaceResponse")
        );
    }

    #[test]
    fn landscape_contract_value_uses_camel_case_json() {
        let contract = landscape::LandscapeOperationContract::new(
            landscape::LandscapeFunction::new("demo.curated", "moenarch-runtime-core")
                .input(
                    landscape::LandscapePort::new(
                        "optionalRequest",
                        landscape::well_known::runtime_surface_request(),
                    )
                    .optional(),
                )
                .output(
                    landscape::LandscapePort::new(
                        "responses",
                        landscape::well_known::runtime_surface_response(),
                    )
                    .many(),
                ),
        );
        let value = landscape_operation_contract_value(&contract);

        assert_eq!(value["function"]["stability"], "stable");
        assert_eq!(
            value["function"]["inputs"][0]["typeRef"]["schemaRef"],
            serde_json::Value::Null
        );
        assert_eq!(value["function"]["inputs"][0]["required"], false);
        assert_eq!(value["function"]["inputs"][0]["cardinality"], "optional");
        assert_eq!(value["function"]["outputs"][0]["cardinality"], "many");
    }

    #[test]
    fn new_surface_error_constructors_are_typed_json() {
        let cancelled = SurfaceError::cancelled("demo.run", "cancelled by request");
        assert_eq!(cancelled.code, "cancelled");
        assert_eq!(cancelled.operation.unwrap().as_str(), "demo.run");

        let execution = SurfaceError::execution_failed(
            "demo.run",
            "execution failed",
            serde_json::json!({
                "stage": "prepare"
            }),
        );
        assert_eq!(execution.code, "execution_failed");
        assert_eq!(execution.details["stage"], "prepare");

        let artifact = SurfaceError::artifact_error(
            "demo.run",
            "artifact invalid",
            serde_json::json!({
                "artifact": "report"
            }),
        );
        assert_eq!(artifact.code, "artifact_error");
        assert_eq!(artifact.details["artifact"], "report");
    }

    #[test]
    fn structured_operation_response_preserves_result_fields() {
        let surface = PackageSurface {
            library: "demo".to_string(),
            version: "0.1.0".to_string(),
            capabilities: RuntimeCapabilities::pure_rust(),
            operations: vec![surface_operation(
                "demo.run",
                "Run demo",
                "Run a demo workflow",
                serde_json::json!({"values": [1, 2]}),
            )],
        };
        let response = structured_operation_response(
            &surface,
            OperationId::new("demo.run"),
            serde_json::json!({"count": 2, "values": [1, 2]}),
        );

        assert_eq!(response.value["count"], 2);
        assert_eq!(response.value["operation"], "demo.run");
        assert_eq!(response.value["title"], "Run demo");
        assert_eq!(response.value["summary"]["count"], 2);
        assert_eq!(
            response.value["result"]["values"],
            serde_json::json!([1, 2])
        );
    }
}
