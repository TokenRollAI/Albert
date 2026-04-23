# Decision 0001: Foundation Scope

## Decision

Phase 1 is a foundation-oriented release that prioritizes documentation, repository structure, canonical modeling, and extension-point scaffolding over end-to-end feature completeness.

## Rationale

- The project is greenfield.
- Architecture drift is most likely at the beginning.
- Canonical schema and module boundaries must stabilize before runtime features expand.

## Consequences

- Some crates intentionally return `not implemented`.
- UI exists as a complete shell before backend capabilities are fully wired.
- Future phases must upgrade placeholders rather than invent new boundaries.

