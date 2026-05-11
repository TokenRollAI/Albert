//! OpenAI-compatible generation adapter.
//!
//! Phase 4 runtime: builds structured mock examples by calling any
//! OpenAI-compatible Chat Completions endpoint (e.g. OpenAI, Azure OpenAI,
//! Qwen-compatible gateways, Together, DeepInfra...) or the OpenAI Responses
//! endpoint. JSON mode is requested and the raw response is parsed back into a
//! `MockExample`.

use std::time::Duration;

use albert_core::{
    CanonicalEndpoint, CanonicalParameter, CanonicalResponse, CapabilityStatus, DeliveryStage,
    MockExample, MockExampleKind, ProviderApiType, ProviderConfig, ProviderReasoningEffort,
    SchemaNode, SchemaNodeType, validate_value,
};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub mod prompt;

pub use prompt::{
    GenerationContext, GenerationIntent, PromptBundle, build_prompt_bundle,
    build_prompt_bundle_with_context,
};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_SCHEMA_REPAIR_ATTEMPTS: usize = 2;
const MAX_SCHEMA_REPAIR_ATTEMPTS: usize = 5;

#[derive(Debug, Clone)]
pub struct OpenAiChatAdapter {
    pub config: ProviderConfig,
    pub api_key: Option<String>,
    pub timeout: Duration,
}

