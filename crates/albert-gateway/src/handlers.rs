//! Axum handlers + HTTP helpers for the mock gateway.

use std::time::Duration;

use albert_core::{HttpMethod, MockExample, MockExampleKind};
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::time::sleep;

const MAX_CAPTURED_BODY_BYTES: usize = 4 * 1024;

use crate::route::route_key;
use crate::state::{AppState, RequestLogEntry};

pub(crate) async fn status_handler(State(state): State<AppState>) -> Response {
    let table = state.snapshot_table();
    let payload = serde_json::json!({
        "service": "albert-mock-gateway",
        "route_count": table.len(),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

pub(crate) async fn metrics_handler(State(state): State<AppState>) -> Response {
    let metrics = state.snapshot_metrics();
    let payload = serde_json::json!({
        "total_requests": metrics.total_requests,
        "by_method": metrics.by_method,
        "by_status_class": metrics.by_status_class,
        "average_latency_ms": metrics.average_latency_ms(),
        "max_latency_ms": metrics.max_latency_ms,
        "started_at_epoch_ms": metrics.started_at_epoch_ms,
        "uptime_ms": epoch_ms_now().saturating_sub(metrics.started_at_epoch_ms),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

pub(crate) async fn mock_handler(State(state): State<AppState>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());
    let capture_bodies = state.snapshot_capture_bodies();
    // Snapshot request headers before we consume the body; required-header
    // gating needs them and we don't want the Request value moved into the
    // body-capture helper before we've read them.
    let request_headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_ascii_lowercase(), v.to_string()))
        })
        .collect();
    let captured_body = if capture_bodies && method != Method::GET && method != Method::HEAD {
        capture_request_body(request).await
    } else {
        CapturedBody::None
    };
    let captured_string = captured_body.as_string();

    let Some(method_kind) = http_method_to_canonical(&method) else {
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
            latency_ms: 0,
            request_body: captured_string.clone(),
        });
        return not_found(format!("unsupported method {method}"));
    };
    let table = state.snapshot_table();
    let overrides = state.snapshot_overrides();
    let latency = state.snapshot_latency();
    // Fall back to GET when a HEAD request doesn't match a declared HEAD
    // route — real APIs rarely declare HEAD separately but health checks
    // still expect it to succeed with the GET response headers.
    let fallback_to_get = matches!(method_kind, HttpMethod::Head);
    let matched = match table.match_route(&method_kind, &path) {
        Some(m) => m,
        None if fallback_to_get => match table.match_route(&HttpMethod::Get, &path) {
            Some(m) => m,
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
                    source: "unmatched",
                    latency_ms: 0,
                    request_body: captured_string.clone(),
                });
                return not_found(format!(
                    "no mock registered for {} {}",
                    method.as_str(),
                    path
                ));
            }
        },
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
                source: "unmatched",
                latency_ms: 0,
                request_body: captured_string.clone(),
            });
            return not_found(format!(
                "no mock registered for {} {}",
                method.as_str(),
                path
            ));
        }
    };
    let route = matched.route;
    let matched_key = route_key(&route.method, &route.path);
    let strip_body = fallback_to_get;

    // Required-header gate: evaluated before example selection so an
    // unauthorized request never touches the mock data.
    let required = state.snapshot_required_headers();
    if let Some(rules) = required.get(&matched_key)
        && let Some(err) = evaluate_required_headers(rules, &request_headers)
    {
        state.record(RequestLogEntry {
            at_epoch_ms: epoch_ms_now(),
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            matched_route: Some(matched_key.clone()),
            collection_name: Some(route.collection_name.clone()),
            status: 401,
            kind: None,
            source: "auth-required",
            latency_ms: 0,
            request_body: captured_string.clone(),
        });
        return unauthorized(err);
    }

    let (override_kind, query_selected) = parse_query_override(query.as_deref());
    let fallback_override = overrides.get(&matched_key).cloned();
    let mut chosen_override = override_kind.clone().or(fallback_override.clone());
    let error_rate = state.snapshot_error_rate();
    let error_injected = if error_rate > 0.0 && roll_probability(error_rate) {
        chosen_override = Some(MockExampleKind::Error);
        true
    } else {
        false
    };
    let Some(example) = route.preferred_example(chosen_override.as_ref()) else {
        state.record(RequestLogEntry {
            at_epoch_ms: epoch_ms_now(),
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            matched_route: Some(matched_key.clone()),
            collection_name: Some(route.collection_name.clone()),
            status: 404,
            kind: None,
            source: "no-example",
            latency_ms: 0,
            request_body: captured_string.clone(),
        });
        return not_found(format!(
            "no example configured for {} {}",
            method.as_str(),
            path
        ));
    };

    let latency_ms = latency.resolve(&matched_key);
    if latency_ms > 0 {
        sleep(Duration::from_millis(latency_ms)).await;
    }

    let status_line_code: u16 = match example.kind {
        MockExampleKind::Success | MockExampleKind::Empty => 200,
        MockExampleKind::Error => 400,
    };
    let source = if query_selected {
        "query"
    } else if error_injected {
        "error-rate"
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
        matched_route: Some(matched_key.clone()),
        collection_name: Some(route.collection_name.clone()),
        status: status_line_code,
        kind: Some(example.kind.clone()),
        source,
        latency_ms,
        request_body: captured_string,
    });

    let (status, body) = render_example(example);
    let body = if strip_body { Body::empty() } else { body };
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
        && let Ok(value) = HeaderValue::from_str(&matched_key)
    {
        headers.insert(name, value);
    }
    if query_selected && let Ok(name) = HeaderName::from_bytes(b"x-albert-mock-source") {
        headers.insert(name, HeaderValue::from_static("query"));
    }
    if latency_ms > 0
        && let Ok(name) = HeaderName::from_bytes(b"x-albert-mock-latency-ms")
        && let Ok(value) = HeaderValue::from_str(&latency_ms.to_string())
    {
        headers.insert(name, value);
    }

    let response_headers = state.snapshot_response_headers();
    if let Some(extras) = response_headers.get(&matched_key) {
        for (name, value) in extras {
            if let Ok(header_name) = HeaderName::from_bytes(name.as_bytes())
                && let Ok(header_value) = HeaderValue::from_str(value)
            {
                headers.insert(header_name, header_value);
            }
        }
    }

    (status, headers, body).into_response()
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

