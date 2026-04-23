use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, MockExample, MockExampleKind, synthesize_value,
};
use serde::{Deserialize, Serialize};

use crate::services::default_database_url;

/// Persist a single or bundled set of collections into the store.
fn persist_collections(
    store: &albert_storage::SqliteStore,
    collections: &[CanonicalApiCollection],
) -> Result<(), String> {
    store.migrate().map_err(|error| error.to_string())?;
    for collection in collections {
        store
            .save_collection(collection)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub collection_id: String,
    pub collection_name: String,
    pub endpoint_count: usize,
    pub database_url: String,
}

/// Synthesize a JSON sample for a request body based on the canonical
/// schema. Returns `null` when the endpoint doesn't declare a request body,
/// so the frontend can show a placeholder instead of failing.
#[tauri::command]
pub fn synthesize_request_body(endpoint: CanonicalEndpoint) -> serde_json::Value {
    endpoint
        .request_body
        .as_ref()
        .map(|body| synthesize_value(&body.schema))
        .unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
pub fn parse_api_description(
    body: String,
    name: Option<String>,
) -> Result<CanonicalApiCollection, String> {
    albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
pub struct BundleImportResult {
    pub database_url: String,
    pub imported: Vec<ImportResult>,
}

#[tauri::command]
pub fn import_api_description(
    body: String,
    name: Option<String>,
    database_url: Option<String>,
) -> Result<ImportResult, String> {
    // Fast path: bundle import. If the body is a JSON array of canonical
    // snapshots we persist every entry and return the first one's summary.
    // For more visibility the caller can invoke `import_bundle` explicitly.
    if let Some(collections) =
        albert_parser::try_parse_bundle(&body).map_err(|error| error.to_string())?
        && let Some(first) = collections.first().cloned()
    {
        let database_url = database_url.unwrap_or_else(default_database_url);
        let store = albert_storage::SqliteStore::new(database_url.clone());
        persist_collections(&store, &collections)?;
        return Ok(ImportResult {
            collection_id: first.id,
            collection_name: first.name,
            endpoint_count: collections.iter().map(|c| c.endpoints.len()).sum(),
            database_url,
        });
    }

    let collection = albert_parser::parse_source(albert_parser::ParseSource { name, body })
        .map_err(|error| error.to_string())?;
    let database_url = database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url.clone());
    persist_collections(&store, std::slice::from_ref(&collection))?;

    Ok(ImportResult {
        collection_id: collection.id,
        collection_name: collection.name,
        endpoint_count: collection.endpoints.len(),
        database_url,
    })
}

#[tauri::command]
pub fn import_bundle(
    body: String,
    database_url: Option<String>,
) -> Result<BundleImportResult, String> {
    let collections = albert_parser::try_parse_bundle(&body)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "body is not a recognized collection bundle".to_string())?;
    let database_url = database_url.unwrap_or_else(default_database_url);
    let store = albert_storage::SqliteStore::new(database_url.clone());
    persist_collections(&store, &collections)?;
    let imported = collections
        .iter()
        .map(|c| ImportResult {
            collection_id: c.id.clone(),
            collection_name: c.name.clone(),
            endpoint_count: c.endpoints.len(),
            database_url: database_url.clone(),
        })
        .collect();
    Ok(BundleImportResult {
        database_url,
        imported,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SaveMockExampleArgs {
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub kind: MockExampleKind,
    pub title: Option<String>,
    pub payload: serde_json::Value,
    pub note: Option<String>,
    #[serde(default)]
    pub database_url: Option<String>,
}

#[tauri::command]
pub fn save_mock_example(args: SaveMockExampleArgs) -> Result<MockExample, String> {
    let store =
        albert_storage::SqliteStore::new(args.database_url.unwrap_or_else(default_database_url));
    store.migrate().map_err(|error| error.to_string())?;
    let kind = args.kind;
    let example = MockExample {
        kind: kind.clone(),
        title: args.title.unwrap_or_else(|| match kind {
            MockExampleKind::Success => "Success".to_string(),
            MockExampleKind::Empty => "Empty".to_string(),
            MockExampleKind::Error => "Error".to_string(),
        }),
        payload: args.payload,
        note: args.note.or_else(|| Some("Hand-edited".to_string())),
    };
    store
        .replace_mock_example(&args.collection_id, &args.method, &args.path, &example)
        .map_err(|error| error.to_string())?;
    Ok(example)
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
