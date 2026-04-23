# Mock Gateway Routes

## Runtime shape

- Crate: `albert-gateway` (`axum` + `hyper` via `tokio`).
- Modules:
  - `config` — `GatewayConfig`, `GatewayStatus`, `GatewayRouteSummary`,
    capability surface.
  - `error` — `GatewayError`.
  - `route` — `MockRoute`, `MatchedRoute`, `route_key`, `build_routes`.
  - `routing` — `RouteTable`, `CompiledRoute`, path template compilation.
  - `state` — `AppState`, `LatencyConfig`, `RequestLogEntry`.
  - `handlers` — axum handlers (`status_handler`, `mock_handler`) plus
    `parse_query_override`, `not_found`, `epoch_ms_now`.
  - `lib` — public `MockGateway`, `start`/`stop`/`update`/`reconfigure` +
    tests.
- Shutdown: `MockGateway::stop()` sends a oneshot signal; axum uses
  `with_graceful_shutdown`, then the spawned task is awaited.
- Shared with handlers via `AppState` (three `StdMutex`-guarded slots: the
  route table, the overrides map, the latency config) + a request log
  `VecDeque` bounded to 100 entries.

## Route matching

- Path template syntax accepted from canonical endpoints: `/{name}` placeholders.
- Colon-prefixed segments (`:name`) are also recognized for future-proofing.
- `RouteTable::from_routes` sorts routes by:
  1. number of literal segments (desc)
  2. segment count (desc)
  3. lexicographic path (asc)
  so `/users/me` beats `/users/{id}` on the same method.
- `match_route` returns the first candidate whose method matches and all
  segments are equal count + literal match (params capture any value).
- No wildcards beyond segment-scoped params (no `*` catch-all yet).

## Example selection

Order of precedence:

1. Request query override: `?__albert_mock=success|empty|error`.
2. Per-route override in `GatewayConfig.example_overrides`
   (key: `"METHOD /path"`).
3. The endpoint's `success` example.
4. Any available example (first one).

Status code mapping:

- `MockExampleKind::Success` → 200
- `MockExampleKind::Empty` → 200
- `MockExampleKind::Error` → 400

## Response headers

Always emitted:

- `content-type: application/json`
- `x-albert-mock-kind: success | empty | error`
- `x-albert-mock-route: METHOD /path`
- `x-albert-mock-source: query` when a query override was honored
- `x-albert-mock-latency-ms: <n>` when latency injection delayed the response

Per-route extras are configurable via
`GatewayConfig.response_headers: BTreeMap<String, BTreeMap<String, String>>`
keyed by `METHOD /path`. Invalid header names or values are silently
skipped rather than causing the whole response to fail — the gateway
favors serving something over serving nothing.

## Latency injection

`GatewayConfig.default_latency_ms` adds a fixed delay to every served route.
`GatewayConfig.latency_overrides` is a `METHOD /path → u64` map that is
added on top of the default. Delays are applied after route matching and
example selection but before returning the response body. The total
effective delay is echoed in the `x-albert-mock-latency-ms` header and
the request log's `latency_ms` field.

## Request body capture

`GatewayConfig.capture_bodies` (default `false`). When `true`, the handler
reads the body before dispatch (`axum::body::to_bytes` with an 8KB hard
cap), truncates to the first 4KB, and records a UTF-8 best-effort string
into `RequestLogEntry.request_body`. Failures surface as
`"<capture failed: …>"` so the log remains faithful even for binary
payloads or over-large requests.

GET and HEAD requests always skip capture. The flag can be toggled live
via `MockGateway::reconfigure` (or the Mock Server drawer / CLI
`--capture-bodies`).

## Error-rate injection

`GatewayConfig.error_rate` (0.0 – 1.0, clamped) is the probability that a
matched request is served its error example instead of the selected one.
The roll uses a zero-dependency thread-local LCG seeded from the monotonic
clock. Log entries tagged with `source: "error-rate"` indicate the
injection fired. A rate of `0.0` disables the behavior completely; `1.0`
always serves the error example.

## Special routes

- `GET /__albert/status` returns `{service, route_count}`.
- `404` responses are JSON: `{error: "mock_not_found", message}`.

## CORS

- `CorsLayer::permissive()` is attached when `GatewayConfig.cors_enabled` is true.
- The default config enables CORS so browser clients can hit the mock during development.

## Hot reload + observability

- `MockGateway::update(collections, overrides)` swaps the route table and
  override map of a running server without releasing the port — useful when
  importing a new collection or flipping example kinds from the UI.
- `MockGateway::reconfigure(collections, overrides, default_latency, latency_overrides)`
  also rewrites the latency config in-place.
- `MockGateway::recent_requests(limit)` returns up to 100 of the most recent
  entries (newest first). Each entry captures timestamp, method, path, query,
  the matched route key, status, served `MockExampleKind`, a `source`
  label (`default | override | query | unmatched | unsupported | no-example`),
  and the `latency_ms` injected.
- Exposed via Tauri commands `update_mock_server` and `mock_server_requests`.

## Tests

- Unit tests in `crates/albert-gateway/src/lib.rs` and `.../routing.rs`
  cover routing, double-start rejection, request log capture, and hot
  reload via `update(...)`.
- Integration test in `crates/albert-gateway/tests/end_to_end.rs` exercises
  parse → persist → reload → serve with real TCP on an ephemeral port.
