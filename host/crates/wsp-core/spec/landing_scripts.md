# landing_scripts
Source at 65f6b89c7: packages/runtime/src/project-landing.ts:106-352 and :483-504 (the scripts, about 260 lines).
Tests: project-seed-script.test.ts (25, real git), branch-of-record.test.ts:158-363 (6), box-folder.test.ts:603 (1).
Rust: `src/project_landing.rs` (the script functions); tests `tests/landing_scripts.rs` (50). Fixture:
`landing_scripts.json`. Every script runs on a computer with no git identity and no config of the person's.

## Rules
- **L1** `clone_lines` (:125-145). Brokered: `set -e`, `mkdir -p '<checkout's folder>'`, `git clone --depth 1
  [--branch <b>] -- <remote> <checkout>` (shell_line), then `on_record_lines`. Otherwise: `set -e`; where the source
  has a command line, `command -v <bin> >/dev/null 2>&1 || { echo '<missing(computer)>' >&2; exit 1; }` and, unless
  `setup_git` is `Some(false)`, its setup-git line; `mkdir -p`; the source's own clone command. The checkout's folder is
  the checkout with its last `/part` cut (`/x` gives the empty string, `x` stays `x`).
- **L2** `on_record_lines` (:155-167): a fork is owner/name differing from `origin`, case-insensitively, and its remote
  is named for the owner (`remote add -t '<branch>*'` its github url), else origin gets one fetch refspec for
  `<branch>*`. With a start commit: fetch it at depth 1 by id unless `cat-file -e` finds it, then `checkout -q -B
  <branch> <sha>`; without, `checkout -q -B <branch>`. A held branch with a start commit writes its remote-tracking
  ref; a fork or a held branch sets `branch.<b>.remote` and `.merge`. Last, `git rev-parse HEAD`. Every line is
  `git -C <checkout> ...` through shell_line.
- **L3** `brokered_git_env` (:181-195): GIT_CONFIG_COUNT first, the numbered KEY/VALUE pairs, GIT_TERMINAL_PROMPT=0
  last. The pairs: each of the four GitHub remote forms rewritten to `http://github.com/`; for each public repo of
  record and each `direct` repo (`.git` form only, deduplicated, record first) and each form, an https insteadOf and an
  http pushInsteadOf; the proxy `http://wsp:<token>@127.0.0.1:7071`; `http.proxyAuthMethod basic`; an empty
  credential helper. A private record repo adds nothing.
- **L4** `clone_script` (:200-204): the clone lines, then `tar -xzf '<seed>' -C '<checkout>'` where a seed is given,
  joined by `\n`.
- **L5** `patch_script` (:221-231): `set -e` and five git lines as wsp (`-c user.name=wsp -c user.email=wsp@localhost`):
  `checkout -B <branch> <base>`, `am --3way <checkout>/.wsp-seed/commits.patch`, `update-ref refs/wsp/seeded/<branch>
  HEAD`, `checkout <back>`, `rev-list --count <base>..<branch>`. One command a line, never an `&&` chain.
- **L6** `patch_cleanup_script` (:239-247): no `set -e`: `am --abort`, `checkout -f <back>`, `reset --hard
  origin/<back>`, and `branch -D <branch>` unless it is `back`.
- **L7** `unsaved_script` (:262-297) prints `<ahead> <changed> <stashes>`: commits on any branch or worktree HEAD no
  remote and no seeded ref holds, every worktree's uncommitted files less the install stores at its root, the stashes;
  `wsp-no-checkout` for a missing folder or one git does not track; exit 1 with nothing for no git, a `.git` git cannot
  open, a dangling `.git` link (checked before git is asked) or any failing step. Tags are not counted.
- **L8** `unsaved_of` (:301-310): only stdout's last line is read, and only on exit 0. `wsp-no-checkout` is `None`;
  exactly three words (split on whitespace) each a non-negative integer as JS `Number` reads it give `unpushed_line`
  (`None` when all are 0). JS `Number` takes `0x1` as 1 and `1e1` as 10 (fixture rows), and refuses `1.5` and `-2`.
  Anything else is `<name>: could not read what is not pushed: <why>`, why being stderr's last line or `the read
  exited <n>` for a failed read, or `the read printed "<said>"` (JSON-quoted, `""` for nothing) for one that exited 0.
- **L9** `read_unsaved` (:314-322): runs `unsaved_script` with STEP_MS (120,000); a run that fails answers the
  could-not-read line with the error's message.
- **L10** `seed_rest_script` (:336-352): with memory, `rmdir` the memory folder quietly, then keep what stands (print
  `wsp-memory-kept`) or make its parent and move `.wsp-seed/memory` there; always remove `.wsp-seed` and the archive.
  `set -e`, one command a line, so a failed mkdir stops before the sweep.
- **L11** `seed_into_script` (:485-488): unpack with `--skip-old-files --exclude='.wsp-seed'`, removing the archive on
  failure with the tar's exit code, and after success.
- **L12** `under_login_home` (:492): a path that is `/root` or under it moves under the home with trailing slashes cut.
- **L13** `free_folder_script` (:500-504): tries the name, then `-2` to `-99`, `mkdir` as the claim, prints the folder
  taken; else `every name from <name> to <name>-99 is taken in <home>` on stderr, exit 1. The home's trailing slashes
  are cut.
- **L14** `last_line` (:636-639): the last line of the trimmed text, trimmed; `None` where empty.

## Tests (`tests/landing_scripts.rs`)
Each fixture test names its key: `cloneLines` (L1), `onRecord` (L2), `gitEnv` (L3, key order), `cloneScript` (L4),
`patchScript` (L5), `patchCleanup` (L6), `unsavedScript` (L7), `unsavedOf` (L8, 18 rows), `readUnsaved` (L9),
`seedRest` (L10), `seedInto` (L11), `underLoginHome` (L12), `freeFolder` (L13). The 25 project-seed-script cases
pin L4 to L10 against real git (identity, an unappliable patch, a base the remote lost, memory kept or landed, worktrees,
stashes, dangling links). The six branch-of-record cases pin L1 to L3 against real git and a local proxy door. Three
box-folder tests pin L13 by running it.
