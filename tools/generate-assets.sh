#!/bin/sh
# Generate Grimmoire's image assets.
#
# Every PNG in assets/ is produced by this script: the artwork is original,
# generated procedurally from gradients and shapes, so the repository contains
# no third-party imagery and there are no usage rights to track.
#
# Requires ImageMagick 7 (`magick`). Run from the repository root:
#
#     tools/generate-assets.sh
#
# The output is deterministic, so regenerating produces identical files.
#
# Structure: each scene is built as a background, a silhouette layer with holes
# punched in it, and then a foreground. The silhouette is drawn on a
# transparent canvas and composited, because filling a path with holes onto the
# background itself would make the holes transparent instead of revealing what
# is behind them.

set -eu

out="assets"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"

# ---------------------------------------------------------------- chapel plate
# A ruined wall at dusk: the sky behind, two arched windows punched through it.
magick -size 600x200 gradient:'#120c22-#6f4419' \
  \( -size 600x200 radial-gradient:'#ffcf82-#000000' -resize 190x190 -blur 0x20 \
     -evaluate multiply 0.45 -background '#000000' -gravity north -extent 600x200 \) \
  -compose screen -composite "$work/sky.png"

magick -size 600x200 xc:none \
  -draw "fill-rule evenodd fill '#0b0811' \
         path 'M 0,200 L 0,58 L 600,58 L 600,200 Z \
               M 62,164 L 62,96 A 34,34 0 0 1 130,96 L 130,164 Z \
               M 470,164 L 470,96 A 34,34 0 0 1 538,96 L 538,164 Z'" \
  -draw "stroke '#6b4a2a' stroke-width 2 fill none \
         path 'M 62,164 L 62,96 A 34,34 0 0 1 130,96 L 130,164'" \
  -draw "stroke '#6b4a2a' stroke-width 2 fill none \
         path 'M 470,164 L 470,96 A 34,34 0 0 1 538,96 L 538,164'" \
  -draw "stroke '#3c2a1a' stroke-width 1 fill none line 0,120 600,120" \
  "$work/wall.png"

magick "$work/sky.png" "$work/wall.png" -compose over -composite \
  -draw "fill '#07050c' rectangle 0,184 600,200" \
  -attenuate 0.4 +noise Gaussian -blur 0x0.5 \
  -vignette 0x28 -strip -colors 128 -define png:compression-level=9 \
  "$out/chapel.png"

# ---------------------------------------------------------------- cellar plate
# A flooded crypt: a distant arch, cold depths, and still water below.
magick -size 600x200 gradient:'#02040a-#0a222b' \
  \( -size 600x200 radial-gradient:'#5fc6d0-#000000' -resize 170x170 -blur 0x18 \
     -evaluate multiply 0.40 -background '#000000' -gravity south -extent 600x200 \) \
  -compose screen -composite "$work/cave.png"

magick -size 600x200 xc:none \
  -draw "fill-rule evenodd fill '#01030a' \
         path 'M 0,200 L 0,104 L 600,104 L 600,200 Z \
               M 46,104 L 46,58 A 30,30 0 0 1 106,58 L 106,104 Z \
               M 494,104 L 494,58 A 30,30 0 0 1 554,58 L 554,104 Z \
               M 264,104 L 264,74 A 36,36 0 0 1 336,74 L 336,104 Z'" \
  -draw "stroke '#1c4e58' stroke-width 2 fill none \
         path 'M 46,104 L 46,58 A 30,30 0 0 1 106,58 L 106,104'" \
  -draw "stroke '#1c4e58' stroke-width 2 fill none \
         path 'M 494,104 L 494,58 A 30,30 0 0 1 554,58 L 554,104'" \
  -draw "stroke '#2b6b76' stroke-width 2 fill none \
         path 'M 264,104 L 264,74 A 36,36 0 0 1 336,74 L 336,104'" \
  -draw "fill '#01030a' polygon 96,0 126,0 111,44" \
  -draw "fill '#01030a' polygon 340,0 366,0 353,34" \
  -draw "fill '#01030a' polygon 512,0 540,0 526,40" \
  "$work/rock.png"

magick "$work/cave.png" "$work/rock.png" -compose over -composite \
  -draw "fill '#0b2c36' rectangle 0,140 600,200" \
  -draw "fill '#154855' rectangle 0,140 600,146" \
  -draw "fill '#08202a' rectangle 0,152 600,200" \
  -draw "stroke '#3f97a4' stroke-width 1 fill none line 30,149 170,149" \
  -draw "stroke '#3f97a4' stroke-width 1 fill none line 250,156 430,156" \
  -draw "stroke '#3f97a4' stroke-width 1 fill none line 460,148 580,148" \
  -attenuate 0.3 +noise Gaussian -blur 0x0.5 \
  -vignette 0x30 -strip -colors 128 -define png:compression-level=9 \
  "$out/cellar.png"

# ------------------------------------------------------------------- portrait
# The hooded figure, seen close: a warm glow behind a near-black silhouette.
magick -size 320x300 xc:'#05040a' \
  \( -size 320x300 radial-gradient:'#8a5a2c-#000000' -blur 0x26 -evaluate multiply 0.85 \) \
  -compose screen -composite \
  -draw "fill '#07050e' ellipse 160,120 58,66 0,360" \
  -draw "fill '#07050e' polygon 44,300 276,300 224,168 96,168" \
  -draw "stroke '#c08a44' stroke-width 5 fill none arc 100,118 220,188 194,346" \
  -draw "stroke '#f2c982' stroke-width 2 fill none arc 108,124 212,182 200,340" \
  -draw "fill '#2b1a09' ellipse 140,116 7,5 0,360" \
  -draw "fill '#2b1a09' ellipse 180,116 7,5 0,360" \
  -draw "fill '#0a0712' rectangle 0,288 320,300" \
  -attenuate 0.30 +noise Gaussian -blur 0x0.5 \
  -strip -colors 96 -define png:compression-level=9 \
  "$out/portrait.png"

printf 'wrote %s\n' "$out"/*.png
