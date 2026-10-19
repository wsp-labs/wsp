#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Our patches over the published gpui-pre crates (backdrop blur, rounded clipping of children, text placement, list
# heights kept across a width change, an X11 window's ground before its first frame) and over gpui-kit's pinned commit
# (text placement in its input and shimmer, the ink a disabled input draws its text at), kept as diffs per crate or
# repository in patches/
# and applied to fresh copies in vendor/, which the [patch] tables in Cargo.toml point at. A folder's diffs are
# patches/<folder>.patch, then any patches/<folder>.<topic>.patch in name order; one folder's diffs touch different files.
#   apply  unpack each pinned .crate (checksum-verified) and gpui-kit's commit into vendor/ and apply each patch; run after a fresh clone
#          and after a pull that changes patches/ (vendor/.applied keeps the patches it applied, for dev.sh to compare)
#   save   rewrite patches/ from your edits under vendor/; save patches/<folder>.<topic>.patch writes that folder's
#          edits into the topic file instead of patches/<folder>.patch
#   check  apply into a scratch folder and fail if a patch no longer applies or vendor/ differs from the result
set -eu

usage='usage: apps/desktop-gpui/scripts/gpui-patches.sh apply|save [patches/<folder>.<topic>.patch]|check'
cd "$(dirname "$0")/.."
root=$(pwd)

# name version sha256, the sums Cargo.lock held for crates.io's copies before the patch replaced them.
crates='gpui-pre 0.3.9 34f61c1cfd36fc9c436af09308c963822ae27a45a84feb21a77ba5c9e051f1c0
gpui-pre-wgpu 0.3.9 5b1cdb26c1503bf9662ee8fbc5ab0b44b2deeee4b5c12dc67491ba6fb5fc544b
gpui-pre-apple 0.3.9 b52f58893fe1a0836464428dfad2b5eaf42c9c303a9638f4c2aced5a69072a15
gpui-pre-linux 0.3.9 e28213c3fde6f13e91ba0b74b90cbaadec9373e36a031eda98561d31059870f3'
# gpui-kit, a git dependency pinned by its commit, whose hash is its checksum.
kit_url=https://github.com/longbridge/gpui-kit
kit_rev=14f59330a4aa0cb5becf140e067d64a616328612
kit=gpui-kit-${kit_rev:0:7}
# Once [patch] points at vendor/, app/Cargo.toml's rev no longer decides which gpui-kit builds; kit_rev and the [patch]
# paths do. All three must agree, or a bump made in one place keeps building the old copy.
pinned() {
  local revs paths
  revs=$(grep -o 'longbridge/gpui-kit", rev = "[0-9a-f]*"' app/Cargo.toml | grep -o '[0-9a-f]\{40\}' | sort -u)
  paths=$(grep -o 'vendor/gpui-kit-[0-9a-f]*/' Cargo.toml | sort -u)
  [ "$revs" = "$kit_rev" ] && [ "$paths" = "vendor/$kit/" ] || {
    echo "refused: app/Cargo.toml pins gpui-kit ${revs:-at no rev} and Cargo.toml patches it from ${paths:-nowhere}; this script vendors $kit_rev into vendor/$kit/" >&2
    exit 1
  }
}
# gpui-pre-apple's build script reads these gpui-pre files from its own vendor/gpui to generate the Metal header.
apple_mirror='src/scene.rs src/geometry.rs src/color.rs src/window.rs src/platform.rs'

# A GIT_DIR from a hook or a linked worktree would point every git below at another repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

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

# Prints a git repository whose HEAD is gpui-kit's pinned commit: cargo's own checkout, or a fetch into vendor/.kit.
fetch_kit() {
  local cached
  for cached in "${CARGO_HOME:-$HOME/.cargo}"/git/checkouts/gpui-kit-*/"${kit_rev:0:7}" vendor/.kit; do
    if [ "$(git -C "$cached" rev-parse HEAD 2>/dev/null)" = "$kit_rev" ]; then
      echo "$cached"
      return
    fi
  done
  rm -rf vendor/.kit
  { git init -q vendor/.kit && git -C vendor/.kit fetch -q --depth 1 "$kit_url" "$kit_rev" && git -C vendor/.kit update-ref --no-deref HEAD FETCH_HEAD; } >&2 || {
    echo "refused: could not fetch gpui-kit $kit_rev from $kit_url, and cargo's git cache has no checkout of it" >&2
    exit 1
  }
  echo vendor/.kit
}

# Writes gpui-kit's pinned commit, exactly as committed, into the folder $1.
unpack_kit() {
  local repo
  repo=$(fetch_kit)
  mkdir -p "$1"
  git -C "$repo" archive "$kit_rev" | tar -x -C "$1"
}

