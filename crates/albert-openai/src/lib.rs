use albert_core::{
    CanonicalEndpoint, CapabilityStatus, DeliveryStage, MockExample, ProviderConfig,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct OpenAiChatAdapter {
    pub config: ProviderConfig,
}

impl OpenAiChatAdapter {
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
    }

    pub fn generate_mock_example(
        &self,
        _endpoint: &CanonicalEndpoint,
    ) -> Result<MockExample, OpenAiError> {
        Err(OpenAiError::NotImplemented(
            "OpenAI request execution lands after foundation setup",
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub raw_text: String,
}

#[derive(Debug, Error)]
pub enum OpenAiError {
    #[error("provider not implemented: {0}")]
    NotImplemented(&'static str),
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "OpenAI Chat Completions adapter".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Provider boundary is defined with request and response envelopes.".to_string(),
        },
        CapabilityStatus {
            name: "Responses API adapter".to_string(),
            stage: DeliveryStage::Planned,
            note: "Reserved in docs, intentionally omitted from runtime wiring.".to_string(),
        },
        CapabilityStatus {
            name: "Structured output enforcement".to_string(),
            stage: DeliveryStage::Planned,
            note: "Validation and repair loop should land with real provider execution."
                .to_string(),
        },
    ]
}
