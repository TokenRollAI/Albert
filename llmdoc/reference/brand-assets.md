# Brand Assets Reference

## Scope

This document captures the stable source-of-truth and export rules for Albert brand assets.

## Source Files

- `assets/branding/albert-logo-reference.png`: primary raster source used for icon exports
- `assets/branding/albert-logo.svg`: vector reference asset kept for reuse and iteration
- `scripts/generate_brand_assets.sh`: the only supported regeneration workflow

## Stable Export Rules

- The approved icon blueprint is based on the raster source, not the SVG reference.
- The square master is created by cropping the original image to `min(iw, ih)` with centered offsets.
- For the current source, that means the export is driven by the original image height.
- `assets/branding/albert-logo-1024.png` is the master square canvas for future scaled PNG exports.

## Platform Rules

- Brand PNG exports in `assets/branding/` come directly from the approved square master.
- Desktop application icons under `apps/desktop/src-tauri/icons/` use a slightly padded app canvas for safer platform masking.
- Web favicons under `apps/desktop/public/` use a tighter canvas to preserve small-size legibility.
- Tauri PNG icons must remain `RGBA`, even on white backgrounds, or `cargo check` will fail during `tauri::generate_context!`.

## Verification

- After regenerating, inspect the exported master and at least one desktop icon and favicon.
- Re-run `cargo check` after icon changes because Tauri validates the icon format.
- Re-run `npm run build` to ensure web assets still resolve.

## Sources of Truth

- `scripts/generate_brand_assets.sh`: export logic
- `README.md`: public-facing asset entry points
- `README.zh-CN.md`: Chinese public-facing asset entry points

