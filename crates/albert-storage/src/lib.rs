use std::time::{SystemTime, UNIX_EPOCH};

use albert_core::{
    CanonicalApiCollection, CapabilityStatus, DeliveryStage, MockExample, ProviderConfig,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub database_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCollectionSummary {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub endpoint_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEndpointSummary {
    pub id: String,
    pub collection_id: String,
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
}

/// Summary row returned from `list_scenarios`. The `payload` is not loaded
/// here — fetch it with `load_scenario` when the user activates one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredScenarioSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

impl SqliteStore {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        let connection = self.connect()?;
        connection.execute_batch(self.migration_sql())?;
        Ok(())
    }

    pub fn save_collection(&self, collection: &CanonicalApiCollection) -> Result<(), StorageError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT OR IGNORE INTO projects (id, name, created_at) VALUES (?1, ?2, ?3)",
            params!["default-project", "Default Project", unix_timestamp()],
        )?;

        transaction.execute(
            "INSERT OR REPLACE INTO api_collections (id, project_id, source_kind, name, raw_snapshot) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                collection.id,
                "default-project",
                collection.source.as_str(),
                collection.name,
                serde_json::to_string(collection)?
            ],
        )?;

        transaction.execute(
            "DELETE FROM api_schemas WHERE endpoint_id IN (SELECT id FROM api_endpoints WHERE collection_id = ?1)",
            params![collection.id],
        )?;
        transaction.execute(
            "DELETE FROM mock_examples WHERE endpoint_id IN (SELECT id FROM api_endpoints WHERE collection_id = ?1)",
            params![collection.id],
        )?;
        transaction.execute(
            "DELETE FROM api_endpoints WHERE collection_id = ?1",
            params![collection.id],
        )?;

        for endpoint in &collection.endpoints {
            let endpoint_id = endpoint_id(&collection.id, endpoint.method.as_str(), &endpoint.path);

            transaction.execute(
                "INSERT INTO api_endpoints (id, collection_id, method, path, summary) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    endpoint_id,
                    collection.id,
                    endpoint.method.as_str(),
                    endpoint.path,
                    endpoint.summary
                ],
            )?;

            if let Some(request_body) = &endpoint.request_body {
                transaction.execute(
                    "INSERT INTO api_schemas (id, endpoint_id, schema_role, payload) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        format!("{endpoint_id}:request"),
                        endpoint_id,
                        format!("request:{}", request_body.content_type),
                        serde_json::to_string(&request_body.schema)?
                    ],
                )?;
            }

            for response in &endpoint.responses {
                if let Some(schema) = &response.schema {
                    transaction.execute(
                        "INSERT INTO api_schemas (id, endpoint_id, schema_role, payload) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            format!("{endpoint_id}:response:{}", response.status_code),
                            endpoint_id,
                            format!("response:{}:{}", response.status_code, response.content_type),
                            serde_json::to_string(schema)?
                        ],
                    )?;
                }
            }

            for example in &endpoint.examples {
                save_mock_example(&transaction, &endpoint_id, example)?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn save_gateway_preferences(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), StorageError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT OR REPLACE INTO gateway_preferences (id, payload) VALUES (?1, ?2)",
            params!["singleton", serde_json::to_string(payload)?],
        )?;
        Ok(())
    }

    pub fn load_gateway_preferences(&self) -> Result<Option<serde_json::Value>, StorageError> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT payload FROM gateway_preferences WHERE id = ?1")?;
        let mut rows = statement.query(params!["singleton"])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let value = serde_json::from_str(&raw)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Upsert a named scenario. The `payload` should be a `GatewayConfigBundle`
    /// JSON value — this layer stays opaque to the shape so future bundle
    /// versions can land without a migration. The `created_at` is preserved
    /// on updates; only `updated_at` refreshes.
    pub fn save_scenario(
        &self,
        name: &str,
        payload: &serde_json::Value,
    ) -> Result<StoredScenarioSummary, StorageError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(StorageError::InvalidInput("scenario name cannot be empty"));
        }
        let now = unix_timestamp();
        let connection = self.connect()?;
        let existing: Option<(String, String)> = connection
            .query_row(
                "SELECT id, created_at FROM gateway_scenarios WHERE name = ?1",
                params![trimmed],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let (id, created_at) = existing
            .unwrap_or_else(|| (format!("scenario-{}-{}", now, slug(trimmed)), now.clone()));
        connection.execute(
            "INSERT OR REPLACE INTO gateway_scenarios (id, name, payload, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                trimmed,
                serde_json::to_string(payload)?,
                created_at,
                now
            ],
        )?;
        Ok(StoredScenarioSummary {
            id,
            name: trimmed.to_string(),
            created_at,
            updated_at: now,
        })
    }

    pub fn list_scenarios(&self) -> Result<Vec<StoredScenarioSummary>, StorageError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, name, created_at, updated_at \
             FROM gateway_scenarios \
             ORDER BY name ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(StoredScenarioSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn load_scenario(&self, name: &str) -> Result<Option<serde_json::Value>, StorageError> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT payload FROM gateway_scenarios WHERE name = ?1")?;
        let mut rows = statement.query(params![name])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            let value = serde_json::from_str(&raw)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn delete_scenario(&self, name: &str) -> Result<bool, StorageError> {
        let connection = self.connect()?;
        let affected = connection.execute(
            "DELETE FROM gateway_scenarios WHERE name = ?1",
            params![name],
        )?;
        Ok(affected > 0)
    }

    pub fn rename_scenario(&self, old_name: &str, new_name: &str) -> Result<bool, StorageError> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(StorageError::InvalidInput("scenario name cannot be empty"));
        }
        let connection = self.connect()?;
        let now = unix_timestamp();
        let affected = connection.execute(
            "UPDATE gateway_scenarios SET name = ?1, updated_at = ?2 WHERE name = ?3",
            params![trimmed, now, old_name],
        )?;
        Ok(affected > 0)
    }

    pub fn save_provider_config(&self, provider: &ProviderConfig) -> Result<(), StorageError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT OR REPLACE INTO provider_configs (id, provider_name, base_url, model, api_key_env) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                provider.provider_name,
                provider.provider_name,
                provider.base_url,
                provider.model,
                provider.api_key_env
            ],
        )?;
        Ok(())
    }

    pub fn list_collections(&self) -> Result<Vec<StoredCollectionSummary>, StorageError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT c.id, c.name, c.source_kind, COUNT(e.id)
             FROM api_collections c
             LEFT JOIN api_endpoints e ON e.collection_id = c.id
             GROUP BY c.id, c.name, c.source_kind
             ORDER BY c.name ASC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(StoredCollectionSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                source_kind: row.get(2)?,
                endpoint_count: row.get::<_, i64>(3)? as usize,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn load_collection(
        &self,
        collection_id: &str,
    ) -> Result<Option<CanonicalApiCollection>, StorageError> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT raw_snapshot FROM api_collections WHERE id = ?1")?;
        let mut rows = statement.query(params![collection_id])?;
        if let Some(row) = rows.next()? {
            let snapshot: String = row.get(0)?;
            let collection: CanonicalApiCollection = serde_json::from_str(&snapshot)?;
            Ok(Some(collection))
        } else {
            Ok(None)
        }
    }

    pub fn rename_collection(
        &self,
        collection_id: &str,
        new_name: &str,
    ) -> Result<bool, StorageError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;

        // Update both the top-level metadata row and the embedded snapshot
        // so `load_collection` stays consistent.
        let snapshot: Option<String> = transaction
            .query_row(
                "SELECT raw_snapshot FROM api_collections WHERE id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .ok();

        if snapshot.is_none() {
            return Ok(false);
        }

        let updated_snapshot = snapshot.and_then(|raw| {
            serde_json::from_str::<CanonicalApiCollection>(&raw)
                .ok()
                .map(|mut collection| {
                    collection.name = new_name.to_string();
                    serde_json::to_string(&collection).ok()
                })
                .and_then(|x| x)
        });

        let rows = transaction.execute(
            "UPDATE api_collections SET name = ?1, raw_snapshot = COALESCE(?2, raw_snapshot) WHERE id = ?3",
            params![new_name, updated_snapshot, collection_id],
        )?;
        transaction.commit()?;
        Ok(rows > 0)
    }

    pub fn delete_collection(&self, collection_id: &str) -> Result<bool, StorageError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM mock_examples WHERE endpoint_id IN (SELECT id FROM api_endpoints WHERE collection_id = ?1)",
            params![collection_id],
        )?;
        transaction.execute(
            "DELETE FROM api_schemas WHERE endpoint_id IN (SELECT id FROM api_endpoints WHERE collection_id = ?1)",
            params![collection_id],
        )?;
        transaction.execute(
            "DELETE FROM api_endpoints WHERE collection_id = ?1",
            params![collection_id],
        )?;
        let removed = transaction.execute(
            "DELETE FROM api_collections WHERE id = ?1",
            params![collection_id],
        )?;
        transaction.commit()?;
        Ok(removed > 0)
    }

    pub fn load_all_collections(&self) -> Result<Vec<CanonicalApiCollection>, StorageError> {
        let connection = self.connect()?;
        let mut statement =
            connection.prepare("SELECT raw_snapshot FROM api_collections ORDER BY name ASC")?;
        let rows = statement.query_map([], |row| {
            let snapshot: String = row.get(0)?;
            Ok(snapshot)
        })?;
        let mut collections = Vec::new();
        for row in rows {
            let snapshot = row?;
            let collection: CanonicalApiCollection = serde_json::from_str(&snapshot)?;
            collections.push(collection);
        }
        Ok(collections)
    }

    pub fn replace_mock_example(
        &self,
        collection_id: &str,
        method: &str,
        path: &str,
        example: &MockExample,
    ) -> Result<(), StorageError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let endpoint_id = endpoint_id(collection_id, method, path);
        transaction.execute(
            "DELETE FROM mock_examples WHERE endpoint_id = ?1 AND kind = ?2",
            params![endpoint_id, example.kind.as_str()],
        )?;
        save_mock_example(&transaction, &endpoint_id, example)?;

        let snapshot_raw: Option<String> = transaction
            .query_row(
                "SELECT raw_snapshot FROM api_collections WHERE id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(snapshot_raw) = snapshot_raw
            && let Ok(mut collection) =
                serde_json::from_str::<CanonicalApiCollection>(&snapshot_raw)
        {
            for endpoint in collection.endpoints.iter_mut() {
                if endpoint.method.as_str().eq_ignore_ascii_case(method) && endpoint.path == path {
                    if let Some(slot) = endpoint
                        .examples
                        .iter_mut()
                        .find(|candidate| candidate.kind == example.kind)
                    {
                        *slot = example.clone();
                    } else {
                        endpoint.examples.push(example.clone());
                    }
                }
            }
            transaction.execute(
                "UPDATE api_collections SET raw_snapshot = ?1 WHERE id = ?2",
                params![serde_json::to_string(&collection)?, collection_id],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn list_endpoints(
        &self,
        collection_id: &str,
    ) -> Result<Vec<StoredEndpointSummary>, StorageError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, collection_id, method, path, summary
             FROM api_endpoints
             WHERE collection_id = ?1
             ORDER BY method ASC, path ASC",
        )?;

        let rows = statement.query_map(params![collection_id], |row| {
            Ok(StoredEndpointSummary {
                id: row.get(0)?,
                collection_id: row.get(1)?,
                method: row.get(2)?,
                path: row.get(3)?,
                summary: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn migration_sql(&self) -> &'static str {
        include_str!("../migrations/0001_initial.sql")
    }

    fn connect(&self) -> Result<Connection, StorageError> {
        let connection = if self.database_url == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(&self.database_url)?
        };
        configure_connection(&connection)?;
        Ok(connection)
    }
}

/// Set connection-level pragmas that improve concurrent behavior:
///
/// - `journal_mode = WAL` lets readers proceed while a single writer commits,
///   which is the behavior we want for a desktop app that may hold a long
///   import transaction while the mock gateway polls for preferences.
/// - `busy_timeout = 5000` tells SQLite to retry internally for up to 5s
///   before surfacing `SQLITE_BUSY` to the caller, so transient writer
///   contention looks like "slow" rather than "failed".
/// - `synchronous = NORMAL` is the WAL-recommended default — retains crash
///   safety against power loss at transaction boundaries while skipping the
///   extra fsync on every journal page.
///
/// `:memory:` databases don't benefit from WAL but accept the pragma as a
/// no-op, so we apply it unconditionally.
fn configure_connection(connection: &Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "busy_timeout", 5000)?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn save_mock_example(
    transaction: &rusqlite::Transaction<'_>,
    endpoint_id: &str,
    example: &MockExample,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO mock_examples (id, endpoint_id, kind, title, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            format!("{endpoint_id}:example:{}", example.kind.as_str()),
            endpoint_id,
            example.kind.as_str(),
            example.title,
            serde_json::to_string(&example.payload)?
        ],
    )?;

    Ok(())
}

fn endpoint_id(collection_id: &str, method: &str, path: &str) -> String {
    format!(
        "{}:{}:{}",
        collection_id,
        method.to_ascii_lowercase(),
        path.replace('/', "_")
    )
}

fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
}

/// Produce a slug suitable for embedding in a scenario id: lowercase
/// ASCII alphanumerics and dashes, other characters become `-`. Collapses
/// runs of `-` and trims leading/trailing dashes so `id` stays
/// human-readable.
fn slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = true;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed
    }
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "projects table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Migrations and default project persistence are implemented.".to_string(),
        },
        CapabilityStatus {
            name: "api_collections table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Canonical collection metadata and snapshots are persisted.".to_string(),
        },
        CapabilityStatus {
            name: "api_endpoints table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Endpoint records can be inserted and listed back out of SQLite.".to_string(),
        },
        CapabilityStatus {
            name: "api_schemas table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Request and response schemas are stored as normalized JSON payloads."
                .to_string(),
        },
        CapabilityStatus {
            name: "mock_examples table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Default mock examples are persisted alongside each endpoint.".to_string(),
        },
        CapabilityStatus {
            name: "provider_configs table".to_string(),
            stage: DeliveryStage::Partial,
            note: "Provider configuration persistence is available for future runtime wiring."
                .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::SqliteStore;
    use albert_core::{
        CanonicalApiCollection, CanonicalEndpoint, CanonicalResponse, HttpMethod, InputSourceKind,
        ProviderConfig, SchemaNode, default_mock_examples,
    };

    #[test]
    fn migrates_expected_tables() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());

        store.migrate().unwrap();

        let connection = store.connect().unwrap();
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(rows.contains(&"projects".to_string()));
        assert!(rows.contains(&"api_collections".to_string()));
        assert!(rows.contains(&"api_endpoints".to_string()));
        assert!(rows.contains(&"api_schemas".to_string()));
        assert!(rows.contains(&"mock_examples".to_string()));
        assert!(rows.contains(&"provider_configs".to_string()));
        assert!(rows.contains(&"gateway_scenarios".to_string()));
    }

    #[test]
    fn lists_are_empty_after_fresh_migration() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());

        store.migrate().unwrap();

        assert!(store.list_collections().unwrap().is_empty());
        assert!(store.list_endpoints("missing").unwrap().is_empty());
    }

    #[test]
    fn saves_collection_and_lists_it_back() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let collection = sample_collection();
        store.save_collection(&collection).unwrap();

        let collections = store.list_collections().unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, "orders");
        assert_eq!(collections[0].endpoint_count, 1);

        let endpoints = store.list_endpoints("orders").unwrap();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].method, "GET");
        assert_eq!(endpoints[0].path, "/orders");

        let connection = store.connect().unwrap();
        let schema_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM api_schemas", [], |row| row.get(0))
            .unwrap();
        let example_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM mock_examples", [], |row| row.get(0))
            .unwrap();

        assert_eq!(schema_count, 2);
        assert_eq!(example_count, 3);
    }

    #[test]
    fn save_collection_is_idempotent_for_same_collection_id() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let collection = sample_collection();
        store.save_collection(&collection).unwrap();
        store.save_collection(&collection).unwrap();

        let connection = store.connect().unwrap();
        let endpoint_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM api_endpoints", [], |row| row.get(0))
            .unwrap();
        let example_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM mock_examples", [], |row| row.get(0))
            .unwrap();

        assert_eq!(endpoint_count, 1);
        assert_eq!(example_count, 3);
    }

    #[test]
    fn save_collection_replaces_existing_endpoint_set() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let mut collection = sample_collection();
        store.save_collection(&collection).unwrap();

        collection.endpoints = vec![
            CanonicalEndpoint {
                path: "/orders/{id}".to_string(),
                method: HttpMethod::Get,
                operation_id: Some("getOrder".to_string()),
                summary: Some("Get order".to_string()),
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
            },
            CanonicalEndpoint {
                path: "/orders".to_string(),
                method: HttpMethod::Post,
                operation_id: Some("createOrder".to_string()),
                summary: Some("Create order".to_string()),
                description: None,
                tags: vec!["orders".to_string()],
                parameters: Vec::new(),
                request_body: Some(albert_core::CanonicalRequestBody {
                    content_type: "application/json".to_string(),
                    required: true,
                    schema: SchemaNode::object(),
                }),
                responses: vec![CanonicalResponse {
                    status_code: "201".to_string(),
                    description: Some("Created".to_string()),
                    content_type: "application/json".to_string(),
                    schema: Some(SchemaNode::object()),
                }],
                examples: default_mock_examples(),
                auth: None,
            },
        ];

        store.save_collection(&collection).unwrap();

        let endpoints = store.list_endpoints("orders").unwrap();
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.iter().any(|endpoint| endpoint.method == "POST"));
        assert!(
            endpoints
                .iter()
                .any(|endpoint| endpoint.path == "/orders/{id}")
        );
    }

    #[test]
    fn rename_collection_updates_metadata_and_snapshot() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let collection = CanonicalApiCollection {
            id: "orders".to_string(),
            name: "Orders (old)".to_string(),
            source: InputSourceKind::OpenApi,
            description: None,
            endpoints: vec![],
        };
        store.save_collection(&collection).unwrap();

        let renamed = store.rename_collection("orders", "Orders v2").unwrap();
        assert!(renamed);

        let summary = &store.list_collections().unwrap()[0];
        assert_eq!(summary.name, "Orders v2");

        let snapshot = store.load_collection("orders").unwrap().unwrap();
        assert_eq!(snapshot.name, "Orders v2");

        // renaming a missing collection returns false
        assert!(!store.rename_collection("missing", "whatever").unwrap());
    }

    #[test]
    fn delete_collection_removes_all_related_rows() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let collection = CanonicalApiCollection {
            id: "orders".to_string(),
            name: "orders".to_string(),
            source: InputSourceKind::OpenApi,
            description: None,
            endpoints: vec![CanonicalEndpoint {
                operation_id: Some("list".to_string()),
                method: HttpMethod::Get,
                path: "/orders".to_string(),
                summary: None,
                description: None,
                tags: vec![],
                parameters: vec![],
                request_body: None,
                responses: vec![CanonicalResponse {
                    status_code: "200".to_string(),
                    description: None,
                    content_type: "application/json".to_string(),
                    schema: Some(SchemaNode::object()),
                }],
                examples: default_mock_examples(),
                auth: None,
            }],
        };
        store.save_collection(&collection).unwrap();
        assert_eq!(store.list_collections().unwrap().len(), 1);

        let removed = store.delete_collection("orders").unwrap();
        assert!(removed);
        assert!(store.list_collections().unwrap().is_empty());
        assert!(store.list_endpoints("orders").unwrap().is_empty());

        // Deleting again is a no-op
        let removed = store.delete_collection("orders").unwrap();
        assert!(!removed);
    }

    #[test]
    fn gateway_preferences_roundtrip() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        assert!(store.load_gateway_preferences().unwrap().is_none());

        let payload = serde_json::json!({
            "host": "127.0.0.1",
            "port": 4317,
            "cors_enabled": true,
            "default_latency_ms": 50,
            "error_rate": 0.1
        });
        store.save_gateway_preferences(&payload).unwrap();
        let loaded = store.load_gateway_preferences().unwrap().unwrap();
        assert_eq!(loaded, payload);

        // upsert behavior: second save replaces the value
        let next = serde_json::json!({"host": "0.0.0.0", "port": 0});
        store.save_gateway_preferences(&next).unwrap();
        assert_eq!(store.load_gateway_preferences().unwrap().unwrap(), next);

        // The slot is shape-agnostic: persisting the full runtime config
        // (rate_limits, required_headers, overrides) must survive a
        // round-trip without migration.
        let full = serde_json::json!({
            "host": "127.0.0.1",
            "port": 4317,
            "cors_enabled": true,
            "default_latency_ms": 25,
            "latency_overrides": { "GET /slow": 150 },
            "error_rate": 0.2,
            "capture_bodies": true,
            "response_headers": {
                "GET /users": { "x-custom": "hello" }
            },
            "required_headers": {
                "GET /secret": [
                    { "name": "Authorization", "value_prefix": "Bearer " }
                ]
            },
            "rate_limits": {
                "GET /ping": { "limit": 5, "window_ms": 1000 }
            },
            "example_overrides": { "GET /users": "error" }
        });
        store.save_gateway_preferences(&full).unwrap();
        assert_eq!(store.load_gateway_preferences().unwrap().unwrap(), full);
    }

    #[test]
    fn saves_provider_configuration() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        store
            .save_provider_config(&ProviderConfig {
                provider_name: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-4.1-mini".to_string(),
                api_key_env: "OPENAI_API_KEY".to_string(),
            })
            .unwrap();

        let connection = store.connect().unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM provider_configs", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(count, 1);
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
                request_body: Some(albert_core::CanonicalRequestBody {
                    content_type: "application/json".to_string(),
                    required: false,
                    schema: SchemaNode::object(),
                }),
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

    #[test]
    fn concurrent_readers_and_writer_dont_block_on_wal() {
        // With the default journal mode (DELETE/TRUNCATE) a reader can
        // stall a writer for a long transaction window; WAL mode lets the
        // two coexist. We can't easily simulate a long transaction, but we
        // can hammer the DB from multiple threads and assert every op
        // completes without surfacing SQLITE_BUSY.
        use std::sync::Arc;
        use std::thread;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_string_lossy().to_string();
        let store = SqliteStore::new(path.clone());
        store.migrate().unwrap();
        let collection = sample_collection();
        store.save_collection(&collection).unwrap();

        let store_arc = Arc::new(store);
        let mut handles = Vec::new();
        for worker in 0..4 {
            let s = Arc::clone(&store_arc);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    s.list_collections().expect("list should not block");
                    let prefs = serde_json::json!({"worker": worker});
                    s.save_gateway_preferences(&prefs)
                        .expect("write should retry under WAL, not fail");
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        // Sanity: the DB is still readable and the collection survived.
        let collections = store_arc.list_collections().unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].id, collection.id);
    }

    #[test]
    fn connect_sets_wal_journal_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_string_lossy().to_string();
        let store = SqliteStore::new(path);
        store.migrate().unwrap();
        let connection = store.connect().unwrap();
        let mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");
    }

    #[test]
    fn scenario_save_list_load_round_trip() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let payload = serde_json::json!({
            "version": 1,
            "collection_ids": ["orders"],
            "config": { "error_rate": 0.5 }
        });

        let summary = store.save_scenario("Broken Backend", &payload).unwrap();
        assert_eq!(summary.name, "Broken Backend");
        assert!(summary.id.starts_with("scenario-"));
        assert!(summary.id.contains("broken-backend"));

        let listed = store.list_scenarios().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Broken Backend");

        let loaded = store.load_scenario("Broken Backend").unwrap().unwrap();
        assert_eq!(loaded, payload);
    }

    #[test]
    fn scenario_save_with_same_name_updates_timestamps_but_not_id() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let first = store
            .save_scenario("happy path", &serde_json::json!({"v": 1}))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = store
            .save_scenario("happy path", &serde_json::json!({"v": 2}))
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.created_at, second.created_at);
        assert_ne!(first.updated_at, second.updated_at);

        let loaded = store.load_scenario("happy path").unwrap().unwrap();
        assert_eq!(loaded, serde_json::json!({"v": 2}));
    }

    #[test]
    fn scenario_rename_and_delete() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        store
            .save_scenario("draft", &serde_json::json!({"v": 1}))
            .unwrap();
        let renamed = store.rename_scenario("draft", "final").unwrap();
        assert!(renamed);
        assert!(store.load_scenario("draft").unwrap().is_none());
        assert!(store.load_scenario("final").unwrap().is_some());

        let deleted = store.delete_scenario("final").unwrap();
        assert!(deleted);
        assert!(store.list_scenarios().unwrap().is_empty());

        // Deleting a missing scenario returns false but doesn't error.
        assert!(!store.delete_scenario("missing").unwrap());
    }

    #[test]
    fn scenario_rejects_empty_name() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let err = store
            .save_scenario("   ", &serde_json::json!({}))
            .unwrap_err();
        assert!(matches!(err, super::StorageError::InvalidInput(_)));
    }

    #[test]
    fn scenario_trims_name_on_save_and_rename() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = SqliteStore::new(temp_file.path().to_string_lossy().to_string());
        store.migrate().unwrap();

        let summary = store
            .save_scenario("  spaced  ", &serde_json::json!({}))
            .unwrap();
        assert_eq!(summary.name, "spaced");

        let ok = store.rename_scenario("spaced", "  trimmed  ").unwrap();
        assert!(ok);
        assert!(store.load_scenario("trimmed").unwrap().is_some());
    }
}
