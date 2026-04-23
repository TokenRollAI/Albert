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
- `llmdoc/architecture/request-flow.md`: current import / mock / AI flows
- `llmdoc/architecture/runtime-state.md`: shared runtime services and tokio lifecycle
- `llmdoc/reference/brand-assets.md`: brand asset source-of-truth and export rules
- `llmdoc/reference/canonical-schema.md`: internal schema model
- `llmdoc/reference/ci.md`: CI validation baseline
- `llmdoc/reference/repo-layout.md`: repository map
- `llmdoc/reference/gateway-routes.md`: mock gateway routing rules and special paths
- `llmdoc/reference/openai-adapter.md`: OpenAI adapter contract and prompt schema
- `llmdoc/reference/ux-conventions.md`: keyboard shortcuts, toasts, drawer shell
- `llmdoc/reference/cli.md`: albert-cli headless binary (serve / import / list / export)
- `llmdoc/reference/remote-fetch.md`: Tauri fetch_remote_source command + URL import UX

## Routing Rules

- Read `startup.md` first.
- Read `reference/brand-assets.md` and `guides/regenerating-brand-assets.md` before changing logo or icon assets.
- Read `reference/canonical-schema.md` before editing parser, gateway, or storage code.
- Read `reference/ci.md` before changing repository validation workflows.
- Read `reference/repo-layout.md` before adding new modules or moving files.
- Read `reference/gateway-routes.md` before modifying route matching, example selection, or mock response headers.
- Read `reference/openai-adapter.md` before changing provider requests, prompt construction, or response parsing.
- Read `memory/decisions/` before changing foundational scope.
