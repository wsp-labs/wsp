#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Our patches over the published gpui-pre crates (backdrop blur, rounded clipping of children) and over crates of
# gpui-kit's pinned commit (the markdown view's lists, task boxes, code block and table chrome), kept as one diff per
# crate in patches/ and applied to fresh copies in vendor/, which Cargo.toml's [patch] tables point at.
#   apply  unpack each pinned .crate (checksum-verified) and gpui-kit crate (by commit id) into vendor/ and apply its
#          patch; run after a fresh clone
#          and after a pull that changes patches/ (vendor/.applied keeps the patches it applied, for dev.sh to compare)
#   save   rewrite patches/ from your edits under vendor/
#   check  apply into a scratch folder and fail if a patch no longer applies or vendor/ differs from the result
set -eu
# A hook or a linked worktree sets these, and the gpui-kit fetch below would then write into that repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

usage='usage: apps/desktop-gpui/scripts/gpui-patches.sh apply|save|check'
cd "$(dirname "$0")/.."
root=$(pwd)

# name version sha256, the sums Cargo.lock held for crates.io's copies before the patch replaced them.
crates='gpui-pre 0.3.9 34f61c1cfd36fc9c436af09308c963822ae27a45a84feb21a77ba5c9e051f1c0
gpui-pre-wgpu 0.3.9 5b1cdb26c1503bf9662ee8fbc5ab0b44b2deeee4b5c12dc67491ba6fb5fc544b
gpui-pre-apple 0.3.9 b52f58893fe1a0836464428dfad2b5eaf42c9c303a9638f4c2aced5a69072a15'
# gpui-pre-apple's build script reads these gpui-pre files from its own vendor/gpui to generate the Metal header.
apple_mirror='src/scene.rs src/geometry.rs src/color.rs src/window.rs src/platform.rs'
# gpui-kit's pinned commit and the crates of it we patch, by their folder under crates/. They unpack beside gpui-kit's
# root manifest, which they inherit from, into vendor/gpui-kit-<short id>/, which Cargo.toml's workspace excludes.
kit_rev=14f59330a4aa0cb5becf140e067d64a616328612
kit_dir=gpui-kit-${kit_rev:0:7}
kit_crates='base'

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
  curl -fsSL "https://static.crates.io/crates/$name/$file" -o "vendor/.crates/$file.part" || {
    echo "refused: could not download $file from crates.io, and no copy with its checksum is in vendor/.crates or cargo's cache" >&2
    exit 1
  }
  if [ "$(sha256 "vendor/.crates/$file.part")" != "$sum" ]; then
    echo "refused: $file from crates.io does not match its checksum $sum" >&2
    exit 1
  fi
  mv "vendor/.crates/$file.part" "vendor/.crates/$file"
  echo "vendor/.crates/$file"
}

# Prints a git folder holding gpui-kit's pinned commit: cargo's own clone, our earlier fetch, or a fetch from GitHub.
kit_source() {
  local db
  for db in "${CARGO_HOME:-$HOME/.cargo}"/git/db/gpui-kit-* vendor/.gpui-kit.git; do
    if [ -d "$db" ] && [ "$(git -C "$db" rev-parse --verify --quiet "$kit_rev^{commit}")" = "$kit_rev" ]; then
      echo "$db"
      return
    fi
  done
  mkdir -p vendor
  git init --quiet --bare vendor/.gpui-kit.git
  git -C vendor/.gpui-kit.git fetch --quiet --depth 1 https://github.com/longbridge/gpui-kit "$kit_rev" || {
    echo "refused: could not fetch gpui-kit $kit_rev from GitHub, and cargo's git cache does not hold it" >&2
    exit 1
  }
  if [ "$(git -C vendor/.gpui-kit.git rev-parse --verify --quiet "$kit_rev^{commit}")" != "$kit_rev" ]; then
    echo "refused: gpui-kit's fetch did not bring commit $kit_rev" >&2
    exit 1
  fi
  echo vendor/.gpui-kit.git
}

# Unpacks gpui-kit's root manifest and fresh copies of kit_crates into $1/$kit_dir.
kit_unpack() {
  local db
  db=$(kit_source)
  mkdir -p "$1/$kit_dir"
  # shellcheck disable=SC2046,SC2086
  git -C "$db" archive --format=tar -o "$scratch/gpui-kit.tar" "$kit_rev" Cargo.toml $(printf 'crates/%s ' $kit_crates)
  tar -xf "$scratch/gpui-kit.tar" -C "$1/$kit_dir"
}

