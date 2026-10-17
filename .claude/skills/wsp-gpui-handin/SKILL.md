---
name: wsp-gpui-handin
description: What cold reviews in wsp's GPUI lane (map wsp-map#2364, branch gpui/main) keep finding, one section per kind of mistake with the real case and the check that catches it before hand-in, and the script every lane branch passes first. Load it before handing in or calling ready any GPUI lane branch, and before reviewing one, together with wsp-gpui and wsp-review.
---

# Handing in GPUI lane work

On 2026-10-17 the lane ran four cold reviews: wsp#1591 (#2365), the fork spikes (#2366), wsp#1595 (#2367) and
wsp#1592 (#2368). With the five builder reports and the rulings on each ticket, they found 31 problems, and most of
them were the same few kinds. The kinds are below, the most frequent first, each with its count. A finding is
named by ticket and the review's finding number, so "#2367 finding 3" is finding 3 of the review of #2367.

Reviews are now grouped, one reviewer for several branches. A branch comes in already passing what a reviewer
would check by hand.

## Before hand-in

1. On Boat, from the branch's worktree, after the last amend, run
   [handin-check.sh](../../../apps/desktop-gpui/scripts/handin-check.sh) with no flags.
2. Paste its whole output into the report. Its first line names the commit; it must be the sha you pushed.
3. Go through the sections below for what no script can see, and say in the report what you ran for each.
4. Say in the report that the self-review ran and what it caught (wsp-review, Process laws).

`--no-rust` skips cargo, for a quick run on the owner's Mac where a GPUI build loads the machine. It is never the
run a report quotes when the run without it would have run cargo.

### What the script checks, so a reviewer does not

- the clone is not shallow;
- the worktree is clean, so every check ran on the commit and not on a local edit or an untracked
  `.cargo/config.toml`;
- the branch is one commit whose parent is a fresh fetch of `origin/gpui/main`, and the commit has a parent line;
- the commit message passes `.githooks/commit-msg`: no em or en dash, no co-author trailer, no ticket number;
- no added line has an em dash, and no added line outside Markdown names `pkill`, `killall` or `pgrep` (folders
  holding an `UPSTREAM` file are vendored copies and skipped by both);
- if anything cargo builds or its tests read changed (`apps/desktop-gpui` but its `bench` and `scripts`,
  `daemon/crates`, `packages/protocol/src`), these three in `apps/desktop-gpui`: `cargo fmt --check`,
  `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked`;
- the gpui-skills test, which also fails when a file a skill links to is gone.

A reviewer handed this output, every line `ok` or `skip`, for the PR's head sha does not rerun any of it. Output
for another sha, or run with `--no-rust` on a Rust change, is not a pass. Everything below stays the reviewer's.

## 1. A claim that does not match its source (13)

Most review time went here: a guide line or a report number said something its cited file, PR or command did not.

- #2365 finding 1: the guide's "Effects upstream are gone" read as Zed refusing effects. The cited PRs were closed
  for a limit of three open PRs with a request for smaller ones; #63794 by its own author; and #43273 moved to a
  maintainer's tracker, where it is open.
- #2365 finding 3: "A pinned fork as a git dependency is allowed" had no source. It came from the coordinator's
  brief, not the map, and got ahead of the fork ruling.
- #2367 finding 4: the guide named `gpui_kit::base` and `#[gpui_kit::test]`. The vendored gpui-kit skills describe
  their `UPSTREAM` commit d4c5681; at our pin ae30c2aa the crate is `gpui_component` and tests use `#[gpui::test]`.
- #2367 finding 5: "Shoot on Boat" said our app draws under Xvfb; the fork section of the same page said it stays
  black.
- #2366 findings 3 and 4: "76 to 82 method clashes" were two method names, and "13 errors is small" was a floor,
  because rustc stops at missing names before it type-checks the code that uses them.
- #2368 findings 3 and 5: the test header said a staged daemon was needed (the host runs without one), and the
  report said an unknown status fails "that row" when it fails the whole `threads()` call.
