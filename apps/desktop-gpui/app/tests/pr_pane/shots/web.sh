#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# web.sh [--fixture <out.json>] [--out <dir>] [--theme paper|graphite] [glob]: today's Electron app in each W2-PRP
# state (or the glob), shot by parity/baseline.mjs at scale 2 in Paper and Graphite (or the one named) under an Xvfb
# and openbox of its own, on a host bench/fixture.sh starts (or the one --fixture names, left running). Writes
# <out>/<id>-<theme>.png and <out>/index.json with each clip in css px. Every child runs under env -i with only the
# names it needs. SHOTS is the shots folder (default /root/shots/2397; out is its web), APP the Electron executable
# (default apps/desktop/dist/linux-unpacked/wsp), DISPLAY_NUM the display (default 96).
set -eu
# Today's app asks avatars.githubusercontent.com for every face the host's page names no picture for, so the run goes
# into a network namespace holding only loopback: nothing it starts can reach GitHub or any other machine.
if ip -o link | grep -qv ': lo:'; then
  exec unshare --net -- sh -c 'ip link set lo up && exec sh "$@"' offline "$0" "$@"
fi
echo "web: network interfaces: $(ip -o link | awk -F': ' '{ print $2 }' | tr '\n' ' ')"
here=$(cd "$(dirname "$0")" && pwd)
desktop=$(cd "$here/../../../.." && pwd)
shots=${SHOTS:-/root/shots/2397}
out=$shots/web
fixture=
theme=
while [ $# -gt 0 ]; do
  case $1 in
    --fixture) fixture=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    --theme) theme=$2; shift 2 ;;
    --*) echo "usage: web.sh [--fixture <out.json>] [--out <dir>] [--theme paper|graphite] [glob]" >&2; exit 2 ;;
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
xpid='' wpid='' own=''
cleanup() {
  [ -n "$wpid" ] && kill "$wpid" 2>/dev/null || true
  [ -n "$xpid" ] && kill "$xpid" 2>/dev/null || true
  [ -n "$own" ] && clean sh "$desktop/bench/stop.sh" "$own" || true
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

if [ -z "$fixture" ]; then
  own=$tmp/fixture.json
  fixture=$own
  echo "web: starting a throwaway host (about 140 s)"
  clean sh "$desktop/bench/fixture.sh" "$fixture"
fi

# 2560x1600 device pixels for the window at scale 2, with room for openbox's frame. Backgrounded children call env -i
# themselves, so $! is the pid of the program and not of a subshell around it.
env -i PATH=/usr/bin:/bin Xvfb ":$num" -screen 0 3000x2000x24 -nolisten tcp >"$tmp/xvfb.log" 2>&1 &
xpid=$!
sleep 1
kill -0 "$xpid" 2>/dev/null || { echo "web: Xvfb :$num did not start: $(cat "$tmp/xvfb.log")" >&2; exit 1; }
env -i HOME="$tmp" PATH=/usr/bin:/bin DISPLAY=":$num" openbox >"$tmp/openbox.log" 2>&1 &
wpid=$!
sleep 1
mkdir -p "$tmp/home"
env -i HOME="$tmp/home" PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" LANG=en_US.UTF-8 DISPLAY=":$num" \
  "$node" "$desktop/parity/baseline.mjs" --wave 2 --fixture "$fixture" --app "$app" --out "$out" --only "$only" --scale 2 ${theme:+--theme "$theme"}
