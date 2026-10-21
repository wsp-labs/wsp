#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# activity-scroll.sh: Settings > Activity under 30 s of trackpad scroll while its numbers move, against wsp-map#2414's
# line: no frame late on screen after its input. A fresh copy of desktop-gpui on the fixture's throwaway host and a temp HOME, never the
# person's. The page lists every process the computer runs by CPU, its longest list, and the daemon's samples move its
# figures and reorder its rows throughout. bench/momentum.mjs writes 30 s of trackpad gestures and their momentum, which
# the app plays aimed at the list, timing every frame it draws: on a Mac it posts them to its own event queue from a
# thread of their own at each event's time, as a trackpad's arrive; elsewhere through the window's dispatch. Prints the trace's summary, each event's wait from when it was due to the present of the frame that showed
# it (and the part before the app had it), its five slowest frames with the views each drew, and exits 1 on any frame
# that showed an event more than 16 ms after it was due, 2 when no event moved the list. Keep the window in front:
# GPUI caps a window without focus at 30 fps, so the run stops and says so, exit 3, as it does on a locked screen,
# which draws no window: it refuses to start on one and stops the app if the screen locks mid-run.
#   apps/desktop-gpui/bench/activity-scroll.sh
# BENCH_GPUI_APP=<path> runs that build instead of building one; BENCH_OUT=<dir> keeps the results there. Off macOS,
# under a display and a window manager, a frame is judged by the app's own time (trace.mjs --app), as the software
# renderer's present is not the app's.
set -eu
if [ "$(uname -s)" = Darwin ] && [ -z "${SCROLL_AWAKE:-}" ]; then SCROLL_AWAKE=1 exec caffeinate -dims "$0" "$@"; fi
bench=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lock.sh
. "$bench/lock.sh"
refuse_locked
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-activity-scroll-XXXXXX")}
mkdir -p "$out"
app=${BENCH_GPUI_APP:-$repo/apps/desktop-gpui/target/release/desktop-gpui}
pid=""
watcher=""
finish() {
  code=$?
  if [ -n "$watcher" ]; then kill "$watcher" 2>/dev/null || true; fi
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then kill "$pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" > /dev/null || true; fi
  [ $code -eq 0 ] || echo "activity-scroll: exit $code; results in $out" >&2
}
trap finish EXIT
trap 'exit 130' INT TERM HUP

if [ ! -f "$repo/packages/host/dist/bin.js" ] || [ ! -f "$repo/apps/web/dist/index.html" ]; then
  echo "== building the host and the web app the fixture serves"
  (cd "$repo" && pnpm install --frozen-lockfile && pnpm build)
fi
if [ -z "${BENCH_GPUI_APP:-}" ]; then
  echo "== building desktop-gpui, release"
  (cd "$repo/apps/desktop-gpui" && cargo build --release --locked -p desktop-gpui)
fi

echo "== starting the fixture's host"
"$bench/fixture.sh" "$out/fixture.json"
state=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$out/fixture.json")
home=$(dirname "$(dirname "$state")")/scroll-home
mkdir -p "$home"
node "$bench/momentum.mjs" --seconds 30 --out "$out/momentum.jsonl"

echo "== scrolling Settings > Activity for 30 s: keep the window in front until it quits"
code=0
# shellcheck disable=SC2086
env -i HOME="$home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} \
  "$app" --state "$state" --bench "$out/trace.json" --bench-screen settings:activity --bench-replay "$out/momentum.jsonl" \
  > "$out/app.log" 2>&1 < /dev/null &
pid=$!
watch_lock "$pid" "$out/locked" &
watcher=$!
wait "$pid" || code=$?
pid=""
kill "$watcher" 2>/dev/null || true
watcher=""
[ ! -f "$out/locked" ] || { echo "the screen locked mid-run, so nothing the app drew was measured; keep it unlocked and run again" >&2; exit 3; }
[ $code -ne 3 ] || { echo "the window was covered or lost focus, so its frame rate is not the app's; keep it in front and run again" >&2; exit 3; }
[ $code -eq 0 ] || { echo "desktop-gpui failed (exit $code):" >&2; tail -20 "$out/app.log" >&2; exit 1; }
judge=--latency
[ "$(uname -s)" = Darwin ] || judge=--app
# shellcheck disable=SC2086
node "$bench/trace.mjs" $judge "$out/trace.json"
