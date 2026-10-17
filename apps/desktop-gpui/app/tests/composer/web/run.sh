#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# run.sh <out dir> [shots.mjs flags]: photographs the web composer's states into <out dir> through shots.mjs, which
# it starts with a cleared environment, and says where the images went.
set -eu
[ $# -ge 1 ] || { echo "usage: run.sh <out dir> [--only a,b] [--keep-host]" >&2; exit 2; }
case $1 in /*) out=$1 ;; *) out=$(pwd)/$1 ;; esac
shift
node=$(command -v node) || { echo "run.sh: node is not on PATH" >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
pid=
# A job started with & ignores the terminal's Ctrl-C, so the signal is passed on and its cleanup waited for.
trap 'if [ -n "$pid" ]; then kill -TERM "$pid" 2>/dev/null; wait "$pid"; fi; exit 1' HUP INT TERM
env -i PATH="$(dirname "$node"):/usr/local/bin:/usr/bin:/bin" ${DISPLAY:+DISPLAY="$DISPLAY"} \
  ${WSP_TEST_DAEMON:+WSP_TEST_DAEMON="$WSP_TEST_DAEMON"} ${CHROME:+CHROME="$CHROME"} \
  "$node" "$here/shots.mjs" --out "$out" "$@" &
pid=$!
status=0
wait "$pid" || status=$?
pid=
echo "run.sh: images in $out"
exit "$status"
