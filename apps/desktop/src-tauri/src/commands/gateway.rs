use std::collections::BTreeMap;

use albert_core::MockExampleKind;
use albert_gateway::{
    CachedResponse, ConditionalExampleRule, GatewayConfig, GatewayConfigBundle, GatewayStatus,
    MetricsSnapshot, RateLimitRule, ReconfigureOptions, RequestLogEntry, RequiredHeader,
};
use serde::Deserialize;
use tauri::State;

use crate::services::{AppServices, default_database_url};

#[derive(Debug, Clone, Deserialize)]
pub struct StartMockServerArgs {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub cors_enabled: Option<bool>,
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
    #[serde(default)]
    pub example_overrides: Option<BTreeMap<String, MockExampleKind>>,
    #[serde(default)]
    pub conditional_example_rules: Option<BTreeMap<String, Vec<ConditionalExampleRule>>>,
    #[serde(default)]
    pub use_request_cache: Option<bool>,
    #[serde(default)]
    pub default_latency_ms: Option<u64>,
    #[serde(default)]
    pub latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub latency_jitter_ms: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub error_rate: Option<f32>,
    #[serde(default)]
    pub capture_bodies: Option<bool>,
    #[serde(default)]
    pub enforce_request_bodies: Option<bool>,
    #[serde(default)]
    pub response_headers: Option<BTreeMap<String, BTreeMap<String, String>>>,
    #[serde(default)]
    pub required_headers: Option<BTreeMap<String, Vec<RequiredHeader>>>,
    #[serde(default)]
    pub rate_limits: Option<BTreeMap<String, RateLimitRule>>,
    #[serde(default)]
    pub status_overrides: Option<BTreeMap<String, u16>>,
    #[serde(default)]
    pub proxy_upstream: Option<String>,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub async fn start_mock_server(
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

    let use_request_cache = args.use_request_cache.unwrap_or(false);
    let request_cache_entries = if use_request_cache {
        load_gateway_request_cache(&store, &collections)?
    } else {
        BTreeMap::new()
    };

    let config = GatewayConfig {
        host: args.host.unwrap_or_else(|| "127.0.0.1".to_string()),
        port: args.port.unwrap_or(4317),
        cors_enabled: args.cors_enabled.unwrap_or(true),
        example_overrides: args.example_overrides.unwrap_or_default(),
        conditional_example_rules: args.conditional_example_rules.unwrap_or_default(),
        use_request_cache,
        request_cache_entries,
        default_latency_ms: args.default_latency_ms,
        latency_overrides: args.latency_overrides.unwrap_or_default(),
        latency_jitter_ms: args.latency_jitter_ms.unwrap_or_default(),
        error_rate: args.error_rate.unwrap_or(0.0),
        capture_bodies: args.capture_bodies.unwrap_or(false),
        enforce_request_bodies: args.enforce_request_bodies.unwrap_or(false),
        response_headers: args.response_headers.unwrap_or_default(),
        required_headers: args.required_headers.unwrap_or_default(),
        rate_limits: args.rate_limits.unwrap_or_default(),
        status_overrides: args.status_overrides.unwrap_or_default(),
        proxy_upstream: args.proxy_upstream.filter(|s| !s.trim().is_empty()),
    };

    services
        .gateway
        .start(collections, config)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn stop_mock_server(services: State<'_, AppServices>) -> Result<GatewayStatus, String> {
    services
        .gateway
        .stop()
        .await
        .map_err(|error| error.to_string())?;
    Ok(services.gateway.status().await)
}

#[tauri::command]
pub async fn mock_server_status(services: State<'_, AppServices>) -> Result<GatewayStatus, String> {
    Ok(services.gateway.status().await)
}

#[tauri::command]
pub async fn mock_server_requests(
    limit: Option<usize>,
    services: State<'_, AppServices>,
) -> Result<Vec<RequestLogEntry>, String> {
    Ok(services.gateway.recent_requests(limit.unwrap_or(50)).await)
}

#[tauri::command]
pub async fn mock_server_metrics(
    services: State<'_, AppServices>,
) -> Result<MetricsSnapshot, String> {
    Ok(services.gateway.metrics().await)
}

#[tauri::command]
pub async fn export_gateway_config(
    services: State<'_, AppServices>,
) -> Result<GatewayConfigBundle, String> {
    services
        .gateway
        .export_bundle()
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportGatewayConfigArgs {
    pub bundle: GatewayConfigBundle,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub async fn import_gateway_config(
    args: ImportGatewayConfigArgs,
    services: State<'_, AppServices>,
) -> Result<GatewayStatus, String> {
    let database_url = args.database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url);
    store.migrate().map_err(|error| error.to_string())?;

    // Resolve the collection IDs the bundle references against the local
    // SQLite. Missing ids are surfaced rather than silently dropped so
    // the user knows when their dataset is out of sync with what the
    // bundle expects.
    let mut collections = Vec::with_capacity(args.bundle.collection_ids.len());
    let mut missing: Vec<String> = Vec::new();
    for id in &args.bundle.collection_ids {
        match store
            .load_collection(id)
            .map_err(|error| error.to_string())?
        {
            Some(c) => collections.push(c),
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "bundle references unknown collections: {}",
            missing.join(", ")
        ));
    }
    services
        .gateway
        .import_bundle(args.bundle, collections)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mock_server_clear_log(services: State<'_, AppServices>) -> Result<(), String> {
    services
        .gateway
        .clear_log()
        .await
        .map_err(|error| error.to_string())
}

/// Load the last-saved gateway preferences. Returns `null` when the user
/// has never started the mock server before.
#[tauri::command]
pub fn load_gateway_preferences(
    database_url: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .load_gateway_preferences()
        .map_err(|error| error.to_string())
}

/// Save gateway preferences so the next session can restore them. Accepts
/// any JSON blob — the frontend owns the shape so we don't need to bump
/// the migration for new fields.
#[tauri::command]
pub fn save_gateway_preferences(
    payload: serde_json::Value,
    database_url: Option<String>,
) -> Result<(), String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .save_gateway_preferences(&payload)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateMockServerArgs {
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
    #[serde(default)]
    pub example_overrides: Option<BTreeMap<String, MockExampleKind>>,
    #[serde(default)]
    pub conditional_example_rules: Option<BTreeMap<String, Vec<ConditionalExampleRule>>>,
    #[serde(default)]
    pub use_request_cache: Option<bool>,
    #[serde(default)]
    pub default_latency_ms: Option<u64>,
    #[serde(default)]
    pub latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub latency_jitter_ms: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub error_rate: Option<f32>,
    #[serde(default)]
    pub capture_bodies: Option<bool>,
    #[serde(default)]
    pub enforce_request_bodies: Option<bool>,
    #[serde(default)]
    pub response_headers: Option<BTreeMap<String, BTreeMap<String, String>>>,
    #[serde(default)]
    pub required_headers: Option<BTreeMap<String, Vec<RequiredHeader>>>,
    #[serde(default)]
    pub rate_limits: Option<BTreeMap<String, RateLimitRule>>,
    #[serde(default)]
    pub status_overrides: Option<BTreeMap<String, u16>>,
    /// Use `Some(None)` to clear the proxy; `Some(Some("..."))` to set;
    /// `None` to leave the current value alone. `null` and empty strings
    /// both land on "clear" so the Mock Server panel's "none" radio works.
    #[serde(default, deserialize_with = "deserialize_nullable_option")]
    pub proxy_upstream: Option<Option<String>>,
    #[serde(default)]
    pub database_url: Option<String>,
}

/// Distinguish the three cases JSON can't normally express: field missing
/// (None), field present as `null` (Some(None)), field present as a
/// string (Some(Some(…))). Empty/whitespace strings are treated as
/// explicit clears so "proxy_upstream": "" also means "turn it off".
fn deserialize_nullable_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    Ok(match value {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(serde_json::Value::String(s)) => {
            if s.trim().is_empty() {
                Some(None)
            } else {
                Some(Some(s))
            }
        }
        Some(other) => {
            return Err(serde::de::Error::custom(format!(
                "proxy_upstream must be null or a string, got {}",
                other
            )));
        }
    })
}

fn load_gateway_request_cache(
    store: &albert_storage::SqliteStore,
    collections: &[albert_core::CanonicalApiCollection],
) -> Result<BTreeMap<String, CachedResponse>, String> {
    let mut out = BTreeMap::new();
    for collection in collections {
        for endpoint in &collection.endpoints {
            let entries = store
                .list_request_cache(&collection.id, endpoint.method.as_str(), &endpoint.path, 25)
                .map_err(|error| error.to_string())?;
            for entry in entries {
                let Some(cached) = cached_response_from_entry(entry) else {
                    continue;
                };
                out.insert(cached.fingerprint.clone(), cached);
            }
        }
    }
    Ok(out)
}

fn cached_response_from_entry(entry: albert_storage::RequestCacheEntry) -> Option<CachedResponse> {
    let response = entry.response_snapshot.as_object()?;
    let status = response
        .get("status")
        .and_then(|value| value.as_u64())
        .and_then(|value| u16::try_from(value).ok())?;
    let body = response
        .get("body")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let headers = response
        .get("headers")
        .and_then(|value| value.as_object())
        .map(|map| {
            map.iter()
                .map(|(key, value)| {
                    (
                        key.to_ascii_lowercase(),
                        value
                            .as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| value.to_string()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Some(CachedResponse {
        collection_id: entry.collection_id,
        method: method_from_cache_entry(&entry.method)?,
        path: entry.path,
        fingerprint: entry.fingerprint,
        status,
        headers,
        body,
        hit_count: entry.hit_count,
        last_seen_at: Some(entry.last_seen_at),
    })
}

fn method_from_cache_entry(method: &str) -> Option<albert_core::HttpMethod> {
    match method.trim().to_ascii_uppercase().as_str() {
        "GET" => Some(albert_core::HttpMethod::Get),
        "POST" => Some(albert_core::HttpMethod::Post),
        "PUT" => Some(albert_core::HttpMethod::Put),
        "PATCH" => Some(albert_core::HttpMethod::Patch),
        "DELETE" => Some(albert_core::HttpMethod::Delete),
        "OPTIONS" => Some(albert_core::HttpMethod::Options),
        "HEAD" => Some(albert_core::HttpMethod::Head),
        _ => None,
    }
}

#[tauri::command]
pub async fn update_mock_server(
    args: UpdateMockServerArgs,
    services: State<'_, AppServices>,
) -> Result<GatewayStatus, String> {
    update_mock_server_impl(args, services.inner()).await
}

async fn update_mock_server_impl(
    args: UpdateMockServerArgs,
    services: &AppServices,
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
    let example_overrides = args.example_overrides.unwrap_or(current.example_overrides);
    // Treat 0 as "clear", any positive number as "set to n".
    let default_latency_ms = match args.default_latency_ms {
        None => current.default_latency_ms,
        Some(0) => None,
        Some(n) => Some(n),
    };
    let latency_overrides = args.latency_overrides.unwrap_or(current.latency_overrides);
    let latency_jitter_ms = args.latency_jitter_ms.unwrap_or(current.latency_jitter_ms);
    let error_rate = args.error_rate.unwrap_or(current.error_rate);
    let capture_bodies = args.capture_bodies.unwrap_or(current.capture_bodies);
    let enforce_request_bodies = args
        .enforce_request_bodies
        .unwrap_or(current.enforce_request_bodies);
    let response_headers = args.response_headers.unwrap_or(current.response_headers);
    let required_headers = args.required_headers.unwrap_or(current.required_headers);
    let conditional_example_rules = args
        .conditional_example_rules
        .unwrap_or(current.conditional_example_rules);
    let rate_limits = args.rate_limits.unwrap_or(current.rate_limits);
    let status_overrides = args.status_overrides.unwrap_or(current.status_overrides);
    let proxy_upstream = match args.proxy_upstream {
        None => current.proxy_upstream,
        Some(next) => next,
    };
    let use_request_cache = args.use_request_cache.unwrap_or(current.use_request_cache);
    let request_cache_entries = if use_request_cache {
        load_gateway_request_cache(&store, &collections)?
    } else {
        BTreeMap::new()
    };

    services
        .gateway
        .reconfigure(ReconfigureOptions {
            collections,
            overrides: example_overrides,
            conditional_example_rules,
            use_request_cache,
            request_cache_entries,
            default_latency_ms,
            latency_overrides,
            latency_jitter_ms,
            error_rate,
            capture_bodies,
            enforce_request_bodies,
            response_headers,
            required_headers,
            rate_limits,
            status_overrides,
            proxy_upstream,
        })
        .await
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// Scenarios: named gateway config presets
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_gateway_scenarios(
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredScenarioSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store.list_scenarios().map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveScenarioArgs {
    pub name: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

/// Capture the live gateway config as a named scenario. The payload is the
/// same `GatewayConfigBundle` shape produced by `export_gateway_config`, so
/// scenarios can be round-tripped through file-based bundle export/import.
#[tauri::command]
pub async fn save_gateway_scenario(
    args: SaveScenarioArgs,
    services: State<'_, AppServices>,
) -> Result<albert_storage::StoredScenarioSummary, String> {
    let bundle = services
        .gateway
        .export_bundle()
        .await
        .map_err(|error| error.to_string())?;
    let payload = serde_json::to_value(&bundle).map_err(|error| error.to_string())?;

    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .save_scenario(&args.name, &payload)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadScenarioArgs {
    pub name: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

/// Activate a saved scenario: load its bundle, hydrate collections from
/// SQLite, and apply via `import_bundle`. Returns the resulting gateway
/// status so the UI can refresh immediately.
#[tauri::command]
pub async fn load_gateway_scenario(
    args: LoadScenarioArgs,
    services: State<'_, AppServices>,
) -> Result<GatewayStatus, String> {
    let database_url = args.database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url);
    store.migrate().map_err(|error| error.to_string())?;

    let payload = store
        .load_scenario(&args.name)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("scenario '{}' not found", args.name))?;
    let bundle: GatewayConfigBundle =
        serde_json::from_value(payload).map_err(|error| error.to_string())?;

    let mut collections = Vec::with_capacity(bundle.collection_ids.len());
    let mut missing: Vec<String> = Vec::new();
    for id in &bundle.collection_ids {
        match store
            .load_collection(id)
            .map_err(|error| error.to_string())?
        {
            Some(c) => collections.push(c),
            None => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "scenario '{}' references unknown collections: {}",
            args.name,
            missing.join(", ")
        ));
    }
    services
        .gateway
        .import_bundle(bundle, collections)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteScenarioArgs {
    pub name: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub fn delete_gateway_scenario(args: DeleteScenarioArgs) -> Result<bool, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_scenario(&args.name)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameScenarioArgs {
    pub old_name: String,
    pub new_name: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub fn rename_gateway_scenario(args: RenameScenarioArgs) -> Result<bool, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .rename_scenario(&args.old_name, &args.new_name)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{
        CanonicalApiCollection, CanonicalEndpoint, CanonicalResponse, HttpMethod, InputSourceKind,
        MockExampleKind, SchemaNode, default_mock_examples,
    };
    use albert_gateway::{ConditionalExampleRule, MockGateway, RequestCondition};
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn update_preserves_overrides_and_replaces_conditional_rules() {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = temp_file.path().to_string_lossy().to_string();
        let store = albert_storage::SqliteStore::new(database_url.clone());
        store.migrate().unwrap();
        store.save_collection(&sample_collection()).unwrap();

        let gateway = MockGateway::new();
        gateway
            .start(
                store.load_all_collections().unwrap(),
                GatewayConfig {
                    port: 0,
                    example_overrides: BTreeMap::from([(
                        "GET /orders".to_string(),
                        MockExampleKind::Error,
                    )]),
                    ..GatewayConfig::default()
                },
            )
            .await
            .unwrap();

        let rules = BTreeMap::from([(
            "GET /orders".to_string(),
            vec![ConditionalExampleRule {
                name: "VIP empty list".to_string(),
                example: MockExampleKind::Empty,
                when: vec![RequestCondition::Query {
                    name: "status".to_string(),
                    equals: "empty".to_string(),
                }],
            }],
        )]);
        let services = AppServices {
            gateway: std::sync::Arc::new(gateway),
        };
        let status = update_mock_server_impl(
            UpdateMockServerArgs {
                conditional_example_rules: Some(rules.clone()),
                database_url: Some(database_url),
                ..UpdateMockServerArgs::default()
            },
            &services,
        )
        .await
        .unwrap();

        assert_eq!(
            status.config.example_overrides.get("GET /orders"),
            Some(&MockExampleKind::Error)
        );
        assert_eq!(status.config.conditional_example_rules, rules);
        services.gateway.stop().await.unwrap();
    }

    fn sample_collection() -> CanonicalApiCollection {
        CanonicalApiCollection {
            id: "orders".to_string(),
            name: "Orders".to_string(),
            source: InputSourceKind::OpenApi,
            description: Some("Sample collection".to_string()),
            endpoints: vec![CanonicalEndpoint {
                operation_id: Some("listOrders".to_string()),
                method: HttpMethod::Get,
                path: "/orders".to_string(),
                summary: Some("List orders".to_string()),
                description: None,
                tags: vec!["orders".to_string()],
                parameters: Vec::new(),
                request_body: None,
                responses: vec![CanonicalResponse {
                    status_code: "200".to_string(),
                    description: Some("OK".to_string()),
                    content_type: "application/json".to_string(),
                    schema: Some(SchemaNode::object()),
                }],
                examples: default_mock_examples(),
                auth: None,
            }],
        }
    }
}
