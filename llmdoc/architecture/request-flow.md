# Architecture of Request Flow

## Purpose

This document captures the runtime flows as they are now implemented.

## Import Flow (Phase 2)

1. UI receives OpenAPI or cURL input.
2. Tauri command selects a parser.
3. Parser emits `CanonicalApiCollection`.
4. Storage persists project, endpoints, schemas, and mock examples.
5. UI renders imported assets.

Current commands:

- `parse_api_description`
- `import_api_description`
- `list_imported_collections`
- `list_imported_endpoints`
- `load_collection_snapshot`

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
   (`success | empty | error`).
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

## Related Docs

- `docs/roadmap.md`
- `llmdoc/reference/canonical-schema.md`
- `llmdoc/architecture/system-boundaries.md`
