#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# A day of a real host's traffic as tapes, on the Mac whose launchd agent serves that host.
#
#   parity/record/shadow-day.sh start [<state.json>]   restart the host with the tap on
#   parity/record/shadow-day.sh status                 how many tapes so far
#   parity/record/shadow-day.sh stop [<held dir>]      restart it with the tap off, keep the day in the held-back folder
#
# The state defaults to the dev host's, ~/wsp-work/ui-home/state.json. Each restart ends the host's sockets the way
# any update does. The tapes hold the person's own prompts, paths and titles, so they go to the held-back folder
# (default ~/wsp-work/parity-shadow/held), outside the repository: hand it to the reviewer by copying, never by a commit.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work="${PARITY_SHADOW_HOME:-$HOME/wsp-work/parity-shadow}"
uid=$(id -u)
mkdir -p "$work"

label_of() {
  printf 'com.wsp.host.%s' "$(printf '%s' "$1" | shasum -a 256 | cut -c1-8)"
}

restart() {
  launchctl kickstart -k "gui/$uid/$1" >/dev/null
}

case "${1:-}" in
  start)
    state="${2:-$HOME/wsp-work/ui-home/state.json}"
    label=$(label_of "$state")
    launchctl print "gui/$uid/$label" >/dev/null 2>&1 || { echo "no launchd agent $label serves $state" >&2; exit 1; }
    day="$work/$(date -u +%Y-%m-%dT%H%MZ)"
    mkdir -p "$day"
    printf '%s\n%s\n%s\n' "$label" "$day" "$(date -u +%s)" > "$work/current"
    # The domain's env reaches the agent at its restart and nothing started after the unset, so no other host tapes.
    launchctl setenv WSP_TAPE_DIR "$day"
    launchctl setenv WSP_TAPE_SOURCE "dev host"
    restart "$label"
    launchctl unsetenv WSP_TAPE_DIR
    launchctl unsetenv WSP_TAPE_SOURCE
    n=0
    while [ -z "$(ls -A "$day")" ] && [ "$n" -lt 120 ]; do sleep 1; n=$((n + 1)); done
    if [ -z "$(ls -A "$day")" ]; then
      echo "the host restarted but wrote no tape in 2 minutes: is the app open on it? $day stays empty" >&2
      exit 1
    fi
    echo "taping $label into $day; run $0 stop in about a day"
    ;;
  status)
    [ -f "$work/current" ] || { echo "not taping"; exit 0; }
    label=$(sed -n 1p "$work/current"); day=$(sed -n 2p "$work/current"); since=$(sed -n 3p "$work/current")
    echo "$label: $(ls "$day" | wc -l | tr -d ' ') tapes, $(du -sh "$day" | cut -f1), $(( ($(date -u +%s) - since) / 3600 )) h"
    ;;
  stop)
    [ -f "$work/current" ] || { echo "not taping" >&2; exit 1; }
    label=$(sed -n 1p "$work/current"); day=$(sed -n 2p "$work/current"); since=$(sed -n 3p "$work/current")
    restart "$label"
    rm "$work/current"
    hours=$(( ($(date -u +%s) - since) / 3600 ))
    echo "taped $label for $hours h: $(ls "$day" | wc -l | tr -d ' ') tapes in $day"
    held="${2:-$work/held}"
    node "$root/parity/record/collect.mjs" --from "$day" --held "$held"
    echo "copy it to the reviewer's held-back folder: rsync -a $held/tapes/shadow/ <box>:<held dir>/tapes/shadow/"
    ;;
  *)
    sed -n 3,11p "$0" | sed 's/^# \{0,1\}//' >&2
    exit 3
    ;;
esac
