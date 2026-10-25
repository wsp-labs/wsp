#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# web.sh [--out <dir>] [--theme paper|graphite] [glob]: today's Electron app in each W2-PRP state (or the glob), shot
# by parity/baseline.mjs at scale 2 in Paper and Graphite (or the one named) under an Xvfb and openbox of its own, on a
# host bench/fixture.sh starts inside the run's namespace. Each state's go() ends by parking the pointer off the window,
# so no hover tint lands in a shot. Writes <out>/<id>-<theme>.png and <out>/index.json with each clip in css px. Every
# child runs under env -i with only the names it needs. SHOTS is the shots folder (default /tmp/shots/pr-pane; out is its web), APP the Electron executable
# (default apps/desktop/dist/linux-unpacked/wsp), DISPLAY_NUM the display (default 96).
set -eu
# Today's app asks avatars.githubusercontent.com for every face the host's page names no picture for, so the run goes
# into a network namespace holding only loopback: nothing it starts can reach GitHub or any other machine.
# /proc/net/dev lists this process's own namespace; a run that cannot read it, or sees anything but lo, enters one,
# so a machine without ip never leaves the run online.
if ! awk -F: 'NR > 2 { gsub(/ /, "", $1); n++; if ($1 != "lo") other = 1 } END { exit !(n > 0 && !other) }' /proc/net/dev 2>/dev/null; then
  exec unshare --net -- sh -c 'ip link set lo up && exec sh "$@"' offline "$0" "$@"
fi
echo "web: network interfaces: $(ip -o link | awk -F': ' '{ print $2 }' | tr '\n' ' ')"
here=$(cd "$(dirname "$0")" && pwd)
desktop=$(cd "$here/../../../.." && pwd)
shots=${SHOTS:-/tmp/shots/pr-pane}
out=$shots/web
theme=
while [ $# -gt 0 ]; do
  case $1 in
    --out) out=$2; shift 2 ;;
    --theme) theme=$2; shift 2 ;;
    --*) echo "usage: web.sh [--out <dir>] [--theme paper|graphite] [glob]" >&2; exit 2 ;;
    *) break ;;
  esac
done
only=${1:-W2-PRP-*}
app=${APP:-$desktop/../desktop/dist/linux-unpacked/wsp}
[ -x "$app" ] || { echo "web: no Electron app at $app" >&2; exit 1; }
node=$(command -v node)
num=${DISPLAY_NUM:-96}
tmp=$(mktemp -d "${TMPDIR:-/tmp}/pr-shots-web-XXXXXX")
clean() { env -i HOME="$tmp" PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" LANG=en_US.UTF-8 "$@"; }
fixture=$tmp/fixture.json
xpid='' wpid=''
cleanup() {
  [ -n "$wpid" ] && kill "$wpid" 2>/dev/null || true
  [ -n "$xpid" ] && kill "$xpid" 2>/dev/null || true
  [ -f "$fixture" ] && clean sh "$desktop/bench/stop.sh" "$fixture" || true
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

echo "web: starting a throwaway host (about 140 s)"
clean sh "$desktop/bench/fixture.sh" "$fixture"

# baseline.mjs reads its states from wave-<n>/ beside itself, so a copy of it in the temp folder reads a wave-2/ that
# wraps the real wave-2/states.mjs with a park after each go(); bench/ beside it is the real one.
mkdir -p "$tmp/desktop/parity/wave-2"
cp "$desktop/parity/baseline.mjs" "$tmp/desktop/parity/"
ln -s "$desktop/parity/png.mjs" "$desktop/parity/claude-stub.mjs" "$tmp/desktop/parity/"
ln -s "$desktop/bench" "$tmp/desktop/bench"
cat >"$tmp/desktop/parity/wave-2/states.mjs" <<STATES
import states, * as wave from "$desktop/parity/wave-2/states.mjs";
export const setup = wave.setup;
export default states.map(s => (s.go === undefined ? s : { ...s, go: async (app, data) => (await s.go(app, data), await app.park()) }));
STATES

# 2560x1600 device pixels for the window at scale 2, with room for openbox's frame. Backgrounded children call env -i
# themselves, so $! is the pid of the program and not of a subshell around it.
env -i PATH=/usr/bin:/bin Xvfb ":$num" -screen 0 3000x2000x24 -nolisten tcp >"$tmp/xvfb.log" 2>&1 &
xpid=$!
sleep 1
kill -0 "$xpid" 2>/dev/null || { echo "web: Xvfb :$num did not start: $(cat "$tmp/xvfb.log")" >&2; exit 1; }
env -i HOME="$tmp" PATH=/usr/bin:/bin DISPLAY=":$num" openbox >"$tmp/openbox.log" 2>&1 &
wpid=$!
sleep 1
env -i PATH=/usr/bin:/bin DISPLAY=":$num" xdotool mousemove 2999 1999
mkdir -p "$tmp/home"
env -i HOME="$tmp/home" PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" LANG=en_US.UTF-8 DISPLAY=":$num" \
  "$node" "$tmp/desktop/parity/baseline.mjs" --wave 2 --fixture "$fixture" --app "$app" --out "$out" --only "$only" --scale 2 ${theme:+--theme "$theme"}
