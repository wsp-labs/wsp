#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# usage: apps/desktop-gpui/bench/session.sh
# Memory across a session of 30 threads opened one after another, as a person moves down the sidebar: a throwaway host
# with long, real, short-1..3 and session-1..25 (fixture.sh with BENCH_SESSION=25), then `desktop-gpui --bench-session`,
# which reads itself 30 s after the threads are listed ("bench: session step 0/30", a clean launch), opens each thread,
# holds 2 s once its rows are drawn, says "bench: session step <i>/30", and holds 3 s more, in which this script reads
# the app's memory: on macOS the physical footprint (mem.sh, Activity Monitor's number), the graphics memory
# `footprint <pid>` names (IOAccelerator) and its whole report; elsewhere resident size. Each line also carries the
# app's own reading of its window's atlas, the GPU textures of glyphs and images. The app then opens the first 3
# threads again and exits 1 if one reads otherwise than it first did, and presses Ctrl+Tab 60 times, each read the same
# way after a 1 s glance ("bench: session walk <i>/60"). It stops only what it started, by pid, and never touches the
# live host: the app and the host each get a temp HOME and nothing else of the caller's environment.
# BENCH_OUT=<dir> keeps the results there (default: a new temp folder); BENCH_FIXTURE=<fixture.json> uses a host
# fixture.sh already started with BENCH_SESSION=25 and leaves it running; BENCH_GPUI_APP=<path> runs that build
# instead of building one; BENCH_HEAP=1 builds with the heap-profile feature, so the app also reads its live heap and
# writes the heap's biggest holders beside its result. The script runs on Linux too, under Xvfb with a window manager.
set -eu
bench=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-session-out-XXXXXX")}
mkdir -p "$out"
fixture=${BENCH_FIXTURE:-$out/fixture.json}
started_fixture=""
app_pid=""

