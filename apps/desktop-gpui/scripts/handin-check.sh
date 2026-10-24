#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# The checks a GPUI lane branch passes before hand-in, run from its worktree. Every check runs; each failure names
# its fix, and the exit code is 1 if any failed. --no-rust skips cargo for a quick run off Boat; --commits N is for a
# hand-in the coordinator ruled N commits.
set -u

usage='usage: apps/desktop-gpui/scripts/handin-check.sh [--no-rust] [--commits N]'
rust=1
commits=1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-rust) rust=0 ;;
    --commits) commits=${2-}; shift ;;
    -h|--help) echo "$usage"; exit 0 ;;
    *) echo "refused: $1 is not a flag this script takes; $usage" >&2; exit 2 ;;
  esac
  shift
done
case "$commits" in
  '' | *[!0-9]* | 0*) echo "refused: --commits takes a count of 1 or more; $usage" >&2; exit 2 ;;
esac

root=$(git rev-parse --show-toplevel) && cd "$root" || exit 2
base=origin/gpui/main
failed=0
ok() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n      fix: %s\n' "$1" "$2"; failed=$((failed + 1)); }
skip() { printf 'skip  %s\n' "$1"; }
indent() { printf '%s\n' "$1" | sed 's/^/        /'; }

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

# wsp send --file drops the coordinator's attachments in .wsp-files/, which no commit carries.
dirty=$(git status --porcelain -- . ':(exclude).wsp-files')
attached=
[ -d .wsp-files ] && attached="; .wsp-files/, the coordinator's attachments, is not counted"
if [ -n "$dirty" ]; then
  fail "the worktree has changes outside the commit, so the checks below would test them and not the commit:
$(indent "$dirty")" "commit them with --amend, or remove them (an untracked .cargo/config.toml counts)"
else
  ok "the worktree is clean, so the checks below test the commit$attached"
fi

mb=$(git merge-base "$base" HEAD 2>/dev/null) || mb=
# The branch's own commits, first parents only, where a merge of origin/main brings main's commits and counts for none.
git fetch -q origin +refs/heads/main:refs/remotes/origin/main || echo "could not fetch main from origin, so every merge counts as a commit" >&2
line=$(git rev-list --first-parent "$base..HEAD")
count=0
merged_main=0
for sha in $line; do
  others=$(git rev-list --no-walk --parents "$sha" | cut -s -d' ' -f3-)
  from_main=1
  [ -n "$others" ] || from_main=0
  for p in $others; do git merge-base --is-ancestor "$p" origin/main 2>/dev/null || from_main=0; done
  if [ "$from_main" -eq 1 ]; then merged_main=1; else count=$((count + 1)); fi
done
if [ "$commits" -eq 1 ]; then
  wanted="one commit"
else
  wanted="$commits commits, as the coordinator ruled (--commits $commits),"
fi
parent=$(git rev-parse -q --verify "$(printf '%s\n' "$line" | tail -n 1)^") || parent=
if [ -z "$parent" ] && [ "$shallow" = true ]; then
  skip "the commit's history: the shallow clone hides the oldest commit's parent; unshallow and run this again"
elif [ "$count" -ne "$commits" ]; then
  fail "commits on $base, a merge of origin/main not counted: $count, not $commits" \
    "git reset --soft ${mb:-$base} && git commit, with one message for the whole change (then merge origin/main again if the branch held it), unless the coordinator ruled otherwise (--commits N)"
elif [ -z "$parent" ]; then
  fail "the oldest commit has no parent line, so GitHub sees no history in common with gpui/main and closes the PR" \
    "put the tree back on the commit it was built on (the parent of the commit before the amend, in git reflog): git reset --soft <that commit> && git commit -C ORIG_HEAD && git rebase $base"
elif [ "$parent" != "$base_sha" ]; then
  fail "the oldest commit's parent is ${parent:0:9}, not $base at ${base_sha:0:9}" "git rebase $base"
else
  ok "$wanted on $base (${base_sha:0:9}), with its parent line"
fi

em=$(printf '\342\200\224')
message=$(mktemp)
refusals=
for sha in $line; do
  git log -1 --format=%B "$sha" > "$message"
  refused=$(sh .githooks/commit-msg "$message" 2>&1) || refusals="$refusals ${sha:0:9}: ${refused#refused: }"
done
rm -f "$message"
if [ -z "$refusals" ]; then
  ok "the commit message passes the repo's commit-msg hook"
else
  fail "the commit message:$refusals" "reword each commit named with that fixed (git commit --amend for HEAD)"
fi

