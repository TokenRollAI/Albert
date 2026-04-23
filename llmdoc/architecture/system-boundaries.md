# Architecture of System Boundaries

## Purpose

This document defines ownership boundaries so the foundation does not collapse into a single mixed crate.

## Core Components

- `apps/desktop` (`albert-desktop`): Tauri frontend shell + Rust command host.
  Owns `AppServices` (currently wraps `MockGateway`) and all `#[tauri::command]`
  entry points.
- `crates/albert-cli` (`albert`): headless CLI binary that drives the same
  parser / storage / gateway crates for CI and scripted workflows
  (`serve` / `import` / `list` / `export`).
- `crates/albert-core`: canonical types (`CanonicalApiCollection`,
  `CanonicalEndpoint`, `SchemaNode`, `MockExample`, `ProviderConfig` …) and
  shared contracts. Dependency-light.
- `crates/albert-parser`: OpenAPI (v3) + cURL input normalization into the
  canonical schema.
- `crates/albert-storage`: SQLite schema, migrations, CRUD (`save_collection`,
  `load_collection`, `load_all_collections`, `list_collections`,
  `list_endpoints`, `replace_mock_example`, `save_provider_config`).
- `crates/albert-gateway`: live mock HTTP gateway on axum + tokio. Owns
  `MockGateway`, `MockRoute`, `RouteTable`, `GatewayConfig`, `GatewayStatus`.
- `crates/albert-openai`: OpenAI-compatible Chat Completions adapter. Owns
  `OpenAiChatAdapter`, `PromptBundle`, `GenerationIntent`, schema hinting
  helpers.

## Flow

- UI → Tauri commands → internal crates.
- Parser transforms inputs into canonical structures.
- Storage persists canonical structures and per-endpoint mock examples.
- Gateway is given in-memory snapshots at start and serves HTTP until stopped.
- OpenAI adapter consumes a canonical endpoint + intent, speaks HTTP to the
  provider, returns a canonical `MockExample`.

## Invariants

- `albert-core` remains dependency-light. No HTTP/runtime in core.
- Parsers never expose raw upstream formats as the only persistent model;
  everything flows through the canonical schema.
- UI concerns stay out of domain crates.
- Gateway never mutates storage. Storage never calls the gateway. The Tauri
  host is the only place they touch each other.
- OpenAI adapter never touches storage directly; persistence is the host's job.

## Dependency direction

```
apps/desktop                 crates/albert-cli
  ├─> albert-parser  ─┐        ├─> albert-parser
  ├─> albert-storage ─┤        ├─> albert-storage
  ├─> albert-gateway ─┼─> albert-core   <─ crates/albert-cli
  └─> albert-openai  ─┘        └─> albert-gateway
```

## Related Docs

- `llmdoc/architecture/request-flow.md`
- `llmdoc/architecture/runtime-state.md`
- `llmdoc/reference/canonical-schema.md`
- `llmdoc/reference/gateway-routes.md`
- `llmdoc/reference/openai-adapter.md`
- `docs/architecture.md`
