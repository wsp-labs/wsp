# local_exec_reap
Source: packages/runtime/src/local-exec.ts:38-435 (398 lines) at 65f6b89c7. Rust: `src/local_exec/reap.rs`.
Tests: the tree, agent-pid, stranger and reap cases of packages/runtime/test/local-exec.test.ts (9 cases), the
whole of local-exec-no-ps.test.ts (3), and 6 pinned beyond them. Rust: `tests/local_exec_reap.rs`,
`tests/local_exec_no_ps.rs`. Every test that starts a process runs in a process group of its own (support
`own_group`), and the strays it records are killed after it.

## What it does
The process side of a turn on this computer: reading a turn's tree's CPU through `ps` (`tree_work`), what a kept turn
left in its group (`leftovers`), whether a recorded pid still leads its run's group (`leads_this_run`), and `reap_run`,
which ends a run's whole group and deletes its files. Also the held seed and late line hand-overs into a run's input
file, and `Lines`, the queue a local stream's lines wait in.

## Behaviour
1. `signal_group(pid, sig)` sends to `-pid` and swallows every error; `alive(pid)` is `kill(pid, 0)` succeeding or
   failing EPERM; `group_exists(pid)` is `kill(-pid, 0)` succeeding or failing EPERM (local-exec.ts:88-103, 258-265).
   `signal_pid` sends to `pid` and swallows every error. None of them is called with a pid below 1.
2. `tree_work()` (local-exec.ts:131-176) makes one reader with its own memory. Each call runs `ps -eo
   pid=,ppid=,pgid=,time=`; a spawn failure or a failed run answers `None`. Rows: each line trimmed and split on
   whitespace, kept when the pid word is non-empty, keyed by `Number(pid)`; ticks are `round(ps_cpu_seconds(time) *
   100)` (`js_round`), 0 when not a safe integer. A row is in the tree when its walk up `ppid` reaches the leader, or a
   row whose group is the leader, before a missing row, a pid at or below 1, or a loop; each walked pid remembers the
   answer for this reading. For each row in the tree, `total +=` its ticks when unseen or lower than last time (a new
   process on that number), else its ticks less last time's; the memory becomes this reading's rows. Answers `total`,
   which never falls.
3. `group_rows(pgid)` runs `ps -axo pid=,ppid=,pgid=` and keeps rows of that group whose pid and ppid are safe
   integers; any failure is `[]`. `orphans_of(rows, leader)`: members other than the leader whose parent is not a
   member. `pump_of(base)`: the positive safe integers, one per line, of `base.pump`. `leftovers(rows, leader,
   pump)`: the orphans not in the pump, closed under "child of a leftover" (local-exec.ts:187-226).
4. `agent_pid_of(run)`: the number in `run.agent`, trimmed, when a positive safe integer that is `alive`; else `None`.
5. `started_at(pid)` runs `ps -o etime= -p <pid>`: `[[dd-]hh:]mm:ss` gives now less that span in ms; fewer than two
   colon parts or any non-finite part, or a failed run, is `None` (local-exec.ts:270-286).
6. `leads_this_run(base, pid)` (local-exec.ts:299-313): false when the group is gone; true when the leader is dead
   (its children hold the number); true when `started_at` is `None`; true when `base.d` has no mtime; else
   `|started - mtime| <= 5000`.
7. `reap_run(base, last_read, grace = RUN_STOP_MS)` (local-exec.ts:321-333): read `base.pid` (trimmed number); when a
   positive safe integer that `leads_this_run`: TERM the group, then wait in 200 ms steps (the last one shortened to
   the grace) while the group exists, then KILL the group whatever the wait saw. Then `last_read(log size, 0 when no
   log)`. Then delete `sh log pid agent exit in held late fifo tail pump` in that order, each ignoring a missing file,
   then `base.d` recursively. `.late.key` and `.late.went` are never deleted here.
8. `release_held(base)`: with `base.held` present, append it to `base.in` only when `base.d` exists, `base.in` exists
   and is empty; then delete `base.held` either way (local-exec.ts:345-350).
9. `release_late(base)`: `release_held`, then with `base.late` present append it to `base.in` when `base.d` exists,
   `base.exit` does not, and `base.late.key`'s text (or none) differs from `base.late.went`'s (or none); then write
   `base.late.went` (0600) with the key text or `""`; delete `base.late` and `base.late.key` either way.
10. `Lines` (local-exec.ts:372-435): `feed(bytes)` pushes `LineCut` lines unless let go, and wakes the reader; `end()`
    pushes the tail no newline ended (unless let go) and marks done; `fail(e)` ends with `e` after every queued line.
    `iterate()` is the one reader: yields the queue, then the end or the error. Dropping the reader (before or after
    its first poll) lets go: the queue empties and later feeds are dropped.

## Edge cases
- A `.pid` that is missing, empty, `0`, negative, `12x` or `1.5`: nothing is signalled, the files still go.
- A recorded pid whose group belongs to a process started long after the claim (claim mtime 10 min ago): files go,
  the process lives.
- `ps` printing `999 1 999 0:00.50`, `1000 999 999 0:01.00`, `500 600 500 0:01.00`, `600 500 600 0:01.00` for leader
  999 reads 150: the loop 500/600 ends its walk.
- `started_at` of `01:02` is 62 s ago, `1-00:00:01` is 86,401 s ago, `5` is `None`.

## Behaviour nobody wrote down
- Rule 6.36: signal errors are swallowed, EPERM counts as existing, so a group under another login waits out the grace.
- Rule 6.24: a run with no readable pid is never signalled; its group outlives the reap.
- `last_read` runs after the KILL and before the log is deleted, bounded by the size at that moment.

## Decisions
1. `ps` is found on the host process's PATH, as `execFile("ps")` finds it; the no-ps tests run in their own process
   with a PATH whose `ps` fails, which stands for the TS mock of a throwing `execFile`.
2. `TreeWorkReader` is `Arc<dyn Fn(i32) -> BoxFuture<'static, Option<i64>> + Send + Sync>`; the TS callback form
   becomes one future per reading.

## After the port
- `.late.went` stays in `runs/` after every turn that released a late line (rule 6.35).
- A pid recorded as 0 or negative is read as no leader, so nothing ends that run's group.

## Rules against tests
| Rule | Test |
|---|---|
| 2 | `the_tree_s_reading_keeps_the_work_of_a_child_its_parent_has_reaped`, `the_tree_s_reading_counts_a_child_in_a_session_of_its_own_as_an_agent_s_tool_ca`, `the_tree_s_reading_counts_an_orphan_the_leader_left_in_its_group`, `the_tree_s_reading_answers_when_ps_names_two_processes_as_each_other_s_parent`, `the_tree_s_reading_never_falls_when_a_process_is_read_lower` |
| 2, D1 | local_exec_no_ps (3) |
| 3 | `leftovers_are_orphans_less_the_pump_and_everything_under_them` |
| 4 | `the_agent_a_launch_starts_through_agent_word_writes_its_own_pid...`, `the_pid_agent_word_writes_is_the_agent_s_own...` |
| 5 | `started_at_reads_the_elapsed_forms_ps_prints` |
| 6, 7 | `a_run_whose_recorded_pid_leads_somebody_else_s_group_is_cleaned_up_and_never_s`, `a_reap_with_no_readable_pid_signals_nothing_and_deletes_the_files` |
| 8, 9 | `release_held_appends_only_into_an_empty_channel_and_drops_the_seed`, `release_late_goes_in_once_by_its_key` |
| 10 | `lines_hands_every_queued_line_before_the_error_and_drops_feeds_after_let_go` |