if [ -n "$mb" ]; then range=("$mb" HEAD); else range=("$base" HEAD); fi
# Lines a merge of origin/main brought are main's, so the branch's own are read against gpui/main merged with main.
if [ "$merged_main" -eq 1 ] && both=$(git merge-tree --write-tree "$base" origin/main 2>/dev/null); then range=("$both" HEAD); fi
# A folder with an UPSTREAM file is a byte-for-byte copy of another repo, so its lines are not ours to judge.
vendored=()
for upstream in .claude/skills/*/UPSTREAM; do
  [ -f "$upstream" ] && vendored+=(":(exclude)${upstream%/UPSTREAM}")
done
# Each added line under the pathspecs given, as file:line: text. A file's name is read only in its header, since an
# added line of a .patch can itself begin with ++.
added() {
  git diff -U0 --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ "${range[@]}" -- "$@" ${vendored[@]+"${vendored[@]}"} |
    awk '/^diff --git /{head = 1; next}
      head && /^\+\+\+ /{file = substr($0, 7); next}
      /^@@/{head = 0; split($3, at, ","); line = substr(at[1], 2) + 0; next}
      !head && /^\+/{print file ":" line ": " substr($0, 2); line++}'
}
code=(. ':(exclude)*.md' ':(exclude)apps/desktop-gpui/scripts/handin-check.sh')

hits=$(added . | grep -F "$em")
if [ -n "$hits" ]; then
  fail "added lines have an em dash:
$(indent "$hits")" "rewrite each with a comma or a period"
else
  ok "no added line has an em dash"
fi

hits=$(added "${code[@]}" | grep -E '(^|[^[:alnum:]_-])(pkill|kill[a]ll|pgrep)([^[:alnum:]_-]|$)')
if [ -n "$hits" ]; then
  fail "added lines find or stop processes by pattern:
$(indent "$hits")" "record the pid of each process you start and stop that pid, only while it is still your child"
else
  ok "no added line finds or stops processes by pattern (pkill, kill""all, pgrep)"
fi

# In a shell file, the command alone, or into a pipe or a file, where a command starts: each prints every value.
# shellcheck disable=SC2016
dump='^[^:]+\.(sh|bash|zsh):[0-9]+: (.*([;&|{`]|\$\(|(then|do|else|if|while|!|exec|command|sudo) ))?[[:space:]]*'
dump+='(env|set|export|export[[:space:]]+-p|(declare|typeset)[[:space:]]+-[px])[[:space:]]*($|[|>;&)`#]|[0-9]>)'
hits=$(added "${code[@]}" | grep -E -e "$dump" -e '(^|[^[:alnum:]_-])printenv([^[:alnum:]_-]|$)' -e '/proc/[^[:space:]]*/environ')
if [ -n "$hits" ]; then
  fail "added lines print the environment, which holds the owner's tokens:
$(indent "$hits")" "list names only (bash -c 'compgen -e'); a test compares a child's environment by names, never values"
else
  ok "no added line prints the environment (env, printenv, export -p, a bare set or export, /proc/*/environ)"
fi

hits=$(added apps/desktop-gpui/app/tests apps/desktop-gpui/parity | grep -F deviceScaleFactor)
if [ -n "$hits" ]; then
  fail "added lines under app/tests or parity use Playwright's emulated deviceScaleFactor, where Blink rounds font metrics in CSS px:
$(indent "$hits")" "launch Chrome with --force-device-scale-factor=2 instead"
else
  ok "no added line under app/tests or parity uses Playwright's deviceScaleFactor"
fi

# HOME from the caller's, all of process.env passed on, or HOME in a list of names copied from the caller's.
hits=$(added apps/desktop-gpui/app/tests | grep -E \
  -e "HOME[\"']?[[:space:]]*[:=][[:space:]]*[\"'\`]?(process\.env(\.HOME|\[[\"']HOME)|\\$\{?HOME)" \
  -e '\.\.\.process\.env([^.[:alnum:]_]|$)' -e 'env:[[:space:]]*process\.env([^.[:alnum:]_]|$)' \
  -e '\.env\("HOME",.*var(_os)?\("HOME"\)' -e "\\[[^]]*[\"']HOME[\"'][^]]*\\].*(process\.env\\[|var(_os)?\\()")
if [ -n "$hits" ]; then
  fail "added lines under app/tests hand the caller's HOME to a child, so the app or Chrome reads and writes the person's own:
$(indent "$hits")" "give each child a HOME in the run's temp folder and nothing else of the caller's, as composer/web/shots.mjs does"
else
  ok "no added line under app/tests hands the caller's HOME to a child"
fi

