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
- `lib/__tests__/authHints.test.ts` — covers the hint→RequiredHeader
  conversion for bearer / basic / OAuth2 / apiKey-header schemes and
  the `seedRequiredHeadersFromEndpoints` batch helper.

## CI integration

The Linux job in `.github/workflows/ci.yml` runs `npm test` before the
`npm run build` step, so a broken frontend test fails the run before
we waste time on the production build.
