# OpenAI Adapter Contract

## Crate

- `albert-openai` — depends only on `albert-core`, `reqwest`, `serde`, `tokio`.

## Types

- `OpenAiChatAdapter { config: ProviderConfig, api_key, timeout }` —
  `new` reads the API key from the configured env variable; `with_api_key`
  provides a session-scoped override.
- `ProviderConfig` includes an optional `environment` label for grouping saved
  profiles (`local`, `staging`, `prod`, etc.), `api_type`
  (`openai_compatible` by default, `openai_responses`, `azure_openai`, or
  `azure_openai_responses`) plus optional `azure_deployment` and
  `azure_api_version` fields for Azure OpenAI.
  Generation controls are stored on the same config:
  `temperature` is optional, clamped to `0.0..=2.0`, and defaults to
  `0.7` when absent; `max_output_tokens` is optional and omitted from
  provider requests when absent or zero. `reasoning_effort` is optional
  (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`) and currently only
  affects OpenAI Responses requests; blank profiles omit the `reasoning`
  object so the provider default applies. `schema_repair_attempts` is optional,
  defaults to `2`, and is clamped to `0..=5`; `0` disables schema repair
  retries after the first invalid payload.
- `GenerationIntent::{Success, Empty, Error}` — drives prompt wording and the
  resulting `MockExampleKind`.
- `PromptBundle { system, user, endpoint_context: serde_json::Value }` — what
  `preview_prompt` returns and what `call_chat` ultimately sends.

## Provider Requests

### OpenAI-compatible

- `POST {base_url}/v1/chat/completions`.
- `base_url` is normalized (trailing `/` and trailing `/v1` stripped before
  re-appending `/v1/chat/completions`), so configurations like
  `https://api.openai.com` and `https://api.openai.com/v1` both work.
- `Authorization: Bearer <api_key>` and `Content-Type: application/json`.
- Request body includes `model: <provider_config.model>`.

```json
{
  "model": "<provider_config.model>",
  "messages": [
    {"role": "system", "content": "<bundle.system>"},
    {"role": "user",   "content": "<bundle.user>"}
  ],
  "response_format": {"type": "json_object"},
  "temperature": 0.7,
  "max_tokens": 2048
}
```

`max_tokens` is only present when the provider profile sets
`max_output_tokens`.

### Azure OpenAI

- `POST {base_url}/openai/deployments/{deployment}/chat/completions?api-version={api_version}`.
- `base_url` is treated as the Azure resource root, for example
  `https://example.openai.azure.com`; trailing `/` is stripped.
- `deployment` is `azure_deployment` when set, otherwise the adapter falls
  back to `model` for compatibility with older drafts.
- `azure_api_version` is required.
- Authentication uses `api-key: <api_key>` rather than
  `Authorization: Bearer ...`.
- Request body intentionally omits `model`; Azure selects the model through
  the deployment segment.

```json
{
  "messages": [
    {"role": "system", "content": "<bundle.system>"},
    {"role": "user",   "content": "<bundle.user>"}
  ],
  "response_format": {"type": "json_object"},
  "temperature": 0.7,
  "max_tokens": 2048
}
```

`max_tokens` is only present when the provider profile sets
`max_output_tokens`.

### OpenAI Responses

- `POST {base_url}/v1/responses`.
- `base_url` is normalized the same way as OpenAI-compatible Chat
  Completions: trailing `/` and trailing `/v1` are stripped before
  re-appending `/v1/responses`.
- Authentication uses `Authorization: Bearer <api_key>`.
- Request body includes `model`, `instructions`, `input`, and
  `text.format: {type: "json_object"}`.

```json
{
  "model": "<provider_config.model>",
  "instructions": "<bundle.system>",
  "input": "<bundle.user>",
  "text": {
    "format": {
      "type": "json_object"
    }
  },
  "temperature": 0.7,
  "max_output_tokens": 2048,
  "reasoning": {
    "effort": "low"
  }
}
```

`max_output_tokens` is only present when the provider profile sets it.
`reasoning` is only present when `reasoning_effort` is set; Chat
Completions and Azure Chat requests intentionally do not receive a
reasoning parameter in this slice.

### Azure OpenAI Responses

- `POST {base_url}/openai/v1/responses`.
- `base_url` is treated as the Azure resource root, for example
  `https://example.openai.azure.com`; trailing `/` is stripped.
- Authentication uses `api-key: <api_key>`.
- The request body uses the same Responses payload shape as OpenAI Responses,
  but `model` is the Azure deployment name (`azure_deployment` when set,
  otherwise `model` as a compatibility fallback).
- `azure_api_version` is not required for this path; the current Azure
  Responses REST surface uses the `/openai/v1` route.

## Prompt construction

- System message: hard-coded persona, "return a single JSON object, no
  markdown, no commentary".
- User message contains a pretty-printed JSON of:
  - `operation_id, method, path, summary, description, tags`
  - `parameters` → each `{name, in, required, schema, description}`
  - `request_body` → `{content_type, required, schema}`
  - `responses` → each `{status_code, content_type, description, schema}`
  - plus an instruction tailored to the `GenerationIntent`.
- `schema_hint` includes the canonical validation surface used by the repair
  loop, including object dependencies (`dependentRequired`,
  `dependentSchemas`), object-level `unevaluatedProperties: false`,
  conditional schemas (`if`, `then`, `else`), array tuple constraints
  (`prefixItems`, `unevaluatedItems: false`), and array containment
  constraints (`contains`, `minContains`, `maxContains`) when present.
- `GenerationContext` may optionally add a second pretty-printed block with
  `request_snapshot`, `previous_response_snapshot`, and a short note. This is
  used by ResponsePane to carry the current mock example during per-kind and
  batch generation, and by Try-it latest/cache **AI refresh** so real
  request/response evidence can steer a single generation without changing the
  endpoint schema contract.
  `preview_generation_prompt` accepts the same context, keeping preview and
  generation aligned.

## Response handling

- Chat Completions responses extract the first
  `choices[0].message.content`.
- Responses API responses prefer top-level `output_text`, then fall back to
  walking `output[].content[]` and using a textual `text` field from
  `output_text` / `text` content parts.
- Markdown code fences are stripped if the model wrapped the JSON with
  ```` ```json ... ``` ````.
- Decoded JSON becomes `MockExample.payload`; the result is tagged with the
  intent's kind/title and a provenance note.

## Schema validation + bounded repair

When the endpoint declares a matching response schema (2xx for
success/empty, 4xx/5xx for error), the adapter calls
`albert_core::validate_value(schema, payload)`:

- ✅ validates → example is returned with the standard note.
- ❌ fails → the adapter sends up to the configured number of repair requests
  (`schema_repair_attempts`, default `2`, max `5`) with the same
  system prompt, the original user prompt, and an appended block listing
  the current validation errors and instructing "return a new JSON object
  that fixes the listed issues".
  - If the profile sets `schema_repair_attempts: 0`, the adapter returns the
    original invalid payload with the validation errors in the note.
  - If the repaired payload validates, the resulting example's note is
    annotated with the successful retry attempt count.
  - If all configured repair attempts still fail validation, the latest repaired
    payload is returned anyway with the remaining errors appended to the note
    so the caller can decide.
  - If a repair HTTP call itself errors, the adapter falls back to the latest
    payload and annotates the note with the attempt number and transport error.

Endpoints without a response schema skip validation entirely (no retry
loop, no note annotation).

## Errors

- `MissingApiKey(env_var_name)` — no key found in env or override.
- `Transport(String)` — network / client build error.
- `Provider { status, body }` — non-2xx from the remote.
- `Decode(String)` — either JSON parsing of the transport body or the model
  content itself failed.
- `MissingContent` — response shape had no assistant message.
- `Config(&'static str)` — local provider configuration is incomplete
  before the HTTP request is sent (currently used for missing Azure API
  version / deployment).

## Tauri surface

`generate_mock_example(request: GenerationRequest) -> MockExample`
where `GenerationRequest` includes the full `CanonicalEndpoint`, an intent, a
`ProviderConfigInput`, optional `collection_id` + `persist: bool` +
`database_url`, an optional `api_key_override` (session-only), and optional
`generation_context`.

When `persist` is true the example is saved via
`SqliteStore::replace_mock_example`, which also rewrites the collection
snapshot JSON to keep subsequent `load_collection_snapshot` reads consistent.

## Test-connection probe

The `test_provider_connection` Tauri command exercises the same
`call_chat` path as `generate_mock_example` but uses a trivial synthetic
endpoint (`GET /ping`) and an 8-second timeout. It returns
`{ ok, message, status }` so the Providers panel can surface a green
`✓ connected` chip or a red `✗ failed` chip with the raw error (missing
key / transport / provider 4xx / provider 5xx) rendered in a banner.

## Provider environment status

The `provider_env_status` Tauri command performs a local-only key source
check for the Providers panel. It does not call the provider. It returns
`{ env_var, env_present, override_present, usable, message }`, where
`usable` is true when either a non-blank session `api_key_override` was
provided or the configured env var is present and non-blank in the Tauri
backend environment. In static browser mode the frontend does not invoke
the command and shows a Tauri-required status instead.

## Provider profiles

The desktop Tauri surface also exposes `list_provider_configs`,
`save_provider_config`, and `delete_provider_config`. These commands persist
only non-secret `ProviderConfig` fields (`provider_name`, `base_url`, `model`,
`api_key_env`, optional `environment`, `api_type`, `azure_deployment`,
`azure_api_version`, `temperature`, `max_output_tokens`, `reasoning_effort`,
`schema_repair_attempts`) into SQLite
`provider_configs`; API key material is never stored there. The Providers
panel lists Saved profiles, can load a saved profile into the active draft,
duplicate a saved profile into a new `*-copy` draft for environment variants,
save the current draft by `provider_name`, filter saved profiles by
environment label, and delete saved profiles. Same-name saves replace the
existing row; environment is metadata, not part of the primary key. Static
browser mode disables profile persistence because the Tauri command surface is
unavailable.

## Tests

- Unit tests (`src/lib.rs`) cover prompt construction, schema hints, code
  fence stripping, missing-api-key error surfacing.
- Integration tests (`tests/adapter_integration.rs`) stand up a tiny
  dependency-free `tokio::net::TcpListener` that replies with a canned
  chat-completions JSON body. They assert the adapter decodes the content
  end-to-end into a `MockExample` (success path), surfaces HTTP 4xx/5xx
  responses with status + body (error path), and sends Azure OpenAI requests
  to the deployment URL with the `api-key` header and no request-body `model`.
  A Responses API case asserts `/v1/responses`, bearer auth,
  `instructions` / `input` / `text.format: json_object`, and `output_text`
  extraction. An Azure Responses case asserts `/openai/v1/responses`,
  `api-key` auth, deployment-name `model`, Responses payload shape, and
  `output_text` extraction.
