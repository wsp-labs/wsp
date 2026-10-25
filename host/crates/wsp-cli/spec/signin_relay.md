# signin_relay

Source: `packages/host/src/signin-relay.ts` (470 lines) at 65f6b89c7, one unit (its URL and question scanners,
lines 62-160, are a seam an implementer may take first). TS tests: `test/signin-relay.test.ts` (27), with the link
`test/fake-pty-link.ts` ported to `tests/common/mod.rs`. Rust tests: `tests/signin_relay.rs` (42).

## What it does

Three roads over one daemon link (`PtyLink`: `op`, `on_event`, `closed`, `bare`): `relay_pty` shows a pty on this
terminal the way ssh would; `watch_pty` runs one with nobody at the terminal and reports pages and questions;
`run_quiet` runs one command and answers the words between the echo and the exit marker. Only URLs and declared
questions are read from the stream; codes and tokens never.

## Rules

1. `strip_osc8` removes `ESC ] 8 ; <params> ; <uri> (BEL | ESC \)` (:72-77). `urls_in` (:80-89): on the stripped
   text, matches of `https?://[^\s"'<>\x00-\x1f\x7f]+`; a match that runs to the very end is skipped (left for the
   next chunk); a trailing run of `.,;:!?)]` is cut; duplicates dropped, first-seen order.
2. `UrlScanner.feed` (:95-99) scans tail + chunk, keeps the last 2048 UTF-16 units of that as the tail, answers the
   URLs not reported before. `flush` scans tail + `\n` (:101-103). Seen URLs are kept for the scanner's life.
3. `QuestionScanner.feed` (:124-137): once every question was seen it answers nothing. It joins the raw tail and the
   chunk, keeps the last 2048 units raw, strips OSC 8 then every VT sequence (node `stripVTControlCharacters`) from
   the joined text, and runs each unseen question's `asks` (a JS regex, `JsRegex::find`) on it, in list order;
   each match is answered once with its matched text.
