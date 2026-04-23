//! File ingestion helpers: read, detect format (including bundles), parse,
//! persist.

use std::path::Path;

use albert_core::CanonicalApiCollection;
use albert_parser::{ParseSource, parse_source, try_parse_bundle};
use albert_storage::SqliteStore;

pub struct Ingested {
    pub collections: Vec<CanonicalApiCollection>,
    pub kind: IngestKind,
}

#[derive(Debug, Clone, Copy)]
pub enum IngestKind {
    Single,
    Bundle,
}

/// Read a file from disk, parse it (as a single collection or as a
/// `[...]` bundle of pre-canonicalized snapshots), and persist every entry
/// into the store.
pub fn ingest_file(path: &Path, store: &SqliteStore) -> Result<Ingested, IngestError> {
    let body = std::fs::read_to_string(path).map_err(|source| IngestError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    if let Some(collections) = try_parse_bundle(&body).map_err(IngestError::Parse)? {
        for collection in &collections {
            store
                .save_collection(collection)
                .map_err(IngestError::Store)?;
        }
        return Ok(Ingested {
            collections,
            kind: IngestKind::Bundle,
        });
    }

    let collection = parse_source(ParseSource { name, body }).map_err(IngestError::Parse)?;
    store
        .save_collection(&collection)
        .map_err(IngestError::Store)?;
    Ok(Ingested {
        collections: vec![collection],
        kind: IngestKind::Single,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error: {0}")]
    Parse(albert_parser::ParseError),
    #[error("storage error: {0}")]
    Store(albert_storage::StorageError),
}
