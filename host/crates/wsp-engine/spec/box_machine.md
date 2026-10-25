# box_machine (box-backend.ts:1046-1430)

The environment landing, the root wrapper, the daemon's exits, and `BoxMachine`. Rust:
`src/box_backend/machine.rs`. Other slices it calls: exec-detached's `exec_detached(machine, script, RunOptions)`,
`DEADLINE_EXIT` (124), `DAEMON_ENV_FILE`, `INLINE_EXEC_MS` (20,000); run-env's `DROP_SUDO_MARKS`; preview's
`DAEMON_PORT` (7070); golden_seam's `EXEC_ENV`.

## Text (pure, byte for byte, oracle `box.json`)

- `env_landing_script(envs)` (:1054): `mkdir -p <dir of DAEMON_ENV_FILE>`, `cat > <file> <<'WSP_ENV'`, one
  `NAME="value"` line per env in order with `\` and `"` backslashed (nothing else escaped, a newline stays), `WSP_ENV`,
  `chmod 0600 <file>`; paths shell-quoted. No env: the same script with no lines between.
- `sudo_command(cmd)` (:1067): `sudo -n bash -c ` + `shell_quote(DROP_SUDO_MARKS + "\n" + EXEC_ENV + "\n" + cmd)`.
- `DAEMON_EXITS_CMD` (:1079): the TS script. `daemon_exits_of(stdout)` (:1098): lines trimmed; no line starting with
  `WSP_DAEMON_EXITS` is `None`. After the **first** such line, the non-empty lines, each kept at its **last**
  appearance in order; with any, the last four, each over 120 UTF-16 units cut to 120 and `…`, joined `"; "`. With
  none, the mark line's words after it: sub, result, status; where sub starts `auto-restart` or is `failed` and result
  is non-empty, `exit_words(result, status)` (exit-code `exit status N`; signal and core-dump `killed by signal N`;
  oom-kill `killed for want of memory`; else the result word); otherwise `it printed nothing`.

## BoxMachine

A handle: backend, id, kind, labels, seen, replayed; `deletion` set by kill. `stream_url` none; no daemon supervisor
(systemd). `offers()`: resume_at, preview_url, prove_road, start_daemon, daemon_exits, put_bytes, read_bytes,
describe. Nothing it holds survives a restart beyond the provider's box.

- `exec(cmd, opts)` (:1128): timeout `opts.timeout_ms` or `INLINE_EXEC_MS` (20,000, so a plain exec asks `timeoutSeconds: 20`). Above 30,000 (`BOX_INLINE_MAX_MS`) it goes
  `exec_detached` with `deadline_ms = timeout`, `poll_ms = budgets.poll_ms`; the detached road's own execs ask 20,000
  and stay inline. `idempotency_key` and `stdin` are ignored.
- `exec_inline(cmd, timeout_ms, signal)` (:1147): `while_restoring` of POST `/sandboxes/<id>/commands` with
  `{ command: sudo_command(cmd), timeoutSeconds: max(1, ceil(timeout / 1000)) }`, cap `timeoutSeconds * 1000 +
  30,000`. A failure whose message matches `/cwd must be an existing directory/i` is `guest_unusable(id, "Boat"
  provider key name, no_work_folder_line(name, "/home/user", message), status)`; kind unknown with status 500 is
  `guest_unusable(id, name, message, 500)`; code `machine_not_running` is `machine_stopped(id, e)`; else as itself.
  An answer whose stderr matches `/^(bash|sudo): error while loading shared libraries: .*: Error 24/m` is
  `guest_unusable(id, name, stderr.trim(), 200)`. Else `{ exitCode: exitCode, or 124 where timedOut, or -1; stdout
  or ""; stderr or "" }`.
- `prove_road(signal)` (:1174): `exec_inline("true", 20,000, signal)`; a still-restoring failure becomes
  `restore_unfinished(id, e)`.
