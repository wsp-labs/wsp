#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Fetches the Ghostty source libghostty-vt-sys builds into vendor/ghostty, which .cargo/config.toml hands it as
# GHOSTTY_SOURCE_DIR, and checks the Zig on PATH is the one that source builds with. Run after a fresh clone.
set -eu

# GHOSTTY_COMMIT in libghostty-vt-sys's build.rs at the rev app/Cargo.toml pins, and the Zig that commit's
# build.zig.zon names as minimum_zig_version.
commit=3425025e585a3403340d4dc6d65132f42e05e605
# sha256 of codeload.github.com's tarball of that commit, checked before it is unpacked.
sum=268760f13121b4f4cd33456e1b216415a8c0c5a8c62ff2b87a14ee4f74bd7590
zig_wanted=0.16.0

cd "$(dirname "$0")/.."
dir=vendor/ghostty

zig_found=$(zig version 2>/dev/null) || zig_found=none
if [ "$zig_found" != "$zig_wanted" ]; then
  echo "refused: Ghostty $commit builds with Zig $zig_wanted, and the zig on PATH is $zig_found; put Zig $zig_wanted first on PATH (https://ziglang.org/download/#release-$zig_wanted)" >&2
  exit 1
fi

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# The crate's own blobless clone, and a shallow git fetch too, stalled for minutes on Boat; the commit's tarball
# takes seconds. The folder names its commit in a stamp file, so a pin bump fetches again.
if [ "$(cat "$dir/.commit" 2>/dev/null)" != "$commit" ]; then
  rm -rf "$dir" "$dir.part" "$dir.tgz"
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
  echo "$commit" >"$dir.part/.commit"
  mv "$dir.part" "$dir"
fi
echo "Ghostty $commit in $dir, Zig $zig_found"
