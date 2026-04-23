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

    pub fn string() -> Self {
        Self {
            node_type: SchemaNodeType::String,
            description: None,
            required: false,
            nullable: false,
            properties: BTreeMap::new(),
            items: None,
            enum_values: Vec::new(),
            example: None,
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
            enum_values: Vec::new(),
            example: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_name: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
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
            description: None,
            required: false,
            nullable: false,
            properties: map,
            items: None,
            enum_values: Vec::new(),
            example: None,
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