4. `hyperlink(url)` is `ESC ]8;; url ESC \ url ESC ]8;; ESC \` (:140-142).
5. `stage` (:174-181) sends `exec {cmd: STAGE, stdin: base64(command)}`. Refused unless `exitCode` is exactly 0 and
   `stdout` is a string opening with `/` with no control character (`has_control_char`): `could not put the command
   on that computer: <last line of stderr, else error, else "">` or `no reason given` when that has no line.
   `Staged.file` is `shellQuote(dir + "/line")`. `clear_staged` sends `exec {cmd: "rm -rf -- <shellQuote(dir)>"}`
   and swallows any failure.
6. `shell_line` (:185-187): `c=$(< FILE); rm -rf -- 'DIR'; printf '\036'; exec bash -c "$c"` and `\r`.
7. A refused op (`ok !== true`) is `<op> refused: <error>` or `no reason given` (:147-150); a create with no string
   `ptyId` is `pty.create answered without a pty id`.
8. `relay_pty` (:202-300), in order:
   1. `pty.create {cols, rows, shell: "bash", run?: BARE_BASH, env?}`, keys in that order, `run` only when
      `link.bare() == Some(true)`, `env` only when given; cols and rows from the output, else 80 and 24.
   2. listen to the link's events for this pty id: `pty.data` is written to the output whole, then (with a command)
      only what follows the first `\x1e` the tool printed is scanned; each fresh URL counts, becomes the offer
      `{url, at: now(), typed: false}`, and writes `\r\n  ` + dim(`link`) + `  ` + hyperlink + `  ` +
      dim(`o opens it on this computer`) + `\r\n`; any chunk restarts a flush timer (`flush_ms`, 300) that offers a
      URL ending the chunk. `pty.exit` sets `exitCode` and ends the wait. Dim is node's styleText: plain off a
      terminal.
   3. `closed` settling before an exit marks `dropped`; the timeout (`timeout_ms`) marks `timedOut`. Either ends it.
   4. `pty.attach` (refused: rule 7 after the finally); raw mode on when the input is a terminal; keystrokes and
      resizes followed; with a command, `stage` then `pty.write` of `shell_line`; wait.
   5. A keystroke chunk that is exactly `o`, while an offer stands untyped and `now - at <= OFFER_MS` (60 000),
      opens `callback_url()` if it answers, else the offered URL; `on_consent(url)` first; the result writes
      `\r\n  ` + dim(`opened the sign-in page; it returns to the machine on its own` when a page was opened,
      `opened on this computer; if the page shows a code, paste it into the terminal above` otherwise, or
      `could not open a browser here; use the link above` when the open failed) + `\r\n`; a success counts
      `opened`. That `o` is not sent. Any other chunk marks the offer typed when it holds a printable character
      (`[\x20-\x7e -￿]`) once CSI and SS3 sequences are out, and goes to `pty.write`, failures swallowed.
   6. A resize sends `pty.resize {ptyId, cols, rows}` with the output's size now, else the opening size.
   7. Finally, always: timers cleared, input no longer read, raw mode restored to what it was (only if this set
      it), events detached, `pty.kill`, the staged folder cleared. Answer `{exitCode (-1 unless it exited),
      timedOut, dropped, urls, opened}`.
9. `watch_pty` (:347-409): create at 200 by 50 with the same shell rule; only output after `\x1e` counts (always,
   since there is always a command). Per chunk: `on_data`, then each newly asked question (its `Answer` typed with
   `pty.write`, failures swallowed; `on_question` for every one), then URLs to `on_url`, then `on_scanned`; the flush
   timer reports and calls `on_scanned` again. The first of exit, closed, stop and timeout ends it and marks only
   itself. After attach, `on_typing(Some(write))`, where a refused write fails with rule 7; then stage and type the
   line. Finally `on_typing(None)`, detach, kill, clear.
10. `run_quiet` (:424-470): create `{cols: 200, rows: 50, shell: "/bin/sh", env: {PS1: "", ...env}}`; collect every
    data chunk; attach; stage; type `c=$(cat FILE); rm -rf -- 'DIR'; eval "$c"; printf '\nWSP_STATUS %s\n' $?;
    exit\r`. Ends on exit, timeout, or closed (dropped only when no marker line has arrived). Lines are the text with
    `\r` removed split on `\n`; the marker is the last whole line matching `^WSP_STATUS \d+$`; exit code is its
    number, else -1; the output is lines 1 up to the marker (or all after the echo), joined with `\n`, VT stripped,
    trimmed.
11. A refused create throws before anything goes raw or any other op is sent; a refused attach throws after the pty
    is killed, with the terminal untouched and nothing typed.

## Decisions

1. Callbacks are `Arc<dyn Fn>`; `open` returns a boxed future; `now` is milliseconds as f64.
2. `relay_pty` borrows the `RelayTerminal` (input and resize receivers, raw mode, output) and hands it back.
3. Questions are told apart by their place in the list, not identity.

## After the port

1. The relay's offer can open a URL the shell's banner printed when no command is given (the bare shell scans
   everything).
2. `run_quiet` reads the first line as the echo; a shell that prints a banner first shifts the output by a line.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | osc8_wrappers_come_off_trailing_punctuation_drops_and_a_match_at_the_chunks_end_waits, a_url_ends_at_space_quotes_angles_and_controls_and_sheds_its_trailing_punctuation |
| 2 | a_url_split_across_chunks_is_reported_once_and_the_same_url_again_not_at_all, flush_settles_a_url_that_ended_the_last_chunk_once, the_scanner_keeps_a_tail_of_2048_characters |
| 3 | each_question_is_reported_once_past_the_colours_and_reading_stops_once_every_one_is_seen, a_question_split_across_chunks_and_an_escape_split_with_it_are_seen, a_question_carries_the_tools_own_words, a_question_painted_inside_a_hyperlink_is_still_seen |
| 4 | a_terminal_hyperlink_wraps_the_url_in_osc_8 |
| 5, 6 | a_stage_that_does_not_answer_a_plain_absolute_folder_is_refused_with_its_last_line, the_typed_line_reads_the_staged_command_removes_its_folder_marks_the_tool_and_execs_it |
| 7, 11 | pty_create_refused_throws_the_daemons_reason_and_nothing_goes_raw_or_follows, pty_create_with_no_pty_id_throws, pty_attach_refused_throws_with_the_pty_killed_and_the_terminal_untouched |
| 8.1 | a_pty_on_a_computer_somebody_owns_runs_bash_reading_no_startup_file, the_environment_rides_the_pty_create_after_the_shell, a_terminal_with_no_size_opens_the_pty_at_80_by_24 |
| 8.2 | a_printed_url_is_shown_once_with_the_o_offer_and_o_opens_it_here_without_reaching_the_pty, the_offer_line_is_the_dim_link_word_the_hyperlink_and_the_dim_offer, only_a_page_the_tool_printed_is_offered_and_the_shells_banner_is_text, a_url_that_ends_the_chunk_is_offered_after_a_short_quiet |
| 8.3 | a_tool_that_never_finishes_times_out_and_its_pty_is_killed, a_link_that_drops_under_the_pty_ends_the_relay_with_the_terminal_restored |
| 8.4, 8.6, 8.7 | the_relay_opens_at_the_terminals_size_relays_both_ways_follows_a_resize_and_restores_the_terminal, a_bare_shell_types_no_command_line_and_leaves_raw_mode_alone_off_a_tty |
| 8.5 | o_opens_the_forwarded_page_when_one_arrived_and_the_printed_link_otherwise, arrow_keys_and_other_csi_sequences_keep_the_o_offer_and_typed_text_ends_it, o_is_the_offer_only_as_a_whole_chunk, an_o_long_after_the_url_is_a_keystroke_and_a_failed_open_says_so |
| 9 | an_answer_is_typed_on_the_tools_pty_as_its_line_arrives_and_nothing_for_the_persons, nothing_the_shell_printed_before_the_tool_started_is_answered_reported_or_handed_on, a_watched_pty_is_wide_types_nothing_back_reports_each_page_once_and_ends_with_the_tools_exit, the_callers_stop_a_dropped_link_and_a_timeout_each_end_it_with_no_exit_code_and_kill_the_pty, the_first_end_wins_and_a_later_one_changes_nothing, on_typing_hands_the_ptys_input_while_attached_and_none_after |
| 10 | the_quiet_run_returns_only_what_sits_between_the_echo_and_the_exit_marker, the_quiet_runs_environment_rides_after_an_empty_ps1, the_quiet_runs_output_is_plain_text_with_colours_taken_out, the_last_whole_marker_line_is_the_exit_and_everything_after_it_is_dropped, a_status_command_that_never_answers_times_out, a_link_that_drops_during_the_status_command_reads_as_dropped |
