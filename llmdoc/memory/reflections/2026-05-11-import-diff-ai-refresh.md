# Import Diff AI Refresh Reflection

## Task

- Continue Phase 5 import-evolution work by making re-import drift visible and actionable.
- Connect changed endpoint diff evidence to prompt preview and one-click AI refresh.

## Expected vs Actual

- Expected outcome: show `added / changed / removed / unchanged` and let users inspect changed endpoints.
- Actual outcome: endpoint-level diff became more useful after adding coarse reasons and concise details:
  metadata, parameters, request body, responses, auth, plus parameter/request-body/response detail lines.
- The same evidence now flows into `generation_context.note` for Prompt and Refresh, so AI mock refresh is tied to the import drift instead of a generic endpoint prompt.

## What Went Wrong

- Exporting `importChangeGenerationContext` from `App.tsx` made Vite Fast Refresh fall back to page reloads because the file no longer had component-only exports.
- The fix was to move the helper into `src/lib/importReportContext.ts` and test it as a pure function.

## Root Cause

- Root component files should stay focused on composition. Workflow helpers that are useful in tests or reused by actions belong in `lib/` or hooks, not in `App.tsx`.
- Import diff is easy to overgrow into a full Schema Diff Engine. Keeping this slice to stable, human-readable details avoided premature JSON Pointer diff semantics.

## Missing Docs or Signals

- Stable docs needed to distinguish the current concise import details from a full persisted Schema Diff Engine/version history.
- UX docs needed to spell out that changed-row Prompt and Refresh share the same diff context.

## Promotion Candidates

- Keep `ImportReportPanel` responsible for display/actions only; transform diff rows into AI context in a small pure helper.
- When adding AI workflow context, add both prompt-preview and mutation-path tests so inspection and execution stay aligned.
- Avoid non-component named exports from React root component files to preserve Fast Refresh behavior.

## Follow-up

- If import diff keeps growing, introduce a dedicated `ImportDiffDetail` enum/shape instead of free-form strings.
- Consider stale mock invalidation only after persisted import-history or schema-diff storage exists.
