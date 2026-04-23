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
pub mod route;
pub mod routing;
pub mod state;

pub use config::{
    GatewayConfig, GatewayRouteSummary, GatewayStatus, planned_capabilities,
    supported_example_kinds,
};
pub use error::GatewayError;
pub use route::{MatchedRoute, MockRoute, build_routes, route_key};
pub use routing::{CompiledRoute, RouteTable};
pub use state::RequestLogEntry;

use state::{AppState, LatencyConfig};

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
        let latency =
            LatencyConfig::new(config.default_latency_ms, config.latency_overrides.clone());

        let response_headers = Arc::new(config.response_headers.clone());
        let state = AppState::new(
            table_arc.clone(),
            overrides_arc.clone(),
            latency,
            config.error_rate,
            config.capture_bodies,
            response_headers,
        );
        let state_for_runtime = state.clone();

        let mut router = Router::new()
            .route(
                "/__albert/status",
                axum::routing::get(handlers::status_handler),
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

        let started_at = handlers::epoch_ms_now();
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
        self.reconfigure(
            collections,
            overrides,
            config.default_latency_ms,
            config.latency_overrides,
            config.error_rate,
            config.capture_bodies,
            config.response_headers,
        )
        .await
    }

    /// Full reconfigure entry point; can change overrides, latency, error
    /// rate, body-capture flag, and per-route response headers in one
    /// atomic swap.
    #[allow(clippy::too_many_arguments)]
    pub async fn reconfigure(
        &self,
        collections: Vec<CanonicalApiCollection>,
        overrides: BTreeMap<String, MockExampleKind>,
        default_latency_ms: Option<u64>,
        latency_overrides: BTreeMap<String, u64>,
        error_rate: f32,
        capture_bodies: bool,
        response_headers: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Result<GatewayStatus, GatewayError> {
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
        ));
        running.state.replace_error_rate(clamped_rate);
        running.state.replace_capture_bodies(capture_bodies);
        running
            .state
            .replace_response_headers(Arc::new(response_headers.clone()));
        running.config.example_overrides = overrides;
        running.config.default_latency_ms = default_latency_ms;
        running.config.latency_overrides = latency_overrides;
        running.config.error_rate = clamped_rate;
        running.config.capture_bodies = capture_bodies;
        running.config.response_headers = response_headers;
        running.route_summaries = summaries;
        Ok(running.to_status())
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
