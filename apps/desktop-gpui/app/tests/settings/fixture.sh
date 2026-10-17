#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# fixture.sh <out.json>: the bench's throwaway host (bench/fixture.sh) with what the Settings pages need on top of it,
# added through wsp lines and host ops only. Stop it with bench/stop.sh <out.json>. WSP_ROOT names the tree whose
# packages/host and apps/web are built, this one by default.
set -eu
[ $# -eq 1 ] || { echo "usage: fixture.sh <out.json>" >&2; exit 2; }
case $1 in /*) out=$1 ;; *) out=$(pwd)/$1 ;; esac
here=$(cd "$(dirname "$0")" && pwd)
root=${WSP_ROOT:-$(cd "$here/../../../../.." && pwd)}
node=$(command -v node)
"$root/apps/desktop-gpui/bench/fixture.sh" "$out"
# From here a host runs, so leaving early stops it.
trap '"$root/apps/desktop-gpui/bench/stop.sh" "$out"; exit 1' HUP INT TERM
state=$(env -i PATH="$PATH" "$node" -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).state)' "$out")
dir=$(dirname "$(dirname "$state")")
# The same cleared environment the bench's host runs in: HOME is the fixture's folder, where the agents' files live.
env -i HOME="$dir" PATH="$dir/bin:$(dirname "$node"):/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin" LANG=en_US.UTF-8 WSP_UPDATE_CHECK=0 \
  "$node" "$here/fixture.mjs" "$root" "$out" || { "$root/apps/desktop-gpui/bench/stop.sh" "$out"; exit 1; }
trap - HUP INT TERM