pub(crate) fn parse_query_override(query: Option<&str>) -> (Option<MockExampleKind>, bool) {
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

fn evaluate_required_headers(
    rules: &[crate::config::RequiredHeader],
    request_headers: &[(String, String)],
) -> Option<String> {
    for rule in rules {
        let wanted = rule.name.to_ascii_lowercase();
        let actual = request_headers
            .iter()
            .find(|(name, _)| *name == wanted)
            .map(|(_, value)| value.as_str());
        match actual {
            None => {
                return Some(format!("missing required header '{}'", rule.name));
            }
            Some(value) => {
                if let Some(prefix) = &rule.value_prefix
                    && !value.starts_with(prefix)
                {
                    return Some(format!("header '{}' must start with '{prefix}'", rule.name));
                }
                if let Some(expected) = &rule.value_equals
                    && value != expected
                {
                    return Some(format!(
                        "header '{}' does not match expected value",
                        rule.name
                    ));
                }
            }
        }
    }
    None
}

fn unauthorized(reason: String) -> Response {
    let payload = serde_json::json!({
        "error": "unauthorized",
        "message": reason,
    });
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(payload),
    )
        .into_response()
}

pub(crate) fn not_found(message: String) -> Response {
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

pub(crate) fn epoch_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Dependency-free coin flip for error-rate injection. Uses a `SplitMix64`-
/// derived linear congruential sequence seeded from the OS monotonic clock.
fn roll_probability(threshold: f32) -> bool {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new(seed());
    }
    let clamped = threshold.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return false;
    }
    if clamped >= 1.0 {
        return true;
    }
    STATE.with(|slot| {
        let mut x = slot.get();
        if x == 0 {
            x = seed();
        }
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        slot.set(x);
        let scaled = ((x >> 33) as f32) / ((1u64 << 31) as f32);
        scaled < clamped
    })
}

/// Captured request body variants: we want to distinguish "nothing to capture"
/// from "attempted but failed" to keep the log faithful.
pub(crate) enum CapturedBody {
    None,
    Truncated(String),
    Full(String),
    Failed(String),
}

impl CapturedBody {
    pub(crate) fn as_string(&self) -> Option<String> {
        match self {
            CapturedBody::None => None,
            CapturedBody::Truncated(body) | CapturedBody::Full(body) => Some(body.clone()),
            CapturedBody::Failed(msg) => Some(format!("<capture failed: {msg}>")),
        }
    }
}

async fn capture_request_body(request: Request) -> CapturedBody {
    let (_parts, body) = request.into_parts();
    match to_bytes(body, MAX_CAPTURED_BODY_BYTES * 2).await {
        Ok(bytes) if bytes.is_empty() => CapturedBody::None,
        Ok(bytes) => {
            let cap = MAX_CAPTURED_BODY_BYTES.min(bytes.len());
            let truncated = bytes.len() > MAX_CAPTURED_BODY_BYTES;
            let slice = &bytes[..cap];
            let body = String::from_utf8_lossy(slice).into_owned();
            if truncated {
                CapturedBody::Truncated(body + "…[truncated]")
            } else {
                CapturedBody::Full(body)
            }
        }
        Err(err) => CapturedBody::Failed(err.to_string()),
    }
}

fn seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64 ^ 0xA24BAED4963EE407)
        .unwrap_or(0xD1B54A32D192ED03)
}
