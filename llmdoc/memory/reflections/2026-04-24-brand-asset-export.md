# Brand Asset Export Reflection

## Task

- Add the monkey astronaut logo to the repository.
- Generate desktop and web icon assets.
- Align the exported icons with the user's preferred composition.

## Expected vs Actual

- Expected outcome: reuse the provided logo faithfully and derive icons without visual drift.
- Actual outcome: the first attempt recreated the mark too loosely, then a later SVG-based export was visually off-center, and finally the accepted result came from the original raster with height-based square cropping.

## What Went Wrong

- I first treated the provided image as a reference to redraw instead of preserving it as the asset source.
- I assumed a generic square-centering rule would work for the SVG version.
- I missed that Tauri requires `RGBA` PNG icons even when the image is visually opaque.

## Root Cause

- The asset workflow was not documented yet.
- The export rules for this specific logo were not stable until the user clarified the desired crop policy.
- Platform icon constraints were discovered only through build validation.

## Missing Docs or Signals

- The repository lacked a stable brand asset reference doc.
- The repository lacked a repeatable guide for regenerating icons.
- The Tauri RGBA constraint was not documented.

## Promotion Candidates

- Promote the source-of-truth files and crop policy into `reference/`.
- Promote the regeneration sequence and verification steps into `guides/`.

## Follow-up

- Keep the raster asset as the approved export source until the user explicitly changes that decision.
- Reuse `./scripts/generate_brand_assets.sh` for all future icon regeneration.

