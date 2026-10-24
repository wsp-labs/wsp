#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# gpui.sh [--out <dir>] [--web <dir>] [id ...]: our app in each state host.mjs lists, on a host bench/fixture.sh
# starts inside the run's namespace. Each state's data() runs once through prep.mjs; then per theme the host's theme
# preference is set and desktop-gpui opens on the state's thread and pane, in the capture state WSP_GPUI_SHOT names
# where the state has one, under its own Xvfb and openbox at 1280x800 css px and scale 2, the pointer parked off the
# window before the keys and again before the shot. Writes <out>/<id>-<theme>.window.png and <id>-<theme>.png, cut to the clip today's shot recorded in
# <web>/index.json (web.sh), and prints what each state lacks on our side. Every child runs under env -i with only the
# names it needs, so no host the caller's environment names is ever dialled. SHOTS is the shots folder (default
# /root/shots/2397; out and web are its gpui and web), BIN the app (default debug/desktop-gpui under CARGO_TARGET_DIR,
# else target), DISPLAY_NUM the display (default 97).
set -eu
# Today's app asks avatars.githubusercontent.com for every face the host's page names no picture for, so the run goes
# into a network namespace holding only loopback: nothing it starts can reach GitHub or any other machine.
# /proc/net/dev lists this process's own namespace; a run that cannot read it, or sees anything but lo, enters one,
# so a machine without ip never leaves the run online.
if ! awk -F: 'NR > 2 { gsub(/ /, "", $1); n++; if ($1 != "lo") other = 1 } END { exit !(n > 0 && !other) }' /proc/net/dev 2>/dev/null; then
  exec unshare --net -- sh -c 'ip link set lo up && exec sh "$@"' offline "$0" "$@"
fi
echo "gpui: network interfaces: $(ip -o link | awk -F': ' '{ print $2 }' | tr '\n' ' ')"
here=$(cd "$(dirname "$0")" && pwd)
desktop=$(cd "$here/../../../.." && pwd)
shots=${SHOTS:-/root/shots/2397}
out=$shots/gpui
web=$shots/web
while [ $# -gt 0 ]; do
  case $1 in
    --out) out=$2; shift 2 ;;
    --web) web=$2; shift 2 ;;
    --*) echo "usage: gpui.sh [--out <dir>] [--web <dir>] [id ...]" >&2; exit 2 ;;
    *) break ;;
  esac