impl OpenAiChatAdapter {
    pub fn new(config: ProviderConfig) -> Self {
        let api_key = std::env::var(&config.api_key_env).ok();
        Self {
            config,
            api_key,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn generate_mock_example(
        &self,
        endpoint: &CanonicalEndpoint,
        intent: GenerationIntent,
    ) -> Result<MockExample, OpenAiError> {
        self.generate_mock_example_with_context(endpoint, intent, None)
            .await
    }

    pub async fn generate_mock_example_with_context(
        &self,
        endpoint: &CanonicalEndpoint,
        intent: GenerationIntent,
        generation_context: Option<&GenerationContext>,
    ) -> Result<MockExample, OpenAiError> {
        let bundle = build_prompt_bundle_with_context(endpoint, intent, generation_context);
        let raw = self.call_chat(&bundle).await?;
        let Some(validation_schema) = pick_validation_schema(endpoint, intent) else {
            return parse_response_payload(&raw, intent, None);
        };

        let mut current = raw;
        let mut errors = validate_value(&validation_schema, &current);
        if errors.is_empty() {
            return parse_response_payload(&current, intent, None);
        }

        let repair_attempts = provider_schema_repair_attempts(&self.config);
        if repair_attempts == 0 {
            return parse_response_payload(
                &current,
                intent,
                Some(format!(
                    "Schema validation failed and repair retries are disabled: {}",
                    errors.join("; ")
                )),
            );
        }

        let mut repair_bundle = build_repair_bundle(&bundle, &errors);
        for attempt in 1..=repair_attempts {
            let repaired = match self.call_chat(&repair_bundle).await {
                Ok(value) => value,
                Err(err) => {
                    // If repair fails over the wire, prefer returning the
                    // latest generation with a warning note rather than
                    // bubbling the retry error — the payload may still be
                    // useful for manual editing.
                    return parse_response_payload(
                        &current,
                        intent,
                        Some(format!(
                            "Schema validation failed; repair attempt {attempt} errored: {err}"
                        )),
                    );
                }
            };
            current = repaired;
            errors = validate_value(&validation_schema, &current);
            if errors.is_empty() {
                return parse_response_payload(
                    &current,
                    intent,
                    Some(format!(
                        "Repaired after {attempt} validation retry attempt(s)."
                    )),
                );
            }
            repair_bundle = build_repair_bundle(&bundle, &errors);
        }

        parse_response_payload(
            &current,
            intent,
            Some(format!(
                "Schema validation still failing after {repair_attempts} repair attempt(s): {}",
                errors.join("; ")
            )),
        )
    }

    pub async fn call_chat(&self, bundle: &PromptBundle) -> Result<Value, OpenAiError> {
        let Some(api_key) = self.api_key.clone() else {
            return Err(OpenAiError::MissingApiKey(self.config.api_key_env.clone()));
        };
        let request = provider_request(&self.config, bundle)?;

        let client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;

        let resp = client
            .post(&request.url)
            .header(&request.auth_header, request.auth_value(&api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request.body)
            .send()
            .await
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;

        let status = resp.status();
        let raw_body = resp
            .text()
            .await
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;
        if !status.is_success() {
            return Err(OpenAiError::Provider {
                status: status.as_u16(),
                body: truncate(&raw_body, 2048),
            });
        }

        let raw: Value =
            serde_json::from_str(&raw_body).map_err(|err| OpenAiError::Decode(err.to_string()))?;
        let content = extract_provider_content(&raw, request.response_api)?;
        parse_json_content(content)
    }
}

#[derive(Debug, Clone, Copy)]
enum ProviderResponseApi {
    ChatCompletions,
    Responses,
}

struct ProviderRequest {
    url: String,
    body: Value,
    auth_header: header::HeaderName,
    response_api: ProviderResponseApi,
}

impl ProviderRequest {
    fn auth_value(&self, api_key: &str) -> String {
        if self.auth_header == header::AUTHORIZATION {
            format!("Bearer {api_key}")
        } else {
            api_key.to_string()
        }
    }
}

fn provider_request(
    config: &ProviderConfig,
    bundle: &PromptBundle,
) -> Result<ProviderRequest, OpenAiError> {
    match config.api_type {
        ProviderApiType::OpenAiCompatible => {
            let base = config.base_url.trim_end_matches('/');
            let base = base.trim_end_matches("/v1");
            Ok(ProviderRequest {
                url: format!("{base}/v1/chat/completions"),
                body: build_chat_completions_body(Some(&config.model), config, bundle),
                auth_header: header::AUTHORIZATION,
                response_api: ProviderResponseApi::ChatCompletions,
            })
        }
        ProviderApiType::OpenAiResponses => {
            let base = config.base_url.trim_end_matches('/');
            let base = base.trim_end_matches("/v1");
            Ok(ProviderRequest {
                url: format!("{base}/v1/responses"),
                body: build_responses_body(&config.model, config, bundle),
                auth_header: header::AUTHORIZATION,
                response_api: ProviderResponseApi::Responses,
            })
        }
        ProviderApiType::AzureOpenAiResponses => {
            let deployment = non_empty_or_model(config.azure_deployment.as_deref(), &config.model)?;
            let base = config.base_url.trim_end_matches('/');
            Ok(ProviderRequest {
                url: format!("{base}/openai/v1/responses"),
                body: build_responses_body(deployment, config, bundle),
                auth_header: header::HeaderName::from_static("api-key"),
                response_api: ProviderResponseApi::Responses,
            })
        }
        ProviderApiType::AzureOpenAi => {
            let deployment = non_empty_or_model(config.azure_deployment.as_deref(), &config.model)?;
            let api_version = non_empty(
                config.azure_api_version.as_deref(),
                "Azure OpenAI API version is required for azure_openai providers",
            )?;
            let base = config.base_url.trim_end_matches('/');
            Ok(ProviderRequest {
                url: format!(
                    "{base}/openai/deployments/{deployment}/chat/completions?api-version={api_version}"
                ),
                body: build_chat_completions_body(None, config, bundle),
                auth_header: header::HeaderName::from_static("api-key"),
                response_api: ProviderResponseApi::ChatCompletions,
            })
        }
    }
}

fn non_empty<'a>(value: Option<&'a str>, message: &'static str) -> Result<&'a str, OpenAiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OpenAiError::Config(message))
}

fn non_empty_or_model<'a>(value: Option<&'a str>, model: &'a str) -> Result<&'a str, OpenAiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let model = model.trim();
            if model.is_empty() { None } else { Some(model) }
        })
        .ok_or(OpenAiError::Config(
            "Azure OpenAI deployment is required; set deployment or model",
        ))
}

