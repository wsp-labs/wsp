#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# The checks a GPUI lane branch passes before hand-in, run from its worktree. Every check runs; each failure names
# its fix, and the exit code is 1 if any failed. --no-rust skips cargo for a quick run off Boat.
set -u

usage='usage: apps/desktop-gpui/scripts/handin-check.sh [--no-rust]'
rust=1
for arg in "$@"; do
  case "$arg" in
    --no-rust) rust=0 ;;
    -h|--help) echo "$usage"; exit 0 ;;
    *) echo "refused: $arg is not a flag this script takes; $usage" >&2; exit 2 ;;
  esac
done

root=$(git rev-parse --show-toplevel) && cd "$root" || exit 2
base=origin/gpui/main
failed=0
ok() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n      fix: %s\n' "$1" "$2"; failed=$((failed + 1)); }
skip() { printf 'skip  %s\n' "$1"; }

printf 'handin-check on %s at %s\n' "$(git rev-parse --abbrev-ref HEAD)" "$(git rev-parse HEAD)"

shallow=$(git rev-parse --is-shallow-repository)
if [ "$shallow" = true ]; then
  fail "the clone is shallow, so an amend of its oldest commit drops that commit's parent" "git fetch --unshallow origin"
else
  ok "the clone is not shallow"
fi

if ! git fetch -q origin +refs/heads/gpui/main:refs/remotes/origin/gpui/main; then
  echo "FAIL  could not fetch gpui/main from origin, so nothing below has a base to compare with" >&2
  exit 1
fi
base_sha=$(git rev-parse "$base")

dirty=$(git status --porcelain)
if [ -n "$dirty" ]; then
  fail "the worktree has changes outside the commit, so the checks below would test them and not the commit:
$(printf '%s\n' "$dirty" | sed 's/^/        /')" "commit them with --amend, or remove them (an untracked .cargo/config.toml counts)"
else
  ok "the worktree is clean, so the checks below test the commit"
fi

mb=$(git merge-base "$base" HEAD 2>/dev/null) || mb=
count=$(git rev-list --count "$base..HEAD")
parent=$(git rev-parse -q --verify 'HEAD^') || parent=
if [ -z "$parent" ] && [ "$shallow" = true ]; then
  skip "the commit's history: the shallow clone hides HEAD's parent; unshallow and run this again"
elif [ -z "$parent" ]; then
  fail "HEAD has no parent line, so GitHub sees no history in common with gpui/main and closes the PR" \
    "put the tree back on the commit it was built on (the parent of the commit before the amend, in git reflog): git reset --soft <that commit> && git commit -C ORIG_HEAD && git rebase $base"
elif [ "$count" -ne 1 ]; then
  fail "the branch is $count commits on $base, not one" "git reset --soft ${mb:-$base} && git commit, with one message for the whole change"
elif [ "$parent" != "$base_sha" ]; then
  fail "the commit's parent is ${parent:0:9}, not $base at ${base_sha:0:9}" "git rebase $base"
else
  ok "one commit on $base (${base_sha:0:9}), with its parent line"
fi

em=$(printf '\342\200\224')
message=$(mktemp)
git log -1 --format=%B HEAD > "$message"
if refused=$(sh .githooks/commit-msg "$message" 2>&1); then
  ok "the commit message passes the repo's commit-msg hook"
else
  fail "the commit message: ${refused#refused: }" "git commit --amend with that fixed"
fi
rm -f "$message"

if [ -n "$mb" ]; then range=("$mb" HEAD); else range=("$base" HEAD); fi
# A folder with an UPSTREAM file is a byte-for-byte copy of another repo, so its lines are not ours to judge.
vendored=()
for upstream in .claude/skills/*/UPSTREAM; do
  [ -f "$upstream" ] && vendored+=(":(exclude)${upstream%/UPSTREAM}")
done
added() {
  git diff -U0 --no-color --no-ext-diff "${range[@]}" -- . ${vendored[@]+"${vendored[@]}"} "$@" |
    awk '/^\+\+\+ /{file = substr($0, 7); next} /^\+/{print file ": " substr($0, 2)}'
}

hits=$(added | grep -F "$em")
if [ -n "$hits" ]; then
  fail "added lines have an em dash:
$(printf '%s\n' "$hits" | sed 's/^/        /')" "rewrite each with a comma or a period"
else
  ok "no added line has an em dash"
fi

hits=$(added ':(exclude)*.md' ':(exclude)apps/desktop-gpui/scripts/handin-check.sh' |
  grep -E '(^|[^[:alnum:]_-])(pkill|killall|pgrep)([^[:alnum:]_-]|$)')
if [ -n "$hits" ]; then
  fail "added lines find or stop processes by pattern:
$(printf '%s\n' "$hits" | sed 's/^/        /')" "record the pid of each process you start and stop that pid, only while it is still your child"
else
  ok "no added line finds or stops processes by pattern (pkill, killall, pgrep)"
fi

# host-client builds against daemon/crates/wsp-mcp by path, and tests/record.rs checks packages/protocol.
rust_inputs=(apps/desktop-gpui daemon/crates packages/protocol/src
  ':(exclude)apps/desktop-gpui/bench' ':(exclude)apps/desktop-gpui/scripts' ':(exclude)*.md')
if [ -z "$(git diff --name-only "${range[@]}" -- "${rust_inputs[@]}")" ]; then
  skip "cargo: nothing the Rust build or its tests read changed"
elif [ "$rust" -eq 0 ]; then
  skip "cargo fmt, clippy and test: --no-rust was given; a hand-in runs them on Boat"
else
  for step in "fmt --check" "clippy --locked --all-targets -- -D warnings" "test --locked"; do
    # shellcheck disable=SC2086
    if (cd apps/desktop-gpui && cargo $step); then
      ok "cargo $step in apps/desktop-gpui"
      continue
    fi
    case "$step" in
      fmt*) fix="run cargo fmt in apps/desktop-gpui and amend" ;;
      clippy*) fix="fix each warning printed above and amend" ;;
      *) fix="read the failure above; live.rs needs pnpm install --frozen-lockfile && pnpm -r build first" ;;
    esac
    fail "cargo $step in apps/desktop-gpui" "$fix"
  done
fi

if pnpm exec vitest run --minWorkers=1 --maxWorkers=2 packages/host/test/gpui-skills.test.ts; then
  ok "the gpui-skills test"
else
  fail "the gpui-skills test" "read the failure above; with no node_modules, run pnpm install --frozen-lockfile first"
fi

if [ "$failed" -gt 0 ]; then
  printf '\nhandin-check: %d failed. Fix each, amend, and run it again before hand-in.\n' "$failed"
  exit 1
fi
printf '\nhandin-check: every check passed at %s. Paste this whole output into the report.\n' "$(git rev-parse --short HEAD)"
