# Working Agreement

## Collaboration

- The main assistant aligns with the user before non-trivial scope or architecture edits.
- Keep asking focused follow-up questions when future direction is still unclear.
- Prefer progress with explicit assumptions over blocked execution.

## Language

- Assistant responses to the user should be in Simplified Chinese.
- Project-facing docs may be bilingual when needed.

## Architecture Rules

- Do not store raw OpenAPI as the only source of truth.
- Route parser output through Canonical API Schema.
- Keep crate dependencies flowing inward toward `albert-core`.
- Mark unfinished behavior explicitly as `not implemented`.

