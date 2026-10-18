#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# diff.sh <web-dir> <gpui-dir> <out-dir>: the share of pixels that differ between each web-shots.mjs shot and its
# gpui-shots.mjs pair (ImageMagick compare, a pixel differs past a 6% colour distance), a diff image per pair, and the
# drawer's own share where the shot holds more than the drawer. Of the GPUI shot's blink phases the closer one counts.
# Writes <out-dir>/table.md.
set -eu
[ $# -eq 3 ] || { echo "usage: diff.sh <web-dir> <gpui-dir> <out-dir>" >&2; exit 2; }
web=$1 gpui=$2 out=$3
mkdir -p "$out"
fuzz=6%
# The drawer in device px of the 1280x800 window at 2x; 023's window is 640 wide.
drawer=2048x560+512+1040
drawer_023=1280x560+0+1040

# Prints the share in percent of pixels of $1 and $2 (cropped to $4 when given) that differ, the diff into $3.
share() {
  a=$1 b=$2 d=$3 box=${4:-}
  if [ -n "$box" ]; then
    convert "$a" -crop "$box" +repage "$out/.a.png"
    convert "$b" -crop "$box" +repage "$out/.b.png"
    a=$out/.a.png b=$out/.b.png
  fi
  n=$(compare -metric AE -fuzz "$fuzz" "$a" "$b" "$d" 2>&1 >/dev/null || true)
  wh=$(identify -format '%w %h' "$a")
  echo "$n $wh" | awk '{ printf "%.2f", 100 * $1 / ($2 * $3) }'
}

{
  echo "| State | Theme | Shot | Differing, shot | Differing, drawer | Phase |"
  echo "|---|---|---|---|---|---|"
  for w in "$web"/W2-TRM-*-paper.png "$web"/W2-TRM-*-graphite.png; do
    name=$(basename "$w" .png)
    case $name in *.window) continue ;; esac
    g=$gpui/$name.png
    [ -f "$g" ] || { echo "| ${name%-*} | ${name##*-} | missing | | | |"; continue; }
    best=$(share "$w" "$g" "$out/$name.png")
    phase=main
    for n in alt alt2; do
      [ -f "$gpui/$name.$n.png" ] || continue
      other=$(share "$w" "$gpui/$name.$n.png" "$out/$name.$n.png")
      if awk -v a="$other" -v m="$best" 'BEGIN { exit !(a < m) }'; then best=$other phase=$n; mv "$out/$name.$n.png" "$out/$name.png"; else rm -f "$out/$name.$n.png"; fi
    done
    shot=$(identify -format '%wx%h' "$w")
    id=${name%-*}
    gp=$g
    [ $phase = main ] || gp=$gpui/$name.$phase.png
    case $id in
      W2-TRM-020) dr=$(share "$w" "$gp" "$out/$name.drawer.png" "$drawer") ;;
      W2-TRM-023) dr=$(share "$w" "$gp" "$out/$name.drawer.png" "$drawer_023") ;;
      W2-TRM-021) dr="(panel)" ;;
      *) dr=$best ;;
    esac
    echo "| $id | ${name##*-} | $shot | $best% | $dr% | $phase |"
  done
} > "$out/table.md"
rm -f "$out/.a.png" "$out/.b.png"
cat "$out/table.md"
