#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# usage: update-live.sh <old-app-binary> <old-version> <new-app-binary> <new-version> <host-app-dir>
#        update-live.sh --check-node <node>   the check each packed Node passes: it links no library outside the system
# Proves on Linux or macOS that the GPUI app one release behind updates itself from a local release feed and that its
# host and daemon follow onto the new bundle, in a temp folder with a temp HOME. The binaries are debug builds of
# apps/desktop-gpui at the two versions (only a debug build reads WSP_RELEASE_API and WSP_UPDATE_RESTART_WHEN_READY,
# which presses Restart once the update is checked). <host-app-dir> is apps/desktop's build/app
# (pnpm --filter @wsp/desktop run build:app) with this platform's daemon under assets/daemon/<triple>/wsp-daemon.
# Each version is packed as scripts/package.sh packs it (host_stage): the app and Node in usr/bin or Contents/MacOS,
# the host under usr/lib/wsp/app or Contents/Resources/app. The Node packed is the official one package.sh pins, never
# the one on PATH, which runs this script's own helpers.
# The host runs as a real service in the temp HOME, installed by the old app as the product installs it and patched
# to carry WSP_RELEASE_API and the temp HOME: on Linux a systemd user unit wsp-host-<tag of the temp state>.service,
# which root's user manager loads through a link in $XDG_RUNTIME_DIR/systemd/user; on a Mac a launchd job labelled
# com.wsp.host.<tag> in the person's own gui domain. wsp down removes it at the end, and the script checks it went.
# REPO=<checkout> names the repo whose bench seed and protocol build seed the state (default: the one holding this
# script). On Linux, DISPLAY must name a running X server. KEEP_WORK=1 keeps the work folder after a pass; a failed
# run always keeps it. The feed names the asset as GPUI_NAMES in packages/protocol/src/bundles.mjs does.
# SCENARIO picks what the feed offers (Linux alone takes the last two):
#   good      the new version, which the old app swaps in, its host and daemon following (the default)
#   electron  today's Electron AppImage under the GPUI app's name: refused off its squashfs, downloaded once across a
#             relaunch, nothing replaced
#   refused   the new version carrying a host of the old release, so the host is left as it is and the new app does
#             not stay: the swap puts the old app back, which says why
#   crash     the new version, whose app is killed once it has moved the host onto its files and before it is up: the
#             swap puts the old app back, which moves the host back onto its own files
set -eu
umask 022

usage='usage: update-live.sh <old-app-binary> <old-version> <new-app-binary> <new-version> <host-app-dir>'
os=$(uname -s)
refuse() { echo "refused: $*" >&2; exit 2; }

# Refuses a Node that links a library outside the system (Homebrew's links its libnode), which a bundle does not carry.
node_alone() {
  local node=$1 out outside
  if [ "$os" = Darwin ]; then
    out=$(otool -L "$node") || refuse "otool -L could not read $node"
    outside=$(printf '%s\n' "$out" | awk 'NR > 1 && NF && !/:$/ && $1 !~ /^\/(usr\/lib|System\/Library)\// { printf "%s%s", s, $1; s = ", " }')
  else
    out=$(ldd "$node") || refuse "ldd could not read $node"
    outside=$(printf '%s\n' "$out" | awk '
      NF { lib = ($2 == "=>") ? $3 : $1 }
      NF && lib !~ /^(linux-vdso|linux-gate)\.so\./ && lib !~ /^\/(usr\/)?lib(64)?\// { printf "%s%s", s, ($2 == "=>") ? $1 " => " ($3 == "not" ? "not found" : $3) : $1; s = ", " }')
  fi
  [ -z "$outside" ] || refuse "$node links $outside, outside the system; its copy in a bundle would not start, so it is not packed"
}
if [ "${1-}" = --check-node ]; then
  [ $# -eq 2 ] || { echo "usage: update-live.sh --check-node <node>" >&2; exit 2; }
  node_alone "$2"
  echo "ok: $2 links nothing outside the system"
  exit 0
fi

[ $# -eq 5 ] || { echo "$usage" >&2; exit 2; }
abs() { (cd "$(dirname "$1")" && printf '%s/%s\n' "$(pwd -P)" "$(basename "$1")"); }
old_bin=$(abs "$1")
old_v=$2
new_bin=$(abs "$3")
new_v=$4
host_app=$(cd "$5" && pwd -P)
here=$(cd "$(dirname "$0")" && pwd -P)
REPO=${REPO:-$(cd "$here/../../.." && pwd -P)}
uid=$(id -u)
scenario=${SCENARIO:-good}

case "$os" in
  Linux) triple=$(uname -m)-unknown-linux-musl; node_pa=linux-$(uname -m | sed 's/^x86_64$/x64/; s/^aarch64$/arm64/') ;;
  Darwin) case "$(uname -m)" in arm64) triple=aarch64-apple-darwin; node_pa=darwin-arm64 ;; *) triple=$(uname -m)-apple-darwin; node_pa=darwin-x64 ;; esac ;;
  *) refuse "this runs on Linux or macOS, not $os" ;;
esac
case "$scenario" in
  good) ;;
  electron|refused|crash) [ "$os" = Linux ] || refuse "SCENARIO=$scenario runs on Linux alone" ;;
  *) refuse "SCENARIO takes good, electron, refused or crash, not $scenario" ;;
