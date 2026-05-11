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
  - `cargo test` on the same six crates (82 unit + integration tests)
  - `npm --workspace apps/desktop run test` (vitest, jsdom)
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

`.github/workflows/release-build.yml` owns distributable desktop artifacts:

- Triggers on published GitHub releases and on manual `workflow_dispatch`.
- Builds four matrix entries:
  - macOS arm64 (`macos-latest`, `--target aarch64-apple-darwin --bundles app`)
  - macOS x64 (`macos-latest`, `--target x86_64-apple-darwin --bundles app`)
  - Linux x64 (`ubuntu-22.04`, `--bundles deb`)
  - Windows x64 (`windows-latest`, `--bundles nsis`)
- Uses `tauri-apps/tauri-action@v0.6.2` with `projectPath:
  apps/desktop` and `npm run tauri:ci`; the action appends `build` and
  matrix args.
- Linux installs the Tauri v2 WebKitGTK/AppIndicator build dependencies
  before running the bundle build.
- Every matrix entry stages normalized files in `dist-artifacts/` and uploads
  GitHub Actions artifacts named `Albert-<platform>`. Release-triggered runs
  also upload those files to the matching GitHub Release; manual runs can opt
  in by setting `upload_to_release=true` and `tag_name=<release tag>`.

## Stable Facts

- CI runs on `push` to `main` and on `pull_request`.
- Both Rust and frontend build errors fail the run.
- `clippy -D warnings` holds the workspace to zero new lints.
- Brand asset regeneration is deterministic; any drift from `scripts/generate_brand_assets.sh` causes CI to fail.
- Tauri bundling is enabled in `apps/desktop/src-tauri/tauri.conf.json`;
  release artifacts depend on that config plus the desktop package's
  `tauri:ci` npm script.
- macOS release builds currently upload `.app` bundles rather than DMG files;
  the workflow zips the `.app` directory before uploading it as an artifact or
  release asset. DMG/notarized distribution should be added after
  signing/notarization secrets and runner behavior are explicitly configured.

## Sources of Truth

- `.github/workflows/ci.yml`
- `.github/workflows/release-build.yml`
- `scripts/generate_brand_assets.sh`
