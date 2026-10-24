#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Builds the GPUI app into dist/. On macOS: a universal wsp.app under today's app's bundle id, its DMG in today's drag
# layout and the zip. On Linux: the bare binary and an x86_64 AppImage on today's static runtime. The app and the AppImage carry
# the host as today's app carries it: a Node, the host's built command (apps/desktop's build/app) and the daemon for
# each target, which packages/wspx/daemon must hold (WSP_REQUIRE_DAEMON=1 refuses a missing one, as the release does). With CSC_LINK and
# CSC_KEY_PASSWORD set it signs with that Developer ID, else ad hoc; with APPLE_ID, APPLE_TEAM_ID and
# APPLE_APP_SPECIFIC_PASSWORD also set it notarizes and staples. WSP_GPUI_BUNDLE_ID overrides the bundle id, so a
# check build on a Mac with wsp installed claims none of its links. Every file is named for the release's version.
#   --target mac|linux  what to build, the host's own by default; mac builds only on macOS
#   --dry-run           print each step and run none of them (no cargo, codesign, xcrun, hdiutil, download)
set -eu
# What a person unpacks or mounts has to be readable by them; a umask of 077 would pack every file as owner-only.
umask 022

usage='usage: apps/desktop-gpui/scripts/package.sh [--target mac|linux] [--dry-run]'
dry=0
target=
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1 ;;
    --target) target=${2-}; shift ;;
    -h|--help) echo "$usage"; exit 0 ;;
    *) echo "refused: $1 is not a flag this script takes; $usage" >&2; exit 2 ;;
  esac
  shift
done
host=$(uname -s)
if [ -z "$target" ]; then
  case "$host" in
    Darwin) target=mac ;;
    Linux) target=linux ;;
    *) echo "refused: this script builds on macOS or Linux, not $host" >&2; exit 2 ;;
  esac
fi
case "$target" in
  mac|linux) ;;
  *) echo "refused: --target takes mac or linux; $usage" >&2; exit 2 ;;
esac
if [ "$dry" = 0 ] && [ "$target" = mac ] && [ "$host" != Darwin ]; then
  echo "refused: the mac build needs lipo, codesign and hdiutil, which only macOS has; --dry-run prints its steps" >&2
  exit 2
fi

cd "$(dirname "$0")/.."
bin=desktop-gpui
# The release's version, which apps/desktop/package.json carries as every manifest does (tag-version.mjs checks it); the
# host copies and the updater read it, so the bundle says the same.
version=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' ../desktop/package.json | head -n 1)
if [ -z "$version" ]; then
  echo "refused: apps/desktop/package.json has no version line" >&2
  exit 1
fi
name=wsp
slug=wsp-$version
bundle_id=${WSP_GPUI_BUNDLE_ID:-wsp.desktop}
case "$bundle_id" in
  *[!A-Za-z0-9.-]*) echo "refused: WSP_GPUI_BUNDLE_ID takes letters, digits, dots and hyphens, as a bundle id does" >&2; exit 2 ;;
esac
min_macos=12.0
icns=../desktop/build/icon.icns
ico=../desktop/build/icon.ico
dist=dist
# AppImage/type2-runtime's 20251108 x86_64 runtime, byte for byte the runtime-x64 of electron-builder's appimage@1.0.3
# toolset, which electron-builder.yml pins.
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sum=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