finish() {
  code=$?
  if [ -n "$app_pid" ] && [ "$(ps -o ppid= -p "$app_pid" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$app_pid"; fi
  if [ -n "$started_fixture" ] && [ -f "$fixture" ]; then "$bench/stop.sh" "$fixture" || true; fi
  [ $code -eq 0 ] || echo "session: stopped (exit $code); results so far in $out" >&2
}
trap finish EXIT
trap 'exit 130' INT TERM
trap 'exit 129' HUP

gpui_app=${BENCH_GPUI_APP:-${CARGO_TARGET_DIR:-$repo/apps/desktop-gpui/target}/release/desktop-gpui}
if [ -z "${BENCH_GPUI_APP:-}" ]; then
  features=""
  [ "${BENCH_HEAP:-}" = 1 ] && features="--features heap-profile"
  echo "== building desktop-gpui, release $features"
  # shellcheck disable=SC2086
  (cd "$repo/apps/desktop-gpui" && cargo build --release --locked -p desktop-gpui $features)
fi
[ -x "$gpui_app" ] || { echo "no app at $gpui_app" >&2; exit 1; }

if [ -z "${BENCH_FIXTURE:-}" ]; then
  echo "== starting the fixture's host with 30 bench threads (about 20 minutes)"
  started_fixture=1
  BENCH_SESSION=25 "$bench/fixture.sh" "$fixture"
fi

# id<TAB>name for every thread the fixture made, and the session's order: the long ones among the medium ones.
# shellcheck disable=SC2016
node -e 'const t = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).threads; for (const [n, id] of Object.entries(t)) console.log(`${id}\t${n}`)' "$fixture" > "$out/names.tsv"
order="session-1 long session-2 session-3 short-1 session-4 session-5 real session-6 session-7 short-2 session-8 session-9 session-10 session-11 short-3 session-12 session-13 session-14 session-15 session-16 session-17 session-18 session-19 session-20 session-21 session-22 session-23 session-24 session-25"
list=""
for name in $order; do
  id=$(awk -F '\t' -v n="$name" '$2 == n { print $1 }' "$out/names.tsv")
  [ -n "$id" ] || { echo "the fixture has no thread $name: start it with BENCH_SESSION=25" >&2; exit 1; }
  list="$list${list:+,}$id"
done
first=${list%%,*}

state=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$fixture")
home=$(mktemp -d "${TMPDIR:-/tmp}/wsp-session-home-XXXXXX")
log="$out/app.log"
echo "== opening 30 threads one after another"
env -i HOME="$home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} \
  "$gpui_app" --state "$state" --bench "$out/session.json" --bench-thread "$first" --bench-session "$list" > "$log" 2>&1 &
app_pid=$!

darwin=""
[ "$(uname -s)" = Darwin ] && darwin=1
printf 'step\tthread\trows\tapp_rss_mb\tapp_heap_mb\tatlas_glyphs_mb\tatlas_images_mb\tmem_sh_mb\tfootprint\tgraphics\n' > "$out/session.tsv"
seen=0
began=$(date +%s)
while kill -0 "$app_pid" 2>/dev/null; do
  [ $(($(date +%s) - began)) -lt 2400 ] || { echo "desktop-gpui --bench-session ran past 40 minutes" >&2; exit 1; }
  said=$(grep -c "^bench: session \(step\|walk\) " "$log" || true)
  if [ "$said" -gt "$seen" ]; then
    seen=$said
    line=$(grep "^bench: session \(step\|walk\) " "$log" | sed -n "${seen}p")
    step=$(echo "$line" | awk '{ print ($3 == "walk" ? "w" : "") $4 }' | cut -d/ -f1)
    id=$(echo "$line" | awk '{ print $6 }')
    name=$(awk -F '\t' -v id="$id" '$1 == id { print $2 }' "$out/names.tsv")
    [ "$id" = rest ] && name=rest
    [ -n "$name" ] || name=$(echo "$id" | cut -c1-8)
    rows=$(echo "$line" | awk '{ print $8 }')
    rss=$(echo "$line" | sed -n 's/.* rss \([0-9.]*\) MB.*/\1/p')
    heap=$(echo "$line" | sed -n 's/.* heap \([0-9.]*\) MB.*/\1/p')
    glyphs=$(echo "$line" | sed -n 's/.* atlas \([0-9.]*\) MB glyphs.*/\1/p')
    images=$(echo "$line" | sed -n 's/.* glyphs \([0-9.]*\) MB images.*/\1/p')
    mem=$("$bench/mem.sh" "$app_pid" 2>/dev/null || echo -)
    fp=-
    gfx=-
    if [ -n "$darwin" ]; then
      footprint "$app_pid" > "$out/footprint-$step.txt" 2>&1 || true
      fp=$(sed -n 's/.*Footprint: *\([0-9.]* [KMG]*B\).*/\1/p' "$out/footprint-$step.txt" | head -1)
      gfx=$(awk '/IOAccelerator/ { print $1 " " $2; exit }' "$out/footprint-$step.txt")
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$step" "$name" "$rows" "${rss:--}" "${heap:--}" "${glyphs:--}" "${images:--}" "$mem" "${fp:--}" "${gfx:--}" | tee -a "$out/session.tsv"
  fi
  sleep 0.2
done
code=0
wait "$app_pid" || code=$?
app_pid=""
grep "^bench: session return" "$log" || true
[ $code -ne 3 ] || { echo "the app's window lost focus; keep it in front and run again" >&2; exit 1; }
[ $code -eq 0 ] || { echo "desktop-gpui --bench-session failed (exit $code):" >&2; tail -20 "$log" >&2; exit 1; }
[ "$seen" -eq 91 ] || { echo "read $seen readings, not the rest, 30 threads and 60 steps of the walk" >&2; exit 1; }
rm -rf "$home"
case $(uname -s) in Darwin) unit="mem_sh_mb: physical footprint; footprint: footprint(1)'s total" ;; *) unit="mem_sh_mb and app_rss_mb: resident size (no footprint off macOS)" ;; esac
echo "memory: MB of 2^20 bytes; $unit; app_heap_mb: live heap, heap-profile builds only"
awk -F '\t' 'NR > 1 && $8 != "-" { if ($1 ~ /^w/) { if ($8 > walk) walk = $8 } else if ($8 > max) max = $8 } END { printf "max mem_sh_mb %.1f over the threads, %.1f over the walk\n", max, walk }' "$out/session.tsv"
echo "results: $out (session.tsv, session.json, app.log${darwin:+, footprint-<step>.txt})"
