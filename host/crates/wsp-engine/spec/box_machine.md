# box_machine (box-backend.ts:1046-1430)

The environment landing, the root wrapper, the daemon's exits, and `BoxMachine`. Rust:
`src/box_backend/machine.rs`. Other slices it calls: exec-detached's `exec_detached(machine, script, RunOptions)`,
`DEADLINE_EXIT` (124), `DAEMON_ENV_FILE`, `INLINE_EXEC_MS` (30,000); run-env's `DROP_SUDO_MARKS`; preview's
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

- `exec(cmd, opts)` (:1128): timeout `opts.timeout_ms` or 30,000. Above 30,000 (`BOX_INLINE_MAX_MS`) it goes
  `exec_detached` with `deadline_ms = timeout`, `poll_ms = budgets.poll_ms`; the detached road's own execs ask 30,000
  and stay inline. `idempotency_key` and `stdin` are ignored.
- `exec_inline(cmd, timeout_ms, signal)` (:1147): `while_restoring` of POST `/sandboxes/<id>/commands` with
  `{ command: sudo_command(cmd), timeoutSeconds: max(1, ceil(timeout / 1000)) }`, cap `timeoutSeconds * 1000 +
  30,000`. A failure whose message matches `/cwd must be an existing directory/i` is `guest_unusable(id, "Boat"
  provider key name, no_work_folder_line(name, "/home/user", message), status)`; kind unknown with status 500 is
  `guest_unusable(id, name, message, 500)`; code `machine_not_running` is `machine_stopped(id, e)`; else as itself.
  An answer whose stderr matches `/^(bash|sudo): error while loading shared libraries: .*: Error 24/m` is
  `guest_unusable(id, name, stderr.trim(), 200)`. Else `{ exitCode: exitCode, or 124 where timedOut, or -1; stdout
  or ""; stderr or "" }`.
- `prove_road(signal)` (:1174): `exec_inline("true", 30,000, signal)`; a still-restoring failure becomes
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
