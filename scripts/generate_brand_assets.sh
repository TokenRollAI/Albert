#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAND_DIR="$ROOT_DIR/assets/branding"
PUBLIC_DIR="$ROOT_DIR/apps/desktop/public"
TAURI_ICONS_DIR="$ROOT_DIR/apps/desktop/src-tauri/icons"
TMP_DIR="$BRAND_DIR/.build"
ICONSET_DIR="$TMP_DIR/albert.iconset"
SOURCE_SVG="$BRAND_DIR/albert-logo.svg"
REFERENCE_PNG="$BRAND_DIR/albert-logo-reference.png"
RAW_PNG="$TMP_DIR/raw-source.png"
BASE_PNG="$BRAND_DIR/albert-logo-1024.png"
APP_CANVAS_PNG="$TMP_DIR/albert-logo-app-1024.png"
FAVICON_CANVAS_PNG="$TMP_DIR/albert-logo-favicon-1024.png"

mkdir -p "$BRAND_DIR" "$PUBLIC_DIR" "$TAURI_ICONS_DIR" "$TMP_DIR"
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

if [[ -f "$REFERENCE_PNG" ]]; then
  ffmpeg \
    -y \
    -i "$REFERENCE_PNG" \
    -vf "crop='min(iw,ih)':'min(iw,ih)':(iw-min(iw\\,ih))/2:(ih-min(iw\\,ih))/2,scale=1024:1024:flags=lanczos" \
    -frames:v 1 \
    "$BASE_PNG" >/dev/null 2>&1
elif [[ -f "$SOURCE_SVG" ]]; then
  qlmanage -t -s 2048 -o "$TMP_DIR" "$SOURCE_SVG" >/dev/null 2>&1
  cp "$TMP_DIR/$(basename "$SOURCE_SVG").png" "$RAW_PNG"
  ffmpeg \
    -y \
    -i "$RAW_PNG" \
    -vf "crop='min(iw,ih)':'min(iw,ih)':(iw-min(iw\\,ih))/2:(ih-min(iw\\,ih))/2,scale=1024:1024:flags=lanczos" \
    -frames:v 1 \
    "$BASE_PNG" >/dev/null 2>&1
else
  echo "Missing brand source: expected $REFERENCE_PNG or $SOURCE_SVG" >&2
  exit 1
fi

# Platform-specific canvases derived from the approved square master:
# - desktop/app icons keep a modest safe area for rounded corners and platform masks
# - favicons stay tighter so the monkey mark remains legible at tiny sizes
ffmpeg \
  -y \
  -i "$BASE_PNG" \
  -vf "scale=928:928:flags=lanczos,pad=1024:1024:(ow-iw)/2:(oh-ih)/2:color=white" \
  -frames:v 1 \
  "$APP_CANVAS_PNG" >/dev/null 2>&1

ffmpeg \
  -y \
  -i "$BASE_PNG" \
  -vf "scale=980:980:flags=lanczos,pad=1024:1024:(ow-iw)/2:(oh-ih)/2:color=white" \
  -frames:v 1 \
  "$FAVICON_CANVAS_PNG" >/dev/null 2>&1

resize_png() {
  local size="$1"
  local output="$2"
  local source="${3:-$BASE_PNG}"

  sips -z "$size" "$size" "$source" --out "$output" >/dev/null
}

resize_png_rgba() {
  local size="$1"
  local output="$2"
  local source="${3:-$BASE_PNG}"

  ffmpeg \
    -y \
    -i "$source" \
    -vf "scale=${size}:${size}:flags=lanczos,format=rgba,drawbox=x=0:y=0:w=1:h=1:color=white@0:t=fill" \
    -frames:v 1 \
    "$output" >/dev/null 2>&1
}

