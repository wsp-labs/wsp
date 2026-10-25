# local_exec_stream
Source: packages/runtime/src/local-exec.ts:437-794 (358 lines) at 65f6b89c7. Rust: `src/local_exec/stream.rs`.
Tests: the stream, launch, attach, sweep, input and group cases of packages/runtime/test/local-exec.test.ts (40
cases) and 3 pinned beyond them. Rust: `tests/local_exec_stream.rs`, every case in a process group of its own.

## What it does
`LocalExecFactory::new(opts, waiting)` is the `ExecStreamFactory` of a turn on this computer: a detached `bash`
running a script that writes the turn's output to a log file and its code to an exit file, all under the state
folder's `runs/`. A reader polls the log from its first poll, whether or not anyone iterates; the run outlives the
host, `attach` re-opens it by handle and `sweep` ends what no running row holds. Rule 6.17 to 6.21 and 6.33 to 6.37 of
the host spec are this unit.

## Behaviour
1. Options (local-exec.ts:438-441): `idle_ms` `TURN_IDLE_MS`, `deadline_ms` `TURN_WALL_MS` (6 h), `now` system,
   `poll_ms` 100, `root` the child's cwd, `run_dir`, `read_work` (`tree_work()` per open when absent), `reading`.
2. `launch` (local-exec.ts:702-755): a `secret` refuses with `a launch on this computer takes no text holding a value;
   nothing was started` (decision 1). `base = run_dir/<12 hex>`; `mkdir -p run_dir` at 0700 (an existing folder keeps
   its mode); `mkdir base.d` 0700. `base.sh`, created exclusive at 0600: with no input `( <command>\n)\necho $? >
   'base.exit'\n`; with one the pump form of rule 6.17 step 2 exactly, end marker `__WSP_EOF_<last 12 of base>__`.
   `base.in` 0600 exclusive holds the seed (each line plus `\n`), or `""` when `input_after` holds it back, and then
   `base.held` 0600 holds the seed; `base.log` 0600 exclusive and empty. Spawn `bash base.sh`, cwd `root`, env
   exactly the turn's plus `WSP_AGENT_PID_FILE=base.agent` (nothing of the host's), stdin null, stdout and stderr the
   log opened for append, a new process group, not waited on; write `base.pid` as `<pid>\n` (default mode). A spawn
   that fails marks the launch failed. With a held seed: once `input_after` resolves `release_held(base)`; a refusal
   kills the stream.
3. The reader (`open`, local-exec.ts:451-700) starts polling at open as a task of its own and registers a drop in
   `reading`. `started_at` is the attach's or `now()`; the idle clock starts at `now()`; `offset` is `from` or 0.
   `pid` (the ExecStream field) is the leader read once at open.
