use albert_core::{
    CanonicalEndpoint, MockExample, ProviderApiType, ProviderConfig, ProviderReasoningEffort,
};
use albert_openai::{
    GenerationContext, GenerationIntent, OpenAiChatAdapter, PromptBundle, preview_prompt,
    preview_prompt_with_context,
};
use serde::{Deserialize, Serialize};

use crate::services::default_database_url;

const MAX_SCHEMA_REPAIR_ATTEMPTS: u8 = 5;

#[derive(Debug, Clone, Deserialize)]
pub struct GenerationRequest {
    pub endpoint: CanonicalEndpoint,
    pub intent: GenerationIntent,
    pub provider: ProviderConfigInput,
    #[serde(default)]
    pub collection_id: Option<String>,
    #[serde(default)]
    pub persist: Option<bool>,
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default)]
    pub api_key_override: Option<String>,
    #[serde(default)]
    pub generation_context: Option<GenerationContext>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfigInput {
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

impl From<ProviderConfigInput> for ProviderConfig {
    fn from(value: ProviderConfigInput) -> Self {
        ProviderConfig {
            provider_name: value.provider_name,
            environment: value.environment,
            base_url: value.base_url,
            model: value.model,
            api_key_env: value.api_key_env,
            api_type: value.api_type,
            azure_deployment: value.azure_deployment,
            azure_api_version: value.azure_api_version,
            temperature: normalize_temperature(value.temperature),
            max_output_tokens: normalize_max_output_tokens(value.max_output_tokens),
            reasoning_effort: value.reasoning_effort,
            schema_repair_attempts: normalize_schema_repair_attempts(value.schema_repair_attempts),
        }
    }
}

#[tauri::command]
pub fn list_provider_configs(
    database_url: Option<String>,
) -> Result<Vec<ProviderConfigInput>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .list_provider_configs()
        .map(|providers| {
            providers
                .into_iter()
                .map(ProviderConfigInput::from)
                .collect()
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_provider_config(
    provider: ProviderConfigInput,
    database_url: Option<String>,
) -> Result<ProviderConfigInput, String> {
    if provider.provider_name.trim().is_empty() {
        return Err("provider name cannot be empty".into());
    }
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let normalized = ProviderConfigInput {
        provider_name: provider.provider_name.trim().to_string(),
        environment: trim_optional(provider.environment),
        base_url: provider.base_url.trim().to_string(),
        model: provider.model.trim().to_string(),
        api_key_env: provider.api_key_env.trim().to_string(),
        api_type: provider.api_type,
        azure_deployment: trim_optional(provider.azure_deployment),
        azure_api_version: trim_optional(provider.azure_api_version),
        temperature: normalize_temperature(provider.temperature),
        max_output_tokens: normalize_max_output_tokens(provider.max_output_tokens),
        reasoning_effort: provider.reasoning_effort,
        schema_repair_attempts: normalize_schema_repair_attempts(provider.schema_repair_attempts),
    };
    let config: ProviderConfig = normalized.clone().into();
    store
        .save_provider_config(&config)
        .map_err(|error| error.to_string())?;
    Ok(normalized)
}

#[tauri::command]
pub fn delete_provider_config(
    provider_name: String,
    database_url: Option<String>,
) -> Result<bool, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_provider_config(&provider_name)
        .map_err(|error| error.to_string())
}

impl From<ProviderConfig> for ProviderConfigInput {
    fn from(value: ProviderConfig) -> Self {
        ProviderConfigInput {
            provider_name: value.provider_name,
            environment: value.environment,
            base_url: value.base_url,
            model: value.model,
            api_key_env: value.api_key_env,
            api_type: value.api_type,
            azure_deployment: value.azure_deployment,
            azure_api_version: value.azure_api_version,
            temperature: value.temperature,
            max_output_tokens: value.max_output_tokens,
            reasoning_effort: value.reasoning_effort,
            schema_repair_attempts: value.schema_repair_attempts,
        }
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_temperature(value: Option<f32>) -> Option<f32> {
    value.and_then(|value| {
        if value.is_finite() {
            Some(value.clamp(0.0, 2.0))
        } else {
            None
        }
    })
}

fn normalize_max_output_tokens(value: Option<u32>) -> Option<u32> {
    value.filter(|value| *value > 0)
}

fn normalize_schema_repair_attempts(value: Option<u8>) -> Option<u8> {
    value.map(|value| value.min(MAX_SCHEMA_REPAIR_ATTEMPTS))
}

#[tauri::command]
pub async fn generate_mock_example(request: GenerationRequest) -> Result<MockExample, String> {
    let provider: ProviderConfig = request.provider.into();
    let mut adapter = OpenAiChatAdapter::new(provider);
    if let Some(key) = request.api_key_override
        && !key.trim().is_empty()
    {
        adapter = adapter.with_api_key(key);
    }
    let endpoint = request.endpoint;
    let intent = request.intent;
    let example = adapter
        .generate_mock_example_with_context(&endpoint, intent, request.generation_context.as_ref())
        .await
        .map_err(|error| error.to_string())?;

    if request.persist.unwrap_or(false)
        && let Some(collection_id) = request.collection_id
    {
        let database_url = request.database_url.unwrap_or_else(default_database_url);
        let store = albert_storage::SqliteStore::new(database_url);
        store.migrate().map_err(|error| error.to_string())?;
        store
            .replace_mock_example(
                &collection_id,
                endpoint.method.as_str(),
                &endpoint.path,
                &example,
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(example)
}

#[derive(Debug, Serialize)]
pub struct PromptPreview {
    pub system: String,
    pub user: String,
    pub endpoint_context: serde_json::Value,
}

impl From<PromptBundle> for PromptPreview {
    fn from(value: PromptBundle) -> Self {
        PromptPreview {
            system: value.system,
            user: value.user,
            endpoint_context: value.endpoint_context,
        }
    }
}

#[tauri::command]
pub fn preview_generation_prompt(
    endpoint: CanonicalEndpoint,
    intent: GenerationIntent,
    generation_context: Option<GenerationContext>,
) -> PromptPreview {
    if generation_context.is_some() {
        preview_prompt_with_context(&endpoint, intent, generation_context.as_ref()).into()
    } else {
        preview_prompt(&endpoint, intent).into()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestConnectionArgs {
    pub provider: ProviderConfigInput,
    #[serde(default)]
    pub api_key_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestConnectionResult {
    pub ok: bool,
    pub message: String,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderEnvStatus {
    pub env_var: String,
    pub env_present: bool,
    pub override_present: bool,
    pub usable: bool,
    pub message: String,
}

#[tauri::command]
pub fn provider_env_status(args: TestConnectionArgs) -> ProviderEnvStatus {
    let env_var = args.provider.api_key_env.trim().to_string();
    let override_present = args
        .api_key_override
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let env_present = if env_var.is_empty() {
        false
    } else {
        std::env::var(&env_var).is_ok_and(|value| !value.trim().is_empty())
    };
    let usable = override_present || env_present;
    let message = if override_present {
        "Session API key override is active.".to_string()
    } else if env_present {
        format!("{env_var} is present in the Tauri backend environment.")
    } else if env_var.is_empty() {
        "No API key environment variable is configured.".to_string()
    } else {
        format!("{env_var} is not set in the Tauri backend environment.")
    };

    ProviderEnvStatus {
        env_var,
        env_present,
        override_present,
        usable,
        message,
    }
}

/// Quick round-trip probe that exercises the configured provider without
/// generating a full payload. Uses the adapter's prompt builder with a
/// trivial endpoint so a missing key / bad base URL / 4xx all surface the
/// same way.
#[tauri::command]
pub async fn test_provider_connection(args: TestConnectionArgs) -> TestConnectionResult {
    use albert_core::{CanonicalEndpoint, HttpMethod};

    let provider: ProviderConfig = args.provider.into();
    let mut adapter = OpenAiChatAdapter::new(provider);
    if let Some(key) = args.api_key_override
        && !key.trim().is_empty()
    {
        adapter = adapter.with_api_key(key);
    }
    // Shorten the timeout so an unreachable provider doesn't hang the UI.
    adapter = adapter.with_timeout(std::time::Duration::from_secs(8));

    let endpoint = CanonicalEndpoint {
        operation_id: Some("test".into()),
        method: HttpMethod::Get,
        path: "/ping".into(),
        summary: Some("connectivity probe".into()),
        description: None,
        tags: Vec::new(),
        parameters: Vec::new(),
        request_body: None,
        responses: Vec::new(),
        examples: Vec::new(),
        auth: None,
    };
    let bundle = albert_openai::build_prompt_bundle(&endpoint, GenerationIntent::Success);

    match adapter.call_chat(&bundle).await {
        Ok(_) => TestConnectionResult {
            ok: true,
            message: "Provider reachable and returned valid JSON.".to_string(),
            status: Some(200),
        },
        Err(err) => {
            let (status, message) = match &err {
                albert_openai::OpenAiError::Provider { status, body } => {
                    (Some(*status), format!("HTTP {status}: {body}"))
                }
                other => (None, other.to_string()),
            };
            TestConnectionResult {
                ok: false,
                message,
                status,
            }
        }
    }
}
