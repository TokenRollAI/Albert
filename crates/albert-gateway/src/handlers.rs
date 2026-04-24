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
use crate::state::{AppState, RateVerdict, RequestLogEntry};
use crate::templating::apply_templates;

pub(crate) async fn status_handler(State(state): State<AppState>) -> Response {
    let table = state.snapshot_table();
    let payload = serde_json::json!({
        "service": "albert-mock-gateway",
        "route_count": table.len(),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

pub(crate) async fn routes_handler(State(state): State<AppState>) -> Response {
    let table = state.snapshot_table();
    let payload = serde_json::json!({
        "routes": table
            .route_pairs()
            .into_iter()
            .map(|(method, path)| serde_json::json!({
                "method": method.as_str(),
                "path": path,
            }))
            .collect::<Vec<_>>(),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

/// Return the full live `GatewayConfig` as JSON. Useful for quickly
/// seeing what's loaded without having to restart the server or check
/// the desktop UI — the CLI's `albert config --url …` command is a
/// thin wrapper around this endpoint. Consumers: UI, CLI, shell
/// scripts. Stays outside the `/__albert/routes` flow so automation
/// can filter routes vs. config independently.
/// Emit the live collections as an OpenAPI 3.0 document. The running
/// bind address is not known at handler time (axum doesn't expose it
/// cheaply), so the caller can pass `?base=<url>` to override what
/// appears in the `servers` array. When absent, `servers` is omitted —
/// downstream tools then fall back to the URL the user typed in.
pub(crate) async fn openapi_handler(State(state): State<AppState>, req: Request) -> Response {
    let collections = state.snapshot_collections();
    let base = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("base=")))
        .and_then(|raw| urldecode(raw).ok());
    let doc = crate::openapi::to_openapi_document(&collections, base.as_deref());
    (StatusCode::OK, axum::Json(doc)).into_response()
}

fn urldecode(input: &str) -> Result<String, String> {
    // Tiny percent-decoder — the query string from the axum router is
    // already split on `&`, so we only need to handle `%NN` escapes.
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| "invalid percent escape")?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| "bad hex")?;
                out.push(byte);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}

/// `GET /__albert/config/bundle` — return the full portable
/// `GatewayConfigBundle` shape (same one `MockGateway::export_bundle`
/// produces). Gives the CLI a shell-friendly path for snapshotting
/// live configs without going through Tauri.
pub(crate) async fn bundle_export_handler(State(state): State<AppState>) -> Response {
    let collections = state.snapshot_collections();
    let collection_ids: Vec<String> = collections.iter().map(|c| c.id.clone()).collect();
    let overrides = state.snapshot_overrides();
    let latency = state.snapshot_latency();
    let response_headers = state.snapshot_response_headers();
    let required_headers = state.snapshot_required_headers();
    let status_overrides = state.snapshot_status_overrides();
    let error_rate = state.snapshot_error_rate();
    let capture_bodies = state.snapshot_capture_bodies();
    let enforce_request_bodies = state.snapshot_enforce_request_bodies();
    let rate_limits = state.snapshot_rate_limit_rules();
    let table = state.snapshot_table();
    // Rebuild a `GatewayConfig`-shaped JSON; host/port/cors_enabled
    // aren't fully recoverable from runtime state (host is stored in
    // the Listener address, not a mutex slot), so we default them —
    // they're the Listener tab's responsibility and won't be part of
    // what import replays. The bundle's config is only read for the
    // enforcement knobs the gateway owns.
    let payload = serde_json::json!({
        "bundle_version": crate::config::GatewayConfigBundle::CURRENT_VERSION,
        "config": {
            "host": "127.0.0.1",
            "port": 0,
            "cors_enabled": true,
            "example_overrides": &*overrides,
            "default_latency_ms": latency.default_ms,
            "latency_overrides": latency.per_route,
            "latency_jitter_ms": latency.jitter_per_route,
            "error_rate": error_rate,
            "capture_bodies": capture_bodies,
            "enforce_request_bodies": enforce_request_bodies,
            "response_headers": &*response_headers,
            "required_headers": &*required_headers,
            "rate_limits": rate_limits,
            "status_overrides": &*status_overrides,
        },
        "collection_ids": collection_ids,
        "_route_count": table.len(),
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

/// `POST /__albert/config/bundle` — body is
/// `{bundle: GatewayConfigBundle, collections: CanonicalApiCollection[]}`.
/// The gateway cannot read SQLite itself, so the caller (CLI) resolves
/// collection IDs first and sends the full canonical collections
/// inline. Returns `204 No Content` on success, `400` with a
/// `{error, message}` JSON on malformed input or a bundle-version
/// mismatch.
pub(crate) async fn bundle_import_handler(
    State(_state): State<AppState>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> Response {
    // We can't call `MockGateway::import_bundle` from inside a handler
    // (the handler holds an AppState clone, not the owning gateway
    // wrapper). So we unpack the payload here and apply via the same
    // `replace_*` pairs the regular reconfigure uses, reusing the
    // gateway's AppState slot-based swapping.
    //
    // (An earlier draft routed through `MockGateway::import_bundle` via
    // a side channel; the indirection didn't pay for itself.)
    let Some(obj) = payload.as_object() else {
        return bundle_bad_request("payload must be an object");
    };
    let Some(bundle_val) = obj.get("bundle") else {
        return bundle_bad_request("payload missing `bundle`");
    };
    let Some(collections_val) = obj.get("collections") else {
        return bundle_bad_request("payload missing `collections`");
    };
    let bundle: crate::config::GatewayConfigBundle =
        match serde_json::from_value(bundle_val.clone()) {
            Ok(b) => b,
            Err(err) => return bundle_bad_request(&format!("bundle parse: {err}")),
        };
    let collections: Vec<albert_core::CanonicalApiCollection> =
        match serde_json::from_value(collections_val.clone()) {
            Ok(c) => c,
            Err(err) => return bundle_bad_request(&format!("collections parse: {err}")),
        };
    let expected_major = crate::config::GatewayConfigBundle::CURRENT_VERSION
        .split('.')
        .next()
        .unwrap_or("0");
    let major = bundle.bundle_version.split('.').next().unwrap_or("0");
    if major != expected_major {
        return bundle_bad_request(&format!(
            "bundle version {} is not compatible with gateway {}",
            bundle.bundle_version,
            crate::config::GatewayConfigBundle::CURRENT_VERSION
        ));
    }
    // Apply state slot swaps — same sequence as `MockGateway::reconfigure`
    // minus the `running.config` mirror (handler has no access to that).
    let table = std::sync::Arc::new(crate::routing::RouteTable::from_routes(
        crate::route::build_routes(&collections),
    ));
    _state.replace_table(table);
    _state.replace_overrides(std::sync::Arc::new(bundle.config.example_overrides.clone()));
    _state.replace_latency(crate::state::LatencyConfig::new(
        bundle.config.default_latency_ms,
        bundle.config.latency_overrides.clone(),
        bundle.config.latency_jitter_ms.clone(),
    ));
    _state.replace_error_rate(bundle.config.error_rate);
    _state.replace_capture_bodies(bundle.config.capture_bodies);
    _state.replace_enforce_request_bodies(bundle.config.enforce_request_bodies);
    _state.replace_response_headers(std::sync::Arc::new(bundle.config.response_headers.clone()));
    _state.replace_required_headers(std::sync::Arc::new(bundle.config.required_headers.clone()));
    _state.replace_rate_limits(bundle.config.rate_limits.clone());
    _state.replace_status_overrides(std::sync::Arc::new(bundle.config.status_overrides.clone()));
    _state.replace_collections(std::sync::Arc::new(collections));
    (StatusCode::NO_CONTENT, HeaderMap::new()).into_response()
}

fn bundle_bad_request(message: &str) -> Response {
    let payload = serde_json::json!({
        "error": "bundle_invalid",
        "message": message,
    });
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(payload),
    )
        .into_response()
}

pub(crate) async fn config_handler(State(state): State<AppState>) -> Response {
    let table = state.snapshot_table();
    let overrides = state.snapshot_overrides();
    let latency = state.snapshot_latency();
    let response_headers = state.snapshot_response_headers();
    let required_headers = state.snapshot_required_headers();
    let status_overrides = state.snapshot_status_overrides();
    let error_rate = state.snapshot_error_rate();
    let capture_bodies = state.snapshot_capture_bodies();
    let rate_limits = state.snapshot_rate_limit_rules();
    let payload = serde_json::json!({
        "route_count": table.len(),
        "overrides": &*overrides,
        "default_latency_ms": latency.default_ms,
        "latency_overrides": latency.per_route,
        "latency_jitter_ms": latency.jitter_per_route,
        "error_rate": error_rate,
        "capture_bodies": capture_bodies,
        "response_headers": &*response_headers,
        "required_headers": &*required_headers,
        "rate_limits": rate_limits,
        "status_overrides": &*status_overrides,
    });
    (StatusCode::OK, axum::Json(payload)).into_response()
}

pub(crate) async fn metrics_handler(State(state): State<AppState>) -> Response {
    let metrics = state.snapshot_metrics();
    let by_route: serde_json::Map<String, serde_json::Value> = metrics
        .by_route
        .iter()
        .map(|(key, rm)| {
            (
                key.clone(),
                serde_json::json!({
                    "count": rm.count,
                    "total_latency_ms": rm.total_latency_ms,
                    "average_latency_ms": rm.average_latency_ms(),
                    "max_latency_ms": rm.max_latency_ms,
                    "p50_ms": rm.p50_ms,
                    "p95_ms": rm.p95_ms,
                }),
            )
        })
        .collect();
    let payload = serde_json::json!({
        "total_requests": metrics.total_requests,
        "by_method": metrics.by_method,
        "by_status_class": metrics.by_status_class,
        "average_latency_ms": metrics.average_latency_ms(),
        "max_latency_ms": metrics.max_latency_ms,
        "started_at_epoch_ms": metrics.started_at_epoch_ms,
        "uptime_ms": epoch_ms_now().saturating_sub(metrics.started_at_epoch_ms),
        "by_route": by_route,
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
    let request_id = resolve_request_id(&request_headers);
    // Capture the body whenever either `capture_bodies` is on (for the
    // log) or `enforce_request_bodies` is on (for the validator). Doing
    // both under the same path keeps the request stream consumed at
    // most once — axum's Body can't be re-read after capture.
    let enforce_bodies = state.snapshot_enforce_request_bodies();
    let wants_body =
        (capture_bodies || enforce_bodies) && method != Method::GET && method != Method::HEAD;
    let captured_body = if wants_body {
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
            request_id: Some(request_id.clone()),
        });
        return not_found(format!("unsupported method {method}"), Some(&request_id));
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
                    request_id: Some(request_id.clone()),
                });
                return not_found(
                    format!("no mock registered for {} {}", method.as_str(), path),
                    Some(&request_id),
                );
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
                request_id: Some(request_id.clone()),
            });
            return not_found(
                format!("no mock registered for {} {}", method.as_str(), path),
                Some(&request_id),
            );
        }
    };
    let route = matched.route;
    let matched_key = route_key(&route.method, &route.path);
    let path_params = matched.params.clone();
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
            request_id: Some(request_id.clone()),
        });
        return unauthorized(err, &request_id);
    }

    // Rate-limit gate: runs after auth but before example selection so a
    // rejected request never incurs configured latency or consumes a mock.
    let now_ms = epoch_ms_now() as u128;
    if let RateVerdict::Denied {
        limit,
        window_ms,
        retry_after_ms,
    } = state.admit_request(&matched_key, now_ms)
    {
        state.record(RequestLogEntry {
            at_epoch_ms: epoch_ms_now(),
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            matched_route: Some(matched_key.clone()),
            collection_name: Some(route.collection_name.clone()),
            status: 429,
            kind: None,
            source: "rate-limited",
            latency_ms: 0,
            request_body: captured_string.clone(),
            request_id: Some(request_id.clone()),
        });
        return too_many_requests(limit, window_ms, retry_after_ms, &request_id);
    }

    // Opt-in request-body validation. Runs after all the cheap gates so
    // denied requests short-circuit without wasting cycles on JSON parse;
    // and before example selection so a malformed body never touches a
    // mock payload. Only POST/PUT/PATCH bodies can fail this check —
    // GET/HEAD never carry one.
    if enforce_bodies
        && wants_body
        && let Some(schema) = route.request_body_schema.as_ref()
        && let Some(body_str) = captured_body.raw_bytes_as_str()
    {
        match serde_json::from_str::<serde_json::Value>(body_str) {
            Ok(parsed) => {
                if let Err(violation) = crate::validator::validate(&parsed, schema) {
                    state.record(RequestLogEntry {
                        at_epoch_ms: epoch_ms_now(),
                        method: method.to_string(),
                        path: path.clone(),
                        query: query.clone(),
                        matched_route: Some(matched_key.clone()),
                        collection_name: Some(route.collection_name.clone()),
                        status: 400,
                        kind: None,
                        source: "schema-mismatch",
                        latency_ms: 0,
                        request_body: captured_string.clone(),
                        request_id: Some(request_id.clone()),
                    });
                    return schema_mismatch(violation, &request_id);
                }
            }
            Err(parse_err) => {
                state.record(RequestLogEntry {
                    at_epoch_ms: epoch_ms_now(),
                    method: method.to_string(),
                    path: path.clone(),
                    query: query.clone(),
                    matched_route: Some(matched_key.clone()),
                    collection_name: Some(route.collection_name.clone()),
                    status: 400,
                    kind: None,
                    source: "schema-mismatch",
                    latency_ms: 0,
                    request_body: captured_string.clone(),
                    request_id: Some(request_id.clone()),
                });
                return schema_mismatch(
                    crate::validator::ValidationError {
                        path: "$".to_string(),
                        message: format!("body is not valid JSON: {parse_err}"),
                    },
                    &request_id,
                );
            }
        }
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
            request_id: Some(request_id.clone()),
        });
        return not_found(
            format!("no example configured for {} {}", method.as_str(), path),
            Some(&request_id),
        );
    };

    let base_latency = latency.resolve(&matched_key);
    let jitter = latency.jitter_for(&matched_key);
    // Draw uniform ±jitter around the base. The existing thread-local LCG
    // is reused (via roll_jitter below) to avoid pulling in a second RNG.
    let jitter_delta = roll_jitter(jitter);
    let latency_ms = (base_latency as i64 + jitter_delta).max(0) as u64;
    if latency_ms > 0 {
        sleep(Duration::from_millis(latency_ms)).await;
    }

    let default_status: u16 = match example.kind {
        MockExampleKind::Success | MockExampleKind::Empty => 200,
        MockExampleKind::Error => 400,
    };
    let status_overrides = state.snapshot_status_overrides();
    let overridden_status = status_overrides
        .get(&matched_key)
        .copied()
        .filter(|code| (100..=599).contains(code));
    let status_applied_override = overridden_status.is_some();
    let status_line_code: u16 = overridden_status.unwrap_or(default_status);
    let source = if query_selected {
        "query"
    } else if error_injected {
        "error-rate"
    } else if status_applied_override {
        "status-override"
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
        request_id: Some(request_id.clone()),
    });

    // Apply response templating (e.g. {{now}}, {{path.id}}) before we
    // serialize — substitution is a string-leaf walk over the JSON value
    // so opting out just means not using `{{ }}` in the payload.
    let templated_payload = apply_templates(&example.payload, &path_params);
    let templated_example = MockExample {
        payload: templated_payload,
        ..example.clone()
    };
    let (status, body) = render_example(&templated_example, overridden_status);
    let body = if strip_body { Body::empty() } else { body };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(name) = HeaderName::from_bytes(b"x-request-id")
        && let Ok(value) = HeaderValue::from_str(&request_id)
    {
        headers.insert(name, value);
    }
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