4. Each poll round, in rule 6.33's order: killed settles `None`; a failed launch settles `None`; a waiting question
   touches the clock; with a leader, no reading in flight, not resting and `reads_work`, start one `read_work(leader)`
   not awaited (a `Some` reading is read into the clock at the round's `at`); `cut` unless resting, wall before idle;
   cut and caught up and no exit seen settles `None` with the cut; read `base.exit` trimmed (missing reads none) BEFORE
   one chunk of the log (at most 262,144 bytes from `offset`, stat then read; an error reads empty). A non-empty chunk:
   touch, feed, caught up when short, `exit_seen` noted, yield the task and loop with no sleep. Empty with a non-empty
   exit: settle `parseInt(exit)`. Else caught up; a cut settles `None` with it; a leader missing or dead on a second
   round over the stream's life settles `None`; else sleep `poll_ms`, 1 s when resting, cut short by a wake.
5. Settling (local-exec.ts:549-561): once; remove the drop; `reap_run(base, last_read)` where `last_read(end)` feeds
   every remaining chunk up to `end`, yielding the task between chunks; then end the lines (or fail them with the cut,
   after the last line); then resolve `exited`. A throw anywhere in the poll fails the lines with it and settles
   `None`, without a reap.
6. `teardown` and `kill` signal the leader's group TERM and KILL, nothing when no leader or settled; `kill` also marks
   killed. A local stream never reports a signal (`signalled()` is `None`).
7. `write(line)` (local-exec.ts:626-643): no input fails `this stream has no input channel`; waits while ANY line is
   parked (a loop, unlike the machine road); settled or input closed answers `Gone`; `base.exit` present or `base.d`
   missing answers `Gone`; `release_held`, append `line\n` to `base.in` (any error answers `Gone`); touch; `Written`.
8. `close_input()`: once, with input, unsettled; `release_held` then append the end marker line, at once when nothing
   is parked, else after every parked line; errors swallowed.
9. `write_after(line, after)`: queues behind the line parked when it is called; parks by creating `base.late.key` (12 hex) and
   `base.late` (`line\n`) exclusive at 0600; `after` resolving runs `release_late(base)`, a refusal deletes both;
   errors swallowed; the queue moves on.
10. `rest()`: resting, then end the leftovers: `leftovers(group_rows(leader), leader, pump_of(base))` each TERM by
    pid, and after `RUN_STOP_MS` KILL those still alive. `next_turn()`: not resting, `started_at = now()`, a fresh
    clock, wake the poll, answer the log's size (the offset when the log cannot be read).
11. `attach(run, {input, started_at, from})` (local-exec.ts:757-773): refuses a handle not `run_dir/<12 hex>` with
    `<run> is not a run this host could have launched`; `run.d` missing answers `Gone`; with input `release_held`,
    then an empty `run.in` (present) reaps the run and answers `Gone`; else opens: with input after
    `release_late(run)`, `taken` is `run.in`'s lines less empty ones and the end marker.
12. `sweep(keep)`: `run_dir` unreadable answers `[]`; the stale runs are names ending `.d`, less the suffix, joined to
    `run_dir`, minted, not in `keep`, in directory order; all reaped at once; answers them.

## Edge cases
- Rule 6.34's table, every row (empty exit, `14abc`, no pid, last line with no newline, a line of exactly 16 MiB,
  `.in` holding only the marker, empty `.in` with and without `.held`, `from` past the log's size).
- The environment is the turn's alone: a host variable never reaches the turn.

## Behaviour nobody wrote down
- The spawned `bash` is reaped when it exits, as node's child handling reaps every child it spawned (a dropped
  `tokio::process::Child` is reaped by tokio's orphan queue; a dropped `std::process::Child` is not). A leader nobody
  reaped is a zombie that `kill(pid, 0)` reads as alive, so the dead-leader rule would never fire.
- Invariant 1: a restarted host re-opens a run by handle and reads its log from byte 0
  (`a_factory_that_never_launched_a_run_attaches_to_it_by_handle_and_reads_its_whol`); the wall counts from the
  row's `started_at` (`a_turn_re_opened_after_a_restart_is_cut_at_its_first_start_plus_the_wall_not`).
- Invariant 2: `next_turn` answers the size a re-open reads from.
- No file under `runs/` holds the turn's environment (`a_turn_and_an_exec_launched_with_a_key...`).

## Decisions
0. Names and keys are drawn through the `Ids` seam (`ids`, `OsIds` by default): `hex(ids.bytes(6))`.
1. As machine_exec_factory decision 1: `try_launch` answers the refusal (and a failed mkdir or exclusive create) as
   `Err`; `launch` turns it into a failed stream.
2. Decision 2 of machine_exec_stream (NaN exit settles `None`).

## After the port
- `write` waits for lines parked after it started; the machine road does not.
- A non-numeric `.exit` settles with NaN in the TS (rule 6.34).

## Rules against tests
2 the launch, mode and key cases; 3-5 the stream, limit, re-open and long-log cases; 6 `kill_ends_the_child`,
`teardown_reaches_what_the_turn_started...`; 7-9 the input, held seed and late line cases; 10 `re_opened_mid_turn_spares_only_its_input_pump...`;
11 the attach cases; 12 the sweep cases; edge rows `broken_run_files_read_as_the_ts_reader_reads_them` (pinned),
`the_environment_is_the_turn_s_alone` (pinned), `a_stored_from_past_the_log_reads_nothing_until_the_log_grows` (pinned).
