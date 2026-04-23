mod curl;
mod openapi;

pub use curl::CurlParser;
pub use openapi::OpenApiParser;

use albert_core::{CanonicalApiCollection, CapabilityStatus, DeliveryStage, InputSourceKind};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ParseSource {
    pub name: Option<String>,
    pub body: String,
}

pub trait ApiParser {
    fn kind(&self) -> InputSourceKind;
    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError>;
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unsupported input source")]
    UnsupportedInput,
    #[error("invalid source: {0}")]
    InvalidSource(String),
    #[error("parser not implemented: {0}")]
    NotImplemented(&'static str),
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "OpenAPI parser".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "OpenAPI parser trait implementation exists with a placeholder body.".to_string(),
        },
        CapabilityStatus {
            name: "cURL parser".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "cURL parser trait implementation exists with a placeholder body.".to_string(),
        },
        CapabilityStatus {
            name: "Canonical schema transform".to_string(),
            stage: DeliveryStage::Partial,
            note: "Shared output types are defined in albert-core.".to_string(),
        },
    ]
}

pub fn detect_parser(raw: &str) -> Result<InputSourceKind, ParseError> {
    let trimmed = raw.trim_start();

    if trimmed.starts_with("curl ") {
        return Ok(InputSourceKind::Curl);
    }

    if trimmed.contains("\"openapi\"") || trimmed.contains("openapi:") {
        return Ok(InputSourceKind::OpenApi);
    }

    Err(ParseError::UnsupportedInput)
}
