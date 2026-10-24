#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# usage: bench/scroll-mac.sh record <thread id> [<out dir>] | bench/scroll-mac.sh replay <thread id> <events.jsonl | momentum> [<out dir>]
# Scroll stalls on a real thread, on a Mac. `record` builds desktop-gpui (release), opens the thread at its end
# on the person's own host and records the trackpad's scroll events over it for 60 s from the first one, with the
# frame-time trace of those 60 s; the app quits by itself. `replay` opens the thread at its end again and plays a
# recording (or `momentum`, bench/momentum.mjs's synthetic trackpad) through the same window, writing the same trace.
# Each prints the frames over 50 ms with what the app did in them, and every run of scroll events that found room to
# scroll and left the thread where it stood (the wheel went to whatever was under the pointer), and exits 1 on either.
# The app opens the thread on the host in $WSP_HOME (default ~/.wsp), and these modes skip the read stamp opening a
# thread writes there; the app's own store goes to a new temp HOME, removed at the end. The host must be running. SCROLL_APP=<path> runs
# that build instead of building one; SCROLL_POINTER=<x>,<y> rests the pointer there, in the list's own pixels, for
# every replayed event. Keep the window in front: a replay whose window loses focus stops (GPUI caps a window without
# focus at 30 fps), and a recording's trace says so.
set -eu
usage() {
  sed -n '3p' "$0" | sed 's/^# //' >&2
  exit 2
}
[ $# -ge 2 ] || usage
mode=$1
thread=$2
events=""
out=""
case $mode in
  record) out=${3:-} ;;
  replay)
    [ $# -ge 3 ] || usage
    events=$3
    out=${4:-}
    ;;
  *) usage ;;
esac
bench=$(cd "$(dirname "$0")" && pwd)
crate=$(cd "$bench/.." && pwd)
[ -n "$out" ] || out=$(mktemp -d "${TMPDIR:-/tmp}/wsp-scroll-XXXXXX")
mkdir -p "$out"
out=$(cd "$out" && pwd)
wsp_home=${WSP_HOME:-$HOME/.wsp}
[ -f "$wsp_home/host.lock" ] || {
  echo "no running host in $wsp_home: start wsp first, or set WSP_HOME to the host's folder" >&2
  exit 1
}
if [ "$mode" = replay ] && [ "$events" != momentum ] && [ ! -f "$events" ]; then
  echo "no events file at $events" >&2
  exit 1
fi

pid=""
home=$(mktemp -d "${TMPDIR:-/tmp}/wsp-scroll-home-XXXXXX")
# shellcheck disable=SC2329
finish() {
  code=$?
  if [ -n "$pid" ] && [ "$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$pid"; fi
  rm -rf "$home"
  exit "$code"
}
trap finish EXIT
trap 'exit 130' INT TERM
trap 'exit 129' HUP

app=${SCROLL_APP:-}
if [ -z "$app" ]; then
  echo "== building desktop-gpui, release"
  (cd "$crate" && cargo build --release --locked -p desktop-gpui)
  target=${CARGO_TARGET_DIR:-target}
  case $target in
    /*) ;;
    *) target=$crate/$target ;;
  esac
  app=$target/release/desktop-gpui
fi
[ -x "$app" ] || {
  echo "no app at $app" >&2
  exit 1
}
echo "host: $wsp_home (read stamp skipped); app HOME: $home (temp, removed at the end); results: $out"

# The app starts as a plain command so the pid recorded is its own (env execs it), with nothing of the caller's
# environment but what it needs.
run() {
  env -i HOME="$home" WSP_HOME="$wsp_home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY="$DISPLAY"} "$app" "$@" &
  pid=$!
  code=0
  wait "$pid" || code=$?
  pid=""
  return 0
}

if [ "$mode" = record ]; then
  echo "== recording: scroll the thread with the trackpad as you would read it; 60 s after the first scroll the app quits"
  run --open "$thread" --record-scroll "$out/recorded.jsonl" --trace "$out/record-trace.json" > "$out/record.log" 2>&1
  [ $code -eq 0 ] && [ -f "$out/record-trace.json" ] || {
    echo "desktop-gpui stopped before the recording was written (exit $code):" >&2
    tail -20 "$out/record.log" >&2
    exit 1
  }
  echo "recorded $(wc -l < "$out/recorded.jsonl" | tr -d ' ') scroll events to $out/recorded.jsonl"
  echo "replay it with: $0 replay $thread $out/recorded.jsonl"
  trace=$out/record-trace.json
else
  if [ "$events" = momentum ]; then
    node "$bench/momentum.mjs" --out "$out/momentum.jsonl"
    events=$out/momentum.jsonl
  fi
  echo "== replaying $events: keep the window in front until it quits (about 65 s)"
  run --bench "$out/replay-trace.json" --bench-thread "$thread" --bench-replay "$events" ${SCROLL_POINTER:+--bench-pointer "$SCROLL_POINTER"} > "$out/replay.log" 2>&1
  [ $code -ne 3 ] || {
    echo "the window lost focus, so its frame rate is not the app's; keep it in front and run again" >&2
    exit 1
  }
  [ $code -eq 0 ] || {
    echo "desktop-gpui --bench-replay failed (exit $code):" >&2
    tail -20 "$out/replay.log" >&2
    exit 1
  }
  trace=$out/replay-trace.json
fi
echo "== the trace: $trace"
read_code=0
node "$bench/trace.mjs" "$trace" || read_code=$?
exit $read_code
