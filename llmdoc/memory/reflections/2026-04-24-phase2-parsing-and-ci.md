# Phase 2 Parsing And CI Reflection

## Task

- Move Albert from scaffold-only modules into a minimally working Phase 2 import pipeline.
- Add unit tests with real edge cases.
- Add GitHub Actions validation.

## Expected vs Actual

- Expected outcome: wire parsing and persistence without losing the clean module boundaries from Phase 1.
- Actual outcome: the main boundaries held, but parser details and icon format constraints needed several validation rounds before settling.

## What Went Wrong

- It was easy to overfit early parser logic to happy-path fixtures.
- cURL parsing initially treated protocol headers as domain parameters.
- The parser implementation surfaced several type-level issues only after adding real test coverage.

## Root Cause

- The codebase had stable skeletons but no executable Phase 2 behavior yet.
- Real parser behavior needed both mature libraries and aggressive tests to stay honest.

## Missing Docs or Signals

- The repo lacked a CI reference doc.
- The current phase status in stable docs lagged behind the implementation state.

## Promotion Candidates

- Keep CI rules documented in `reference/ci.md`.
- Keep parser/storage behavior anchored to tests before broadening feature scope.

## Follow-up

- Expand UI around imported collection switching and richer endpoint detail.
- Start Phase 3 only after the current import and persistence path stays stable for a few iterations.