- Smaller: #2365 finding 2 ("neither was tried on Boat" was out of date; `debug_selector` lives in `div.rs`),
  finding 5 (the size table was wrong for Usage and `CopyRow`), finding 6 (four facts), the #2365 report's
  "9 files, 88 tests" (the reviewer got 1 file, 12 tests), and #2367 finding 10 (17 versions moved, not 18).

Before hand-in:

- For every sentence that cites something, open the source and find the words that back it. If you cannot quote
  them, cut the sentence or mark it as your own reading.
- Read code and docs at our pin, not at upstream `main` and not in the vendored skills, which describe their own
  `UPSTREAM` commit: `git -C <checkout> show <pin>:<path>`, or the checkout under cargo's git folder.
- Every count comes with the command that produced it and that command's last lines. A count from a compiler that
  stopped early is a floor; say so.
- Every rule names where it was ruled: the map, a ticket's ruling. A rule only a brief states is a question for the
  coordinator, not a line in the guide.
- After editing one section, `grep -n` the page for every other mention of the same thing and make them agree.
- Do not state a result wider than you ran it: "that row" against the whole call, "needs X" against "ran with X".

## 2. A test that does not go red for the break it claims (5)

- #2365 finding 4: the skills test passed with `gpui-kit/references/usage.md` deleted, though the acceptance was
  "the skill files exist".
- #2367 finding 3: the layout test stayed green with `items_center` removed, because the title's box stretched to
  full height and its center stayed put. The builder said so in the report, which was right; the fix was one
  assertion that the box is shorter than half the window.
- #2367 finding 6: the test drew `Home` alone while the window draws `Root(Home)` after `gpui_component::init`, so
  it never ran what ships.
- #2368 finding 4: deleting the three lines that drop events already in the history page left the live test green,
  because no event in the test is ever in both.
- #2365 finding 9, declined: the link check skips links with a title, reference links and anchors.

#2365 finding 8 was a new script with no test and no reason given; it was ruled fine, but the reason belonged in
the report.

Before hand-in:

- For each test you add or change, list the breaks it claims to catch. Make each break in the source, run the
  test, paste the red line, restore with `git checkout HEAD -- <file>` once everything is committed, since it drops
  uncommitted edits. One red per claim, not one per test. Never `git stash` (wsp-review, red-proofs).
- A break that stays green is a finding against your own test: add the assertion, or say in the report what the
  test does not catch.
- Test through the path the app takes: the same init, the same wrapping view, the same environment.
- A new script has a test, or a line in the report saying why not and how you ran it by hand.

## 3. Code that works on one platform and not the other (3)

Builders run on Boat: Linux, no GPU, Xvfb and Mesa. The app ships on macOS. Each hides faults from the other.

- #2366 finding 1: zui drew nothing under Xvfb, and the report concluded "no Boat screenshot check on this road".
  The cause was one Zed X11 fix (`f4178619a`) missing from zui's July base.
- #2366 finding 2: zui's wgpu fast path for plain fills skipped the rounded clip. Metal applies the clip first, so
  a Mac run looks right and cannot catch it.
- #2367 finding 1: `dev.sh` restarted the app on any write under `app/`. On a Mac, Finder writes `.DS_Store`
  whenever the folder opens, and vim rewrites its swap file every few seconds while you type.

Before hand-in:

- When something fails on one platform, find the cause before writing "X does not work there". A fix upstream that
  your pin lacks is the usual one; check upstream's history for the failing file and say what you tried.
- Name the platform with every result. List in the report what only the other platform can show: fonts, color,
  Metal, Finder, pid limits.
- For a script that runs on the Mac: no `inotifywait`, BSD `sed -i ''` and `stat`, `.DS_Store`, pids that stop at
  99999, and bash 3.2 behind `/usr/bin/env bash`, where an empty array under `set -u` is an error.

## 4. A measurement that could flatter one side (3)

- #2367 finding 9: the guide's rebuild (1.4 to 1.7 s) and cold build (49 s) did not reproduce on another 8x16
  machine (4.8 to 7.6 s, 3 min 51 s), likely a background snapshot job taking three cores. The guide now records
  both machines.