# The Node the host runs on: major 22, as package.json's engines and release.yml's NODE_VERSION name, at the release
# packages/catalog's NODE_RELEASES pins, with the sums nodejs.org/dist/v22.23.2/SHASUMS256.txt gives each archive.
node_version=22.23.2
node_sum() {
  case "$1" in
    darwin-arm64) echo 61130f394c1630d211dd50aecc4353d379480f36d3ac913cd85dbba1aed585c6 ;;
    darwin-x64) echo 58e99022c2ff89395576cc7fd4d98cea24bb68081475d5f88b801ee8729fb026 ;;
    linux-x64) echo b294a556e639d64338823920e5866c21c02741742d2e1529ee1a225c1ec9252a ;;
  esac
}
# Today's app's own staged host: the command the shim runs with its chunks, and assets/ (web, cli, daemon/<triple>).
host_app=../desktop/build/app
entitlements=../desktop/build/entitlements.mac.plist

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wsp-package.XXXXXX")
keychain=
device=
kpass=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 32)
cleanup() {
  if [ -n "$device" ]; then hdiutil detach -force "$device" >/dev/null 2>&1 || true; fi
  if [ -n "$keychain" ] && [ "$dry" = 0 ]; then security delete-keychain "$keychain" >/dev/null 2>&1 || true; fi
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

ok() { printf 'ok    %s\n' "$1"; }
skip() { printf 'skip  %s\n' "$1"; }
refuse() { printf 'refused: %s\n' "$1" >&2; exit 1; }
# Prints a command with every secret's value replaced, so no step line carries one.
show() {
  local arg line=+
  for arg in "$@"; do
    if [ -n "$arg" ]; then
      case "$arg" in
        "${CSC_LINK-}"|"${CSC_KEY_PASSWORD-}"|"${APPLE_ID-}"|"${APPLE_TEAM_ID-}"|"${APPLE_APP_SPECIFIC_PASSWORD-}"|"$kpass") arg='<hidden>' ;;
      esac
    fi
    case "$arg" in
      *[!A-Za-z0-9_./:=@,+-]*|'') arg="'$arg'" ;;
    esac
    line="$line $arg"
  done
  printf '%s\n' "$line"
}
run() {
  show "$@"
  if [ "$dry" = 0 ]; then "$@"; fi
}
# Writes stdin to the file; a dry run prints the path alone.
write() {
  printf '+ write %s\n' "$1"
  if [ "$dry" = 0 ]; then cat >"$1"; else cat >/dev/null; fi
}
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

printf 'package %s %s for %s%s\n' "$name" "$version" "$target" "$([ "$dry" = 1 ] && echo ', dry run')"

