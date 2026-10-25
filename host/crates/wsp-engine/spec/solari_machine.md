# solari_machine: a Solari machine's exec, moves and reads

Source: `packages/engine/src/solari-backend.ts:382-538` at `a4033dc34`. Rust: `SolariMachine` in
`src/solari_backend.rs`. Tests: `tests/solari_machine.rs`, hard tests in `tests/solari_hard.rs`. Every call goes
through `SolariBackend::request` (spec/solari_call.md R1 to R7); paths are `/sandboxes/<encodeURIComponent id><suffix>`.

## exec (solari-backend.ts:397)

- X1. `timeout_ms` defaults to exec-detached's `INLINE_EXEC_MS` 20000. Above `SOLARI_INLINE_MAX_MS` 26000 the command
  runs by exec-detached's `exec_detached(self, cmd, deadline = timeout_ms)` and nothing else is sent here; every call that
  road makes asks at most 26000. At or under it: one `POST /exec` with body `{cmd: "bash", args: ["-c", "<EXEC_ENV>\n<cmd>"],
  timeoutMs}` (keys in that order; `EXEC_ENV` is `export HOME=/root USER=root`; no SHELL, never `-lc`). The answer is
  the reply `{exitCode, stdout, stderr}`. `idempotency_key` and `stdin` are not read.
- X2. A refusal whose message is exactly `Sandbox is not reachable`, whatever its status, becomes
  `MachineUnreachable { machine_id, said: that message, status, line: machine_unreachable_line(said), exec_failed: false }`;
  a 502 whose message is exactly `exec failed` becomes `MachineUnreachable { .., status 502, line: exec_failed_line(said),
  exec_failed: true }` (the TS `ExecFailedError`). Both only after the call's own transient retries are spent (three
  sends for a 502). Any other refusal passes as it came (a 502 with other words, `exec failed` on a 400).
- X3. `run(script, opts)` is `exec_detached(self, script, opts)`.

## snapshot (solari-backend.ts:423)

- N1. `life.first_life` false: `NotFirstLife { machine_id, action: "snapshot <name>" }` before any call. Else one
  `POST /snapshots` with `{name}`, answering `snapshotId`. A 502 `Failed to snapshot sandbox` is snapshotUnavailable and
  is not retried.

## pause (solari-backend.ts:434)

- P1. `deadline = started + pause_ms` on the backend's clock (`clock.now()`, read fresh at each step). Up to two
  attempts. Before each, `left = deadline - now()`; where `left > 0`, `POST /pause` with body `{}` and a cap of `left / 2`
  on the first attempt and `left` on the second (the cap goes through `fetch_cap_ms`: rounded up, so 19.5 is 20). A 2xx
  is done.
- P2. A refusal that is a 409 whose message is exactly `Not pausable` is `NapRefused { machine_id, said, line:
  nap_refused_line(said) }`, no read. Any refusal that is neither capped (`TimeoutError`) nor a network error (message
  `fetch failed`, or a network code on it or its cause) is thrown as it came, no read.
- P3. A capped or network failure is followed by one read of the machine under `state_read_ms`. `paused`: done. `running`
  on the first attempt with the deadline still ahead: the second attempt. Anything else (a second miss, a spent budget,
  another state, no read): warn `<id>: <words>` and fail `MoveUnanswered { message: words, cause: the last capped or
  network failure (None where no call was sent) }` with `words = move_timed_out_line(Pause, now - started, reads)`.
- P4. A read that fails is `gone` where the failure is missing (404), else no read (`None`), never retried; a read that
  hangs is cut at `state_read_ms`. `wait_ms` is not read.

## resume (solari-backend.ts:467)

- W1. One `POST /resume` with `{}`, cap `resume_cap_ms`, the caller's signal. A 2xx is done.
- W2. A failure that is neither capped nor a network error is thrown as it came: the caller's own stop is an
  `AbortError`, not capped, so it is thrown as itself and no read follows.
- W3. Capped or network: one read under `state_read_ms` (as P4). `running` or `starting` is done. Else warn
  `<id>: <RESUME_UNANSWERED>, which still reads <state or "nothing">` and fail `ResumeUnanswered { message:
  RESUME_UNANSWERED, cause: the failure }`. Nothing is sent twice.

## The other roads

- M1. `kill`: `DELETE` the machine. `state`: `GET` it, `state_of(view.state)`; missing is `gone`, any other failure
  thrown. `describe`: `GET`, cpu, memMb, diskGb, createdAt each where present. `metrics`: `GET /metrics`, the answer
  dropped; a 404 there is the missing refusal.
