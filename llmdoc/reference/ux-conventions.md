# UX Conventions

## Keyboard Shortcuts

Registered globally via `useKeyboardShortcuts`. `Mod` = `Cmd` on macOS,
`Ctrl` elsewhere.

| Combo           | Action                              |
|-----------------|-------------------------------------|
| `Mod + K`       | Focus the collection search input   |
| `Mod + P`       | Open the command palette            |
| `Mod + .`       | Toggle the Mock Server drawer       |
| `Mod + I`       | Open the Import dialog              |
| `Mod + Shift+P` | Toggle the Providers drawer         |
| `Mod + W`       | Close the active endpoint tab       |
| `Mod + /`       | Show keyboard shortcuts overlay     |

Within the sidebar:

- `↓` from the search input focuses the first visible endpoint row.
- `↑` / `↓` within the endpoint list walks through the currently-expanded
  collections. Pressing `↑` on the first row jumps back to the search
  input.
- `Enter` on the search input opens the first visible endpoint.
- Tag chips below the search input filter endpoints by `tags[]`. Click
  a chip to toggle an exclusive "show only this tag" filter; click
  again or press the `✕` chip to clear.

Shortcuts are suppressed while focus is inside an editable element unless
a modifier key is held. Conventions live in
`apps/desktop/src/hooks/useKeyboardShortcuts.ts`.

## Command palette

`Mod+P` opens `components/CommandPalette.tsx` — a centered modal that
fuzzy-matches against a single combined list of:

- Every imported endpoint, labeled `METHOD /path` with the collection
  name or operation summary as the subtitle.
- A handful of built-in actions: toggle theme, open each drawer,
  start/stop mock server, show shortcuts.

Scoring lives in `lib/fuzzy.ts`: word-boundary characters and contiguous
runs both score higher, so `gus` promotes `GET /users` over
`GETSTATUSAPI`. Up/down arrows navigate, Enter runs the selection, Esc
(or backdrop click) closes. Mouse hover syncs the selection index so
the keyboard and pointer stay coherent.

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
  zero. Below Chaos the Runtime tab stacks four per-route editors (all
  follow the same draft-then-Apply pattern and ship via
  `update_mock_server`):
  1. **Rate limits** (`components/RateLimitsEditor.tsx`) — `METHOD /path →
     {limit, window_ms}`. Limit 0 models a maintenance window (429 for
     every request).
  2. **Status overrides** (`components/StatusOverridesEditor.tsx`) —
     `METHOD /path → u16`. Replaces the kind-default HTTP status. Clamped
     100–599 at the UI layer with a visible hint.
  3. **Response headers** (`components/ResponseHeadersEditor.tsx`) —
     `METHOD /path → {header: value}`. Flattens the two-level map into
     row-edits; re-adding an existing (route, name) replaces the value
     instead of duplicating.
  4. **Auth gates** — single "Seed from OpenAPI security" button that
     walks every imported endpoint, converts the captured `auth` hint
     into a `required_headers` rule (`lib/authHints.ts`), and applies
     them in one shot. Only seedable schemes (HTTP bearer / basic,
     OAuth2, header-placed API keys) emit rules; unsupported schemes
     surface as descriptive notes on the endpoint card.
  5. **Schema enforcement** — single toggle that arms
     `GatewayConfig.enforce_request_bodies`. When on, POST/PUT/PATCH
     bodies whose endpoint declares a `request_body.schema` are
     validated against it; mismatches respond `400 schema_mismatch`
     with a structured body instead of serving the mock payload. Off
     by default so inspection-only workflows are unaffected.
  6. **Scenarios** (`components/ScenariosPanel.tsx`) — named presets
     that snapshot the live `GatewayConfigBundle` to SQLite. Save with
     a label, one-click Load to snap the gateway into that state,
     Rename in-place, Delete with confirm-less removal. Typical use:
     save "healthy", "rate limited", and "broken backend" and toggle
     between them while demoing integrations. Inline table renders
     under the Schema enforcement panel; empty state shows
     "No scenarios yet". Save requires the server to be running.
- **Routes tab** — one row per registered route, with a dropdown to pick
  the served example kind. Changes collect as a draft; `Apply (N)` sends
  them to `update_mock_server`.
- **Requests tab** — a metrics summary (total, 2xx/4xx/5xx counts, avg
  and max latency, busiest route, top-5 route breakdown with p50/p95),
  a 2xx/4xx/5xx filter chip row + method dropdown + a free-text search
  box (matches path / matched_route / status / request_id / query),
  followed by the scrolling log. Each row shows timestamp, method, path, status,
  latency, served-kind or source label, and a small `id:<prefix>`
  pill for the `x-request-id`; clicking that pill copies the full id.
  A "Capture request bodies" toggle arms the backend to store up to
  4KB per request; when captured, rows expose a `<details>` body
  preview. **Export JSON** streams the current log as a timestamped
  download; **Clear** wipes both the log and the cumulative metrics
  (via `mock_server_clear_log`) so users can iterate on a scenario
  without restarting the server. Rows whose `matched_route` points at
  a known local endpoint are clickable — selecting one opens the
  endpoint tab and seeds the Try-it draft with the recorded query +
  body so the user can tweak and replay.

## URL bar

The URL bar above the request/response grid surfaces the active method,
path, and summary. It also hosts a **Copy as cURL** button that
clipboards a one-liner for the active endpoint — targeting the running
mock server's base URL when one is present, falling back to
`https://api.example.com` otherwise.

