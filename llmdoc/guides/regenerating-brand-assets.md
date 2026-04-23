# How to Regenerate Brand Assets

1. Update `assets/branding/albert-logo-reference.png` when the raster master changes.
2. Keep `assets/branding/albert-logo.svg` only as a reference unless the export workflow intentionally switches source.
3. Run `./scripts/generate_brand_assets.sh`.
4. Inspect:
   - `assets/branding/albert-logo-1024.png`
   - `apps/desktop/src-tauri/icons/icon.png`
   - `apps/desktop/public/favicon-32x32.png`
5. Run `cargo check`.
6. Run `npm run build`.
7. If the padding or crop policy changes, update `llmdoc/reference/brand-assets.md`.

