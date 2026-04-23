# Runtime State

## Purpose

Describe shared runtime services inside the Tauri host and how they cross the
async boundary into axum / reqwest.

## Shared services

- `AppServices { gateway: Arc<MockGateway> }` lives at
  `apps/desktop/src-tauri/src/services.rs` and is registered via
  `tauri::Builder::manage(...)` in `apps/desktop/src-tauri/src/lib.rs`.
- The gateway is constructed once at app boot and reused across Tauri commands
  because its internal `Mutex<Option<RunningGateway>>` is the source of truth
  for "is the mock running".

## Command surface

Tauri commands are split into focused modules under
`apps/desktop/src-tauri/src/commands/`:
- `bootstrap` — `bootstrap_summary`, `default_gateway_config`,
  `supported_http_methods`.
- `parser` — parse / import / list / load-snapshot.
- `gateway` — start / stop / status / requests / update.
- `openai` — `generate_mock_example`, `preview_generation_prompt`.

`lib.rs` only wires the Tauri builder; modules are referenced by full path
in `generate_handler!` because `#[tauri::command]` generates a companion
`__cmd__<name>` shadow item that isn't reachable through a `pub use`.

## Tokio

- The Tauri runtime uses tokio; commands declared `async fn` run on that same
  runtime, so `MockGateway::start` can await `TcpListener::bind` inline.
- The spawned axum server uses `tokio::spawn` with
  `with_graceful_shutdown(async move { let _ = shutdown_rx.await; })`.
- `reqwest::Client` inside `OpenAiChatAdapter` builds per call right now; swap
  for a long-lived pooled client if call rate grows.

## Gateway preferences persistence

- A dedicated `gateway_preferences` SQLite table stores a single JSON
  payload keyed on `"singleton"`. The frontend owns the shape so new
  preferences can be added without a schema migration.
- `SqliteStore::save_gateway_preferences(&Value)` and
  `load_gateway_preferences() -> Option<Value>` are the persistence
  surface. Tauri exposes `save_gateway_preferences` / `load_gateway_preferences`.
- `useMockGateway` loads preferences once on mount and seeds the Mock
  Server panel's host / port / cors form inputs. `start_mock_server`
  writes the chosen values back best-effort (failures are swallowed).

## Storage ownership

- `SqliteStore` is a thin wrapper around a path. Each command builds a fresh
  instance; connections are short-lived. `migrate()` is called defensively on
  every command to keep schema and app in sync during development.
- Every connection opened by `SqliteStore::connect()` sets
  `journal_mode = WAL`, `busy_timeout = 5000`, and `synchronous = NORMAL`
  so readers can proceed while a writer holds a transaction, transient
  write contention retries internally instead of surfacing `SQLITE_BUSY`,
  and WAL durability stays at the recommended tradeoff. A concurrency
  test in `albert-storage` runs four threads hammering the DB to prove
  the setup holds under real contention.
- The mock gateway does NOT query SQLite at request time — it gets fully
  resolved collections when `start_mock_server` calls
  `load_all_collections` / `load_collection`. A restart is required to pick up
  newly imported collections; this is intentional to keep the data plane
  immutable while serving.

## Error boundary

- All Tauri commands convert Rust errors to `String` via `map_err(|e| e.to_string())`.
- Gateway / adapter errors use `thiserror` and preserve the underlying I/O
  error via `#[source]` so higher layers can format consistent messages.
