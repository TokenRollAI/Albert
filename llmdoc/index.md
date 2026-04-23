# llmdoc Index

## Purpose

- This file is the global map of the Albert documentation system.
- Startup reading begins at `llmdoc/startup.md`.

## Categories

- `must/`: recurring startup context
- `overview/`: product identity and scope
- `architecture/`: runtime boundaries and flows
- `guides/`: focused implementation workflows
- `reference/`: stable schemas, layouts, and conventions
- `memory/`: decisions, reflections, and doc gaps

## Key Documents

- `llmdoc/startup.md`: startup reading order
- `llmdoc/overview/project-overview.md`: project identity and boundaries
- `llmdoc/architecture/system-boundaries.md`: module ownership and dependency direction
- `llmdoc/architecture/request-flow.md`: planned import and mock flows
- `llmdoc/reference/brand-assets.md`: brand asset source-of-truth and export rules
- `llmdoc/reference/canonical-schema.md`: internal schema model
- `llmdoc/reference/ci.md`: CI validation baseline
- `llmdoc/reference/repo-layout.md`: repository map

## Routing Rules

- Read `startup.md` first.
- Read `reference/brand-assets.md` and `guides/regenerating-brand-assets.md` before changing logo or icon assets.
- Read `reference/canonical-schema.md` before editing parser, gateway, or storage code.
- Read `reference/ci.md` before changing repository validation workflows.
- Read `reference/repo-layout.md` before adding new modules or moving files.
- Read `memory/decisions/` before changing foundational scope.