resize_png 512 "$BRAND_DIR/albert-logo-512.png"
resize_png 256 "$BRAND_DIR/albert-logo-256.png"
resize_png 128 "$BRAND_DIR/albert-logo-128.png"
resize_png 64 "$BRAND_DIR/albert-logo-64.png"
resize_png 32 "$BRAND_DIR/albert-logo-32.png"
resize_png 16 "$BRAND_DIR/albert-logo-16.png"

resize_png 16 "$ICONSET_DIR/icon_16x16.png" "$APP_CANVAS_PNG"
resize_png 32 "$ICONSET_DIR/icon_16x16@2x.png" "$APP_CANVAS_PNG"
resize_png 32 "$ICONSET_DIR/icon_32x32.png" "$APP_CANVAS_PNG"
resize_png 64 "$ICONSET_DIR/icon_32x32@2x.png" "$APP_CANVAS_PNG"
resize_png 128 "$ICONSET_DIR/icon_128x128.png" "$APP_CANVAS_PNG"
resize_png 256 "$ICONSET_DIR/icon_128x128@2x.png" "$APP_CANVAS_PNG"
resize_png 256 "$ICONSET_DIR/icon_256x256.png" "$APP_CANVAS_PNG"
resize_png 512 "$ICONSET_DIR/icon_256x256@2x.png" "$APP_CANVAS_PNG"
resize_png 512 "$ICONSET_DIR/icon_512x512.png" "$APP_CANVAS_PNG"
cp "$APP_CANVAS_PNG" "$ICONSET_DIR/icon_512x512@2x.png"

iconutil -c icns "$ICONSET_DIR" -o "$TAURI_ICONS_DIR/icon.icns"

resize_png_rgba 512 "$TAURI_ICONS_DIR/icon.png" "$APP_CANVAS_PNG"
resize_png_rgba 32 "$TAURI_ICONS_DIR/32x32.png" "$APP_CANVAS_PNG"
resize_png_rgba 128 "$TAURI_ICONS_DIR/128x128.png" "$APP_CANVAS_PNG"
resize_png_rgba 256 "$TAURI_ICONS_DIR/128x128@2x.png" "$APP_CANVAS_PNG"
resize_png_rgba 30 "$TAURI_ICONS_DIR/Square30x30Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 44 "$TAURI_ICONS_DIR/Square44x44Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 71 "$TAURI_ICONS_DIR/Square71x71Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 89 "$TAURI_ICONS_DIR/Square89x89Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 107 "$TAURI_ICONS_DIR/Square107x107Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 142 "$TAURI_ICONS_DIR/Square142x142Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 150 "$TAURI_ICONS_DIR/Square150x150Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 284 "$TAURI_ICONS_DIR/Square284x284Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 310 "$TAURI_ICONS_DIR/Square310x310Logo.png" "$APP_CANVAS_PNG"
resize_png_rgba 50 "$TAURI_ICONS_DIR/StoreLogo.png" "$APP_CANVAS_PNG"

ffmpeg -y -i "$APP_CANVAS_PNG" -vf scale=256:256 -frames:v 1 "$TAURI_ICONS_DIR/icon.ico" >/dev/null 2>&1

resize_png 16 "$PUBLIC_DIR/favicon-16x16.png" "$FAVICON_CANVAS_PNG"
resize_png 32 "$PUBLIC_DIR/favicon-32x32.png" "$FAVICON_CANVAS_PNG"
resize_png 180 "$PUBLIC_DIR/apple-touch-icon.png" "$APP_CANVAS_PNG"
ffmpeg -y -i "$FAVICON_CANVAS_PNG" -vf scale=64:64 -frames:v 1 "$PUBLIC_DIR/favicon.ico" >/dev/null 2>&1

rm -rf "$ICONSET_DIR"
rm -f "$TMP_DIR/$(basename "$SOURCE_SVG").png" "$RAW_PNG" "$APP_CANVAS_PNG" "$FAVICON_CANVAS_PNG"

if [[ -f "$REFERENCE_PNG" ]]; then
  echo "Generated brand assets from $REFERENCE_PNG"
else
  echo "Generated brand assets from $SOURCE_SVG"
fi