fn render_example(example: &MockExample, override_status: Option<u16>) -> (StatusCode, Body) {
    let fallback = match example.kind {
        MockExampleKind::Success | MockExampleKind::Empty => StatusCode::OK,
        MockExampleKind::Error => StatusCode::BAD_REQUEST,
    };
    let status = override_status
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap_or(fallback);
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

/// Honor the client's `x-request-id` when provided; otherwise fabricate
/// a UUIDish id server-side. The returned string is always non-empty and
/// is what the log entry, response header, and chaos paths all use as a
/// single correlation id for the request.
fn resolve_request_id(request_headers: &[(String, String)]) -> String {
    for (name, value) in request_headers {
        if name == "x-request-id" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                // Keep the value length bounded so a malicious client
                // can't blow up the log.
                const MAX_ID_LEN: usize = 128;
                if trimmed.len() > MAX_ID_LEN {
                    return trimmed.chars().take(MAX_ID_LEN).collect();
                }
                return trimmed.to_string();
            }
        }
    }
    crate::templating::fake_uuid_v4()
}

fn schema_mismatch(violation: crate::validator::ValidationError, request_id: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(name) = HeaderName::from_bytes(b"x-request-id")
        && let Ok(value) = HeaderValue::from_str(request_id)
    {
        headers.insert(name, value);
    }
    let payload = serde_json::json!({
        "error": "schema_mismatch",
        "path": violation.path,
        "message": violation.message,
        "request_id": request_id,
    });
    (StatusCode::BAD_REQUEST, headers, axum::Json(payload)).into_response()
}

