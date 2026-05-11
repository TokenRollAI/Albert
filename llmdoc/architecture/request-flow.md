# Architecture of Request Flow

## Purpose

This document captures the runtime flows as they are now implemented.

## Import Flow (Phase 2)

1. UI receives OpenAPI or cURL input.
2. Tauri command selects a parser.
3. Parser emits `CanonicalApiCollection`.
4. Storage persists project, collection metadata, endpoints, schemas, and
   mock examples.
5. UI renders imported assets. Imported collection summaries carry
   `created_at` / `updated_at`; the sidebar displays the latest update/import
   timestamp and endpoint count, while fallback/preview collections omit this
   metadata. The Workspace collections drawer also uses the summaries to show
   collection count, endpoint count, source, update timestamps, method mix, and
   common collection actions.
6. `updated_at` refreshes on re-import, rename, and persisted mock-example
   edits (`replace_mock_example`) so the sidebar ordering reflects actual mock
   asset changes, not only import time.
7. On re-import, `import_api_description` / `import_bundle` load the existing
   canonical snapshot before overwriting it and return an endpoint-level diff
   summary: added, changed, removed, unchanged. The comparison clears
   `examples` before serializing endpoints, so hand-edited/AI-generated mock
   examples do not make an otherwise identical API contract look changed.
   Changed endpoint entries also include coarse reasons (`metadata changed`,
   `parameters changed`, `request body changed`, `responses changed`, and
   `auth changed`) plus concise details (parameter added/removed/changed,
   request body content type/required/schema, response status/content
   type/schema, auth/metadata changes). This lets the UI explain why an
   endpoint was marked changed without attempting full JSON Pointer schema
   diffing. The frontend uses the summary in the import status line and success
   toast.
8. App state keeps the latest import result plus the newly parsed collection in
   memory. The Import report drawer renders grouped added / changed / removed
   endpoint rows. Changed rows show the coarse reasons and concise details when
   present. Added and changed rows can open endpoints from the new snapshot or
   open a `success` prompt preview for that endpoint. When the changed row has
   reasons/details, prompt preview receives them as a generation-context note so
   the model-facing prompt explains the import drift; changed rows can also
   AI-refresh the `success` mock with the same context and persist the result.
   The drawer header can batch refresh all refreshable changed endpoints through
   the same sequential `generate_mock_example` path. Removed rows are read-only
   because those endpoints no longer exist after import.

Current commands:

- `parse_api_description`
- `import_api_description`
- `list_imported_collections`
- `list_imported_endpoints`
- `load_collection_snapshot`

This diff/report is intentionally a first slice. It is not a persisted
import-history table, field-level Schema Diff Engine, or stale mock
invalidation workflow yet.

## Mock Flow (Phase 3 — live)

1. UI opens the Mock Server panel and clicks Start.
2. Tauri command `start_mock_server` loads one or more collections from SQLite
   (via `load_all_collections` / `load_collection`).
3. `albert-gateway::MockGateway::start` binds a `TcpListener` (axum + hyper) on
   the requested host/port, building a `RouteTable` from all canonical endpoints.
4. Incoming HTTP requests are matched by method + path template (literal
   segments beat `{param}` wildcards). Example selection order:
   1. Query override `?__albert_mock=success|empty|error`.
   2. Per-endpoint override in `GatewayConfig.example_overrides`.
   3. The endpoint's `success` example.
   4. First available example.
5. Response headers include `x-albert-mock-kind` and `x-albert-mock-route` so
   the UI and callers can confirm what was served.
6. `stop_mock_server` issues a graceful shutdown via oneshot; `mock_server_status`
   returns the live `GatewayStatus` (bind address, route list, overrides).

Special route: `GET /__albert/status` returns a JSON handshake payload.

## AI Flow (Phase 4 — Chat Completions)

1. UI calls `generate_mock_example` with the canonical endpoint and intent
   (`success | empty | error`). When a target mock slot already has a payload,
   ResponsePane passes that current example as `generation_context`; per-kind
   generation, prompt preview, and Generate all can iterate from the relevant
   existing mock instead of ignoring it.
2. `albert-openai::build_prompt_bundle` constructs a JSON-Schema-like endpoint
   summary (parameters, request body, responses) plus a system + user message
   instructing the model to return strict JSON.