- M2. `preview_url(port)`: `GET /ports/<port>` answering `{url, token}`, `expires_at = preview_token_expiry(token,
  wall clock now)`.
- M3. `download_url(path)` / `upload_url(path)`: `GET /files/download-url?path=<encodeURIComponent path>` (upload the
  same), answering `url`. The query keeps `/` encoded as `%2F` (encodeURIComponent, not URLSearchParams).
- M4. `offers()`: preview_url, describe, metrics true; the rest false. Handle getters answer what the handle was made
  with (spec/solari_call.md K4, G1).

## Caps, timing, restart

- Pause budget 240000 ms, two sends at most, half then the rest; resume cap 30000, one send; state read 30000. Tests
  shrink them (40, 20, 20) and check elapsed time: a pause that never answers takes at least `pause_ms - 5`; a hung read
  ends inside `pause_ms + state_read_ms + 500`.
- Nothing survives a restart; a handle holds no state that changes.

## After-port notes

- A pause with an odd budget split across attempts relies on `fetch_cap_ms` rounding up (P1); kept.
- `download_url` and `upload_url` take no cap beyond the GET default.

## Rules and their tests

Files: `solari_machine.rs` (machine), `solari_call.rs` (call), `solari_hard.rs` (hard).

| Rule | Tests |
|---|---|
| X1 | machine `every_exec_runs_under_bash_c_with_home_and_user_exported_and_no_shell`, `an_exec_asked_for_longer_than_the_providers_cap_runs_detached_under_the_cap`, `an_exec_at_the_cap_is_one_call_with_the_timeout_it_asked_for`; hard `exec_body_keys_and_default_timeout` |
| X2 | machine `a_502_with_the_providers_words_is_the_typed_refusal_once_the_gateway_retries_are_spent`, `the_same_words_on_a_400_are_the_same_refusal_carrying_the_400`, `a_502_with_any_other_words_stays_the_plain_error_it_was`, `a_502_exec_failed_is_the_same_refusal_with_its_own_sentence_once_the_gateway_retries_are_spent`, `exec_failed_on_any_other_status_stays_the_plain_error_it_was` |
| N1 | call `a_snapshot_of_a_machine_not_in_its_first_life_is_refused_before_any_call`, `surfaces_snapshot_unavailable_without_retrying` |
| P1, P3 | machine `a_pause_the_provider_never_finishes_is_sent_twice_and_ends_unanswered_with_the_rows_words`, `a_pause_whose_call_never_answers_but_landed_is_a_pause_one_call_one_read`, `a_pause_that_fails_on_a_network_error_is_read_and_sent_once_more_then_ends_with_the_words`, `a_pause_that_fails_once_on_a_network_error_and_lands_on_the_second_call_is_a_pause` |
| P2 | machine `a_409_whose_words_are_not_pausable_is_the_typed_refusal_asked_once`, `a_409_with_any_other_words_stays_the_plain_conflict_it_was`, `a_pause_the_provider_refuses_is_not_retried_and_an_odd_millisecond_is_no_reason_to_fail` |
| P4 | machine `a_provider_that_cannot_be_read_after_the_call_ends_the_move_at_once_with_that_in_the_words`, `a_read_that_hangs_is_given_up_on_within_its_own_bound` |
| W1 to W3 | machine `a_resume_whose_call_never_answers_but_landed_goes_through_one_call_one_read`, `a_resume_cut_off_at_its_cap_with_the_machine_still_paused_ends_unanswered_nothing_sent_twice`, `a_resume_the_network_dropped_is_read_like_a_capped_one_and_starting_is_a_resume_that_landed`, `the_callers_own_signal_ends_the_call_so_a_resume_nobody_waits_on_is_not_left_running`, `a_provider_that_answers_nothing_inside_the_cap_ends_the_resume_there` |
| caps into fetch | machine `a_resume_carries_its_cap_a_pause_half_its_budget_and_a_read_the_reads_cap_into_the_fetch`, `the_cap_a_fetch_is_given_is_a_whole_number_of_milliseconds_inside_the_timers_range` |
| M1 | call `reads_the_metrics_route_for_the_hosts_answer_and_a_404_there_is_the_machine_missing`, `reads_the_providers_size_and_creation_time_off_the_machine_route` |
| M2 | call `mints_via_the_ports_route_and_derives_expires_at_from_the_token` |
| M3 | hard `download_and_upload_urls_encode_the_path_as_encode_uri_component` |
