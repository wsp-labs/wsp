#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# rest.sh: the app at rest on each page, against the acceptance of wsp-map#2414. One fresh copy of desktop-gpui per page,
# on the fixture's throwaway host and a temp HOME, never the person's: settle 10 s untouched, then 60 s untouched while
# `top -l 31 -s 2 -pid <pid>` reads its CPU and the process's energy counter (proc_pid_rusage's ri_energy_nj, the figure
# Activity Monitor and wsp's own Activity page are built on) reads its power. DESKTOP_GPUI_FRAMES gives its frames and
# renders a second. Prints one row per page and whether it meets the acceptance:
#   the New thread page under 2% of one core and 1 W (its field animates), Settings > Activity under 5% and 1 W,
#   the computer pane, whose readings move every 2 s, under 1 W; every other page under 0.3 W and drawing no frame but
#   a focused caret's and a sidebar time's step. A missing reading says NO READING.
#   apps/desktop-gpui/bench/rest.sh
# BENCH_GPUI_APP=<path> runs that build instead of building one; BENCH_OUT=<dir> keeps the results there; ONLY="<page>
# ..." measures only those pages. Off macOS it reads CPU from /proc and no power, as a dry run under Xvfb.
set -eu
# A Mac that idles or sleeps throttles the app it measures; caffeinate holds it awake for the whole run.
if [ "$(uname -s)" = Darwin ] && [ -z "${REST_AWAKE:-}" ]; then REST_AWAKE=1 exec caffeinate -dims "$0" "$@"; fi
bench=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-rest-XXXXXX")}
mkdir -p "$out"
app=${BENCH_GPUI_APP:-$repo/apps/desktop-gpui/target/release/desktop-gpui}
SETTLE=10
pid=""
finish() {
  code=$?
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then kill "$pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" || true; fi
  [ $code -eq 0 ] || echo "rest: stopped (exit $code); results so far in $out" >&2
}
trap finish EXIT
trap 'exit 130' INT TERM HUP

if [ ! -f "$repo/packages/host/dist/bin.js" ] || [ ! -f "$repo/apps/web/dist/index.html" ]; then
  echo "== building the host and the web app the fixture serves"
  (cd "$repo" && pnpm install --frozen-lockfile && pnpm build)
fi
if [ -z "${BENCH_GPUI_APP:-}" ]; then
  echo "== building desktop-gpui, release, 3 jobs"
  (cd "$repo/apps/desktop-gpui" && cargo build --release --locked -j 3 -p desktop-gpui)
fi

echo "== starting the fixture's host"
"$bench/fixture.sh" "$out/fixture.json"
field() { node -e 'const o = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); console.log(process.argv[2].split(".").reduce((v, k) => v?.[k], o) ?? "")' "$out/fixture.json" "$1"; }
state=$(field state)
long=$(field threads.long)
short=$(field threads.short-1)
home=$(dirname "$(dirname "$state")")/rest-home

# The process's energy counter in nanojoules: rusage_info_v6's ri_energy_nj, at offset 336.
energy() {
  python3 -c 'import ctypes, sys
buf = ctypes.create_string_buffer(512)
if ctypes.CDLL("/usr/lib/libSystem.B.dylib").proc_pid_rusage(int(sys.argv[1]), 6, buf) != 0: sys.exit(1)
print(int.from_bytes(buf.raw[336:344], "little"))' "$1"
}
# CPU seconds of a process off macOS, from /proc.
cpu_seconds() { awk -v k="$(getconf CLK_TCK)" '{ print ($14 + $15) / k }' "/proc/$1/stat"; }

pages="new-thread|
thread-long|--open $long
panel-launcher|--open $short --pane launcher
panel-processes|--open $short --pane processes
panel-computer|--open $short --pane computer
panel-agents|--open $short --pane agents"
for group in general appearance agents computers recipes projects usage activity keybindings privacy devices account; do
  pages="$pages
settings-$group|--screen settings:$group"
done

