# live_host

Source: `packages/collect/src/live-host.ts` (267 lines) at `a3b870a6e`. Tests: `packages/collect/test/node-exec.test.ts` (12 cases). Rust: `src/live_host.rs`; tests `tests/live_host.rs` (12 ported, 13 added). Chapter 06's `spawnRun` row is this unit.

## What it does

The real Host: the disk behind `HostFs` and processes behind `HostExec`. `spawn_run` is the collector's one spawn helper: each child leads its own process group, reads end of file on stdin, and dies with the run.

## Behaviour

1. `stat` (live-host.ts:54-64): follow links (`std::fs::metadata`); a dir is `{Dir, tree_bytes, mtime_ms}`, a regular file `{File, len, mtime_ms}`, anything else (fifo, socket, missing) `None`. `mtime_ms` is node's `mtimeMs`: seconds × 1000 + nanoseconds / 1e6, as `f64`.
2. `tree_bytes` (:13-35): every regular file's size under dir; a symlink entry is skipped (not followed); `.git` and `node_modules` folders are not entered; an unreadable folder or file counts 0.
3. `list` (:65-71): the names directly under dir (dot entries and links included), sorted by UTF-16 units; `[]` when unreadable. `read_text` (:72-78): the file as UTF-8 (invalid bytes replaced, as node's `"utf8"`), `None` on any error.
4. `walk` (:37-51, 79-83): every regular file under dir, absolute, links skipped, `SKIP_DIRS` not entered, sorted by UTF-16 units; `[]` for a missing dir.
5. `lines` (:84-125): open fails, nothing. Read `LINE_CHUNK` (1 MiB) at a time; a line is the bytes up to LF, joined across reads; with `holding`, a line whose bytes do not contain the needle's bytes is skipped before any decoding; one trailing CR is dropped; decoded as UTF-8 with U+FFFD. After end of file, leftover bytes are one last line (no LF needed); an empty remainder gives nothing. A read error ends the stream quietly. The file is closed when the stream ends or is dropped.
6. `on_path` (:133-144): each non-empty entry of the PATH text in order, true when `<dir>/<bin>` passes `access(X_OK)`; `None` or empty PATH is false. `NodeExec::which` reads this process's PATH.
7. `NodeExec::run` (:150-153): `spawn_run(cmd, args, this process's environment with opts.env laid over it, opts)`.
8. `spawn_run` (:177-252): a signal already cancelled answers `None` and spawns nothing. Spawn with exactly `env` (cleared first), stdin null, stdout piped, stderr piped and drained unread, in a new session (node's `detached: true` is `setsid()`: the child leads a new session and a new group, pgid and sid equal to its pid, with no controlling terminal; `process_group(0)` alone is not enough, since `login-path.ts` runs an interactive login shell this way). A spawn error (ENOENT, EACCES, ENOTDIR, a folder) answers `None`. A NUL byte in the command, an argument or an env value, or an empty command, makes node throw synchronously, so the TS promise rejects with `TypeError [ERR_INVALID_ARG_VALUE]`; `spawn_run` answers `Err(SpawnRefusal)` with node's message, in node's checking order (command empty, command NUL, each argument, then each env key and value), after the already-cancelled check. `NodeExec::run`, whose seam has no error, reads a refusal as `None`; no caller passes such a value.
9. Budget `timeout_ms` or 120,000 ms: on expiry mark failed, SIGTERM the group, and settle `None` after `CLOSE_GRACE_MS` (1 s) if nothing settled first (an interactive shell ignores SIGTERM).
10. Exit: when the child exits, wait up to 1 s for both stdout and stderr to close (node's `close` event); settle on close or at the grace, whichever first, the grace timer started at the exit and replacing a budget grace still pending. A grandchild that holds only stderr costs the full grace too; a child that closes stdout early but keeps running is not settled by that. A background job that kept stdout does not hold the run past the grace.
11. Settle (once): stop timers, SIGKILL the group (whatever is left of it dies), drop the pipes, answer stdout as UTF-8 only when not failed, exit code 0 and no signal; else `None`.
12. stdout over `MAX_OUTPUT` (64 MiB) in total marks the run failed and SIGKILLs the group; the bytes past the cap are not kept.
13. Cancel (:204-221): mark failed; if the child already exited, settle at once and signal only its group (never the exited pid, which the system may reuse); else read `tree_of(pid)`, SIGKILL the group, then SIGKILL every pid of the tree (a child that leads a group of its own is reached this way).
14. `tree_of` (:157-173): `ps -A -o pid=,ppid=`; failing, `[pid]`. `children_of` maps each parent to its children in listing order: each line `js_trim`med, split on JS `\s+`, read with JavaScript's `Number()`, the first token the child and the second the parent, any later column ignored; a line whose first two tokens are not both numbers is skipped; a repeated line gives a repeated child; the tree is breadth first from `pid`.
15. `platform_of`: `macos` is darwin, `linux` linux, else `None`. `node_host` (:258-267): platform from `std::env::consts::OS`; an unknown one is `HostError::new("wsp reads a computer's setup on macOS or Linux only, not <os>")` with `<os>` node's `process.platform` word (`windows` is `win32`, `solaris` is `sunos`; `freebsd`, `openbsd`, `netbsd`, `aix`, `android` as Rust names them); `platform_of` takes Rust's OS word, so `platform_of("darwin")` is `None`; `home` from `HOME` (node's `homedir()`); `shell`, `terminal`, `xdg_config_home`, `claude_config_dir`, `codex_home` from `SHELL`, `TERM_PROGRAM`, `XDG_CONFIG_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` when set and not empty; `stores`, `seeds` `None`; `fs` NodeFs, `exec` NodeExec.

## Kill and stop paths

Every exit from `spawn_run` SIGKILLs the child's group (rule 11), so jobs an rc file started never outlive the run. A group signal that fails (the group is gone) is ignored. Nothing is borrowed across an await (rule 3.3): the run's state is local to the future.

## Behaviour nobody wrote down

- Chapter 06, rule 6.23: `spawn_run` reports no exit code at all; a timeout, a signal, a non-zero exit, too much output and a spawn error all read `None`.

## Decisions

1. `node_host` takes `HOME` as node does; node falls back to the password entry when `HOME` is unset, Rust may do the same.
2. The TS spies on `process.kill` to prove the exited pid is never signalled; Rust cannot, so the test pins only that the run answers `None` and the job dies.

## After the port

- A budget past which the child already exited still runs the 1 s grace. Kept.

## Wave 2 rules

Disk cases: `tests/fixtures/oracle/w2/live_host.json` (`oracle/w2-live_host.oracle.ts`, real trees under a temp
folder), replayed by `tests/live_host.rs` `w2_oracle::*`. Process cases: `tests/live_host.rs` `w2_processes::*`, each
outcome measured on the TS `spawnRun`.

- W1. `stat`: `.git` and `node_modules` folders anywhere under the root are not entered (a file of that name is
  sized; a root named `node_modules` is sized); a hard link is counted twice; a fifo has no stat and is not counted
  inside a folder; links are followed at the top and skipped below; a path under a file has no stat (`node_fs_stat`).
- W2. `list` and `walk` sort by UTF-16 units over the whole path (`a-b/x`, `a.b/x`, `a/x`, `a0`); `walk` joins as
  `path.join` does (a double slash and a trailing slash normalised); links skipped; a file or a missing folder walks to
  `[]` (`node_fs_list`, `node_fs_walk`). `read_text` of a folder or a fifo-free missing path is `None`
  (`node_fs_read_text`).
- W3. `lines`: a CR at the last byte of one 1 MiB read with its LF first in the next is still one line ending; a
  multi-byte character cut by a read stays whole; a BOM is kept; a lone CR is one empty line; `holding` tested on raw
  bytes (`node_fs_lines`, line lengths for the chunk cases).
- W4. `on_path`: empty entries skipped, a folder named like the bin counts (X_OK), a link to an executable counts, a
  dead link does not, mode 0644 does not (`on_path`).
- W5. `children_of`: extra columns ignored, `Number()` tokens, repeats kept (`children_of`); `platform_of` takes Rust
  OS words (`platform_of_takes_rust_s_os_words_only`).
- W6. The child leads its own session (`the_child_leads_its_own_session_*`).
- W7. A grandchild holding stdout or stderr costs the 1 s grace; one holding neither costs nothing; a child that closes
  stdout early is settled by its exit (`a_grandchild_holding_*`, `a_child_that_closes_stdout_early_*`).
- W8. Exactly 64 MiB is kept; one byte more is `None` and ends the run at once (`exactly_64_mib_*`).
- W9. A cancel while running kills the process tree, a setsid job included; the budget kills only the group, so that
  job lives on (`a_cancel_while_running_*`).
- W10. A zero budget answers `None` at once (`a_zero_budget_*`).
- W11. stdout is decoded as node decodes UTF-8 (each bad byte one U+FFFD, a BOM kept); a stderr flood never blocks;
  exit 143 is `None`; `true` is `""` (`stdout_is_decoded_*`).
- W12. An empty command, or a NUL in the command, an argument or an env value, is a `SpawnRefusal` with the message node
  threw on node 22.23; a missing binary and a folder answer `None` (`an_empty_command_or_a_nul_*`).

## Rules pinned

| Rule | Test |
|---|---|
| 1, 2 | `reads_the_modification_time_off_the_disk_*`, `stat_sizes_a_folder_by_every_file_under_it_*` |
| 3, 4 | `walk_lists_every_regular_file_sorted_absolute_*` |
| 5 | `gives_each_line_without_its_ending_*`, `keeps_a_line_longer_than_one_read_whole_*`, `gives_only_the_lines_holding_the_text_asked_for_*`, `lines_drop_one_carriage_return_*`, `lines_holding_text_skip_lines_*`, `lines_read_bad_utf8_*` |
| 6 | `on_path_finds_an_executable_file_and_skips_empty_entries` |
| 7, 8 | `returns_stdout_on_exit_0_*`, `gives_every_child_end_of_file_on_stdin_*`, `adds_the_given_variables_*`, `the_child_runs_under_exactly_the_environment_it_is_given`, `stderr_is_dropped_*`, `a_run_asked_after_its_signal_was_cancelled_starts_nothing` |
| 9 | `ends_a_child_at_the_budget`, `ends_a_child_that_ignores_the_budget_s_sigterm_*`, `the_budget_ends_the_whole_process_group_*` |
| 10, 11 | `does_not_wait_on_a_grandchild_that_kept_stdout_*`, `a_child_killed_by_a_signal_answers_nothing_even_with_output` |
| 12 | `a_run_whose_stdout_passes_64_mib_answers_nothing` |
| 13 | `an_abort_after_the_child_exited_signals_its_group_alone_*` |
| 14, 15 | `children_of_reads_a_ps_listing_*`, `platform_of_knows_darwin_and_linux_only` |