# CI=true, or pnpm stops to ask on a terminal nobody is at. The tests read the host's, the web app's and protocol's builds.
build=(--filter '@wsp/host...' --filter '@wsp/web...' build)
if (export CI=true && pnpm install --frozen-lockfile && pnpm "${build[@]}"); then
  ok "pnpm install --frozen-lockfile and pnpm ${build[*]}, with CI=true so pnpm never asks"
else
  fail "pnpm install --frozen-lockfile and pnpm ${build[*]}" "read the failure above"
fi

# Stops the sentinel host, only while it is still this shell's child.
sentinel=
sentinel_pid=
stop_sentinel() {
  if [ -n "$sentinel_pid" ] && [ "$(ps -o ppid= -p "$sentinel_pid" | tr -d ' ')" = "$$" ]; then
    kill "$sentinel_pid" 2>/dev/null
    wait "$sentinel_pid" 2>/dev/null
  fi
  sentinel_pid=
  [ -n "$sentinel" ] && rm -rf "$sentinel"
  sentinel=
}
trap stop_sentinel EXIT

# A HOME whose one host on the account is a port this script listens on, so a test that reads ~/.wsp dials it.
start_sentinel() {
  sentinel=$(mktemp -d) || return 1
  mkdir -p "$sentinel/.wsp/hosts"
  node -e 'const fs = require("fs"), [port, log] = process.argv.slice(1);
    const server = require("net").createServer(socket => { fs.appendFileSync(log, "dial\n"); socket.destroy(); });
    server.listen(0, "127.0.0.1", () => fs.writeFileSync(port, String(server.address().port)));' \
    "$sentinel/port" "$sentinel/dialed" &
  sentinel_pid=$!
  for _ in $(seq 50); do [ -s "$sentinel/port" ] && break; sleep 0.1; done
  [ -s "$sentinel/port" ] || return 1
  sentinel_url="ws://127.0.0.1:$(cat "$sentinel/port")"
  # wsp-mcp refuses an account record with no hostKey before it dials, so without one nothing would ever reach us.
  printf '{"url":"%s","deviceId":"sentinel","deviceToken":"sentinel","hostKey":"sentinel","via":{"kind":"account","hostId":"sentinel"}}\n' \
    "$sentinel_url" > "$sentinel/.wsp/hosts/sentinel.json"
}

# Runs a command with HOME at the sentinel and the variables that aim a command line aimed at it too.
in_sentinel() {
  env -u WSP_HOME -u WSP_HOST_KEY HOME="$sentinel" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" WSP_HOST="$sentinel_url" WSP_HOST_URL="$sentinel_url" \
    WSP_HOST_TOKEN=sentinel "$@"
}

# CARGO_TARGET_DIR as cargo run from apps/desktop-gpui reads it, resolved through its nearest folder that exists.
target=${CARGO_TARGET_DIR-}
if [ -n "$target" ]; then
  [ "${target#/}" = "$target" ] && target="$root/apps/desktop-gpui/$target"
  rest=
  while [ ! -d "$target" ]; do rest="/$(basename "$target")$rest"; target=$(dirname "$target"); done
  target="$(cd "$target" && pwd -P)$rest"
  case "$rest" in */..*) target=/ ;; esac
fi

# host-client builds against daemon/crates/wsp-mcp by path, and tests/record.rs checks packages/protocol.
rust_inputs=(apps/desktop-gpui daemon/crates packages/protocol/src
  ':(exclude)apps/desktop-gpui/bench' ':(exclude)apps/desktop-gpui/scripts/handin-check.sh' ':(exclude)*.md')
if [ -z "$(git diff --name-only "${range[@]}" -- "${rust_inputs[@]}")" ]; then
  skip "cargo: nothing the Rust build or its tests read changed"
elif [ "$rust" -eq 0 ]; then
  skip "cargo fmt, clippy and test: --no-rust was given; a hand-in runs them on Boat"
elif [ -n "$target" ] && [ "${target#"$(pwd -P)"/}" = "$target" ]; then
  fail "refused to run cargo: CARGO_TARGET_DIR is $CARGO_TARGET_DIR, outside this worktree. Cargo knows the crates in vendor/ by their path inside the workspace and judges freshness by file times, so another worktree's patched crates build here as Fresh and the tests run the wrong code" \
    "unset CARGO_TARGET_DIR, or point it inside $(pwd -P), and run this again"
