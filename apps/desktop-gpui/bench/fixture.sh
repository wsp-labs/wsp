#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# fixture.sh <out.json>: a throwaway host serving the bench's four threads, left running until stop.sh <out.json>.
set -eu
[ $# -eq 1 ] || { echo "usage: fixture.sh <out.json>" >&2; exit 2; }
case $1 in /*) out=$1 ;; *) out=$(pwd)/$1 ;; esac
node=$(command -v node)
bench=$(cd "$(dirname "$0")" && pwd)
dir=$(cd "$(mktemp -d "${TMPDIR:-/tmp}/wsp-bench-XXXXXX")" && pwd -P)
cd "$dir"
# From here on nothing sees the person's environment: HOME is the temp folder, for the wsp lines run inside node too.
exec env -i HOME="$dir" PATH="$dir/bin:$(dirname "$node"):/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin" LANG=en_US.UTF-8 WSP_UPDATE_CHECK=0 \
  "$node" "$bench/fixture.mjs" "$dir" "$out"
