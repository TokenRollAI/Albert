# Canonical Schema Reference

## Scope

This document defines the internal representation that all ingestion formats should target.

## Stable Facts

- Upstream inputs include OpenAPI and cURL in Phase 1.
- Persistent business objects should target a canonical collection and endpoint model.
- JSON-schema-like nodes are preferred over raw upstream structures.
- Mock examples are stored separately from endpoint shape.

## Core Types

- `CanonicalApiCollection`
- `CanonicalEndpoint`
- `CanonicalParameter`
- `CanonicalRequestBody`
- `CanonicalResponse`
- `SchemaNode`
- `MockExample`
- `ProviderConfig`

## Sources of Truth

- `crates/albert-core/src/lib.rs`: canonical Rust types
- `docs/architecture.md`: higher-level layering and ownership
- `docs/prd.md`: product intent and phase boundaries

