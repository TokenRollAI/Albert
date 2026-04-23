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
}

pub(crate) const DEFAULT_REQUEST_LOG_CAPACITY: usize = 100;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) table: Arc<StdMutex<Arc<RouteTable>>>,
    pub(crate) overrides: Arc<StdMutex<Arc<BTreeMap<String, MockExampleKind>>>>,
    pub(crate) latency: Arc<StdMutex<LatencyConfig>>,
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
    pub(crate) fn new(
        table: Arc<RouteTable>,
        overrides: Arc<BTreeMap<String, MockExampleKind>>,
        latency: LatencyConfig,
    ) -> Self {
        Self {
            table: Arc::new(StdMutex::new(table)),
            overrides: Arc::new(StdMutex::new(overrides)),
            latency: Arc::new(StdMutex::new(latency)),
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

    pub(crate) fn record(&self, entry: RequestLogEntry) {
        let mut log = self.request_log.lock().expect("log poisoned");
        if log.len() >= DEFAULT_REQUEST_LOG_CAPACITY {
            log.pop_front();
        }
        log.push_back(entry);
    }
}
