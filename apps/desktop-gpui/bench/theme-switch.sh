#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# theme-switch.sh: every theme picked on Settings > Appearance, then transparency off and on, against the line that a
# switch is on screen within 100 ms, draws no frame over 50 ms and draws nothing more once it settles. A fresh copy of
# desktop-gpui on the fixture's throwaway host and a temp HOME, never the person's. Once the page has the host's
# preferences and has stood still a second, the app writes each switch through the page's own write, 600 ms apart, as
# a click on its cards does, and traces every frame it draws. Prints one line per switch: when its last frame within
# 300 ms reached the screen, its slowest frame, and the frames and renders still drawn after those 300 ms, which a
# loop of renders shows as; exits 1 on any switch over the line. Keep the window in front: GPUI caps a window without
# focus at 30 fps, so the run stops and says so, exit 3, as it does on a locked screen, which draws no window.
#   apps/desktop-gpui/bench/theme-switch.sh
# BENCH_GPUI_APP=<path> runs that build instead of building one; BENCH_OUT=<dir> keeps the results there. Off macOS,
# under a display and a window manager, a switch is judged by the app's own time (trace.mjs --switches --app), as the
# software renderer's present is not the app's.
set -eu
if [ "$(uname -s)" = Darwin ] && [ -z "${THEME_AWAKE:-}" ]; then THEME_AWAKE=1 exec caffeinate -dims "$0" "$@"; fi
bench=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lock.sh
. "$bench/lock.sh"
refuse_locked
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-theme-switch-XXXXXX")}
mkdir -p "$out"
app=${BENCH_GPUI_APP:-${CARGO_TARGET_DIR:-$repo/apps/desktop-gpui/target}/release/desktop-gpui}
pid=""
watcher=""
finish() {
  code=$?
  if [ -n "$watcher" ]; then kill "$watcher" 2>/dev/null || true; fi
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then kill "$pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" > /dev/null || true; fi
  [ $code -eq 0 ] || echo "theme-switch: exit $code; results in $out" >&2
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
home=$(dirname "$(dirname "$state")")/theme-home
mkdir -p "$home"

echo "== switching themes on Settings > Appearance: keep the window in front until it quits"
code=0
# shellcheck disable=SC2086
env -i HOME="$home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} \
  "$app" --state "$state" --bench "$out/trace.json" --bench-screen settings:appearance --bench-themes all \
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
judge=""
[ "$(uname -s)" = Darwin ] || judge=--app
# shellcheck disable=SC2086
node "$bench/trace.mjs" --switches $judge "$out/trace.json"