fn unauthorized(reason: String, request_id: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(name) = HeaderName::from_bytes(b"x-request-id")
        && let Ok(value) = HeaderValue::from_str(request_id)
    {
        headers.insert(name, value);
    }
    let payload = serde_json::json!({
        "error": "unauthorized",
        "message": reason,
        "request_id": request_id,
    });
    (StatusCode::UNAUTHORIZED, headers, axum::Json(payload)).into_response()
}

fn too_many_requests(
    limit: u32,
    window_ms: u64,
    retry_after_ms: u64,
    request_id: &str,
) -> Response {
    // Retry-After is specified in whole seconds. Round up so clients never
    // poll before the rolling window actually opens.
    let retry_after_secs = retry_after_ms.div_ceil(1000).max(1);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
        headers.insert(header::RETRY_AFTER, value);
    }
    if let Ok(name) = HeaderName::from_bytes(b"x-request-id")
        && let Ok(value) = HeaderValue::from_str(request_id)
    {
        headers.insert(name, value);
    }
    if let Ok(name) = HeaderName::from_bytes(b"x-albert-rate-limit")
        && let Ok(value) = HeaderValue::from_str(&limit.to_string())
    {
        headers.insert(name, value);
    }
    if let Ok(name) = HeaderName::from_bytes(b"x-albert-rate-window-ms")
        && let Ok(value) = HeaderValue::from_str(&window_ms.to_string())
    {
        headers.insert(name, value);
    }
    let payload = serde_json::json!({
        "error": "rate_limited",
        "limit": limit,
        "window_ms": window_ms,
        "retry_after_ms": retry_after_ms,
        "request_id": request_id,
    });
    (StatusCode::TOO_MANY_REQUESTS, headers, axum::Json(payload)).into_response()
}

