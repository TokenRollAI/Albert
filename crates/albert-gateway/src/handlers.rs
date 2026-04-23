//! Axum handlers + HTTP helpers for the mock gateway.

use std::time::Duration;

use albert_core::{HttpMethod, MockExample, MockExampleKind};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::time::sleep;

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

pub(crate) async fn mock_handler(State(state): State<AppState>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());

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
        });
        return not_found(format!("unsupported method {method}"));
    };
    let table = state.snapshot_table();
    let overrides = state.snapshot_overrides();
    let latency = state.snapshot_latency();
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
            latency_ms: 0,
        });
        return not_found(format!(
            "no mock registered for {} {}",
            method.as_str(),
            path
        ));
    };
    let route = matched.route;
    let matched_key = route_key(&route.method, &route.path);

    let (override_kind, query_selected) = parse_query_override(query.as_deref());
    let fallback_override = overrides.get(&matched_key).cloned();
    let chosen_override = override_kind.clone().or(fallback_override.clone());
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
