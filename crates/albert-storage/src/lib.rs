use albert_core::{CanonicalApiCollection, CapabilityStatus, DeliveryStage};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub database_url: String,
}

impl SqliteStore {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "SQLite migration execution lands in Phase 2",
        ))
    }

    pub fn save_collection(
        &self,
        _collection: &CanonicalApiCollection,
    ) -> Result<(), StorageError> {
        Err(StorageError::NotImplemented(
            "collection persistence lands in Phase 2",
        ))
    }

    pub fn migration_sql(&self) -> &'static str {
        include_str!("../migrations/0001_initial.sql")
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage not implemented: {0}")]
    NotImplemented(&'static str),
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "projects table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Logical storage contract reserved in the initial migration.".to_string(),
        },
        CapabilityStatus {
            name: "api_collections table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Collection metadata contract reserved in the initial migration.".to_string(),
        },
        CapabilityStatus {
            name: "api_endpoints table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Endpoint metadata contract reserved in the initial migration.".to_string(),
        },
        CapabilityStatus {
            name: "api_schemas table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Canonical schema payload contract reserved in the initial migration."
                .to_string(),
        },
        CapabilityStatus {
            name: "mock_examples table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Static mock states are modeled as persistent examples.".to_string(),
        },
        CapabilityStatus {
            name: "provider_configs table".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Provider settings contract reserved for OpenAI configuration.".to_string(),
        },
    ]
}