else
  # vendor/ is generated and gitignored, so a fresh clone has none until apply.
  [ -d apps/desktop-gpui/vendor ] || apps/desktop-gpui/scripts/gpui-patches.sh apply ||
    fail "gpui-patches.sh apply, which makes apps/desktop-gpui/vendor before cargo" "read the error above"
  for step in "fmt --check" "clippy --locked --all-targets -- -D warnings"; do
    # shellcheck disable=SC2086
    if (cd apps/desktop-gpui && cargo $step); then
      ok "cargo $step in apps/desktop-gpui"
    elif [ "$step" = "fmt --check" ]; then
      fail "cargo $step in apps/desktop-gpui" "run cargo fmt in apps/desktop-gpui and amend"
    else
      fail "cargo $step in apps/desktop-gpui" "fix each warning printed above and amend"
    fi
  done
  if ! start_sentinel; then
    fail "the sentinel host for cargo test did not start listening" "read the error above; it needs node"
  else
    stage=apps/desktop-gpui/scripts/stage-daemon.sh
    # Where common::wsp_daemon in host-client/tests looks when nothing staged one, in its order, for this platform.
    arch=$(uname -m)
    [ "$arch" = arm64 ] && arch=aarch64
    if [ "$(uname -s)" = Darwin ]; then triple=$arch-apple-darwin; else triple=$arch-unknown-linux-musl; fi
    if [ -n "${WSP_TEST_DAEMON-}" ]; then
      tried=("$WSP_TEST_DAEMON")
    else
      tried=("packages/wspx/daemon/$triple/wsp-daemon" "/root/wsp-daemon/wsp/assets/daemon/$triple/wsp-daemon")
    fi
    daemon=
    for found in "${tried[@]}"; do [ -f "$found" ] && daemon=$found && break; done
    if [ -x "$stage" ]; then
      if in_sentinel "$stage"; then
        ok "$stage staged a wsp-daemon, so the tests that need one ran"
      else
        fail "$stage did not stage a wsp-daemon" "read the error above"
      fi
    elif [ -n "$daemon" ]; then
      ok "a wsp-daemon is at $daemon, where the tests look, so the tests that need one ran"
    else
      skip "daemon tests: skipped: no daemon staged ($stage is not on gpui/main yet, and none is at WSP_TEST_DAEMON, packages/wspx/daemon or /root/wsp-daemon), so composer_live passed without its daemon checks"
    fi
    if (cd apps/desktop-gpui && in_sentinel cargo test --locked); then
      ok "cargo test --locked in apps/desktop-gpui, with HOME at a sentinel host record and WSP_HOST aimed at it"
    else
      fail "cargo test --locked in apps/desktop-gpui, with HOME at a sentinel host record" \
        "read the failure above; a test that passes with your own HOME and fails here reads the person's home"
    fi
    if [ -s "$sentinel/dialed" ]; then
      fail "a test dialed the sentinel host in \$HOME/.wsp/hosts ($(grep -c . "$sentinel/dialed") dials), so it reads the person's ~/.wsp or WSP_HOST and would reach their real host" \
        "give the test a HOME of its own in a temp folder, and env_clear every child it starts"
    else
      ok "no test dialed the sentinel host, so none reads the person's ~/.wsp or WSP_HOST"
    fi
  fi
  stop_sentinel
fi

# main's laws and size check, on this branch merged with origin/main; their contract test runs a staged wsp-daemon.
staged=packages/wspx/daemon/$(uname -m)-unknown-linux-musl/wsp-daemon
if [ "$rust" -eq 1 ] && [ "$(uname -s)" = Linux ] && [ ! -f "$staged" ] && [ -x apps/desktop-gpui/scripts/stage-daemon.sh ]; then
  apps/desktop-gpui/scripts/stage-daemon.sh || fail "apps/desktop-gpui/scripts/stage-daemon.sh, for the laws' contract test" "read the error above"
fi
if scripts/pre-review.sh --laws; then
  ok "scripts/pre-review.sh --laws: the repo's laws and scripts/file-size-check.mjs"
else
  fail "scripts/pre-review.sh --laws: the repo's laws or scripts/file-size-check.mjs" \
    "fix each FAIL printed above (a file over 1500 lines is split, never added to scripts/file-size-known.json); the log folder it names holds each failing test's output"
fi

if CI=true pnpm exec vitest run --minWorkers=1 --maxWorkers=2 packages/host/test/gpui-skills.test.ts packages/host/test/gpui-package.test.ts; then
  ok "the gpui-skills and gpui-package tests"
else
  fail "the gpui-skills and gpui-package tests" "read the failure above"
fi

if [ "$failed" -gt 0 ]; then
  printf '\nhandin-check: %d failed. Fix each, amend, and run it again before hand-in.\n' "$failed"
  exit 1
fi
printf '\nhandin-check: every check passed at %s. Paste this whole output into the report.\n' "$(git rev-parse --short HEAD)"
