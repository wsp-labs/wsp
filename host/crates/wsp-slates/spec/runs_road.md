# runs_road
Source: slate-runs.ts:165-195 (`RoadStart`, `RoadEnd`, `RunRoad`), 244-294 (`reshapeResult`, `reshapeAnswer`), 296-361
(`localRoad`), 491-497 (`killGroup`) at 65f6b89c7. Tests: slate-runs.test.ts `process lifetime` (cancel, restart,
timeout, exit code, output caps), `then` (3 cases), `a command's process`.

## What it does
Runs one command on this computer as the host's child: `bash -c <cmd> bash <args...>` in its own process group, the
values as environment, positional parameters or stdin, never in the text. It streams lines when asked, caps output
as the daemon caps an exec's, kills the whole group at the deadline or on a kill, and reports the end once. A
`then` reshapes a result the same way with the raw output on stdin and must print JSON.

## Behaviour
1. `local_road().start(o, on)` (:300-359) spawns `bash` with argv `["-c", o.cmd, "bash", ...o.args]`, cwd `o.cwd`,
   environment exactly `o.env` (nothing inherited), in a new session and process group (node `detached`, rule
   6.24), stdin a pipe holding `o.stdin` then closed when given, else `/dev/null`; errors writing stdin ignored.
2. Output (:302-321): stdout and stderr share one cap of `EXEC_OUTPUT_MAX` = 2,097,152 bytes; a chunk that would pass
   it is cut to what is left and `cut` set; empty chunks are dropped. With `o.stream`, each stream is decoded as
   UTF-8 across chunk edges and every complete line (split on `\n`, `\r` kept) goes to `on.line(stream, line)` as it
   arrives; the rest of each stream goes out at the end when not empty (:339-344).
3. The deadline (:329-332): after `o.timeout_s` seconds, `timed_out` and SIGKILL to the group.
4. The end is reported once (:333-346), from whichever comes first: the spawn error (`code` none), `close`, or 1,000
   ms after `exit` (pipes a setsid grandchild holds are dropped then, :348-356). On `exit` the group is killed at
   once. At the end: deadline and grace cleared, SIGKILL to the group again, partial lines flushed, then
   `on.end({code, signal, error?, out, err, cut, timed_out})` with each stream's bytes decoded as UTF-8 (invalid
   bytes as U+FFFD).
5. `start` answers `kill` (SIGKILL to the group) and the child's `pid` (:358). A kill does not end the run itself;
   the end still comes through 4.
6. `kill_group(pid)` (:491-497) sends SIGKILL to `-pid` and ignores every error (rule 6.36).
7. `reshape_result(o)` (:246-280): `bash -c o.cmd` (no extra argv), cwd and env as given, its own group, `o.input` on
   stdin. Output shares the 2 MiB cap with no `cut` flag. Deadline `o.timeout_s` as 3. It settles once, on the spawn
   error or `close` only (no exit grace): deadline cleared, group killed, then `reshape_answer`.
8. `reshape_answer(end, timeout_s, scrub)` (:283-294), first match wins: `err` is `run_output_tail(scrub(end.err))`;
   a spawn error answers `{why: "then could not start: <scrubbed message>", exit: null, err}`; a timeout
   `{why: "then timed out after <s> s", exit: 124, err}`; a code not 0 `{why: "then exited with <code or "a
   signal">", exit: code, err}`; else the scrubbed out parsed as JSON answers `{json}`, or when it does not parse
   `{why: "then printed no JSON[: <first line of the trimmed out, its first 120 units>]", exit: 0, err}`.

## Edge cases
- `echo $$; sleep 300 & sleep 301 & bash -c 'sleep 302' & wait`: four processes share the printed pgid; a kill
  leaves none of them (test `a cancel kills the whole process group`).
