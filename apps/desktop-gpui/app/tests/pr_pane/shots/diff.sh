#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# diff.sh [<web-dir> <gpui-dir> <out-dir>]: for each state and theme today's side reached (<web>/index.json, web.sh)
# and ours shot (gpui.sh; its whole window is cut again to today's clip, so the two runs may finish in any order),
# writes <out>/<theme>/<id>.png (differing pixels in red at fuzz 3%) and <id>.pair.png (today's, ours, that diff),
# and prints the share of differing pixels at fuzz 0%, 3% and 10% into <out>/table.txt.
# A pair of two sizes is padded to the larger, each side in a colour of its own, so what one side lacks counts.
# Defaults: the web, gpui and diff folders under SHOTS (default /root/shots/2397). Every child runs under env -i.
set -eu
shots=${SHOTS:-/root/shots/2397}
web=${1:-$shots/web}
gpui=${2:-$shots/gpui}
out=${3:-$shots/diff}
node=$(command -v node)
[ -f "$web/index.json" ] || { echo "diff: no $web/index.json; run web.sh first" >&2; exit 1; }
tmp=$(mktemp -d "${TMPDIR:-/tmp}/pr-shots-diff-XXXXXX")
clean() { env -i HOME="$tmp" PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" LANG=en_US.UTF-8 "$@"; }
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM

# compare exits 1 when the images differ, which is an answer here, and 2 when it could not compare them.
differing() {
  said=$(clean compare -metric AE -fuzz "$3" "$1" "$2" null: 2>&1) || [ $? -eq 1 ] || { echo "diff: compare failed: $said" >&2; exit 1; }
  echo "$said" | awk '{ printf "%d", $1 + 0 }'
}

# shellcheck disable=SC2016
clean "$node" -e '
  const rows = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  for (const r of rows.sort((a, b) => a.state.localeCompare(b.state) || b.theme.localeCompare(a.theme)))
    console.log([r.state, r.theme, r.reached ? r.file : "-", r.clip == null ? "whole" : `${r.clip.width * r.scale}x${r.clip.height * r.scale}+${r.clip.x * r.scale}+${r.clip.y * r.scale}`, r.reached ? "" : (r.reason ?? "").replace(/\s+/g, " ")].join("\t"));
' "$web/index.json" >"$tmp/rows"

mkdir -p "$out"
{
  printf '%-17s %-9s %-11s %9s %9s %9s\n' state theme size "fuzz 0%" "fuzz 3%" "fuzz 10%"
  tab=$(printf '\t')
  while IFS=$tab read -r id theme file box why; do
    if [ "$file" = "-" ]; then
      printf '%-17s %-9s today not reached: %s\n' "$id" "$theme" "$why"
      continue
    fi
    ours=$gpui/$id-$theme.png
    if [ -f "$gpui/$id-$theme.window.png" ]; then
      ours=$tmp/ours.png
      if [ "$box" = whole ]; then cp "$gpui/$id-$theme.window.png" "$ours"; else clean convert "$gpui/$id-$theme.window.png" -crop "$box" +repage "$ours"; fi
    fi
    if [ ! -f "$ours" ]; then
      printf '%-17s %-9s ours not shot\n' "$id" "$theme"
      continue
    fi
    sizes=$(clean identify -format '%w %h ' "$web/$file" "$ours")
    read -r w1 h1 w2 h2 <<EOF
$sizes
EOF
    w=$((w1 > w2 ? w1 : w2))
    h=$((h1 > h2 ? h1 : h2))
    clean convert "$web/$file" -background magenta -extent "${w}x${h}" "$tmp/web.png"
    clean convert "$ours" -background cyan -extent "${w}x${h}" "$tmp/gpui.png"
    mkdir -p "$out/$theme"
    clean compare -fuzz 3% -highlight-color red "$tmp/web.png" "$tmp/gpui.png" "$out/$theme/$id.png" 2>/dev/null || [ $? -eq 1 ]
    clean convert "$tmp/web.png" "$tmp/gpui.png" "$out/$theme/$id.png" +append "$out/$theme/$id.pair.png"
    pixels=$((w * h))
    share() { echo "$1 $pixels" | awk '{ printf "%.2f", 100 * $1 / $2 }'; }
    printf '%-17s %-9s %-11s %8s%% %8s%% %8s%%\n' "$id" "$theme" "${w}x${h}" \
      "$(share "$(differing "$tmp/web.png" "$tmp/gpui.png" 0%)")" \
      "$(share "$(differing "$tmp/web.png" "$tmp/gpui.png" 3%)")" \
      "$(share "$(differing "$tmp/web.png" "$tmp/gpui.png" 10%)")"
  done <"$tmp/rows"
} | tee "$out/table.txt"
