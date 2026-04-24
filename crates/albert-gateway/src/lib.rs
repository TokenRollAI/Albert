//! Local mock HTTP gateway.
//!
//! Phase 3 runtime: serves canonical endpoints over an axum-based HTTP server,
//! selecting from the per-endpoint `success / empty / error` mock examples,
//! with hot reload, request log capture, and optional latency injection.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use albert_core::{CanonicalApiCollection, MockExampleKind};
use axum::{Router, routing::any};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;

pub mod config;
pub mod error;
pub mod handlers;
pub mod openapi;
pub mod route;
pub mod routing;
pub mod state;
pub mod templating;
pub mod validator;

pub use config::{
    GatewayConfig, GatewayConfigBundle, GatewayRouteSummary, GatewayStatus, RateLimitRule,
    RequiredHeader, planned_capabilities, supported_example_kinds,
};
pub use error::GatewayError;
pub use route::{MatchedRoute, MockRoute, build_routes, route_key};
pub use routing::{CompiledRoute, RouteTable};
pub use state::{MetricsSnapshot, MinuteBucket, RequestLogEntry, RouteMetrics};

use state::{AppState, LatencyConfig};

/// Bag-of-fields passed to `MockGateway::reconfigure`. Grouping the
/// previously-eleven positional parameters into one struct means each
/// new enforcement knob can be added without touching every call site
/// (tests, CLI, Tauri update handler), and call sites read like the
/// field names on `GatewayConfig`. `Default` zeroes every map and
/// falls back to `error_rate: 0`, `capture_bodies: false`, matching
/// `GatewayConfig::default` so the struct can be built incrementally.
#[derive(Debug, Clone, Default)]
pub struct ReconfigureOptions {
    pub collections: Vec<CanonicalApiCollection>,
    pub overrides: BTreeMap<String, MockExampleKind>,
    pub default_latency_ms: Option<u64>,
    pub latency_overrides: BTreeMap<String, u64>,
    pub latency_jitter_ms: BTreeMap<String, u64>,
    pub error_rate: f32,
    pub capture_bodies: bool,
    pub enforce_request_bodies: bool,
    pub response_headers: BTreeMap<String, BTreeMap<String, String>>,
    pub required_headers: BTreeMap<String, Vec<RequiredHeader>>,
    pub rate_limits: BTreeMap<String, RateLimitRule>,
    pub status_overrides: BTreeMap<String, u16>,
}

/// A running or idle mock gateway.
pub struct MockGateway {
    inner: Mutex<Option<RunningGateway>>,
}

