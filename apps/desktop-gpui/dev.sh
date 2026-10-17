#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Rebuilds desktop-gpui whenever a source file changes and swaps the running app only when the build passes.
set -u
cd "$(dirname "$0")"
# vendor/ is generated, so a fresh clone makes it before the first build.
[ -d vendor ] || scripts/gpui-patches.sh apply || exit 1
bin=${CARGO_TARGET_DIR:-target}/debug/desktop-gpui
stamp=$(mktemp)
pid=
# Once the app quits on its own its pid can be reused, so only a child of this script counts as the app.
running() { [ -n "$pid" ] && [ "$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')" = "$$" ]; }
stop() {
  running && kill "$pid" 2>/dev/null
  rm -f "$stamp"
  exit 0
}
trap stop INT TERM HUP
while :; do
  touch "$stamp"
  # A write that changes no source, such as Finder's .DS_Store or an editor's swap file, leaves the binary as it was.
  if cargo build -p desktop-gpui && { ! running || [ "$bin" -nt "$stamp" ]; }; then
    if running; then
      kill "$pid" 2>/dev/null
      wait "$pid" 2>/dev/null
    fi
    "$bin" &
    pid=$!
  fi
  # Polls with find, because macOS has no inotifywait.
  while [ -z "$(find app host-client/src Cargo.toml -newer "$stamp" -type f 2>/dev/null | head -1)" ]; do
    sleep 0.3
  done
done
