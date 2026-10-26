#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Sets a Linux box up so every thread on it builds Rust warm: one sccache on the box's own disk behind every cargo
# (cargo's own config names it, so no thread sets anything), the Zig that Ghostty's build wants, and the C libraries the
# daemon and the GPUI app link. Run as root, once per box, from any checkout; running it again changes nothing.
set -eu

sccache_version=0.19.0
zig_version=0.16.0
cache_dir=/root/sccache
cache_gb=20
cargo_home=${CARGO_HOME:-/opt/wsp/cargo}
# sudo's secure_path and env_reset drop the toolchain's folder and its homes, which rustup below needs.
export CARGO_HOME=$cargo_home RUSTUP_HOME=${RUSTUP_HOME:-/opt/wsp/rustup} PATH=$cargo_home/bin:$PATH
wrapper=/usr/local/bin/sccache-rustc

if [ "$(id -u)" != 0 ]; then
  echo "refused: this installs into /usr/local, /opt and apt; run it as root" >&2
  exit 3
fi
case "$(uname -m)" in
  x86_64)
    sccache_sum=aa73c7270c997894038c0d8f0d297865f57569e1687883322bc431713d4bb9dd
    zig_sum=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00 ;;
  aarch64)
    sccache_sum=3e47fc4a71b1ddd1b4294a0de9d34498922fdf41e3cd0bda5df3954e3d6deb9d
    zig_sum=ea4b09bfb22ec6f6c6ceac57ab63efb6b46e17ab08d21f69f3a48b38e1534f17 ;;
  *) echo "refused: no pinned sccache or Zig for $(uname -m)" >&2; exit 3 ;;
esac
arch=$(uname -m)

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends build-essential pkg-config cmake clang gperf musl-tools xz-utils curl \
  libseccomp-dev libssl-dev libdbus-1-dev libasound2-dev libxdo-dev libfontconfig-dev libvulkan1 mesa-vulkan-drivers \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libxcb1-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev squashfs-tools xvfb fonts-wqy-microhei fonts-liberation

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
fetch() { # url sum file
  curl -fsSL "$1" -o "$scratch/$3"
  [ "$(sha256sum "$scratch/$3" | cut -d' ' -f1)" = "$2" ] || { echo "refused: $1 does not match its pinned sha256" >&2; exit 1; }
}

if [ "$(sccache --version 2>/dev/null)" != "sccache $sccache_version" ]; then
  name=sccache-v$sccache_version-$arch-unknown-linux-musl
  fetch "https://github.com/mozilla/sccache/releases/download/v$sccache_version/$name.tar.gz" "$sccache_sum" sccache.tgz
  tar -xzf "$scratch/sccache.tgz" -C "$scratch"
  install -m 755 "$scratch/$name/sccache" /usr/local/bin/sccache
fi

zig_dir=/opt/zig/zig-$arch-linux-$zig_version
if [ ! -x "$zig_dir/zig" ]; then
  fetch "https://ziglang.org/download/$zig_version/zig-$arch-linux-$zig_version.tar.xz" "$zig_sum" zig.txz
  mkdir -p /opt/zig
  tar -xJf "$scratch/zig.txz" -C /opt/zig
fi

# sccache keys every compile on CARGO_TARGET_DIR's value, so a thread with a target of its own would miss what every
# other thread compiled; rustc never reads it.
printf '#!/bin/sh\nunset CARGO_TARGET_DIR\nexec /usr/local/bin/sccache "$@"\n' > "$wrapper"
chmod 755 "$wrapper"

mkdir -p "$cache_dir" /root/.config/sccache "$cargo_home"
cat > /root/.config/sccache/config <<CONF
[cache.disk]
dir = "$cache_dir"
size = $((cache_gb * 1024 * 1024 * 1024))
CONF
# The cache folder rides in cargo's env table too: the server keeps the folder of whichever client starts it, and a
# build run with another HOME (the hand-in check's sentinel) would start one on an empty cache of its own.
cat > "$cargo_home/config.toml" <<CONF
[build]
rustc-wrapper = "$wrapper"

[env]
SCCACHE_DIR = "$cache_dir"
SCCACHE_CACHE_SIZE = "${cache_gb}G"
CONF

# The channel each lockfile's folder pins, so no thread waits on a toolchain download at its first build.
root=$(git rev-parse --show-toplevel)
for dir in "$root"/daemon "$root"/apps/desktop-gpui; do
  [ -f "$dir/rust-toolchain.toml" ] && (cd "$dir" && rustup toolchain install --profile minimal -c clippy -c rustfmt)
done
# The daemon's static build and the hand-in check's staging of it target this chip's own musl, which the file names only for x86_64.
(cd "$root/daemon" && rustup target add "$(uname -m)-unknown-linux-musl")
sccache --stop-server >/dev/null 2>&1 || true
sccache --start-server
sccache --show-stats | grep -E 'Cache location|Max cache size'
echo "zig: $zig_dir/zig"
