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
| `Mod + Shift+I` | Open the latest Import report       |
| `Mod + Shift+P` | Toggle the Providers drawer         |
| `Mod + Shift+W` | Open Workspace collections          |
| `Mod + W`       | Close the active endpoint tab       |
| `Mod + /`       | Show keyboard shortcuts overlay     |
| `Mod + Alt + →` | Next endpoint tab (wraps)           |
| `Mod + Alt + ←` | Previous endpoint tab (wraps)       |
| `Mod + 1..9`    | Jump to tab N (1-indexed)           |

Within the sidebar:

- `↓` from the search input focuses the first visible endpoint row.
- `↑` / `↓` within the endpoint list walks through the currently-expanded
  collections. Pressing `↑` on the first row jumps back to the search
  input.
- `Enter` on the search input opens the first visible endpoint.
- Tag chips below the search input filter endpoints by `tags[]`. Click
  a chip to toggle an exclusive "show only this tag" filter; click
  again or press the `✕` chip to clear.
- Imported collection rows show a compact metadata line under the collection
  name: latest update/import timestamp plus endpoint count. The timestamp
  refreshes after re-import, rename, and persisted mock-example edits. Preview
  and fallback rows omit this line.
- Two-token search syntax: when the first whitespace-separated token is
  an HTTP method (`get`, `post`, `put`, `patch`, `delete`, `options`,
  `head`, `trace`) AND a second token is present, the method is matched
  exactly and the second token filters path / summary / operation id.
  So `get /users` narrows to GET endpoints mentioning `/users`, while
  `get` alone still does a single-token method filter. Non-method first
  tokens fall through to single-substring behavior (e.g. `foo bar`
  looks for the literal "foo bar" in any field).

## Import feedback

- Successful SQLite imports include an endpoint-level diff summary in both the
  status bar and success toast: `N added`, `N changed`, `N removed`, and
  `N unchanged`.
- After import, App keeps the latest report in memory. The Import report drawer
  opens automatically and is also available from the command palette or
  `Mod+Shift+I`.
- The report groups endpoint rows by Added, Changed, and Removed. Added and
  Changed rows expose Open and Prompt because the endpoint exists in the latest
  snapshot. Prompt opens the normal generation prompt preview for `success`.
  Changed rows also show coarse reasons (`metadata changed`, `parameters
  changed`, `request body changed`, `responses changed`, `auth changed`) when
  the backend reports them, plus concise details such as `parameter added:
  query status`, `request body schema changed`, or `response changed: 200
  (schema)`. Reasons and details are passed into prompt preview as a
  generation-context note, and Changed rows also expose Refresh to regenerate
  and persist the `success` mock with the same context. When one or more
  changed endpoints are refreshable, the drawer header exposes **Refresh
  changed (n)** to run the same refresh sequentially for every changed endpoint.
  Removed rows are display-only.
- New collections report all endpoints as added. Re-importing the same
  collection id compares the previous canonical snapshot with the new parse
  result before saving.
- Changed endpoints compare the API contract and ignore mock examples, so
  hand-edited or AI-generated examples do not produce false contract-change
  messages.

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

## Workspace collections drawer

- Opens from the database icon next to the top-bar workspace name, from the
  command palette, or via `Mod+Shift+W`.
- Summarizes imported collection count, endpoint count, and data source
  (`SQLite` when Tauri is connected, `Preview` in static fallback mode).
- Collection cards show latest update/import timestamp, endpoint count, origin
  badge, and method distribution chips.
- Imported cards reuse the existing collection actions: Open first endpoint,
  Rename, Export, and Delete. The drawer header also exposes Refresh and
  Import. Refresh is disabled in fallback preview mode.
- If no imported collection exists, the drawer falls back to the preview/demo
  collections so the layout remains inspectable in browser-only development.

## Providers panel

- Presets select the provider API shape (`openai_compatible`,
  `openai_responses`, `azure_openai`, or `azure_openai_responses`) and include
  the current default generation controls.
- Saved profiles persist only non-secret provider settings through Tauri:
  provider name, optional environment label, base URL, model, API key
  environment variable, provider API type, Azure deployment/version,
  temperature, max output tokens, reasoning effort, and schema repair retry
  count.