esac
for v in "$old_v" "$new_v"; do
  case "$v" in [0-9]*.[0-9]*.[0-9]*) ;; *) refuse "$v is not a version like 1.2.3" ;; esac
done
for b in "$old_bin" "$new_bin"; do [ -f "$b" ] && [ -x "$b" ] || refuse "$b is not an executable file"; done
[ -f "$host_app/main/cli.mjs" ] && [ -f "$host_app/package.json" ] || refuse "$host_app is not apps/desktop's build/app (no main/cli.mjs or package.json)"
[ -x "$host_app/assets/daemon/$triple/wsp-daemon" ] || refuse "$host_app carries no daemon at assets/daemon/$triple/wsp-daemon"
[ -f "$REPO/apps/desktop-gpui/bench/seed.mjs" ] || refuse "set REPO to a wsp checkout (it reads apps/desktop-gpui/bench/seed.mjs)"
[ -f "$REPO/packages/protocol/dist/index.js" ] || refuse "$REPO/packages/protocol is not built (pnpm --filter @wsp/protocol build)"
node_bin=$(command -v node) || refuse "node is not on PATH"
if [ "$os" = Linux ]; then
  [ -n "${DISPLAY-}" ] || refuse "DISPLAY is not set; start an X server (Xvfb) and set it"
  command -v mksquashfs >/dev/null || refuse "mksquashfs is missing: apt-get install squashfs-tools"
  [ -f "$REPO/apps/desktop/build/icon.ico" ] || refuse "$REPO/apps/desktop/build/icon.ico is missing"
  XDG_RUNTIME_DIR=/run/user/$uid
  DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus
  export XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS
  # degraded is a running manager with a failed unit somewhere, which loads units all the same.
  case "$(systemctl --user is-system-running 2>/dev/null || true)" in
    running|degraded) ;;
    *) refuse "systemctl --user is-system-running fails: this account's systemd user manager is not up at $XDG_RUNTIME_DIR, and the host runs as its unit" ;;
  esac
  lang=${LANG:-C.UTF-8}
else
  lang=${LANG:-en_US.UTF-8}
fi

sha() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi | cut -d' ' -f1; }

work=$(mktemp -d "${TMPDIR:-/tmp}/update-live.XXXXXX")
work=$(cd "$work" && pwd -P)
home=$work/home
state=$home/.wsp/state.json
lock=$home/.wsp/host.lock
wsp=$home/.wsp/bin/wsp
stage_dir=$work/install/.wsp-gpui-update
tag=$(printf '%s' "$state" | sha | cut -c1-8)
if [ "$os" = Linux ]; then
  installed=$work/install/wsp.AppImage
  unit_name=wsp-host-$tag.service
  unit=$home/.config/systemd/user/$unit_name
  unit_link=$XDG_RUNTIME_DIR/systemd/user/$unit_name
else
  installed=$work/install/wsp.app
  unit_name=com.wsp.host.$tag
  unit=$home/Library/LaunchAgents/$unit_name.plist
  unit_link=
fi
mkdir -p "$home/.wsp" "$work/install" "$work/feed" "$work/pack"

feed_pid=
app_old=
hosts=
kids=
result=
linked=

say() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
args_of() { ps -o args= -p "$1" 2>/dev/null || true; }
exe_of() {
  if [ "$os" = Linux ]; then readlink "/proc/$1/exe" 2>/dev/null || echo '?'; else ps -o comm= -p "$1" 2>/dev/null || echo '?'; fi
}
children() { ps -A -o pid= -o ppid= | awk -v p="$1" '$2 == p { print $1 }'; }
tree() {
  local kid
  for kid in $(children "$1"); do echo "$kid"; tree "$kid"; done
}
lock_pid() { sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$lock" 2>/dev/null || true; }
lock_by() { sed -n 's/.*"startedBy":"\([a-z]*\)".*/\1/p' "$lock" 2>/dev/null || true; }
# Starts a command detached from this shell (so it is never this script's zombie) and prints its pid.
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
# The environment every wsp line and the app's own service calls need: the temp HOME and, on Linux, the user manager.
wsp_env() {
  if [ "$os" = Linux ]; then
    env -i HOME="$home" PATH="$PATH" LANG="$lang" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" "$@"
  else
    env -i HOME="$home" PATH="$PATH" LANG="$lang" "$@"
  fi
}
# Whether a pid runs the installed app: on a Mac its executable inside install/wsp.app; on Linux the app binary inside
# an AppImage mount of a file named wsp.AppImage.
runs_installed() {
  local cmd mount
  cmd=$(args_of "$1")
  if [ "$os" = Darwin ]; then
    case "$cmd" in "$installed/Contents/MacOS/desktop-gpui"*) return 0 ;; esac
    return 1
  fi
  case "$cmd" in /tmp/.mount_*/usr/bin/desktop-gpui*) ;; *) return 1 ;; esac
  mount=${cmd%%/usr/bin/desktop-gpui*}
  grep -q " $mount .* fuse.wsp.AppImage " /proc/self/mountinfo
}
new_app_pid() { sed -n 's/.* opened wsp .* as pid \([0-9]*\)$/\1/p' "$home/.wsp/logs/update.log" 2>/dev/null | tail -n 1; }
# The mount points of AppImages named wsp.AppImage, one per line.
wsp_mounts() {
  [ "$os" = Linux ] || return 0
  awk '{ for (i = 7; i <= NF; i++) if ($i == "-") { if ($(i + 1) == "fuse.wsp.AppImage") print $5; break } }' /proc/self/mountinfo
}
mounts_before=$(wsp_mounts)
# Every process running this run's installed app: on a Mac its executable under install/ (the swap reopens the old
# app with open, which names no pid), on Linux the app binary inside a wsp.AppImage mount made during this run.
installed_apps() {
  local m
  if [ "$os" = Darwin ]; then
    ps -A -o pid= -o args= | awk -v w="$work/install/" '
      index($2, w) == 1 && substr($2, length($2) - 27) == "/Contents/MacOS/desktop-gpui" { print $1 }'
    return 0
  fi
  for m in $(wsp_mounts); do
    case "
$mounts_before
" in *"
$m
"*) continue ;; esac
    ps -A -o pid= -o args= | awk -v p="$m/usr/bin/desktop-gpui" '$2 == p { print $1 }'
  done
}
# Whether the manager still has the unit: loaded on Linux, in the gui domain on a Mac.
unit_loaded() {
  if [ "$os" = Linux ]; then
    [ "$(systemctl --user show "$unit_name" --property=LoadState --value 2>/dev/null)" = loaded ]
  else
    launchctl print "gui/$uid/$unit_name" >/dev/null 2>&1
  fi
}

