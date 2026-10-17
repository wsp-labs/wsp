#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Renders the effects scenes in headless Chrome into chromium/<scene>-<scroll>.png and checks them.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
out=${EFFECTS_REFERENCE_DIR:-$here/chromium}
scenes="plain-light blur-light blur-dark blur-two clip"
scrolls="0 50 117"

chrome=${CHROME:-}
if [ -z "$chrome" ]; then
  for candidate in google-chrome google-chrome-stable chromium chromium-browser \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"; do
    if command -v "$candidate" >/dev/null 2>&1 || [ -x "$candidate" ]; then
      chrome=$candidate
      break
    fi
  done
fi
if [ -z "$chrome" ]; then
  echo "no Chrome found, set CHROME=/path/to/chrome" >&2
  exit 1
fi

if command -v magick >/dev/null 2>&1; then
  im_convert() { magick "$@"; }
  im_compare() { magick compare "$@"; }
  im_identify() { magick identify "$@"; }
else
  im_convert() { convert "$@"; }
  im_compare() { compare "$@"; }
  im_identify() { identify "$@"; }
fi

profile=$(mktemp -d "${TMPDIR:-/tmp}/effects-chrome.XXXXXX")
chrome_pid=

stop_chrome() {
  if [ -n "$chrome_pid" ] && kill -0 "$chrome_pid" 2>/dev/null; then
    kill "$chrome_pid" 2>/dev/null || true
    wait "$chrome_pid" 2>/dev/null || true
  fi
  chrome_pid=
}

cleanup() {
  stop_chrome
  rm -rf "$profile"
}
trap cleanup EXIT
trap 'exit 130' HUP INT TERM

flags="--headless=new --force-device-scale-factor=2 --window-size=640,480 --hide-scrollbars"
flags="$flags --use-angle=swiftshader --enable-unsafe-swiftshader"
flags="$flags --no-first-run --no-default-browser-check --disable-extensions"
if [ "$(id -u)" = 0 ]; then
  flags="$flags --no-sandbox"
fi

echo "chrome: $("$chrome" --version 2>/dev/null)"
echo "flags: $flags --user-data-dir=<tmp> --screenshot=<png> <url>"

mkdir -p "$out"

for scene in $scenes; do
  for scroll in $scrolls; do
    png="$out/$scene-$scroll.png"
    rm -f "$png"
    # An X DISPLAY without access makes the GPU process fail and Chrome silently falls back to the software compositor.
    # shellcheck disable=SC2086
    env -u DISPLAY -u WAYLAND_DISPLAY "$chrome" $flags --user-data-dir="$profile" \
      --screenshot="$png" "file://$here/scene.html?scene=$scene&scroll=$scroll" >/dev/null 2>&1 &
    chrome_pid=$!
    waited=0
    while kill -0 "$chrome_pid" 2>/dev/null; do
      if [ "$waited" -ge 300 ]; then
        echo "chrome timed out on $scene $scroll" >&2
        exit 1
      fi
      sleep 0.1
      waited=$((waited + 1))
    done
    wait "$chrome_pid" || true
    chrome_pid=
    if [ ! -s "$png" ]; then
      echo "chrome wrote no screenshot for $scene $scroll" >&2
      exit 1
    fi
    size=$(im_identify -format '%wx%h' "$png")
    if [ "$size" != 1280x960 ]; then
      echo "$png is $size, expected 1280x960" >&2
      exit 1
    fi
    echo "wrote $png"
  done
done

fail=0
check() {
  if [ "$2" != "$3" ]; then
    echo "FAIL $1: got $2, expected $3" >&2
    fail=1
  else
    echo "ok   $1: $2"
  fi
}

pixel() {
  im_convert "$1" -format "%[hex:p{$2,$3}]" info: | cut -c1-6
}

# Row 0 bar at CSS (300, 18), row 9 bar under the plain panel at CSS (550, 350): #3b82f6 under 55% white.
check "plain-light-0 row 0 bar" "$(pixel "$out/plain-light-0.png" 600 36)" E11D48
check "plain-light-0 ink stripe" "$(pixel "$out/plain-light-0.png" 81 40)" 18181B
check "plain-light-0 page" "$(pixel "$out/plain-light-0.png" 20 20)" F4F4F5
check "plain-light-0 tinted bar" "$(pixel "$out/plain-light-0.png" 1100 700)" A7C7FB
check "clip-0 box corner outside radius" "$(pixel "$out/clip-0.png" 361 81)" F4F4F5
check "clip-0 row 0" "$(pixel "$out/clip-0.png" 800 120)" E11D48

for scroll in $scrolls; do
  plain="$out/plain-light-$scroll.png"
  blur="$out/blur-light-$scroll.png"
  outside=0
  for crop in 1280x600+0+0 1280x120+0+840 80x240+0+600 80x240+1200+600; do
    n=$(im_convert "$plain" "$blur" -crop "$crop" +repage \
      -compose difference -composite -separate -evaluate-sequence max -threshold 0 -format '%[fx:int(mean*w*h+0.5)]' info:)
    outside=$((outside + n))
  done
  check "blur-light-$scroll equals plain-light outside the panel (px differing)" "$outside" 0
  inside=$(im_convert "$plain" "$blur" -crop 1032x152+124+644 +repage \
    -compose difference -composite -separate -evaluate-sequence max -threshold 3% \
    -format '%[fx:int(mean*100)]' info:)
  if [ "$inside" -lt 60 ]; then
    echo "FAIL blur-light-$scroll differs from plain-light in only $inside% of the panel interior" >&2
    fail=1
  else
    echo "ok   blur-light-$scroll differs from plain-light in $inside% of the panel interior"
  fi
done

if [ "$fail" != 0 ]; then
  exit 1
fi
