#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Builds the GPUI app into dist/. On macOS: a universal "wsp Experimental.app", its DMG in today's drag layout and
# the zip. On Linux: the bare binary and an x86_64 AppImage on today's static runtime. With CSC_LINK and
# CSC_KEY_PASSWORD set it signs with that Developer ID, else ad hoc; with APPLE_ID, APPLE_TEAM_ID and
# APPLE_APP_SPECIFIC_PASSWORD also set it notarizes and staples. WSP_GPUI_BUNDLE_ID overrides the bundle id.
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
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
if [ -z "$version" ]; then
  echo "refused: Cargo.toml's [workspace.package] has no version line" >&2
  exit 1
fi
name="wsp Experimental"
slug=wsp-experimental-$version
bundle_id=${WSP_GPUI_BUNDLE_ID:-wsp.desktop.experimental}
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
  local app="$dist/$name.app" dmg="$dist/$slug.dmg" zip="$dist/$slug-mac.zip" triple
  export MACOSX_DEPLOYMENT_TARGET=$min_macos
  run rustup target add aarch64-apple-darwin x86_64-apple-darwin
  for triple in aarch64-apple-darwin x86_64-apple-darwin; do
    run cargo build --release --locked -p "$bin" --target "$triple"
  done

  run rm -rf "$app" "$dmg" "$zip"
  run mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  run lipo -create -output "$app/Contents/MacOS/$bin" \
    "target/aarch64-apple-darwin/release/$bin" "target/x86_64-apple-darwin/release/$bin"
  run cp "$icns" "$app/Contents/Resources/icon.icns"
  # No CFBundleURLTypes while Experimental: today's app keeps wsp:// links and hands them here while its switch is on.
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
  <string>APPL</string>
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
  local appdir=$tmp/AppDir appimage="$dist/$slug-x86_64.AppImage" runtime=vendor/.appimage/runtime-x86_64-20251108
  if [ "$dry" = 0 ]; then
    [ "$(uname -m)" = x86_64 ] || refuse "the AppImage is x86_64, as today's is, and this machine is $(uname -m)"
    command -v mksquashfs >/dev/null || refuse "mksquashfs is missing: apt-get install squashfs-tools"
  fi
  run cargo build --release --locked -p "$bin"
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
  write "$appdir/AppRun" <<EOF
#!/bin/sh
here=\${APPDIR:-\$(dirname "\$(readlink -f "\$0")")}
exec "\$here/usr/bin/$bin" "\$@"
EOF
  run chmod 755 "$appdir/AppRun"
  # No MimeType=x-scheme-handler/wsp while Experimental: today's app keeps wsp:// links and hands them here while its switch is on.
  write "$appdir/wsp-experimental.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$name
Comment=wsp's desktop app on GPUI, Experimental
Exec=$bin
Icon=wsp-experimental
Terminal=false
Categories=Development;
X-AppImage-Version=$version
EOF
  printf '+ write %s from %s\n' "$appdir/wsp-experimental.png" "$ico"
  if [ "$dry" = 0 ]; then icon_png >"$appdir/wsp-experimental.png"; fi
  run cp "$appdir/wsp-experimental.png" "$appdir/usr/share/icons/hicolor/256x256/apps/wsp-experimental.png"
  run ln -s wsp-experimental.png "$appdir/.DirIcon"
  # electron-builder's own mksquashfs flags for the static runtime, gzip by default as today's.
  run mksquashfs "$appdir" "$tmp/app.squashfs" -all-root -noappend -no-progress -quiet -no-xattrs -no-fragments
  printf '+ write %s from %s and %s\n' "$appimage" "$runtime" "$tmp/app.squashfs"
  if [ "$dry" = 0 ]; then cat "$runtime" "$tmp/app.squashfs" >"$appimage"; fi
  run chmod 755 "$appimage"
  ok "built $dist/$bin and $appimage"
}

"$target"