cleanup() {
  local code=$? pid n
  trap - EXIT HUP INT TERM
  set +e
  echo "== cleanup"
  # The swap first, so it opens no app after the apps are stopped.
  pid=$(cat "$stage_dir"/opening 2>/dev/null | head -n 1)
  if alive "$pid"; then
    case "$(args_of "$pid")" in *wsp-update*) stop "$pid" "swap script" ;; esac
  fi
  # The apps before the service, so none of them starts the host again.
  for pid in $(installed_apps) $app_old; do stop "$pid" app; done
  pid=$(lock_pid)
  if alive "$pid"; then kids="$kids $(tree "$pid")"; fi
  if [ -x "$wsp" ]; then
    say "wsp down --state $state"
    (cd "$home" && wsp_env "$wsp" down --state "$state" 2>&1 | sed 's/^/  /')
  fi
  if [ -e "$unit" ] || unit_loaded; then
    say "the unit is still there after wsp down; taking it away"
    if [ "$os" = Linux ]; then
      systemctl --user disable --now "$unit_name" 2>&1 | sed 's/^/  /'
    else
      launchctl bootout "gui/$uid/$unit_name" 2>&1 | sed 's/^/  /'
    fi
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
  for pid in $kids; do stop "$pid" "child of a host"; done
  stop "$feed_pid" feed
  # Each AppImage's own runtime process unmounts and exits once its app is gone.
  n=0
  # shellcheck disable=SC2009
  while [ "$n" -lt 50 ] && ps -A -o args= | grep -qxF "$installed"; do sleep 0.1; n=$((n + 1)); done
  echo "== left running (processes naming $work or running from a mount this run made)"
  # shellcheck disable=SC2009
  ps -A -o pid= -o args= | grep -F -e "$work" | grep -v -e 'grep -F' || echo "  nothing"
  for pid in $(installed_apps); do echo "  $pid $(args_of "$pid")"; done
  if [ "$result" = PASS ] && [ "${KEEP_WORK-}" != 1 ]; then rm -rf "$work"; else echo "kept $work"; fi
  exit "$code"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

# Sets "version" in the host's package.json and in the copy of the command under assets/cli it hands to machines.
set_version() {
  "$node_bin" -e '
    const fs = require("node:fs");
    const [version, ...files] = process.argv.slice(1);
    for (const file of files) {
      if (!fs.existsSync(file)) continue;
      const pkg = JSON.parse(fs.readFileSync(file, "utf8"));
      pkg.version = version;
      fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + "\n");
    }
  ' "$1" "$2/package.json" "$2/assets/cli/package.json"
}

# The release asset GPUI_NAMES in bundles.mjs names under key $1 for version $2.
# shellcheck disable=SC2016
gpui_asset() {
  "$node_bin" --input-type=module -e '
    import { pathToFileURL } from "node:url";
    const [repo, key, version] = process.argv.slice(1);
    const { GPUI_NAMES } = await import(pathToFileURL(`${repo}/packages/protocol/src/bundles.mjs`).href);
    console.log(GPUI_NAMES[key].replace("{version}", version));
  ' "$REPO" "$1" "$2"
}

