use albert_core::{CanonicalApiCollection, InputSourceKind};

use crate::{ApiParser, ParseError, ParseSource};

#[derive(Debug, Default)]
pub struct OpenApiParser;

impl ApiParser for OpenApiParser {
    fn kind(&self) -> InputSourceKind {
        InputSourceKind::OpenApi
    }

    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError> {
        if source.body.trim().is_empty() {
            return Err(ParseError::InvalidSource(
                "OpenAPI content cannot be empty".to_string(),
            ));
        }

        Err(ParseError::NotImplemented(
            "OpenAPI normalization lands in Phase 2",
        ))
    }
}
