#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# compare.sh <dir>: for each <state>-<theme>-web.png with a <state>-<theme>-gpui.png beside it, writes
# <state>-<theme>-diff.png (ImageMagick compare, pixels apart by more than FUZZ, 2% by default, counted as differing)
# and <state>-<theme>-pair.png (web left, gpui right), and prints one line per state: state, theme, differing pixels,
# their share. A gpui shot of another size is cropped or padded to the web shot's at the top left, and the line says so.
# States marked compare: false in states.mjs are left out. A second share, "blurred", compares both shots after a blur of
# 2 CSS px (2 px for each device px the 800 px high window is drawn at) at a 6% fuzz: what stays apart there is layout,
# color or a missing piece, not glyph rasterizing or a 1 CSS px text baseline.
set -eu
[ $# -eq 1 ] || { echo "usage: compare.sh <dir>" >&2; exit 2; }
dir=$1
fuzz=${FUZZ:-2%}
here=$(cd "$(dirname "$0")" && pwd)
skip=" $(node -e 'import(process.argv[1]).then(m => console.log(m.STATES.filter(s => s.compare === false).map(s => s.name).join(" ")))' "$here/states.mjs") "
tmp=$(mktemp -d "${TMPDIR:-/tmp}/settings-compare-XXXXXX")
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM
printf '%-36s %-9s %10s %8s %8s  %s\n' state theme pixels share blurred "(fuzz $fuzz)"
found=0
for web in "$dir"/*-web.png; do
  [ -e "$web" ] || continue
  base=${web%-web.png}
  gpui=$base-gpui.png
  [ -e "$gpui" ] || continue
  theme=${base##*-}
  state=$(basename "${base%-*}")
  case $skip in *" $state "*) continue ;; esac
  found=$((found + 1))
  size=$(identify -format '%wx%h' "$web")
  other=$(identify -format '%wx%h' "$gpui")
  note=""
  if [ "$size" != "$other" ]; then
    note="gpui is $other, web $size"
    convert "$gpui" -background magenta -gravity northwest -extent "$size" "$tmp/gpui.png"
    gpui=$tmp/gpui.png
  fi
  # compare exits 1 when the images differ, which is the point here; it prints the count on stderr.
  pixels=$(compare -metric AE -fuzz "$fuzz" "$web" "$gpui" "$base-diff.png" 2>&1 >/dev/null || true)
  pixels=${pixels%% *}
  convert "$web" "$gpui" +append "$base-pair.png"
  share=$(awk -v p="$pixels" -v s="$size" 'BEGIN { split(s, d, "x"); printf "%.2f%%", 100 * p / (d[1] * d[2]) }')
  scale=$(( ${size#*x} / 800 ))
  [ "$scale" -ge 1 ] || scale=1
  convert "$web" -blur "0x$((2 * scale))" "$tmp/web-blur.png"
  convert "$gpui" -blur "0x$((2 * scale))" "$tmp/gpui-blur.png"
  soft=$(compare -metric AE -fuzz 6% "$tmp/web-blur.png" "$tmp/gpui-blur.png" null: 2>&1 || true)
  soft=$(awk -v p="${soft%% *}" -v s="$size" 'BEGIN { split(s, d, "x"); printf "%.2f%%", 100 * p / (d[1] * d[2]) }')
  printf '%-36s %-9s %10s %8s %8s  %s\n' "$state" "$theme" "$pixels" "$share" "$soft" "$note"
done
[ $found -gt 0 ] || echo "no <state>-<theme>-web.png in $dir has a -gpui.png beside it"
