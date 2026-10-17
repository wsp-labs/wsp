#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Stops the host fixture.sh started, by the pid its out.json records, and removes the fixture's temp folder.
set -eu
[ $# -eq 1 ] || { echo "usage: stop.sh <out.json>" >&2; exit 2; }
read_out() { node -e 'const o = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); console.log(o[process.argv[2]])' "$1" "$2"; }
pid=$(read_out "$1" pid)
state=$(read_out "$1" state)
dir=$(dirname "$(dirname "$state")")
# A pid outlives its process and can be reused, so it is signalled only while it still runs the fixture's host.
if ps -o args= -p "$pid" 2>/dev/null | grep -qF -- "$state"; then
  kill -TERM "$pid"
  i=0
  while kill -0 "$pid" 2>/dev/null && [ $i -lt 100 ]; do sleep 0.2; i=$((i + 1)); done
  if kill -0 "$pid" 2>/dev/null; then kill -KILL "$pid"; echo "stop: host $pid did not stop in 20 s, killed"; else echo "stop: host $pid stopped"; fi
else
  echo "stop: host $pid is not running"
fi
[ -f "$dir/.wsp-bench-fixture" ] && rm -rf "$dir"
exit 0
