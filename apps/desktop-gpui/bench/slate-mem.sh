#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# slate-mem.sh <desktop-gpui> [runs]: the app's memory with the fixture's long thread open and the right panel on its
# launcher, against the same with every.slate written on that thread and its Slate tab open, on one build. One slate
# run first is thrown away, to warm the caches and mark the slate shown so the launcher runs do not open it; then the
# two alternate, `runs` times each (3 by default), and the script prints each reading and the medians. It starts its
# own throwaway host with fixture.sh, gives the app that host's temp home and nothing else of the caller's, and stops
# only the pids it started.
#   apps/desktop-gpui/bench/slate-mem.sh apps/desktop-gpui/target/release/desktop-gpui
# On Linux it needs DISPLAY on a display with a window manager (Xvfb and openbox). SLATE_MEM_OUT=<dir> keeps the logs
# and, where ImageMagick's import is installed, a shot of each run's window (default: a new temp folder).
set -eu
[ $# -ge 1 ] && [ $# -le 2 ] || { echo "usage: slate-mem.sh <desktop-gpui> [runs]" >&2; exit 2; }
case $1 in /*) app=$1 ;; *) app=$(pwd)/$1 ;; esac
[ -x "$app" ] || { echo "slate-mem: $app is not an executable" >&2; exit 2; }
runs=${2:-3}
bench=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$bench/../../.." && pwd)
out=${SLATE_MEM_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-slate-mem-XXXXXX")}
mkdir -p "$out"
# The time a launch is given to connect, open the thread and draw the panel before its memory is read, as run-mac.sh.
SETTLE=10

app_pid=""
finish() {
  code=$?
  if [ -n "$app_pid" ] && [ "$(ps -o ppid= -p "$app_pid" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$app_pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" || true; fi
  [ $code -eq 0 ] || echo "slate-mem: failed (exit $code); logs in $out" >&2
}
trap finish EXIT
trap 'exit 130' INT TERM
trap 'exit 129' HUP

echo "slate-mem: making the fixture's host"
"$bench/fixture.sh" "$out/fixture.json"
read_out() { node -e 'const o = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); console.log(process.argv[2] === "long" ? o.threads.long : o.state)' "$out/fixture.json" "$1"; }
state=$(read_out state)
long=$(read_out long)
home=$(dirname "$(dirname "$state")")
mkdir -p "$home/gpui-home"
node=$(command -v node)

env -i HOME="$home" PATH="$(dirname "$node"):/usr/bin:/bin" LANG=en_US.UTF-8 WSP_UPDATE_CHECK=0 \
  "$node" "$repo/packages/host/dist/bin.js" slate write "$long" "$repo/apps/desktop-gpui/parity/wave-2/data/every.slate" --state "$state" >"$out/write.log" 2>&1 ||
  { echo "slate-mem: writing every.slate on the long thread failed:" >&2; cat "$out/write.log" >&2; exit 1; }

# One launch on `--pane <pane>`, in this shell so the traps reach it; its memory in MB lands in $out/<name>.mb.
measure() {
  env -i HOME="$home/gpui-home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} \
    "$app" --state "$state" --open "$long" --pane "$1" >"$out/$2.log" 2>&1 &
  app_pid=$!
  sleep $SETTLE
  kill -0 "$app_pid" 2>/dev/null || { echo "desktop-gpui exited early:" >&2; cat "$out/$2.log" >&2; exit 1; }
  if [ -n "${DISPLAY:-}" ] && command -v import >/dev/null 2>&1; then import -window root "$out/$2.png" 2>/dev/null || true; fi
  "$bench/mem.sh" "$app_pid" 2>>"$out/$2.log" >"$out/$2.mb"
  kill "$app_pid"
  wait "$app_pid" || true
  app_pid=""
}

median() { tr ' ' '\n' | grep . | sort -n | awk '{ v[NR] = $1 } END { print (NR % 2) ? v[(NR + 1) / 2] : (v[NR / 2] + v[NR / 2 + 1]) / 2 }'; }

measure slate warm
echo "slate-mem: warm-up, slate, thrown away: $(cat "$out/warm.mb") MB"
launcher=""
slate=""
i=1
while [ "$i" -le "$runs" ]; do
  measure launcher "launcher-$i"
  measure slate "slate-$i"
  l=$(cat "$out/launcher-$i.mb")
  s=$(cat "$out/slate-$i.mb")
  echo "slate-mem: run $i: launcher $l MB, slate $s MB"
  launcher="$launcher $l"
  slate="$slate $s"
  i=$((i + 1))
done
ml=$(echo "$launcher" | median)
ms=$(echo "$slate" | median)
echo "launcher:$launcher MB, median $ml"
echo "every.slate:$slate MB, median $ms"
awk -v l="$ml" -v s="$ms" 'BEGIN { printf "every.slate over the launcher: %+.1f MB (%+.1f%%)\n", s - l, (s - l) * 100 / l }'
case $(uname -s) in Darwin) echo "memory: physical footprint, MB of 2^20 bytes" ;; *) echo "memory: resident size (no footprint off macOS), MB of 2^20 bytes" ;; esac
echo "logs: $out"
