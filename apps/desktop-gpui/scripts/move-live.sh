#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# usage: move-live.sh <todays-app> <todays-version> <gpui-app> <gpui-version>
# Proves on Linux or macOS that today's Electron app moves a person over to the GPUI app through today's own update
# code, in a temp folder with a temp HOME. <todays-app> is today's app one release behind (an AppImage on Linux, a
# wsp.app on a Mac, built by apps/desktop's electron-builder), <gpui-app> the GPUI app scripts/package.sh built at
# <gpui-version> (its AppImage, or its .app). Today's app runs on a state seeded with threads and installs its host as
# a real service: on Linux a systemd user unit, which root's user manager loads through a link in
# $XDG_RUNTIME_DIR/systemd/user; on a Mac a launchd job labelled com.wsp.host.<tag> in the person's own gui domain. A
# local feed offers the GPUI app as the newer release under the name 0.3.8 asks for (bundleNames in
# packages/protocol/src/bundles.mjs). Today's update code (apps/desktop/src/get-bundle.ts and self-update.ts, bundled
# here as apps/desktop's tsup bundles them) gets the release, checks it and opens it as the update card's Get and
# Restart do, on the feed, which a packaged app would not read: on Linux into Downloads, which the person then runs; on a
# Mac the swap into today's place, which starts the GPUI app and waits on its up. SCENARIO=swap on Linux runs the Mac's
# swap instead: today's SWAP_SCRIPT, read out of self-update.ts, moves a wsp.app shaped bundle whose executable starts
# the GPUI AppImage standing beside it in a fake Applications, with plutil, codesign and open faked, launchctl kickstart
# made `systemctl --user restart` of the unit, and the files today's wsp command names taken away first, as a Mac's swap
# takes them from under the service; the GPUI app is handed DISPLAY, which a Mac's swap does not need. Then it checks that one host serves
# every seeded thread on the GPUI app's Node and command, that the wsp command and the unit run its files, that today's
# switch is gone, and on Linux that a wsp:// link opened with xdg-open reaches the running GPUI app.
# On a Mac today's app runs with WSP_DESKTOP_SMOKE=1, so it neither offers its move nor tells LaunchServices it handles
# wsp://, and both builds should carry a bundle id of their own (electron-builder -c.appId=..., package.sh's
# WSP_GPUI_BUNDLE_ID), so nothing here changes which app opens the person's own links. wsp down removes the service at
# the end, and the script checks it went. KEEP_WORK=1 keeps the work folder after a pass; a failed run always keeps it.
# REPO=<checkout> names the repo whose sources and bench seed it reads (default: the one holding this script).
set -euo pipefail
umask 022

usage='usage: [SCENARIO=download|swap] move-live.sh <todays-app> <todays-version> <gpui-app> <gpui-version>'
scenario=${SCENARIO:-download}
[ $# -eq 4 ] || { echo "$usage" >&2; exit 2; }
abs() { (cd "$(dirname "$1")" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$1")"); }
today_app=$(abs "$1")
old_v=$2
gpui_app=$(abs "$3")
new_v=$4
here=$(cd "$(dirname "$0")" && pwd -P)
REPO=${REPO:-$(cd "$here/../../.." && pwd -P)}
os=$(uname -s)
uid=$(id -u)

refuse() { echo "refused: $*" >&2; exit 2; }
case "$scenario" in
  download) ;;
  swap) [ "$(uname -s)" = Linux ] || refuse "SCENARIO=swap runs on Linux; a Mac runs the real swap" ;;
  *) refuse "SCENARIO takes download or swap, not $scenario" ;;
esac
for v in "$old_v" "$new_v"; do
  case "$v" in [0-9]*.[0-9]*.[0-9]*) ;; *) refuse "$v is not a version like 1.2.3" ;; esac