3. `OpenAiChatAdapter::call_chat` POSTs to `${base_url}/v1/chat/completions`
   with `response_format: {type: "json_object"}`, Bearer auth from either the
   env variable in `ProviderConfig.api_key_env` or a session-only override.
4. The model's message content is parsed, markdown code fences stripped, and
   wrapped into a `MockExample` tagged with the requested intent.
5. When `persist: true`, the example is upserted through
   `SqliteStore::replace_mock_example`, and the collection snapshot JSON is
   updated in lockstep.

## Try-it Capture + Fingerprint Cache (Phase 5 — first slice)

1. The Try-it panel sends a request to the running mock gateway from the
   browser and captures status, selected response headers, body, elapsed ms,
   and wire-size.
2. The latest response can be saved as the endpoint's `success | empty | error`
   mock example through `save_mock_example`; before saving, the frontend calls
   `validate_mock_payload` when Tauri is available and falls back to the
   lightweight TypeScript validator otherwise.
3. The same latest response can be passed directly as `generation_context` to
   `generate_mock_example` or `preview_generation_prompt`, so the user can
   refresh or inspect an AI prompt immediately after a real Try-it call without
   first finding the persisted cache row.
4. On every successful Try-it send for a persisted collection, the frontend
   also calls `save_request_cache` best-effort with a request snapshot
   (`query`, headers, JSON/raw body) plus response snapshot.
5. `albert-storage` writes `request_fingerprint_cache` using a stable
   normalized fingerprint of `method + path + request_snapshot`. Header names
   are normalized case-insensitively and sensitive header values are redacted
   before fingerprinting / persistence.
6. Repeating the same request upserts the same row and increments `hit_count`.
   The UI shows `cached` for the first capture and `cache hit ×N` for repeats.
   If Request cache routing is already enabled for the running Mock Server,
   the latest-response area offers **Reload routing** so the newly persisted
   fingerprint can be injected immediately without switching to the Runtime tab.
7. The Try-it panel also loads the latest cached fingerprints for the active
   persisted endpoint through `list_request_cache`. Each cached response can be
   saved back into the endpoint's `success | empty | error` mock slot, reusing
   the same schema-warning path as latest-response capture.
8. Cache rows older than 24 hours are labeled stale in the UI. The Replay
   action copies the cached request snapshot back into the Try-it draft so the
   user can resend and refresh that fingerprint intentionally.
9. Cache management stays endpoint-scoped: `delete_request_cache` removes one
   cached fingerprint by id, while `delete_stale_request_cache` removes rows
   for the active `collection_id + method + path` whose `last_seen_at` is older
   than the frontend's 24-hour stale threshold.
10. Manual AI refresh is cache-contextual: a Try-it cache row can call
   `generate_mock_example` with `generation_context` containing the cached
   request snapshot, cached response snapshot, and fingerprint note. The result
   follows the normal `persist=true` replacement path for the selected
   `success | empty | error` slot.
11. When stale rows exist, Try-it shows a visible **Refresh queue** before the
    collapsible cache list. The queue summarizes stale vs refreshable rows,
    exposes batch refresh, can preview the first stale prompt, and can clear
    stale rows for the active endpoint. Batch refresh still runs the same
    cache-contextual generation serially, preserves the selected/fallback
    example kind per row, and does not delete stale cache rows or resend
    requests.
12. The same cache context can be passed to `preview_generation_prompt`, so
    prompt preview and AI refresh share the same request/response evidence.
13. Mock Server **Request cache routing** is opt-in. When enabled, Tauri loads
    recent request-cache responses for the bound collections during
    `start_mock_server` / `update_mock_server` and injects them into the
    gateway config. A request whose method/path/query/header/body fingerprint
    matches returns the cached response with `x-albert-mock-source: cache`.
    Query overrides and per-route overrides still take precedence. The Runtime
    tab shows the number of injected cache entries and exposes **Reload request
    cache**, which calls `update_mock_server(use_request_cache=true)` to pull in
    newly recorded Try-it rows without restarting the listener.

Current limitation: request-cache routing is in-memory and opt-in. It does not
run background recording or automatic stale refresh jobs, and the gateway still
does not read SQLite on the request hot path. Newly captured cache rows only
participate in routing after start, toggle/update, or the explicit reload action.

## Related Docs

- `docs/roadmap.md`
- `llmdoc/reference/canonical-schema.md`
- `llmdoc/architecture/system-boundaries.md`
