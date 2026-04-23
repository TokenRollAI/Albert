use albert_core::{CanonicalApiCollection, InputSourceKind};

use crate::{ApiParser, ParseError, ParseSource};

#[derive(Debug, Default)]
pub struct CurlParser;

impl ApiParser for CurlParser {
    fn kind(&self) -> InputSourceKind {
        InputSourceKind::Curl
    }

    fn parse(&self, source: ParseSource) -> Result<CanonicalApiCollection, ParseError> {
        if !source.body.trim_start().starts_with("curl ") {
            return Err(ParseError::InvalidSource(
                "cURL input should begin with the curl command".to_string(),
            ));
        }

        Err(ParseError::NotImplemented(
            "cURL tokenization and canonical mapping land in Phase 2",
        ))
    }
}