# The files under $1 with their executable bit set, which save records as a mode and diff -r does not compare.
executables() {
  (cd "$1" && find . -name target -prune -o -type f -perm -u+x -print | sort)
}

# Every folder vendor/ holds, one per line.
folders() {
  while read -r name version _; do echo "$name-$version"; done <<<"$crates"
  echo "$kit"
}

# Applies the diffs for the folder $2 to its fresh copy $1, every one but $3.
patch_into() {
  local file
  [ -f "patches/$2.patch" ] || { echo "refused: patches/$2.patch is missing" >&2; exit 1; }
  for file in "patches/$2.patch" "patches/$2".*.patch; do
    if [ ! -s "$file" ] || [ "$file" = "${3:-}" ]; then continue; fi
    # Under a git worktree (a TMPDIR inside one), git apply skips every path outside it and still succeeds.
    (cd "$1" && GIT_CEILING_DIRECTORIES="$(dirname "$1")" git apply -p1 --whitespace=nowarn "$root/$file") || {
      echo "refused: $file no longer applies to $2" >&2
      exit 1
    }
  done
}

# Unpacks fresh, patched copies of every crate into $1.
build() {
  local out=$1 name version sum dir file crate
  pinned
  mkdir -p "$out"
  while read -r name version sum; do
    dir="$name-$version"
    # An assignment, so a refused fetch stops the script under set -e instead of handing tar an empty path.
    crate=$(fetch "$name" "$version" "$sum")
    tar -xzf "$crate" -C "$out"
    patch_into "$out/$dir" "$dir"
  done <<<"$crates"
  unpack_kit "$out/$kit"
  patch_into "$out/$kit" "$kit"
  for file in $apple_mirror; do
    cp "$out/gpui-pre-0.3.9/$file" "$out/gpui-pre-apple-0.3.9/vendor/gpui/$file"
  done
}

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT INT TERM HUP

case "${1:-}" in
  apply)
    build "$scratch/new"
    rm -f vendor/.applied
    for dir in $(folders); do
      rm -rf "vendor/$dir"
      mv "$scratch/new/$dir" "vendor/$dir"
    done
    cat patches/*.patch >vendor/.applied
    echo "applied patches/ to fresh copies in vendor/"
    ;;
  save)
    pinned
    if [ -n "${2:-}" ]; then
      topic=
      for dir in $(folders); do
        case $2 in "patches/$dir".*.patch) topic=$2 ;; esac
      done
      [ -n "$topic" ] || { echo "refused: $2 is not patches/<folder>.<topic>.patch for a folder in vendor/" >&2; exit 2; }
    fi
    for file in $apple_mirror; do
      cp "vendor/gpui-pre-0.3.9/$file" "vendor/gpui-pre-apple-0.3.9/vendor/gpui/$file"
    done
    while read -r name version sum; do
      mkdir -p "$scratch/$name"
      crate=$(fetch "$name" "$version" "$sum")
      tar -xzf "$crate" -C "$scratch/$name"
      mv "$scratch/$name/$name-$version" "$scratch/$name/a"
    done <<<"$crates"
    unpack_kit "$scratch/$kit/a"
    for dir in $(folders); do
      name=${dir%-*}
      [ "$dir" = "$kit" ] && name=$kit
      out=patches/$dir.patch
      case ${2:-} in "patches/$dir".*.patch) out=$2 ;; esac
      patch_into "$scratch/$name/a" "$dir" "$out"
      cp -R "vendor/$dir" "$scratch/$name/b"
      rm -rf "$scratch/$name/b/vendor/gpui" "$scratch/$name/a/vendor/gpui"
      find "$scratch/$name" -name target -prune -exec rm -rf {} + -o -name .DS_Store -exec rm -f {} +
      (cd "$scratch/$name" && git diff --no-index --no-color --no-ext-diff --no-prefix a b >"$root/$out") || true
      echo "$out: $(grep -c '^diff --git' "$out") files"
    done
    cat patches/*.patch >vendor/.applied
    ;;
  check)
    build "$scratch/new"
    status=0
    for dir in $(folders); do
      if ! diff -r -q -x target -x .DS_Store "$scratch/new/$dir" "vendor/$dir" ||
        [ "$(executables "$scratch/new/$dir")" != "$(executables "vendor/$dir")" ]; then
        echo "vendor/$dir differs from patches/$dir.patch applied to a fresh copy; run save or apply" >&2
        status=1
      fi
    done
    [ "$status" -eq 0 ] && echo "every patch applies, and vendor/ matches"
    exit "$status"
    ;;
  *) echo "$usage" >&2; exit 2 ;;
esac