# The same gate dev.sh keeps: vendor/ is what patches/ makes, and the Ghostty source and Zig are in place.
if [ "$dry" = 0 ]; then
  [ -d vendor ] || scripts/gpui-patches.sh apply
  cat patches/*.patch | cmp -s - vendor/.applied 2>/dev/null ||
    refuse "vendor/ is not what patches/ makes: run scripts/gpui-patches.sh apply, or save if you edited vendor/"
  scripts/ghostty.sh
else
  show scripts/gpui-patches.sh apply
  show scripts/ghostty.sh
fi
run mkdir -p "$dist"

# Runs a check of a file the bundle must hold, and refuses the build where it fails.
check() {
  show test "$1" "$2"
  if [ "$dry" = 0 ] && ! test "$1" "$2"; then refuse "the bundle lacks $2"; fi
}

# The Node archive for one target, out of vendor/.node once it is there and matches its sum, unpacked to $tmp/node-<t>.
node_fetch() {
  local pa=$1 sum archive
  sum=$(node_sum "$pa")
  archive=vendor/.node/node-v$node_version-$pa.tar.gz
  if [ "$dry" = 0 ] && [ -f "$archive" ] && [ "$(sha256 "$archive")" = "$sum" ]; then
    ok "Node $node_version for $pa is in $archive"
  else
    run mkdir -p vendor/.node
    run curl -fsSL "https://nodejs.org/dist/v$node_version/node-v$node_version-$pa.tar.gz" -o "$archive.part"
    if [ "$dry" = 0 ] && [ "$(sha256 "$archive.part")" != "$sum" ]; then
      rm -f "$archive.part"
      refuse "Node $node_version for $pa does not match its sha256 $sum"
    fi
    run mv "$archive.part" "$archive"
  fi
  run mkdir -p "$tmp/node-$pa"
  run tar -xzf "$archive" -C "$tmp/node-$pa" --strip-components 1 "node-v$node_version-$pa/bin/node" "node-v$node_version-$pa/LICENSE"
}

# Today's app's build of the host, the same two steps its own package runs before electron-builder.
host_build() {
  if [ "$dry" = 1 ] || [ ! -d ../../node_modules ]; then run pnpm -C ../.. install --frozen-lockfile; fi
  run pnpm -C ../.. --filter @wsp/desktop run build:deps
  run pnpm -C ../.. --filter @wsp/desktop run build:app
}

# Puts the host under a bundle's resources folder as today's app lays it out under its own: app/main/cli.mjs with every
# module it loads (the window's main.mjs stays out), app/assets and app/package.json; Node's licence beside them.
host_stage() {
  local res=$1 pa=$2 f
  run mkdir -p "$res/app/main" "$res/node"
  run cp "$host_app/package.json" "$res/app/package.json"
  printf '+ cp %s/main/*.mjs but main.mjs %s\n' "$host_app" "$res/app/main"
  if [ "$dry" = 0 ]; then
    for f in "$host_app"/main/*.mjs; do
      [ "$(basename "$f")" = main.mjs ] || cp "$f" "$res/app/main/"
    done
  fi
  run cp -R "$host_app/assets" "$res/app/assets"
  run cp "$tmp/node-$pa/LICENSE" "$res/node/LICENSE"
  # A build under umask 077 writes owner-only files, which a person other than the builder could not run.
  run chmod -R u+rwX,go+rX-w "$res/app" "$res/node"
}

# What each part of the bundle weighs on disk, in MB.
sizes() {
  local whole=$1 binary=$2 node=$3 res=$4 daemons
  [ "$dry" = 0 ] || return 0
  mb() { du -sk "$@" | awk '{ k += $1 } END { printf "%.1f", k / 1024 }'; }
  daemons=$(mb "$res/app/assets/daemon")
  ok "sizes on disk: the bundle $(mb "$whole") MB; the app binary $(mb "$binary") MB, Node $(mb "$node") MB, the host's JS $(mb "$res/app/main" "$res/app/assets/web" "$res/app/assets/cli" "$res/app/package.json") MB, the daemons $daemons MB ($(cd "$res/app/assets/daemon" && echo *))"
}

identity=
signed=0
sign_setup() {
  if [ -z "${CSC_LINK-}" ] && [ -z "${CSC_KEY_PASSWORD-}" ]; then
    skip "signing with a Developer ID: CSC_LINK and CSC_KEY_PASSWORD are not set, so the app is signed ad hoc"
    return
  fi
  if [ -z "${CSC_LINK-}" ] || [ -z "${CSC_KEY_PASSWORD-}" ]; then
    refuse "CSC_LINK and CSC_KEY_PASSWORD go together: the .p12 and its password; set both, or neither to sign ad hoc"
  fi
  local p12=${CSC_LINK#file://}
  if [ "$dry" = 1 ] || [ ! -f "$p12" ]; then
    p12=$tmp/cert.p12
    printf '+ decode CSC_LINK into %s\n' "$p12"
    if [ "$dry" = 0 ]; then
      printf '%s' "$CSC_LINK" | base64 -D >"$p12" 2>/dev/null || refuse "CSC_LINK is neither a .p12 file nor base64"
    fi
  fi
  keychain=$tmp/wsp-sign.keychain-db
  run security create-keychain -p "$kpass" "$keychain"
  run security set-keychain-settings -lut 3600 "$keychain"
  run security unlock-keychain -p "$kpass" "$keychain"
  run security import "$p12" -k "$keychain" -P "$CSC_KEY_PASSWORD" -T /usr/bin/codesign
  show security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$kpass" "$keychain"
  if [ "$dry" = 0 ]; then
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$kpass" "$keychain" >/dev/null
    # The search list has to hold the keychain for codesign to build the chain; xargs keeps the quoted paths whole.
    security list-keychains -d user | xargs security list-keychains -d user -s "$keychain"
    identity=$(security find-identity -v -p codesigning "$keychain" |
      sed -n 's/^ *[0-9]*) \([0-9A-F]\{40\}\) "Developer ID Application: .*"$/\1/p' | head -n 1)
    [ -n "$identity" ] || refuse "the certificate in CSC_LINK holds no Developer ID Application identity"
  else
    identity="Developer ID Application"
  fi
  signed=1
}

mac() {
  local app="$dist/$name.app" dmg="$dist/$slug-mac.dmg" zip="$dist/$slug-mac.zip" triple
  export MACOSX_DEPLOYMENT_TARGET=$min_macos
  run rustup target add aarch64-apple-darwin x86_64-apple-darwin
  for triple in aarch64-apple-darwin x86_64-apple-darwin; do
    run cargo build --release --locked -p "$bin" --target "$triple"
  done

  host_build
  node_fetch darwin-arm64
  node_fetch darwin-x64

  run rm -rf "$app" "$dmg" "$zip"
  run mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  run lipo -create -output "$app/Contents/MacOS/$bin" \
    "target/aarch64-apple-darwin/release/$bin" "target/x86_64-apple-darwin/release/$bin"
  # One Node for both chips, as the app binary is: the host runs on whichever slice the Mac runs natively.
  run lipo -create -output "$app/Contents/MacOS/node" "$tmp/node-darwin-arm64/bin/node" "$tmp/node-darwin-x64/bin/node"
  # Node's local symbols are 21 MB a slice and nothing reads them at run time; the exported ones an addon links stay.
  run strip -x "$app/Contents/MacOS/node"
  host_stage "$app/Contents/Resources" darwin-arm64
  local triple_mac
  check -x "$app/Contents/MacOS/node"
  check -f "$app/Contents/Resources/app/main/cli.mjs"
  for triple_mac in aarch64-apple-darwin x86_64-apple-darwin; do
    check -x "$app/Contents/Resources/app/assets/daemon/$triple_mac/wsp-daemon"
  done
  if [ "$host" = Darwin ]; then run "$app/Contents/MacOS/node" "$app/Contents/Resources/app/main/cli.mjs" --version; fi
  run cp "$icns" "$app/Contents/Resources/icon.icns"
  # Only today's bundle id takes wsp:// links, so a check build under another claims none of the person's.
  local links=
  if [ "$bundle_id" = wsp.desktop ]; then
    links='
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key>
      <string>wsp</string>
      <key>CFBundleURLSchemes</key>
      <array>
        <string>wsp</string>
      </array>
    </dict>
  </array>'
  fi
  write "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>$name</string>
  <key>CFBundleExecutable</key>
  <string>$bin</string>
  <key>CFBundleIconFile</key>
  <string>icon.icns</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$name</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>$links
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>$min_macos</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF
  run plutil -lint "$app/Contents/Info.plist"

  sign_setup
  # Every Mach-O inside is signed before the bundle seals them: V8 needs the JIT entitlements under the hardened
  # runtime, as Electron's does (entitlements.mac.plist), and the daemons take the runtime alone.
  local inner
  if [ "$signed" = 1 ]; then
    run codesign --force --options runtime --timestamp --keychain "$keychain" --entitlements "$entitlements" --sign "$identity" "$app/Contents/MacOS/node"
    for inner in aarch64-apple-darwin x86_64-apple-darwin; do
      run codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$identity" "$app/Contents/Resources/app/assets/daemon/$inner/wsp-daemon"
    done
  else
    run codesign --force --options runtime --entitlements "$entitlements" --sign - "$app/Contents/MacOS/node"
    for inner in aarch64-apple-darwin x86_64-apple-darwin; do
      run codesign --force --options runtime --sign - "$app/Contents/Resources/app/assets/daemon/$inner/wsp-daemon"
    done
  fi
  if [ "$signed" = 1 ]; then
    run codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$identity" "$app/Contents/MacOS/$bin"
    run codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$identity" "$app"
  else
    run codesign --force --options runtime --sign - "$app/Contents/MacOS/$bin"
    run codesign --force --options runtime --sign - "$app"
  fi
  run codesign --verify --strict --verbose=2 "$app"

  dmg_build "$app" "$dmg"
  if [ "$signed" = 1 ]; then
    run codesign --force --timestamp --keychain "$keychain" --sign "$identity" "$dmg"
  fi

  if [ "$signed" = 0 ]; then
    skip "notarizing: Apple notarizes only a Developer ID signature, and this build is signed ad hoc"
  elif [ -z "${APPLE_ID-}" ] || [ -z "${APPLE_TEAM_ID-}" ] || [ -z "${APPLE_APP_SPECIFIC_PASSWORD-}" ]; then
    skip "notarizing: APPLE_ID, APPLE_TEAM_ID and APPLE_APP_SPECIFIC_PASSWORD are not all set"
  else
    # The disk image carries the app, so one submission covers both, and both take the ticket.
    show xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait
    if [ "$dry" = 0 ]; then
      local out
      out=$(xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait 2>&1) || true
      printf '%s\n' "$out"
      printf '%s\n' "$out" | grep -q 'status: Accepted' ||
        refuse "Apple did not accept $dmg; read why with xcrun notarytool log <the id above> and the same three credentials"
    fi
    run xcrun stapler staple "$app"
    run xcrun stapler staple "$dmg"
  fi

  run ditto -c -k --sequesterRsrc --keepParent "$app" "$zip"
  sizes "$app" "$app/Contents/MacOS/$bin" "$app/Contents/MacOS/node" "$app/Contents/Resources"
  ok "built $app, $dmg and $zip"
}

# Today's drag window (apps/desktop/electron-builder.yml, dmg:): the 660x400 background, 128 px icons, the app at
# 172,186 and /Applications at 488,186, titled with the version.
dmg_build() {
  local app=$1 dmg=$2 title="$name $version" stage=$tmp/dmg rw=$tmp/rw.dmg size
  if [ "$dry" = 0 ] && [ -e "/Volumes/$title" ]; then
    refuse "a volume named $title is mounted already, and Finder would lay out that one; eject it and run this again"
  fi
  run mkdir -p "$stage/.background"
  run ditto "$app" "$stage/$name.app"
  run ln -s /Applications "$stage/Applications"
  run tiffutil -cathidpicheck ../desktop/build/dmg-background.png ../desktop/build/dmg-background@2x.png \
    -out "$stage/.background/background.tiff"
  size=0
  [ "$dry" = 1 ] || size=$(du -sm "$stage" | cut -f1)
  run hdiutil create -srcfolder "$stage" -volname "$title" -fs HFS+ -format UDRW -size "$((size + 20))m" -ov "$rw"
  show hdiutil attach -readwrite -noverify -noautoopen "$rw"
  if [ "$dry" = 0 ]; then
    device=$(hdiutil attach -readwrite -noverify -noautoopen "$rw" | sed -n '1s/[[:space:]].*//p')
  fi
  write "$tmp/layout.applescript" <<'EOF'
