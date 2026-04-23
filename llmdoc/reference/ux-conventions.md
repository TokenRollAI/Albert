# UX Conventions

## Keyboard Shortcuts

Registered globally via `useKeyboardShortcuts`. `Mod` = `Cmd` on macOS,
`Ctrl` elsewhere.

| Combo           | Action                              |
|-----------------|-------------------------------------|
| `Mod + K`       | Focus the collection search input   |
| `Mod + .`       | Toggle the Mock Server drawer       |
| `Mod + I`       | Open the Import dialog              |
| `Mod + Shift+P` | Toggle the Providers drawer         |
| `Mod + W`       | Close the active endpoint tab       |

Within the sidebar:

- `↓` from the search input focuses the first visible endpoint row.
- `↑` / `↓` within the endpoint list walks through the currently-expanded
  collections. Pressing `↑` on the first row jumps back to the search
  input.
- `Enter` on the search input opens the first visible endpoint.

Shortcuts are suppressed while focus is inside an editable element unless
a modifier key is held. Conventions live in
`apps/desktop/src/hooks/useKeyboardShortcuts.ts`.

## Toasts

`useToasts` returns `{toasts, push, info, success, warn, error, dismiss}`.
Every event is auto-dismissed after 3.5s (6s for errors). Renderer is
`components/ToastHost`. Prefer toasts for *transient* confirmations and
errors; use the status bar for persistent state like runtime or collection
counts.

## Drawer panels

- Full-screen overlay with a 540–720px right panel.
- Header pills: `pill--ok | pill--warn | pill--idle` for lifecycle state.
- Body uses `.panel` sections for logical grouping; tabs when a drawer has
  three or more distinct views (e.g. Mock Server: Runtime / Routes /
  Requests).

## Mock Server panel

- **Runtime tab** — host / port / CORS + start/stop + Chaos controls
  (default latency in ms, error rate in %). Reset button restores both to
  zero.
- **Routes tab** — one row per registered route, with a dropdown to pick
  the served example kind. Changes collect as a draft; `Apply (N)` sends
  them to `update_mock_server`.
- **Requests tab** — a metrics summary (total, 2xx/4xx/5xx counts, avg
  and max latency, busiest route) followed by the scrolling log. Each
  row shows timestamp, method, path, status, latency, served-kind or
  source label. A "Capture request bodies" toggle arms the backend to
  store up to 4KB per request; when captured, rows expose a `<details>`
  body preview. Rows whose `matched_route` points at a known local
  endpoint are clickable — selecting one opens the endpoint tab and
  seeds the Try-it draft with the recorded query + body so the user
  can tweak and replay.

## Try-it panel

Lives under the response pane when an endpoint tab is open. Reads the
currently running mock gateway's bind address and lets the user send a
request with:

- path-parameter inputs (auto-extracted from `{id}` tokens)
- a query string
- repeatable `KEY: VALUE` custom headers (auth tokens, etc.)
- a JSON body draft (shown only for non-GET/HEAD methods)

Drafts are persisted in `localStorage` keyed by `METHOD /path` via
`useTryItDraft`, so switching tabs or restarting the app preserves every
field. A **Clear** button wipes the draft for the active route.

The panel displays the response status, elapsed ms, select headers
(`x-albert-*`, `content-type`), and body via `JsonView`.

## Mock example editing

Every mock payload can be edited directly without going through the AI.
In the response pane:

- **Edit** toggles the JSON payload into a textarea seeded with the
  current value.
- **Save** parses the draft, fails fast with a `banner--error` if the
  JSON is malformed, and otherwise calls the `save_mock_example` Tauri
  command which upserts via `SqliteStore::replace_mock_example` and
  refreshes the tab state.
- **Generate all** (next to the per-kind Generate button) runs
  success → empty → error sequentially, updating the UI as each
  completes and surfacing a single toast with the final success count.

## Gateway preferences

The Mock Server panel remembers the last-used host/port/cors combo
across sessions. On app startup, `load_gateway_preferences` returns the
persisted payload (if any); on every successful `start_mock_server`
the current host/port/cors is written back via `save_gateway_preferences`.
The persistence is a single-row SQLite table (`gateway_preferences`)
whose payload is an arbitrary JSON object — extending the shape on the
frontend doesn't require a migration.
