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

## Status code overrides

`GatewayConfig.status_overrides: Map<METHOD /path, u16>` lets the user
pick the exact HTTP status emitted for a matched route instead of the
default derived from the example kind (200 for success/empty, 400 for
error). Out-of-range codes (outside 100–599) silently fall back to the
kind default so a bad config never strands a route. When an override
fires, the log entry's `source` becomes `status-override` so the UI can
show why a non-standard code was returned. Common uses: `201 Created`
for POST handlers, `204 No Content` for DELETE, `403 Forbidden` for a
differentiated auth failure.

## Response headers

Always emitted:

- `content-type: application/json`
- `x-request-id: <id>` on every response (including 401/429/404 error
  paths). Honored from the client's `x-request-id` header when present,
  otherwise generated server-side as a v4-shaped lowercase UUID. The id
  is also recorded on the `RequestLogEntry` so UI rows, log exports, and
  external traces share one correlation key. Client-supplied ids are
  trimmed and clamped to 128 characters so a malicious header can't
  grow the log unboundedly.
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

`GatewayConfig.latency_jitter_ms` (also `METHOD /path → u64`) adds a
uniform ± bound to the resolved latency on every request. For a route
with `base=50ms` and `jitter=20ms`, each hit sleeps in `[30ms, 70ms]`.
Saturates at zero so the overall sleep never goes negative, regardless
of the sampled delta. Zero-valued entries and missing keys are no-ops
on the hot path — routes without jitter bypass the RNG entirely.

## Response templating

Mock payloads can embed `{{ }}` tokens that the gateway expands on every
request, before serializing to JSON:

- `{{now}}` — current UTC time, RFC 3339 (`2026-04-24T12:34:56Z`).
- `{{now.epoch_ms}}` — current time in milliseconds since the Unix epoch.
- `{{uuid}}` — v4-shaped lowercase UUID (not cryptographic).
- `{{random.int}}` — random integer in `0..1_000_000`.
- `{{random.int.<max>}}` — random integer in `0..<max>`.
- `{{path.<name>}}` — value of the matched path parameter (empty string
  when absent).
- `{{env.<NAME>}}` — value of the named environment variable, read at
  request time. Expands to an empty string when the var is unset.
  **Secrets are redacted**: names whose uppercase form contains any of
  `SECRET`, `PASSWORD`, `PRIVATE_KEY`, `API_KEY`, `APIKEY`, `TOKEN`,
  `COOKIE`, `AUTH`, or `CREDENTIAL` always expand to an empty string,
  even when explicitly set — the gateway will not leak credentials
  through a mock payload, no matter how the template was written.

The substitution walks every string leaf of the JSON payload. Non-string
values pass through untouched. Unknown tokens are left in place as
`{{unknown.token}}` so users can diagnose typos instead of getting
silent empty strings. Nine unit tests in
`crates/albert-gateway/src/templating.rs` lock in the token set plus the
RFC 3339 formatter.

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

## Required headers (auth simulation)

`GatewayConfig.required_headers` is a `METHOD /path → Vec<RequiredHeader>`
map. Each rule is `{ name, value_prefix?, value_equals? }`:

- `name` — header name (case-insensitive on the wire).
- `value_prefix` — request header value must start with this string. Use
  for things like `Bearer ` or `Basic `.
- `value_equals` — request header value must match exactly. Combine with
  `value_prefix` when both need to hold.
- Empty `value_prefix` + `value_equals` means "presence only".

If any rule fails the gateway returns `401 Unauthorized` with a JSON body
`{error: "unauthorized", message: "<reason>"}` and records the request
with `source: "auth-required"`. The gate runs **before** example selection
so unauthorized requests never touch mock data and never incur latency.

## Rate limiting

`GatewayConfig.rate_limits` is a `METHOD /path → RateLimitRule` map. Each
rule is `{ limit: u32, window_ms: u64 }` — "at most `limit` requests per
`window_ms` milliseconds per route." Semantics:

- Sliding window, per route. Every admitted hit pushes a timestamp onto a
  `VecDeque<u128>`; expired entries are popped on the next evaluation.
- `limit: 0` is an explicit deny-all — useful for simulating a maintenance
  window. Every request is rejected until the rule is removed.
- On rejection the gateway emits `429 Too Many Requests` with:
  - `Retry-After: <seconds>` — the rolling-window residual, rounded up to
    at least one second so clients can't poll before the slot opens.
  - `x-albert-rate-limit: <limit>` / `x-albert-rate-window-ms: <window_ms>`
    echoing the rule that fired.
  - JSON body `{error: "rate_limited", limit, window_ms, retry_after_ms}`.
- The log entry records `source: "rate-limited"` and `status: 429`.
- The gate runs **after** the required-header gate but **before** example
  selection / latency injection, so denied requests never touch mock data
  and never sleep.
- Reconfiguring rules via `MockGateway::reconfigure` (or `update_mock_server`)
  preserves the rolling history for routes that keep an entry, so a
  tightened rule starts applying immediately against the in-flight window
  instead of resetting the counter.

## Error-rate injection

`GatewayConfig.error_rate` (0.0 – 1.0, clamped) is the probability that a
matched request is served its error example instead of the selected one.
The roll uses a zero-dependency thread-local LCG seeded from the monotonic
clock. Log entries tagged with `source: "error-rate"` indicate the
injection fired. A rate of `0.0` disables the behavior completely; `1.0`
always serves the error example.

## Special routes

- `GET /__albert/status` returns `{service, route_count}`.
- `GET /__albert/routes` returns `{routes: [{method, path}, ...]}` — the
  compiled route table without payload data. Used by the CLI `verify`
  subcommand to enumerate what to probe.
- `GET /__albert/config` returns the full live gateway config as JSON
  — `{route_count, overrides, default_latency_ms, latency_overrides,
  latency_jitter_ms, error_rate, capture_bodies, response_headers,
  required_headers, rate_limits, status_overrides}`. Read-only; any
  mutation still goes through the desktop panel or `update_mock_server`
  Tauri command. The CLI `albert config --url …` is a thin wrapper that
  pretty-prints this payload.
- `GET /__albert/metrics` returns a `MetricsSnapshot`:
  `{total_requests, by_method, by_status_class, average_latency_ms,
    max_latency_ms, started_at_epoch_ms, uptime_ms, by_route}`.
  Incremented on every mock_handler call (not on hits to `/__albert/*`
  itself). The `by_route` map is keyed on `METHOD /path` and contains
  `{count, total_latency_ms, average_latency_ms, max_latency_ms,
    p50_ms, p95_ms}` per route. Percentiles are nearest-rank over a
  bounded reservoir of the 200 most recent samples, so a hot route
  can't make the snapshot grow unbounded. Requests that don't match a
  registered route (404 / unsupported method) don't appear in
  `by_route` — it stays a faithful picture of declared-route traffic.
  Also exposed from the desktop host via the `mock_server_metrics`
  Tauri command.
- `404` responses are JSON: `{error: "mock_not_found", message}`.
- `HEAD` requests fall back to matching the `GET` route with the same
  path; the body is then suppressed so the response stays well-formed.
  This lets health-check probes succeed without having to declare HEAD
  explicitly.
- Trailing slashes are ignored during matching — `/users` and `/users/`
  resolve to the same route.
- `OPTIONS` preflight requests are handled by the `CorsLayer` when
  `cors_enabled` is `true` and echo permissive CORS headers.

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
