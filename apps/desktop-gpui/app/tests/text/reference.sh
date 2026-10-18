#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Shoots samples.html in headless Chrome at device scale 2 into chromium/<os>.png, and writes the layout numbers its
# script computes (each row's width and baseline, the faces Chrome drew) to chromium/<os>.json.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
out=${TEXT_REFERENCE:-$here/chromium}
case "$(uname -s)" in
  Darwin) os=macos ;;
  *) os=linux ;;
esac

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

profile=$(mktemp -d "${TMPDIR:-/tmp}/text-chrome.XXXXXX")
trap 'rm -rf "$profile"' EXIT
trap 'exit 130' HUP INT TERM

# Grayscale antialiasing, as macOS draws text and GPUI's headless capture does; Chrome on Linux would draw LCD text.
flags=(--headless=new --force-device-scale-factor=2 "--window-size=760,3840" --hide-scrollbars --disable-lcd-text
  --no-first-run --no-default-browser-check --disable-extensions --user-data-dir="$profile")
if [ "$(id -u)" = 0 ]; then
  flags+=(--no-sandbox)
fi

mkdir -p "$out"
url="file://$here/samples.html"
echo "chrome: $("$chrome" --version 2>/dev/null)"
# Nothing of the caller's environment, no DISPLAY above all: one without access fails Chrome's GPU process quietly.
env -i HOME="$profile" PATH="$PATH" "$chrome" "${flags[@]}" --screenshot="$out/$os.png" "$url" >/dev/null 2>&1
env -i HOME="$profile" PATH="$PATH" "$chrome" "${flags[@]}" --dump-dom "$url" 2>/dev/null |
  sed -n 's/.*<pre id="out">\(.*\)<\/pre>.*/\1/p' >"$out/$os.json"
if [ ! -s "$out/$os.png" ] || [ ! -s "$out/$os.json" ]; then
  echo "chrome wrote no screenshot or no numbers to $out" >&2
  exit 1
fi
echo "wrote $out/$os.png and $out/$os.json"
