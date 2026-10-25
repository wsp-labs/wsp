# SPDX-License-Identifier: AGPL-3.0-only
# shellcheck shell=sh
# Sourced by the benches: a locked Mac screen draws no window, so a bench reads it before it starts and every 2 s while
# the app runs, and measures nothing it locked through. BENCH_LOCKED=<command> reads the lock instead (exit 0 when
# locked), for a test; elsewhere than a Mac the screen never locks for a bench.

# Whether the screen is locked now: CGSSessionScreenIsLocked, which the session sets only while it is.
screen_locked() {
  if [ -n "${BENCH_LOCKED:-}" ]; then
    $BENCH_LOCKED
  elif [ "$(uname -s)" = Darwin ]; then
    ioreg -n Root -d1 -a | grep -A1 '<key>CGSSessionScreenIsLocked</key>' | grep -q '<true/>'
  else
    return 1
  fi
}

# Refuses to start on a locked screen, exit 3.
refuse_locked() {
  if screen_locked; then
    echo "the screen is locked, and a locked screen draws no window; unlock it and run again" >&2
    exit 3
  fi
}

# watch_lock <pid> <marker>: while <pid>, a child of this script, runs, reads the lock every 2 s; once locked, writes
# <marker> and stops <pid>. Run it in the background and stop it by its own pid.
watch_lock() {
  while kill -0 "$1" 2>/dev/null; do
    if screen_locked; then
      : > "$2"
      echo "the screen locked mid-run; stopping the app, as a locked screen draws no window" >&2
      if [ "$(ps -o ppid= -p "$1" 2>/dev/null | tr -d ' ')" = "$$" ]; then kill "$1"; fi
      return
    fi
    sleep 2
  done
}
