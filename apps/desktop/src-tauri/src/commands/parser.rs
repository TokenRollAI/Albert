use albert_core::CanonicalApiCollection;
use serde::Serialize;

use crate::services::default_database_url;

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub collection_id: String,
    pub collection_name: String,
    pub endpoint_count: usize,
    pub database_url: String,
}

#[tauri::command]
pub fn parse_api_description(
    body: String,
    name: Option<String>,
) -> Result<CanonicalApiCollection, String> {
    albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn import_api_description(
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
pub fn list_imported_collections(
    database_url: Option<String>,
) -> Result<Vec<albert_storage::StoredCollectionSummary>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store.list_collections().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_imported_endpoints(
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
pub fn load_collection_snapshot(
    collection_id: String,
    database_url: Option<String>,
) -> Result<Option<CanonicalApiCollection>, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .load_collection(&collection_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_collection(
    collection_id: String,
    new_name: String,
    database_url: Option<String>,
) -> Result<bool, String> {
    if new_name.trim().is_empty() {
        return Err("collection name cannot be empty".into());
    }
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .rename_collection(&collection_id, new_name.trim())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_collection(
    collection_id: String,
    database_url: Option<String>,
) -> Result<bool, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    store
        .delete_collection(&collection_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_collection_json(
    collection_id: String,
    database_url: Option<String>,
) -> Result<String, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let collection = store
        .load_collection(&collection_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("collection '{collection_id}' not found"))?;
    serde_json::to_string_pretty(&collection).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_all_collections_json(database_url: Option<String>) -> Result<String, String> {
    let store = albert_storage::SqliteStore::new(database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let collections = store
        .load_all_collections()
        .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&collections).map_err(|err| err.to_string())
}
