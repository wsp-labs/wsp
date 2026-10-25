# child_exec: one child process, a fed spawn, the config write here, the env road

Source at `a4033dc34`: `packages/engine/src/child-exec.ts` (140), `spawn-fed.ts` (29), `config-here.ts` (30),
`run-env.ts` (47). Rust: `src/child_exec.rs`, `src/spawn_fed.rs`, `src/config_here.rs`, `src/run_env.rs`. Tests:
`tests/child_exec.rs`, hard tests in `tests/child_exec_hard.rs`. Oracle: `small.json` (`lineFeed`), `commands.json`.

## run_child (child-exec.ts:63)

- H1. The child leads its own process group (`setsid`-free `process_group(0)`, the TS `detached: true`); cwd and the
  whole set of variables only where given (`env` given means nothing inherited).
- H2. `stdin` given: written whole, then closed; a broken pipe on that write is ignored (the child's own failure is
  the answer). Not given: stdin is inherited.
- H3. stdout and stderr are read as UTF-8 (lossy) to strings; `on_line` gets each complete stdout line (no `\n`) as it
  arrives, and once at the very end the held partial line if it is not empty. stderr has no lines.
- H4. The deadline (`timeout_ms`): `SIGKILL` to the whole group (`-pid`); the answer is `CHILD_TIMED_OUT` 124 whatever
  the child's own code, with the output read so far. A group already gone is no failure.
- H5. The answer comes after the child exits: its code then (124 if the deadline fired; else the exit code; a signal
  death -1; neither 0). After the exit the pipes are read until they close, or have been quiet for `DRAIN_MS` 200 (each
  chunk restarts the quiet), or `DRAIN_CAP_MS` 1000 have passed since the exit; then the pipes are dropped. A child
  that leaves a grandchild holding the pipes open answers within about a second, writing or not.
- H6. A spawn failure (no such file, not executable): 127, stderr gains the error's message and `\n`; stdout empty.
- H7. `LineFeed`: `feed(chunk)` appends, answers every line the buffer now completes (split on `\n` only, a `\r`
  kept), then cuts the held tail to its last `keep` UTF-16 units; `flush()` answers the held text once if not empty and
  holds nothing after. Oracle `lineFeed` (keep 4): `["ab", "éfghij\r", "🎉🎉"]`, the second flush nothing. A cut that would
  split a surrogate pair writes U+FFFD for the half it keeps, as the coordinator ruled for every such
  cut on #2446 (the TS keeps a lone surrogate, which reads as U+FFFD once encoded).

## spawn_sync_fed (spawn-fed.ts:14)

- H8. The bytes are written with mode 0600 and exclusive create to `in` inside a fresh `<tmp>/wsp-stdin-XXXXXX`, the
  file opened for reading, and the folder removed (recursively, ignoring errors) before the child runs, whatever
  happened; the child's stdin is that open file; stdout and stderr piped and returned as bytes. The open file is closed
  after. A spawn failure is `status None` with `error` the message; a timeout kills with SIGTERM and `error` says so.

## config_here (config-here.ts)

- H9. `config_landed(res, file, shown)`: `config_refusal(exit, stdout, file, shown)` answering words is `Other {message:
  words, code: Some("usage")}`. Else exit 0 is fine; else `<shown(file)> stands as it was: the write did not finish
  (<first>).` with `first` the first line of `(stderr or stdout).trim()` (stderr wins when it is non-empty), `exit <code>`
  where that is empty. `shown` defaults to the path itself.
- H10. `write_config_here(file, base, sum, text)`: `/bin/sh -c <config_write_line({file, base, bytes: UTF-8 length,
  sum?})>` fed the UTF-8 bytes by `spawn_sync_fed`, timeout `WRITE_MS` 60000, the host's own variables with `PATH`
  exactly `/usr/bin:/bin`. Status `None` reads as exit 1; empty stderr takes the spawn error's message.

## run_env (run-env.ts)

- H11. `env_input`: `NAME=value\0` per entry in map order, nothing escaped; oracle `envInput` (an empty name gives
  `=\0`). `with_env_from_input(s)` = `ENV_FROM_INPUT + "\n" + s`.
- H12. `env_exec`: a machine that takes stdin gets `exec(command, {timeout_ms, stdin: env_input})`. Else the oracle's
  two lines: `printf '%s\0' <shell_quote("A=1")> <shell_quote("B=...")> | {\n<command>\n}`, and for an empty map
  `{\n<command>\n} </dev/null`; no stdin sent.
- H13. `harness_exec(machine, timeout_ms)`: no variables runs `exec(command, {timeout_ms})`; with variables
  `env_exec`; the answer is stdout whatever the exit code.

## Caps and restart

124, 127, -1; 200 ms quiet; 1000 ms after exit; 60000 ms for the config write. Nothing is kept across calls; the
stdin file never outlives the spawn call.

## Rules and their tests

Files: `child_exec.rs` (ported), `child_exec_hard.rs` (hard).

| Rule | Tests |
|---|---|
| H1, H4 | ported `returns_at_its_deadline_with_the_commands_own_children_gone_the_output_so_far_kept`; hard `a_deadline_kills_the_whole_group_and_answers_124` |
| H5 | ported `answers_when_the_command_exits_though_a_child_it_left_behind_still_holds_its_output`, `answers_a_command_that_exited_0_before_its_deadline_with_0_though_what_it_left_outlives_the_deadline`, `answers_a_command_whose_child_keeps_writing_within_a_second_of_the_commands_exit`; hard `an_exit_answers_within_the_drain_cap_while_a_grandchild_holds_the_pipes` |
| H2 | hard `stdin_is_fed_and_closed_and_a_given_env_is_all_there_is` |
| H3 | hard `on_line_gets_each_line_then_the_partial_one_once` |
| H6 | hard `an_unstartable_child_answers_127_with_the_error` |
| H7 | hard `line_feed_matches_the_ts_oracle` |
| H8 | ported `reads_every_byte_and_their_end_from_a_file_never_from_a_pipe_whose_end_can_be_lost`, `gives_the_child_nothing_to_read_where_the_bytes_are_empty`; hard `the_stdin_file_is_gone_before_the_child_reads_it` |
| H9, H10 | hard `config_landed_reads_refusals_and_first_lines` |
| H11, H12 | ported `gets_them_on_the_execs_input_where_the_machine_takes_it_and_no_exec_text_holds_the_value`, `gets_them_piped_in_from_the_execs_text_on_a_machine_whose_exec_drops_its_input`, `reads_nothing_and_waits_on_nothing_when_it_is_handed_none_on_either_road`; hard `env_exec_writes_the_ts_lines` |
| H13 | hard `harness_exec_answers_stdout_whatever_the_exit` |
