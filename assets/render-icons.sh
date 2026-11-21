#!/bin/sh
# Regenerate the raster icons from icon.svg. Needs ImageMagick (with librsvg)
# and, for the .icns, macOS iconutil. The outputs are committed, so this only
# needs re-running when icon.svg changes.
set -eu
cd "$(dirname "$0")"
MAGICK=${MAGICK:-magick}
render() { $MAGICK -background none -density 384 icon.svg -resize "$1x$1" "PNG32:$2"; }

render 256 icon-256.png
render 512 icon-512.png

# Windows: multi-size .ico
tmp=$(mktemp -d)
for s in 16 24 32 48 64 128 256; do render "$s" "$tmp/$s.png"; done
$MAGICK "$tmp"/16.png "$tmp"/24.png "$tmp"/32.png "$tmp"/48.png "$tmp"/64.png \
  "$tmp"/128.png "$tmp"/256.png fluxshot.ico

# macOS: .icns
set_dir="$tmp/fluxshot.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
  render "$s" "$set_dir/icon_${s}x${s}.png"
  render $((s * 2)) "$set_dir/icon_${s}x${s}@2x.png"
done
iconutil -c icns -o fluxshot.icns "$set_dir"
rm -rf "$tmp"
