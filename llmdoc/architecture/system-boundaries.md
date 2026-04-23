# Architecture of System Boundaries

## Purpose

This document defines ownership boundaries so the foundation does not collapse into a single mixed crate.

## Core Components

- `apps/desktop`: Tauri frontend shell and Rust command host
- `crates/albert-core`: canonical types and shared contracts
- `crates/albert-parser`: input normalization
- `crates/albert-storage`: SQLite contract and migration ownership
- `crates/albert-gateway`: local mock runtime contract
- `crates/albert-openai`: OpenAI adapter contract

## Flow

- UI sends intent to Tauri commands.
- Tauri commands coordinate internal crates.
- Parser transforms inputs into canonical structures.
- Storage persists canonical structures and mock examples.
- Gateway consumes persisted artifacts later.
- OpenAI adapter consumes canonical structures later.

## Invariants

- `albert-core` remains dependency-light.
- Parsers never expose raw upstream formats as the only persistent model.
- UI concerns stay out of domain crates.

## Related Docs

- `llmdoc/architecture/request-flow.md`
- `llmdoc/reference/canonical-schema.md`
- `docs/architecture.md`

