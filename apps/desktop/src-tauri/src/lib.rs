use std::collections::BTreeMap;
use std::sync::Arc;

use albert_core::{
    AppBootstrapSummary, CanonicalApiCollection, CanonicalEndpoint, HttpMethod, MockExample,
    MockExampleKind, ProviderConfig,
};
use albert_gateway::{GatewayConfig, GatewayStatus, MockGateway, RequestLogEntry};
use albert_openai::{GenerationIntent, OpenAiChatAdapter, PromptBundle, preview_prompt};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
struct ImportResult {
    collection_id: String,
    collection_name: String,
    endpoint_count: usize,
    database_url: String,
}

struct AppServices {
    gateway: Arc<MockGateway>,
}

impl AppServices {
    fn new() -> Self {
        Self {
            gateway: Arc::new(MockGateway::new()),
        }
    }
}

#[tauri::command]
fn bootstrap_summary() -> AppBootstrapSummary {
    AppBootstrapSummary {
        project_name: "Albert".to_string(),
        current_phase: "Phase 3 - Static Mock Runtime".to_string(),
        ui_surfaces: vec![
            "Overview".to_string(),
            "Import".to_string(),
            "Endpoints".to_string(),
            "Providers".to_string(),
            "Mock Server".to_string(),
        ],
        parser_capabilities: albert_parser::planned_capabilities(),
        storage_capabilities: albert_storage::planned_capabilities(),
        provider_capabilities: albert_openai::planned_capabilities(),
        gateway_capabilities: albert_gateway::planned_capabilities(),
    }
}

#[tauri::command]
fn parse_api_description(
    body: String,
    name: Option<String>,
) -> Result<CanonicalApiCollection, String> {
    albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_api_description(
    body: String,
    name: Option<String>,
    database_url: Option<String>,
) -> Result<ImportResult, String> {
    let collection = albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())?;
    let database_url = database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url.clone());

    store.migrate().map_err(|error| error.to_string())?;
    store
        .save_collection(&collection)
        .map_err(|error| error.to_string())?;

    Ok(ImportResult {
        collection_id: collection.id,
        collection_name: collection.name,
        endpoint_count: collection.endpoints.len(),
        database_url,
    })
}

#[tauri::command]
fn list_imported_collections(
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredCollectionSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store.list_collections().map_err(|error| error.to_string())
}