done
node_bin=$(command -v node) || refuse "node is not on PATH"
[ -f "$REPO/packages/protocol/dist/index.js" ] || refuse "$REPO/packages/protocol is not built (pnpm --filter @wsp/protocol build)"
[ -x "$REPO/node_modules/.bin/tsup" ] || refuse "$REPO has no node_modules (pnpm install)"
case "$os" in
  Linux)
    [ -f "$today_app" ] && [ -f "$gpui_app" ] || refuse "on Linux both apps are AppImage files"
    [ -n "${DISPLAY-}" ] || refuse "DISPLAY is not set; start an X server (Xvfb) and set it"
    [ -x /usr/bin/xdg-open ] || refuse "/usr/bin/xdg-open is missing: apt-get install xdg-utils"
    XDG_RUNTIME_DIR=/run/user/$uid
    DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus
    export XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS
    case "$(systemctl --user is-system-running 2>/dev/null || true)" in
      running|degraded) ;;
      *) refuse "systemctl --user is-system-running fails: this account's systemd user manager is not up at $XDG_RUNTIME_DIR" ;;
    esac
    lang=${LANG:-C.UTF-8}
    ;;
  Darwin)
    [ -d "$today_app" ] && [ -d "$gpui_app" ] || refuse "on a Mac both apps are .app folders"
    lang=${LANG:-en_US.UTF-8}
    ;;
  *) refuse "this runs on Linux or macOS, not $os" ;;
esac

sha() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi | cut -d' ' -f1; }
work=$(mktemp -d "${TMPDIR:-/tmp}/move-live.XXXXXX")
work=$(cd "$work" && pwd -P)
home=$work/home
state=$home/.wsp/state.json
lock=$home/.wsp/host.lock
wsp=$home/.wsp/bin/wsp
tag=$(printf '%s' "$state" | sha | cut -c1-8)
if [ "$os" = Linux ]; then
  installed=$home/Applications/wsp.AppImage
  unit_name=wsp-host-$tag.service
  unit=$home/.config/systemd/user/$unit_name
  unit_link=$XDG_RUNTIME_DIR/systemd/user/$unit_name
else
  installed=$work/install/wsp.app
  unit_name=com.wsp.host.$tag
  unit=$home/Library/LaunchAgents/$unit_name.plist
  unit_link=
fi
mkdir -p "$home/.wsp" "$home/Downloads" "$(dirname "$installed")" "$work/feed" "$work/driver"