fn build_chat_completions_body(
    model: Option<&str>,
    config: &ProviderConfig,
    bundle: &PromptBundle,
) -> Value {
    let mut body = json!({
        "messages": [
            {"role": "system", "content": bundle.system},
            {"role": "user", "content": bundle.user},
        ],
        "response_format": {"type": "json_object"},
        "temperature": provider_temperature(config),
    });
    if let Some(model) = model {
        body["model"] = Value::String(model.to_string());
    }
    if let Some(max_output_tokens) = provider_max_output_tokens(config) {
        body["max_tokens"] = json!(max_output_tokens);
    }
    body
}

fn build_responses_body(model: &str, config: &ProviderConfig, bundle: &PromptBundle) -> Value {
    let mut body = json!({
        "model": model,
        "instructions": bundle.system,
        "input": bundle.user,
        "text": {
            "format": {
                "type": "json_object"
            }
        },
        "temperature": provider_temperature(config),
    });
    if let Some(max_output_tokens) = provider_max_output_tokens(config) {
        body["max_output_tokens"] = json!(max_output_tokens);
    }
    if let Some(reasoning_effort) = provider_reasoning_effort(config) {
        body["reasoning"] = json!({ "effort": reasoning_effort.as_str() });
    }
    body
}

fn provider_temperature(config: &ProviderConfig) -> f32 {
    config
        .temperature
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 2.0))
        .unwrap_or(0.7)
}

fn provider_max_output_tokens(config: &ProviderConfig) -> Option<u32> {
    config.max_output_tokens.filter(|value| *value > 0)
}

fn provider_reasoning_effort(config: &ProviderConfig) -> Option<&ProviderReasoningEffort> {
    config.reasoning_effort.as_ref()
}

fn provider_schema_repair_attempts(config: &ProviderConfig) -> usize {
    config
        .schema_repair_attempts
        .map(usize::from)
        .unwrap_or(DEFAULT_SCHEMA_REPAIR_ATTEMPTS)
        .min(MAX_SCHEMA_REPAIR_ATTEMPTS)
}

fn extract_provider_content(
    raw: &Value,
    response_api: ProviderResponseApi,
) -> Result<&str, OpenAiError> {
    match response_api {
        ProviderResponseApi::ChatCompletions => extract_chat_completions_content(raw),
        ProviderResponseApi::Responses => extract_responses_content(raw),
    }
}

fn extract_chat_completions_content(raw: &Value) -> Result<&str, OpenAiError> {
    raw.get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
        .ok_or(OpenAiError::MissingContent)
}

fn extract_responses_content(raw: &Value) -> Result<&str, OpenAiError> {
    if let Some(output_text) = raw.get("output_text").and_then(Value::as_str) {
        return Ok(output_text);
    }

    raw.get("output")
        .and_then(Value::as_array)
        .and_then(|output| {
            output.iter().find_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| {
                        content.iter().find_map(|part| {
                            let part_type = part.get("type").and_then(Value::as_str);
                            match part.get("text").and_then(Value::as_str) {
                                Some(text) if part_type == Some("output_text") => Some(text),
                                Some(text) if part_type.is_none() => Some(text),
                                Some(text) if part_type == Some("text") => Some(text),
                                _ => None,
                            }
                        })
                    })
            })
        })
        .ok_or(OpenAiError::MissingContent)
}

fn parse_json_content(content: &str) -> Result<Value, OpenAiError> {
    let trimmed = content.trim();
    let stripped = strip_code_fence(trimmed);
    serde_json::from_str::<Value>(stripped)
        .map_err(|err| OpenAiError::Decode(format!("model content not valid JSON: {err}")))
}

fn strip_code_fence(input: &str) -> &str {
    let trimmed = input.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let (_lang, body) = rest.split_once('\n').unwrap_or(("", rest));
    body.trim_end_matches("```").trim()
}

