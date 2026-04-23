//! File ingestion helpers: read, detect format, parse, persist.

use std::path::Path;

use albert_core::CanonicalApiCollection;
use albert_parser::{ParseSource, parse_source};
use albert_storage::SqliteStore;

/// Read a file from disk, parse it, and persist it into the given store.
/// Uses the filename (minus extension) as the collection name when the
/// caller has no better hint.
pub fn ingest_file(
    path: &Path,
    store: &SqliteStore,
) -> Result<CanonicalApiCollection, IngestError> {
    let body = std::fs::read_to_string(path).map_err(|source| IngestError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    let collection = parse_source(ParseSource { name, body }).map_err(IngestError::Parse)?;
    store
        .save_collection(&collection)
        .map_err(IngestError::Store)?;
    Ok(collection)
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