- #2369, ruling 1: both apps fold finished turns, so the 2,000-row fixture showed about 120 rows and the bench would
  not have tested the gate. The long thread became 900 turns, so both apps show 1,800 messages.
- #2366 report: the cold build table set Road A with its lock against Road B without one, and Road B's 8 min 51 s
  was mostly download. The builder said so under the table, which is how to do it.

#2369 part 1's dry run measured zui with a local patch in an uncommitted `.cargo/config.toml`. The report said so;
the script's clean-tree check now fails a run like that.

Before hand-in:

- Both sides do the same work: the same data, the same rows on screen, the same folding, window size, build profile
  and caches. List every difference you know of next to the numbers.
- Every timed window is focused and in front: GPUI caps an unfocused window at 30 fps and Chrome throttles a hidden
  one (#2369 part 1 review, finding 1, a blocker).
- Time the moment the person sees, such as the first presented frame holding the new thread's rows.
- Take three runs or more and give the spread, with the machine, `nproc` and `uptime`'s load at the time. A number
  nobody else can reproduce is a note, not a result.
- Measure the commit you push: no local patches, no uncommitted config.

## 5. A child process that inherits the caller's environment (2)

- #2368 finding 2: the live test's `git` set only `HOME`. With `GIT_DIR` set, which git does for hooks in linked
  worktrees, the test's commit landed in another repository and the test still passed.
- #2368 finding 1: `connect` read `WSP_HOST` and a turn's launch pair before the state file. From a shell inside a
  wsp thread that is a thread-scoped token, which listed 1 of 2 threads with no error; the live proof would have
  looked right.

Before hand-in:

- Every `Command` starts with `env_clear()` (`env -i` in shell) and adds back only the names it needs
  (wsp-review, Threat model).
- Run the test once with `GIT_DIR`, `WSP_HOST`, `WSP_HOST_URL` and `WSP_HOST_TOKEN` aimed somewhere wrong: another
  repository, a closed port. It still passes and leaves that place untouched.
- Code that finds the host reads what the desktop app reads: the state file's host, then the account's one host.

## 6. Stopping processes by pattern, or by a pid that may be reused (2)

- #2366 report: the builder cleaned up with `pkill -f`, which killed its own shell twice. A pattern kill once
  killed the owner's live host (wsp-review, Process laws).
- #2367 finding 2: `dev.sh` killed the app's old pid after the app had quit on its own. macOS pids wrap at 99999
  and the poll loop spawned about 13 processes a second, so within about two hours that pid is someone else's.

Before hand-in:

- Record the pid of what you start (`$!`, `Child::id()`) and stop only that pid. Never find one by name, pattern or
  port. The script fails an added `pkill`, `killall` or `pgrep`.
- Before signalling a pid that may have exited, check it is still your child, as `dev.sh` does:
  `[ "$(ps -o ppid= -p "$pid" | tr -d ' ')" = "$$" ]`.
- Stop with SIGTERM so the process stops its own children, then check `ps` and say nothing is left.

## 7. A script that assumes the default setup (2)

- #2367 finding 7: with `CARGO_TARGET_DIR` set, `dev.sh` built and then ran `target/debug/desktop-gpui`, which was
  not there.
- #2367 finding 8: closing the terminal left the stamp file behind, because the trap caught INT and TERM, not HUP.

Before hand-in: take paths from the variables that move them (`${CARGO_TARGET_DIR:-target}`), trap HUP with INT
and TERM, run shellcheck, and run the script once from another folder.

## 8. Amending on a shallow clone (1)

- #2368 fix round: a fresh machine fetched the branch shallow, and `git commit --amend` made e1d1fc5 a root commit.
  GitHub closed PR #1592 as unrelated history and would not reopen it; the same tree landed again through #1594.

Before any amend, `git rev-parse --is-shallow-repository`; on `true`, `git fetch --unshallow origin` first. After
the push, `git cat-file -p HEAD | head -3` shows a `parent` line. The script checks both.

## Related

- [wsp-gpui](../wsp-gpui/SKILL.md): the lane's guide, pins and Boat notes.
- [wsp-review](../wsp-review/SKILL.md): the repo's laws, which every section above leans on.
