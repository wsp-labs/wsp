#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Runs a command on this worktree's commit in the box's one fixed build folder, where the cargo targets of the daemon
# and the GPUI app stay warm from one thread to the next. Threads take turns on a lock, since two cargo builds in one
# target wait on each other anyway, and the command runs from the same folder inside it that this one is in.
#   scripts/box-build.sh apps/desktop-gpui/scripts/handin-check.sh
#   scripts/box-build.sh cargo test --locked      (from daemon/, builds daemon/ of the fixed folder)
#   scripts/box-build.sh --stage-daemon           builds this chip's static daemon there, and stages it into this
#                                                 worktree's packages/wspx/daemon/, where pre-review.sh's laws read it
# The fixed folder is wt-build beside the project's worktrees, made on first use; BOX_BUILD_DIR moves it. It holds
# committed work only, so commit first. Set the box up once with scripts/box-warm.sh.
set -eu

if [ $# -eq 0 ]; then
  echo "refused: name the command to run; usage: scripts/box-build.sh <command>..." >&2
  exit 3
fi
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "refused: the fixed folder builds commits, and this worktree has changes that are not in one; commit them first" >&2
  exit 3
fi
export PATH=${CARGO_HOME:-/opt/wsp/cargo}/bin:$PATH
here=$(git rev-parse --show-toplevel)
sha=$(git rev-parse HEAD)
sub=$(git rev-parse --show-prefix)
project=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
build=${BOX_BUILD_DIR:-$project/wt-build}

mkdir -p "$(dirname "$build")"
exec 9> "$build.lock"
if ! flock -w 1 9; then
  echo "waiting for the build folder, held by another thread" >&2
  flock -w 3000 9 || { echo "refused: the build folder stayed held for 50 minutes" >&2; exit 1; }
fi

[ -d "$build" ] || git worktree add --detach "$build" "$sha"
[ "$(git -C "$build" rev-parse --show-toplevel)" = "$(cd "$build" && pwd -P)" ] ||
  { echo "refused: $build is not a worktree of its own, so a checkout there would move the checkout around it" >&2; exit 3; }
git -C "$build" checkout --detach --force --quiet "$sha"

# Cargo leaves the binaries of every earlier build beside the new ones, so a target only grows; past the cap, or
# with under 30 GB left on the disk every thread shares, it goes.
cap_kb=$(( ${BOX_TARGET_CAP_GB:-120} * 1024 * 1024 ))
for target in "$build/apps/desktop-gpui/target" "$build/daemon/target"; do
  if [ -d "$target" ] && { [ "$(du -sk "$target" | cut -f1)" -gt "$cap_kb" ] ||
    [ "$(df -Pk "$target" | awk 'NR == 2 { print $4 }')" -lt $((30 * 1024 * 1024)) ]; }; then
    echo "$target passed ${BOX_TARGET_CAP_GB:-120} GB, or the disk has under 30 GB free, so it starts again from nothing" >&2
    rm -rf "$target"
  fi
done

gpui=$build/apps/desktop-gpui
if git -C "$build" cat-file -e HEAD:apps/desktop-gpui/Cargo.toml 2>/dev/null; then
  PATH=/opt/zig/zig-$(uname -m)-linux-$(sed -n 's/^zig_wanted=//p' "$gpui/scripts/ghostty.sh"):$PATH
  # vendor/ is made from patches/ and the script that reads them, and only a change to either makes it again.
  want=$(git -C "$build" rev-parse "HEAD:apps/desktop-gpui/patches" "HEAD:apps/desktop-gpui/scripts/gpui-patches.sh" | tr '\n' ' ')
  (
    cd "$gpui"
    scripts/ghostty.sh
    if [ ! -d vendor ] || [ "$(cat target/.box-patches 2>/dev/null)" != "$want" ]; then
      scripts/gpui-patches.sh apply
      mkdir -p target
      echo "$want" > target/.box-patches
    fi
  )
fi

stage=
if [ "$1" = --stage-daemon ]; then
  stage=$(uname -m)-unknown-linux-musl
  sub=
  # shellcheck disable=SC2016
  set -- sh -c 'cd daemon && ./scripts/libseccomp-archive.sh musl && LIBSECCOMP_LIB_PATH=$PWD/target/libseccomp/musl cargo build --locked --release --target "$1" -p wsp-daemon-bin $(node ../packages/wspx/scripts/daemon-features.mjs "$1")' sh "$stage"
fi

unset CARGO_TARGET_DIR
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
cd "$build/$sub"
# The box's two cargo slots, when it has them; the child lets go of the folder's lock so nothing it starts can hold it.
if [ -x /root/cargo-slot.sh ]; then set -- /root/cargo-slot.sh "$@"; fi
"$@" 9>&-
if [ -n "$stage" ]; then
  (cd "$here" && node packages/wspx/scripts/daemon-binary.mjs --triple "$stage" --from "$build/daemon/target/$stage/release/wsp-daemon")
fi