# package.sh's host_stage: app/package.json, app/main/*.mjs but main.mjs, app/assets, readable by everyone.
host_stage() {
  local res=$1 version=$2 f
  mkdir -p "$res/app/main"
  cp "$host_app/package.json" "$res/app/package.json"
  for f in "$host_app"/main/*.mjs; do
    [ "$(basename "$f")" = main.mjs ] || cp "$f" "$res/app/main/"
  done
  cp -R "$host_app/assets" "$res/app/assets"
  set_version "$version" "$res/app"
  chmod -R u+rwX,go+rX-w "$res/app"
}

# The 256 px PNG in apps/desktop/build/icon.ico, as package.sh's icon_png reads it.
icon_png() {
  local ico=$REPO/apps/desktop/build/icon.ico count i at width size offset
  count=$(od -An -t u2 -j 4 -N 2 "$ico" | tr -d ' ')
  i=0
  while [ "$i" -lt "$count" ]; do
    at=$((6 + 16 * i))
    width=$(od -An -t u1 -j "$at" -N 1 "$ico" | tr -d ' ')
    if [ "$width" = 0 ]; then
      size=$(od -An -t u4 -j $((at + 8)) -N 4 "$ico" | tr -d ' ')
      offset=$(od -An -t u4 -j $((at + 12)) -N 4 "$ico" | tr -d ' ')
      dd if="$ico" bs=1 skip="$offset" count="$size" 2>/dev/null
      return
    fi
    i=$((i + 1))
  done
  refuse "$ico holds no 256 px picture"
}

# package.sh's pinned type2 runtime, kept beside this script once its sha256 matches.
runtime=$here/.appimage/runtime-x86_64-20251108
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sum=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
fetch_runtime() {
  if [ -f "$runtime" ] && [ "$(sha "$runtime")" = "$runtime_sum" ]; then return; fi
  mkdir -p "$(dirname "$runtime")"
  curl -fsSL "$runtime_url" -o "$runtime.part"
  if [ "$(sha "$runtime.part")" != "$runtime_sum" ]; then
    rm -f "$runtime.part"
    refuse "the AppImage runtime at $runtime_url does not match its sha256 $runtime_sum"
  fi
  mv "$runtime.part" "$runtime"
}

# package.sh's pinned node_version and node_sum, read from it so the Node packed here is the one a release carries.
eval "$(sed -n '/^node_version=/,/^}/p' "$here/package.sh")"
[ -n "${node_version-}" ] && [ "$(type -t node_sum)" = function ] || refuse "$here/package.sh no longer defines node_version and node_sum"
# The official Node every bundle carries, out of package.sh's vendor/.node or fetched, unpacked into the work folder.
fetch_node() {
  local sum name cache archive
  sum=$(node_sum "$node_pa")
  [ -n "$sum" ] || refuse "package.sh pins no Node for $node_pa, so there is no official Node to pack"
  name=node-v$node_version-$node_pa
  # Making vendor/ where there is none would let package.sh skip gpui-patches.sh apply.
  if [ -d "$here/../vendor" ]; then cache=$here/../vendor/.node; else cache=$work/pack; fi
  archive=$cache/$name.tar.gz
  if ! { [ -f "$archive" ] && [ "$(sha "$archive")" = "$sum" ]; }; then
    mkdir -p "$cache"
    curl -fsSL "https://nodejs.org/dist/v$node_version/$name.tar.gz" -o "$archive.part"
    if [ "$(sha "$archive.part")" != "$sum" ]; then
      rm -f "$archive.part"
      refuse "Node $node_version for $node_pa does not match its sha256 $sum"
    fi
    mv "$archive.part" "$archive"
  fi
  mkdir -p "$work/node"
  tar -xzf "$archive" -C "$work/node" --strip-components 1 "$name/bin/node"
  pack_node=$work/node/bin/node
  node_alone "$pack_node"
  say "packing the official Node $node_version for $node_pa"
}

# An AppImage laid out as package.sh linux() writes it; $4, where given, is the release its host says it is.
pack_linux() {
  local bin=$1 version=$2 out=$3 appdir=$work/pack/AppDir-$2
  mkdir -p "$appdir/usr/bin" "$appdir/usr/lib/wsp" "$appdir/usr/share/icons/hicolor/256x256/apps"
  install -m 755 "$bin" "$appdir/usr/bin/desktop-gpui"
  install -m 755 "$pack_node" "$appdir/usr/bin/node"
  host_stage "$appdir/usr/lib/wsp" "${4:-$version}"
  if [ "${5-}" = crash ]; then
    # The app killed once the stage says the host is moving and the host lock names a new pid, before it can say up.
    cat >"$appdir/AppRun" <<'EOF'
#!/bin/sh
here=${APPDIR:-$(dirname "$(readlink -f "$0")")}
"$here/usr/bin/desktop-gpui" "$@" &
app=$!
stage=$(dirname "$APPIMAGE")/.wsp-gpui-update
pid_of() { sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$HOME/.wsp/host.lock" 2>/dev/null; }
first=$(pid_of)
n=0
while [ "$n" -lt 900 ]; do
  now=$(pid_of)
  if [ -e "$stage/restarted" ] && [ -n "$now" ] && [ "$now" != "$first" ]; then break; fi
  sleep 0.05
  n=$((n + 1))
done
kill -9 "$app"
EOF
  else
    cat >"$appdir/AppRun" <<'EOF'
#!/bin/sh
here=${APPDIR:-$(dirname "$(readlink -f "$0")")}
exec "$here/usr/bin/desktop-gpui" "$@"
EOF
  fi
  chmod 755 "$appdir/AppRun"
  cat >"$appdir/wsp-experimental.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=wsp Experimental
Comment=wsp's desktop app on GPUI, Experimental
Exec=desktop-gpui
Icon=wsp-experimental
Terminal=false
Categories=Development;
X-AppImage-Version=$version
EOF
  icon_png >"$appdir/wsp-experimental.png"
  cp "$appdir/wsp-experimental.png" "$appdir/usr/share/icons/hicolor/256x256/apps/wsp-experimental.png"
  ln -s wsp-experimental.png "$appdir/.DirIcon"
  mksquashfs "$appdir" "$work/pack/app-$version.squashfs" -all-root -noappend -no-progress -quiet -no-xattrs -no-fragments
  cat "$runtime" "$work/pack/app-$version.squashfs" >"$out"
  chmod 755 "$out"
  rm -rf "$appdir" "$work/pack/app-$version.squashfs"
}

# Today's Electron AppImage as electron-builder lays one out (the app at the top, its code in resources/app.asar), on
# the same runtime, so it carries the same ELF header and AppImage marks as the GPUI app's.
pack_electron() {
  local out=$1 appdir=$work/pack/AppDir-electron
  mkdir -p "$appdir/resources" "$appdir/usr/lib"
  install -m 755 "$pack_node" "$appdir/wsp"
  printf 'asar' >"$appdir/resources/app.asar"
  printf 'so' >"$appdir/usr/lib/libffmpeg.so"
  # shellcheck disable=SC2016
  printf '#!/bin/sh\nexec "$(dirname "$0")/wsp" "$@"\n' >"$appdir/AppRun"
  chmod 755 "$appdir/AppRun"
  mksquashfs "$appdir" "$work/pack/electron.squashfs" -all-root -noappend -no-progress -quiet -no-xattrs -no-fragments
  cat "$runtime" "$work/pack/electron.squashfs" >"$out"
  chmod 755 "$out"
  rm -rf "$appdir" "$work/pack/electron.squashfs"
}

# A wsp.app laid out as package.sh mac() writes it, signed ad hoc.
pack_mac() {
  local bin=$1 version=$2 app=$3/wsp.app
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  install -m 755 "$bin" "$app/Contents/MacOS/desktop-gpui"
  install -m 755 "$pack_node" "$app/Contents/MacOS/node"
  host_stage "$app/Contents/Resources" "$version"
  cat >"$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>desktop-gpui</string>
  <key>CFBundleIdentifier</key>
  <string>wsp.desktop.update-live</string>
  <key>CFBundleName</key>
  <string>wsp</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
</dict>
</plist>
EOF
  codesign --force --deep --sign - "$app"
}

say "work folder $work"
say "packing $old_v and $new_v"
fetch_node
if [ "$os" = Linux ]; then
  asset=$(gpui_asset appImage "$new_v")
  fetch_runtime
  pack_linux "$old_bin" "$old_v" "$installed"
  case "$scenario" in
    good) pack_linux "$new_bin" "$new_v" "$work/feed/$asset" ;;
    electron) pack_electron "$work/feed/$asset" ;;
    refused) pack_linux "$new_bin" "$new_v" "$work/feed/$asset" "$old_v" ;;
    crash) pack_linux "$new_bin" "$new_v" "$work/feed/$asset" "$new_v" crash ;;
  esac
  new_sum=$(sha "$work/feed/$asset")
  old_sum=$(sha "$installed")
else
  asset=$(gpui_asset macZip "$new_v")
  pack_mac "$old_bin" "$old_v" "$work/install"
  mkdir -p "$work/pack/new"
  pack_mac "$new_bin" "$new_v" "$work/pack/new"
  (cd "$work/pack/new" && ditto -c -k --norsrc --noextattr --keepParent wsp.app "$work/feed/$asset")
fi
say "the feed offers $asset ($scenario)"
echo "$new_v" >"$work/feed/VERSION"

port=$("$node_bin" -e 'const s = require("node:net").createServer().listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });')
feed_pid=$(spawn "$work/feed.log" "$node_bin" "$here/release-feed.mjs" "$work/feed" "$port")
api=http://127.0.0.1:$port
n=0
until grep -q '^feed:' "$work/feed.log" 2>/dev/null; do
  alive "$feed_pid" || { cat "$work/feed.log" >&2; refuse "the feed did not start"; }
  n=$((n + 1)); [ "$n" -lt 100 ] || refuse "the feed did not start within 10 s"
  sleep 0.1
done
say "feed (pid $feed_pid) offers $new_v at $api"

# The state the bench seeds (bench/fixture.mjs with BENCH_SEED): projects, a workspace and settled threads, so the app
# opens its main window and not onboarding.
# shellcheck disable=SC2016
"$node_bin" --input-type=module -e '
  import { mkdirSync, writeFileSync } from "node:fs";
  import { pathToFileURL } from "node:url";
  const [repo, home, state] = process.argv.slice(1);
  const { seededState } = await import(pathToFileURL(`${repo}/apps/desktop-gpui/bench/seed.mjs`).href);
  const { STATE_SHAPE, DAEMON_VERSION } = await import(pathToFileURL(`${repo}/packages/protocol/dist/index.js`).href);
  const seeded = seededState({ threads: 3, workspaces: 1, dir: home });
  for (const project of Object.values(seeded.projects)) mkdirSync(project.path, { recursive: true });
  const shape = { shape: STATE_SHAPE, wsp: "update-live", daemon: DAEMON_VERSION, bin: "update-live.sh", at: new Date().toISOString() };
  writeFileSync(state, `${JSON.stringify({ $shape: shape, ...seeded })}\n`);
' "$REPO" "$home" "$state"
say "seeded $state"

if [ "$os" = Linux ]; then
  # Root's user manager reads units from its own HOME, not the temp one; a link in its runtime folder, where the unit
  # will be written, is how it finds the unit the app's wsp up --service writes there.
  [ ! -e "$unit_link" ] && [ ! -L "$unit_link" ] || refuse "$unit_link is there already; another run holds this unit"
  mkdir -p "$(dirname "$unit_link")"
  ln -s "$unit" "$unit_link"
  linked=1
  say "linked $unit_link to $unit"
fi

# WSP_HOME names the default state's own folder, so the unit carries it and the service host, whose HOME is the
# manager's until the unit is patched, keeps its files in the temp HOME.
envs=(HOME="$home" WSP_HOME="$home/.wsp" PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG="$lang" WSP_RELEASE_API="$api" WSP_UPDATE_RESTART_WHEN_READY=1 WSP_DESKTOP_SMOKE=1)
if [ "$os" = Linux ]; then
  envs+=(DISPLAY="$DISPLAY" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS")
  launch=$installed
else
  launch=$installed/Contents/MacOS/desktop-gpui
fi
launch_old() {
  app_old=$(cd "$home" && spawn "$work/app-old.log" env -i "${envs[@]}" "$launch")
  say "launched the old app $old_v as pid $app_old"
}
launch_old

fail() {
  result=FAIL
  logs
  echo "FAIL: $*"
  exit 1
}
logged=
logs() {
  [ -z "$logged" ] || return 0
  logged=1
  echo "== feed requests"
  cat "$work/feed.log"
  echo "== $home/.wsp/logs/update.log"
  cat "$home/.wsp/logs/update.log" 2>/dev/null || echo "  (none)"
  echo "== update: and service lines of the app log"
  grep -h -e 'update:' -e 'wsp command' -e 'wsp up --service' -e 'restarting it on' "$home"/.wsp/logs/app*.log 2>/dev/null || echo "  (none)"
}

# Waits up to $1 seconds for the lock to name a live pid that is not in $2, and prints it.
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
  alive "$app_old" || fail "the old app exited before it wrote the unit $unit"
  [ "$t" -lt 120 ] || fail "the old app wrote no unit at $unit within 120 s"
  sleep 1
  t=$((t + 1))
done
# The unit's first host may not serve at all where another host holds the default port, so it is not waited on long.
first=$(until_host 15 "" || true)
hosts="$hosts $first"
say "the old app installed $unit_name; its first host is ${first:+pid $first, started by $(lock_by)}${first:-not serving within 15 s}"

# serviceEnv does not carry WSP_RELEASE_API, the manager's HOME is not the temp one, and the unit binds wsp's default
# port, which the person's own host may hold; the app reads the port off host.lock.
if [ "$os" = Linux ]; then
  awk -v api="$api" -v home="$home" '
    /^ExecStart=/ { gsub(/\047--port\047 \047[0-9]+\047/, "\047--port\047 \0470\047") }
    { print }
    $0 == "[Service]" { print "Environment=WSP_RELEASE_API=" api; print "Environment=HOME=" home }
  ' "$unit" >"$unit.part"
  mv "$unit.part" "$unit"
  echo "== patched $unit"
  grep -e '^Environment=WSP_RELEASE_API=' -e '^Environment=HOME=' -e '^ExecStart=' "$unit" | sed 's/^/  /'
  systemctl --user daemon-reload
  systemctl --user restart "$unit_name"
else
  plutil -extract EnvironmentVariables xml1 -o /dev/null "$unit" 2>/dev/null || plutil -insert EnvironmentVariables -xml '<dict/>' "$unit"
  plutil -replace EnvironmentVariables.WSP_RELEASE_API -string "$api" "$unit"
  plutil -replace EnvironmentVariables.HOME -string "$home" "$unit"
  i=0
  while word=$(plutil -extract "ProgramArguments.$i" raw -o - "$unit" 2>/dev/null); do
    if [ "$word" = --port ]; then plutil -replace "ProgramArguments.$((i + 1))" -string 0 "$unit"; fi
    i=$((i + 1))
  done
  echo "== patched $unit"
  echo "  ProgramArguments: $(i=0; while w=$(plutil -extract "ProgramArguments.$i" raw -o - "$unit" 2>/dev/null); do printf '%s ' "$w"; i=$((i + 1)); done)"
  echo "  EnvironmentVariables.WSP_RELEASE_API = $(plutil -extract EnvironmentVariables.WSP_RELEASE_API raw -o - "$unit")"
  echo "  EnvironmentVariables.HOME = $(plutil -extract EnvironmentVariables.HOME raw -o - "$unit")"
  launchctl bootout "gui/$uid/$unit_name" 2>/dev/null || true
  n=0
  until launchctl bootstrap "gui/$uid" "$unit" 2>/dev/null; do
    n=$((n + 1)); [ "$n" -lt 20 ] || fail "launchctl bootstrap gui/$uid $unit failed 20 times"
    sleep 0.5
  done
fi
old_host=$(until_host 60 "$hosts") || fail "no host came back on the patched unit within 60 s"
hosts="$hosts $old_host"
old_args=$(args_of "$old_host")
say "the patched unit's host is pid $old_host: $old_args"
old_copy=
if [ "$os" = Linux ]; then
  old_copy=$(printf '%s\n' "$old_args" | sed -n "s|.*\\($home/.wsp/gpui-app/[^/ ]*\\)/.*|\\1|p")
  [ -n "$old_copy" ] || fail "the old host does not run from a copy under $home/.wsp/gpui-app: $old_args"
fi

# The version the host at pid $1 names, off the package.json beside the command it runs.
host_version() {
  local cmd pkg
  cmd=$(args_of "$1" | awk '{ print $2 }')
  pkg=$(dirname "$(dirname "$cmd")")/package.json
  sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$pkg" 2>/dev/null | head -n 1
}
downloads() { grep -c " GET /wsp-labs/wsp/releases/download/" "$work/feed.log" 2>/dev/null || true; }
app_said() { grep -h -e "$1" "$home"/.wsp/logs/app*.log 2>/dev/null | tail -n 1; }
# Waits up to $2 seconds for the app log to say $1, failing with $3.
until_said() {
  local t=0
  until [ -n "$(app_said "$1")" ]; do
    [ "$t" -lt "$2" ] || fail "$3"
    sleep 1
    t=$((t + 1))
  done
  say "app log: $(app_said "$1" | cut -c1-220)"
}

if [ "$scenario" = electron ]; then
  until_said 'holds another app than this one' 240 "the old app did not refuse today's AppImage within 240 s"
  stop "$app_old" "old app"
  launch_old
  until_said 'was refused before' 240 "the relaunched app did not remember the refused asset within 240 s"
  logs
  why=
  [ "$(downloads)" = 1 ] || why="$why; the asset was downloaded $(downloads) times, not once"
  [ "$(sha "$installed")" = "$old_sum" ] || why="$why; the installed app changed"
  left=$(ls -A "$stage_dir" 2>/dev/null || true)
  [ -z "$left" ] || why="$why; the stage holds: $left"
  alive "$app_old" || why="$why; the relaunched old app is not running"
  echo "== downloads of the asset: $(downloads); installed sha256 $(sha "$installed" | cut -c1-12), before $(printf '%s' "$old_sum" | cut -c1-12)"
  [ -z "$why" ] || fail "${why#; }"
  result=PASS
  echo "PASS: today's AppImage under $asset was refused off its squashfs, downloaded once across a relaunch, and nothing was replaced"
  exit 0
fi

if [ "$scenario" = refused ] || [ "$scenario" = crash ]; then
  t=0
  until grep -q ' opened wsp ' "$home/.wsp/logs/update.log" 2>/dev/null; do
    [ "$t" -lt 240 ] || fail "the swap did not open the new app within 240 s"
    sleep 1
    t=$((t + 1))
  done
  # The app put back would find the same release and try again; the feed now offers nothing newer.
  echo "$old_v" >"$work/feed/VERSION"
  napp=$(new_app_pid)
  say "the swap opened the new app as pid $napp; the feed now offers $old_v"
  t=0
  until grep -q 'putting the old app back' "$home/.wsp/logs/update.log" 2>/dev/null; do
    [ "$t" -lt 240 ] || fail "the swap did not put the old app back within 240 s"
    sleep 1
    t=$((t + 1))
  done
  t=0
  back_host=
  while [ "$t" -lt 180 ]; do
    pid=$(lock_pid)
    if [ ! -e "$stage_dir" ] && [ -n "$(installed_apps)" ] && alive "$pid" && args_of "$pid" | grep -qF "$old_copy/"; then back_host=$pid; break; fi
    sleep 1
    t=$((t + 1))
  done
  logs
  why=
  [ "$(sha "$installed")" = "$old_sum" ] || why="$why; the installed app is not the old one"
  [ -n "$back_host" ] || why="$why; no host on the old files with the old app running and the stage swept within 180 s (the lock names $(lock_pid): $(args_of "$(lock_pid)"))"
  alive "$napp" && why="$why; the new app $napp still runs"
  echo "== the new app's own words: $(app_said 'did not open on its host\|the host did not follow' | cut -c1-260)"
  echo "== the app put back says: $(app_said 'did not update' | cut -c1-260)"
  echo "== the host names version $(host_version "$back_host")"
  [ "$(host_version "$back_host")" = "$old_v" ] || why="$why; the host names $(host_version "$back_host"), not $old_v"
  echo "== apps running the installed app: $(installed_apps | tr '\n' ' ')"
  echo "== host $back_host: $(args_of "$back_host")"
  [ -z "$why" ] || fail "${why#; }"
  result=PASS
  if [ "$scenario" = crash ]; then
    echo "PASS: the new app died after moving the host and before up; the swap put the old app back, which moved the host back onto $old_copy"
  else
    echo "PASS: the new app could not open on its host, the swap put the old app back, and the host followed back onto $old_copy"
  fi
  exit 0
fi

installed_now() {
  if [ "$os" = Linux ]; then
    [ "$(sha "$installed" 2>/dev/null)" = "$new_sum" ] && echo "$new_v" || echo "not $new_v"
  else
    plutil -extract CFBundleShortVersionString raw -o - "$installed/Contents/Info.plist" 2>/dev/null || echo '?'
  fi
}

t=0
while :; do
  now=$(installed_now)
  [ "$now" = "$new_v" ] && [ ! -e "$stage_dir" ] && break
  back=$(grep -e 'putting the old app back' -e 'nothing was changed' "$home/.wsp/logs/update.log" 2>/dev/null | tail -n 1)
  [ -z "$back" ] || fail "the swap gave up: ${back#* }"
  if [ $((t % 5)) = 0 ]; then
    say "${t}s: installed is $now; stage $([ -e "$stage_dir" ] && echo present || echo absent); feed answered $(grep -c ' GET ' "$work/feed.log") requests; old app $(alive "$app_old" && echo running || echo gone)"
  fi
  [ "$t" -lt 240 ] || fail "the installed app is not $new_v with its stage gone after 240 s"
  sleep 1
  t=$((t + 1))
done
say "installed app is $new_v and the stage is gone"

# The new host: a pid the lock names that no earlier host had, running the new files.
t=0
new_host=
while [ "$t" -lt 120 ]; do
  pid=$(until_host 1 "$hosts" || true)
  if [ -n "$pid" ]; then
    cmd=$(args_of "$pid")
    if [ "$os" = Linux ]; then
      copy=$(printf '%s\n' "$cmd" | sed -n "s|.*\\($home/.wsp/gpui-app/[^/ ]*\\)/.*|\\1|p")
      if [ -n "$copy" ] && [ "$copy" != "$old_copy" ]; then new_host=$pid; files=$copy; break; fi
    else
      case "$cmd" in "$installed"/*) new_host=$pid; files=$installed; break ;; esac
    fi
  fi
  t=$((t + 1))
done
[ -n "$new_host" ] || fail "no host on the new files within 120 s (the lock names $(lock_pid): $(args_of "$(lock_pid)"))"
say "host $new_host runs the new files in $files"

echo "== wsp activity --state through $wsp (starts the daemon)"
(cd "$home" && wsp_env "$wsp" activity --state "$state" 2>&1 | tail -n 4) || true
sleep 2

logs
echo "== host and its children"
show() {
  local pid=$1 depth=$2 kid
  printf '%*spid %s: %s\n' "$depth" '' "$pid" "$(args_of "$pid")"
  printf '%*s  exe %s\n' "$depth" '' "$(exe_of "$pid")"
  for kid in $(children "$pid"); do show "$kid" $((depth + 2)); done
}
show "$new_host" 2
daemon=
for kid in $(tree "$new_host"); do
  case "$(exe_of "$kid")" in */wsp-daemon) daemon=$kid ;; esac
