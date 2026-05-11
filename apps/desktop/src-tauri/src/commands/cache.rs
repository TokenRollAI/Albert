use albert_storage::{RequestCacheEntry, RequestCacheInput};
use serde::Deserialize;

use crate::services::default_database_url;

#[derive(Debug, Clone, Deserialize)]
pub struct SaveRequestCacheArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub request_snapshot: serde_json::Value,
    pub response_snapshot: serde_json::Value,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListRequestCacheArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FindRequestCacheArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub request_snapshot: serde_json::Value,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteRequestCacheArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub cache_id: String,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteStaleRequestCacheArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub stale_before_epoch_seconds: u64,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub fn save_request_cache(args: SaveRequestCacheArgs) -> Result<RequestCacheEntry, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .upsert_request_cache(&RequestCacheInput {
            collection_id: args.collection_id,
            method: args.method,
            path: args.path,
            request_snapshot: args.request_snapshot,
            response_snapshot: args.response_snapshot,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_request_cache(args: ListRequestCacheArgs) -> Result<Vec<RequestCacheEntry>, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .list_request_cache(
            &args.collection_id,
            &args.method,
            &args.path,
            args.limit.unwrap_or(5),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_request_cache(args: DeleteRequestCacheArgs) -> Result<bool, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_request_cache(
            &args.collection_id,
            &args.method,
            &args.path,
            &args.cache_id,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_stale_request_cache(args: DeleteStaleRequestCacheArgs) -> Result<usize, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_stale_request_cache(
            &args.collection_id,
            &args.method,
            &args.path,
            args.stale_before_epoch_seconds,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn find_request_cache(args: FindRequestCacheArgs) -> Result<Option<RequestCacheEntry>, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .find_request_cache(
            &args.collection_id,
            &args.method,
            &args.path,
            &args.request_snapshot,
        )
        .map_err(|error| error.to_string())
}
