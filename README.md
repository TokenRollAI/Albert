# Albert

[中文说明](./README.zh-CN.md)

<p align="center">
  <img src="./assets/branding/albert-logo-reference.png" alt="Albert monkey astronaut logo" width="420" />
</p>

Albert is an AI-driven desktop client for API mocking. It is designed for frontend and client developers who want to turn API descriptions into usable mock responses without spending time on repetitive field wiring and fake-data authoring.

## Why Albert

Most mock tools lean on the word "Monkey" to imply playful simulation and test doubles. Albert takes a different route.

- `Albert` is named in tribute to Albert II, the first monkey sent into space.
- The name keeps the semantic wink between `mock` and `monkey`, but moves the product identity away from generic monkey branding.
- The primary product mark is the monkey astronaut emblem stored under `assets/branding/`.

## Phase 1 Scope

Phase 1 is intentionally narrow. The current repository establishes:

- bilingual project documentation
- a workspace and module skeleton
- a Tauri + React + TypeScript desktop shell
- Rust crates for canonical schema, parsing, storage, gateway, and OpenAI integration
- placeholder implementations that mark the intended extension points with explicit `not implemented` behavior

Phase 1 does not yet include:

- production-ready OpenAPI or cURL parsing
- a live local HTTP mock gateway
- SQLite persistence wiring
- dynamic caching or request fingerprinting
- AI runtime generation

## Architecture Snapshot

- `apps/desktop`: Tauri desktop shell and React control panel
- `crates/albert-core`: canonical domain model and shared contracts
- `crates/albert-parser`: OpenAPI and cURL parser entry points
- `crates/albert-storage`: SQLite-facing repository and migration boundary
- `crates/albert-gateway`: local mock gateway boundary
- `crates/albert-openai`: OpenAI Chat Completions adapter boundary
- `docs/`: PRD, architecture, roadmap, and open questions
- `llmdoc/`: persistent project knowledge for future implementation work

## Documents

- [PRD](./docs/prd.md)
- [Architecture](./docs/architecture.md)
- [Roadmap](./docs/roadmap.md)
- [Open Questions](./docs/open-questions.md)

## Brand Assets

- Export master raster: [assets/branding/albert-logo-reference.png](./assets/branding/albert-logo-reference.png)
- Vector reference: [assets/branding/albert-logo.svg](./assets/branding/albert-logo.svg)
- Exported PNG set: [assets/branding](./assets/branding)
- Desktop app icons: [apps/desktop/src-tauri/icons](./apps/desktop/src-tauri/icons)
- Web favicon set: [apps/desktop/public](./apps/desktop/public)
- Regenerate all brand assets: `./scripts/generate_brand_assets.sh`

## Workspace Layout

```text
.
|-- assets/
|   `-- branding/
|-- apps/
|   `-- desktop/
|       |-- src/
|       `-- src-tauri/
|-- crates/
|   |-- albert-core/
|   |-- albert-gateway/
|   |-- albert-openai/
|   |-- albert-parser/
|   `-- albert-storage/
|-- docs/
|-- fixtures/
|-- scripts/
`-- llmdoc/
```

## Getting Started

The scaffold is intentionally lightweight and some modules are placeholders.

```bash
npm install
npm run dev
```

To prepare the desktop runtime after dependencies are installed:

```bash
npm --workspace apps/desktop run tauri:dev
```

## Current Status

- React UI scaffold: available
- Tauri shell: wired with a bootstrap command
- Canonical schema model: defined
- Parser, storage, gateway, provider crates: scaffolded
- Full implementation: pending future phases
