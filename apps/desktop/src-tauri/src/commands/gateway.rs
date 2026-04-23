use std::collections::BTreeMap;

use albert_core::MockExampleKind;
use albert_gateway::{GatewayConfig, GatewayStatus, RequestLogEntry};
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
    pub default_latency_ms: Option<u64>,
    #[serde(default)]
    pub latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub error_rate: Option<f32>,
    #[serde(default)]
    pub capture_bodies: Option<bool>,
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

    let config = GatewayConfig {
        host: args.host.unwrap_or_else(|| "127.0.0.1".to_string()),
        port: args.port.unwrap_or(4317),
        cors_enabled: args.cors_enabled.unwrap_or(true),
        example_overrides: args.example_overrides.unwrap_or_default(),
        default_latency_ms: args.default_latency_ms,
        latency_overrides: args.latency_overrides.unwrap_or_default(),
        error_rate: args.error_rate.unwrap_or(0.0),
        capture_bodies: args.capture_bodies.unwrap_or(false),
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

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMockServerArgs {
    #[serde(default)]
    pub collection_ids: Option<Vec<String>>,
    #[serde(default)]
    pub example_overrides: Option<BTreeMap<String, MockExampleKind>>,
    #[serde(default)]
    pub default_latency_ms: Option<u64>,
    #[serde(default)]
    pub latency_overrides: Option<BTreeMap<String, u64>>,
    #[serde(default)]
    pub error_rate: Option<f32>,
    #[serde(default)]
    pub capture_bodies: Option<bool>,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub async fn update_mock_server(
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
    // Treat 0 as "clear", any positive number as "set to n".
    let default_latency_ms = match args.default_latency_ms {
        None => current.default_latency_ms,
        Some(0) => None,
        Some(n) => Some(n),
    };
    let latency_overrides = args.latency_overrides.unwrap_or(current.latency_overrides);
    let error_rate = args.error_rate.unwrap_or(current.error_rate);
    let capture_bodies = args.capture_bodies.unwrap_or(current.capture_bodies);

    services
        .gateway
        .reconfigure(
            collections,
            args.example_overrides.unwrap_or_default(),
            default_latency_ms,
            latency_overrides,
            error_rate,
            capture_bodies,
        )
        .await
        .map_err(|error| error.to_string())
}
