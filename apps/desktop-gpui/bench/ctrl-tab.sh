#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# The Ctrl+Tab bench: starts the fixture's throwaway host with 26 top-level threads and nothing else, opens
# desktop-gpui on the topmost, holds Ctrl+Tab down through the window's own key dispatch for one step a thread and one
# more, which wraps to the start, at this machine's key repeat, and prints each step's key-to-screen time from the
# trace. It stops only what it started, by pid, and never touches the live host: the app reads the fixture's state.
#   apps/desktop-gpui/bench/ctrl-tab.sh
# BENCH_MORE=<n> lists n threads instead of 26; BENCH_OUT=<dir> keeps the trace there (default: a new temp folder);
# BENCH_GPUI_APP=<path> runs that build instead of building one; BENCH_REPEAT=<delay ms>,<every ms> sets the key
# repeat, else a Mac's own from `defaults read -g` (InitialKeyRepeat and KeyRepeat, in 15 ms ticks) and elsewhere a
# Mac's fastest, 225,30. On Linux it runs under Xvfb with DISPLAY set and a window manager on that display (openbox will
# do), or the window never takes focus and the bench exits 3, as it does for a window covered or not key on a Mac and
# on a locked screen, which draws no window: it refuses to start on one and stops the app if the screen locks mid-run.
# It prints the app's memory through the walk from mem.sh.
set -eu
# A Mac whose display sleeps covers the window; caffeinate holds it awake for the whole run.
if [ "$(uname -s)" = Darwin ] && [ -z "${CTRL_TAB_AWAKE:-}" ]; then CTRL_TAB_AWAKE=1 exec caffeinate -dims "$0" "$@"; fi
bench=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lock.sh
. "$bench/lock.sh"
refuse_locked
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-ctrl-tab-XXXXXX")}
mkdir -p "$out"
MORE=${BENCH_MORE:-26}
gpui_app=${BENCH_GPUI_APP:-$repo/apps/desktop-gpui/target/release/desktop-gpui}
gpui_pid=""

watcher=""
finish() {
  code=$?
  if [ -n "$watcher" ]; then kill "$watcher" 2>/dev/null || true; fi
  if [ -n "$gpui_pid" ] && [ "$(ps -o ppid= -p "$gpui_pid" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$gpui_pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" || true; fi
  [ "$code" -eq 0 ] || echo "ctrl-tab: stopped (exit $code); what it wrote is in $out" >&2
}
trap finish EXIT
trap 'exit 130' INT TERM
trap 'exit 129' HUP

if [ -n "${BENCH_REPEAT:-}" ]; then
  repeat=$BENCH_REPEAT
elif [ "$(uname -s)" = Darwin ]; then
  delay=$(defaults read -g InitialKeyRepeat 2>/dev/null || echo 15)
  every=$(defaults read -g KeyRepeat 2>/dev/null || echo 2)
  repeat="$((delay * 15)),$((every * 15))"
else
  repeat=225,30
fi

if [ -z "${BENCH_GPUI_APP:-}" ]; then
  echo "== building desktop-gpui, release, 3 jobs"
  (cd "$repo/apps/desktop-gpui" && cargo build --release --locked -j 3 -p desktop-gpui)
fi

echo "== starting the fixture's host with $MORE threads"
"$bench/fixture.sh" "$out/fixture.json" "$MORE" bare
# The state, the newest thread, which the sidebar lists first, and how many top-level threads it lists.
read_out() {
  node -e 'const o = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    console.log([o.state, o.extra.at(-1), Object.keys(o.threads).length + (o.realHost.panesThread ? 1 : 0) + o.extra.length][process.argv[2]])' "$out/fixture.json" "$1"
}
state=$(read_out 0)
top=$(read_out 1)
listed=$(read_out 2)
home=$(dirname "$(dirname "$state")")
mkdir -p "$home/gpui-home"

echo "== holding ctrl-tab through $listed top-level threads from $top, key repeat $repeat ms"
# The same clean environment as the host's, a temp home and nothing of the person's; env execs the app, so the pid is its own.
env -i HOME="$home/gpui-home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} \
  "$gpui_app" --state "$state" --bench "$out/ctrl-tab.json" --bench-thread "$top" --bench-ctrl-tab $((listed + 1)) --bench-repeat "$repeat" \
  > "$out/ctrl-tab.log" 2>&1 &
gpui_pid=$!
watch_lock "$gpui_pid" "$out/locked" &
watcher=$!
# The app's memory through the walk, sampled twice a second: its first sample, its peak and its last, in MB.
: > "$out/mem.mb"
while [ "$(ps -o stat= -p "$gpui_pid" 2>/dev/null | cut -c1)" != Z ] && kill -0 "$gpui_pid" 2>/dev/null; do
  "$bench/mem.sh" "$gpui_pid" 2>/dev/null >> "$out/mem.mb" || break
  sleep 0.5
done
code=0
wait "$gpui_pid" || code=$?
gpui_pid=""
kill "$watcher" 2>/dev/null || true
watcher=""
[ ! -f "$out/locked" ] || { echo "the screen locked mid-run, so nothing the app drew was measured; keep it unlocked and run again" >&2; exit 3; }
[ $code -ne 3 ] || { echo "the bench window was covered or lost focus, so its frames are not the app's; keep the window in front and run again" >&2; tail -20 "$out/ctrl-tab.log" >&2; exit 3; }
[ $code -eq 0 ] || { echo "desktop-gpui --bench-ctrl-tab failed (exit $code):" >&2; tail -40 "$out/ctrl-tab.log" >&2; exit 1; }

judged=0
node "$bench/ctrl-tab.mjs" "$out/ctrl-tab.json" > "$out/ctrl-tab.txt" || judged=$?
cat "$out/ctrl-tab.txt"
awk 'NR == 1 { first = $1 } $1 > peak { peak = $1 } { last = $1 } END { if (NR) printf "memory: first %s MB, peak %s MB, last %s MB over %d samples\n", first, peak, last, NR }' "$out/mem.mb"
echo "trace: $out/ctrl-tab.json"
exit $judged
