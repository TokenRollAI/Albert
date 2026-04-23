//! Local mock HTTP gateway.
//!
//! Phase 3 runtime: serves canonical endpoints over an axum-based HTTP server,
//! selecting from the per-endpoint `success / empty / error` mock examples.

use std::collections::{BTreeMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex as StdMutex};

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, CapabilityStatus, DeliveryStage, HttpMethod,
    MockExample, MockExampleKind, default_mock_examples,
};
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;

pub mod routing;

pub use routing::{CompiledRoute, RouteTable};

/// A snapshot of one mock endpoint that the HTTP runtime can serve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockRoute {
    pub collection_id: String,
    pub collection_name: String,
    pub method: HttpMethod,
    pub path: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub examples: Vec<MockExample>,
}

impl MockRoute {
    pub fn from_endpoint(
        collection: &CanonicalApiCollection,
        endpoint: &CanonicalEndpoint,
    ) -> Self {
        let examples = if endpoint.examples.is_empty() {
            default_mock_examples()
        } else {
            endpoint.examples.clone()
        };
        Self {
            collection_id: collection.id.clone(),
            collection_name: collection.name.clone(),
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            operation_id: endpoint.operation_id.clone(),
            summary: endpoint.summary.clone(),
            examples,
        }
    }

    pub fn example(&self, kind: &MockExampleKind) -> Option<&MockExample> {
        self.examples
            .iter()
            .find(|candidate| &candidate.kind == kind)
    }

    pub fn preferred_example(
        &self,
        override_kind: Option<&MockExampleKind>,
    ) -> Option<&MockExample> {
        if let Some(kind) = override_kind
            && let Some(found) = self.example(kind)
        {
            return Some(found);
        }
        self.example(&MockExampleKind::Success)
            .or_else(|| self.examples.first())
    }
}

/// Configuration for a running mock gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayConfig {
    /// Host binding, e.g. "127.0.0.1".
    pub host: String,
    /// Port number. Use 0 for ephemeral.
    pub port: u16,
    /// Enables permissive CORS so that browser clients can hit the mock.
    pub cors_enabled: bool,
    /// Per-endpoint overrides, keyed by `METHOD path`.
    pub example_overrides: BTreeMap<String, MockExampleKind>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 4317,
            cors_enabled: true,
            example_overrides: BTreeMap::new(),
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
}

const DEFAULT_REQUEST_LOG_CAPACITY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayRouteSummary {
    pub method: HttpMethod,
    pub path: String,
    pub collection_name: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub selected_example: Option<MockExampleKind>,
    pub available_examples: Vec<MockExampleKind>,
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("mock gateway is already running")]
    AlreadyRunning,
    #[error("mock gateway is not running")]
    NotRunning,
    #[error("failed to bind to {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("gateway task panicked: {0}")]
    JoinPanic(String),
    #[error("invalid gateway configuration: {0}")]
    InvalidConfig(String),
}

/// Shared snapshot handed to axum handlers.
#[derive(Clone)]
struct AppState {
    table: Arc<StdMutex<Arc<RouteTable>>>,
    overrides: Arc<StdMutex<Arc<BTreeMap<String, MockExampleKind>>>>,
    request_log: Arc<StdMutex<VecDeque<RequestLogEntry>>>,
}

impl AppState {
    fn snapshot_table(&self) -> Arc<RouteTable> {
        self.table.lock().expect("route table poisoned").clone()
    }

    fn snapshot_overrides(&self) -> Arc<BTreeMap<String, MockExampleKind>> {
        self.overrides.lock().expect("overrides poisoned").clone()
    }

    fn record(&self, entry: RequestLogEntry) {
        let mut log = self.request_log.lock().expect("log poisoned");
        if log.len() >= DEFAULT_REQUEST_LOG_CAPACITY {
            log.pop_front();
        }
        log.push_back(entry);
    }
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
        let summaries = summarize(&routes, &config.example_overrides);
        let table_arc = Arc::new(RouteTable::from_routes(routes));
        let overrides_arc = Arc::new(config.example_overrides.clone());
        let request_log = Arc::new(StdMutex::new(VecDeque::with_capacity(
            DEFAULT_REQUEST_LOG_CAPACITY,
        )));

        let state = AppState {
            table: Arc::new(StdMutex::new(table_arc.clone())),
            overrides: Arc::new(StdMutex::new(overrides_arc.clone())),
            request_log: request_log.clone(),
        };
        let state_for_runtime = state.clone();

        let mut router = Router::new()
            .route("/__albert/status", axum::routing::get(status_handler))
            .fallback(any(mock_handler))
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

