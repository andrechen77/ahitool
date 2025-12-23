#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <input.png> <output.icns>"
  exit 1
fi

SRC="$1"
OUT="$2"
ICONSET="$(mktemp -d)/AppIcon.iconset"

if [[ ! -f "$SRC" ]]; then
  echo "Error: input file not found: $SRC"
  exit 1
fi

mkdir -p "$ICONSET"

declare -A ICONS=(
  [16]="icon_16x16.png"
  [32]="icon_16x16@2x.png"
  [32_2]="icon_32x32.png"
  [64]="icon_32x32@2x.png"
  [128]="icon_128x128.png"
  [256]="icon_128x128@2x.png"
  [256_2]="icon_256x256.png"
  [512]="icon_256x256@2x.png"
  [512_2]="icon_512x512.png"
  [1024]="icon_512x512@2x.png"
)

echo "Generating iconset…"

sips -z 16 16     "$SRC" --out "$ICONSET/icon_16x16.png"         >/dev/null
sips -z 32 32     "$SRC" --out "$ICONSET/icon_16x16@2x.png"      >/dev/null
sips -z 32 32     "$SRC" --out "$ICONSET/icon_32x32.png"         >/dev/null
sips -z 64 64     "$SRC" --out "$ICONSET/icon_32x32@2x.png"      >/dev/null
sips -z 128 128   "$SRC" --out "$ICONSET/icon_128x128.png"       >/dev/null
sips -z 256 256   "$SRC" --out "$ICONSET/icon_128x128@2x.png"    >/dev/null
sips -z 256 256   "$SRC" --out "$ICONSET/icon_256x256.png"       >/dev/null
sips -z 512 512   "$SRC" --out "$ICONSET/icon_256x256@2x.png"    >/dev/null
sips -z 512 512   "$SRC" --out "$ICONSET/icon_512x512.png"       >/dev/null
sips -z 1024 1024 "$SRC" --out "$ICONSET/icon_512x512@2x.png"    >/dev/null

echo "Converting to icns…"
iconutil -c icns "$ICONSET" -o "$OUT"

echo "Done: $OUT"
