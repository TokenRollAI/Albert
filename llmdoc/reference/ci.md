# CI Reference

## Scope

This document captures the current GitHub Actions validation baseline.

## Workflow

`.github/workflows/ci.yml` fans out into two jobs:

- **test (ubuntu-latest)** — fast, cheap checks on the platform-independent
  crates.
  - `cargo fmt --all --check`
  - `cargo clippy -- -D warnings` on
    `albert-core / -parser / -storage / -gateway / -openai / -cli`
  - `cargo test` on the same six crates (all 49 unit + integration tests)
  - `npm --workspace apps/desktop run build`
  - The Tauri crate is excluded here because the Linux runner lacks
    GTK/WebKit system libraries required by `tauri`.
- **desktop (macos-latest)** — platform-specific validation.
  - `cargo check --workspace` (includes `albert-desktop`)
  - `cargo clippy -p albert-desktop -- -D warnings`
  - `npm run build`
  - `./scripts/generate_brand_assets.sh` + `git diff --exit-code` guard
    ensuring regenerated assets are committed.

Concurrency is keyed on `workflow + ref` so redundant runs get cancelled
when a branch receives new pushes.

## Stable Facts

- CI runs on `push` to `main` and on `pull_request`.
- Both Rust and frontend build errors fail the run.
- `clippy -D warnings` holds the workspace to zero new lints.
- Brand asset regeneration is deterministic; any drift from `scripts/generate_brand_assets.sh` causes CI to fail.

## Sources of Truth

- `.github/workflows/ci.yml`
- `scripts/generate_brand_assets.sh`