# Unpacks fresh, patched copies of every crate into $1.
build() {
  local out=$1 name version sum dir file crate
  mkdir -p "$out"
  while read -r name version sum; do
    dir="$name-$version"
    # An assignment, so a refused fetch stops the script under set -e instead of handing tar an empty path.
    crate=$(fetch "$name" "$version" "$sum")
    tar -xzf "$crate" -C "$out"
    [ -f "patches/$dir.patch" ] || { echo "refused: patches/$dir.patch is missing" >&2; exit 1; }
    [ -s "patches/$dir.patch" ] || continue
    # Under a git worktree (a TMPDIR inside one), git apply skips every path outside it and still succeeds.
    (cd "$out/$dir" && GIT_CEILING_DIRECTORIES="$out" git apply -p1 --whitespace=nowarn "$root/patches/$dir.patch") || {
      echo "refused: patches/$dir.patch no longer applies to $name $version" >&2
      exit 1
    }
  done <<<"$crates"
  for file in $apple_mirror; do
    cp "$out/gpui-pre-0.3.9/$file" "$out/gpui-pre-apple-0.3.9/vendor/gpui/$file"
  done
  kit_unpack "$out"
  for crate in $kit_crates; do
    file=patches/gpui-kit-$crate-${kit_rev:0:7}.patch
    [ -f "$file" ] || { echo "refused: $file is missing" >&2; exit 1; }
    [ -s "$file" ] || continue
    (cd "$out/$kit_dir/crates/$crate" && GIT_CEILING_DIRECTORIES="$out" git apply -p1 --whitespace=nowarn "$root/$file") || {
      echo "refused: $file no longer applies to gpui-kit ${kit_rev:0:7}" >&2
      exit 1
    }
  done
}

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT INT TERM HUP

case "${1:-}" in
  apply)
    build "$scratch/new"
    rm -f vendor/.applied
    while read -r name version _; do
      rm -rf "vendor/$name-$version"
      mv "$scratch/new/$name-$version" "vendor/$name-$version"
    done <<<"$crates"
    rm -rf "vendor/$kit_dir"
    mv "$scratch/new/$kit_dir" "vendor/$kit_dir"
    cat patches/*.patch >vendor/.applied
    echo "applied patches/ to fresh copies in vendor/"
    ;;
  save)
    for file in $apple_mirror; do
      cp "vendor/gpui-pre-0.3.9/$file" "vendor/gpui-pre-apple-0.3.9/vendor/gpui/$file"
    done
    while read -r name version sum; do
      mkdir -p "$scratch/$name"
      crate=$(fetch "$name" "$version" "$sum")
      tar -xzf "$crate" -C "$scratch/$name"
      mv "$scratch/$name/$name-$version" "$scratch/$name/a"
      cp -R "vendor/$name-$version" "$scratch/$name/b"
      rm -rf "$scratch/$name/b/vendor/gpui" "$scratch/$name/a/vendor/gpui" "$scratch/$name/b/target"
      find "$scratch/$name" -name .DS_Store -exec rm -f {} +
      (cd "$scratch/$name" && git diff --no-index --no-color --no-ext-diff --no-prefix a b >"$root/patches/$name-$version.patch") || true
      echo "patches/$name-$version.patch: $(grep -c '^diff --git' "patches/$name-$version.patch") files"
    done <<<"$crates"
    kit_unpack "$scratch/kit"
    for crate in $kit_crates; do
      file=patches/gpui-kit-$crate-${kit_rev:0:7}.patch
      mkdir -p "$scratch/kit-$crate"
      mv "$scratch/kit/$kit_dir/crates/$crate" "$scratch/kit-$crate/a"
      cp -R "vendor/$kit_dir/crates/$crate" "$scratch/kit-$crate/b"
      rm -rf "$scratch/kit-$crate/b/target"
      find "$scratch/kit-$crate" -name .DS_Store -exec rm -f {} +
      (cd "$scratch/kit-$crate" && git diff --no-index --no-color --no-ext-diff --no-prefix a b >"$root/$file") || true
      echo "$file: $(grep -c '^diff --git' "$file") files"
    done
    cat patches/*.patch >vendor/.applied
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
    if ! diff -r -q -x target -x .DS_Store "$scratch/new/$kit_dir" "vendor/$kit_dir"; then
      echo "vendor/$kit_dir differs from patches/gpui-kit-*-${kit_rev:0:7}.patch applied to a fresh copy; run save or apply" >&2
      status=1
    fi
    [ "$status" -eq 0 ] && echo "every patch applies, and vendor/ matches"
    exit "$status"
    ;;
  *) echo "$usage" >&2; exit 2 ;;
esac