- `start_daemon` (:1137): `exec("systemctl start wsp-daemon.service")`. `daemon_exits` (:1141):
  `daemon_exits_of(exec(DAEMON_EXITS_CMD).stdout)`.
- `run(script, opts)` (:1180): `exec_detached` with `poll_ms` the opts' or the backend's.
- `land_envs(envs)` (:1188): exec the landing with timeout 60,000 (so detached); exit 0 is done; exit 124 inside
  `restore_ms` from the start sleeps the poll and lands again; any other exit, or 124 past the budget, is
  `EnvNotLanded { exit_code, detail: stderr.trim() or stdout.trim() }`.
- `snapshot(name, _, _)` (:1202): `saved = box_snapshot_name(name)`; POST `/named-snapshots { sandboxId, name: saved
  }`; then loop: sleep the poll, GET `/named-snapshots/<saved>`; ready answers `saved`; failed fails `named snapshot
  <saved> of <id> failed[: <error>]`; at or past `snapshot_ms` fails `... still reads saving after <round(s)> s`.
  Life and progress are ignored (any life saves).
- `pause(wait)` (:1221): budget `wait` or `pause_ms`. Read the view: archived is done with no call; archiving sends
  no stop; anything else POST `/stop {}`. Then loop: sleep the poll, read; archived is done; any state but archiving
  warns `<id>: pause did not take: the provider left it <state> and refused the stop: <stop_refusal_of>` and fails
  `stop_refused(id, said)`; at or past the budget warns `<id>: <move_timed_out_line("pause", elapsed, mapped state)>`
  and fails `MoveUnanswered` with those words. Never `force`.
- `resume(signal)` / `resume_at(cpu, mem, signal)` (:1249-:1273): `before_start()`; ttl from the timer on record
  re-armed from now (`now + span_ms`) or the cap; POST `/resume { ttlSeconds, type? }` (`type` only for resume_at, the
  class of `{cpu, mem}`) with the signal. A refusal with code `snapshot_data_expired` is `DiskGone { line:
  BOAT_DISK_GONE_LINE, said: message }`; others go through `start_refused(e, labels.name)`. Then `counted()`,
  `settle` until running (budget resume, the signal), `prove_road(signal)`.
- `kill` (:1277): DELETE `/sandboxes/<id>` with `X-Ascii-Confirm-Delete: <id>`; `deletion` = the answer's
  `operation.id`; a missing answer is done; then `forget_backstop(id)` in both cases; any other failure answered
  before forgetting.
- `state` (:1287): `state_of(view.state)`; missing is gone.
- `describe` (:1297): `{ cpu: vcpu?, memMb: memoryGB * 1024?, diskGb: the class's disk?, createdAt? }`.
- `preview_url(port)` (:1314): port 7070 skips the firewall and asks `public: true`; any other opens the port first
  and asks `public: false`. POST `/host { port, public }`; `{ url, token: the url's _token query or "", expiresAt:
  9007199254740991 }`.
- `open_port(port)` (:1330): one `ufw status | grep -q '^<port>/tcp '` standing is done. Else adds until `start +
  restore_ms + 30,000`: `ufw allow <port>/tcp` (a failure reads exit -1), exit 0 sets `held = now` and sleeps 5,000,
  else keeps the last 200 chars of stderr or stdout trimmed and sleeps the poll. While held, each check standing for
  30,000 since the add returns, standing sleeps 5,000, gone clears `held`. Past the add window it warns `<id>: the
  firewall did not hold a rule for port <port>[ (<said>)]; the route is minted without one` and returns Ok.
- `put_bytes(path, bytes, timeout)` (:1360): pieces `max(1, ceil(len / 5,242,880))`. One piece to a path under
  `/tmp/` or `/home/user/`: one PUT `/files { path, content: base64, encoding: "base64" }` through
  `while_restoring`, cap `timeout` where given. Else pieces `/tmp/wsp-put-<12 hex>.<i>`, each PUT, then one exec of the
  TS's four-line join script (done if the first piece is gone and the target is already at the size; `mkdir -p` the
  folder; `cat`, `chmod 0644`, `rm`; check the size or print `WSP_SHORT <size>`); non-zero fails `<path> did not land
  on <id> (exit <code>): <stderr or stdout trimmed>`. Answers `Ok(None)`.
