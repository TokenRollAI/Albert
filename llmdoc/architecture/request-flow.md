# Architecture of Request Flow

## Purpose

This document captures the planned runtime flow before full implementation exists.

## Import Flow

1. UI receives OpenAPI or cURL input.
2. Tauri command selects a parser.
3. Parser emits `CanonicalApiCollection`.
4. Storage persists project, endpoints, schemas, and mock examples.
5. UI renders imported assets.

## Planned Mock Flow

1. Local gateway receives request.
2. Router resolves endpoint and mock strategy.
3. Static mock example is selected.
4. Response payload is returned to caller.

## Future AI Flow

1. Gateway or UI requests generation.
2. OpenAI adapter receives canonical schema and generation intent.
3. Structured result is validated.
4. Storage persists generated example.

## Related Docs

- `docs/roadmap.md`
- `llmdoc/reference/canonical-schema.md`

