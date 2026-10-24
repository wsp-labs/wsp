#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Rebuilds desktop-gpui whenever a source file changes and swaps the running app only when the build passes.
set -u
cd "$(dirname "$0")"
# vendor/ is generated: a fresh clone makes it, and a pull that changed patches/ since the last apply stops here.
[ -d vendor ] || scripts/gpui-patches.sh apply || exit 1
cat patches/*.patch | cmp -s - vendor/.applied 2>/dev/null ||
  { echo "vendor/ is not what patches/ makes: run scripts/gpui-patches.sh apply, or save if you edited vendor/" >&2; exit 1; }
scripts/ghostty.sh >/dev/null || exit 1
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