fn parse_response_payload(
    value: &Value,
    intent: GenerationIntent,
    extra_note: Option<String>,
) -> Result<MockExample, OpenAiError> {
    let kind = intent.kind();
    let base_note = format!("Generated by OpenAI adapter ({})", kind.as_str());
    let note = match extra_note {
        Some(extra) => Some(format!("{base_note}. {extra}")),
        None => Some(base_note),
    };
    Ok(MockExample {
        kind: kind.clone(),
        title: intent.title(),
        payload: value.clone(),
        note,
    })
}

fn pick_validation_schema(
    endpoint: &CanonicalEndpoint,
    intent: GenerationIntent,
) -> Option<SchemaNode> {
    let predicate: fn(&str) -> bool = match intent.kind() {
        MockExampleKind::Error => |code: &str| code.starts_with('4') || code.starts_with('5'),
        _ => |code: &str| code.starts_with('2'),
    };
    endpoint
        .responses
        .iter()
        .find(|r| predicate(&r.status_code))
        .and_then(|r| r.schema.clone())
}

fn build_repair_bundle(original: &PromptBundle, errors: &[String]) -> PromptBundle {
    let amendment = format!(
        "{original_user}\n\nThe previous response failed schema validation:\n{errors}\n\nReturn a new JSON object that fixes the listed issues. Respond with JSON only.",
        original_user = original.user,
        errors = errors
            .iter()
            .map(|e| format!("- {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    PromptBundle {
        system: original.system.clone(),
        user: amendment,
        endpoint_context: original.endpoint_context.clone(),
    }
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        let mut end = max;
        while !input.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}…", &input[..end])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub raw_text: String,
}

#[derive(Debug, Error)]
pub enum OpenAiError {
    #[error("OpenAI API key environment variable `{0}` is not set")]
    MissingApiKey(String),
    #[error("transport error talking to provider: {0}")]
    Transport(String),
    #[error("provider returned HTTP {status}: {body}")]
    Provider { status: u16, body: String },
    #[error("failed to decode provider response: {0}")]
    Decode(String),
    #[error("provider response did not include an assistant message")]
    MissingContent,
    #[error("provider configuration error: {0}")]
    Config(&'static str),
    #[error("provider not implemented: {0}")]
    NotImplemented(&'static str),
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "OpenAI Chat Completions adapter".to_string(),
            stage: DeliveryStage::Partial,
            note: "Chat Completions call with JSON mode wired; ready for real generation."
                .to_string(),
        },
        CapabilityStatus {
            name: "Responses API adapter".to_string(),
            stage: DeliveryStage::Partial,
            note: "OpenAI Responses endpoint is wired with JSON object mode and response text extraction; Azure Responses remains future work.".to_string(),
        },
        CapabilityStatus {
            name: "Structured output enforcement".to_string(),
            stage: DeliveryStage::Partial,
            note: "JSON-object mode plus canonical schema validation and bounded repair retries are wired; stricter JSON Schema constraints remain future work.".to_string(),
        },
    ]
}

/// Returns a lightweight preview of the prompt bundle for UI debugging without
/// making a network call. Useful for the frontend to surface what would be sent.
pub fn preview_prompt(endpoint: &CanonicalEndpoint, intent: GenerationIntent) -> PromptBundle {
    build_prompt_bundle(endpoint, intent)
}

pub fn preview_prompt_with_context(
    endpoint: &CanonicalEndpoint,
    intent: GenerationIntent,
    generation_context: Option<&GenerationContext>,
) -> PromptBundle {
    build_prompt_bundle_with_context(endpoint, intent, generation_context)
}

/// Describe the canonical schema as a compact JSON-Schema-like object so the
/// LLM has enough hints without the noise of the internal struct field names.
pub fn schema_hint(schema: &SchemaNode) -> Value {
    if let Some(value) = schema.bool_schema {
        return Value::Bool(value);
    }
    match &schema.node_type {
        SchemaNodeType::Object => {
            let mut properties = serde_json::Map::new();
            let mut required: Vec<String> = Vec::new();
            for (name, child) in schema.properties.iter() {
                properties.insert(name.clone(), schema_hint(child));
                if child.required {
                    required.push(name.clone());
                }
            }
            let mut payload = serde_json::Map::new();
            payload.insert("type".to_string(), Value::String("object".to_string()));
            payload.insert("properties".to_string(), Value::Object(properties));
            if !required.is_empty() {
                payload.insert(
                    "required".to_string(),
                    Value::Array(required.into_iter().map(Value::String).collect()),
                );
            }
            if !schema.allow_additional_properties {
                payload.insert("additionalProperties".to_string(), Value::Bool(false));
            } else if let Some(additional) = schema.additional_properties.as_deref() {
                payload.insert("additionalProperties".to_string(), schema_hint(additional));
            }
            if !schema.allow_unevaluated_properties {
                payload.insert("unevaluatedProperties".to_string(), Value::Bool(false));
            }
            if !schema.dependent_required.is_empty() {
                let dependent_required = schema
                    .dependent_required
                    .iter()
                    .map(|(name, dependents)| {
                        (
                            name.clone(),
                            Value::Array(dependents.iter().cloned().map(Value::String).collect()),
                        )
                    })
                    .collect();
                payload.insert(
                    "dependentRequired".to_string(),
                    Value::Object(dependent_required),
                );
            }
            if !schema.dependent_schemas.is_empty() {
                let dependent_schemas = schema
                    .dependent_schemas
                    .iter()
                    .map(|(name, dependent)| (name.clone(), schema_hint(dependent)))
                    .collect();
                payload.insert(
                    "dependentSchemas".to_string(),
                    Value::Object(dependent_schemas),
                );
            }
            if let Some(ex) = &schema.example {
                payload.insert("example".to_string(), ex.clone());
            }
            add_common_constraints(&mut payload, schema);
            Value::Object(payload)
        }
        SchemaNodeType::Array => {
            let items = schema
                .items
                .as_ref()
                .map(|inner| schema_hint(inner))
                .unwrap_or(Value::Object(serde_json::Map::new()));
            let prefix_items = schema
                .prefix_items
                .iter()
                .map(schema_hint)
                .collect::<Vec<_>>();
            let mut payload = serde_json::Map::new();
            payload.insert("type".to_string(), Value::String("array".to_string()));
            payload.insert("items".to_string(), items);
            if !prefix_items.is_empty() {
                payload.insert("prefixItems".to_string(), Value::Array(prefix_items));
            }
            if !schema.allow_unevaluated_items {
                payload.insert("unevaluatedItems".to_string(), Value::Bool(false));
            }
            add_common_constraints(&mut payload, schema);
            Value::Object(payload)
        }
        other => {
            let name = match other {
                SchemaNodeType::String => "string",
                SchemaNodeType::Integer => "integer",
                SchemaNodeType::Number => "number",
                SchemaNodeType::Boolean => "boolean",
                SchemaNodeType::Null => "null",
                SchemaNodeType::Unknown => "any",
                _ => "any",
            };
            let mut payload = serde_json::Map::new();
            payload.insert("type".to_string(), Value::String(name.to_string()));
            if schema.nullable {
                payload.insert("nullable".to_string(), Value::Bool(true));
            }
            if !schema.enum_values.is_empty() {
                payload.insert("enum".to_string(), Value::Array(schema.enum_values.clone()));
            }
            if let Some(ex) = &schema.example {
                payload.insert("example".to_string(), ex.clone());
            }
            add_common_constraints(&mut payload, schema);
            Value::Object(payload)
        }
    }
}

fn add_common_constraints(payload: &mut serde_json::Map<String, Value>, schema: &SchemaNode) {
    if let Some(format) = &schema.format {
        payload.insert("format".to_string(), Value::String(format.clone()));
    }
    if let Some(pattern) = &schema.pattern {
        payload.insert("pattern".to_string(), Value::String(pattern.clone()));
    }
    if let Some(min) = schema.min_length {
        payload.insert(
            "minLength".to_string(),
            Value::Number(serde_json::Number::from(min as u64)),
        );
    }
    if let Some(max) = schema.max_length {
        payload.insert(
            "maxLength".to_string(),
            Value::Number(serde_json::Number::from(max as u64)),
        );
    }
    if let Some(min) = schema.minimum.and_then(serde_json::Number::from_f64) {
        payload.insert("minimum".to_string(), Value::Number(min));
    }
    if let Some(max) = schema.maximum.and_then(serde_json::Number::from_f64) {
        payload.insert("maximum".to_string(), Value::Number(max));
    }
    if let Some(multiple) = schema.multiple_of.and_then(serde_json::Number::from_f64) {
        payload.insert("multipleOf".to_string(), Value::Number(multiple));
    }
    if schema.exclusive_minimum {
        payload.insert("exclusiveMinimum".to_string(), Value::Bool(true));
    }
    if schema.exclusive_maximum {
        payload.insert("exclusiveMaximum".to_string(), Value::Bool(true));
    }
    if let Some(min) = schema.min_items {
        payload.insert(
            "minItems".to_string(),
            Value::Number(serde_json::Number::from(min as u64)),
        );
    }
    if let Some(max) = schema.max_items {
        payload.insert(
            "maxItems".to_string(),
            Value::Number(serde_json::Number::from(max as u64)),
        );
    }
    if let Some(contains) = schema.contains.as_deref() {
        payload.insert("contains".to_string(), schema_hint(contains));
    }
    if let Some(min) = schema.min_contains {
        payload.insert(
            "minContains".to_string(),
            Value::Number(serde_json::Number::from(min as u64)),
        );
    }
    if let Some(max) = schema.max_contains {
        payload.insert(
            "maxContains".to_string(),
            Value::Number(serde_json::Number::from(max as u64)),
        );
    }
    if let Some(min) = schema.min_properties {
        payload.insert(
            "minProperties".to_string(),
            Value::Number(serde_json::Number::from(min as u64)),
        );
    }
    if let Some(max) = schema.max_properties {
        payload.insert(
            "maxProperties".to_string(),
            Value::Number(serde_json::Number::from(max as u64)),
        );
    }
    if schema.unique_items {
        payload.insert("uniqueItems".to_string(), Value::Bool(true));
    }
    add_conditional_schema_hints(payload, schema);
}

fn add_conditional_schema_hints(payload: &mut serde_json::Map<String, Value>, schema: &SchemaNode) {
    if let Some(if_schema) = schema.if_schema.as_deref() {
        payload.insert("if".to_string(), schema_hint(if_schema));
    }
    if let Some(then_schema) = schema.then_schema.as_deref() {
        payload.insert("then".to_string(), schema_hint(then_schema));
    }
    if let Some(else_schema) = schema.else_schema.as_deref() {
        payload.insert("else".to_string(), schema_hint(else_schema));
    }
}

pub fn parameter_hints(params: &[CanonicalParameter]) -> Vec<Value> {
    params
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "in": format!("{:?}", p.location).to_lowercase(),
                "required": p.required,
                "schema": schema_hint(&p.schema),
                "description": p.description,
            })
        })
        .collect()
}

