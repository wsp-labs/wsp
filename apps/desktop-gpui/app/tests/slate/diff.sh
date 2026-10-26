#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# diff.sh <baseline-dir> <gpui-dir> <out-dir>: pairs each parity/baseline.mjs shot (<id>-<theme>.png, its clip read from
# index.json) with slate_shots's <theme>/<state>.png by the table below, then hands both to ../usage/diff.sh, which
# writes <out>/<theme>/<id>-<state>.diff.png and .pair.png and prints the share of differing pixels at fuzz 0%, 3% and
# 10%. A panel state compares the panel's body: the right panel under its 52 px tab bar and inside its 1 px left
# border, 539 x 748 css px at x 741 of the 1280 px window; GPUI draws it 539 wide. A dialog state
# compares the baseline's clip, the dialog padded 16 px, with the same size cut from the middle of GPUI's panel, where
# its dialogs centre as the web's centre in the window.
set -eu
[ $# -eq 3 ] || { echo "usage: diff.sh <baseline-dir> <gpui-dir> <out-dir>" >&2; exit 2; }
base=$1
gpui=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
if command -v magick >/dev/null 2>&1; then
  im() { magick "$@"; }
else
  im() { convert "$@"; }
fi
tmp=$(mktemp -d "${TMPDIR:-/tmp}/slate-diff-XXXXXX")
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM

# GPUI state, the spec id whose baseline draws the same thing, and whether it compares the panel or a dialog.
MAP="every W2-SLT-045 panel
empty-data W2-SLT-046 panel
long W2-SLT-047 panel
many W2-SLT-048 panel
failed W2-SLT-049 panel
held W2-SLT-050 panel
variants W2-SLT-051 panel
runs W2-SLT-059 panel
approval W2-SLT-060 dialog
running W2-SLT-061 panel
done W2-SLT-062 panel
failed-run W2-SLT-063 panel
cancelled W2-SLT-064 panel
confirm W2-SLT-068 dialog
batch W2-SLT-069 dialog
held-row W2-SLT-070 panel
refreshing W2-SLT-072 panel
link W2-SLT-073 dialog
approvals W2-SLT-074 dialog
menu W2-SLT-075 panel
forget W2-SLT-076 dialog
no-slate W2-SLT-077 panel
cleared W2-SLT-078 panel"

# Each reached shot's state, theme, file, scale and clip in css px.
node -e '
const [dir] = process.argv.slice(1);
for (const e of JSON.parse(require("fs").readFileSync(dir + "/index.json", "utf8"))) {
  if (e.reached) console.log([e.state, e.theme, e.file, e.scale, e.clip.x, e.clip.y, e.clip.width, e.clip.height].join(" "));
}' "$base" > "$tmp/shots"

echo "$MAP" | while read -r state id kind; do
  grep "^$id " "$tmp/shots" | while read -r _ theme file s cx cy cw ch; do
    g="$gpui/$theme/$state.png"
    [ -f "$g" ] || continue
    mkdir -p "$tmp/web/$theme" "$tmp/gpui/$theme"
    if [ "$kind" = panel ]; then
      [ $((cx + cw)) -ge 1280 ] && [ "$cx" -le 741 ] && [ "$cy" -le 52 ] || continue
      im "$base/$file" -crop "$((539 * s))x$((748 * s))+$(((741 - cx) * s))+$(((52 - cy) * s))" +repage "$tmp/web/$theme/$id-$state.content.png"
      cp "$g" "$tmp/gpui/$theme/$id-$state.content.png"
    else
      cp "$base/$file" "$tmp/web/$theme/$id-$state.content.png"
      im "$g" -crop "$((cw * s))x$((ch * s))+$(((539 - cw) * s / 2))+$(((748 - ch) / 2 * s))" +repage "$tmp/gpui/$theme/$id-$state.content.png"
    fi
  done
done

sh "$here/../usage/diff.sh" "$tmp/web" "$tmp/gpui" "$out"
