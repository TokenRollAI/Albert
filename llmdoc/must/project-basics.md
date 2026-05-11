# Project Basics

## Identity

- Albert is an AI-driven API mock desktop client.
- The repository has delivered the Phase 3 static mock runtime and substantial
  Phase 4/5 slices: OpenAI-backed generation, provider profiles, Responses API
  basics, Try-it capture, and request fingerprint cache workflows.

## Current Focus

- bilingual README
- PRD, architecture, roadmap, and llmdoc
- Tauri + React + TypeScript desktop shell with productization underway
- canonical schema plus OpenAPI / cURL parsing
- SQLite persistence for imported assets, collection created/updated metadata,
  and per-endpoint mock examples
- Workspace collections drawer for scanning imported collection history,
  method mix, update timestamps, and common collection actions
- Import commands return endpoint-level diff summaries on re-import
  (added/changed/removed/unchanged) with coarse changed-endpoint reasons and
  concise parameter/request-body/response details, surfaced in frontend import
  status/toasts
- Import report drawer keeps the latest import diff visible and can open
  still-present added/changed endpoints or their success prompt preview;
  changed rows show metadata / parameters / request body / responses / auth
  reasons plus concise details when available and pass them into prompt preview
  or one-click / batch Refresh success mock as context
- live mock gateway on axum + hyper with runtime overrides, request logs,
  conditional example rules, chaos controls, auth gates, schema enforcement,
  proxy upstream, and scenarios
- OpenAI-compatible / Azure OpenAI Chat Completions plus OpenAI Responses API
  basics, provider profile generation controls, JSON-object mode, canonical
  schema validation, and one repair retry
- ResponsePane AI generation that can use the current mock example as
  generation context for iterative refinement
- Try-it response capture, schema warning, request fingerprint cache, latest
  response context refresh, Replay, Save, Remove, Clear stale, single and
  stale Refresh queue AI refresh, and context prompt preview
- CI coverage for Rust, frontend build, and brand asset drift

## Explicit Non-Goals

- production parser coverage of every OpenAPI corner case
- background request recording and automatic stale refresh jobs
- full multi-provider matrix, Azure Responses, streaming, tool calling, and
  reasoning control
- documentation diffing / JIT refresh (Phase 5)