rows=$out/rest.tsv
printf 'page\tcpu%%\tpower_mW\tframes/s\trenders/s\tverdict\n' > "$rows"
while IFS='|' read -r page words; do
  if [ -n "${ONLY:-}" ]; then case " $ONLY " in *" $page "*) ;; *) continue ;; esac; fi
  echo "== $page: $SETTLE s settle, 60 s at rest"
  rm -rf "$home" && mkdir -p "$home"
  frames=$out/$page.frames
  rm -f "$frames"
  # shellcheck disable=SC2086
  env -i HOME="$home" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} DESKTOP_GPUI_FRAMES="$frames" \
    "$app" --state "$state" $words > "$out/$page.log" 2>&1 < /dev/null &
  pid=$!
  sleep $SETTLE
  kill -0 "$pid" 2>/dev/null || { echo "desktop-gpui exited early:" >&2; tail -20 "$out/$page.log" >&2; exit 1; }
  skip=$(wc -l < "$frames" 2>/dev/null || echo 0)
  began=$(date +%s)
  if [ "$(uname -s)" = Darwin ]; then
    e0=$(energy "$pid")
    top -l 31 -s 2 -pid "$pid" -stats pid,cpu,power > "$out/$page.top" < /dev/null
    e1=$(energy "$pid")
    elapsed=$(($(date +%s) - began))
    # The first sample counts since launch, so it is left out.
    cpu=$(awk -v p="$pid" '$1 == p { n++; if (n > 1) { s += $2; m++ } } END { printf "%.1f", m ? s / m : -1 }' "$out/$page.top")
    # A counter that never moved read nothing, which is not zero watts.
    power=$(awk -v a="$e0" -v b="$e1" -v t="$elapsed" 'BEGIN { if (b > a) printf "%.0f", (b - a) / 1e6 / t; else print "NA" }')
  else
    c0=$(cpu_seconds "$pid")
    sleep 60
    c1=$(cpu_seconds "$pid")
    elapsed=$(($(date +%s) - began))
    cpu=$(awk -v a="$c0" -v b="$c1" -v t="$elapsed" 'BEGIN { printf "%.1f", 100 * (b - a) / t }')
    power=NA
  fi
  # Frames and renders a second over the lines written while measured; renders are every counted view's but "frame".
  # A focused composer's caret blinks, drawing its field alone: the frames besides those are the still page's.
  rates=$(tail -n +"$((skip + 1))" "$frames" 2>/dev/null | awk '{ n++; for (i = 2; i <= NF; i++) { split($i, kv, "="); if (kv[1] == "frame") f += kv[2]; else r += kv[2]; if (kv[1] == "ComposerField") c += kv[2] } }
    END { if (n) printf "%.2f\t%.2f\t%.2f", f / n, r / n, (f - c) / n; else printf "0\t0\t0" }')
  still_fps=${rates##*"$(printf '\t')"}
  rates=${rates%"$(printf '\t')"*}
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  pid=""
  # A page that moves nothing draws no frame but the caret's; a reading that is missing meets nothing.
  verdict=$(awk -v page="$page" -v cpu="$cpu" -v mw="$power" -v fps="$still_fps" -v mac="$(uname -s)" 'BEGIN {
    if (cpu < 0 || (mac == "Darwin" && mw == "NA")) { print "NO READING"; exit }
    if (page == "new-thread") ok = cpu < 2 && (mw == "NA" || mw < 1000)
    else if (page == "settings-activity") ok = cpu < 5 && (mw == "NA" || mw < 1000)
    else if (page == "panel-computer") ok = mw == "NA" || mw < 1000
    else ok = (mw == "NA" || mw < 300) && fps < 0.1
    print ok ? "meets" : "MISSES" }')
  printf '%s\t%s\t%s\t%s\t%s\n' "$page" "$cpu" "$power" "$rates" "$verdict" | tee -a "$rows"
done <<EOF
$pages
EOF

echo "== the table (CPU: % of one core; power: the app's own, from its energy counter)"
column -t -s "$(printf '\t')" "$rows"
echo "results, with each page's top samples and frame counts: $out"
