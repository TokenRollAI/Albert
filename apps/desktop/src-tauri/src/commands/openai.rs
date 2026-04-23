use albert_core::{CanonicalEndpoint, MockExample, ProviderConfig};
use albert_openai::{GenerationIntent, OpenAiChatAdapter, PromptBundle, preview_prompt};
use serde::{Deserialize, Serialize};

use crate::services::default_database_url;

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfigInput {
    pub provider_name: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
}

impl From<ProviderConfigInput> for ProviderConfig {
    fn from(value: ProviderConfigInput) -> Self {
        ProviderConfig {
            provider_name: value.provider_name,
            base_url: value.base_url,
            model: value.model,
            api_key_env: value.api_key_env,
        }
    }
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
        .generate_mock_example(&endpoint, intent)
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
) -> PromptPreview {
    preview_prompt(&endpoint, intent).into()
}
