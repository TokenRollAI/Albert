use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStage {
    Planned,
    Scaffolded,
    Partial,
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityStatus {
    pub name: String,
    pub stage: DeliveryStage,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppBootstrapSummary {
    pub project_name: String,
    pub current_phase: String,
    pub ui_surfaces: Vec<String>,
    pub parser_capabilities: Vec<CapabilityStatus>,
    pub storage_capabilities: Vec<CapabilityStatus>,
    pub provider_capabilities: Vec<CapabilityStatus>,
    pub gateway_capabilities: Vec<CapabilityStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputSourceKind {
    OpenApi,
    Curl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Head,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalApiCollection {
    pub id: String,
    pub name: String,
    pub source: InputSourceKind,
    pub description: Option<String>,
    pub endpoints: Vec<CanonicalEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalEndpoint {
    pub operation_id: Option<String>,
    pub method: HttpMethod,
    pub path: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Vec<CanonicalParameter>,
    pub request_body: Option<CanonicalRequestBody>,
    pub responses: Vec<CanonicalResponse>,
    pub examples: Vec<MockExample>,
    /// Hint extracted from the source spec (OpenAPI security) describing
    /// the simplest auth header expected by this endpoint. The mock
    /// gateway does not enforce it on its own — the UI surfaces it so the
    /// user can opt into seeding a real `required_headers` rule. Stays
    /// `None` for cURL imports and for endpoints with no security
    /// requirement.
    #[serde(default)]
    pub auth: Option<AuthRequirement>,
}

/// Minimal description of an endpoint's auth expectation — enough to
/// derive a `gateway::RequiredHeader` rule when the user asks to seed one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthRequirement {
    pub scheme: AuthScheme,
    /// Header the client is expected to send (e.g. `Authorization`, or
    /// a custom name for header-placed API keys).
    pub header_name: String,
    /// Prefix the header value must start with — typically `Bearer ` for
    /// HTTP bearer tokens or `Basic ` for HTTP basic. `None` for API keys
    /// (the raw key is the whole value).
    #[serde(default)]
    pub value_prefix: Option<String>,
    /// Free-form description lifted from the OpenAPI securityScheme, so
    /// the UI can hint at expected formats without re-reading the spec.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    /// RFC 6750 bearer tokens via the `Authorization` header.
    HttpBearer,
    /// RFC 7617 basic auth via the `Authorization` header.
    HttpBasic,
    /// OpenAPI `apiKey in: header` — raw key in a custom header.
    ApiKeyHeader,
    /// OAuth2 flows always send `Authorization: Bearer <token>` on
    /// request, so we normalize them to the bearer shape when seeding.
    #[serde(rename = "oauth2")]
    OAuth2,
    /// Anything we can't map faithfully (mTLS, OIDC with unusual
    /// placement). Kept as a hint so the UI can show a note without
    /// attempting to generate a gate.
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalParameter {
    pub name: String,
    pub location: ParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub schema: SchemaNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalRequestBody {
    pub content_type: String,
    pub required: bool,
    pub schema: SchemaNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalResponse {
    pub status_code: String,
    pub description: Option<String>,
    pub content_type: String,
    pub schema: Option<SchemaNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchemaNodeType {
    Object,
    Array,
    String,
    Integer,
    Number,
    Boolean,
    Null,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaNode {
    pub node_type: SchemaNodeType,
    pub description: Option<String>,
    pub required: bool,
    pub nullable: bool,
    pub properties: BTreeMap<String, SchemaNode>,
    pub items: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub prefix_items: Vec<SchemaNode>,
    pub enum_values: Vec<Value>,
    pub example: Option<Value>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(default)]
    pub exclusive_minimum: bool,
    #[serde(default)]
    pub exclusive_maximum: bool,
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub contains: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub min_contains: Option<usize>,
    #[serde(default)]
    pub max_contains: Option<usize>,
    #[serde(default)]
    pub min_properties: Option<usize>,
    #[serde(default)]
    pub max_properties: Option<usize>,
    #[serde(default)]
    pub dependent_required: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub dependent_schemas: BTreeMap<String, SchemaNode>,
    #[serde(default)]
    pub if_schema: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub then_schema: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub else_schema: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub unique_items: bool,
    #[serde(default)]
    pub multiple_of: Option<f64>,
    #[serde(default)]
    pub additional_properties: Option<Box<SchemaNode>>,
    #[serde(default)]
    pub allow_additional_properties: bool,
    #[serde(default)]
    pub allow_unevaluated_properties: bool,
    #[serde(default)]
    pub allow_unevaluated_items: bool,
    #[serde(default)]
    pub bool_schema: Option<bool>,
}

impl SchemaNode {
    pub fn object() -> Self {
        Self {
            node_type: SchemaNodeType::Object,
            description: None,
            required: false,
            nullable: false,
            properties: BTreeMap::new(),
            items: None,
            prefix_items: Vec::new(),
            enum_values: Vec::new(),
            example: None,
            format: None,
            pattern: None,
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            exclusive_minimum: false,
            exclusive_maximum: false,
            min_items: None,
            max_items: None,
            contains: None,
            min_contains: None,
            max_contains: None,
            min_properties: None,
            max_properties: None,
            dependent_required: BTreeMap::new(),
            dependent_schemas: BTreeMap::new(),
            if_schema: None,
            then_schema: None,
            else_schema: None,
            unique_items: false,
            multiple_of: None,
            additional_properties: None,
            allow_additional_properties: true,
            allow_unevaluated_properties: true,
            allow_unevaluated_items: true,
            bool_schema: None,
        }
    }

    pub fn string() -> Self {
        Self {
            node_type: SchemaNodeType::String,
            description: None,
            required: false,
            nullable: false,
            properties: BTreeMap::new(),
            items: None,
            prefix_items: Vec::new(),
            enum_values: Vec::new(),
            example: None,
            format: None,
            pattern: None,
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            exclusive_minimum: false,
            exclusive_maximum: false,
            min_items: None,
            max_items: None,
            contains: None,
            min_contains: None,
            max_contains: None,
            min_properties: None,
            max_properties: None,
            dependent_required: BTreeMap::new(),
            dependent_schemas: BTreeMap::new(),
            if_schema: None,
            then_schema: None,
            else_schema: None,
            unique_items: false,
            multiple_of: None,
            additional_properties: None,
            allow_additional_properties: true,
            allow_unevaluated_properties: true,
            allow_unevaluated_items: true,
            bool_schema: None,
        }
    }

    pub fn array(items: SchemaNode) -> Self {
        Self {
            node_type: SchemaNodeType::Array,
            description: None,
            required: false,
            nullable: false,
            properties: BTreeMap::new(),
            items: Some(Box::new(items)),
            prefix_items: Vec::new(),
            enum_values: Vec::new(),
            example: None,
            format: None,
            pattern: None,
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            exclusive_minimum: false,
            exclusive_maximum: false,
            min_items: None,
            max_items: None,
            contains: None,
            min_contains: None,
            max_contains: None,
            min_properties: None,
            max_properties: None,
            dependent_required: BTreeMap::new(),
            dependent_schemas: BTreeMap::new(),
            if_schema: None,
            then_schema: None,
            else_schema: None,
            unique_items: false,
            multiple_of: None,
            additional_properties: None,
            allow_additional_properties: true,
            allow_unevaluated_properties: true,
            allow_unevaluated_items: true,
            bool_schema: None,
        }
    }

    pub fn bool_schema(value: bool) -> Self {
        Self {
            node_type: SchemaNodeType::Unknown,
            bool_schema: Some(value),
            ..Self::string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MockExampleKind {
    Success,
    Empty,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockExample {
    pub kind: MockExampleKind,
    pub title: String,
    pub payload: Value,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderApiType {
    #[serde(rename = "openai_compatible", alias = "open_ai_compatible")]
    #[default]
    OpenAiCompatible,
    #[serde(rename = "azure_openai", alias = "azure_open_ai")]
    AzureOpenAi,
    #[serde(rename = "openai_responses", alias = "open_ai_responses")]
    OpenAiResponses,
    #[serde(rename = "azure_openai_responses", alias = "azure_open_ai_responses")]
    AzureOpenAiResponses,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

impl ProviderReasoningEffort {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub provider_name: String,
    #[serde(default)]
    pub environment: Option<String>,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default)]
    pub api_type: ProviderApiType,
    #[serde(default)]
    pub azure_deployment: Option<String>,
    #[serde(default)]
    pub azure_api_version: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning_effort: Option<ProviderReasoningEffort>,
    #[serde(default)]
    pub schema_repair_attempts: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockHttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockHttpResponse {
    pub status_code: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestFingerprintError {
    pub message: String,
}

impl std::fmt::Display for RequestFingerprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RequestFingerprintError {}

/// Normalize the request snapshot shape used by Try-it and the mock gateway
/// before computing a request fingerprint. Header keys are case-insensitive,
/// and sensitive header values are redacted so auth tokens do not affect
/// persisted cache rows or leak through fingerprints.
pub fn normalize_request_snapshot(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut normalized = serde_json::Map::new();
            for (key, value) in map {
                normalized.insert(key.clone(), normalize_request_field(key, value));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

fn normalize_request_field(key: &str, value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            if key.eq_ignore_ascii_case("headers") {
                let mut normalized = serde_json::Map::new();
                for (child_key, child_value) in map {
                    normalized.insert(
                        child_key.to_ascii_lowercase(),
                        normalize_header_value(child_key, child_value),
                    );
                }
                return Value::Object(normalized);
            }
            Value::Object(
                map.iter()
                    .map(|(child_key, child_value)| {
                        (
                            child_key.clone(),
                            normalize_request_field(child_key, child_value),
                        )
                    })
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize_request_field(key, item))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn normalize_header_value(header_name: &str, value: &Value) -> Value {
    if is_sensitive_header_name(header_name) {
        Value::String("<redacted>".to_string())
    } else {
        normalize_request_field(header_name, value)
    }
}

pub fn is_sensitive_header_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "AUTH", "TOKEN", "SECRET", "PASSWORD", "COOKIE", "API_KEY", "APIKEY",
    ]
    .iter()
    .any(|marker| upper.contains(marker))
}

pub fn request_fingerprint(
    method: &str,
    path: &str,
    request_snapshot: &Value,
) -> Result<String, RequestFingerprintError> {
    let canonical = serde_json::to_string(&serde_json::json!({
        "method": method.trim().to_ascii_uppercase(),
        "path": path.trim(),
        "request": normalize_request_snapshot(request_snapshot)
    }))
    .map_err(|error| RequestFingerprintError {
        message: error.to_string(),
    })?;
    Ok(format!("{:016x}", fnv1a64(canonical.as_bytes())))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
        }
    }
}

impl InputSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenApi => "openapi",
            Self::Curl => "curl",
        }
    }
}

impl MockExampleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }
}

/// Strategy for walking a `SchemaNode` when synthesizing a mock payload.
#[derive(Debug, Clone, Copy)]
enum SynthMode {
    Rich,
    Empty,
    Error,
}

/// Walk a `SchemaNode` and produce a plausible JSON value that matches its
/// shape. When `example` is set on the node we use it directly; otherwise we
/// fall back to per-type defaults.
pub fn synthesize_value(schema: &SchemaNode) -> Value {
    synthesize_value_with_name(schema, None, SynthMode::Rich)
}

fn synthesize_value_with_name(schema: &SchemaNode, name: Option<&str>, mode: SynthMode) -> Value {
    if let Some(example) = &schema.example {
        return example.clone();
    }
    match schema.node_type {
        SchemaNodeType::Object => {
            let mut obj = serde_json::Map::new();
            for (prop, child) in schema.properties.iter() {
                obj.insert(
                    prop.clone(),
                    synthesize_value_with_name(child, Some(prop), mode),
                );
            }
            Value::Object(obj)
        }
        SchemaNodeType::Array => {
            if matches!(mode, SynthMode::Empty) {
                return Value::Array(Vec::new());
            }
            let items = schema
                .items
                .as_deref()
                .map(|inner| synthesize_value_with_name(inner, None, mode))
                .unwrap_or(Value::Null);
            Value::Array(vec![items])
        }
        SchemaNodeType::String => {
            if let Some(first_enum) = schema.enum_values.first() {
                return first_enum.clone();
            }
            if matches!(mode, SynthMode::Empty) {
                return Value::String(String::new());
            }
            let hint = name.unwrap_or("value").to_ascii_lowercase();
            let sample = match hint.as_str() {
                "id" | "uuid" | "guid" => "00000000-0000-4000-8000-000000000000",
                n if n.ends_with("id") => "00000000-0000-4000-8000-000000000000",
                n if n.ends_with("at") || n.contains("time") || n.contains("date") => {
                    "2026-01-01T00:00:00Z"
                }
                n if n.contains("email") => "user@example.com",
                n if n.contains("url") || n.contains("link") => "https://example.com",
                n if n.contains("name") => "Ada Lovelace",
                n if n.contains("phone") => "+1-555-0100",
                _ => "sample",
            };
            Value::String(sample.to_string())
        }
        SchemaNodeType::Integer => {
            if let Some(first_enum) = schema.enum_values.first() {
                return first_enum.clone();
            }
            Value::Number(serde_json::Number::from(
                if matches!(mode, SynthMode::Empty) {
                    0
                } else {
                    42
                },
            ))
        }
        SchemaNodeType::Number => {
            let value = if matches!(mode, SynthMode::Empty) {
                0.0
            } else {
                1.5
            };
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        SchemaNodeType::Boolean => {
            Value::Bool(!matches!(mode, SynthMode::Empty | SynthMode::Error))
        }
        SchemaNodeType::Null => Value::Null,
        SchemaNodeType::Unknown => {
            if schema.nullable {
                Value::Null
            } else {
                Value::String(String::new())
            }
        }
    }
}

fn pick_response(
    responses: &[CanonicalResponse],
    matches: fn(&str) -> bool,
) -> Option<&CanonicalResponse> {
    responses.iter().find(|r| matches(&r.status_code))
}

fn is_success_status(code: &str) -> bool {
    code.starts_with('2')
}

fn is_error_status(code: &str) -> bool {
    code.starts_with('4') || code.starts_with('5')
}

impl AuthRequirement {
    /// Whether this requirement maps cleanly onto a header rule the gateway
    /// can enforce. `Other` schemes (OIDC, mTLS, unusual placements) are
    /// surfaced as hints but do not seed rules.
    pub fn seedable(&self) -> bool {
        matches!(
            self.scheme,
            AuthScheme::HttpBearer
                | AuthScheme::HttpBasic
                | AuthScheme::ApiKeyHeader
                | AuthScheme::OAuth2
        )
    }
}

/// Produce `success / empty / error` mock examples for an endpoint by walking
/// its response schemas. Falls back to the generic default payload when a
/// response is missing.
pub fn synthesize_examples(endpoint: &CanonicalEndpoint) -> Vec<MockExample> {
    let success_schema =
        pick_response(&endpoint.responses, is_success_status).and_then(|r| r.schema.as_ref());
    let error_schema =
        pick_response(&endpoint.responses, is_error_status).and_then(|r| r.schema.as_ref());

    let success_payload = success_schema
        .map(|s| synthesize_value_with_name(s, None, SynthMode::Rich))
        .unwrap_or_else(|| json!({"success": true}));

    let empty_payload = success_schema
        .map(|s| synthesize_value_with_name(s, None, SynthMode::Empty))
        .unwrap_or_else(|| json!({"data": []}));

    let error_payload = error_schema
        .map(|s| synthesize_value_with_name(s, None, SynthMode::Error))
        .unwrap_or_else(|| {
            json!({
                "error": {
                    "code": "bad_request",
                    "message": "Mock error"
                }
            })
        });

    vec![
        MockExample {
            kind: MockExampleKind::Success,
            title: "Success".to_string(),
            payload: success_payload,
            note: Some("Synthesized from response schema.".to_string()),
        },
        MockExample {
            kind: MockExampleKind::Empty,
            title: "Empty".to_string(),
            payload: empty_payload,
            note: Some("Empty-state variant synthesized from schema.".to_string()),
        },
        MockExample {
            kind: MockExampleKind::Error,
            title: "Error".to_string(),
            payload: error_payload,
            note: Some("Error variant synthesized from schema or defaults.".to_string()),
        },
    ]
}

/// Validate a JSON value against a canonical `SchemaNode`. Returns a list of
/// human-readable problems — empty slice means the value matched.
///
/// The checker is intentionally conservative: it enforces the declared
/// `node_type`, required-property presence, nullable-null agreement, and enum
/// membership. It does NOT attempt min/max/pattern/format validation — those
/// are expected to be layered on top by callers that care. Arrays validate
/// every item against the declared `items` schema; objects walk `properties`.
pub fn validate_value(schema: &SchemaNode, value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    validate_at(schema, value, "$", &mut errors);
    errors
}

fn validate_at(schema: &SchemaNode, value: &Value, path: &str, errors: &mut Vec<String>) {
    if schema.bool_schema == Some(false) {
        errors.push(format!("{path}: value is not allowed by false schema"));
        return;
    }
    if schema.bool_schema == Some(true) {
        return;
    }
    if value.is_null() {
        if schema.nullable {
            return;
        }
        match schema.node_type {
            SchemaNodeType::Null => return,
            SchemaNodeType::Unknown => return,
            _ => {
                errors.push(format!(
                    "{path}: expected {} but got null",
                    type_label(&schema.node_type)
                ));
                return;
            }
        }
    }
    if !schema.enum_values.is_empty() && !schema.enum_values.iter().any(|v| v == value) {
        errors.push(format!(
            "{path}: value {} is not in the declared enum",
            compact_json(value)
        ));
        return;
    }
    match schema.node_type {
        SchemaNodeType::Object => match value.as_object() {
            Some(obj) => {
                if let Some(min) = schema.min_properties
                    && obj.len() < min
                {
                    errors.push(format!(
                        "{path}: expected at least {min} propert{} but got {}",
                        plural_y(min),
                        obj.len()
                    ));
                }
                if let Some(max) = schema.max_properties
                    && obj.len() > max
                {
                    errors.push(format!(
                        "{path}: expected at most {max} propert{} but got {}",
                        plural_y(max),
                        obj.len()
                    ));
                }
                for (name, child_schema) in &schema.properties {
                    match obj.get(name) {
                        Some(child_value) => {
                            validate_at(
                                child_schema,
                                child_value,
                                &format!("{path}.{name}"),
                                errors,
                            );
                        }
                        None if child_schema.required => {
                            errors.push(format!("{path}.{name}: required property missing"));
                        }
                        None => {}
                    }
                }
                for (name, dependents) in &schema.dependent_required {
                    if !obj.contains_key(name) {
                        continue;
                    }
                    for dependent in dependents {
                        if !obj.contains_key(dependent) {
                            errors.push(format!(
                                "{path}.{dependent}: required because {path}.{name} is present"
                            ));
                        }
                    }
                }
                for (name, dependent_schema) in &schema.dependent_schemas {
                    if obj.contains_key(name) {
                        validate_at(
                            dependent_schema,
                            value,
                            &format!("{path} dependent schema for {name}"),
                            errors,
                        );
                    }
                }
                for (name, child_value) in obj {
                    if schema.properties.contains_key(name) {
                        continue;
                    }
                    if let Some(additional_schema) = schema.additional_properties.as_deref() {
                        validate_at(
                            additional_schema,
                            child_value,
                            &format!("{path}.{name}"),
                            errors,
                        );
                    } else if !schema.allow_additional_properties {
                        errors.push(format!("{path}.{name}: additional property not allowed"));
                    } else if !schema.allow_unevaluated_properties {
                        errors.push(format!("{path}.{name}: unevaluated property not allowed"));
                    }
                }
            }
            None => errors.push(format!(
                "{path}: expected object but got {}",
                json_type_label(value)
            )),
        },
        SchemaNodeType::Array => match value.as_array() {
            Some(items) => {
                if let Some(min) = schema.min_items
                    && items.len() < min
                {
                    errors.push(format!(
                        "{path}: expected at least {min} item(s) but got {}",
                        items.len()
                    ));
                }
                if let Some(max) = schema.max_items
                    && items.len() > max
                {
                    errors.push(format!(
                        "{path}: expected at most {max} item(s) but got {}",
                        items.len()
                    ));
                }
                if schema.unique_items {
                    for (idx, item) in items.iter().enumerate() {
                        if items.iter().take(idx).any(|previous| previous == item) {
                            errors.push(format!(
                                "{path}[{idx}]: array item duplicates an earlier value"
                            ));
                        }
                    }
                }
                if let Some(contains_schema) = schema.contains.as_deref() {
                    let matches = items
                        .iter()
                        .filter(|item| {
                            let mut item_errors = Vec::new();
                            validate_at(contains_schema, item, path, &mut item_errors);
                            item_errors.is_empty()
                        })
                        .count();
                    let min = schema.min_contains.unwrap_or(1);
                    if matches < min {
                        errors.push(format!(
                            "{path}: expected at least {min} item(s) matching contains schema but got {matches}"
                        ));
                    }
                    if let Some(max) = schema.max_contains
                        && matches > max
                    {
                        errors.push(format!(
                            "{path}: expected at most {max} item(s) matching contains schema but got {matches}"
                        ));
                    }
                }
                for (idx, prefix_schema) in schema.prefix_items.iter().enumerate() {
                    if let Some(item) = items.get(idx) {
                        validate_at(prefix_schema, item, &format!("{path}[{idx}]"), errors);
                    }
                }
                let prefix_len = schema.prefix_items.len();
                if let Some(item_schema) = schema.items.as_deref() {
                    for (idx, item) in items.iter().enumerate().skip(prefix_len) {
                        validate_at(item_schema, item, &format!("{path}[{idx}]"), errors);
                    }
                } else if !schema.allow_unevaluated_items && items.len() > prefix_len {
                    for idx in prefix_len..items.len() {
                        errors.push(format!("{path}[{idx}]: unevaluated item not allowed"));
                    }
                }
            }
            None => errors.push(format!(
                "{path}: expected array but got {}",
                json_type_label(value)
            )),
        },
        SchemaNodeType::String => match value.as_str() {
            Some(text) => validate_string_constraints(schema, text, path, errors),
            None => {
                errors.push(format!(
                    "{path}: expected string but got {}",
                    json_type_label(value)
                ));
            }
        },
        SchemaNodeType::Integer => {
            let is_integer = match value {
                Value::Number(n) => {
                    n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|f| f.fract() == 0.0)
                }
                _ => false,
            };
            if !is_integer {
                errors.push(format!(
                    "{path}: expected integer but got {}",
                    json_type_label(value)
                ));
            } else if let Some(number) = value.as_f64() {
                validate_number_constraints(schema, number, path, errors);
            }
        }
        SchemaNodeType::Number => match value.as_f64() {
            Some(number) => validate_number_constraints(schema, number, path, errors),
            None => {
                errors.push(format!(
                    "{path}: expected number but got {}",
                    json_type_label(value)
                ));
            }
        },
        SchemaNodeType::Boolean => {
            if !value.is_boolean() {
                errors.push(format!(
                    "{path}: expected boolean but got {}",
                    json_type_label(value)
                ));
            }
        }
        SchemaNodeType::Null => {
            errors.push(format!(
                "{path}: expected null but got {}",
                json_type_label(value)
            ));
        }
        SchemaNodeType::Unknown => {
            // Unknown schemas pass through; we can't reliably assert anything.
        }
    }
    validate_conditional_schemas(schema, value, path, errors);
}

fn validate_conditional_schemas(
    schema: &SchemaNode,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    let Some(if_schema) = schema.if_schema.as_deref() else {
        return;
    };
    let mut if_errors = Vec::new();
    validate_at(if_schema, value, path, &mut if_errors);
    if if_errors.is_empty() {
        if let Some(then_schema) = schema.then_schema.as_deref() {
            validate_at(then_schema, value, &format!("{path} then schema"), errors);
        }
    } else if let Some(else_schema) = schema.else_schema.as_deref() {
        validate_at(else_schema, value, &format!("{path} else schema"), errors);
    }
}

fn validate_string_constraints(
    schema: &SchemaNode,
    text: &str,
    path: &str,
    errors: &mut Vec<String>,
) {
    let length = text.chars().count();
    if let Some(min) = schema.min_length
        && length < min
    {
        errors.push(format!(
            "{path}: expected string length at least {min} but got {length}"
        ));
    }
    if let Some(max) = schema.max_length
        && length > max
    {
        errors.push(format!(
            "{path}: expected string length at most {max} but got {length}"
        ));
    }
    if let Some(pattern) = &schema.pattern {
        match regex::Regex::new(pattern) {
            Ok(regex) if !regex.is_match(text) => {
                errors.push(format!("{path}: string does not match pattern /{pattern}/"));
            }
            Ok(_) => {}
            // OpenAPI patterns use ECMA-262 syntax; Rust regex deliberately
            // omits features such as look-around. Unsupported patterns remain
            // prompt hints, but should not make every generated payload fail.
            Err(_) => {}
        }
    }
    if let Some(format) = &schema.format {
        validate_string_format(format, text, path, errors);
    }
}

fn validate_number_constraints(
    schema: &SchemaNode,
    number: f64,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(multiple) = schema.multiple_of
        && multiple > 0.0
    {
        let quotient = number / multiple;
        if (quotient - quotient.round()).abs() > 1e-9 {
            errors.push(format!(
                "{path}: expected number to be a multiple of {multiple} but got {number}"
            ));
        }
    }
    if let Some(min) = schema.minimum {
        let failed = if schema.exclusive_minimum {
            number <= min
        } else {
            number < min
        };
        if failed {
            let comparator = if schema.exclusive_minimum { ">" } else { ">=" };
            errors.push(format!(
                "{path}: expected number {comparator} {min} but got {number}"
            ));
        }
    }
    if let Some(max) = schema.maximum {
        let failed = if schema.exclusive_maximum {
            number >= max
        } else {
            number > max
        };
        if failed {
            let comparator = if schema.exclusive_maximum { "<" } else { "<=" };
            errors.push(format!(
                "{path}: expected number {comparator} {max} but got {number}"
            ));
        }
    }
}

fn plural_y(count: usize) -> &'static str {
    if count == 1 { "y" } else { "ies" }
}

fn validate_string_format(format: &str, text: &str, path: &str, errors: &mut Vec<String>) {
    match format {
        "email" if !is_email_like(text) => {
            errors.push(format!("{path}: string is not a valid email format"));
        }
        "date" if !is_date_like(text) => {
            errors.push(format!("{path}: string is not a valid date format"));
        }
        "date-time" if !is_date_time_like(text) => {
            errors.push(format!("{path}: string is not a valid date-time format"));
        }
        _ => {}
    }
}

fn is_email_like(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn is_date_like(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn is_date_time_like(text: &str) -> bool {
    text.len() >= 20
        && text.contains('T')
        && (text.ends_with('Z')
            || text.contains('+')
            || text.get(10..).is_some_and(|tail| tail.contains('-')))
}

fn compact_json(value: &Value) -> String {
    let raw = value.to_string();
    if raw.len() <= 40 {
        return raw;
    }
    let mut out = raw.chars().take(40).collect::<String>();
    out.push_str("...");
    out
}

fn type_label(kind: &SchemaNodeType) -> &'static str {
    match kind {
        SchemaNodeType::Object => "object",
        SchemaNodeType::Array => "array",
        SchemaNodeType::String => "string",
        SchemaNodeType::Integer => "integer",
        SchemaNodeType::Number => "number",
        SchemaNodeType::Boolean => "boolean",
        SchemaNodeType::Null => "null",
        SchemaNodeType::Unknown => "any",
    }
}

fn json_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn default_mock_examples() -> Vec<MockExample> {
    vec![
        MockExample {
            kind: MockExampleKind::Success,
            title: "Success".to_string(),
            payload: json!({
                "success": true
            }),
            note: Some("Default success placeholder generated during import.".to_string()),
        },
        MockExample {
            kind: MockExampleKind::Empty,
            title: "Empty".to_string(),
            payload: json!({
                "data": []
            }),
            note: Some("Default empty placeholder generated during import.".to_string()),
        },
        MockExample {
            kind: MockExampleKind::Error,
            title: "Error".to_string(),
            payload: json!({
                "error": {
                    "message": "Mock error placeholder"
                }
            }),
            note: Some("Default error placeholder generated during import.".to_string()),
        },
    ]
}

#[cfg(test)]
mod synth_tests {
    use super::*;

    fn schema_with_properties(props: Vec<(&str, SchemaNode, bool)>) -> SchemaNode {
        let mut map = BTreeMap::new();
        for (name, mut node, required) in props {
            node.required = required;
            map.insert(name.to_string(), node);
        }
        SchemaNode {
            node_type: SchemaNodeType::Object,
            properties: map,
            ..SchemaNode::object()
        }
    }

    fn response(status: &str, schema: SchemaNode) -> CanonicalResponse {
        CanonicalResponse {
            status_code: status.into(),
            description: None,
            content_type: "application/json".into(),
            schema: Some(schema),
        }
    }

    fn endpoint(responses: Vec<CanonicalResponse>) -> CanonicalEndpoint {
        CanonicalEndpoint {
            operation_id: Some("test".into()),
            method: HttpMethod::Get,
            path: "/users".into(),
            summary: None,
            description: None,
            tags: Vec::new(),
            parameters: Vec::new(),
            request_body: None,
            responses,
            examples: Vec::new(),
            auth: None,
        }
    }

    #[test]
    fn synthesizes_primitive_defaults() {
        let schema = schema_with_properties(vec![
            ("id", SchemaNode::string(), true),
            ("createdAt", SchemaNode::string(), true),
            ("email", SchemaNode::string(), false),
        ]);
        let value = synthesize_value(&schema);
        assert_eq!(value["id"], "00000000-0000-4000-8000-000000000000");
        assert_eq!(value["createdAt"], "2026-01-01T00:00:00Z");
        assert_eq!(value["email"], "user@example.com");
    }

    #[test]
    fn synthesize_examples_uses_success_and_error() {
        let success = schema_with_properties(vec![(
            "total",
            {
                let mut n = SchemaNode::string();
                n.node_type = SchemaNodeType::Integer;
                n
            },
            true,
        )]);
        let error = schema_with_properties(vec![("error", SchemaNode::string(), true)]);
        let endpoint = endpoint(vec![response("200", success), response("400", error)]);
        let examples = synthesize_examples(&endpoint);
        assert_eq!(examples.len(), 3);
        assert_eq!(examples[0].kind, MockExampleKind::Success);
        assert_eq!(examples[0].payload["total"], 42);
        assert_eq!(examples[1].kind, MockExampleKind::Empty);
        assert_eq!(examples[1].payload["total"], 0);
        assert_eq!(examples[2].kind, MockExampleKind::Error);
    }

    #[test]
    fn array_schema_empties_when_empty_mode() {
        let items = SchemaNode::string();
        let schema = SchemaNode::array(items);
        let empty = synthesize_value_with_name(&schema, None, SynthMode::Empty);
        assert!(empty.as_array().unwrap().is_empty());
        let rich = synthesize_value(&schema);
        assert_eq!(rich.as_array().unwrap().len(), 1);
    }

    #[test]
    fn validates_required_properties() {
        let mut properties = BTreeMap::new();
        let mut id = SchemaNode::string();
        id.required = true;
        properties.insert("id".to_string(), id);
        let mut active = SchemaNode::string();
        active.node_type = SchemaNodeType::Boolean;
        active.required = true;
        properties.insert("active".to_string(), active);
        let schema = SchemaNode {
            node_type: SchemaNodeType::Object,
            required: true,
            properties,
            ..SchemaNode::object()
        };

        // missing required property
        let errs = validate_value(&schema, &json!({"id": "abc"}));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("active"));
        assert!(errs[0].contains("required"));

        // wrong type
        let errs = validate_value(&schema, &json!({"id": 42, "active": true}));
        assert!(
            errs.iter()
                .any(|e| e.contains("id") && e.contains("string"))
        );

        // happy path
        let errs = validate_value(&schema, &json!({"id": "abc", "active": true}));
        assert!(errs.is_empty());
    }

    #[test]
    fn validates_null_only_when_nullable() {
        let mut schema = SchemaNode::string();
        schema.nullable = false;
        assert!(!validate_value(&schema, &json!(null)).is_empty());
        schema.nullable = true;
        assert!(validate_value(&schema, &json!(null)).is_empty());
    }

    #[test]
    fn validates_array_items_recursively() {
        let mut item = SchemaNode::string();
        item.node_type = SchemaNodeType::Integer;
        let schema = SchemaNode::array(item);
        let errs = validate_value(&schema, &json!([1, 2, "bad", 4]));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("[2]"));
    }

    #[test]
    fn validates_prefix_items_and_unevaluated_items() {
        let mut any = SchemaNode::string();
        any.node_type = SchemaNodeType::Unknown;
        let mut tuple = SchemaNode::array(any);
        tuple.items = None;
        tuple.prefix_items = vec![SchemaNode::string(), {
            let mut integer = SchemaNode::string();
            integer.node_type = SchemaNodeType::Integer;
            integer
        }];
        tuple.allow_unevaluated_items = false;

        assert!(validate_value(&tuple, &json!(["status", 200])).is_empty());
        let errs = validate_value(&tuple, &json!(["status", "ok"]));
        assert!(
            errs.iter()
                .any(|err| err.contains("[1]") && err.contains("integer"))
        );
        let errs = validate_value(&tuple, &json!(["status", 200, "extra"]));
        assert!(
            errs.iter()
                .any(|err| err.contains("[2]") && err.contains("unevaluated item"))
        );

        tuple.items = Some(Box::new(SchemaNode::string()));
        assert!(validate_value(&tuple, &json!(["status", 200, "extra"])).is_empty());
    }

    #[test]
    fn validates_boolean_schemas() {
        assert!(
            validate_value(&SchemaNode::bool_schema(true), &json!({"any": ["value"]})).is_empty()
        );

        let errs = validate_value(&SchemaNode::bool_schema(false), &json!("blocked"));
        assert!(errs.iter().any(|err| err.contains("false schema")));

        let schema = SchemaNode::array(SchemaNode::bool_schema(false));
        let errs = validate_value(&schema, &json!(["blocked"]));
        assert!(
            errs.iter()
                .any(|err| err.contains("$[0]") && err.contains("false schema"))
        );

        let mut object = schema_with_properties(vec![("id", SchemaNode::string(), true)]);
        object.additional_properties = Some(Box::new(SchemaNode::bool_schema(false)));
        let errs = validate_value(&object, &json!({"id": "u1", "blocked": true}));
        assert!(
            errs.iter()
                .any(|err| err.contains("$.blocked") && err.contains("false schema"))
        );
    }

    #[test]
    fn validates_enum_membership() {
        let mut schema = SchemaNode::string();
        schema.enum_values = vec![json!("active"), json!("archived")];

        assert!(validate_value(&schema, &json!("active")).is_empty());
        let errs = validate_value(&schema, &json!("pending"));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("enum"));
        assert!(errs[0].contains("pending"));
    }

    #[test]
    fn integer_validation_accepts_whole_floats() {
        let mut schema = SchemaNode::string();
        schema.node_type = SchemaNodeType::Integer;

        assert!(validate_value(&schema, &json!(42)).is_empty());
        assert!(validate_value(&schema, &json!(42.0)).is_empty());
        let errs = validate_value(&schema, &json!(42.5));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("integer"));
    }

    #[test]
    fn validates_string_constraints() {
        let mut schema = SchemaNode::string();
        schema.min_length = Some(3);
        schema.max_length = Some(5);
        schema.pattern = Some("^[A-Z]+$".to_string());
        schema.format = Some("email".to_string());

        let errs = validate_value(&schema, &json!("ab"));
        assert!(errs.iter().any(|err| err.contains("at least 3")));
        assert!(errs.iter().any(|err| err.contains("pattern")));
        assert!(errs.iter().any(|err| err.contains("email")));

        schema.format = None;
        assert!(validate_value(&schema, &json!("ABCDE")).is_empty());
        let errs = validate_value(&schema, &json!("ABCDEF"));
        assert!(errs.iter().any(|err| err.contains("at most 5")));
    }

    #[test]
    fn validates_number_and_array_constraints() {
        let mut schema = SchemaNode::string();
        schema.node_type = SchemaNodeType::Integer;
        schema.minimum = Some(10.0);
        schema.maximum = Some(20.0);
        schema.exclusive_minimum = true;
        schema.multiple_of = Some(2.0);

        let errs = validate_value(&schema, &json!(10));
        assert!(errs.iter().any(|err| err.contains("> 10")));
        let errs = validate_value(&schema, &json!(11));
        assert!(errs.iter().any(|err| err.contains("multiple")));
        assert!(validate_value(&schema, &json!(12)).is_empty());
        let errs = validate_value(&schema, &json!(21));
        assert!(errs.iter().any(|err| err.contains("<= 20")));

        let mut array = SchemaNode::array(SchemaNode::string());
        array.min_items = Some(2);
        array.max_items = Some(3);
        array.unique_items = true;
        assert!(validate_value(&array, &json!(["a", "b"])).is_empty());
        assert!(
            validate_value(&array, &json!(["a"]))
                .iter()
                .any(|err| err.contains("at least 2"))
        );
        assert!(
            validate_value(&array, &json!(["a", "b", "c", "d"]))
                .iter()
                .any(|err| err.contains("at most 3"))
        );
        assert!(
            validate_value(&array, &json!(["a", "a"]))
                .iter()
                .any(|err| err.contains("duplicates"))
        );

        let mut contains = SchemaNode::string();
        contains.node_type = SchemaNodeType::Integer;
        let mut any_item = SchemaNode::string();
        any_item.node_type = SchemaNodeType::Unknown;
        let mut with_contains = SchemaNode::array(any_item);
        with_contains.contains = Some(Box::new(contains));
        with_contains.min_contains = Some(2);
        with_contains.max_contains = Some(2);
        assert!(validate_value(&with_contains, &json!(["a", 1, 2])).is_empty());
        let errs = validate_value(&with_contains, &json!(["a", 1]));
        assert!(
            errs.iter()
                .any(|err| err.contains("contains schema") && err.contains("at least 2"))
        );
        let errs = validate_value(&with_contains, &json!([1, 2, 3]));
        assert!(
            errs.iter()
                .any(|err| err.contains("contains schema") && err.contains("at most 2"))
        );
    }

    #[test]
    fn validates_additional_properties() {
        let mut schema = schema_with_properties(vec![("id", SchemaNode::string(), true)]);
        schema.allow_additional_properties = false;
        let errs = validate_value(&schema, &json!({"id": "u1", "role": "admin"}));
        assert!(errs.iter().any(|err| err.contains("additional property")));

        schema.allow_additional_properties = true;
        let mut additional = SchemaNode::string();
        additional.node_type = SchemaNodeType::Integer;
        schema.additional_properties = Some(Box::new(additional));
        let errs = validate_value(&schema, &json!({"id": "u1", "rank": "high"}));
        assert!(
            errs.iter()
                .any(|err| err.contains("rank") && err.contains("integer"))
        );
        assert!(validate_value(&schema, &json!({"id": "u1", "rank": 2})).is_empty());
    }

    #[test]
    fn validates_unevaluated_properties_closure() {
        let mut schema = schema_with_properties(vec![("id", SchemaNode::string(), true)]);
        schema.allow_unevaluated_properties = false;

        let errs = validate_value(&schema, &json!({"id": "u1", "role": "admin"}));
        assert!(
            errs.iter()
                .any(|err| err.contains("role") && err.contains("unevaluated property"))
        );

        let mut additional = SchemaNode::string();
        additional.node_type = SchemaNodeType::Integer;
        schema.additional_properties = Some(Box::new(additional));
        assert!(validate_value(&schema, &json!({"id": "u1", "rank": 2})).is_empty());
    }

    #[test]
    fn validates_object_property_count_constraints() {
        let mut schema = schema_with_properties(vec![("id", SchemaNode::string(), false)]);
        schema.min_properties = Some(2);
        schema.max_properties = Some(3);

        let errs = validate_value(&schema, &json!({"id": "u1"}));
        assert!(errs.iter().any(|err| err.contains("at least 2")));
        assert!(errs.iter().any(|err| err.contains("properties")));

        assert!(validate_value(&schema, &json!({"id": "u1", "name": "Ada"})).is_empty());

        let errs = validate_value(
            &schema,
            &json!({"id": "u1", "name": "Ada", "role": "admin", "team": "api"}),
        );
        assert!(errs.iter().any(|err| err.contains("at most 3")));
    }

    #[test]
    fn validates_dependent_required_and_dependent_schemas() {
        let mut schema = schema_with_properties(vec![
            ("credit_card", SchemaNode::string(), false),
            ("billing_address", SchemaNode::string(), false),
            ("country", SchemaNode::string(), false),
        ]);
        schema.dependent_required.insert(
            "credit_card".to_string(),
            vec!["billing_address".to_string()],
        );

        let mut dependent = schema_with_properties(vec![("country", SchemaNode::string(), true)]);
        dependent.min_properties = Some(3);
        schema
            .dependent_schemas
            .insert("credit_card".to_string(), dependent);

        assert!(
            validate_value(
                &schema,
                &json!({
                    "credit_card": "4111",
                    "billing_address": "1 Main",
                    "country": "US"
                })
            )
            .is_empty()
        );

        let errs = validate_value(&schema, &json!({"credit_card": "4111"}));
        assert!(
            errs.iter()
                .any(|err| err.contains("billing_address") && err.contains("credit_card"))
        );
        assert!(
            errs.iter()
                .any(|err| err.contains("dependent schema") && err.contains("country"))
        );
        assert!(
            errs.iter()
                .any(|err| err.contains("dependent schema") && err.contains("at least 3"))
        );
    }

    #[test]
    fn validates_if_then_else_conditionals() {
        let mut schema = schema_with_properties(vec![
            ("kind", SchemaNode::string(), true),
            ("admin_code", SchemaNode::string(), false),
            ("guest_token", SchemaNode::string(), false),
        ]);

        let mut if_schema = schema_with_properties(vec![("kind", SchemaNode::string(), true)]);
        if_schema
            .properties
            .get_mut("kind")
            .unwrap()
            .enum_values
            .push(Value::String("admin".to_string()));

        let then_schema = schema_with_properties(vec![("admin_code", SchemaNode::string(), true)]);
        let else_schema = schema_with_properties(vec![("guest_token", SchemaNode::string(), true)]);
        schema.if_schema = Some(Box::new(if_schema));
        schema.then_schema = Some(Box::new(then_schema));
        schema.else_schema = Some(Box::new(else_schema));

        assert!(validate_value(&schema, &json!({"kind": "admin", "admin_code": "A1"})).is_empty());
        assert!(validate_value(&schema, &json!({"kind": "guest", "guest_token": "G1"})).is_empty());

        let errs = validate_value(&schema, &json!({"kind": "admin"}));
        assert!(
            errs.iter()
                .any(|err| err.contains("then schema") && err.contains("admin_code"))
        );

        let errs = validate_value(&schema, &json!({"kind": "guest"}));
        assert!(
            errs.iter()
                .any(|err| err.contains("else schema") && err.contains("guest_token"))
        );
    }

    #[test]
    fn enum_values_win_over_defaults() {
        let mut s = SchemaNode::string();
        s.enum_values = vec![
            Value::String("active".into()),
            Value::String("archived".into()),
        ];
        let value = synthesize_value(&s);
        assert_eq!(value, Value::String("active".into()));
    }
}
