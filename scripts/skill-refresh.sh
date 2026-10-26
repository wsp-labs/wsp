#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Replaces a vendored skill folder with the upstream copy its UPSTREAM file names,
# at the commit given or the upstream default branch, and records the commit taken.
#   scripts/skill-refresh.sh .claude/skills/gpui-kit [commit]
set -euo pipefail
dir=${1:?usage: scripts/skill-refresh.sh <skill folder> [commit]}
repo=$(sed -n 's/^repo=//p' "$dir/UPSTREAM")
path=$(sed -n 's/^path=//p' "$dir/UPSTREAM")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
git -C "$tmp" init -q
git -C "$tmp" fetch -q --depth 1 "$repo" "${2:-HEAD}"
git -C "$tmp" checkout -q FETCH_HEAD
test -f "$tmp/$path/SKILL.md"
find "$dir" -mindepth 1 ! -name UPSTREAM -delete
cp -R "$tmp/$path/." "$dir/"
printf 'repo=%s\npath=%s\ncommit=%s\n' "$repo" "$path" "$(git -C "$tmp" rev-parse HEAD)" > "$dir/UPSTREAM"
