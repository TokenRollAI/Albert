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
| `import`  | Parse an OpenAPI/cURL file (or a JSON bundle) and persist it |
| `watch`   | Keep re-importing one or more files on every mtime change    |
| `list`    | Print the collections stored in the database                 |
| `export`  | Print a collection snapshot as JSON (optionally to a file)   |
| `delete`  | Remove a collection (and its endpoints/examples) from SQLite |
| `rename`  | Rename a stored collection (`--id` + `--name`)               |
| `export-all` | Print every collection as a JSON array (optionally to a file) |
| `doctor`  | Health checks: db migratability, env keys, provider probe   |
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
- `--capture-bodies` — record up to 4KB of each POST/PUT/PATCH/DELETE
  body into the in-memory request log (off by default; see gateway-routes
  for caveats).

## `export` options

- `--id <collection_id>` — collection to serialize (required)
- `--output <path>` — write to file (default: stdout)

## `delete` options

- `--id <collection_id>` — collection to remove (required)

## `rename` options

- `--id <collection_id>` — collection to rename (required)
- `--name <new_name>` — new display name (required, trimmed, non-empty)

## `export-all` options

- `--output <path>` — write to file; default: stdout

## `import` bundle behavior

If the input body is a JSON array whose entries each have `id`, `name`,
and `endpoints`, `import` (and the Tauri `import_api_description`
command) persists every entry in one call. This is the mirror image of
`export-all` — a bundle round-trips losslessly through SQLite. Bodies
that are not arrays, or arrays whose entries don't look like canonical
snapshots, fall through to the regular OpenAPI / cURL parsers.

## `watch` options

- `<file>` — one or more positional file paths to watch (required).
- `--interval-ms <n>` — poll interval (default `1000`, minimum `100`).
- `--auto-stop-secs <n>` — exit after N seconds (for scripted tests;
  production use relies on Ctrl-C).

The watcher stats every file each tick; when its `mtime` changes (or on
startup), the file is re-imported. Errors are written to stderr without
aborting the loop.

## `doctor`

Runs three sequential checks and exits non-zero when any fail:

1. **Database** — `SqliteStore::migrate()` against `--db` (default
   `albert.db`).
2. **Environment keys** — warns when `OPENAI_API_KEY` or
   `ANTHROPIC_API_KEY` are missing or empty. These are advisory only;
   a missing key does not fail the run because not every user uses every
   provider.
3. **Provider reachability** — issues a HEAD request against
   `ALBERT_PROVIDER_URL` (default `https://api.openai.com/v1/models`).
   5xx responses fail; 4xx is treated as reachable (auth errors still
   mean the host answered). Override `ALBERT_PROVIDER_URL` in tests or
   air-gapped setups to point somewhere local.

Output is a plain-text report with `[ ok ]`, `[warn]`, `[fail]` prefixes.

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
