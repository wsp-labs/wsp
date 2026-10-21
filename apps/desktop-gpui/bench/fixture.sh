#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# fixture.sh <out.json> [<n> [bare]]: a throwaway host serving the bench's threads and n more short ones, left running
# until stop.sh <out.json>; bare makes the n short ones alone, for a bench that steps through top-level threads only.
# BENCH_SESSION=<n> adds session-1 to session-<n> (session.mjs), for bench/session.sh; BENCH_SEED=<n> starts the host on
# n threads, most of them settled, over n/10 workspaces written into its state first (seed.mjs), shaped like a
# long-used host.
set -eu
[ $# -eq 1 ] || { [ $# -le 3 ] && [ "$2" -ge 0 ] 2>/dev/null && [ "${3:-bare}" = bare ]; } ||
  { echo "usage: fixture.sh <out.json> [<n more threads> [bare]]" >&2; exit 2; }
case $1 in /*) out=$1 ;; *) out=$(pwd)/$1 ;; esac
node=$(command -v node)
bench=$(cd "$(dirname "$0")" && pwd)
dir=$(cd "$(mktemp -d "${TMPDIR:-/tmp}/wsp-bench-XXXXXX")" && pwd -P)
cd "$dir"
# From here on nothing sees the person's environment: HOME is the temp folder, for the wsp lines run inside node too.
exec env -i HOME="$dir" PATH="$dir/bin:$(dirname "$node"):/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin" LANG=en_US.UTF-8 WSP_UPDATE_CHECK=0 ${BENCH_SESSION:+BENCH_SESSION=$BENCH_SESSION} ${BENCH_SEED:+BENCH_SEED=$BENCH_SEED} \
  "$node" "$bench/fixture.mjs" "$dir" "$out" "${2:-0}" ${3:+"$3"}