done
bin=${BIN:-${CARGO_TARGET_DIR:-$desktop/target}/debug/desktop-gpui}
[ -x "$bin" ] || { echo "gpui: no app at $bin; cargo build -p desktop-gpui" >&2; exit 1; }
node=$(command -v node)
num=${DISPLAY_NUM:-97}
tmp=$(mktemp -d "${TMPDIR:-/tmp}/pr-shots-gpui-XXXXXX")
clean() { env -i HOME="$tmp" PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" LANG=en_US.UTF-8 "$@"; }
onx() { clean DISPLAY=":$num" "$@"; }
ids=$*
[ -n "$ids" ] || ids=$(cd "$here" && clean "$node" -e 'import("./host.mjs").then(m => console.log(m.IDS.join(" ")))')
fixture=$tmp/fixture.json
xpid='' wpid='' apid=''
cleanup() {
  [ -n "$apid" ] && kill "$apid" 2>/dev/null || true
  [ -n "$wpid" ] && kill "$wpid" 2>/dev/null || true
  [ -n "$xpid" ] && kill "$xpid" 2>/dev/null || true
  [ -f "$fixture" ] && clean sh "$desktop/bench/stop.sh" "$fixture" || true
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

echo "gpui: starting a throwaway host (about 140 s)"
clean sh "$desktop/bench/fixture.sh" "$fixture"
state=$(clean "$node" -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$fixture")

# Device pixels: 2560x1600 for the window at scale 2, with room for openbox's frame.
# Backgrounded children call env -i themselves, so $! is the pid of the program and not of a subshell around it.
env -i PATH=/usr/bin:/bin Xvfb ":$num" -screen 0 3000x2000x24 -nolisten tcp >"$tmp/xvfb.log" 2>&1 &
xpid=$!
sleep 1
kill -0 "$xpid" 2>/dev/null || { echo "gpui: Xvfb :$num did not start: $(cat "$tmp/xvfb.log")" >&2; exit 1; }
env -i HOME="$tmp" PATH=/usr/bin:/bin DISPLAY=":$num" openbox >"$tmp/openbox.log" 2>&1 &
wpid=$!
sleep 1

# The screen's far corner, outside the window and its frame, so the app sees the pointer leave and draws no hover.
park() { onx xdotool mousemove 2999 1999; }

field() { clean "$node" -e 'const v = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))[process.argv[2]]; console.log(Array.isArray(v) ? v.join("\n") : v ?? "")' "$1" "$2"; }

shoot() { # id theme prep.json
  id=$1 theme=$2 prep=$3
  shot=$out/$id-$theme
  mkdir -p "$out" "$tmp/home-$theme-$id"
  thread=$(field "$prep" thread)
  screen=$(field "$prep" screen)
  capture=$(field "$prep" shot)
  set -- --state "$state" --open "$thread"
  [ -n "$screen" ] && set -- "$@" --screen "$screen"
  env -i HOME="$tmp/home-$theme-$id" PATH=/usr/local/bin:/usr/bin:/bin LANG=en_US.UTF-8 DISPLAY=":$num" GPUI_X11_SCALE_FACTOR=2 \
    ${capture:+"WSP_GPUI_SHOT=$capture"} "$bin" "$@" >"$tmp/app-$theme-$id.log" 2>&1 &
  apid=$!
  wid=
  i=0
  while [ -z "$wid" ] && [ $i -lt 60 ]; do
    sleep 0.5
    kill -0 "$apid" 2>/dev/null || break
    wid=$(onx xdotool search --onlyvisible --pid "$apid" 2>/dev/null | head -1 || true)
    i=$((i + 1))
  done
  if [ -z "$wid" ]; then
    echo "gpui: $theme/$id: no window; the app said: $(tail -5 "$tmp/app-$theme-$id.log")"
    kill "$apid" 2>/dev/null || true
    apid=
    return 1
  fi
  onx xdotool windowactivate --sync "$wid" 2>/dev/null || true
  w=$(field "$prep" size | sed -n 1p)
  h=$(field "$prep" size | sed -n 2p)
  onx xdotool windowsize --sync "$wid" $((w * 2)) $((h * 2)) 2>/dev/null || true
  park
  sleep 4
  field "$prep" keys | while IFS= read -r key; do
    [ -n "$key" ] || continue
    case $key in
      type:*) onx xdotool type --delay 40 "${key#type:}" ;;
      *) onx xdotool key "$key" ;;
    esac
    sleep 0.8
  done
  park
  sleep "$(field "$prep" wait)"
  onx import -window "$wid" "$shot.window.png"
  kill "$apid" 2>/dev/null || true
  wait "$apid" 2>/dev/null || true
  apid=
  got=$(clean identify -format '%wx%h' "$shot.window.png")
  [ "$got" = "$((w * 2))x$((h * 2))" ] || echo "gpui: $theme/$id: the window is $got, not $((w * 2))x$((h * 2))"
  # shellcheck disable=SC2016
  box=$(clean "$node" -e '
    const [index, id, theme] = process.argv.slice(1);
    let rows = [];
    try { rows = JSON.parse(require("fs").readFileSync(index, "utf8")); } catch {}
    const row = rows.find(r => r.state === id && r.theme === theme && r.reached);
    const c = row?.clip;
    console.log(row === undefined ? "none" : c == null ? "" : `${c.width * 2}x${c.height * 2}+${c.x * 2}+${c.y * 2}`);
  ' "$web/index.json" "$id" "$theme")
  case $box in
    none)
      echo "gpui: $theme/$id: today's shot is not in $web/index.json, so ours is the whole window"
      cp "$shot.window.png" "$shot.png" ;;
    "") cp "$shot.window.png" "$shot.png" ;;
    *) clean convert "$shot.window.png" -crop "$box" +repage "$shot.png" ;;
  esac
  echo "gpui: $theme/$id shot (${screen:-thread only}${capture:+, WSP_GPUI_SHOT=$capture})"
}

for id in $ids; do
  echo "gpui: $id: data"
  (cd "$here" && clean "$node" prep.mjs data "$fixture" "$id") >"$tmp/$id.json"
  needs=$(field "$tmp/$id.json" needs)
  [ -n "$needs" ] && echo "gpui: $id: missing on our side: $needs"
  for theme in paper graphite; do
    (cd "$here" && clean "$node" prep.mjs theme "$fixture" "$theme")
    shoot "$id" "$theme" "$tmp/$id.json" || true
  done
  (cd "$here" && clean "$node" prep.mjs undo "$fixture" "$id" "$(field "$tmp/$id.json" thread)")
done
