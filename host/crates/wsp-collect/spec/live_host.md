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
8. `spawn_run` (:177-252): a signal already cancelled answers `None` and spawns nothing. Spawn with exactly `env` (cleared first), stdin null, stdout piped, stderr piped and drained unread, in a new process group (`process_group(0)`). A spawn error answers `None`.
9. Budget `timeout_ms` or 120,000 ms: on expiry mark failed, SIGTERM the group, and settle `None` after `CLOSE_GRACE_MS` (1 s) if nothing settled first (an interactive shell ignores SIGTERM).
10. Exit: when the child exits, wait up to 1 s for stdout to close; settle on close or at the grace, whichever first. A background job that kept stdout does not hold the run past the grace.
11. Settle (once): stop timers, SIGKILL the group (whatever is left of it dies), drop the pipes, answer stdout as UTF-8 only when not failed, exit code 0 and no signal; else `None`.
12. stdout over `MAX_OUTPUT` (64 MiB) in total marks the run failed and SIGKILLs the group; the bytes past the cap are not kept.
13. Cancel (:204-221): mark failed; if the child already exited, settle at once and signal only its group (never the exited pid, which the system may reuse); else read `tree_of(pid)`, SIGKILL the group, then SIGKILL every pid of the tree (a child that leads a group of its own is reached this way).
14. `tree_of` (:157-173): `ps -A -o pid=,ppid=`; failing, `[pid]`. `children_of` maps each parent to its children in listing order, skipping lines that are not two numbers; the tree is breadth first from `pid`.
15. `platform_of`: `macos` is darwin, `linux` linux, else `None`. `node_host` (:258-267): platform from `std::env::consts::OS`; an unknown one is `HostError::new("wsp reads a computer's setup on macOS or Linux only, not <os>")`; `home` from `HOME` (node's `homedir()`); `shell`, `terminal`, `xdg_config_home`, `claude_config_dir`, `codex_home` from `SHELL`, `TERM_PROGRAM`, `XDG_CONFIG_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME` when set and not empty; `stores`, `seeds` `None`; `fs` NodeFs, `exec` NodeExec.

## Kill and stop paths

Every exit from `spawn_run` SIGKILLs the child's group (rule 11), so jobs an rc file started never outlive the run. A group signal that fails (the group is gone) is ignored. Nothing is borrowed across an await (rule 3.3): the run's state is local to the future.

## Behaviour nobody wrote down

- Chapter 06, rule 6.23: `spawn_run` reports no exit code at all; a timeout, a signal, a non-zero exit, too much output and a spawn error all read `None`.

## Decisions

1. `node_host` takes `HOME` as node does; node falls back to the password entry when `HOME` is unset, Rust may do the same.
2. The TS spies on `process.kill` to prove the exited pid is never signalled; Rust cannot, so the test pins only that the run answers `None` and the job dies.

## After the port

- A budget past which the child already exited still runs the 1 s grace. Kept.

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