        let started_at = epoch_ms_now();
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
        let mut guard = self.inner.lock().await;
        let Some(running) = guard.as_mut() else {
            return Err(GatewayError::NotRunning);
        };
        let routes = build_routes(&collections);
        let summaries = summarize(&routes, &overrides);
        let table = Arc::new(RouteTable::from_routes(routes));
        {
            let mut slot = running.state.table.lock().expect("route table poisoned");
            *slot = table;
        }
        {
            let mut slot = running.state.overrides.lock().expect("overrides poisoned");
            *slot = Arc::new(overrides.clone());
        }
        running.config.example_overrides = overrides;
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

fn build_routes(collections: &[CanonicalApiCollection]) -> Vec<MockRoute> {
    let mut routes = Vec::new();
    for collection in collections {
        for endpoint in &collection.endpoints {
            routes.push(MockRoute::from_endpoint(collection, endpoint));
        }
    }
    routes
}

fn summarize(
    routes: &[MockRoute],
    overrides: &BTreeMap<String, MockExampleKind>,
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
            }
        })
        .collect()
}

pub fn route_key(method: &HttpMethod, path: &str) -> String {
    format!("{} {}", method.as_str(), path)
}

async fn status_handler(State(state): State<AppState>) -> Response {
    let table = state.snapshot_table();
    let payload = serde_json::json!({
        "service": "albert-mock-gateway",
        "route_count": table.len(),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

async fn mock_handler(State(state): State<AppState>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());

    let method_kind = match http_method_to_canonical(&method) {
        Some(kind) => kind,
        None => {
            state.record(RequestLogEntry {
                at_epoch_ms: epoch_ms_now(),
                method: method.to_string(),
                path: path.clone(),
                query: query.clone(),
                matched_route: None,
                collection_name: None,
                status: 404,
                kind: None,
                source: "unsupported",
            });
            return not_found(format!("unsupported method {method}"));
        }
    };
    let table = state.snapshot_table();
    let overrides = state.snapshot_overrides();
    let Some(matched) = table.match_route(&method_kind, &path) else {
        state.record(RequestLogEntry {
            at_epoch_ms: epoch_ms_now(),
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            matched_route: None,
            collection_name: None,
            status: 404,
            kind: None,
            source: "unmatched",
        });
        return not_found(format!(
            "no mock registered for {} {}",
            method.as_str(),
            path
        ));
    };
    let route = matched.route;

    let (override_kind, query_selected) = parse_query_override(query.as_deref());
    let fallback_override = overrides
        .get(&route_key(&route.method, &route.path))
        .cloned();
    let chosen_override = override_kind.clone().or(fallback_override.clone());
    let Some(example) = route.preferred_example(chosen_override.as_ref()) else {
        state.record(RequestLogEntry {
            at_epoch_ms: epoch_ms_now(),
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            matched_route: Some(route_key(&route.method, &route.path)),
            collection_name: Some(route.collection_name.clone()),
            status: 404,
            kind: None,
            source: "no-example",
        });
        return not_found(format!(
            "no example configured for {} {}",
            method.as_str(),
            path
        ));
    };

    let status_line_code: u16 = match example.kind {
        MockExampleKind::Success | MockExampleKind::Empty => 200,
        MockExampleKind::Error => 400,
    };
    let source = if query_selected {
        "query"
    } else if override_kind.is_some() || fallback_override.is_some() {
        "override"
    } else {
        "default"
    };
    state.record(RequestLogEntry {
        at_epoch_ms: epoch_ms_now(),
        method: method.to_string(),
        path: path.clone(),
        query: query.clone(),
        matched_route: Some(route_key(&route.method, &route.path)),
        collection_name: Some(route.collection_name.clone()),
        status: status_line_code,
        kind: Some(example.kind.clone()),
        source,
    });

    let (status, body) = render_example(example);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(name) = HeaderName::from_bytes(b"x-albert-mock-kind")
        && let Ok(value) = HeaderValue::from_str(example.kind.as_str())
    {
        headers.insert(name, value);
    }
    if let Ok(name) = HeaderName::from_bytes(b"x-albert-mock-route")
        && let Ok(value) = HeaderValue::from_str(&route_key(&route.method, &route.path))
    {
        headers.insert(name, value);
    }
    if query_selected && let Ok(name) = HeaderName::from_bytes(b"x-albert-mock-source") {
        headers.insert(name, HeaderValue::from_static("query"));
    }

    (status, headers, body).into_response()
}

pub struct MatchedRoute<'a> {
    pub route: &'a MockRoute,
    pub params: BTreeMap<String, String>,
}

fn render_example(example: &MockExample) -> (StatusCode, Body) {
    let status = match example.kind {
        MockExampleKind::Success => StatusCode::OK,
        MockExampleKind::Empty => StatusCode::OK,
        MockExampleKind::Error => StatusCode::BAD_REQUEST,
    };
    let body = serde_json::to_vec(&example.payload).unwrap_or_else(|_| b"{}".to_vec());
    (status, Body::from(body))
}

fn parse_query_override(query: Option<&str>) -> (Option<MockExampleKind>, bool) {
    let Some(query) = query else {
        return (None, false);
    };
    for pair in query.split('&') {
        let mut iter = pair.splitn(2, '=');
        let key = iter.next().unwrap_or("");
        let value = iter.next().unwrap_or("");
        if key == "__albert_mock" {
            return match value {
                "success" => (Some(MockExampleKind::Success), true),
                "empty" => (Some(MockExampleKind::Empty), true),
                "error" => (Some(MockExampleKind::Error), true),
                _ => (None, false),
            };
        }
    }
    (None, false)
}

fn http_method_to_canonical(method: &Method) -> Option<HttpMethod> {
    Some(match method.as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "HEAD" => HttpMethod::Head,
        _ => return None,
    })
}

