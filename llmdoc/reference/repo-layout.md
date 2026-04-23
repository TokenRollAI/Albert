# Repository Layout Reference

## Scope

This document maps stable top-level directories.

## Stable Facts

- `assets/branding` owns stable logo and exported brand assets.
- `apps/desktop` owns the desktop UI shell.
- `apps/desktop/public` owns web-facing static assets such as favicons.
- `apps/desktop/src-tauri` owns the Tauri Rust entrypoint.
- `crates/albert-core` owns canonical shared models.
- `crates/albert-parser` owns ingestion boundaries.
- `crates/albert-storage` owns persistence boundaries.
- `crates/albert-gateway` owns local runtime boundaries.
- `crates/albert-openai` owns OpenAI adapter boundaries.
- `docs/` owns project-facing planning and architecture docs.
- `llmdoc/` owns project memory and operational docs.
- `scripts/` owns reproducible local project automation helpers.

## Sources of Truth

- `Cargo.toml`: Rust workspace members
- `package.json`: npm workspace members