pub fn response_hints(responses: &[CanonicalResponse]) -> Vec<Value> {
    responses
        .iter()
        .map(|r| {
            json!({
                "status_code": r.status_code,
                "content_type": r.content_type,
                "description": r.description,
                "schema": r.schema.as_ref().map(schema_hint),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{HttpMethod, MockExampleKind, SchemaNode};
    use std::collections::BTreeMap;

    fn endpoint() -> CanonicalEndpoint {
        let mut properties = BTreeMap::new();
        let mut name = SchemaNode::string();
        name.required = true;
        name.example = Some(Value::String("Ada".to_string()));
        properties.insert("name".to_string(), name);
        let schema = SchemaNode {
            node_type: SchemaNodeType::Object,
            required: true,
            properties,
            ..SchemaNode::object()
        };
        CanonicalEndpoint {
            operation_id: Some("createUser".into()),
            method: HttpMethod::Post,
            path: "/users".into(),
            summary: Some("Create a user".into()),
            description: None,
            tags: vec!["users".into()],
            parameters: Vec::new(),
            request_body: None,
            responses: vec![CanonicalResponse {
                status_code: "201".into(),
                description: Some("Created".into()),
                content_type: "application/json".into(),
                schema: Some(schema),
            }],
            examples: Vec::new(),
            auth: None,
        }
    }

    #[test]
    fn builds_prompt_bundle_for_success() {
        let bundle = preview_prompt(&endpoint(), GenerationIntent::Success);
        assert!(bundle.system.to_lowercase().contains("mock"));
        assert!(bundle.user.contains("/users"));
        assert!(bundle.user.contains("success"));
    }

    #[test]
    fn builds_prompt_bundle_for_error() {
        let bundle = preview_prompt(&endpoint(), GenerationIntent::Error);
        assert!(bundle.user.contains("error"));
    }

    #[test]
    fn builds_prompt_bundle_with_request_context() {
        let context = GenerationContext {
            request_snapshot: Some(json!({
                "query": "status=paid",
                "headers": {"x-trace": "abc"},
                "body": null
            })),
            response_snapshot: Some(json!({
                "status": 200,
                "body": {"name": "Grace"}
            })),
            note: Some("cached fingerprint abc123".to_string()),
        };
        let bundle =
            preview_prompt_with_context(&endpoint(), GenerationIntent::Success, Some(&context));

        assert!(bundle.user.contains("Request context"));
        assert!(bundle.user.contains("status=paid"));
        assert!(bundle.user.contains("cached fingerprint abc123"));
        assert_eq!(
            bundle.endpoint_context["request_context"]["request_snapshot"]["query"],
            "status=paid"
        );
    }

    #[test]
    fn strip_code_fence_handles_markdown_blocks() {
        let input = "```json\n{\"a\": 1}\n```";
        assert_eq!(strip_code_fence(input), "{\"a\": 1}");
    }

    #[test]
    fn parse_response_payload_wraps_kind() {
        let v = json!({"data": {"id": 1}});
        let example = parse_response_payload(&v, GenerationIntent::Success, None).unwrap();
        assert_eq!(example.kind, MockExampleKind::Success);
        assert_eq!(example.payload, v);
    }

    #[test]
    fn schema_hint_emits_required_list() {
        let mut properties = BTreeMap::new();
        let mut a = SchemaNode::string();
        a.required = true;
        properties.insert("a".to_string(), a);
        let schema = SchemaNode {
            node_type: SchemaNodeType::Object,
            properties,
            ..SchemaNode::object()
        };
        let hint = schema_hint(&schema);
        assert_eq!(hint["type"], "object");
        assert_eq!(hint["required"][0], "a");
    }

    #[test]
    fn schema_hint_emits_object_property_count_constraints() {
        let mut schema = SchemaNode::object();
        schema.min_properties = Some(1);
        schema.max_properties = Some(3);

        let hint = schema_hint(&schema);
        assert_eq!(hint["minProperties"], 1);
        assert_eq!(hint["maxProperties"], 3);
    }

    #[test]
    fn schema_hint_emits_validation_constraints() {
        let mut schema = SchemaNode::string();
        schema.format = Some("email".to_string());
        schema.pattern = Some("^.+@example\\.com$".to_string());
        schema.min_length = Some(6);
        schema.max_length = Some(120);

        let hint = schema_hint(&schema);
        assert_eq!(hint["format"], "email");
        assert_eq!(hint["pattern"], "^.+@example\\.com$");
        assert_eq!(hint["minLength"], 6);
        assert_eq!(hint["maxLength"], 120);

        let mut array = SchemaNode::array(schema);
        array.min_items = Some(1);
        array.max_items = Some(3);
        array.unique_items = true;
        let mut contains = SchemaNode::string();
        contains.node_type = SchemaNodeType::Integer;
        array.contains = Some(Box::new(contains));
        array.min_contains = Some(1);
        array.max_contains = Some(1);
        let mut prefix_status = SchemaNode::string();
        prefix_status.enum_values = vec![json!("status")];
        let mut prefix_code = SchemaNode::string();
        prefix_code.node_type = SchemaNodeType::Integer;
        array.prefix_items = vec![prefix_status, prefix_code];
        array.allow_unevaluated_items = false;
        let hint = schema_hint(&array);
        assert_eq!(hint["minItems"], 1);
        assert_eq!(hint["maxItems"], 3);
        assert_eq!(hint["uniqueItems"], true);
        assert_eq!(hint["items"]["format"], "email");
        assert_eq!(hint["prefixItems"][0]["enum"][0], "status");
        assert_eq!(hint["prefixItems"][1]["type"], "integer");
        assert_eq!(hint["unevaluatedItems"], false);
        assert_eq!(hint["contains"]["type"], "integer");
        assert_eq!(hint["minContains"], 1);
        assert_eq!(hint["maxContains"], 1);

        let blocked = SchemaNode::bool_schema(false);
        assert_eq!(schema_hint(&blocked), json!(false));

        let mut number = SchemaNode::string();
        number.node_type = SchemaNodeType::Number;
        number.multiple_of = Some(0.5);
        let hint = schema_hint(&number);
        assert_eq!(hint["multipleOf"], 0.5);
    }

    #[test]
    fn schema_hint_emits_additional_properties() {
        let mut closed = SchemaNode::object();
        closed.allow_additional_properties = false;
        closed.allow_unevaluated_properties = false;
        let hint = schema_hint(&closed);
        assert_eq!(hint["additionalProperties"], false);
        assert_eq!(hint["unevaluatedProperties"], false);

        let mut typed = SchemaNode::object();
        let mut additional = SchemaNode::string();
        additional.node_type = SchemaNodeType::Integer;
        typed.additional_properties = Some(Box::new(additional));
        typed.dependent_required.insert(
            "credit_card".to_string(),
            vec!["billing_address".to_string()],
        );
        typed
            .dependent_schemas
            .insert("credit_card".to_string(), SchemaNode::object());
        let mut if_schema = SchemaNode::object();
        if_schema
            .properties
            .insert("kind".to_string(), SchemaNode::string());
        typed.if_schema = Some(Box::new(if_schema));
        typed.then_schema = Some(Box::new(SchemaNode::object()));
        typed.else_schema = Some(Box::new(SchemaNode::object()));
        let hint = schema_hint(&typed);
        assert_eq!(hint["additionalProperties"]["type"], "integer");
        assert_eq!(
            hint["dependentRequired"]["credit_card"],
            json!(["billing_address"])
        );
        assert_eq!(hint["dependentSchemas"]["credit_card"]["type"], "object");
        assert_eq!(hint["if"]["type"], "object");
        assert_eq!(hint["then"]["type"], "object");
        assert_eq!(hint["else"]["type"], "object");
    }

    #[test]
    fn missing_api_key_surfaces_descriptive_error() {
        unsafe {
            std::env::remove_var("ALBERT_TEST_MISSING_KEY");
        }
        let config = ProviderConfig {
            provider_name: "test".into(),
            environment: None,
            base_url: "https://example.invalid".into(),
            model: "m".into(),
            api_key_env: "ALBERT_TEST_MISSING_KEY".into(),
            api_type: ProviderApiType::OpenAiCompatible,
            azure_deployment: None,
            azure_api_version: None,
            temperature: None,
            max_output_tokens: None,
            reasoning_effort: None,
            schema_repair_attempts: None,
        };
        let adapter = OpenAiChatAdapter::new(config);
        let err = tokio::runtime::Runtime::new().unwrap().block_on(async {
            adapter
                .generate_mock_example(&endpoint(), GenerationIntent::Success)
                .await
                .err()
                .unwrap()
        });
        match err {
            OpenAiError::MissingApiKey(name) => assert_eq!(name, "ALBERT_TEST_MISSING_KEY"),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
