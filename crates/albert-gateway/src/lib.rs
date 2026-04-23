use albert_core::{
    CapabilityStatus, DeliveryStage, MockExampleKind, MockHttpRequest, MockHttpResponse,
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct MockGateway {
    pub bind_addr: String,
}

impl MockGateway {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
        }
    }

    pub fn handle_request(
        &self,
        _request: MockHttpRequest,
    ) -> Result<MockHttpResponse, GatewayError> {
        Err(GatewayError::NotImplemented(
            "HTTP runtime and route matching land in Phase 3",
        ))
    }
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("gateway not implemented: {0}")]
    NotImplemented(&'static str),
}

pub fn supported_example_kinds() -> Vec<MockExampleKind> {
    vec![
        MockExampleKind::Success,
        MockExampleKind::Empty,
        MockExampleKind::Error,
    ]
}

pub fn planned_capabilities() -> Vec<CapabilityStatus> {
    vec![
        CapabilityStatus {
            name: "Static mock states".to_string(),
            stage: DeliveryStage::Scaffolded,
            note: "Success, empty, and error examples are modeled in albert-core.".to_string(),
        },
        CapabilityStatus {
            name: "Route matching".to_string(),
            stage: DeliveryStage::Planned,
            note: "REST-style path resolution will be added when the listener exists.".to_string(),
        },
        CapabilityStatus {
            name: "HTTP listener".to_string(),
            stage: DeliveryStage::NotImplemented,
            note: "Rust async server runtime is deferred to Phase 3.".to_string(),
        },
    ]
}
