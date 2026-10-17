#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# diff.sh <web-dir> <gpui-dir> <out-dir>: for each <theme>/<state>.content.png and <theme>/<state>.full.png present in
# both shot folders, writes <out>/<theme>/<name>.diff.png (differing pixels in red) and <name>.pair.png (web left,
# gpui right), and prints the share of differing pixels at fuzz 0%, at 3% (past the web's 3.5% surface grain) and at 10%. A pair of two sizes is compared over the
# top-left area both cover, so a taller full page is cut to the shorter one.
set -eu
[ $# -eq 3 ] || { echo "usage: diff.sh <web-dir> <gpui-dir> <out-dir>" >&2; exit 2; }
web=$1
gpui=$2
out=$3
if command -v magick >/dev/null 2>&1; then
  im() { magick "$@"; }
  cmp() { magick compare "$@"; }
  ident() { magick identify "$@"; }
else
  im() { convert "$@"; }
  cmp() { compare "$@"; }
  ident() { identify "$@"; }
fi
tmp=$(mktemp -d "${TMPDIR:-/tmp}/usage-diff-XXXXXX")
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM

# compare exits 1 when the images differ, which is an answer here, and 2 when it could not compare them.
differing() {
  said=$(cmp -metric AE -fuzz "$3" "$1" "$2" null: 2>&1) || [ $? -eq 1 ] || { echo "diff: compare failed: $said" >&2; exit 1; }
  echo "$said" | awk '{ printf "%d", $1 + 0 }'
}

printf '%-9s %-30s %-11s %9s %9s %9s\n' theme state size "fuzz 0%" "fuzz 3%" "fuzz 10%"
pairs=0
for shot in "$web"/*/*.content.png "$web"/*/*.full.png; do
  [ -f "$shot" ] || continue
  rel=${shot#"$web"/}
  [ -f "$gpui/$rel" ] || continue
  theme=${rel%%/*}
  file=${rel#*/}
  case $file in
    *.content.png) name=${file%.content.png} ;;
    *) name=${file%.png} ;;
  esac
  sizes=$(ident -format '%w %h ' "$shot" "$gpui/$rel")
  read -r w1 h1 w2 h2 <<EOF
$sizes
EOF
  w=$((w1 < w2 ? w1 : w2))
  h=$((h1 < h2 ? h1 : h2))
  im "$shot" -crop "${w}x${h}+0+0" +repage "$tmp/web.png"
  im "$gpui/$rel" -crop "${w}x${h}+0+0" +repage "$tmp/gpui.png"
  mkdir -p "$out/$theme"
  cmp -fuzz 3% -highlight-color red "$tmp/web.png" "$tmp/gpui.png" "$out/$theme/$name.diff.png" 2>/dev/null || [ $? -eq 1 ]
  im "$tmp/web.png" "$tmp/gpui.png" +append "$out/$theme/$name.pair.png"
  pixels=$((w * h))
  exact=$(differing "$tmp/web.png" "$tmp/gpui.png" 0%)
  grain=$(differing "$tmp/web.png" "$tmp/gpui.png" 3%)
  loose=$(differing "$tmp/web.png" "$tmp/gpui.png" 10%)
  share() { echo "$1 $pixels" | awk '{ print 100 * $1 / $2 }'; }
  printf '%-9s %-30s %-11s %8.2f%% %8.2f%% %8.2f%%\n' "$theme" "$name" "${w}x${h}" "$(share "$exact")" "$(share "$grain")" "$(share "$loose")"
  pairs=$((pairs + 1))
done
[ "$pairs" -gt 0 ] || { echo "diff: no <theme>/<state>.content.png or .full.png is in both $web and $gpui" >&2; exit 1; }
