use std::time::{SystemTime, UNIX_EPOCH};

use albert_core::{
    CanonicalApiCollection, CapabilityStatus, DeliveryStage, MockExample, ProviderConfig,
};
use rusqlite::{Connection, params};
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
        if self.database_url == ":memory:" {
            return Connection::open_in_memory().map_err(StorageError::from);
        }

        Connection::open(&self.database_url).map_err(StorageError::from)
    }
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
            }],
        }
    }
}