feed_pid='' today_pid='' gpui_pid='' hosts='' kids='' result='' linked=''
say() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
args_of() { ps -o args= -p "$1" 2>/dev/null || true; }
children() { ps -A -o pid= -o ppid= | awk -v p="$1" '$2 == p { print $1 }'; }
tree() {
  local kid
  for kid in $(children "$1"); do echo "$kid"; tree "$kid"; done
}
lock_pid() { sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lock" 2>/dev/null || true; }
spawn() {
  local log=$1
  shift
  ( "$@" >>"$log" 2>&1 </dev/null & echo $! )
}
stop() {
  local pid=$1 what=$2 n=0
  alive "$pid" || return 0
  kill "$pid" 2>/dev/null || true
  while alive "$pid"; do
    n=$((n + 1))
    if [ "$n" -gt 200 ]; then kill -KILL "$pid" 2>/dev/null || true; say "killed $what $pid after 20 s"; return 0; fi
    sleep 0.1
  done
  say "stopped $what $pid"
}
base_env=(HOME="$home" WSP_HOME="$home/.wsp" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG="$lang")
if [ "$os" = Linux ]; then base_env+=(DISPLAY="$DISPLAY" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS"); fi
wsp_line() { (cd "$home" && env -i "${base_env[@]}" "$wsp" "$@"); }
unit_loaded() {
  if [ "$os" = Linux ]; then
    [ "$(systemctl --user show "$unit_name" --property=LoadState --value 2>/dev/null)" = loaded ]
  else
    launchctl print "gui/$uid/$unit_name" >/dev/null 2>&1
  fi
}
cleanup() {
  local code=$? pid
  trap - EXIT HUP INT TERM
  set +e
  echo "== cleanup"
  for pid in $gpui_pid $today_pid; do kids="$kids $(tree "$pid")"; stop "$pid" app; done
  pid=$(lock_pid)
  if alive "$pid"; then kids="$kids $(tree "$pid")"; fi
  if [ -x "$wsp" ]; then
    say "wsp down --state $state"
    wsp_line down --state "$state" 2>&1 | sed 's/^/  /'
  fi
  if [ -e "$unit" ] || unit_loaded; then
    say "the unit is still there after wsp down; taking it away"
    if [ "$os" = Linux ]; then systemctl --user disable --now "$unit_name" 2>&1 | sed 's/^/  /'; else launchctl bootout "gui/$uid/$unit_name" 2>&1 | sed 's/^/  /'; fi
    rm -f "$unit"
  fi
  if [ -n "$linked" ] && [ -L "$unit_link" ]; then
    rm -f "$unit_link"
    systemctl --user daemon-reload
  fi
  if [ -e "$unit" ] || unit_loaded; then echo "  the unit $unit_name is still there"; else echo "  the unit $unit_name is gone"; fi
  for pid in $(lock_pid) $hosts; do
    if alive "$pid"; then kids="$kids $(tree "$pid")"; stop "$pid" host; fi
  done
  for pid in $kids; do stop "$pid" "child of an app or a host"; done
  stop "$feed_pid" feed
  sleep 1
  echo "== left running (processes naming $work)"
  # shellcheck disable=SC2009
  ps -A -o pid= -o args= | grep -F -e "$work" | grep -v -e 'grep -F' || echo "  nothing"
  if [ "$result" = PASS ] && [ "${KEEP_WORK-}" != 1 ]; then rm -rf "$work"; else echo "kept $work"; fi
  exit "$code"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

fail() {
  result=FAIL
  echo "== feed requests"; cat "$work/feed.log" 2>/dev/null
  echo "== today's update log"; cat "$home/.wsp/desktop/update.log" 2>/dev/null || echo "  (none)"
  echo "== the GPUI app's log, its launch lines"
  grep -h -e 'update:' -e 'wsp command' -e 'switch' -e 'wsp://' -e 'restart' "$home"/.wsp/logs/app*.log 2>/dev/null | tail -n 40 || echo "  (none)"
  echo "FAIL: $*"
  exit 1
}

# The name today's update asks the release for, by its own table.
# shellcheck disable=SC2016
asset_of() {
  "$node_bin" --input-type=module -e '
    import { pathToFileURL } from "node:url";
    const [repo, key, version] = process.argv.slice(1);
    const { bundleNames } = await import(pathToFileURL(`${repo}/packages/protocol/src/bundles.mjs`).href);
    console.log(bundleNames(version)[key]);
  ' "$REPO" "$1" "$2"
}

say "work folder $work"
if [ "$os" = Linux ]; then
  cp "$today_app" "$installed"
  chmod 755 "$installed"
  asset=$(asset_of appImage "$new_v")
  cp "$gpui_app" "$work/feed/$asset"
else
  ditto "$today_app" "$installed"
  asset=$(asset_of macZip "$new_v")
  (cd "$(dirname "$gpui_app")" && ditto -c -k --sequesterRsrc --keepParent "$(basename "$gpui_app")" "$work/feed/$asset")
fi
echo "$new_v" >"$work/feed/VERSION"
say "today's app $old_v at $installed; the feed offers the GPUI app $new_v as $asset"

port=$("$node_bin" -e 'const s = require("node:net").createServer().listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });')
feed_pid=$(spawn "$work/feed.log" "$node_bin" "$here/release-feed.mjs" "$work/feed" "$port")
api=http://127.0.0.1:$port
n=0
until grep -q '^feed:' "$work/feed.log" 2>/dev/null; do
  alive "$feed_pid" || fail "the feed did not start"
  n=$((n + 1)); [ "$n" -lt 100 ] || fail "the feed did not start within 10 s"
  sleep 0.1
done

# Today's update road, bundled as apps/desktop bundles it, driven as its bridge channels are.
cat >"$work/driver/package.json" <<EOF
{ "name": "move-live-driver", "version": "$old_v", "type": "module" }
EOF
# 0.3.8's road, which today's source keeps but for the asset it names: 0.3.8 asks for bundleNames, which now names the
# GPUI app's files, where today's source asks for the classic app's.
cat >"$work/driver/driver.ts" <<EOF
import { getBundle, openBundle } from "$REPO/apps/desktop/src/get-bundle.ts";
import { stageOf, stageUpdate, startSwap, swapEnv } from "$REPO/apps/desktop/src/self-update.ts";
import { bundleNames } from "$REPO/packages/protocol/src/bundles.mjs";
import { mkdir } from "node:fs/promises";
const [platform, dir, running, version, appPid, bundle, service, log] = process.argv.slice(2);
const quit = (): void => process.kill(Number(appPid), "SIGTERM");
const deps = { platform: platform as NodeJS.Platform, env: process.env, userAgent: \`wsp/\${running}\`, fetch };
if (platform === "darwin") {
  await mkdir(stageOf(bundle), { recursive: true, mode: 0o700 });
  const got = await getBundle(version, { ...deps, dir: stageOf(bundle) }, v => bundleNames(v).macZip);
  console.log(\`get: \${JSON.stringify(got)}\`);
  if (!got.ok) process.exit(1);
  const staged = await stageUpdate(got.file, version, bundle);
  console.log(\`stage: \${JSON.stringify(staged)}\`);
  if (!staged.ok) process.exit(1);
  const { ok: _ok, ...copy } = staged;
  startSwap({ pid: Number(appPid), bundle, staged: copy, version, service, log }, swapEnv(process.env));
  quit();
} else {
  const got = await getBundle(version, { ...deps, dir }, v => bundleNames(v).appImage);
  console.log(\`get: \${JSON.stringify(got)}\`);
  if (!got.ok) process.exit(1);
  const opened = await openBundle({ file: got.file, platform: deps.platform }, { open: async () => "", reveal: file => console.log(\`revealed \${file}\`) });
  console.log(\`open: \${JSON.stringify(opened)}\`);
  if (!opened.ok) process.exit(1);
  quit();
}
EOF
cat >"$work/driver/tsup.config.mjs" <<EOF
import { skillText } from "$REPO/packages/host/skill-text.js";
import { buildDefines } from "$REPO/packages/wspx/scripts/build-defines.mjs";
export default { entry: { driver: "driver.ts" }, format: ["esm"], outDir: "out", platform: "node", target: "node22", external: ["electron", "bufferutil", "utf-8-validate"], define: buildDefines(), esbuildPlugins: [skillText()], outExtension: () => ({ js: ".mjs" }), banner: { js: 'import { createRequire as __createRequire } from "node:module"; const require = __createRequire(import.meta.url);' } };
EOF
(cd "$work/driver" && "$REPO/node_modules/.bin/tsup" --config tsup.config.mjs >"$work/driver/build.log" 2>&1) || { cat "$work/driver/build.log"; fail "today's update code did not bundle"; }
say "bundled today's update code into $work/driver/out/driver.mjs"

# shellcheck disable=SC2016
"$node_bin" --input-type=module -e '
  import { mkdirSync, writeFileSync } from "node:fs";
  import { pathToFileURL } from "node:url";
  const [repo, home, state] = process.argv.slice(1);
  const { seededState } = await import(pathToFileURL(`${repo}/apps/desktop-gpui/bench/seed.mjs`).href);
  const { STATE_SHAPE, DAEMON_VERSION } = await import(pathToFileURL(`${repo}/packages/protocol/dist/index.js`).href);
  const seeded = seededState({ threads: 3, workspaces: 1, dir: home });
  for (const project of Object.values(seeded.projects)) mkdirSync(project.path, { recursive: true });
  const shape = { shape: STATE_SHAPE, wsp: "move-live", daemon: DAEMON_VERSION, bin: "move-live.sh", at: new Date().toISOString() };
  writeFileSync(state, `${JSON.stringify({ $shape: shape, ...seeded })}\n`);
' "$REPO" "$home" "$state"
seeded=$("$node_bin" -e 'const s = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8")); console.log(Object.values(s.sessions).flatMap(w => Object.keys(w.threads)).sort().join(" "))' "$state")
say "seeded $state with threads $seeded"
# Today's switch, as a person who tried the GPUI app through it leaves it.
mkdir -p "$home/.wsp/desktop"
: >"$home/.wsp/desktop/experimental"

if [ "$os" = Linux ]; then
  [ ! -e "$unit_link" ] && [ ! -L "$unit_link" ] || refuse "$unit_link is there already; another run holds this unit"
  mkdir -p "$(dirname "$unit_link")"
  ln -s "$unit" "$unit_link"
  linked=1
  today_env=("${base_env[@]}")
  today_cmd=("$installed")
  # Chromium will not run as root with its sandbox on.
  [ "$uid" != 0 ] || today_cmd+=(--no-sandbox)
else
  today_env=("${base_env[@]}" WSP_DESKTOP_SMOKE=1)
  today_cmd=("$installed/Contents/MacOS/wsp")
fi
today_pid=$(cd "$home" && spawn "$work/today.log" env -i "${today_env[@]}" "${today_cmd[@]}")
say "launched today's app $old_v as pid $today_pid"

until_host() {
  local t=0 pid
  while [ "$t" -lt "$1" ]; do
    pid=$(lock_pid)
    if alive "$pid"; then
      case " $2 " in *" $pid "*) ;; *) echo "$pid"; return 0 ;; esac
    fi
    sleep 1
    t=$((t + 1))
  done
  return 1
}
t=0
until [ -f "$unit" ]; do
  alive "$today_pid" || fail "today's app exited before it wrote the unit $unit: $(tail -n 5 "$work/today.log")"
  [ "$t" -lt 180 ] || fail "today's app wrote no unit at $unit within 180 s"
  sleep 1
  t=$((t + 1))
done
first=$(until_host 30 "" || true)
hosts="$hosts $first"
# The manager's HOME is not the temp one, and the unit binds wsp's default port, which another host may hold.
if [ "$os" = Linux ]; then
  awk -v home="$home" '
    /^ExecStart=/ { gsub(/\047--port\047 \047[0-9]+\047/, "\047--port\047 \0470\047") }
    { print }
    $0 == "[Service]" { print "Environment=HOME=" home }
  ' "$unit" >"$unit.part"
  mv "$unit.part" "$unit"
  systemctl --user daemon-reload
  systemctl --user restart "$unit_name"
else
  plutil -extract EnvironmentVariables xml1 -o /dev/null "$unit" 2>/dev/null || plutil -insert EnvironmentVariables -xml '<dict/>' "$unit"
  plutil -replace EnvironmentVariables.HOME -string "$home" "$unit"
  i=0
  while word=$(plutil -extract "ProgramArguments.$i" raw -o - "$unit" 2>/dev/null); do
    if [ "$word" = --port ]; then plutil -replace "ProgramArguments.$((i + 1))" -string 0 "$unit"; fi
    i=$((i + 1))
  done
  launchctl bootout "gui/$uid/$unit_name" 2>/dev/null || true
  n=0
  until launchctl bootstrap "gui/$uid" "$unit" 2>/dev/null; do
    n=$((n + 1)); [ "$n" -lt 20 ] || fail "launchctl bootstrap gui/$uid $unit failed 20 times"
    sleep 0.5
  done
fi
old_host=$(until_host 60 "$hosts") || fail "no host came back on the patched unit within 60 s"
hosts="$hosts $old_host"
say "today's host is pid $old_host: $(args_of "$old_host" | cut -c1-200)"
echo "== the wsp command today's app wrote"
sed 's/^/  /' "$wsp"
# Whether the service starts at login, as the manager keeps it apart from the unit file.
login_start() {
  if [ "$os" = Linux ]; then systemctl --user is-enabled "$unit_name" 2>/dev/null || true; else launchctl print-disabled "gui/$uid" 2>/dev/null | grep -F "\"$unit_name\"" || echo "never switched"; fi
}
login_before=$(login_start)
threads_now() { wsp_line threads --json --state "$state" 2>/dev/null | "$node_bin" -e 'let t = ""; process.stdin.on("data", d => (t += d)).on("end", () => console.log(t.split("\n").filter(Boolean).map(l => JSON.parse(l)).flatMap(r => r.threads ?? [r]).map(t => t.threadId ?? t.id).filter(Boolean).sort().join(" ")))'; }
before=$(threads_now)
say "today's host lists threads: $before"
[ -n "$before" ] || fail "today's host lists no threads"

# Today's Electron main process: the one under the AppImage this run started that runs the app's binary.
electron_main() {
  local pid
  for pid in $(tree "$today_pid"); do
    case "$(args_of "$pid")" in /tmp/.mount_*/wsp|"/tmp/.mount_"*"/wsp --no-sandbox") echo "$pid"; return ;; esac
  done
  echo "$today_pid"
}

# A wsp.app at $1 whose Info.plist names the executable $2 at version $3, which runs the lines $4.
bundle_at() {
  mkdir -p "$1/Contents/MacOS"
  printf '<plist version="1.0">\n<dict>\n  <key>CFBundleExecutable</key>\n  <string>%s</string>\n  <key>CFBundleShortVersionString</key>\n  <string>%s</string>\n</dict>\n</plist>\n' "$2" "$3" >"$1/Contents/Info.plist"
  printf '#!/bin/sh\n%s\n' "$4" >"$1/Contents/MacOS/$2"
  chmod 755 "$1/Contents/MacOS/$2"
}

if [ "$scenario" = swap ]; then
  echo "== today's swap moves the GPUI app $new_v into today's place"
  apps=$work/Applications
  bundle_at "$apps/wsp.app" wsp "$old_v" 'exit 0'
  cp "$gpui_app" "$apps/wsp-gpui.AppImage"
  chmod 755 "$apps/wsp-gpui.AppImage"
  stage=$apps/.wsp-update/stage-live
  bundle_at "$stage/new/wsp.app" desktop-gpui "$new_v" "exec '$apps/wsp-gpui.AppImage' \"\$@\""
  chmod 700 "$apps/.wsp-update" "$stage"
  fakes=$work/fakes
  mkdir -p "$fakes"
  # shellcheck disable=SC2016
  printf '#!/bin/sh\nsed -n "/<key>$2<\\/key>/{n;s/.*<string>\\(.*\\)<\\/string>.*/\\1/p;}" "$6"\n' >"$fakes/plutil"
  printf '#!/bin/sh\nexit 0\n' >"$fakes/codesign"
  printf '#!/bin/sh\necho "open $*" >>"%s"\n' "$work/open.calls" >"$fakes/open"
  # shellcheck disable=SC2016
  printf '#!/bin/sh\n[ "$1 $2" = "kickstart -k" ] || exit 1\nexec systemctl --user restart %s\n' "$unit_name" >"$fakes/launchctl"
  chmod 755 "$fakes"/*
  # shellcheck disable=SC2016
  "$node_bin" -e '
    const fs = require("node:fs");
    const [src, fakes, out] = process.argv.slice(1);
    const text = fs.readFileSync(src, "utf8");
    const from = text.indexOf("export const SWAP_SCRIPT = `") + 28;
    let script = text.slice(from, text.indexOf("`;", from)).replaceAll("\\${", "${");
    for (const tool of ["/usr/bin/plutil", "/usr/bin/codesign", "/bin/launchctl", "/usr/bin/open"]) script = script.replaceAll(tool, `${fakes}/${tool.split("/").pop()}`);
    fs.writeFileSync(out, script);
  ' "$REPO/apps/desktop/src/self-update.ts" "$fakes" "$work/swap.sh"
  electron=$(electron_main)
  mv "$home/.wsp/app" "$work/today-files-gone"
  log=$home/.wsp/desktop/update.log
  (cd "$home" && env -i "${base_env[@]}" USER=root LOGNAME=root setsid /bin/sh -c "$(cat "$work/swap.sh")" wsp-update "$electron" "$apps/wsp.app" "$apps/.wsp-update" "$stage" "$stage/new/wsp.app" desktop-gpui "$new_v" "gui/$uid/$unit_name" 60 10 "$log" </dev/null >/dev/null 2>&1 &)
  kill "$electron"
  t=0
  until grep -q -e ' is up$' -e 'putting the old app back' -e 'nothing was changed' "$log" 2>/dev/null; do
    [ "$t" -lt 120 ] || fail "today's swap said nothing within 120 s"
    sleep 1
    t=$((t + 1))
  done
  echo "== today's update log"
  sed 's/^/  /' "$log"
  grep -q ' is up$' "$log" || fail "today's swap did not keep the GPUI app"
  [ ! -e "$apps/.wsp-update" ] || fail "today's swap left $apps/.wsp-update"
  [ -x "$apps/wsp.app/Contents/MacOS/desktop-gpui" ] || fail "the GPUI app does not stand in today's place"
  n=0
  while alive "$today_pid"; do n=$((n + 1)); [ "$n" -lt 100 ] || fail "today's app did not quit"; sleep 0.1; done
  today_pid=
  gpui_pid=$(sed -n 's/.* as pid \([0-9]*\)$/\1/p' "$log" | tail -n 1)
  gpui_files=$home/.wsp/gpui-app
  got=$apps/wsp-gpui.AppImage
elif [ "$os" = Linux ]; then
  echo "== today's update gets $new_v from the feed and opens it"
  electron=$(electron_main)
  (cd "$home" && env -i "${base_env[@]}" WSP_RELEASE_API="$api" "$node_bin" "$work/driver/out/driver.mjs" linux "$home/Downloads" "$old_v" "$new_v" "$electron") 2>&1 | sed 's/^/  /' || fail "today's update did not get and open $new_v"
  n=0
  while alive "$today_pid"; do n=$((n + 1)); [ "$n" -lt 300 ] || fail "today's app did not quit after the update opened"; sleep 0.1; done
  today_pid=
  got=$home/Downloads/$asset
  [ -x "$got" ] && [ "$(sha "$got")" = "$(sha "$work/feed/$asset")" ] || fail "$got is not the feed's $asset, runnable"
  say "today's app kept $got (sha256 matches, mode $(stat -c %a "$got")) and quit; the person opens it"
  gpui_pid=$(cd "$home" && spawn "$work/gpui.log" env -i "${base_env[@]}" "$got")
  gpui_files=$home/.wsp/gpui-app
else
  echo "== today's update gets $new_v from the feed and swaps it in"
  (cd "$home" && env -i "${base_env[@]}" WSP_RELEASE_API="$api" "$node_bin" "$work/driver/out/driver.mjs" darwin "$work" "$old_v" "$new_v" "$today_pid" "$installed" "gui/$uid/$unit_name" "$home/.wsp/desktop/update.log") 2>&1 | sed 's/^/  /' || fail "today's update did not get and open $new_v"
  t=0
  until grep -q -e ' is up$' -e 'putting the old app back' -e 'nothing was changed' "$home/.wsp/desktop/update.log" 2>/dev/null; do
    [ "$t" -lt 180 ] || fail "today's swap said nothing within 180 s"
    sleep 1
    t=$((t + 1))
  done
  echo "== today's update log"
  sed 's/^/  /' "$home/.wsp/desktop/update.log"
  grep -q ' is up$' "$home/.wsp/desktop/update.log" || fail "today's swap did not keep the GPUI app"
  today_pid=
  gpui_pid=$(sed -n 's/.* as pid \([0-9]*\)$/\1/p' "$home/.wsp/desktop/update.log" | tail -n 1)
  gpui_files=$installed/Contents
fi
say "the GPUI app runs as pid $gpui_pid"

new_host=$(until_host 120 "$hosts") || fail "no host came up on the GPUI app's files within 120 s"
hosts="$hosts $new_host"
sleep 3
[ "$(lock_pid)" = "$new_host" ] || fail "the host changed again after pid $new_host: $(lock_pid)"
new_args=$(args_of "$new_host")
say "the host is pid $new_host: $(printf '%s' "$new_args" | cut -c1-200)"
after=$(threads_now)
echo "== the wsp command now"
sed 's/^/  /' "$wsp"
why=
case "$new_args" in "$gpui_files"/*node*) ;; *) why="$why; the host does not run the GPUI app's Node under $gpui_files" ;; esac
grep -q "$gpui_files/" "$wsp" || why="$why; the wsp command does not run the GPUI app's files"
grep -q 'ELECTRON_RUN_AS_NODE' "$wsp" && why="$why; the wsp command still runs today's app"
alive "$old_host" && why="$why; today's host (pid $old_host) still runs"
[ "$after" = "$seeded" ] || why="$why; the host lists threads [$after], not the seeded [$seeded]"
up=$(ps -A -o pid= -o args= | awk -v s="$state" 'index($0, " up ") && index($0, s) && index($0, "cli.mjs") && !index($0, "forward") { print $1 }' | sort -u | tr '\n' ' ')
[ "$up" = "$new_host " ] || why="$why; the processes serving $state are [$up], not the one host $new_host"
if [ "$os" = Linux ]; then
  exec_start=$(grep '^ExecStart=' "$unit")
  case "$exec_start" in "ExecStart=$wsp "*|"ExecStart='$wsp' "*) ;; *) why="$why; the unit runs $exec_start, not the wsp command" ;; esac
else
  [ "$(plutil -extract ProgramArguments.0 raw -o - "$unit")" = "$wsp" ] || why="$why; the unit does not run the wsp command"
fi
[ -e "$home/.wsp/desktop/experimental" ] && why="$why; today's switch still stands"
login_after=$(login_start)
say "starts at login: $login_before before the move, $login_after after"
[ "$login_after" = "$login_before" ] || why="$why; the service's login setting moved from $login_before to $login_after"
echo "== the GPUI app's launch lines"
grep -h -e 'wsp command' -e 'switch' -e 'wsp://' -e 'restart' -e 'update:' "$home"/.wsp/logs/app*.log 2>/dev/null | sed 's/^/  /' | tail -n 20 || true

if [ "$os" = Linux ]; then
  link_to=$(printf '%s' "$seeded" | cut -d' ' -f1)
  say "xdg-mime names $(env -i "${base_env[@]}" /usr/bin/xdg-mime query default x-scheme-handler/wsp) for wsp://; opening wsp://thread/$link_to"
  sed 's/^/  /' "$home/.local/share/applications/wsp.desktop" || why="$why; no wsp.desktop entry was written"
  before_link=$(ps -A -o pid= | wc -l)
  (cd "$home" && env -i "${base_env[@]}" /usr/bin/xdg-open "wsp://thread/$link_to") >"$work/xdg-open.log" 2>&1 || why="$why; xdg-open failed: $(cat "$work/xdg-open.log")"
  sleep 4
  alive "$gpui_pid" || why="$why; the GPUI app is gone after the link"
  second=$(ps -A -o pid= -o args= | awk -v l="wsp://thread/$link_to" 'index($0, l) && !index($0, "awk") { print $1 }')
  [ -z "$second" ] || why="$why; the launch the link started is still running (pid $second), not handed to the first"
  say "the launch xdg-open started handed the link to pid $gpui_pid and exited ($before_link processes before)"
  if command -v import >/dev/null; then import -display "$DISPLAY" -window root "$work/after-link.png" && say "screenshot $work/after-link.png"; fi
fi

[ -z "$why" ] || fail "${why#; }"
result=PASS
echo "PASS: today's app $old_v moved to the GPUI app $new_v through its own update: one host (pid $new_host) serves the same $(echo "$after" | wc -w) threads on the GPUI app's Node, the wsp command and the unit run its files, and today's switch is gone$([ "$os" = Linux ] && echo "; a wsp:// link reached the running GPUI app")"
