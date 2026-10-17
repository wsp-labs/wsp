#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# The whole bench on the owner's Mac, one step at a time: build both apps, start the fixture's throwaway host, read
# each app's memory at rest, with `long` open and after one scroll through it, run each app's bench, stop the host,
# print the table. GPUI's memory with `long` open needs `desktop-gpui --state <path> --open <thread id>`, which opens
# that thread and stays. It stops at the first failure and names the step. It stops only the processes it started, by
# their pids, and never touches the live host: every process it starts reads the fixture's temp state.
#   apps/desktop-gpui/bench/run-mac.sh
# BENCH_OUT=<dir> keeps the results there (default: a new temp folder). BENCH_SKIP_GPUI=1 leaves GPUI's row empty, for
# a machine where it cannot draw; BENCH_GPUI_APP=<path> runs that build instead of building one;
# DESKTOP_GPUI_COMPOSER_GLASS=off reaches GPUI's runs and draws its composer without its backdrop blur. GPUI's bench
# switches 30 times among the short threads in turn, the same 30 in any build; a build that times its switches also
# carries 30 timed switches after those. The script runs on Linux too, as a dry run under Xvfb with DISPLAY set and a
# window manager on that display (openbox will do): GPUI asks the window manager to focus its window, so with none the
# GPUI bench exits 3.
set -eu
bench=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$bench/../../.." && pwd)
out=${BENCH_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/wsp-bench-out-XXXXXX")}
mkdir -p "$out"
SETTLE=10

now=""
gpui_pid=""
e_rest=""
e_long=""
e_scrolled=""
g_rest=""
g_long=""
g_scrolled=""
# GPUI's bench holds 10 s after its scroll, then switches; its reading after the scroll lands inside that hold, and
# electron.mjs waits as long after its own scroll.
SCROLLED_SETTLE=9
began=$(date +%s)
step() {
  now=$1
  echo
  echo "== $(($(date +%s) - began)) s: $now"
}
table() {
  node "$bench/table.mjs" "electron=$out/electron.json,$e_rest,$e_long,$e_scrolled" "gpui=$out/gpui.json,$g_rest,$g_long,$g_scrolled" | tee "$out/table.txt"
  case $(uname -s) in Darwin) echo "memory: physical footprint, MB of 2^20 bytes" ;; *) echo "memory: resident size (no footprint off macOS), MB of 2^20 bytes" ;; esac
}
finish() {
  code=$?
  if [ -n "$gpui_pid" ] && [ "$(ps -o ppid= -p "$gpui_pid" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$gpui_pid"; fi
  if [ -f "$out/fixture.json" ]; then "$bench/stop.sh" "$out/fixture.json" || true; fi
  if [ $code -ne 0 ]; then
    echo "run-mac: stopped at \"$now\" (exit $code); results so far in $out" >&2
    [ -f "$out/electron.json" ] && table
  fi
}
trap finish EXIT
trap 'exit 130' INT TERM
trap 'exit 129' HUP

case $(uname -s) in
  Darwin)
    electron_dir="--mac --dir"
    case $(uname -m) in arm64) electron_app="$repo/apps/desktop/dist/mac-arm64/wsp.app/Contents/MacOS/wsp" ;; *) electron_app="$repo/apps/desktop/dist/mac/wsp.app/Contents/MacOS/wsp" ;; esac
    ;;
  *)
    electron_dir="--linux --dir"
    electron_app="$repo/apps/desktop/dist/linux-unpacked/wsp"
    ;;
esac
gpui_app=${BENCH_GPUI_APP:-$repo/apps/desktop-gpui/target/release/desktop-gpui}

step "checking the tools"
node -e 'if (+process.versions.node.split(".")[0] < 22) { console.error(`node ${process.version} is older than 22`); process.exit(1) }'
pnpm --version >/dev/null
if [ "${BENCH_SKIP_GPUI:-}" != 1 ] && [ -z "${BENCH_GPUI_APP:-}" ]; then (cd "$repo/apps/desktop-gpui" && cargo --version); fi
echo "results go to $out"

step "installing packages"
cd "$repo"
pnpm install --frozen-lockfile

step "building the packages the Electron app holds (3 at a time)"
pnpm --filter "@wsp/desktop^..." --workspace-concurrency=3 build

step "staging the Electron app"
pnpm --filter @wsp/desktop run build:app

step "packaging the Electron app for this machine, unsigned"
# shellcheck disable=SC2086
# No identity: a CSC_LINK or CSC_NAME in the person's shell would otherwise sign the bench build and send it to notarize.
CSC_IDENTITY_AUTO_DISCOVERY=false pnpm --filter @wsp/desktop exec electron-builder --config electron-builder.yml $electron_dir -c.mac.identity=null --publish never
[ -x "$electron_app" ] || { echo "no app at $electron_app" >&2; exit 1; }

if [ "${BENCH_SKIP_GPUI:-}" != 1 ] && [ -z "${BENCH_GPUI_APP:-}" ]; then
  step "building desktop-gpui, release, 3 jobs"
  (cd "$repo/apps/desktop-gpui" && cargo build --release --locked -j 3 -p desktop-gpui)
