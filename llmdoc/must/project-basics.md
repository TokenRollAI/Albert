# Project Basics

## Identity

- Albert is an AI-driven API mock desktop client.
- The repository is actively delivering Phase 3 (static mock runtime) and the
  early slice of Phase 4 (OpenAI-backed generation).

## Current Focus

- bilingual README
- PRD, architecture, roadmap, and llmdoc
- Tauri + React + TypeScript desktop shell with productization underway
- canonical schema plus OpenAPI / cURL parsing
- SQLite persistence for imported assets and per-endpoint mock examples
- live mock gateway on axum + hyper (Phase 3)
- OpenAI-compatible chat completions adapter with JSON mode (Phase 4)
- CI coverage for Rust, frontend build, and brand asset drift

## Explicit Non-Goals

- production parser coverage of every OpenAPI corner case
- caching and request fingerprints (Phase 5)
- multi-provider matrix and Responses API (Phase 5)
- documentation diffing / JIT refresh (Phase 5)
