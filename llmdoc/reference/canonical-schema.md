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
presence. Arrays validate tuple-style `prefix_items` first, then validate
remaining entries against `items` when present; objects walk `properties`;
enum values are rejected when the payload is off-list. The
canonical `SchemaNode` also carries common JSON Schema constraints parsed
from OpenAPI: `format`, `pattern`, `min_length`, `max_length`, numeric
`minimum` / `maximum` (including exclusive flags), and `min_items` /
`max_items`. It also carries `multiple_of`, `unique_items`, `contains` /
`min_contains` / `max_contains`, `prefix_items`, `allow_unevaluated_items`,
and `additional_properties` /
`allow_additional_properties` for OpenAPI typed maps and closed objects,
plus object `min_properties` / `max_properties`, `dependent_required`,
`dependent_schemas`, and `allow_unevaluated_properties`. Conditional schemas
(`if` / `then` / `else`) are also carried as nested `SchemaNode`s and enforced
by applying `then` when the `if` schema validates, otherwise applying `else`
when present. Boolean JSON Schemas are represented with
`SchemaNode::bool_schema(true|false)`: `true` accepts any payload and `false`
rejects every payload at that schema position. This is used for raw OpenAPI /
JSON Schema positions such as `items: false`, `prefixItems: [false]`,
`additionalProperties: false`, and conditional/dependent schemas.

The OpenAPI parser uses `openapiv3` for the standard 3.0 typed schema
surface and also keeps a raw JSON/YAML `Value` copy long enough to overlay
JSON Schema keywords that `openapiv3` 2.2 does not expose directly, including
`contains`, `minContains`, `maxContains`, `dependentRequired`, and
`dependentSchemas`, plus `if`, `then`, `else`, `prefixItems`,
`unevaluatedItems: false`, and object-level `unevaluatedProperties: false`.
The `unevaluated*` support is conservative: object closure checks properties
not covered by `properties` or typed `additionalProperties`; array closure
checks items beyond `prefixItems` when no tail `items` schema exists. This is
not a complete JSON Schema evaluated-set algorithm. Because `openapiv3` 2.2
does not accept JSON Schema boolean schemas on its typed parse path, the parser
sanitizes schema-position booleans to `{}` only for typed deserialization, then
re-applies the original raw `true` / `false` semantics through the raw overlay.

Validation enforces these constraints with
conservative format checks for `email`, `date`, and `date-time`; unknown
formats are left as hints rather than hard errors. Patterns that Rust
`regex` can compile are enforced; unsupported ECMA-262 patterns remain
prompt hints instead of failing every payload.

Used by the OpenAI adapter for the schema-aware repair loop.

## cURL parser coverage

The `albert-parser::curl` module recognizes the following flag set:

- `-X` / `--request` — explicit HTTP method
- `-H` / `--header` — request headers (preserved as `ParameterLocation::Header` parameters)
- `-d` / `--data` / `--data-raw` / `--data-ascii` — raw request body
- `--data-binary` — raw request body, with `@file` / `<file` references represented as string schema with `format: binary`
- `--data-urlencode` — accumulates into an `application/x-www-form-urlencoded` body with percent encoding
- `-F` / `--form` — builds a `multipart/form-data` object schema; `@file` / `<file` parts are represented as required binary string fields, and `;type=...` is kept as a field description hint
- `--form-string` — builds a `multipart/form-data` string field even when the value starts with `@` / `<`
- `-u` / `--user <user:pass>` — surfaces as `Authorization: Basic <user:pass>` header
- `-b` / `--cookie <value>` — stored as the `Cookie` header
- `--url <url>` — explicit URL (otherwise any positional token beginning
  with `http://`, `https://`, or `/` is treated as the URL)

`Content-Type` is intentionally suppressed from the header parameter list
because it's already materialized on the request body. `Authorization`,
`Accept`, `Cookie`, and custom headers all land as canonical parameters.
Repeated query parameters and repeated headers are preserved as separate
canonical parameters in source order rather than collapsed to one value.
