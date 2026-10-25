# exec_run: one long guest command through short execs

Source: exec-detached.ts:229-399 at `a4033dc34`. Module `exec_detached::run`, re-exported from `exec_detached`.
Shared rules: `roads-README.md`. Oracle: `tests/fixtures/roads/exec_run.json` (every command, byte for byte, with
the base pinned to `abababababab`).

## Commands (public so a test fixes the base)

- `launch(machine, base, script)`: `put_files` of one file `{base}.sh` holding the script with `before = ["u=$(umask)",
  "umask 077", "b={base}", "mkdir \"$b.d\" 2>/dev/null || { echo WSP_LAUNCHED; exit 0; }"]` and `after = [the setsid
  line, "echo $! > \"$b.pid\"", "echo WSP_LAUNCHED"]` (:231). The setsid line, exact: `setsid nohup bash -c '( umask
  "$1"; exec bash "$0.sh" ) > "$0.out" 2> "$0.err" < /dev/null; echo $? > "$0.exit"' "$b" "$u" > /dev/null 2>&1 &`.
  The `.d` claim makes a second send of the same launch print the handshake and start nothing. The mask is the
  run's alone: every run file is 0600 from birth, the script runs under the mask the shell came with.
- `poll_command(base, out_offset, err_offset)` (:248): lines `b={base}`, `echo WSP_POLL`, the exit file, `tail -c
  +{out+1}` of `.out` capped at `head -c 262144` and `base64 -w0`, the same for `.err`, the pid check printing `up` or
  `down`, `echo WSP_POLL_END`. The exit file is read before the streams.