- Saved profiles can be filtered by environment label. Blank legacy labels are
  grouped as `default`; presets seed `local` for OpenAI-compatible defaults and
  `staging` for Azure examples.
- API key entry is session-only and is never saved into profiles.
- Generation controls live in the Active provider form. Temperature is edited
  with a range control plus numeric input, clamped from `0` to `2`, and defaults
  to `0.7`. Max output tokens is a numeric input; blank means "provider
  default" and is omitted from outbound provider requests. Reasoning effort is
  a select with Default / None / Minimal / Low / Medium / High / Xhigh; Default
  stores `null` and omits the `reasoning` request object. The control is
  persisted for every profile, but the current adapter sends it only for
  OpenAI/Azure Responses providers. Schema repair retries is a numeric control
  clamped to `0..=5`; blank means the adapter default of `2`, while `0`
  disables bounded repair after the initial schema validation failure.

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
  6. **Proxy upstream** — single text input + Apply/Disable buttons
     that set `GatewayConfig.proxy_upstream`. When active, unmatched
     routes forward to the upstream base URL instead of returning
     404. Useful for hybrid setups: mock a subset, let the rest hit a
     real staging / partner API. Requests bodies above 4KB are
     truncated before proxying (see gateway-routes.md for details).
  7. **Scenarios** (`components/ScenariosPanel.tsx`) — named presets
     that snapshot the live `GatewayConfigBundle` to SQLite. Save with
     a label, one-click Load to snap the gateway into that state,
     Rename in-place, Delete with confirm-less removal. Typical use:
     save "healthy", "rate limited", and "broken backend" and toggle
     between them while demoing integrations. Inline table renders
     under the Schema enforcement panel; empty state shows
     "No scenarios yet". Save requires the server to be running.
- **Routes tab** — one row per registered route, with a dropdown to pick
  the served example kind. Changes collect as a draft; `Apply (N)` sends
  them to `update_mock_server`. Below the override rows, **Conditional
  examples** edits `GatewayConfig.conditional_example_rules` for the same
  route keys. It supports query/header/body equality conditions, multiple
  conditions per rule (AND), rule ordering with first-match-wins semantics,
  and a draft-then-Apply flow. Body equality values are parsed as JSON when
  possible, so `2`, `true`, and objects are not forced to strings.
- **Requests tab** — a metrics summary (total, 2xx/4xx/5xx counts, avg
  and max latency, a 15-minute request-rate sparkline with 5xx shares
  tinted in error color, busiest route, top-5 route breakdown with
  p50/p95),
  a 2xx/4xx/5xx filter chip row + method dropdown + a free-text search
  box (matches path / matched_route / status / request_id / query),
  followed by the scrolling log. Each row shows timestamp, method, path, status,
  latency, served-kind or source label, a small `id:<prefix>`
  pill for the `x-request-id` (clicking copies the full id), and a
  `cURL` button that clipboards a runnable cURL command reproducing
  the captured request — URL targets the live gateway base, original
  `x-request-id` is preserved as a header, the captured body (if any)
  is re-embedded with `'\''` shell escaping and the `…[truncated]`
  sentinel stripped first. `<capture failed:…>` sentinels are
  respected and omitted from the body.
  A "Capture request bodies" toggle arms the backend to store up to
  4KB per request; when captured, rows expose a `<details>` body
  preview. **Export JSON** streams the current log as a timestamped
  download; **Export CSV** does the same with RFC 4180-quoted
  cells for pasting into Excel / Google Sheets. **Clear** wipes both
  the log and the cumulative metrics
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
When the Tauri runtime is available, the latest Try-it response also
shows **Save as mock**. It maps `2xx/3xx` responses to the `success`
example, `204` to `empty`, and `4xx/5xx` to `error`, then exposes a
compact `as Success|Empty|Error` select so the user can override the
slot before persisting. The response body is saved through
`save_mock_example` with a `Captured from Try-it` note. Before saving,
the frontend asks the Tauri backend to run `validate_mock_payload`
against the selected response schema, so mismatch warnings use the same
canonical Rust validator as AI generation and gateway schema checks.
Static previews or older backends fall back to a lightweight frontend
check. A warning banner is shown when the captured payload looks
mismatched; the save still proceeds so real upstream behavior is not
lost. The
existing `saveExample` action refreshes the endpoint tab and selects the
saved example kind, while the Try-it button briefly flips to `Saved` so
the user has local confirmation. This is the first manual slice of the
"record real response → mock example" workflow. The same latest response action
row also exposes **AI refresh latest** and **Prompt latest** when provider
actions are wired. Both use the latest Try-it request/response snapshots as
`generation_context`; the mock-kind select controls the target
`success | empty | error` slot.

