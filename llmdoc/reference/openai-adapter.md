# OpenAI Adapter Contract

## Crate

- `albert-openai` — depends only on `albert-core`, `reqwest`, `serde`, `tokio`.

## Types

- `OpenAiChatAdapter { config: ProviderConfig, api_key, timeout }` —
  `new` reads the API key from the configured env variable; `with_api_key`
  provides a session-scoped override.
- `GenerationIntent::{Success, Empty, Error}` — drives prompt wording and the
  resulting `MockExampleKind`.
- `PromptBundle { system, user, endpoint_context: serde_json::Value }` — what
  `preview_prompt` returns and what `call_chat` ultimately sends.

## Endpoint

- `POST {base_url}/v1/chat/completions`.
- `base_url` is normalized (trailing `/` and trailing `/v1` stripped before
  re-appending `/v1/chat/completions`), so configurations like
  `https://api.openai.com` and `https://api.openai.com/v1` both work.
- `Authorization: Bearer <api_key>` and `Content-Type: application/json`.

## Request body shape

```json
{
  "model": "<provider_config.model>",
  "messages": [
    {"role": "system", "content": "<bundle.system>"},
    {"role": "user",   "content": "<bundle.user>"}
  ],
  "response_format": {"type": "json_object"},
  "temperature": 0.7
}
```

## Prompt construction

- System message: hard-coded persona, "return a single JSON object, no
  markdown, no commentary".
- User message contains a pretty-printed JSON of:
  - `operation_id, method, path, summary, description, tags`
  - `parameters` → each `{name, in, required, schema, description}`
  - `request_body` → `{content_type, required, schema}`
  - `responses` → each `{status_code, content_type, description, schema}`
  - plus an instruction tailored to the `GenerationIntent`.

## Response handling

- The first `choices[0].message.content` is extracted.
- Markdown code fences are stripped if the model wrapped the JSON with
  ```` ```json ... ``` ````.
- Decoded JSON becomes `MockExample.payload`; the result is tagged with the
  intent's kind/title and a provenance note.

## Schema validation + one-shot repair

When the endpoint declares a matching response schema (2xx for
success/empty, 4xx/5xx for error), the adapter calls
`albert_core::validate_value(schema, payload)`:

- ✅ validates → example is returned with the standard note.
- ❌ fails → the adapter sends a single repair request with the same
  system prompt, the original user prompt, and an appended block listing
  the validation errors and instructing "return a new JSON object that
  fixes the listed issues".
  - If the repaired payload validates, the resulting example's note is
    annotated with `Repaired after one validation retry.`
  - If it still fails, the repaired payload is returned anyway with the
    remaining errors appended to the note so the caller can decide.
  - If the repair HTTP call itself errors, the adapter falls back to the
    original payload and annotates the note with the transport error.

Endpoints without a response schema skip validation entirely (no retry
loop, no note annotation).

## Errors

- `MissingApiKey(env_var_name)` — no key found in env or override.
- `Transport(String)` — network / client build error.
- `Provider { status, body }` — non-2xx from the remote.
- `Decode(String)` — either JSON parsing of the transport body or the model
  content itself failed.
- `MissingContent` — response shape had no assistant message.

## Tauri surface

`generate_mock_example(request: GenerationRequest) -> MockExample`
where `GenerationRequest` includes the full `CanonicalEndpoint`, an intent, a
`ProviderConfigInput`, optional `collection_id` + `persist: bool` +
`database_url`, and an optional `api_key_override` (session-only).

When `persist` is true the example is saved via
`SqliteStore::replace_mock_example`, which also rewrites the collection
snapshot JSON to keep subsequent `load_collection_snapshot` reads consistent.

## Tests

- Unit tests (`src/lib.rs`) cover prompt construction, schema hints, code
  fence stripping, missing-api-key error surfacing.
- Integration tests (`tests/adapter_integration.rs`) stand up a tiny
  dependency-free `tokio::net::TcpListener` that replies with a canned
  chat-completions JSON body. They assert the adapter decodes the content
  end-to-end into a `MockExample` (success path) and surfaces HTTP 4xx/5xx
  responses with status + body (error path).
