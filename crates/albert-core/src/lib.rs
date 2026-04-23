use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStage {
    Planned,
    Scaffolded,
    Partial,
    NotImplemented,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityStatus {
    pub name: String,
    pub stage: DeliveryStage,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBootstrapSummary {
    pub project_name: String,
    pub current_phase: String,
    pub ui_surfaces: Vec<String>,
    pub parser_capabilities: Vec<CapabilityStatus>,
    pub storage_capabilities: Vec<CapabilityStatus>,
    pub provider_capabilities: Vec<CapabilityStatus>,
    pub gateway_capabilities: Vec<CapabilityStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSourceKind {
    OpenApi,
    Curl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalApiCollection {
    pub id: String,
    pub name: String,
    pub source: InputSourceKind,
    pub description: Option<String>,
    pub endpoints: Vec<CanonicalEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalParameter {
    pub name: String,
    pub location: ParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub schema: SchemaNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRequestBody {
    pub content_type: String,
    pub required: bool,
    pub schema: SchemaNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalResponse {
    pub status_code: String,
    pub description: Option<String>,
    pub content_type: String,
    pub schema: Option<SchemaNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaNode {
    pub node_type: SchemaNodeType,
    pub description: Option<String>,
    pub required: bool,
    pub nullable: bool,
    pub properties: BTreeMap<String, SchemaNode>,
    pub items: Option<Box<SchemaNode>>,
    pub enum_values: Vec<Value>,
    pub example: Option<Value>,
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
            enum_values: Vec::new(),
            example: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MockExampleKind {
    Success,
    Empty,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockExample {
    pub kind: MockExampleKind,
    pub title: String,
    pub payload: Value,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_name: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockHttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockHttpResponse {
    pub status_code: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}
