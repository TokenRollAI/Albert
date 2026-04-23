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
- **Requests tab** — scrolling log. Each row shows timestamp, method,
  path, status, latency, served-kind or source label.
