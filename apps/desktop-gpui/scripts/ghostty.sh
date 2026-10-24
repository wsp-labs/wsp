#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Fetches the Ghostty source .cargo/config.toml names as GHOSTTY_SOURCE_DIR and checks the Zig on PATH builds it.
set -eu

cd "$(dirname "$0")/.."

# The commit is GHOSTTY_COMMIT in libghostty-vt-sys's build.rs at app/Cargo.toml's rev; a bump renames the folder.
dir=$(sed -n 's/^GHOSTTY_SOURCE_DIR = { value = "\(vendor\/ghostty-[0-9a-f]\{40\}\)".*/\1/p' .cargo/config.toml)
if [ -z "$dir" ]; then
  echo "refused: .cargo/config.toml names no GHOSTTY_SOURCE_DIR of the form vendor/ghostty-<commit>" >&2
  exit 1
fi
commit=${dir#vendor/ghostty-}
# sha256 of codeload.github.com's tarball of that commit, checked before it is unpacked.
sum=268760f13121b4f4cd33456e1b216415a8c0c5a8c62ff2b87a14ee4f74bd7590
# The Zig that commit's build.zig.zon names as minimum_zig_version.
zig_wanted=0.16.0

zig_found=$(zig version 2>/dev/null) || zig_found=none
if [ "$zig_found" != "$zig_wanted" ]; then
  echo "refused: Ghostty $commit builds with Zig $zig_wanted, and the zig on PATH is $zig_found; put Zig $zig_wanted first on PATH (https://ziglang.org/download/#release-$zig_wanted)" >&2
  exit 1
fi

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# A git clone or fetch stalled for minutes on Boat where the tarball takes seconds; unpacked aside, moved in whole.
if [ ! -d "$dir" ]; then
  rm -rf vendor/ghostty vendor/ghostty-*
  mkdir -p vendor
  curl -fsSL "https://codeload.github.com/ghostty-org/ghostty/tar.gz/$commit" -o "$dir.tgz"
  if [ "$(sha256 "$dir.tgz")" != "$sum" ]; then
    rm -f "$dir.tgz"
    echo "refused: the tarball of Ghostty $commit does not match its sha256 $sum" >&2
    exit 1
  fi
  mkdir -p "$dir.part"
  tar -xzf "$dir.tgz" -C "$dir.part" --strip-components 1
  rm -f "$dir.tgz"
  mv "$dir.part" "$dir"
fi
echo "Ghostty $commit in $dir, Zig $zig_found"
