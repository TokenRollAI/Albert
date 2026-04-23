//! Prompt construction for the OpenAI adapter.

use albert_core::{CanonicalEndpoint, MockExampleKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{parameter_hints, response_hints, schema_hint};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationIntent {
    Success,
    Empty,
    Error,
}

impl GenerationIntent {
    pub fn kind(&self) -> MockExampleKind {
        match self {
            GenerationIntent::Success => MockExampleKind::Success,
            GenerationIntent::Empty => MockExampleKind::Empty,
            GenerationIntent::Error => MockExampleKind::Error,
        }
    }

    pub fn title(&self) -> String {
        match self {
            GenerationIntent::Success => "AI Success".to_string(),
            GenerationIntent::Empty => "AI Empty".to_string(),
            GenerationIntent::Error => "AI Error".to_string(),
        }
    }

    pub fn intent_label(&self) -> &'static str {
        match self {
            GenerationIntent::Success => "success",
            GenerationIntent::Empty => "empty",
            GenerationIntent::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptBundle {
    pub system: String,
    pub user: String,
    pub endpoint_context: Value,
}

pub fn build_prompt_bundle(endpoint: &CanonicalEndpoint, intent: GenerationIntent) -> PromptBundle {
    let system = String::from(
        "You are Albert, an API mock data generator. Produce a single JSON object \
         that is a realistic mock response body for the described endpoint. \
         Respond with JSON only, no markdown fences, no commentary. Use diverse \
         but plausible values and respect the field types.",
    );

    let context = json!({
        "operation_id": endpoint.operation_id,
        "method": endpoint.method.as_str(),
        "path": endpoint.path,
        "summary": endpoint.summary,
        "description": endpoint.description,
        "tags": endpoint.tags,
        "parameters": parameter_hints(&endpoint.parameters),
        "request_body": endpoint.request_body.as_ref().map(|body| {
            json!({
                "content_type": body.content_type,
                "required": body.required,
                "schema": schema_hint(&body.schema),
            })
        }),
        "responses": response_hints(&endpoint.responses),
    });

    let instruction = match intent {
        GenerationIntent::Success => {
            "Return a representative success response payload. Include realistic IDs, \
             timestamps (ISO-8601), and populated collections where applicable."
        }
        GenerationIntent::Empty => {
            "Return a minimal empty-state response payload. Collections should be empty arrays, \
             counters should be zero, but the shape must still match the schema."
        }
        GenerationIntent::Error => {
            "Return a realistic validation-error response payload. Include an error code, a \
             human readable message, and any 'errors' list the schema implies."
        }
    };

    let user = format!(
        "Endpoint context (JSON):\n{context}\n\nMock type: {intent}\nInstruction: {instruction}\n\n\
         Respond with a single JSON object only.",
        context = serde_json::to_string_pretty(&context).unwrap_or_else(|_| "{}".to_string()),
        intent = intent.intent_label(),
        instruction = instruction,
    );

    PromptBundle {
        system,
        user,
        endpoint_context: context,
    }
}
