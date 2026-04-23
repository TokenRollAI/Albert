//! Gateway configuration + status types.

use std::collections::BTreeMap;

use albert_core::{CapabilityStatus, DeliveryStage, HttpMethod, MockExampleKind};
use serde::{Deserialize, Serialize};

/// Configuration for a running mock gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayConfig {
    /// Host binding, e.g. "127.0.0.1".
    pub host: String,
    /// Port number. Use 0 for ephemeral.
    pub port: u16,
    /// Enables permissive CORS so that browser clients can hit the mock.
    pub cors_enabled: bool,
    /// Per-endpoint example overrides, keyed by `METHOD path`.
    #[serde(default)]
    pub example_overrides: BTreeMap<String, MockExampleKind>,
    /// Optional global latency floor applied to every served request.
    #[serde(default)]
    pub default_latency_ms: Option<u64>,
    /// Per-route latency overrides, keyed by `METHOD path`.
    /// Applied on top of `default_latency_ms`.
    #[serde(default)]
    pub latency_overrides: BTreeMap<String, u64>,
    /// Probability (0.0–1.0) of serving the error example instead of the
    /// selected one, for chaos-style testing of consumer error paths.
    #[serde(default)]
    pub error_rate: f32,
    /// Capture the first ≤4KB of each request body into the request log.
    /// Disabled by default to avoid leaking sensitive payloads; opt in from
    /// the UI or CLI.
    #[serde(default)]
    pub capture_bodies: bool,
    /// Per-route extra response headers keyed by `METHOD path`. Each
    /// inner map's keys are header names, values are the verbatim header
    /// value string. Merged on top of the content-type + observability
    /// headers the gateway writes natively. Unknown route keys are
    /// silently ignored.
    #[serde(default)]
    pub response_headers: BTreeMap<String, BTreeMap<String, String>>,
    /// Per-route required-header gates keyed by `METHOD path`. If any
    /// listed rule is not satisfied the gateway returns `401 Unauthorized`
    /// with a structured JSON body describing which rule failed. Use the
    /// empty `value_prefix`/`value_equals` to require presence only.
    #[serde(default)]
    pub required_headers: BTreeMap<String, Vec<RequiredHeader>>,
}

/// A single header-presence or header-value requirement. `name` is the
/// header name (case-insensitive on the wire). `value_prefix` and
/// `value_equals` are mutually compatible — if both are set, both must
/// hold. When neither is set, only presence is checked.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequiredHeader {
    pub name: String,
    #[serde(default)]
    pub value_prefix: Option<String>,
    #[serde(default)]
    pub value_equals: Option<String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4317,
            cors_enabled: true,
            example_overrides: BTreeMap::new(),
            default_latency_ms: None,
            latency_overrides: BTreeMap::new(),
            error_rate: 0.0,
            capture_bodies: false,
            response_headers: BTreeMap::new(),
            required_headers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayStatus {
    pub running: bool,
    pub bind_address: Option<String>,
    pub route_count: usize,
    pub started_at_epoch_ms: Option<i64>,
    pub config: GatewayConfig,
    pub routes: Vec<GatewayRouteSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayRouteSummary {
    pub method: HttpMethod,
    pub path: String,
    pub collection_name: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub selected_example: Option<MockExampleKind>,
    pub available_examples: Vec<MockExampleKind>,
    pub latency_ms: Option<u64>,
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
            stage: DeliveryStage::Partial,
            note: "Success, empty, and error examples are selected per request via override or query param.".to_string(),
        },
        CapabilityStatus {
            name: "Route matching".to_string(),
            stage: DeliveryStage::Partial,
            note: "Matches by HTTP method and path template with `{param}` placeholders.".to_string(),
        },
        CapabilityStatus {
            name: "HTTP listener".to_string(),
            stage: DeliveryStage::Partial,
            note: "Axum + hyper server with graceful shutdown, permissive CORS, and latency injection.".to_string(),
        },
    ]
}
