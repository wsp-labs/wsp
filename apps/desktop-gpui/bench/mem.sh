#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# mem.sh <pid>: the memory of a process and every process under it, in MB (2^20 bytes), as one number. On macOS it is
# the physical footprint Activity Monitor shows, read by proc_pid_rusage; elsewhere resident size, which counts a page
# shared between two of the processes twice. A wsp host or daemon under the process is shared and never counted.
set -eu
[ $# -eq 1 ] || { echo "usage: mem.sh <pid>" >&2; exit 2; }
root=$1
kill -0 "$root" 2>/dev/null || { echo "mem: no process $root" >&2; exit 1; }

pids=$(ps -A -o pid= -o ppid= | awk -v root="$root" '
  { parent[$1] = $2; seen[NR] = $1 }
  END {
    keep[root] = 1; grew = 1
    while (grew) { grew = 0; for (i in seen) { p = seen[i]; if (!(p in keep) && (parent[p] in keep)) { keep[p] = 1; grew = 1 } } }
    for (p in keep) print p
  }')

counted=""
for pid in $pids; do
  args=$(ps -o args= -p "$pid" 2>/dev/null || true)
  case "$args" in
    "") continue ;;
    *packages/host/dist/bin.js*|*wsp-daemon*|*" up --state "*|*" up --port "*) echo "mem: not counting $pid, the wsp host or daemon: $args" >&2; continue ;;
  esac
  counted="$counted $pid"
done

if [ "$(uname -s)" = Darwin ]; then
  total=0
  for pid in $counted; do
    # phys_footprint, Activity Monitor's Memory column, at offset 72 of rusage_info_v0. It needs no task port, so the
    # hardened Electron app reads the same way as GPUI.
    kb=$(python3 -c 'import ctypes, sys
buf = ctypes.create_string_buffer(512)
if ctypes.CDLL("/usr/lib/libSystem.B.dylib").proc_pid_rusage(int(sys.argv[1]), 0, buf) != 0: sys.exit(1)
print(int.from_bytes(buf.raw[72:80], "little") / 1024)' "$pid") || { kill -0 "$pid" 2>/dev/null || continue; echo "mem: proc_pid_rusage could not read pid $pid" >&2; exit 1; }
    total=$(awk -v a="$total" -v b="$kb" 'BEGIN { printf "%.1f\n", a + b }')
  done
  echo "mem: physical footprint of $(echo "$counted" | wc -w | tr -d ' ') processes" >&2
else
  # shellcheck disable=SC2086
  total=$(ps -o rss= -p "$(echo $counted | tr ' ' ',')" | awk '{ s += $1 } END { print s }')
  echo "mem: resident size of $(echo "$counted" | wc -w | tr -d ' ') processes (no footprint here)" >&2
fi
awk -v kb="$total" 'BEGIN { printf "%.1f\n", kb / 1024 }'
