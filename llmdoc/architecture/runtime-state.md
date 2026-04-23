# Runtime State

## Purpose

Describe shared runtime services inside the Tauri host and how they cross the
async boundary into axum / reqwest.

## Shared services

- `AppServices { gateway: Arc<MockGateway> }` is registered via
  `tauri::Builder::manage(...)` in `apps/desktop/src-tauri/src/lib.rs`.
- The gateway is constructed once at app boot and reused across Tauri commands
  because its internal `Mutex<Option<RunningGateway>>` is the source of truth
  for "is the mock running".

## Tokio

- The Tauri runtime uses tokio; commands declared `async fn` run on that same
  runtime, so `MockGateway::start` can await `TcpListener::bind` inline.
- The spawned axum server uses `tokio::spawn` with
  `with_graceful_shutdown(async move { let _ = shutdown_rx.await; })`.
- `reqwest::Client` inside `OpenAiChatAdapter` builds per call right now; swap
  for a long-lived pooled client if call rate grows.

## Storage ownership

- `SqliteStore` is a thin wrapper around a path. Each command builds a fresh
  instance; connections are short-lived. `migrate()` is called defensively on
  every command to keep schema and app in sync during development.
- The mock gateway does NOT query SQLite at request time — it gets fully
  resolved collections when `start_mock_server` calls
  `load_all_collections` / `load_collection`. A restart is required to pick up
  newly imported collections; this is intentional to keep the data plane
  immutable while serving.

## Error boundary

- All Tauri commands convert Rust errors to `String` via `map_err(|e| e.to_string())`.
- Gateway / adapter errors use `thiserror` and preserve the underlying I/O
  error via `#[source]` so higher layers can format consistent messages.
