# Remote source fetch

The Import dialog can pull an OpenAPI spec directly from an `http(s)://`
URL so users don't have to paste raw text.

## Tauri command

`fetch_remote_source(args: { url: string }) -> FetchedSource`

```rust
pub struct FetchedSource {
    pub url: String,
    pub content_type: Option<String>,
    pub body: String,
    pub suggested_name: Option<String>,
}
```

## Contract

- `url` must have scheme `http` or `https`; anything else is rejected.
- Uses `reqwest::Client` with a 20s timeout and an `Accept` header that
  advertises JSON, YAML, and text.
- Responses are capped at 2 MB — larger payloads return an error so a
  runaway download can't exhaust memory.
- Non-2xx responses produce an error containing the status and a 512-char
  prefix of the remote body.
- `suggested_name` is derived from the last URL path segment, stripped of
  `.json` / `.yaml` suffixes; falls back to the host when the path is
  empty.

## UI

The Import dialog renders a "Fetch from URL" row when the Tauri runtime
is available. Entering a URL and clicking **Fetch**:

1. Invokes the command above.
2. On success, overwrites the textarea with the fetched body and seeds
   the collection name input with `suggested_name` when the user hadn't
   already typed one.
3. On failure, displays the backend error inline without clearing the
   existing draft.