done
if [ "$os" = Linux ]; then
  host_node=$files/usr/bin/node
  served=$files/usr/lib/wsp/app/package.json
else
  host_node=$files/Contents/MacOS/node
  served=$files/Contents/Resources/app/package.json
fi
echo "  $served says version $(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$served" | head -n 1)"
# Dials the host with its token (read here, never printed) and prints the version its auth reply names.
# shellcheck disable=SC2016
said=$(cd "$home" && env -i HOME="$home" PATH="$PATH" "$host_node" -e '
  const [port, tokenFile] = process.argv.slice(1);
  const token = require("node:fs").readFileSync(tokenFile, "utf8").trim();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  setTimeout(() => { console.log("no-answer"); process.exit(1); }, 10000);
  ws.onopen = () => ws.send(JSON.stringify({ id: 1, op: "auth", token }));
  ws.onerror = () => { console.log("no-dial"); process.exit(1); };
  ws.onmessage = m => { const a = JSON.parse(String(m.data)); if (a.id === 1) { console.log(a.ok ? (a.version ?? "none") : "refused"); process.exit(0); } };
' "$(sed -n 's/.*"port":\([0-9]*\).*/\1/p' "$lock")" "$home/.wsp/host-token" 2>&1) || true
echo "  the host names version $said"
napp=$(new_app_pid)
echo "== apps"
echo "  old app $app_old: $(alive "$app_old" && echo "still running: $(args_of "$app_old")" || echo gone)"
if [ -n "$napp" ] && alive "$napp"; then
  echo "  new app $napp: running, exe $(exe_of "$napp"), args $(args_of "$napp")"
else
  echo "  new app ${napp:-(no pid in update.log)}: not running"
fi

why=
[ "$said" = "$new_v" ] || why="$why; the host names $said, not $new_v"
[ -n "$daemon" ] || why="$why; the new host has no wsp-daemon child"
if [ -n "$daemon" ]; then
  case "$(exe_of "$daemon")" in "$files"/*) ;; *) why="$why; the daemon runs $(exe_of "$daemon"), outside the host's files $files" ;; esac
fi
alive "$app_old" && why="$why; the old app $app_old still runs"
{ [ -n "$napp" ] && alive "$napp" && runs_installed "$napp"; } || why="$why; the new app ${napp:-?} does not run the installed app"
if [ -z "$why" ]; then
  result=PASS
  echo "PASS: the app moved from $old_v to $new_v, and the service's host $new_host on the new files names $new_v with its daemon"
else
  fail "${why#; }"
fi