fn not_found(message: String) -> Response {
    let payload = serde_json::json!({
        "error": "mock_not_found",
        "message": message,
    });
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(payload),
    )
        .into_response()
}

fn epoch_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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
            note: "Axum + hyper server with graceful shutdown and permissive CORS.".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{
        CanonicalApiCollection, CanonicalEndpoint, InputSourceKind, MockExample, MockExampleKind,
    };
    use serde_json::json;

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

        let resp = client
            .get(format!("{base}/users"))
            .send()
            .await
            .expect("get list");
        assert_eq!(resp.status().as_u16(), 200);
        let v: serde_json::Value = resp.json().await.expect("json");
        assert_eq!(v["data"][0]["id"], 1);

        let resp = client
            .get(format!("{base}/users/42"))
            .send()
            .await
            .expect("get item");
        assert_eq!(resp.status().as_u16(), 200);
        let v: serde_json::Value = resp.json().await.expect("json");
        assert_eq!(v["id"], 42);

        let resp = client
            .get(format!("{base}/users?__albert_mock=error"))
            .send()
            .await
            .expect("force error");
        assert_eq!(resp.status().as_u16(), 400);
        assert_eq!(
            resp.headers()
                .get("x-albert-mock-kind")
                .and_then(|v| v.to_str().ok()),
            Some("error")
        );

        let resp = client
            .get(format!("{base}/unknown"))
            .send()
            .await
            .expect("404");
        assert_eq!(resp.status().as_u16(), 404);

        let resp = client
            .get(format!("{base}/__albert/status"))
            .send()
            .await
            .expect("status");
        assert_eq!(resp.status().as_u16(), 200);

        gateway.stop().await.expect("stop");
        assert!(!gateway.is_running().await);
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
        client
            .get(format!("{base}/users"))
            .send()
            .await
            .expect("hit");
        client
            .get(format!("{base}/users?__albert_mock=error"))
            .send()
            .await
            .expect("hit err");
        client
            .get(format!("{base}/missing"))
            .send()
            .await
            .expect("hit 404");

        let log = gateway.recent_requests(10).await;
        assert_eq!(log.len(), 3);
        // newest first
        assert_eq!(log[0].status, 404);
        assert_eq!(log[0].path, "/missing");
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
        let bind = status.bind_address.clone().expect("bind");
        let base = format!("http://{}", bind);

        let client = reqwest::Client::new();
        let resp = client.get(format!("{base}/a")).send().await.expect("get a");
        assert_eq!(resp.status().as_u16(), 200);
        let resp = client.get(format!("{base}/b")).send().await.expect("get b");
        assert_eq!(resp.status().as_u16(), 404);

        let replaced = collection("b", vec![endpoint(HttpMethod::Get, "/b", json!({"r": 2}))]);
        gateway
            .update(vec![replaced], BTreeMap::new())
            .await
            .expect("update");

        let resp = client
            .get(format!("{base}/b"))
            .send()
            .await
            .expect("get b after update");
        assert_eq!(resp.status().as_u16(), 200);
        let resp = client
            .get(format!("{base}/a"))
            .send()
            .await
            .expect("get a after update");
        assert_eq!(resp.status().as_u16(), 404);

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