fi

step "starting the fixture's host"
"$bench/fixture.sh" "$out/fixture.json"
state=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$out/fixture.json")
thread() { node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).threads[process.argv[2]])' "$out/fixture.json" "$1"; }

step "Electron: memory at rest, memory with long open, scroll, memory after it, switch"
node "$bench/electron.mjs" --app "$electron_app" --fixture "$out/fixture.json" --out "$out/electron.json" --mem "$out/electron-mem.json"
e_rest=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).rest)' "$out/electron-mem.json")
e_long=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).long)' "$out/electron-mem.json")
e_scrolled=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).scrolled)' "$out/electron-mem.json")

if [ "${BENCH_SKIP_GPUI:-}" != 1 ]; then
  home=$(dirname "$(dirname "$state")")
  # GPUI's own runs get the same clean environment as the host, the temp home and nothing of the person's, and start
  # as a plain command so the pid recorded is the app's own (env execs it).
  gpui_env="HOME=$home/gpui-home PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=en_US.UTF-8 ${DISPLAY:+DISPLAY=$DISPLAY} ${DESKTOP_GPUI_COMPOSER_GLASS:+DESKTOP_GPUI_COMPOSER_GLASS=$DESKTOP_GPUI_COMPOSER_GLASS}"
  mkdir -p "$home/gpui-home"

  step "GPUI: memory at rest"
  # shellcheck disable=SC2086
  env -i $gpui_env "$gpui_app" --state "$state" > "$out/gpui-rest.log" 2>&1 &
  gpui_pid=$!
  sleep $SETTLE
  kill -0 "$gpui_pid" 2>/dev/null || { echo "desktop-gpui exited early:" >&2; cat "$out/gpui-rest.log" >&2; exit 1; }
  g_rest=$("$bench/mem.sh" "$gpui_pid")
  echo "$g_rest MB"
  kill "$gpui_pid"
  wait "$gpui_pid" || true
  gpui_pid=""

  long=$(thread long)
  step "GPUI: scroll, switch"
  # Every build switches the same 30 times, so switch medians from two builds count the same switches.
  short="$(thread short-1),$(thread short-2),$(thread short-3)"
  switch=$short
  for _ in 2 3 4 5 6 7 8 9 10; do switch="$switch,$short"; done
  # shellcheck disable=SC2086
  env -i $gpui_env "$gpui_app" --state "$state" --bench "$out/gpui.json" --bench-thread "$long" --bench-switch "$switch" > "$out/gpui-bench.log" 2>&1 &
  gpui_pid=$!
  # The bench says "bench: scrolled" on stderr once its scroll is done; memory after the scroll is read off its pid then.
  began_bench=$(date +%s)
  while kill -0 "$gpui_pid" 2>/dev/null; do
    [ $(($(date +%s) - began_bench)) -lt 600 ] || { echo "desktop-gpui --bench ran past 10 minutes" >&2; exit 1; }
    if [ -z "$g_scrolled" ] && grep -q "bench: scrolled" "$out/gpui-bench.log"; then
      sleep $SCROLLED_SETTLE
      g_scrolled=$("$bench/mem.sh" "$gpui_pid") || { echo "desktop-gpui was gone $SCROLLED_SETTLE s after \"bench: scrolled\", inside the hold its bench must keep before it switches:" >&2; tail -20 "$out/gpui-bench.log" >&2; exit 1; }
      echo "memory after scrolling long: $g_scrolled MB"
    fi
    sleep 0.2
  done
  # Exit 3 is the app's own word that its window was not focused: an unfocused GPUI window is held to 30 fps.
  code=0
  wait "$gpui_pid" || code=$?
  gpui_pid=""
  [ $code -ne 3 ] || { echo "the GPUI bench window lost focus, so its frame rate is not the app's; keep the window in front and run again" >&2; tail -20 "$out/gpui-bench.log" >&2; exit 1; }
  [ $code -eq 0 ] || { echo "desktop-gpui --bench failed (exit $code):" >&2; tail -40 "$out/gpui-bench.log" >&2; exit 1; }
  [ -n "$g_scrolled" ] || { echo "desktop-gpui --bench never said \"bench: scrolled\", so its memory after the scroll was not read" >&2; exit 1; }

  step "GPUI: memory with long open"
  # shellcheck disable=SC2086
  env -i $gpui_env "$gpui_app" --state "$state" --open "$long" > "$out/gpui-long.log" 2>&1 &
  gpui_pid=$!
  sleep $SETTLE
  kill -0 "$gpui_pid" 2>/dev/null || { echo "desktop-gpui exited early:" >&2; cat "$out/gpui-long.log" >&2; exit 1; }
  g_long=$("$bench/mem.sh" "$gpui_pid")
  echo "$g_long MB"
  kill "$gpui_pid"
  wait "$gpui_pid" || true
  gpui_pid=""
fi

step "stopping the fixture's host"
"$bench/stop.sh" "$out/fixture.json"
rm "$out/fixture.json"

step "the table"
table
echo "results: $out"
