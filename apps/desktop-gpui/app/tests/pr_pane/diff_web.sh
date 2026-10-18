#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# diff_web.sh <out dir>: builds diff_web/ (today's StyledDiffCodeView over diff_sample.patch, with the web app's own
# stylesheet and themes) with apps/web's vite, serves it on 127.0.0.1, and shoots web-paper.png, web-graphite.png,
# web-paper-split.png and web-graphite-split.png in headless Chrome at --force-device-scale-factor=2, 800 x 700.
# pr_diff_shots.rs reads them through DIFF_WEB. Chrome and the server run with a cleared environment and a HOME of
# their own; the script stops only the pids it started.
set -eu
[ $# -eq 1 ] || { echo "usage: diff_web.sh <out dir>" >&2; exit 2; }
case $1 in /*) out=$1 ;; *) out=$(pwd)/$1 ;; esac
here=$(cd "$(dirname "$0")" && pwd)
web=$(cd "$here/../../../../web" && pwd)
node=$(command -v node) || { echo "diff_web.sh: node is not on PATH" >&2; exit 1; }
chrome=${CHROME:-$(command -v google-chrome || command -v chromium || true)}
[ -n "$chrome" ] || { echo "diff_web.sh: no google-chrome or chromium on PATH" >&2; exit 1; }
work=$(mktemp -d)
server=
cleanup() {
  if [ -n "$server" ] && [ "$(ps -o ppid= -p "$server" | tr -d ' ')" = "$$" ]; then kill -TERM "$server"; wait "$server" 2>/dev/null || true; fi
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir -p "$out" "$work/site" "$work/home"
cp "$here/diff_web/index.html" "$here/diff_web/main.tsx" "$work/site/"
ln -s "$web/node_modules" "$work/site/node_modules"
printf '@import "%s/src/index.css";\n@source "%s/src";\n' "$web" "$web" > "$work/site/app.css"
cat > "$work/site/vite.config.mjs" <<CONFIG
import tailwindcss from "$web/node_modules/@tailwindcss/vite/dist/index.mjs";
import react from "$web/node_modules/@vitejs/plugin-react/dist/index.js";
export default {
  root: "$work/site",
  plugins: [react(), tailwindcss()],
  resolve: { alias: [{ find: /^@web/, replacement: "$web/src" }, { find: /^@sample/, replacement: "$here/diff_sample.patch" }], dedupe: ["react", "react-dom"] },
  build: { target: "es2022", outDir: "$work/dist", emptyOutDir: true, chunkSizeWarningLimit: 4096 },
  worker: { format: "es" },
};
CONFIG
(cd "$work/site" && env -i PATH="$(dirname "$node"):/usr/bin:/bin" HOME="$work/home" "$node" node_modules/vite/bin/vite.js build --logLevel warn --config vite.config.mjs)
port=$(( 20000 + $$ % 20000 ))
env -i PATH=/usr/bin:/bin HOME="$work/home" python3 -m http.server "$port" --bind 127.0.0.1 --directory "$work/dist" >/dev/null 2>&1 &
server=$!
sleep 1
for theme in paper graphite; do
  for mode in stacked split; do
    name=web-$theme.png
    [ "$mode" = split ] && name=web-$theme-split.png
    env -i PATH=/usr/bin:/bin HOME="$work/home" timeout 90 "$chrome" --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
      --user-data-dir="$work/home/chrome" --force-device-scale-factor=2 --window-size=800,700 --virtual-time-budget=20000 \
      --screenshot="$out/$name" "http://127.0.0.1:$port/index.html?theme=$theme&mode=$mode" >/dev/null 2>&1
    echo "diff_web.sh: $out/$name"
  done
done
