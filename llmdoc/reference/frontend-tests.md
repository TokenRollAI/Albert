# Frontend Tests

## Setup

- `apps/desktop/vitest.config.ts` — jsdom environment, globals enabled,
  picks up `src/**/*.{test,spec}.{ts,tsx}`.
- `apps/desktop/src/test/setup.ts` — swaps `window.localStorage` and
  `sessionStorage` with an in-memory shim (jsdom's default behaved oddly
  when multiple tests ran back-to-back), stubs `navigator.clipboard`,
  and clears both storages before + after each test.

## Scripts

- `npm --workspace apps/desktop run test` — single run (used in CI).
- `npm --workspace apps/desktop run test:watch` — interactive watch.
- `npm test` (root) — runs `cargo test --workspace` then the vitest suite.
- `npm run check` (root) — `cargo fmt --check` + clippy + full test run. Use
  locally before pushing; CI runs the same gates.

## Current suites

- `components/__tests__/JsonView.test.tsx` — asserts the tokenizer emits
  the right CSS classes for keys / strings / numbers / booleans / null,
  handles nested objects and arrays, and survives a circular reference.
- `components/__tests__/Markdown.test.tsx` — covers paragraphs, code
  spans, bold, italic, absolute links (`target=_blank` + `rel`), and
  line-break preservation.
- `hooks/__tests__/useTryItDraft.test.tsx` — verifies per-route storage
  isolation, rehydration on remount, reset semantics, and the
  `seedTryItDraft` → live-hook event bridge.
- `hooks/__tests__/useTryItHistory.test.tsx` — verifies the 5-entry cap,
  cross-session rehydration, and `clear` semantics.
- `hooks/__tests__/useEndpointTabs.test.tsx` — verifies tab persistence
  into `localStorage`, `restoreTabs` rehydrating persisted tabs, skipping
  tabs whose endpoint disappeared, and idempotency when tabs already
  exist.
- `components/__tests__/RateLimitsEditor.test.tsx` — adds, removes, and
  applies rate-limit rules, verifying the draft-vs-value dirty gate and
  the exact payload shipped to `onApply`.
- `components/__tests__/StatusOverridesEditor.test.tsx` — adds,
  rejects out-of-range codes, removes, and applies the shrunk map.
- `components/__tests__/ResponseHeadersEditor.test.tsx` — flatten /
  unflatten round-trip semantics + add/replace/remove via the
  flattened-row UI.
- `lib/__tests__/fetchErrors.test.ts` — covers `validateFetchUrl`
  (blank / non-http / unparseable input) and `friendlyFetchError`
  (rewrites of the invalid-URL / unsupported-scheme / network /
  timeout / HTTP 4xx / oversized-payload variants).
- `lib/__tests__/authHints.test.ts` — covers the hint→RequiredHeader
  conversion for bearer / basic / OAuth2 / apiKey-header schemes and
  the `seedRequiredHeadersFromEndpoints` batch helper.
- `components/__tests__/RequestPanel.auth.test.tsx` — renders the
  RequestPanel with each `auth.scheme` variant and asserts the
  warning-tinted chip copy (bearer / api-key / OAuth2 / missing).
- `components/__tests__/TryItPanel.auth.test.tsx` — verifies the
  placeholderForAuthHint helper and the auto-seed effect: the panel
  prefills an Authorization (or custom) header row on first render
  when the endpoint declares auth, and never overwrites a user-edited
  draft header.
- `components/__tests__/MockRequestsTab.test.tsx` — pins down the
  pure-function `computeMetrics`: status-class buckets, average/max
  latency, busiest-route detection, empty-log safety, and the
  `METHOD path` fallback when `matched_route` is null.
- `hooks/__tests__/useAppDrawers.test.tsx` — covers the drawer-state
  hook: independent slots, open/close/toggle/set semantics.
- `lib/__tests__/fuzzy.test.ts` — fuzzy matcher used by the command
  palette: in-order character matching, word-boundary + contiguity
  bonuses, stable sort on ties.
- `lib/__tests__/downloadBlob.test.ts` — covers `timestampSlug` and the
  anchor-based download helper (mocks `URL.createObjectURL`).
- `components/__tests__/CommandPalette.test.tsx` — open/close render,
  query narrowing, arrow-key navigation with wrap, Enter vs. Esc
  dispatch, empty-result copy, action-kind run path.
- `lib/__tests__/jsonLint.test.ts` — the Try-it body lint helper:
  empty-ok, valid JSON scalars/objects/arrays, malformed input with
  line/column extraction, trailing-comma rejection.
- `lib/__tests__/queryString.test.ts` — the key=value query builder
  parse/serialize round-trip: leading `?` stripping, standalone keys,
  `+` → space, percent-escape fidelity, blank-key rejection on
  serialize.
- `components/__tests__/TryItPanel.auth.test.tsx` now also covers the
  `formatBytes` helper (byte / kB / MB thresholds + safety for
  NaN/negatives).

## CI integration

The Linux job in `.github/workflows/ci.yml` runs `npm test` before the
`npm run build` step, so a broken frontend test fails the run before
we waste time on the production build.
