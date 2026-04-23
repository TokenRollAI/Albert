# CI Reference

## Scope

This document captures the current GitHub Actions validation baseline.

## Workflow

- `.github/workflows/ci.yml`

## Stable Facts

- CI runs on `push` to `main` and on `pull_request`.
- CI checks Rust formatting with `cargo fmt --all --check`.
- CI runs unit tests for `albert-core`, `albert-parser`, and `albert-storage`.
- CI runs `cargo check --workspace`.
- CI runs `npm run build`.
- CI regenerates brand assets and fails if tracked outputs drift.

## Sources of Truth

- `.github/workflows/ci.yml`
- `scripts/generate_brand_assets.sh`