pub(crate) fn not_found(message: String, request_id: Option<&str>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Some(id) = request_id
        && let Ok(name) = HeaderName::from_bytes(b"x-request-id")
        && let Ok(value) = HeaderValue::from_str(id)
    {
        headers.insert(name, value);
    }
    let payload = serde_json::json!({
        "error": "mock_not_found",
        "message": message,
        "request_id": request_id,
    });
    (StatusCode::NOT_FOUND, headers, axum::Json(payload)).into_response()
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

/// Draw a uniform integer delta in `[-bound, +bound]` for latency jitter,
/// using the same LCG thread-local stream as `roll_probability`. Returns
/// 0 when `bound` is 0, which is the hot path for routes without jitter.
fn roll_jitter(bound: u64) -> i64 {
    use std::cell::Cell;
    if bound == 0 {
        return 0;
    }
    thread_local! {
        static STATE: Cell<u64> = Cell::new(seed());
    }
    STATE.with(|slot| {
        let mut x = slot.get();
        if x == 0 {
            x = seed();
        }
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        slot.set(x);
        let span = bound.saturating_mul(2).saturating_add(1);
        // (x >> 32) gives 32 uniform bits, plenty of range for sane bounds.
        let pick = ((x >> 32) % span) as i64;
        pick - bound as i64
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

    /// Return just the literal body bytes (as UTF-8 text), without the
    /// truncation marker that `as_string` appends. Used by the validator
    /// so the JSON parser doesn't choke on our sentinel `…[truncated]`
    /// footer. `None` when the body wasn't captured or capture failed.
    pub(crate) fn raw_bytes_as_str(&self) -> Option<&str> {
        match self {
            CapturedBody::None | CapturedBody::Failed(_) => None,
            CapturedBody::Full(body) => Some(body.as_str()),
            // Strip the "…[truncated]" sentinel we appended in
            // capture_request_body so JSON parsing still works for
            // bodies that crossed the 4KB cap.
            CapturedBody::Truncated(body) => {
                let sentinel = "…[truncated]";
                Some(body.strip_suffix(sentinel).unwrap_or(body.as_str()))
            }
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