## Endpoint description

When an endpoint declares a `description`, the RequestPanel renders it
above the sub-tabs via a tiny in-house Markdown renderer
(`components/Markdown.tsx`). Supported spans: `` `code` ``, `**bold**`,
`*italic*`, `[link](https://…)`. Paragraphs split on blank lines, single
newlines become `<br>`. No headings or lists — endpoint descriptions
rarely need more and the renderer stays dependency-free.

## Endpoint auth hint

When the OpenAPI spec declared a `security` requirement for the
endpoint (or inherited one from the document root), the canonical
endpoint carries an `auth` field with enough info to describe the
expected header. RequestPanel shows a compact warning-tinted chip
between the description and the sub-tabs, reading e.g.
`Authorization: Bearer …` or `X-Api-Key: <api key>`. The chip also
echoes the securityScheme description when present. This is purely
informational; gateway enforcement still requires the user to seed
`required_headers` via the Mock Server → Auth gates button.

## Try-it panel

Lives under the response pane when an endpoint tab is open. Reads the
currently running mock gateway's bind address and lets the user send a
request with:

- path-parameter inputs (auto-extracted from `{id}` tokens)
- a structured query-string editor (key=value rows); every edit
  round-trips through `lib/queryString.ts` so the raw form (collapsed
  under a `<details>` summary) stays in sync. Pressing "Add" appends an
  empty row; the trash icon removes one. Blank-keyed rows are dropped
  on serialize so in-progress edits don't leak `&=value` into URLs.
- repeatable `KEY: VALUE` custom headers (auth tokens, etc.)
- a JSON body draft (shown only for non-GET/HEAD methods) with a live
  lint line beneath it — `✓ valid JSON`, `empty body`, or
  `× line N, col M — <parser message>` as the user types. The textarea
  is flagged `aria-invalid` when the body is malformed so screen
  readers and keyboard users both notice. Powered by `lib/jsonLint.ts`
  which wraps the platform's `JSON.parse` and extracts the error
  offset into a 1-based line/column pair
- a "Fill from schema" button next to the body label that calls the
  `synthesize_request_body` Tauri command and replaces the draft with a
  schema-walked sample (only rendered when the endpoint has a declared
  request body)

Drafts are persisted in `localStorage` keyed by `METHOD /path` via
`useTryItDraft`, so switching tabs or restarting the app preserves every
field. A **Clear** button wipes the draft for the active route.

The panel displays the response status, elapsed ms, body size (bytes /
kB / MB), select headers (`x-albert-*`, `content-type`), and body via
`JsonView`. A **Copy body** button next to the status line clipboards
the full response — pretty-printed for JSON, raw text otherwise.
`Mod+Enter` anywhere inside the Try-it surface (including the body
textarea) fires Send, matching the Postman / Insomnia muscle memory.

Every successful send is appended to `useTryItHistory`, a bounded
last-5 history keyed by `METHOD /path` in `localStorage`. The
`<details>Recent (n)</details>` block at the bottom of the panel lists
the status / timestamp / method+url / elapsed ms for each entry; a
`Clear` button wipes the list. History survives across sessions so
users can spot-check whether a change altered response times.

## Tab drag-reorder

Endpoint tabs can be rearranged by dragging them within the tab bar.
Uses the native HTML5 drag API (no dependencies): `draggable` on each
tab, `onDragStart` stamps the id, `onDragOver` marks the drop target
with a left-edge accent rail (`.tab--drop-target`), `onDrop` swaps the
positions via `reorderTabs(fromId, toId)` which splices the dragged
tab into the target's slot. The dragged tab is dimmed mid-drag
(`.tab--dragging`). Reorderings persist through the same
localStorage serialization that powers tab restore, so the last-used
arrangement survives restarts. Tests:
`hooks/__tests__/useEndpointTabs.test.tsx` pins down the splice
semantics, no-op for equal ids, and graceful ignore of unknown ids.

## Endpoint tab persistence

`useEndpointTabs` mirrors the open-tab set into `localStorage` under
`albert.tabs.v1` (tab id + collection id + method + path + inspector +
example). On boot, once `useCollectionData` loads the persisted
collections, App.tsx calls `restoreTabs(storedCollections)` which
re-resolves each persisted ref against the live endpoint tree and
reopens the surviving tabs (along with the last-active id). Tabs whose
collection or endpoint has been deleted are silently dropped rather
than restored with stale data.

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

The Mock Server panel persists the full runtime config across sessions,
not just host/port/cors. On startup, `load_gateway_preferences` returns
the persisted payload (if any); on every successful `start_mock_server`
and `update_mock_server` the current `status.config` is written back via
`save_gateway_preferences`. Persisted fields include:

- `host`, `port`, `cors_enabled`
- `example_overrides`, `default_latency_ms`, `latency_overrides`
- `error_rate`, `capture_bodies`, `enforce_request_bodies`
- `response_headers`, `required_headers`, `rate_limits`
- `status_overrides`, `latency_jitter_ms`

`useGatewayActions.start` reads the saved payload and replays all
enforcement fields on the next start, so restarting the server feels
like a resume, not a reset. The gateway silently ignores route keys
whose endpoints have since been deleted, so stale rules are harmless.

The storage layer is still a single-row SQLite table
(`gateway_preferences`) whose payload is an arbitrary JSON object —
extending the shape on the frontend never requires a migration.
