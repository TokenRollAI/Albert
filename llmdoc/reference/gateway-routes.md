# Mock Gateway Routes

## Runtime shape

- Crate: `albert-gateway` (`axum` + `hyper` via `tokio`).
- Entry point: `MockGateway::start(collections, GatewayConfig)`.
- Shutdown: `MockGateway::stop()` sends a oneshot signal; axum uses
  `with_graceful_shutdown`, then the spawned task is awaited.
- Shared with handlers via `AppState { table: Arc<RouteTable>, overrides }`.

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

- `content-type: application/json`
- `x-albert-mock-kind: success | empty | error`
- `x-albert-mock-route: METHOD /path`
- `x-albert-mock-source: query` when a query override was honored

## Special routes

- `GET /__albert/status` returns `{service, route_count}`.
- `404` responses are JSON: `{error: "mock_not_found", message}`.

## CORS

- `CorsLayer::permissive()` is attached when `GatewayConfig.cors_enabled` is true.
- The default config enables CORS so browser clients can hit the mock during development.

## Tests

- Unit tests in `crates/albert-gateway/src/lib.rs` and `.../routing.rs`.
- Integration test in `crates/albert-gateway/tests/end_to_end.rs` exercises
  parse → persist → reload → serve with real TCP on an ephemeral port.
