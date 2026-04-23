//! Shared axum handler state + request log capture.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};

use albert_core::MockExampleKind;
use serde::{Deserialize, Serialize};

use crate::routing::RouteTable;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestLogEntry {
    pub at_epoch_ms: i64,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub matched_route: Option<String>,
    pub collection_name: Option<String>,
    pub status: u16,
    pub kind: Option<MockExampleKind>,
    pub source: &'static str,
    pub latency_ms: u64,
    /// Truncated request body (UTF-8 best-effort) when capture was enabled.
    #[serde(default)]
    pub request_body: Option<String>,
}

pub(crate) const DEFAULT_REQUEST_LOG_CAPACITY: usize = 100;

pub(crate) type ResponseHeaderMap = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub by_method: BTreeMap<String, u64>,
    pub by_status_class: BTreeMap<String, u64>,
    pub total_latency_ms: u64,
    pub max_latency_ms: u64,
    pub started_at_epoch_ms: i64,
}

impl MetricsSnapshot {
    pub fn average_latency_ms(&self) -> u64 {
        self.total_latency_ms
            .checked_div(self.total_requests)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) table: Arc<StdMutex<Arc<RouteTable>>>,
    pub(crate) overrides: Arc<StdMutex<Arc<BTreeMap<String, MockExampleKind>>>>,
    pub(crate) latency: Arc<StdMutex<LatencyConfig>>,
    pub(crate) error_rate: Arc<StdMutex<f32>>,
    pub(crate) capture_bodies: Arc<StdMutex<bool>>,
    pub(crate) response_headers: Arc<StdMutex<Arc<ResponseHeaderMap>>>,
    pub(crate) metrics: Arc<StdMutex<MetricsSnapshot>>,
    pub(crate) request_log: Arc<StdMutex<VecDeque<RequestLogEntry>>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LatencyConfig {
    pub default_ms: Option<u64>,
    pub per_route: BTreeMap<String, u64>,
}

impl LatencyConfig {
    pub fn new(default_ms: Option<u64>, per_route: BTreeMap<String, u64>) -> Self {
        Self {
            default_ms,
            per_route,
        }
    }

    pub fn resolve(&self, route_key: &str) -> u64 {
        let base = self.default_ms.unwrap_or(0);
        let per = self.per_route.get(route_key).copied().unwrap_or(0);
        base.saturating_add(per)
    }
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        table: Arc<RouteTable>,
        overrides: Arc<BTreeMap<String, MockExampleKind>>,
        latency: LatencyConfig,
        error_rate: f32,
        capture_bodies: bool,
        response_headers: Arc<ResponseHeaderMap>,
        started_at_epoch_ms: i64,
    ) -> Self {
        Self {
            table: Arc::new(StdMutex::new(table)),
            overrides: Arc::new(StdMutex::new(overrides)),
            latency: Arc::new(StdMutex::new(latency)),
            error_rate: Arc::new(StdMutex::new(error_rate.clamp(0.0, 1.0))),
            capture_bodies: Arc::new(StdMutex::new(capture_bodies)),
            response_headers: Arc::new(StdMutex::new(response_headers)),
            metrics: Arc::new(StdMutex::new(MetricsSnapshot {
                started_at_epoch_ms,
                ..Default::default()
            })),
            request_log: Arc::new(StdMutex::new(VecDeque::with_capacity(
                DEFAULT_REQUEST_LOG_CAPACITY,
            ))),
        }
    }

    pub(crate) fn snapshot_table(&self) -> Arc<RouteTable> {
        self.table.lock().expect("route table poisoned").clone()
    }

    pub(crate) fn snapshot_overrides(&self) -> Arc<BTreeMap<String, MockExampleKind>> {
        self.overrides.lock().expect("overrides poisoned").clone()
    }

    pub(crate) fn snapshot_latency(&self) -> LatencyConfig {
        self.latency.lock().expect("latency poisoned").clone()
    }

    pub(crate) fn snapshot_error_rate(&self) -> f32 {
        *self.error_rate.lock().expect("error rate poisoned")
    }

    pub(crate) fn replace_table(&self, next: Arc<RouteTable>) {
        let mut slot = self.table.lock().expect("route table poisoned");
        *slot = next;
    }

    pub(crate) fn replace_overrides(&self, next: Arc<BTreeMap<String, MockExampleKind>>) {
        let mut slot = self.overrides.lock().expect("overrides poisoned");
        *slot = next;
    }

    pub(crate) fn replace_latency(&self, next: LatencyConfig) {
        let mut slot = self.latency.lock().expect("latency poisoned");
        *slot = next;
    }

    pub(crate) fn replace_error_rate(&self, next: f32) {
        let mut slot = self.error_rate.lock().expect("error rate poisoned");
        *slot = next.clamp(0.0, 1.0);
    }

    pub(crate) fn snapshot_capture_bodies(&self) -> bool {
        *self.capture_bodies.lock().expect("capture flag poisoned")
    }

    pub(crate) fn replace_capture_bodies(&self, next: bool) {
        let mut slot = self.capture_bodies.lock().expect("capture flag poisoned");
        *slot = next;
    }

    pub(crate) fn snapshot_response_headers(&self) -> Arc<ResponseHeaderMap> {
        self.response_headers
            .lock()
            .expect("response headers poisoned")
            .clone()
    }

    pub(crate) fn replace_response_headers(&self, next: Arc<ResponseHeaderMap>) {
        let mut slot = self
            .response_headers
            .lock()
            .expect("response headers poisoned");
        *slot = next;
    }

    pub(crate) fn record(&self, entry: RequestLogEntry) {
        {
            let mut metrics = self.metrics.lock().expect("metrics poisoned");
            metrics.total_requests += 1;
            *metrics.by_method.entry(entry.method.clone()).or_insert(0) += 1;
            let class = match entry.status {
                100..=199 => "1xx",
                200..=299 => "2xx",
                300..=399 => "3xx",
                400..=499 => "4xx",
                500..=599 => "5xx",
                _ => "other",
            };
            *metrics
                .by_status_class
                .entry(class.to_string())
                .or_insert(0) += 1;
            metrics.total_latency_ms += entry.latency_ms;
            if entry.latency_ms > metrics.max_latency_ms {
                metrics.max_latency_ms = entry.latency_ms;
            }
        }
        let mut log = self.request_log.lock().expect("log poisoned");
        if log.len() >= DEFAULT_REQUEST_LOG_CAPACITY {
            log.pop_front();
        }
        log.push_back(entry);
    }

    pub(crate) fn snapshot_metrics(&self) -> MetricsSnapshot {
        self.metrics.lock().expect("metrics poisoned").clone()
    }
}
