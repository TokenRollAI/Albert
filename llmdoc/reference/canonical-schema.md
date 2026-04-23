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
- `AuthRequirement` / `AuthScheme` — optional hint captured from the
  source spec describing the header an endpoint expects. Populated by
  the OpenAPI parser from operation-level `security` (falling back to
  the document-level default). An empty operation-level `security: []`
  explicitly clears any inherited requirement. The field is
  `#[serde(default)]` so older snapshots deserialize without migration.
  cURL imports leave it `None`.

## Sources of Truth

- `crates/albert-core/src/lib.rs`: canonical Rust types
- `docs/architecture.md`: higher-level layering and ownership
- `docs/prd.md`: product intent and phase boundaries

## Mock example synthesis

`albert_core::synthesize_examples(endpoint)` walks the endpoint's
`responses` to build `success / empty / error` mock payloads without any
external model call. It:

- picks the first 2xx response as the template for success + empty
- picks the first 4xx/5xx response for the error payload
- uses any `schema.example` verbatim when present
- otherwise assigns type-aware defaults (object/array/string/int/number/bool/null)
- applies simple field-name heuristics (`*id` → uuid, `*at|time|date` →
  ISO-8601, `*email` → user@example.com, etc.)
- collapses arrays to `[]` and primitives to zero/empty values for the
  Empty variant

The parser invokes this right after building each `CanonicalEndpoint`, so
imported collections ship with meaningful mocks out-of-the-box even before
an OpenAI key is configured.

## Bundle round-trip

`albert_parser::try_parse_bundle(body)` recognizes a JSON array whose
elements look like `CanonicalApiCollection` snapshots and decodes them in
bulk. Used by the CLI `import` command, the Tauri
`import_api_description` fast path, and the `import_bundle` command —
letting `export-all` → `import` round-trip losslessly.

Non-bundle bodies return `Ok(None)`, malformed bundles return an error
with a zero-based entry index so the caller can point the user at the
bad element.

## Validation

`albert_core::validate_value(schema, value) -> Vec<String>` enforces the
declared `node_type`, nullable-null agreement, and required-property
presence. Arrays validate every item against `items`; objects walk
`properties`. Enum/format/length constraints are intentionally not
checked — callers that need them layer their own rules on top.

Used by the OpenAI adapter for the schema-aware repair loop.

## cURL parser coverage

The `albert-parser::curl` module recognizes the following flag set:

- `-X` / `--request` — explicit HTTP method
- `-H` / `--header` — request headers (preserved as `ParameterLocation::Header` parameters)
- `-d` / `--data` / `--data-raw` / `--data-binary` / `--data-ascii` — raw request body
- `--data-urlencode` — accumulates into an `application/x-www-form-urlencoded` body with percent encoding
- `-u` / `--user <user:pass>` — surfaces as `Authorization: Basic <user:pass>` header
- `-b` / `--cookie <value>` — stored as the `Cookie` header
- `--url <url>` — explicit URL (otherwise any positional token beginning
  with `http://`, `https://`, or `/` is treated as the URL)

`Content-Type` is intentionally suppressed from the header parameter list
because it's already materialized on the request body. `Authorization`,
`Accept`, `Cookie`, and custom headers all land as canonical parameters.