on run argv
  set volumeName to item 1 of argv
  set appName to item 2 of argv
  tell application "Finder"
    tell disk volumeName
      open
      set current view of container window to icon view
      set toolbar visible of container window to false
      set statusbar visible of container window to false
      set the bounds of container window to {200, 120, 860, 520}
      set opts to the icon view options of container window
      set arrangement of opts to not arranged
      set icon size of opts to 128
      set background picture of opts to file ".background:background.tiff"
      set position of item appName of container window to {172, 186}
      set position of item "Applications" of container window to {488, 186}
      close
      open
      update without registering applications
      delay 2
      close
    end tell
  end tell
end run
EOF
  # Finder writes the layout into the volume's .DS_Store; the first run asks to let this terminal control Finder.
  run osascript "$tmp/layout.applescript" "$title" "$name.app"
  run sync
  if [ "$dry" = 0 ]; then
    hdiutil detach "$device" >/dev/null || { sleep 3; hdiutil detach -force "$device" >/dev/null; }
    device=
  else
    show hdiutil detach "<the attached disk>"
  fi
  run hdiutil convert "$rw" -format UDZO -imagekey zlib-level=9 -o "$dmg"
}

# The 256 px PNG in today's icon.ico, the largest picture of the mark this repo carries outside the icns.
icon_png() {
  local count i at width size offset
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

linux() {
  local appdir=$tmp/AppDir appimage="$dist/$slug.AppImage" runtime=vendor/.appimage/runtime-x86_64-20251108
  if [ "$dry" = 0 ]; then
    [ "$(uname -m)" = x86_64 ] || refuse "the AppImage is x86_64, as today's is, and this machine is $(uname -m)"
    command -v mksquashfs >/dev/null || refuse "mksquashfs is missing: apt-get install squashfs-tools"
  fi
  run cargo build --release --locked -p "$bin"
  host_build
  node_fetch linux-x64
  run rm -f "$dist/$bin" "$appimage"
  run install -m 755 "target/release/$bin" "$dist/$bin"

  if [ "$dry" = 0 ] && [ -f "$runtime" ] && [ "$(sha256 "$runtime")" = "$runtime_sum" ]; then
    ok "the AppImage runtime is in $runtime"
  else
    run mkdir -p vendor/.appimage
    run curl -fsSL "$runtime_url" -o "$runtime.part"
    if [ "$dry" = 0 ] && [ "$(sha256 "$runtime.part")" != "$runtime_sum" ]; then
      rm -f "$runtime.part"
      refuse "the AppImage runtime at $runtime_url does not match its sha256 $runtime_sum"
    fi
    run mv "$runtime.part" "$runtime"
  fi

  run mkdir -p "$appdir/usr/bin" "$appdir/usr/share/icons/hicolor/256x256/apps"
  run install -m 755 "target/release/$bin" "$appdir/usr/bin/$bin"
  run install -m 755 "$tmp/node-linux-x64/bin/node" "$appdir/usr/bin/node"
  host_stage "$appdir/usr/lib/wsp" linux-x64
  check -x "$appdir/usr/bin/node"
  check -f "$appdir/usr/lib/wsp/app/main/cli.mjs"
  check -x "$appdir/usr/lib/wsp/app/assets/daemon/x86_64-unknown-linux-musl/wsp-daemon"
  run "$appdir/usr/bin/node" "$appdir/usr/lib/wsp/app/main/cli.mjs" --version
  write "$appdir/AppRun" <<EOF
#!/bin/sh
here=\${APPDIR:-\$(dirname "\$(readlink -f "\$0")")}
exec "\$here/usr/bin/$bin" "\$@"
EOF
  run chmod 755 "$appdir/AppRun"
  write "$appdir/wsp.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$name
Comment=wsp's desktop app
Exec=$bin %U
Icon=wsp
Terminal=false
Categories=Development;
MimeType=x-scheme-handler/wsp;
X-AppImage-Version=$version
EOF
  printf '+ write %s from %s\n' "$appdir/wsp.png" "$ico"
  if [ "$dry" = 0 ]; then icon_png >"$appdir/wsp.png"; fi
  run cp "$appdir/wsp.png" "$appdir/usr/share/icons/hicolor/256x256/apps/wsp.png"
  run ln -s wsp.png "$appdir/.DirIcon"
  # electron-builder's own mksquashfs flags for the static runtime, gzip by default as today's.
  run mksquashfs "$appdir" "$tmp/app.squashfs" -all-root -noappend -no-progress -quiet -no-xattrs -no-fragments
  printf '+ write %s from %s and %s\n' "$appimage" "$runtime" "$tmp/app.squashfs"
  if [ "$dry" = 0 ]; then cat "$runtime" "$tmp/app.squashfs" >"$appimage"; fi
  run chmod 755 "$appimage"
  sizes "$appdir" "$appdir/usr/bin/$bin" "$appdir/usr/bin/node" "$appdir/usr/lib/wsp"
  if [ "$dry" = 0 ]; then ok "the AppImage file $(du -k "$appimage" | awk '{ printf "%.1f", $1 / 1024 }') MB, its squashfs compressed"; fi
  ok "built $dist/$bin and $appimage"
}

"$target"