After every successful Try-it send against a persisted collection, the
panel best-effort calls `save_request_cache` with a normalized request
snapshot and response snapshot. The row stores into SQLite under
`request_fingerprint_cache`; repeated method/path/query/body/header-shape
requests increment `hit_count`. The action row shows `cached` for a first
capture and `cache hit ×N` for repeated fingerprints, with a small
timestamp line above the response. If Mock Server Request cache routing is
currently enabled, the latest response area shows **Reload routing** after a
successful cache write; clicking it calls the same reload action as the Runtime
tab so the just-captured fingerprint can serve immediately. The panel also
loads the five most recent cached fingerprints for the active endpoint; each
row shows status, last-seen time, fingerprint, hit count, age, a Replay button,
a Remove button, a mock-kind select, a Save button that writes the cached
response body into the selected `success | empty | error` slot through the same
`save_mock_example` path, an **AI refresh** button, and a **Prompt** button.
AI refresh uses the cached request/response snapshots as
`generation_context` for `generate_mock_example` and persists the generated
payload into the selected mock slot. Prompt opens the same prompt preview
modal with that cache context included, so users can inspect what the model
will see before refreshing. Rows older than 24 hours are marked `stale`;
Replay loads the cached request snapshot (`query`, headers, body) back into
the Try-it draft so the user can resend it and refresh the fingerprint
deliberately. When stale rows exist, Try-it shows a visible **Refresh queue**
above the collapsible cache list. The queue summarizes stale vs refreshable
rows, exposes **AI refresh stale (n)** for stale rows whose response body can
be reused, **Preview first** for the first refreshable stale prompt, and
**Clear stale (n)**, which deletes stale rows only for the active
collection/method/path. Stale batch refresh runs serially through the same
cache-contextual generation path as per-row **AI refresh**; it does not resend
requests or remove cache rows.
Static previews, fallback collections, and older backends degrade to
`cache unavailable` or an empty cache list without blocking the request
or Save-as-mock path. Sensitive header values are redacted by the storage
layer before fingerprinting and persistence.
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
- Per-kind **Generate**, **Generate all**, and **Preview prompt** include each
  target slot's current mock example as `generation_context` when that slot
  already has a payload, so a hand edit or previous AI result becomes the next
  iteration's reference instead of being discarded.

## Gateway preferences

The Runtime tab includes **Request cache routing**, an opt-in toggle that
loads recent Try-it cache rows into the running gateway. Its meta label shows
how many cache entries are currently injected. When enabled, **Reload request
cache** re-runs `update_mock_server(use_request_cache=true)` so newly captured
Try-it rows can participate without a listener restart. Matching
method/path/query/header/body fingerprints return the cached response before
falling back to ordinary mock example selection; the response includes
`x-albert-mock-source: cache` and `x-albert-cache-fingerprint`. Query overrides
and explicit route overrides still win over cache routing. The gateway consumes
only the injected in-memory cache map and never queries SQLite on the request
path.

The Mock Server panel persists the full runtime config across sessions,
not just host/port/cors. On startup, `load_gateway_preferences` returns
the persisted payload (if any); on every successful `start_mock_server`
and `update_mock_server` the current `status.config` is written back via
`save_gateway_preferences`. Persisted fields include:

- `host`, `port`, `cors_enabled`
- `example_overrides`, `conditional_example_rules`, `use_request_cache`,
  `default_latency_ms`, `latency_overrides`
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