#[tauri::command]
fn list_imported_endpoints(
    collection_id: String,
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredEndpointSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .list_endpoints(&collection_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_collection_snapshot(
    collection_id: String,
    database_url: Option<String>,
) -> Result<Option<CanonicalApiCollection>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .load_collection(&collection_id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
struct StartMockServerArgs {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    cors_enabled: Option<bool>,
    #[serde(default)]
    collection_ids: Option<Vec<String>>,
    #[serde(default)]
    example_overrides: Option<BTreeMap<String, MockExampleKind>>,
    #[serde(default)]
    default_latency_ms: Option<u64>,
    #[serde(default)]
    latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    database_url: Option<String>,
}

#[tauri::command]
async fn start_mock_server(
    args: StartMockServerArgs,
    services: State<'_, AppServices>,
) -> Result<GatewayStatus, String> {
    let database_url = args.database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url);
    store.migrate().map_err(|error| error.to_string())?;

    let collections = if let Some(ids) = args.collection_ids {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(collection) = store
                .load_collection(&id)
                .map_err(|error| error.to_string())?
            {
                out.push(collection);
            }
        }
        out
    } else {
        store
            .load_all_collections()
            .map_err(|error| error.to_string())?
    };

    let config = GatewayConfig {
        host: args.host.unwrap_or_else(|| "127.0.0.1".to_string()),
        port: args.port.unwrap_or(4317),
        cors_enabled: args.cors_enabled.unwrap_or(true),
        example_overrides: args.example_overrides.unwrap_or_default(),
        default_latency_ms: args.default_latency_ms,
        latency_overrides: args.latency_overrides.unwrap_or_default(),
    };

    services
        .gateway
        .start(collections, config)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn stop_mock_server(services: State<'_, AppServices>) -> Result<GatewayStatus, String> {
    services
        .gateway
        .stop()
        .await
        .map_err(|error| error.to_string())?;
    Ok(services.gateway.status().await)
}

#[tauri::command]
async fn mock_server_status(services: State<'_, AppServices>) -> Result<GatewayStatus, String> {
    Ok(services.gateway.status().await)
}

#[tauri::command]
async fn mock_server_requests(
    limit: Option<usize>,
    services: State<'_, AppServices>,
) -> Result<Vec<RequestLogEntry>, String> {
    Ok(services.gateway.recent_requests(limit.unwrap_or(50)).await)
}

#[derive(Debug, Clone, Deserialize)]
struct UpdateMockServerArgs {
    #[serde(default)]
    collection_ids: Option<Vec<String>>,
    #[serde(default)]
    example_overrides: Option<BTreeMap<String, MockExampleKind>>,
    #[serde(default)]
    default_latency_ms: Option<Option<u64>>,
    #[serde(default)]
    latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    database_url: Option<String>,
}

#[tauri::command]
async fn update_mock_server(
    args: UpdateMockServerArgs,
    services: State<'_, AppServices>,
) -> Result<GatewayStatus, String> {
    let database_url = args.database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url);
    store.migrate().map_err(|error| error.to_string())?;
    let collections = if let Some(ids) = args.collection_ids {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(collection) = store
                .load_collection(&id)
                .map_err(|error| error.to_string())?
            {
                out.push(collection);
            }
        }
        out
    } else {
        store
            .load_all_collections()
            .map_err(|error| error.to_string())?
    };

    let current = services.gateway.status().await.config;
    let default_latency_ms = args
        .default_latency_ms
        .unwrap_or(current.default_latency_ms);
    let latency_overrides = args.latency_overrides.unwrap_or(current.latency_overrides);

    services
        .gateway
        .reconfigure(
            collections,
            args.example_overrides.unwrap_or_default(),
            default_latency_ms,
            latency_overrides,
        )
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
struct GenerationRequest {
    endpoint: CanonicalEndpoint,
    intent: GenerationIntent,
    provider: ProviderConfigInput,
    #[serde(default)]
    collection_id: Option<String>,
    #[serde(default)]
    persist: Option<bool>,
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    api_key_override: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderConfigInput {
    provider_name: String,
    base_url: String,
    model: String,
    api_key_env: String,
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
async fn generate_mock_example(request: GenerationRequest) -> Result<MockExample, String> {
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
struct PromptPreview {
    system: String,
    user: String,
    endpoint_context: serde_json::Value,
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
fn preview_generation_prompt(
    endpoint: CanonicalEndpoint,
    intent: GenerationIntent,
) -> PromptPreview {
    preview_prompt(&endpoint, intent).into()
}

#[tauri::command]
fn default_gateway_config() -> GatewayConfig {
    GatewayConfig::default()
}

#[tauri::command]
fn supported_http_methods() -> Vec<&'static str> {
    vec![
        HttpMethod::Get.as_str(),
        HttpMethod::Post.as_str(),
        HttpMethod::Put.as_str(),
        HttpMethod::Patch.as_str(),
        HttpMethod::Delete.as_str(),
        HttpMethod::Options.as_str(),
        HttpMethod::Head.as_str(),
    ]
}

fn default_database_url() -> String {
    "albert.db".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppServices::new())
        .invoke_handler(tauri::generate_handler![
            bootstrap_summary,
            parse_api_description,
            import_api_description,
            list_imported_collections,
            list_imported_endpoints,
            load_collection_snapshot,
            start_mock_server,
            stop_mock_server,
            mock_server_status,
            mock_server_requests,
            update_mock_server,
            generate_mock_example,
            preview_generation_prompt,
            default_gateway_config,
            supported_http_methods
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Albert desktop app");
}