- A deadline of 1 s on `sleep 30` ends within 5 s with every process of the group gone (test `a timeout ...`).
- 4,000 lines of 51 characters keep a 64 KiB tail from a line start and no `cut`; 3,000,000 bytes give `cut` and a
  tail under 64 KiB (test `keeps the 64 KB tail`; the tail is the table's, through `run_output_tail`).
- `then` exiting 4 with stderr `cannot read it` answers `then exited with 4`, exit 4, err `cannot read it\n`;
  `echo not json` answers `then printed no JSON: not json` (test `a failing reshape ...`).
- JSON with surrounding whitespace parses; an empty out does not and says only `then printed no JSON`.

## Behaviour nobody wrote down
- Rule 6.24: run and `then` lead their own group. Rule 6.36: kill errors ignored. Rule 3.15: the deadline and the
  grace are clock timers. Rule 8.2: the cap counts bytes.
- The 1 s grace is what ends a run whose grandchild left the group with `setsid` holding the pipes (pilot finding).

## Decisions
1. The spawn error's message is node's form, `spawn bash <errno name>` (`ENOENT`, `EACCES`), since it reaches the
   record's `why`.
2. The new group is made with `setsid` in the child before exec, as libuv does for `detached`.
3. `RunRoad` is a trait; `start` answers `RoadStarted { kill: Rc<dyn Fn()>, pid: Option<u32> }` and takes `on` as a
   trait object whose `line` and `end` may be called with no borrow of the caller's state held (rule R1). `end` is
   never called inside `start` for the local road; another road may, and the table allows for it.

## After the port
- A `then` waits for `close` only, so a grandchild holding its pipes keeps it until its deadline.

## Tests

19 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| runs_road Behaviour 1: slate-runs.ts:301 | `runs_road::the_command_is_bash_c_with_the_values_as_positional_parameters` |
| runs_road Behaviour 1: the environment is exactly the one given, nothing inherited (slate-runs.ts:301) | `runs_road::the_environment_is_exactly_the_one_given` |
| runs_road Behaviour 1: no stdin is /dev/null; a given one is written then closed (slate-runs.ts:301, 322-325) | `runs_road::stdin_is_dev_null_unless_given_and_closed_after_it` |
| runs_road Behaviour 2: lines as they arrive, the rest at the end, \r kept (slate-runs.ts:315-318, 339-344) | `runs_road::a_stream_run_tells_each_line_as_it_comes_and_the_rest_at_the_end` |
| runs_road Behaviour 2: UTF-8 decoded across chunk edges (slate-runs.ts:306, 316) | `runs_road::a_character_split_across_two_writes_is_one_line` |
| runs_road Behaviour 2: no stream asked, no line told (slate-runs.ts:315) | `runs_road::a_run_not_streaming_tells_no_line` |
| runs_road Behaviour 2: one 2 MiB cap shared by out and err, cut set past it (slate-runs.ts:307-314) | `runs_road::out_and_err_share_one_2_mib_cap_and_say_they_were_cut` |
| runs_road Behaviour 2, pilot finding: a streamed line grows only from capped output | `runs_road::a_streamed_line_never_grows_past_the_cap` |
| runs_road Behaviour 3, 4: the deadline kills the whole group and the end says timed out (slate-runs.ts:329-332) | `runs_road::the_deadline_kills_the_group_and_ends_timed_out` |
| runs_road Behaviour 5: the kill takes every process of the group, and the end still comes (slate-runs.ts:358) | `runs_road::a_kill_takes_the_whole_group_and_the_end_still_comes_once` |
| runs_road Behaviour 4: a grandchild that left the group holding the pipes ends the run 1 s after the exit (slate-runs.ts:348-356) | `runs_road::a_setsid_grandchild_holding_the_pipes_ends_the_run_after_the_grace` |
| runs_road Decisions 1: a spawn that fails ends with node's message and no code (slate-runs.ts:347) | `runs_road::a_spawn_that_fails_ends_with_the_error_and_no_code` |
| runs_road Behaviour 6, rule 6.36: a kill of a group that is gone is ignored | `runs_road::kill_group_ignores_a_group_that_is_gone` |
| runs_road Behaviour 7, 8: a then reshapes stdin into JSON (slate-runs.ts:246-294) | `runs_road::a_then_reads_the_input_on_stdin_and_answers_its_json` |
| runs_road Behaviour 8: each failure of a then, said as the TS says it (slate-runs.ts:283-294; slate-runs.test.ts:407-412) | `runs_road::a_then_that_fails_says_why_with_its_stderr` |
| runs_road Behaviour 8: the out and err a then answers are scrubbed before the parse | `runs_road::a_then_parses_its_out_after_the_scrub` |
| runs_road Behaviour 8: first match wins, in the TS order: error, timeout, code, parse (slate-runs.ts:283-294) | `runs_road::reshape_answer_says_the_first_reason_in_order` |
| runs_road Behaviour 8: err is cut to its 64 KiB tail from a line start (run_output_tail) | `runs_road::reshape_answer_keeps_the_tail_of_a_long_stderr` |
| rule R1, runs_road Decisions 3: a road that ends inside start leaves no kill behind and finishes the run | `runs_table::values_reach_a_command_as_data::secrets::consent::process_lifetime::then::a_box_that_naps::timers::a_command_s_process::pilot_review::a_road_that_ends_inside_its_start_finishes_the_run` |
