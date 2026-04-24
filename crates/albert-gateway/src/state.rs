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
    /// Correlation id set on the response `x-request-id` header. Honored
    /// from the client when provided; otherwise generated server-side so
    /// every entry can be cross-referenced with a single lookup.
    #[serde(default)]
    pub request_id: Option<String>,
}

pub(crate) const DEFAULT_REQUEST_LOG_CAPACITY: usize = 100;

pub(crate) type ResponseHeaderMap = BTreeMap<String, BTreeMap<String, String>>;

pub(crate) type RequiredHeaderMap = BTreeMap<String, Vec<crate::config::RequiredHeader>>;

pub(crate) type StatusOverrideMap = BTreeMap<String, u16>;

/// Per-route sliding-window counters. Keyed by `METHOD /path`; stores
/// `{rule, timestamps}` where `timestamps` are millisecond epochs of
/// recent requests within the window.
#[derive(Debug, Clone, Default)]
pub(crate) struct RateLimitState {
    pub rules: BTreeMap<String, crate::config::RateLimitRule>,
    pub history: BTreeMap<String, VecDeque<u128>>,
}

/// Verdict returned when attempting to admit a request under the limit.
pub(crate) enum RateVerdict {
    Allowed,
    Denied {
        limit: u32,
        window_ms: u64,
        retry_after_ms: u64,
    },
}

impl RateLimitState {
    pub fn admit(&mut self, route_key: &str, now_ms: u128) -> RateVerdict {
        let Some(rule) = self.rules.get(route_key).cloned() else {
            return RateVerdict::Allowed;
        };
        if rule.limit == 0 {
            // A zero-limit rule is an explicit "deny all" — useful for
            // simulating a maintenance window.
            return RateVerdict::Denied {
                limit: 0,
                window_ms: rule.window_ms,
                retry_after_ms: rule.window_ms.max(1),
            };
        }
        let window = rule.window_ms as u128;
        let cutoff = now_ms.saturating_sub(window);
        let deque = self.history.entry(route_key.to_string()).or_default();
        while deque.front().is_some_and(|t| *t < cutoff) {
            deque.pop_front();
        }
        if (deque.len() as u32) >= rule.limit {
            let oldest = deque.front().copied().unwrap_or(now_ms);
            let retry_after = window.saturating_sub(now_ms.saturating_sub(oldest));
            return RateVerdict::Denied {
                limit: rule.limit,
                window_ms: rule.window_ms,
                retry_after_ms: retry_after.min(u128::from(u64::MAX)) as u64,
            };
        }
        deque.push_back(now_ms);
        RateVerdict::Allowed
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub by_method: BTreeMap<String, u64>,
    pub by_status_class: BTreeMap<String, u64>,
    pub total_latency_ms: u64,
    pub max_latency_ms: u64,
    pub started_at_epoch_ms: i64,
    /// Per-route rollups keyed by `METHOD /path`. Absent for requests
    /// that did not match a registered route (404 / unsupported method).
    #[serde(default)]
    pub by_route: BTreeMap<String, RouteMetrics>,
}

/// Per-route rollup returned from `/__albert/metrics`. Latencies are
/// approximate percentiles over a bounded reservoir (200 samples) so the
/// snapshot stays cheap to compute and the memory footprint is
/// constant per route. `p50_ms` and `p95_ms` collapse to `last_latency_ms`
/// when fewer than two samples have been seen.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RouteMetrics {
    pub count: u64,
    pub total_latency_ms: u64,
    pub max_latency_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    /// Raw latency samples retained for percentile computation. Bounded
    /// at `ROUTE_LATENCY_RESERVOIR` entries to keep per-route memory
    /// bounded even under sustained traffic.
    #[serde(skip)]
    pub(crate) samples: VecDeque<u64>,
}

pub(crate) const ROUTE_LATENCY_RESERVOIR: usize = 200;

impl RouteMetrics {
    pub fn average_latency_ms(&self) -> u64 {
        self.total_latency_ms.checked_div(self.count).unwrap_or(0)
    }