- `read_bytes(path, size, timeout)` (:1393): with no size, exec `stat -c %s <path>`; a non-zero exit or a non-integer
  fails `<path> on <id> could not be read (exit <code>): ...`. Up to 4,194,304 under `/tmp/` or `/home/user/`: one GET
  `/files?path=<encoded>&encoding=base64`. Else `split -b 4194304 -d -a 4` into `/tmp/wsp-get-<12 hex>.NNNN` and
  `chmod 0644`, a failed cut fails `... was not cut for reading ...`; GET each; a total not equal to the size fails
  `<path> on <id> read <n> bytes of <size>`; the pieces are removed in every case (its failure ignored).
- `download_url` / `upload_url` fail with the TS's two sentences (:1423, :1427).

## Rules against tests

| Rule | Tests |
|---|---|
| text: env script, sudo wrapper, daemon exits | `box_machine_oracle::*` (5), `box_machine_exec::the_environment_script_writes_the_daemons_own_file_*`, `box_machine_daemon_exits::*` (12) |
| exec inline, timeout seconds, detached past the cap | `box_machine_exec::runs_every_command_as_root_through_sudo_*`, `box_machine_exec::an_exec_asked_for_longer_than_the_edge_allows_*`, `box_machine_exec::starts_the_daemons_unit_*` |
| guest faults: EMFILE, 500, work folder, stopped | `box_machine_exec::a_command_road_whose_shell_dies_on_the_open_file_limit_*`, `box_machine_exec::a_commands_endpoint_answering_500_*`, `box_machine_exec::a_box_whose_work_folder_is_gone_*`, `box_machine_exec::runs_every_command_as_root_through_sudo_*` (machine_not_running) |
| restoring: asked again, proof typed | `box_machine_exec::a_box_that_reads_ready_but_refuses_commands_*`, `box_machine_exec::a_proof_still_refused_as_restoring_*`, `box_machine_exec::a_box_answering_502_*`, `box_machine_exec::a_resume_whose_box_reads_ready_and_then_cannot_run_a_command_*` |
| pause: one stop, archiving, refused, budget, wait | `box_machine_moves::pauses_with_one_stop_*`, `box_machine_moves::a_box_already_archiving_or_archived_*`, `box_machine_moves::a_stop_the_provider_refuses_*`, `box_machine_moves::a_pause_given_a_longer_wait_*`, `box_machine_moves::a_stop_that_never_lands_*` |
| resume: class, timer re-armed, disk gone, missing, cancel | `box_machine_moves::resumes_*` (2), `box_machine_moves::a_plain_resume_names_no_class_*`, `box_machine_moves::a_resume_boat_refuses_for_a_deleted_disk_*`, `box_machine_moves::a_resume_arms_the_timer_*`, `box_machine_moves::a_resume_of_a_box_the_provider_lost_*`, `box_machine_moves::a_resume_the_caller_stopped_waiting_on_*`, `box_machine_moves::names_the_root_disk_each_size_reads_*` |
| routes and the firewall | `box_machine_moves::routes_the_daemons_port_public_*`, `box_machine_moves::mints_a_private_route_*`, `box_machine_moves::mints_the_route_with_one_read_*`, `box_machine_moves::a_rule_the_boxs_own_agent_wipes_*`, `box_machine_moves::a_rule_the_guest_refuses_while_it_boots_*` |
| bytes in and out, pieces, the join | `box_machine_moves::puts_bytes_through_put_files_*`, `box_machine_moves::a_join_over_a_file_already_at_the_new_bytes_size_*`, `box_machine_moves::bytes_past_the_files_endpoints_cap_*`, `box_machine_moves::reads_a_file_off_the_box_*`, `box_machine_moves::the_signed_url_roads_say_where_a_boxs_bytes_go_instead` |
| snapshot, kill, state, describe, desktop | `box_machine_moves::a_snapshot_is_a_named_snapshot_*`, `box_machine_moves::a_save_the_provider_fails_*`, `box_machine_moves::kills_with_the_confirm_header_*`, `box_machine_moves::reads_the_providers_states_*`, `box_machine_moves::describes_the_class_*`, `box_machine_moves::a_desktop_machine_boots_the_same_box_*` |