- `kill_command(base)` (:260): one line: TERM to `-pid` and `pid`, `sleep 2`, KILL to both, `true`.
- `clean_command(base)` (:266): `rm -rf {base}.*` (base unquoted; it is the run's own random name).
- `parse_poll(stdout)` (:277): the first line exactly `WSP_POLL`, and the line five after it exactly
  `WSP_POLL_END`, else none. Exit is that block's line 1 JS-trimmed; out and err are lines 2 and 3 base64-decoded
  (Node's lenient decode; an empty line is no bytes); up is line 4 exactly `up`.

## `LineStream` (:285)

Bytes in, lines out per stream. Every chunk is kept for the stream's text (decoded once, UTF-8 lossy, at the end) and
counted into `offset`. With a listener, bytes are held until `\n`; each complete line is decoded alone and handed
over unless empty. `flush` hands over what is held with no newline (once nothing more is coming).

## `exec_detached(machine, script, opts, run_dir = RUN_DIR)` (:319)

1. `poll_ms = opts.poll_ms or 2_000`; `base = "{run_dir}/{12 hex}"`; the clock starts before the launch. Every exec
   this function sends itself (polls, kill, clean) carries `timeout_ms = 20_000` and no key.
2. Already cancelled: answer `{ 130, "", "cancelled\n" }` and send nothing.
3. Launch through `opts.launch_on` or the machine. The launch failing (rejects): send `clean_command` on the machine
   (its own failure ignored) and fail with the launch's error. Answering exit not 0 or no `WSP_LAUNCHED` in stdout:
   clean, then fail with `launch failed on {id}: nothing came back saying WSP_LAUNCHED, the word the guest prints once
   the run is up; {machine_answer}`.
4. Loop, with `wait = min(250, poll_ms)` and `downs = 0`:
   - Cancelled: kill, clean (each failure ignored), answer 130 with the output so far and note `cancelled`.
   - Past the deadline (`now - start > deadline_ms`, strictly): kill; one more poll whose output is read if it
     parses (its failure ignored); clean; answer 124 with the output so far, no note.
   - Poll. A failure that is the guest unusable, the machine stopped, or `is_missing` fails the run at once with
     that same error: no kill, no clean, no later poll. Any other failure counts as no answer.
   - No answer (failed or unparsed): pause `poll_ms` (a cancel cuts the pause short) and loop.
   - Read: push out then err. A chunk of exactly 262,144 bytes on either stream loops at once with no pause and no
     exit check, from the new offsets.
   - Exit line non-empty: clean, answer `Number(exit)` where it is an integer, else -1 with the note.
   - Down: the second down of the run (counted across every poll; an up between never resets it) cleans and answers -1 with the note
     `wsp: the command ended without reporting an exit code`; the first loops at once with no pause.
   - Up: pause `wait` (cut short by a cancel), then `wait = min(poll_ms, wait * 2)`.
5. The answer: both streams flushed to the listener (stdout's tail before stderr's), `stdout` all of out, `stderr`
   all of err; a note is appended as `{note}\n` after one `\n` where stderr is non-empty and does not end in one.

The run's files on the guest survive a host restart until cleanup; nothing in memory does. A cancel and the deadline
kill the session's whole group; the unusable, stopped and missing paths leave the files (nothing can run there).

## Pins

| Rule | Tests |
|---|---|
| offsets, streams, exit file, one clean | `launches_the_script_detached_reads_both_streams_from_where_the_last_poll_stopped` |
| every exec bounded at the inline cap | `every_exec_it_sends_is_bounded_at_the_inline_cap` |
| full chunk re-reads at once; exit waits for the rest | `a_full_chunk_is_followed_at_once_by_the_next_read`, `the_exit_file_ends_the_run_only_once_nothing_is_left_to_read` |
| deadline kill and 124 | `past_the_deadline_it_kills_the_recorded_pid_with_its_group`, `a_nap_that_outlasts_the_deadline_still_ends_in_124`, `the_deadline_kill_ends_the_launched_session_for_real` |
| cancel paths | `a_cancel_kills_the_recorded_group_cleans_up_and_answers_at_once`, `a_cancel_during_the_pause_between_polls_cuts_the_pause_short` |
| naps retried; unusable, missing, stopped end at once | `a_poll_that_fails_while_the_machine_naps_is_retried`, `a_poll_that_meets_a_guest_the_backend_calls_unusable`, `a_poll_that_meets_the_providers_404`, `a_poll_that_meets_a_machine_the_provider_stopped` |
| -1 with the note | `a_leader_gone_without_an_exit_file_answers_minus_one` |
| umask before the script lands, 0600 files | `the_launch_sets_the_umask_before_the_script_is_written`, `the_script_and_the_streams_of_a_run_are_readable_by_nobody_but_their_owner`, `a_detached_script_creates_files_as_its_own_shell_would` |
| launch in one exec or pieces, retried pieces | `a_script_that_fits_one_exec_body_goes_up_in_one_launch_exec`, `a_40_kb_script_goes_up_in_pieces`, `a_piece_posted_twice_lands_once`, `a_script_over_the_cap_runs_for_real_from_its_pieces` |
| launch failures and cleanup | `a_piece_that_does_not_confirm_fails_the_run_before_the_launch`, `a_launch_the_machine_answered_nothing_to_names_the_handshake`, `a_launch_that_meets_the_providers_404_fails_with_that_answer`, `a_launch_that_does_not_confirm_fails_the_run`, `a_launch_that_lands_its_script_and_then_fails_leaves_nothing` |
| launch twice starts once | `a_launch_posted_twice_under_one_base_starts_the_script_once` |
| real commands, no files left | `runs_the_real_launch_poll_and_cleanup_commands` |
| commands byte for byte, note placement | `launch_poll_kill_and_clean_commands_match_the_ts`, `parse_poll_reads_only_a_whole_block`, `the_note_goes_on_its_own_line` |

## Pins (wave 2)

Scenes in `oracle/roads_scenes.oracle.ts` (`runs`), replayed by `tests/roads_scenes.rs::<name>`: every command sent
(the base masked), every line handed to `on_line`, and the answer, as the TS made them. Poll pace 1 ms, deadline
60,000 unless a line says.

- R-1 The exit line is read with JS `Number` after JS trim: `0x10` is 16 (`an_exit_in_hex_is_a_number`); `1.5` is not
  an integer, so -1 with the note on its own line after the stderr so far
  (`an_exit_with_a_fraction_is_minus_one_with_the_note`).
- R-2 Within a poll stdout's complete lines go to the listener before stderr's; the tails with no newline go at the
  end, stdout's then stderr's: polls of out `a\nb` err `c\n`, then out `` err `d` hand over `a`, `c`, `b`, `d`
  (`lines_go_stdout_then_stderr_per_poll_and_the_tails_at_the_end`).
- R-3 An empty line is never handed over; a `\r` stays on its line (`a_blank_line_is_not_handed_over_and_a_cr_is_kept`).
- R-4 A full chunk is read again at once even when its poll carries an exit, and the exit is taken from the poll
  after: stdout is the 262,144 bytes and `tail`, and the second poll asks from offset 262,144
  (`a_full_chunk_with_an_exit_reads_again_before_the_exit`).
- R-5 Past the deadline the last poll's streams are read and its exit line is ignored: the answer is 124 with no
  note, and the commands are launch, kill, poll, clean (`past_the_deadline_the_last_polls_exit_is_ignored`, deadline
  0 with a launch that takes 5 ms).
- R-6 A block whose sixth line is not `WSP_POLL_END` is no answer, even with `WSP_POLL_END` later; the run pauses and
  polls again (`a_block_closed_late_is_no_answer`).
- R-7 The stream lines are base64 as Node reads it: no padding needed, and decoding stops at the first `=`, so `YQ`
  is `a` and `Yg==Yw==` is `b` (`the_streams_decode_as_node_decodes_base64`).
- R-8 A poll whose exec fails with any other error is no answer and is asked again with the same offsets
  (`a_poll_that_rejects_is_no_answer_and_is_asked_again`).
- R-9 A UTF-8 character split across two polls reaches the listener whole: bytes are held until `\n`, then the line
  is decoded (`a_utf8_char_split_across_polls_reaches_the_listener_whole`).
- R-10 The deadline and the pauses read the real clock (`Date.now()` and timers in the TS), not an injected one.
