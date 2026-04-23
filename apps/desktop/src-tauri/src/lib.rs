use albert_core::{AppBootstrapSummary, CanonicalApiCollection};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ImportResult {
    collection_id: String,
    collection_name: String,
    endpoint_count: usize,
    database_url: String,
}

#[tauri::command]
fn bootstrap_summary() -> AppBootstrapSummary {
    AppBootstrapSummary {
        project_name: "Albert".to_string(),
        current_phase: "Phase 2 - Parsing And Persistence".to_string(),
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
    let database_url = database_url.unwrap_or_else(|| default_database_url());
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

fn default_database_url() -> String {
    "albert.db".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            bootstrap_summary,
            parse_api_description,
            import_api_description,
            list_imported_collections,
            list_imported_endpoints
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Albert desktop app");
}