    pub(crate) fn record(&mut self, latency_ms: u64) {
        self.count = self.count.saturating_add(1);
        self.total_latency_ms = self.total_latency_ms.saturating_add(latency_ms);
        if latency_ms > self.max_latency_ms {
            self.max_latency_ms = latency_ms;
        }
        if self.samples.len() >= ROUTE_LATENCY_RESERVOIR {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
        let mut sorted: Vec<u64> = self.samples.iter().copied().collect();
        sorted.sort_unstable();
        self.p50_ms = percentile(&sorted, 50);
        self.p95_ms = percentile(&sorted, 95);
    }
}

/// Nearest-rank percentile over a pre-sorted slice. Returns 0 for an
/// empty slice. `pct` is the percentile (1..=100).
fn percentile(sorted: &[u64], pct: u8) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let pct = pct.clamp(1, 100) as usize;
    // Nearest-rank: index = ceil(pct/100 * N) - 1.
    let idx = (pct * sorted.len()).div_ceil(100) - 1;
    sorted[idx.min(sorted.len() - 1)]
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
    pub(crate) required_headers: Arc<StdMutex<Arc<RequiredHeaderMap>>>,
    pub(crate) status_overrides: Arc<StdMutex<Arc<StatusOverrideMap>>>,
    pub(crate) rate_limits: Arc<StdMutex<RateLimitState>>,
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
        required_headers: Arc<RequiredHeaderMap>,
        status_overrides: Arc<StatusOverrideMap>,
        rate_limits: BTreeMap<String, crate::config::RateLimitRule>,
        started_at_epoch_ms: i64,
    ) -> Self {
        Self {
            table: Arc::new(StdMutex::new(table)),
            overrides: Arc::new(StdMutex::new(overrides)),
            latency: Arc::new(StdMutex::new(latency)),
            error_rate: Arc::new(StdMutex::new(error_rate.clamp(0.0, 1.0))),
            capture_bodies: Arc::new(StdMutex::new(capture_bodies)),
            response_headers: Arc::new(StdMutex::new(response_headers)),
            required_headers: Arc::new(StdMutex::new(required_headers)),
            status_overrides: Arc::new(StdMutex::new(status_overrides)),
            rate_limits: Arc::new(StdMutex::new(RateLimitState {
                rules: rate_limits,
                history: BTreeMap::new(),
            })),
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

    pub(crate) fn snapshot_required_headers(&self) -> Arc<RequiredHeaderMap> {
        self.required_headers
            .lock()
            .expect("required headers poisoned")
            .clone()
    }

    pub(crate) fn replace_required_headers(&self, next: Arc<RequiredHeaderMap>) {
        let mut slot = self
            .required_headers
            .lock()
            .expect("required headers poisoned");
        *slot = next;
    }

    pub(crate) fn snapshot_status_overrides(&self) -> Arc<StatusOverrideMap> {
        self.status_overrides
            .lock()
            .expect("status overrides poisoned")
            .clone()
    }

    pub(crate) fn replace_status_overrides(&self, next: Arc<StatusOverrideMap>) {
        let mut slot = self
            .status_overrides
            .lock()
            .expect("status overrides poisoned");
        *slot = next;
    }

    /// Replace the rate-limit ruleset. Keeps the existing per-route history so
    /// that rules tightened during a reconfigure keep applying against the
    /// same rolling window — otherwise a client could dodge a stricter rule
    /// by racing a config change.
    pub(crate) fn replace_rate_limits(&self, next: BTreeMap<String, crate::config::RateLimitRule>) {
        let mut slot = self.rate_limits.lock().expect("rate limits poisoned");
        let keep: std::collections::BTreeSet<String> = next.keys().cloned().collect();
        slot.rules = next;
        slot.history.retain(|route_key, _| keep.contains(route_key));
    }

    pub(crate) fn admit_request(&self, route_key: &str, now_ms: u128) -> RateVerdict {
        self.rate_limits
            .lock()
            .expect("rate limits poisoned")
            .admit(route_key, now_ms)
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
            // Per-route rollup. Only matched routes contribute — for 404 /
            // unsupported rows we leave the by_route map alone so it
            // stays a faithful picture of registered-route traffic.
            if let Some(key) = &entry.matched_route {
                let route_metrics = metrics.by_route.entry(key.clone()).or_default();
                route_metrics.record(entry.latency_ms);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_of_empty_slice_is_zero() {
        assert_eq!(percentile(&[], 50), 0);
        assert_eq!(percentile(&[], 95), 0);
    }

    #[test]
    fn percentile_picks_nearest_rank() {
        // [10, 20, 30, 40, 50] — p50 = 30 (3rd), p95 = 50 (5th).
        let samples = [10u64, 20, 30, 40, 50];
        assert_eq!(percentile(&samples, 50), 30);
        assert_eq!(percentile(&samples, 95), 50);
        assert_eq!(percentile(&samples, 100), 50);
        assert_eq!(percentile(&samples, 20), 10);
    }

    #[test]
    fn percentile_handles_single_sample() {
        assert_eq!(percentile(&[42], 50), 42);
        assert_eq!(percentile(&[42], 95), 42);
    }

    #[test]
    fn route_metrics_tracks_count_and_percentiles() {
        let mut rm = RouteMetrics::default();
        for ms in [10u64, 30, 50, 20, 40] {
            rm.record(ms);
        }
        assert_eq!(rm.count, 5);
        assert_eq!(rm.total_latency_ms, 150);
        assert_eq!(rm.average_latency_ms(), 30);
        assert_eq!(rm.max_latency_ms, 50);
        // Sorted [10, 20, 30, 40, 50] → p50=30, p95=50.
        assert_eq!(rm.p50_ms, 30);
        assert_eq!(rm.p95_ms, 50);
    }

    #[test]
    fn route_metrics_samples_are_bounded() {
        let mut rm = RouteMetrics::default();
        // Record 2x the reservoir to prove we don't grow without bound.
        for i in 0..(ROUTE_LATENCY_RESERVOIR * 2) as u64 {
            rm.record(i);
        }
        assert_eq!(rm.count, (ROUTE_LATENCY_RESERVOIR * 2) as u64);
        assert_eq!(rm.samples.len(), ROUTE_LATENCY_RESERVOIR);
        // The oldest half should have aged out; the samples we still hold
        // should all come from the second half of the stream.
        let smallest = rm.samples.iter().copied().min().unwrap();
        assert!(
            smallest >= ROUTE_LATENCY_RESERVOIR as u64,
            "oldest samples not evicted: min={smallest}"
        );
    }
}
