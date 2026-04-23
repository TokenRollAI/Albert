# Albert CLI

`albert-cli` is a standalone binary that drives the same parser + storage +
gateway crates used by the desktop app. Use it for CI, smoke-testing, or
running the mock server without the Tauri shell.

## Binary

- Cargo package: `albert-cli`
- Installed as `albert` (see `[[bin]]` entry).
- Pure Rust; zero external CLI dependencies — the argument parser is hand
  rolled in `src/args.rs`.

## Commands

| Command   | Purpose                                                      |
|-----------|--------------------------------------------------------------|
| `serve`   | Start the mock HTTP gateway                                  |
| `import`  | Parse an OpenAPI/cURL file and persist it into SQLite        |
| `list`    | Print the collections stored in the database                 |
| `export`  | Print a collection snapshot as JSON (optionally to a file)   |
| `help`    | Show the usage text                                          |
| `version` | Print the crate version                                      |

## Shared options

- `--db <path>` (default: `albert.db`) — SQLite database path.

## `serve` options

- `--host <ip>` (default: `127.0.0.1`)
- `--port <n>` (default: `4317`; `0` picks an ephemeral port)
- `--no-cors` — disable the default permissive CORS layer
- `--default-latency-ms <n>` — add a latency floor to every route
- `--error-rate <0..1>` — probability of returning the error example
- `--collection <id>` (repeatable) — only serve named collections
- `--auto-stop-secs <n>` — stop after N seconds (or on Ctrl-C, whichever
  comes first). Useful in tests and one-shot CI runs.

## `export` options

- `--id <collection_id>` — collection to serialize (required)
- `--output <path>` — write to file (default: stdout)

## Example workflow

```
$ albert import --db ./demo.db ./fixtures/sample-openapi.json
imported Albert Example API (1 endpoints) from ./fixtures/sample-openapi.json

$ albert list --db ./demo.db
Albert Example API              openapi     1 endpoints    id=ee7ac8bf…

$ albert serve --db ./demo.db --port 0
# Ctrl-C to stop

$ albert export --db ./demo.db --id ee7ac8bf… --output snapshot.json
wrote 2348 bytes to snapshot.json
```

## Tests

- Unit tests in `src/args.rs` cover flag parsing + error cases.
- `tests/smoke.rs` runs the full `import → list → export → serve` round
  trip against a real TCP listener.
