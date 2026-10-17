#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Our patches over the published gpui-pre crates (backdrop blur, rounded clipping of children), kept as one diff per
# crate in patches/ and applied to fresh copies in vendor/, which [patch.crates-io] in Cargo.toml points at.
#   apply  unpack each pinned .crate (checksum-verified) into vendor/ and apply its patch; run after a fresh clone
#   save   rewrite patches/ from your edits under vendor/
#   check  apply into a scratch folder and fail if a patch no longer applies or vendor/ differs from the result
set -eu

usage='usage: apps/desktop-gpui/scripts/gpui-patches.sh apply|save|check'
cd "$(dirname "$0")/.."
root=$(pwd)

# name version sha256, the sums Cargo.lock held for crates.io's copies before the patch replaced them.
crates='gpui-pre 0.3.9 34f61c1cfd36fc9c436af09308c963822ae27a45a84feb21a77ba5c9e051f1c0
gpui-pre-wgpu 0.3.9 5b1cdb26c1503bf9662ee8fbc5ab0b44b2deeee4b5c12dc67491ba6fb5fc544b
gpui-pre-apple 0.3.9 b52f58893fe1a0836464428dfad2b5eaf42c9c303a9638f4c2aced5a69072a15'
# gpui-pre-apple's build script reads these gpui-pre files from its own vendor/gpui to generate the Metal header.
apple_mirror='src/scene.rs src/geometry.rs src/color.rs src/window.rs src/platform.rs'

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# Prints the path of the verified .crate, from cargo's cache, our download cache, or crates.io.
fetch() {
  local name=$1 version=$2 sum=$3 file="$1-$2.crate" cached
  mkdir -p vendor/.crates
  for cached in "${CARGO_HOME:-$HOME/.cargo}"/registry/cache/*/"$file" "vendor/.crates/$file"; do
    if [ -f "$cached" ] && [ "$(sha256 "$cached")" = "$sum" ]; then
      echo "$cached"
      return
    fi
  done
  curl -fsSL "https://static.crates.io/crates/$name/$file" -o "vendor/.crates/$file.part"
  if [ "$(sha256 "vendor/.crates/$file.part")" != "$sum" ]; then
    echo "refused: $file from crates.io does not match its checksum $sum" >&2
    exit 1
  fi
  mv "vendor/.crates/$file.part" "vendor/.crates/$file"
  echo "vendor/.crates/$file"
}

# Unpacks fresh, patched copies of every crate into $1.
build() {
  local out=$1 name version sum dir file
  mkdir -p "$out"
  while read -r name version sum; do
    dir="$name-$version"
    tar -xzf "$(fetch "$name" "$version" "$sum")" -C "$out"
    [ -s "patches/$dir.patch" ] || continue
    (cd "$out/$dir" && git apply -p1 --whitespace=nowarn "$root/patches/$dir.patch") || {
      echo "refused: patches/$dir.patch no longer applies to $name $version" >&2
      exit 1
    }
  done <<<"$crates"
  for file in $apple_mirror; do
    cp "$out/gpui-pre-0.3.9/$file" "$out/gpui-pre-apple-0.3.9/vendor/gpui/$file"
  done
}

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT INT TERM HUP

case "${1:-}" in
  apply)
    build "$scratch/new"
    while read -r name version _; do
      rm -rf "vendor/$name-$version"
      mv "$scratch/new/$name-$version" "vendor/$name-$version"
    done <<<"$crates"
    echo "applied patches/ to fresh copies in vendor/"
    ;;
  save)
    while read -r name version sum; do
      mkdir -p "$scratch/$name"
      tar -xzf "$(fetch "$name" "$version" "$sum")" -C "$scratch/$name"
      mv "$scratch/$name/$name-$version" "$scratch/$name/a"
      cp -R "vendor/$name-$version" "$scratch/$name/b"
      rm -rf "$scratch/$name/b/vendor/gpui" "$scratch/$name/a/vendor/gpui" "$scratch/$name/b/target"
      find "$scratch/$name" -name .DS_Store -exec rm -f {} +
      (cd "$scratch/$name" && git diff --no-index --no-color --no-ext-diff --no-prefix a b >"$root/patches/$name-$version.patch") || true
      echo "patches/$name-$version.patch: $(grep -c '^diff --git' "patches/$name-$version.patch") files"
    done <<<"$crates"
    ;;
  check)
    build "$scratch/new"
    status=0
    while read -r name version _; do
      if ! diff -r -q -x target -x .DS_Store "$scratch/new/$name-$version" "vendor/$name-$version"; then
        echo "vendor/$name-$version differs from patches/$name-$version.patch applied to a fresh copy; run save or apply" >&2
        status=1
      fi
    done <<<"$crates"
    [ "$status" -eq 0 ] && echo "every patch applies, and vendor/ matches"
    exit "$status"
    ;;
  *) echo "$usage" >&2; exit 2 ;;
esac