impl MockGateway {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn start(
        &self,
        collections: Vec<CanonicalApiCollection>,
        config: GatewayConfig,
    ) -> Result<GatewayStatus, GatewayError> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Err(GatewayError::AlreadyRunning);
        }

        let host: IpAddr = config
            .host
            .parse()
            .map_err(|_| GatewayError::InvalidConfig(format!("invalid host '{}'", config.host)))?;
        let addr = SocketAddr::new(host, config.port);
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|source| GatewayError::Bind {
                addr: addr.to_string(),
                source,
            })?;
        let bind_address = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| addr.to_string());

        let routes = build_routes(&collections);
        let summaries = summarize(
            &routes,
            &config.example_overrides,
            &config.latency_overrides,
        );
        let table_arc = Arc::new(RouteTable::from_routes(routes));
        let overrides_arc = Arc::new(config.example_overrides.clone());
        let latency = LatencyConfig::new(
            config.default_latency_ms,
            config.latency_overrides.clone(),
            config.latency_jitter_ms.clone(),
        );

        let response_headers = Arc::new(config.response_headers.clone());
        let required_headers = Arc::new(config.required_headers.clone());
        let status_overrides = Arc::new(config.status_overrides.clone());
        let started_at = handlers::epoch_ms_now();
        let collections_arc = Arc::new(collections.clone());
        let state = AppState::new(
            table_arc.clone(),
            overrides_arc.clone(),
            latency,
            config.error_rate,
            config.capture_bodies,
            config.enforce_request_bodies,
            response_headers,
            required_headers,
            status_overrides,
            config.rate_limits.clone(),
            collections_arc,
            started_at,
        );
        let state_for_runtime = state.clone();

        let mut router = Router::new()
            .route(
                "/__albert/status",
                axum::routing::get(handlers::status_handler),
            )
            .route(
                "/__albert/metrics",
                axum::routing::get(handlers::metrics_handler),
            )
            .route(
                "/__albert/routes",
                axum::routing::get(handlers::routes_handler),
            )
            .route(
                "/__albert/config",
                axum::routing::get(handlers::config_handler),
            )
            .route(
                "/__albert/openapi.json",
                axum::routing::get(handlers::openapi_handler),
            )
            .route(
                "/__albert/docs",
                axum::routing::get(handlers::docs_handler),
            )
            .route(
                "/__albert/config/bundle",
                axum::routing::get(handlers::bundle_export_handler)
                    .post(handlers::bundle_import_handler),
            )
            .fallback(any(handlers::mock_handler))
            .with_state(state);
        if config.cors_enabled {
            router = router.layer(CorsLayer::permissive());
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let serve = axum::serve(listener, router.into_make_service());
            let _ = serve
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        let running = RunningGateway {
            bind_address: bind_address.clone(),
            shutdown: Some(shutdown_tx),
            handle: server,
            started_at,
            config: config.clone(),
            route_summaries: summaries.clone(),
            state: state_for_runtime,
        };

        let status = running.to_status();
        *guard = Some(running);
        Ok(status)
    }

    /// Swap the collections + overrides served by a running gateway without
    /// restarting. Returns the updated status.
    pub async fn update(
        &self,
        collections: Vec<CanonicalApiCollection>,
        overrides: BTreeMap<String, MockExampleKind>,
    ) -> Result<GatewayStatus, GatewayError> {
        let config = self.current_config().await;
        self.reconfigure(ReconfigureOptions {
            collections,
            overrides,
            default_latency_ms: config.default_latency_ms,
            latency_overrides: config.latency_overrides,
            latency_jitter_ms: config.latency_jitter_ms,
            error_rate: config.error_rate,
            capture_bodies: config.capture_bodies,
            enforce_request_bodies: config.enforce_request_bodies,
            response_headers: config.response_headers,
            required_headers: config.required_headers,
            rate_limits: config.rate_limits,
            status_overrides: config.status_overrides,
        })
        .await
    }

    /// Full reconfigure entry point; can change overrides, latency, error
    /// rate, body-capture flag, per-route response headers, per-route
    /// required-header gates, rate limits, and status overrides in one
    /// atomic swap. Fields added in future should be appended to
    /// `ReconfigureOptions` so call sites only name what they want to
    /// change — no more eleven-argument positional drift.
    pub async fn reconfigure(
        &self,
        opts: ReconfigureOptions,
    ) -> Result<GatewayStatus, GatewayError> {
        let ReconfigureOptions {
            collections,
            overrides,
            default_latency_ms,
            latency_overrides,
            latency_jitter_ms,
            error_rate,
            capture_bodies,
            enforce_request_bodies,
            response_headers,
            required_headers,
            rate_limits,
            status_overrides,
        } = opts;
        let mut guard = self.inner.lock().await;
        let Some(running) = guard.as_mut() else {
            return Err(GatewayError::NotRunning);
        };
        let clamped_rate = error_rate.clamp(0.0, 1.0);
        let routes = build_routes(&collections);
        let summaries = summarize(&routes, &overrides, &latency_overrides);
        let table = Arc::new(RouteTable::from_routes(routes));
        running.state.replace_table(table);
        running.state.replace_overrides(Arc::new(overrides.clone()));
        running.state.replace_latency(LatencyConfig::new(
            default_latency_ms,
            latency_overrides.clone(),
            latency_jitter_ms.clone(),
        ));
        running.state.replace_error_rate(clamped_rate);
        running.state.replace_capture_bodies(capture_bodies);
        running
            .state
            .replace_enforce_request_bodies(enforce_request_bodies);
        running
            .state
            .replace_response_headers(Arc::new(response_headers.clone()));
        running
            .state
            .replace_required_headers(Arc::new(required_headers.clone()));
        running.state.replace_rate_limits(rate_limits.clone());
        running
            .state
            .replace_status_overrides(Arc::new(status_overrides.clone()));
        running
            .state
            .replace_collections(Arc::new(collections.clone()));
        running.config.example_overrides = overrides;
        running.config.default_latency_ms = default_latency_ms;
        running.config.latency_overrides = latency_overrides;
        running.config.latency_jitter_ms = latency_jitter_ms;
        running.config.error_rate = clamped_rate;
        running.config.capture_bodies = capture_bodies;
        running.config.enforce_request_bodies = enforce_request_bodies;
        running.config.response_headers = response_headers;
        running.config.required_headers = required_headers;
        running.config.rate_limits = rate_limits;
        running.config.status_overrides = status_overrides;
        running.route_summaries = summaries;
        Ok(running.to_status())
    }

    pub async fn metrics(&self) -> MetricsSnapshot {
        let guard = self.inner.lock().await;
        match guard.as_ref() {
            Some(running) => running.state.snapshot_metrics(),
            None => MetricsSnapshot::default(),
        }
    }

    /// Pack the full live config + bound collection IDs into a portable
    /// JSON-friendly bundle. Returns `NotRunning` when the server is
    /// idle (there's nothing meaningful to snapshot).
    pub async fn export_bundle(&self) -> Result<GatewayConfigBundle, GatewayError> {
        let guard = self.inner.lock().await;
        let Some(running) = guard.as_ref() else {
            return Err(GatewayError::NotRunning);
        };
        let collections = running.state.snapshot_collections();
        let collection_ids: Vec<String> = collections.iter().map(|c| c.id.clone()).collect();
        Ok(GatewayConfigBundle {
            bundle_version: GatewayConfigBundle::CURRENT_VERSION.to_string(),
            config: running.config.clone(),
            collection_ids,
        })
    }

    /// Apply a previously-exported bundle to a running server. The
    /// caller supplies the resolved collections (loaded from SQLite),
    /// which the gateway swaps atomically via `reconfigure`. Missing
    /// collection IDs are surfaced in the error rather than silently
    /// dropped so users know when their local store is out of sync.
    pub async fn import_bundle(
        &self,
        bundle: GatewayConfigBundle,
        collections: Vec<CanonicalApiCollection>,
    ) -> Result<GatewayStatus, GatewayError> {
        // Reject incompatible major versions. Minor bumps are additive
        // and handled by `serde(default)` on new fields.
        let major = bundle.bundle_version.split('.').next().unwrap_or("0");
        let expected_major = GatewayConfigBundle::CURRENT_VERSION
            .split('.')
            .next()
            .unwrap_or("0");
        if major != expected_major {
            return Err(GatewayError::InvalidConfig(format!(
                "bundle version {} is not compatible with gateway {}",
                bundle.bundle_version,
                GatewayConfigBundle::CURRENT_VERSION
            )));
        }
        let GatewayConfig {
            default_latency_ms,
            latency_overrides,
            latency_jitter_ms,
            error_rate,
            capture_bodies,
            enforce_request_bodies,
            response_headers,
            required_headers,
            rate_limits,
            status_overrides,
            example_overrides,
            ..
        } = bundle.config;
        self.reconfigure(ReconfigureOptions {
            collections,
            overrides: example_overrides,
            default_latency_ms,
            latency_overrides,
            latency_jitter_ms,
            error_rate,
            capture_bodies,
            enforce_request_bodies,
            response_headers,
            required_headers,
            rate_limits,
            status_overrides,
        })
        .await
    }

    pub async fn recent_requests(&self, limit: usize) -> Vec<RequestLogEntry> {
        let guard = self.inner.lock().await;
        let Some(running) = guard.as_ref() else {
            return Vec::new();
        };
        let log = running.state.request_log.lock().expect("log poisoned");
        let take = limit.min(log.len());
        log.iter().rev().take(take).cloned().collect()
    }

    /// Drop every entry from the request log and reset the cumulative
    /// metrics snapshot. Useful when the user wants a clean slate for a
    /// new scenario without restarting the server (ports, config, routes
    /// all stay put). `started_at_epoch_ms` also rewinds so the
    /// `uptime_ms` in subsequent metrics reflects the new clean period.
    pub async fn clear_log(&self) -> Result<(), GatewayError> {
        let guard = self.inner.lock().await;
        let Some(running) = guard.as_ref() else {
            return Err(GatewayError::NotRunning);
        };
        {
            let mut log = running.state.request_log.lock().expect("log poisoned");
            log.clear();
        }
        {
            let mut metrics = running.state.metrics.lock().expect("metrics poisoned");
            let now = handlers::epoch_ms_now();
            *metrics = MetricsSnapshot {
                started_at_epoch_ms: now,
                ..Default::default()
            };
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), GatewayError> {
        let mut guard = self.inner.lock().await;
        let Some(mut running) = guard.take() else {
            return Err(GatewayError::NotRunning);
        };
        if let Some(tx) = running.shutdown.take() {
            let _ = tx.send(());
        }
        match running.handle.await {
            Ok(()) => Ok(()),
            Err(err) => Err(GatewayError::JoinPanic(err.to_string())),
        }
    }

    pub async fn status(&self) -> GatewayStatus {
        let guard = self.inner.lock().await;
        match guard.as_ref() {
            Some(running) => running.to_status(),
            None => GatewayStatus {
                running: false,
                bind_address: None,
                route_count: 0,
                started_at_epoch_ms: None,
                config: GatewayConfig::default(),
                routes: Vec::new(),
            },
        }
    }

    pub async fn is_running(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    async fn current_config(&self) -> GatewayConfig {
        let guard = self.inner.lock().await;
        guard.as_ref().map(|r| r.config.clone()).unwrap_or_default()
    }
}

impl Default for MockGateway {
    fn default() -> Self {
        Self::new()
    }
}

struct RunningGateway {
    bind_address: String,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
    started_at: i64,
    config: GatewayConfig,
    route_summaries: Vec<GatewayRouteSummary>,
    state: AppState,
}

impl RunningGateway {
    fn to_status(&self) -> GatewayStatus {
        GatewayStatus {
            running: true,
            bind_address: Some(self.bind_address.clone()),
            route_count: self.route_summaries.len(),
            started_at_epoch_ms: Some(self.started_at),
            config: self.config.clone(),
            routes: self.route_summaries.clone(),
        }
    }
}

fn summarize(
    routes: &[MockRoute],
    overrides: &BTreeMap<String, MockExampleKind>,
    latency_overrides: &BTreeMap<String, u64>,
) -> Vec<GatewayRouteSummary> {
    routes
        .iter()
        .map(|route| {
            let key = route_key(&route.method, &route.path);
            let override_kind = overrides.get(&key);
            let selected = route
                .preferred_example(override_kind)
                .map(|example| example.kind.clone());
            GatewayRouteSummary {
                method: route.method.clone(),
                path: route.path.clone(),
                collection_name: route.collection_name.clone(),
                operation_id: route.operation_id.clone(),
                summary: route.summary.clone(),
                selected_example: selected,
                available_examples: route
                    .examples
                    .iter()
                    .map(|example| example.kind.clone())
                    .collect(),
                latency_ms: latency_overrides.get(&key).copied(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{
        CanonicalApiCollection, CanonicalEndpoint, HttpMethod, InputSourceKind, MockExample,
        MockExampleKind,
    };
    use serde_json::json;
    use std::time::Instant;

    fn endpoint(method: HttpMethod, path: &str, success: serde_json::Value) -> CanonicalEndpoint {
        CanonicalEndpoint {
            operation_id: Some(format!("op_{}", path.replace(['/', '{', '}'], "_"))),
            method,
            path: path.to_string(),
            summary: None,
            description: None,
            tags: Vec::new(),
            parameters: Vec::new(),
            request_body: None,
            responses: Vec::new(),
            examples: vec![
                MockExample {
                    kind: MockExampleKind::Success,
                    title: "Success".to_string(),
                    payload: success,
                    note: None,
                },
                MockExample {
                    kind: MockExampleKind::Empty,
                    title: "Empty".to_string(),
                    payload: json!({"items": []}),
                    note: None,
                },
                MockExample {
                    kind: MockExampleKind::Error,
                    title: "Error".to_string(),
                    payload: json!({"error": "forced"}),
                    note: None,
                },
            ],
            auth: None,
        }
    }

    fn collection(name: &str, endpoints: Vec<CanonicalEndpoint>) -> CanonicalApiCollection {
        CanonicalApiCollection {
            id: format!("c:{name}"),
            name: name.to_string(),
            source: InputSourceKind::OpenApi,
            description: None,
            endpoints,
        }
    }

    #[tokio::test]
    async fn starts_and_serves_mock_endpoints() {
        let gateway = MockGateway::new();
        let col = collection(
            "users",
            vec![
                endpoint(HttpMethod::Get, "/users", json!({"data": [{"id": 1}]})),
                endpoint(
                    HttpMethod::Get,
                    "/users/{id}",
                    json!({"id": 42, "name": "Ada"}),
                ),
            ],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().expect("bind");
        assert_eq!(status.route_count, 2);

        let client = reqwest::Client::new();
        let base = format!("http://{}", bind);

        let resp = client.get(format!("{base}/users")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let v: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(v["data"][0]["id"], 1);

        let resp = client.get(format!("{base}/users/42")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let resp = client
            .get(format!("{base}/users?__albert_mock=error"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 400);
        assert_eq!(
            resp.headers()
                .get("x-albert-mock-kind")
                .and_then(|v| v.to_str().ok()),
            Some("error")
        );

        let resp = client.get(format!("{base}/unknown")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 404);

        let resp = client
            .get(format!("{base}/__albert/status"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn captures_request_log() {
        let gateway = MockGateway::new();
        let col = collection(
            "users",
            vec![endpoint(HttpMethod::Get, "/users", json!({"data": [1]}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.unwrap());
        let client = reqwest::Client::new();
        client.get(format!("{base}/users")).send().await.unwrap();
        client
            .get(format!("{base}/users?__albert_mock=error"))
            .send()
            .await
            .unwrap();
        client.get(format!("{base}/missing")).send().await.unwrap();

        let log = gateway.recent_requests(10).await;
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].status, 404);
        assert_eq!(log[1].status, 400);
        assert_eq!(log[1].source, "query");
        assert_eq!(log[2].status, 200);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn updates_collections_without_restart() {
        let gateway = MockGateway::new();
        let initial = collection("a", vec![endpoint(HttpMethod::Get, "/a", json!({"r": 1}))]);
        let status = gateway
            .start(
                vec![initial],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().unwrap();
        let base = format!("http://{}", bind);
        let client = reqwest::Client::new();

        let resp = client.get(format!("{base}/a")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let resp = client.get(format!("{base}/b")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 404);

        let replaced = collection("b", vec![endpoint(HttpMethod::Get, "/b", json!({"r": 2}))]);
        gateway
            .update(vec![replaced], BTreeMap::new())
            .await
            .expect("update");

        let resp = client.get(format!("{base}/b")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let resp = client.get(format!("{base}/a")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 404);

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn applies_latency_overrides() {
        let gateway = MockGateway::new();
        let col = collection(
            "slow",
            vec![endpoint(HttpMethod::Get, "/slow", json!({"ok": true}))],
        );
        let mut latency_overrides = BTreeMap::new();
        latency_overrides.insert("GET /slow".to_string(), 120);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    latency_overrides,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().unwrap();
        let base = format!("http://{}", bind);
        let client = reqwest::Client::new();

        let t0 = Instant::now();
        let resp = client.get(format!("{base}/slow")).send().await.unwrap();
        let elapsed = t0.elapsed();
        assert_eq!(resp.status().as_u16(), 200);
        assert!(
            elapsed.as_millis() >= 100,
            "expected ≥100ms, got {:?}",
            elapsed
        );
        assert_eq!(
            resp.headers()
                .get("x-albert-mock-latency-ms")
                .and_then(|v| v.to_str().ok()),
            Some("120")
        );

        let log = gateway.recent_requests(10).await;
        assert_eq!(log[0].latency_ms, 120);

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn injects_per_route_response_headers() {
        let gateway = MockGateway::new();
        let col = collection(
            "users",
            vec![endpoint(
                HttpMethod::Get,
                "/users",
                json!({"data": [{"id": 1}]}),
            )],
        );
        let mut route_headers: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut headers_for_route = BTreeMap::new();
        headers_for_route.insert("x-request-id".to_string(), "abc-123".to_string());
        headers_for_route.insert("x-rate-limit".to_string(), "100".to_string());
        route_headers.insert("GET /users".to_string(), headers_for_route);

        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    response_headers: route_headers,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().unwrap();
        let base = format!("http://{}", bind);
        let client = reqwest::Client::new();
        let resp = client.get(format!("{base}/users")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(
            resp.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok()),
            Some("abc-123")
        );
        assert_eq!(
            resp.headers()
                .get("x-rate-limit")
                .and_then(|v| v.to_str().ok()),
            Some("100")
        );
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn captures_request_body_when_enabled() {
        let gateway = MockGateway::new();
        let col = collection(
            "echo",
            vec![endpoint(HttpMethod::Post, "/echo", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    capture_bodies: true,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().unwrap();
        let base = format!("http://{}", bind);
        let client = reqwest::Client::new();
        let payload = serde_json::json!({"hello": "world"});
        client
            .post(format!("{base}/echo"))
            .json(&payload)
            .send()
            .await
            .unwrap();

        let log = gateway.recent_requests(1).await;
        let entry = &log[0];
        let body = entry.request_body.clone().unwrap();
        assert!(body.contains("hello"));
        assert!(body.contains("world"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn forced_error_rate_serves_error_example() {
        let gateway = MockGateway::new();
        let col = collection(
            "chaos",
            vec![endpoint(HttpMethod::Get, "/ok", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    error_rate: 1.0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let bind = status.bind_address.clone().unwrap();
        let base = format!("http://{}", bind);
        let client = reqwest::Client::new();

        for _ in 0..3 {
            let resp = client.get(format!("{base}/ok")).send().await.unwrap();
            assert_eq!(resp.status().as_u16(), 400);
            assert_eq!(
                resp.headers()
                    .get("x-albert-mock-kind")
                    .and_then(|v| v.to_str().ok()),
                Some("error")
            );
        }
        let log = gateway.recent_requests(3).await;
        assert!(log.iter().all(|entry| entry.source == "error-rate"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn trailing_slash_matches_declared_route() {
        let gateway = MockGateway::new();
        let col = collection(
            "slash",
            vec![endpoint(HttpMethod::Get, "/items", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        // trailing slash
        let resp = client.get(format!("{base}/items/")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        // no trailing slash
        let resp = client.get(format!("{base}/items")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn head_falls_back_to_get_with_empty_body() {
        let gateway = MockGateway::new();
        let col = collection(
            "head",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"pong": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client.head(format!("{base}/ping")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        let body = resp.bytes().await.unwrap();
        assert!(body.is_empty(), "HEAD must not include a body");
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn options_preflight_returns_cors_headers_when_enabled() {
        let gateway = MockGateway::new();
        let col = collection(
            "cors",
            vec![endpoint(HttpMethod::Get, "/users", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    cors_enabled: true,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .request(reqwest::Method::OPTIONS, format!("{base}/users"))
            .header("origin", "http://example.com")
            .header("access-control-request-method", "GET")
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success() || resp.status().as_u16() == 204);
        assert!(resp.headers().get("access-control-allow-origin").is_some());
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn metrics_endpoint_counts_requests() {
        let gateway = MockGateway::new();
        let col = collection(
            "m",
            vec![endpoint(HttpMethod::Get, "/m", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        client.get(format!("{base}/m")).send().await.unwrap();
        client
            .get(format!("{base}/m?__albert_mock=error"))
            .send()
            .await
            .unwrap();
        client
            .get(format!("{base}/not-there"))
            .send()
            .await
            .unwrap();

        let resp = client
            .get(format!("{base}/__albert/metrics"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        // 3 mock_handler hits; the metrics endpoint itself is served by the
        // status router branch and is not counted.
        assert_eq!(body["total_requests"], 3);
        assert_eq!(body["by_status_class"]["2xx"], 1);
        // 400 from error override + 404 from /not-there
        assert_eq!(body["by_status_class"]["4xx"], 2);
        assert_eq!(body["by_method"]["GET"], 3);
        // uptime should be > 0 (gateway has been running)
        let uptime = body["uptime_ms"].as_i64().unwrap();
        assert!(uptime >= 0);
        // by_route rollup: /m got 2 matched hits (success + error),
        // /not-there is unmatched and must NOT appear in by_route.
        let by_route = body["by_route"].as_object().unwrap();
        let m_route = by_route
            .get("GET /m")
            .expect("matched /m route should appear in by_route");
        assert_eq!(m_route["count"], 2);
        assert!(m_route["p50_ms"].as_u64().is_some());
        assert!(m_route["p95_ms"].as_u64().is_some());
        assert!(!by_route.contains_key("GET /not-there"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn response_templating_substitutes_path_params_and_uuid() {
        let gateway = MockGateway::new();
        let ep = endpoint(
            HttpMethod::Get,
            "/users/{id}",
            json!({
                "id": "{{path.id}}",
                "request_id": "{{uuid}}",
                "fetched_at": "{{now}}"
            }),
        );
        let col = collection("templ", vec![ep]);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client.get(format!("{base}/users/42")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["id"], "42");
        let rid = body["request_id"].as_str().unwrap();
        assert_eq!(rid.split('-').count(), 5, "uuid shape: {rid}");
        let now = body["fetched_at"].as_str().unwrap();
        assert!(now.ends_with('Z'), "rfc3339: {now}");
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn required_headers_gate_returns_401() {
        let gateway = MockGateway::new();
        let col = collection(
            "secure",
            vec![endpoint(HttpMethod::Get, "/secret", json!({"data": "ok"}))],
        );
        let mut required = BTreeMap::new();
        required.insert(
            "GET /secret".to_string(),
            vec![config::RequiredHeader {
                name: "Authorization".to_string(),
                value_prefix: Some("Bearer ".to_string()),
                value_equals: None,
            }],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    required_headers: required,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        // Missing header → 401
        let resp = client.get(format!("{base}/secret")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 401);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"], "unauthorized");
        assert!(body["message"].as_str().unwrap().contains("Authorization"));

        // Wrong prefix → 401
        let resp = client
            .get(format!("{base}/secret"))
            .header("authorization", "Basic abc")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 401);

        // Correct header → 200
        let resp = client
            .get(format!("{base}/secret"))
            .header("authorization", "Bearer secret-token")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        // Source label for the 401 rows
        let log = gateway.recent_requests(3).await;
        assert!(log.iter().any(|e| e.source == "auth-required"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rate_limit_returns_429_when_exceeded() {
        let gateway = MockGateway::new();
        let col = collection(
            "rl",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"ok": true}))],
        );
        let mut rules = BTreeMap::new();
        rules.insert(
            "GET /ping".to_string(),
            config::RateLimitRule {
                limit: 2,
                window_ms: 60_000,
            },
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    rate_limits: rules,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        // Two permitted hits
        for _ in 0..2 {
            let resp = client.get(format!("{base}/ping")).send().await.unwrap();
            assert_eq!(resp.status().as_u16(), 200);
        }

        // Third is denied with structured 429 + Retry-After.
        let resp = client.get(format!("{base}/ping")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 429);
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap()
            .to_string();
        assert!(
            retry_after.parse::<u64>().unwrap() >= 1,
            "retry-after seconds: {retry_after}"
        );
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"], "rate_limited");
        assert_eq!(body["limit"], 2);

        let log = gateway.recent_requests(3).await;
        assert!(log.iter().any(|entry| entry.source == "rate-limited"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rate_limit_recovers_after_window_elapses() {
        // A denied request is informational, not terminal — after the window
        // rolls past, the route should serve again. Uses a 120ms window so
        // the test stays fast.
        let gateway = MockGateway::new();
        let col = collection(
            "win",
            vec![endpoint(HttpMethod::Get, "/rec", json!({"ok": true}))],
        );
        let mut rules = BTreeMap::new();
        rules.insert(
            "GET /rec".to_string(),
            config::RateLimitRule {
                limit: 1,
                window_ms: 120,
            },
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    rate_limits: rules,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        let resp = client.get(format!("{base}/rec")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let resp = client.get(format!("{base}/rec")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 429);
        assert!(resp.headers().get("retry-after").is_some());

        tokio::time::sleep(std::time::Duration::from_millis(160)).await;

        let resp = client.get(format!("{base}/rec")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rate_limit_zero_denies_all_requests() {
        let gateway = MockGateway::new();
        let col = collection(
            "rl0",
            vec![endpoint(
                HttpMethod::Get,
                "/maintenance",
                json!({"ok": true}),
            )],
        );
        let mut rules = BTreeMap::new();
        rules.insert(
            "GET /maintenance".to_string(),
            config::RateLimitRule {
                limit: 0,
                window_ms: 30_000,
            },
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    rate_limits: rules,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/maintenance"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 429);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn reconfigure_preserves_rate_limit_history_for_kept_rules() {
        // When an admin tweaks a limit without restarting, the in-flight
        // window must stay — otherwise a misbehaving client could dodge a
        // tightened rule by racing a config reload.
        let gateway = MockGateway::new();
        let col = collection(
            "hot",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"ok": true}))],
        );
        let mut initial_rules = BTreeMap::new();
        initial_rules.insert(
            "GET /ping".to_string(),
            config::RateLimitRule {
                limit: 2,
                window_ms: 60_000,
            },
        );
        let status = gateway
            .start(
                vec![col.clone()],
                GatewayConfig {
                    port: 0,
                    rate_limits: initial_rules,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        for _ in 0..2 {
            let resp = client.get(format!("{base}/ping")).send().await.unwrap();
            assert_eq!(resp.status().as_u16(), 200);
        }

        // Reconfigure with the same rule; history must carry forward.
        let mut same_rules = BTreeMap::new();
        same_rules.insert(
            "GET /ping".to_string(),
            config::RateLimitRule {
                limit: 2,
                window_ms: 60_000,
            },
        );
        gateway
            .reconfigure(ReconfigureOptions {
                collections: vec![col.clone()],
                rate_limits: same_rules,
                ..Default::default()
            })
            .await
            .expect("reconfigure");

        // Next hit should still be denied.
        let resp = client.get(format!("{base}/ping")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 429);

        // Remove the rule entirely; subsequent requests succeed again.
        gateway
            .reconfigure(ReconfigureOptions {
                collections: vec![col],
                ..Default::default()
            })
            .await
            .expect("reconfigure");
        let resp = client.get(format!("{base}/ping")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn reconfigure_swaps_auth_gate_atomically() {
        // A running server with a bearer gate that flips to an API-key gate
        // must reject bearer-only requests and admit api-key ones after the
        // swap, without dropping the port.
        let gateway = MockGateway::new();
        let col = collection(
            "auth",
            vec![endpoint(HttpMethod::Get, "/secure", json!({"ok": true}))],
        );
        let mut bearer_rules = BTreeMap::new();
        bearer_rules.insert(
            "GET /secure".to_string(),
            vec![config::RequiredHeader {
                name: "Authorization".to_string(),
                value_prefix: Some("Bearer ".to_string()),
                value_equals: None,
            }],
        );
        let status = gateway
            .start(
                vec![col.clone()],
                GatewayConfig {
                    port: 0,
                    required_headers: bearer_rules,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{base}/secure"))
            .header("authorization", "Bearer abc")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let mut api_key_rules = BTreeMap::new();
        api_key_rules.insert(
            "GET /secure".to_string(),
            vec![config::RequiredHeader {
                name: "X-Api-Key".to_string(),
                value_prefix: None,
                value_equals: None,
            }],
        );
        gateway
            .reconfigure(ReconfigureOptions {
                collections: vec![col],
                required_headers: api_key_rules,
                ..Default::default()
            })
            .await
            .expect("reconfigure");

        // Bearer token no longer satisfies the gate.
        let resp = client
            .get(format!("{base}/secure"))
            .header("authorization", "Bearer abc")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 401);
        let resp = client
            .get(format!("{base}/secure"))
            .header("x-api-key", "anything")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn clear_log_resets_requests_and_metrics() {
        let gateway = MockGateway::new();
        let col = collection(
            "clear",
            vec![endpoint(HttpMethod::Get, "/x", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        for _ in 0..3 {
            client.get(format!("{base}/x")).send().await.unwrap();
        }
        assert_eq!(gateway.recent_requests(10).await.len(), 3);
        let before = gateway.metrics().await;
        assert_eq!(before.total_requests, 3);

        gateway.clear_log().await.expect("clear");
        assert!(gateway.recent_requests(10).await.is_empty());
        let after = gateway.metrics().await;
        assert_eq!(after.total_requests, 0);
        assert!(after.by_route.is_empty());

        // Server still running — fresh hits get logged again.
        client.get(format!("{base}/x")).send().await.unwrap();
        assert_eq!(gateway.recent_requests(10).await.len(), 1);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn clear_log_errors_when_not_running() {
        let gateway = MockGateway::new();
        let err = gateway.clear_log().await.err();
        assert!(matches!(err, Some(GatewayError::NotRunning)));
    }

    #[tokio::test]
    async fn schema_validation_rejects_mismatched_bodies_when_enforced() {
        use albert_core::{CanonicalRequestBody, SchemaNode, SchemaNodeType};
        let gateway = MockGateway::new();
        // Hand-roll an endpoint with a request body schema: expects
        // { name: string (required), amount: integer }.
        let mut name_schema = SchemaNode::string();
        name_schema.required = true;
        let mut amount_schema = SchemaNode::string();
        amount_schema.node_type = SchemaNodeType::Integer;
        let mut body_schema = SchemaNode::object();
        body_schema
            .properties
            .insert("name".to_string(), name_schema);
        body_schema
            .properties
            .insert("amount".to_string(), amount_schema);
        let mut ep = endpoint(HttpMethod::Post, "/orders", json!({"id": "o-1"}));
        ep.request_body = Some(CanonicalRequestBody {
            content_type: "application/json".to_string(),
            required: true,
            schema: body_schema,
        });
        let col = collection("val", vec![ep]);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    enforce_request_bodies: true,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        // Missing required field → 400 schema_mismatch pointing at $.name.
        let resp = client
            .post(format!("{base}/orders"))
            .json(&json!({"amount": 10}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 400);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"], "schema_mismatch");
        assert_eq!(body["path"], "$.name");

        // Wrong type for amount → 400 pointing at $.amount.
        let resp = client
            .post(format!("{base}/orders"))
            .json(&json!({"name": "Ada", "amount": "ten"}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 400);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["path"], "$.amount");

        // Valid body → served normally (200 from the success example).
        let resp = client
            .post(format!("{base}/orders"))
            .json(&json!({"name": "Ada", "amount": 10}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        // Validator should be tagging log entries.
        let log = gateway.recent_requests(10).await;
        assert!(log.iter().any(|e| e.source == "schema-mismatch"));

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn schema_validation_off_accepts_any_body() {
        use albert_core::{CanonicalRequestBody, SchemaNode};
        let gateway = MockGateway::new();
        let mut ep = endpoint(HttpMethod::Post, "/lax", json!({"ok": true}));
        ep.request_body = Some(CanonicalRequestBody {
            content_type: "application/json".to_string(),
            required: true,
            schema: SchemaNode::object(),
        });
        let col = collection("lax", vec![ep]);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    // enforce_request_bodies stays false — the gateway
                    // should accept any shape and serve the success example.
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{base}/lax"))
            .json(&json!({"totally": ["wrong", "shape"]}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn bundle_endpoints_round_trip_over_http() {
        let gateway = MockGateway::new();
        let col = collection(
            "h",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"ok": true}))],
        );
        let mut rate_limits = BTreeMap::new();
        rate_limits.insert(
            "GET /ping".to_string(),
            config::RateLimitRule {
                limit: 4,
                window_ms: 500,
            },
        );
        let status = gateway
            .start(
                vec![col.clone()],
                GatewayConfig {
                    port: 0,
                    error_rate: 0.1,
                    rate_limits,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        // GET → the bundle shape
        let resp = client
            .get(format!("{base}/__albert/config/bundle"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let bundle: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(bundle["bundle_version"], "1.0");
        assert_eq!(bundle["collection_ids"][0], col.id);
        assert_eq!(bundle["config"]["rate_limits"]["GET /ping"]["limit"], 4);

        // Flip the server's state so we can see import put things back.
        gateway
            .reconfigure(ReconfigureOptions {
                collections: vec![col.clone()],
                ..Default::default()
            })
            .await
            .expect("reconfigure");
        let reset = gateway.status().await.config;
        assert!(reset.rate_limits.is_empty());

        // POST the bundle back with the inlined collection.
        let payload = serde_json::json!({
            "bundle": bundle,
            "collections": [col],
        });
        let resp = client
            .post(format!("{base}/__albert/config/bundle"))
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 204);

        // GET again to prove the rules came back
        let bundle2: serde_json::Value = client
            .get(format!("{base}/__albert/config/bundle"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(bundle2["config"]["rate_limits"]["GET /ping"]["limit"], 4);

        // Bad body → 400 with a structured error
        let bad = client
            .post(format!("{base}/__albert/config/bundle"))
            .json(&serde_json::json!({"nope": true}))
            .send()
            .await
            .unwrap();
        assert_eq!(bad.status().as_u16(), 400);
        let err: serde_json::Value = bad.json().await.unwrap();
        assert_eq!(err["error"], "bundle_invalid");

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn config_bundle_round_trip_preserves_rules() {
        // A bundle exported from a running server should apply back to
        // another (or the same) server and restore every rule that was
        // set, including per-route rate limits and status overrides.
        let gateway = MockGateway::new();
        let col = collection(
            "rt",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"ok": true}))],
        );
        let mut rate_limits = BTreeMap::new();
        rate_limits.insert(
            "GET /ping".to_string(),
            config::RateLimitRule {
                limit: 5,
                window_ms: 1_000,
            },
        );
        let mut status_overrides = BTreeMap::new();
        status_overrides.insert("GET /ping".to_string(), 418);
        gateway
            .start(
                vec![col.clone()],
                GatewayConfig {
                    port: 0,
                    default_latency_ms: Some(50),
                    error_rate: 0.2,
                    rate_limits,
                    status_overrides,
                    ..Default::default()
                },
            )
            .await
            .expect("start");

        let bundle = gateway.export_bundle().await.expect("export");
        assert_eq!(bundle.bundle_version, "1.0");
        assert_eq!(bundle.collection_ids, vec![col.id.clone()]);
        assert_eq!(bundle.config.default_latency_ms, Some(50));
        assert_eq!(bundle.config.rate_limits.len(), 1);

        // Round-trip through JSON to prove the bundle is actually portable.
        let serialized = serde_json::to_string(&bundle).unwrap();
        let parsed: GatewayConfigBundle = serde_json::from_str(&serialized).unwrap();

        // Flip to a different config on the running server so we can
        // observe import putting the original values back.
        gateway
            .reconfigure(ReconfigureOptions {
                collections: vec![col.clone()],
                ..Default::default()
            })
            .await
            .expect("reconfigure");
        let after_reset = gateway.status().await.config;
        assert_eq!(after_reset.error_rate, 0.0);
        assert!(after_reset.rate_limits.is_empty());

        gateway
            .import_bundle(parsed, vec![col])
            .await
            .expect("import");
        let restored = gateway.status().await.config;
        assert_eq!(restored.default_latency_ms, Some(50));
        assert!((restored.error_rate - 0.2).abs() < 1e-5);
        assert_eq!(restored.rate_limits.len(), 1);
        assert_eq!(
            restored.status_overrides.get("GET /ping").copied(),
            Some(418)
        );

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn import_bundle_rejects_incompatible_major_version() {
        let gateway = MockGateway::new();
        gateway
            .start(
                Vec::new(),
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let future_bundle = GatewayConfigBundle {
            bundle_version: "9.9".to_string(),
            config: GatewayConfig::default(),
            collection_ids: Vec::new(),
        };
        let err = gateway
            .import_bundle(future_bundle, Vec::new())
            .await
            .expect_err("expected version mismatch");
        match err {
            GatewayError::InvalidConfig(msg) => assert!(msg.contains("9.9")),
            other => panic!("unexpected error: {other:?}"),
        }
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn export_bundle_fails_when_not_running() {
        let gateway = MockGateway::new();
        let err = gateway.export_bundle().await.err().unwrap();
        assert!(matches!(err, GatewayError::NotRunning));
    }

    #[tokio::test]
    async fn openapi_endpoint_serves_live_collection_as_spec() {
        let gateway = MockGateway::new();
        let col = collection(
            "orders",
            vec![endpoint(
                HttpMethod::Get,
                "/orders/{id}",
                json!({"id": "o-1"}),
            )],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .get(format!(
                "{base}/__albert/openapi.json?base=http://mock.local"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let doc: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(doc["openapi"], "3.0.3");
        assert!(doc["paths"]["/orders/{id}"]["get"].is_object());
        assert_eq!(doc["servers"][0]["url"], "http://mock.local");
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn docs_endpoint_serves_swagger_ui_html() {
        let gateway = MockGateway::new();
        let col = collection(
            "intro",
            vec![endpoint(HttpMethod::Get, "/users", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/__albert/docs"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let ctype = resp
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("");
        assert!(
            ctype.starts_with("text/html"),
            "expected HTML content-type, got {ctype:?}"
        );
        let body = resp.text().await.unwrap();
        // The loaded page should point at the sibling spec + embed swagger-ui.
        assert!(body.contains("swagger-ui"));
        assert!(body.contains("./openapi.json"));
        assert!(body.contains("SwaggerUIBundle"));
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn config_endpoint_reflects_loaded_rules() {
        // A running server with a scoped rate-limit + status-override
        // should return both in its /__albert/config payload so external
        // tooling can see what's live.
        let gateway = MockGateway::new();
        let col = collection(
            "intro",
            vec![endpoint(HttpMethod::Get, "/users", json!({"ok": true}))],
        );
        let mut rate_limits = BTreeMap::new();
        rate_limits.insert(
            "GET /users".to_string(),
            config::RateLimitRule {
                limit: 3,
                window_ms: 1_000,
            },
        );
        let mut status_overrides = BTreeMap::new();
        status_overrides.insert("GET /users".to_string(), 418);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    error_rate: 0.25,
                    rate_limits,
                    status_overrides,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/__albert/config"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["route_count"], 1);
        assert_eq!(body["error_rate"], 0.25);
        assert_eq!(body["capture_bodies"], false);
        assert_eq!(body["rate_limits"]["GET /users"]["limit"], 3);
        assert_eq!(body["status_overrides"]["GET /users"], 418);
        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn latency_jitter_stays_within_bounds() {
        // Base 50ms + jitter 20ms → every hit sleeps in [30, 70]ms.
        // Runs 20 hits to exercise both tails; with a 20ms bound the
        // worst-case observed should never fall below 30ms.
        let gateway = MockGateway::new();
        let col = collection(
            "jit",
            vec![endpoint(HttpMethod::Get, "/bounce", json!({"ok": true}))],
        );
        let mut latency_overrides = BTreeMap::new();
        latency_overrides.insert("GET /bounce".to_string(), 50);
        let mut latency_jitter_ms = BTreeMap::new();
        latency_jitter_ms.insert("GET /bounce".to_string(), 20);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    latency_overrides,
                    latency_jitter_ms,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        let mut latencies = Vec::new();
        for _ in 0..6 {
            let resp = client.get(format!("{base}/bounce")).send().await.unwrap();
            let logged = resp
                .headers()
                .get("x-albert-mock-latency-ms")
                .and_then(|v| v.to_str().ok())
                .unwrap()
                .parse::<u64>()
                .unwrap();
            latencies.push(logged);
        }
        // Every observed latency must stay within [30, 70].
        for ms in &latencies {
            assert!(*ms >= 30 && *ms <= 70, "latency out of range: {ms}");
        }
        // And at least one draw should differ from the base (otherwise the
        // jitter isn't actually firing).
        assert!(
            latencies.iter().any(|ms| *ms != 50),
            "no jitter observed across {} hits",
            latencies.len()
        );

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn status_overrides_beat_kind_default() {
        // Default success→200; override to 201. Also verify a non-standard
        // code like 418 is honored and an invalid one like 999 falls back.
        let gateway = MockGateway::new();
        let col = collection(
            "stat",
            vec![
                endpoint(HttpMethod::Post, "/orders", json!({"id": "o-1"})),
                endpoint(HttpMethod::Get, "/teapot", json!({"brew": true})),
                endpoint(HttpMethod::Get, "/bogus", json!({})),
            ],
        );
        let mut status_overrides = BTreeMap::new();
        status_overrides.insert("POST /orders".to_string(), 201);
        status_overrides.insert("GET /teapot".to_string(), 418);
        // Out-of-range: the config applier clamps to 100–599, so 999
        // silently falls back to the kind default.
        status_overrides.insert("GET /bogus".to_string(), 999);
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    status_overrides,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        let resp = client.post(format!("{base}/orders")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 201);

        let resp = client.get(format!("{base}/teapot")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 418);

        let resp = client.get(format!("{base}/bogus")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let log = gateway.recent_requests(10).await;
        assert!(log.iter().any(|e| e.source == "status-override"));

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn request_id_header_honored_or_generated() {
        let gateway = MockGateway::new();
        let col = collection(
            "trace",
            vec![endpoint(HttpMethod::Get, "/ping", json!({"ok": true}))],
        );
        let status = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("start");
        let base = format!("http://{}", status.bind_address.clone().unwrap());
        let client = reqwest::Client::new();

        // Client-supplied id is echoed back verbatim on success.
        let resp = client
            .get(format!("{base}/ping"))
            .header("x-request-id", "trace-abc-123")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(
            resp.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok()),
            Some("trace-abc-123")
        );

        // No header supplied → gateway fabricates a UUID-ish id.
        let resp = client.get(format!("{base}/ping")).send().await.unwrap();
        let generated = resp
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap()
            .to_string();
        assert_eq!(generated.split('-').count(), 5);

        // 404 responses also carry a request id + echo it in the JSON body.
        let resp = client
            .get(format!("{base}/missing"))
            .header("x-request-id", "missing-id-xyz")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 404);
        assert_eq!(
            resp.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok()),
            Some("missing-id-xyz")
        );
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["request_id"], "missing-id-xyz");

        // Log entries carry the id so the UI can correlate responses.
        let log = gateway.recent_requests(10).await;
        assert!(
            log.iter()
                .any(|e| e.request_id.as_deref() == Some("trace-abc-123"))
        );
        assert!(
            log.iter()
                .any(|e| e.request_id.as_deref() == Some("missing-id-xyz"))
        );

        gateway.stop().await.expect("stop");
    }

    #[tokio::test]
    async fn rejects_double_start() {
        let gateway = MockGateway::new();
        let col = collection("x", vec![endpoint(HttpMethod::Get, "/x", json!({}))]);
        let status = gateway
            .start(
                vec![col.clone()],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .expect("first start");
        assert!(status.running);
        let err = gateway
            .start(
                vec![col],
                GatewayConfig {
                    port: 0,
                    ..Default::default()
                },
            )
            .await
            .err();
        assert!(matches!(err, Some(GatewayError::AlreadyRunning)));
        gateway.stop().await.expect("stop");
    }
}