## Pins (wave 2)

Each line is a scene in `oracle/box_scenes.oracle.ts`, replayed by `tests/box_machine_scenes.rs::<name>` against
`tests/fixtures/scenes/box.json`: every request (method, path, query pairs in order, headers, the body's exact bytes),
every sleep, every warn line, the clock's travel and the answer must be the TS's. Budgets are the test ones (poll 10,
every other budget 100) unless a line says.

- B-M1 The firewall loop reads, in this order each pass: while `held` is set, one standing check; standing and
  `now - held >= 30,000` returns; standing and younger sleeps 5,000 and goes round without looking at the add window;
  gone clears `held` and falls through. Then, only with `held` unset, `now >= start + restore_ms + 30,000` ends the
  loop (warn, Ok), else one add. A rule wiped after the window ends the loop at that read, with the warn and no
  second add (`firewall_wiped_past_the_add_window_warns_without_words`).
- B-M2 `said` is set by a refused add and never cleared by a later add that succeeds, so the closing warn names the
  last refusal even when an add held in between (`firewall_keeps_the_last_refusal_after_a_later_add_held`). It is
  `(stderr || stdout).trim()` then its last 200 UTF-16 units: stderr is taken when non-empty before trimming.
- B-M3 A stderr of white space alone wins over stdout and trims to `""`, so the warn has no `( ... )`
  (`firewall_blank_stderr_hides_stdout`, poll 1,000: 31 adds, one every 1,000 ms, then the warn).
- B-M4 A refusal longer than 200 units keeps its last 200 (`firewall_keeps_the_last_200_units_of_a_long_refusal`).
- B-M5 A standing check whose exec fails reads as not standing (exit -1), as does an add whose exec fails, whose
  `said` is the error's message (`firewall_standing_read_that_throws_reads_as_not_standing`).
- B-M6 The route's token is the URL's first `_token` query value, decoded as `URLSearchParams` decodes (`+` is a
  space, `%2F` is `/`); the url is answered as given. Port 7070 sends no command at all, and the body is
  `{"port":7070,"public":true}` in that key order (`route_token_is_the_decoded_query_value`).
- B-M7 A pause that finds the box archived on its first read sends no stop and sleeps not
  (`pause_of_an_archived_box_sends_nothing_and_sleeps_not`).
- B-M8 Each pause pass is: sleep the poll, read, archived returns, any state but archiving is refused, then the
  budget. So archived on the read at exactly the budget is done (`pause_archived_at_the_budget_read_is_done`), and
  archiving there fails `MoveUnanswered` with `move_timed_out_line("pause", 100, Paused)` warned as `<id>: <words>`
  (`pause_archiving_at_the_budget_read_times_out`).
- B-M9 A pause whose first read is refused (404) fails with that error, `is_missing` true, and sends no stop
  (`pause_of_a_lost_box_fails_missing_and_sends_no_stop`).
- B-M10 `wait_ms` replaces the pause budget whole: 35 with poll 10 reads four times and fails naming 40 ms
  (`pause_wait_stands_in_for_the_budget`).
- B-M11 A snapshot's first read comes after one poll's sleep. A `failed` status with an empty or absent `error` ends
  `failed` with no colon (`snapshot_failed_with_empty_error_has_no_colon`).
- B-M12 Each snapshot pass checks ready, then failed, then the budget, so ready on the read at the budget answers the
  name (`snapshot_ready_at_the_budget_read_wins`, snapshot 2,500, poll 500).
- B-M13 The budget's words are `Math.round(snapshot_ms / 1000)`, half up: 2,500 reads `3 s`
  (`snapshot_budget_words_round_half_up`) and 100 reads `0 s` (`snapshot_budget_under_half_a_second_reads_0_s`).
- B-M14 kill: an answer with no `operation` leaves `deletion` `None` and forgets the timer
  (`kill_answered_without_an_operation_keeps_no_deletion_and_forgets_the_timer`); a 404 forgets it too
  (`kill_of_a_gone_box_forgets_the_timer`); any other refusal is answered and the timer is kept, `last_timer` still
  `{ until: 1,600,000, span_ms: 600,000 }` (`kill_refused_keeps_the_timer`).
- B-M15 exec reads `exitCode` first: 0 with `timedOut: true` is 0 (`exec_exit_code_wins_over_timed_out`); null with
  `timedOut` is 124 (`exec_null_exit_timed_out_reads_124`, 1,500 ms asks `timeoutSeconds: 2`); null without is -1 and
  absent streams read `""` (`exec_null_exit_reads_minus_1_and_absent_streams_read_empty`, 0 ms asks 1).
- B-M16 Exactly 30,000 stays inline (`exec_at_the_inline_cap_stays_inline`); only above it goes detached.
- B-M17 The loader-EMFILE test runs on stderr with `^` at any line start (JS `m`: after `\n`, `\r`, U+2028, U+2029),
  before the exit code is read, and fails `guest_unusable(id, "Boat", stderr.trim(), 200)` even on exit 0
  (`exec_loader_emfile_on_a_later_line_is_unusable`); a leading space is no match and the answer is plain
  (`exec_loader_emfile_not_at_a_line_start_is_a_plain_answer`).
- B-M18 The work-folder test is case-insensitive and comes first: a 409 `machine_not_running` whose message says
  `CWD must be an existing directory` is `GuestUnusable` with status 409, not `MachineStopped`
  (`exec_no_work_folder_wins_over_machine_not_running`).
- B-M19 No bytes to a path under `/tmp/` is one PUT with `"content":""` (`put_of_no_bytes_into_tmp_is_one_empty_put`).
- B-M20 The prefixes are `/tmp/` and `/home/user/` with their slash: `/tmp` itself goes by pieces and the join's
  folder is `/` (`put_to_the_bare_tmp_path_joins_into_root`). The folder is `path.slice(0, path.lastIndexOf("/")) ||
  "/"`, so a path with no slash loses its last character (`file.txt` makes `mkdir -p 'file.tx'`), as the TS does
  (`put_to_a_relative_path_cuts_the_folder_as_the_ts_does`). The join script is byte for byte the fixture's.
- B-M21 A failed join names `stderr.trim() || stdout.trim()` (`put_join_failure_names_stdout_when_stderr_is_blank`).
- B-M22 The size `stat` prints is read with JS `Number` of the trimmed text and must be an integer: a blank answer is
  0, so a file under `/tmp/` takes the straight road and answers whatever GET returns with no length check
  (`read_with_a_blank_size_reads_zero_and_takes_the_straight_road`); ` 3e0 ` is 3
  (`read_size_in_exponent_form_is_a_number`); `12.5` fails `could not be read (exit 0): 12.5`
  (`read_size_with_a_fraction_is_refused`).
- B-M23 The cut is one exec, `split -b 4194304 -d -a 4 '<path>' '/tmp/wsp-get-<hex>.' && chmod 0644 <every piece
  name>`, and the removal `rm -f <every piece name>` runs after a refused cut, after a short read, and after success;
  its own failure is ignored (`read_cut_refused_still_removes_every_piece`,
  `read_short_still_removes_and_names_both_sizes`). A GET answer with no `content` reads as no bytes.
- B-M24 describe: `memoryGB` 0.5 is `memMb` 512; an absent `vcpu`, a class the table does not know and a null
  `createdAt` each leave their key out
  (`describe_reads_half_a_gigabyte_and_leaves_out_what_is_missing`).
