#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# mem-base.sh <desktop-gpui binary> [runs]: the app at rest against the bench's fixture host, read with bench/mem.sh
# 10 s after launch as run-mac.sh reads it, once per run. The yardstick the spike's memory is set against.
set -u

[ $# -ge 1 ] || { echo "usage: mem-base.sh <desktop-gpui binary> [runs]" >&2; exit 2; }
app=$1
runs=${2:-3}
bench=$(cd "$(dirname "$0")/../bench" && pwd)
out=$(mktemp -d)
pid=
stop() {
  if [ -n "$pid" ] && [ "$(ps -o ppid= -p "$pid" | tr -d ' ')" = "$$" ]; then
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
  fi
  pid=
  [ -f "$out/fixture.json" ] && "$bench/stop.sh" "$out/fixture.json" > /dev/null
  rm -rf "$out"
}
trap stop EXIT
trap 'stop; exit 130' INT TERM HUP

"$bench/fixture.sh" "$out/fixture.json" > "$out/fixture.log" 2>&1
state=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$out/fixture.json")
home=$out/gpui-home
mkdir -p "$home"
for run in $(seq "$runs"); do
  env -i HOME="$home" PATH=/usr/bin:/bin LANG=en_US.UTF-8 DISPLAY="${DISPLAY:-:99}" "$app" --state "$state" > "$out/app.log" 2>&1 &
  pid=$!
  sleep 10
  pss=$(awk '/^Pss:/{print $2}' "/proc/$pid/smaps_rollup" 2>/dev/null)
  echo "run $run: $("$bench/mem.sh" "$pid" 2>/dev/null) MB; proportional $(awk -v kb="${pss:-0}" 'BEGIN { printf "%.1f", kb / 1024 }') MB" || true
  kill -0 "$pid" 2>/dev/null || { echo "the app exited early:"; tail -20 "$out/app.log"; }
  kill "$pid"
  wait "$pid" 2>/dev/null
  pid=
done
