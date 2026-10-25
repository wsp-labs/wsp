#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Starts a built GPUI AppImage once on the display it runs under, with a HOME of its own and nothing else of the
# caller's environment, and passes once a window named wsp is up and the app still runs. Exits 1 with what the app
# printed otherwise. Run it under a display: xvfb-run apps/desktop-gpui/scripts/appimage-launch.sh <AppImage>
set -u

image=$(readlink -f "${1:?usage: appimage-launch.sh <AppImage>}")
[ -n "${DISPLAY-}" ] || { echo "refused: no DISPLAY; run this under xvfb-run" >&2; exit 2; }
home=$(mktemp -d)
log=$home/app.log
# xvfb-run hands its display's cookie through XAUTHORITY, which the app needs to connect. APPIMAGE_EXTRACT_AND_RUN
# unpacks the image under TMPDIR instead of mounting it, which needs a FUSE a runner need not have.
setsid env -i PATH=/usr/bin:/bin HOME="$home" TMPDIR="$home" DISPLAY="$DISPLAY" ${XAUTHORITY:+XAUTHORITY="$XAUTHORITY"} \
  APPIMAGE_EXTRACT_AND_RUN=1 "$image" >"$log" 2>&1 &
pid=$!
# The unpacking runtime runs the app as its child and leaves it running when killed alone, so its whole group goes.
stop() {
  kill -- -"$pid" 2>/dev/null
  for _ in $(seq 50); do
    kill -0 -- -"$pid" 2>/dev/null || break
    sleep 0.1
  done
  rm -rf "$home"
}
trap stop EXIT

for _ in $(seq 120); do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "refused: the AppImage exited before its window was up; it printed:" >&2
    cat "$log" >&2
    exit 1
  fi
  if xwininfo -root -tree | grep -q '^ *0x[0-9a-f]* "wsp":'; then
    echo "ok    the window wsp is up and the app runs (pid $pid)"
    exit 0
  fi
  sleep 0.5
done
echo "refused: no window named wsp within 60 s; the app printed:" >&2
cat "$log" >&2
exit 1
