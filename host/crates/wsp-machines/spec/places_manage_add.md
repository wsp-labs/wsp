# places/manage, unit add: add, dial, road, exec, adds, reportOf, homeOf

Port of `packages/runtime/src/places/manage.ts:197-423` at `65f6b89c7`, into `src/places/manage/add.rs`. Tests:
`tests/places_manage_add.rs`. `file:line` is `manage.ts` unless named.

`ManageDoor::new(ctx, records, setup: Rc<dyn ManageSetup>, views, links: Rc<dyn LinkReads>)` (manage.rs). `ManageSetup`
is what manage reads off places-a's setup area (manage.ts:113-116). Where the TS calls `ctx.door.exec`, the port calls
its own `exec` (the door's `exec` is this one).

## add (:198-342)

- **A1** No `install` road: fail `NO_PLACE_INSTALLER`, no kind, nothing written.
- **A2** `add_id` = `req.addId`, else `a_` and 12 random hex (6 bytes, the `Ids` seam, spec 03 rule 3.16).
- **A3** A job under that id still `running`: usage refusal `ADD_RUNNING_LINE`, fix `ADD_RUNNING_FIX`.
- **A4** One add per address. Among the pending adds with `address == req.address`, one with no `failed` refuses as
  usage: `pending_held_line(address, its step)`, fix `pending_held_fix(its name, else address)`. Otherwise every one of
  them (all failed) is dropped (V3, a `place.pending` frame each) before anything else.
- **A5** The pending add is written at once: `{id: add_id, address, sshPort?, name?, keyPath?, hostKey?, step:
  "connect", choices: req.choices else RecipeFile {name: req.name else address} with every section empty, recipe?,
  startedAt: ISO(at)}`. Later moves go through one chain (`move_pending`) that writes in call order and swallows a
  failed write; the add awaits the chain before the install and again before it answers.
- **A6** `stage(which, state, note, place_id, ms)`: `running` makes `which` the current step; `failed` notes it as said;
  `running` on `check` moves the pending add to `step: "check"`. Each stage is a `place.stage` event `{type, addId, step,
  state, note?, placeId?, ms?}` folded into the job by `with_place_stage` and handed to `opts.onStage`.
- **A7** A code is issued: `devices.issue(now: at, ttl: PAIR_CODE_TTL_MS)`. Then the job is put afresh at the end of
  `adds` (`{addId, address, sshPort?, startedAt, state: running, steps: []}`), and `awaiting[code]` is an empty entry.
- **A8** The install is asked with the request less `addId`, `choices` and `recipe`, plus `code: join_token(code,
  key_fingerprint(host public key))`, `held`: every record's id (R1 order), and `beforeDeploy(undo, ssh, hostKey)` moving
  the pending add to `{step: "wsp", undo, login: ssh, hostKey?}`.
- **A9** The install answers: `awaiting.login` = `{ssh, keyPath: sshKeyPath?, hostKey?}` where it named `ssh`, `back`
  where it named one. Where the join already landed (`awaiting.place_id` set), the record is written again through
  `change(id, unchanged)` so it carries that road (R24).
- **A10** `stage(join, running)`. Where the join's place id is set and linked, it is the place at once. Else wait for
  `awaiting.woken(id)` (places-a's attach calls it) up to `opts.joinWaitMs` else `JOIN_WAIT_MS` (90 s); the wait running
  out fails with `place_no_link_line(installed.name)`, and the failure's message gains, one per line after it, the lines
  `wiring.log(login)` reads where the wiring has a log reader and the install named `ssh` (`box_said`; a failed read adds
  nothing).
- **A11** The record must be there, else fail `place_no_link_line(name)`. `hold_back(record)` (R23). `stage(join, done,
  note, place_id, ms)` with `note` = `linked_over(report.dialed, road.back, req.hostUrls)` where it says anything, and
  `ms` the join wait's length on the clock. The job reads `done` with its `placeId`.
- **A12** `facts_on(place_id, record)` (V11), then the pending chain is awaited.
- **A13** Nothing picked (`req.choices` absent): the pending add moves to `{...pending, placeId, step: "floor"}` and
  `floor_once` (V4) runs behind the answer. Answer `{addId, place: view_of(facts, default_id()), hostKey?, pending:
  pending_view(that)}`; no `said`.
- **A14** Picked: the pending add is dropped, `started_or_said(place_id, add_id, {picks, recipe?})` starts the setup
  before the answer, and the answer is `{addId, place: view_of(record now else facts, default_id()), hostKey?, said?}`
  with `said` where the setup did not start. No `pending`.
- **A15** Any failure from A8 on, in order: `{said, fix, kind}` = `refusal_parts(e)`; the job gains `said:
  kept_said(said)`, `fix`, `kind`, and `hostKey` where the kind is `PLACE_HOST_KEY_KIND` and the error carries one; where
  the pending add has no `placeId` yet it gains `failed: {said: kept_said(said), fix?}`; where the installer did not
  already fail the current step, `stage(step, failed, marked_cut(first line of the message))`; the code is spent (its
  failure ignored); where the install itself failed after its join landed and the error is a taken-back one
  (`is_place_add_taken_back`), the record that join made is forgotten (V18); where the install named a login and a
  back, the back is released unless the landed record dials back through it (then `hold_back`). The original error is
  rethrown. `awaiting[code]` goes on every exit, success included.

## dial (:344-382)

- **D1** No record: fail `no_such_place_refusal(id, names of ctx.kept in insertion order)`, no kind.
- **D2** The road is `place_dial_road({present: linked, road})`: `link` where linked, `ssh` where the record's road has
  an `ssh`, else none. Link: `reach.request("ping")` bounded by `dial_wait_ms` (what `ping on <name>`). Ssh: only where
  the wiring has a `dial` road, `dial(login_of(record))` bounded (what `ssh <login.ssh>`). Either answers `{at, answered:
  true, roundTripMs: max(0, round(clock now - started))}` or `{at, answered: false, said: the message}`. No road, or the
  ssh road with no `dial`: `{at, answered: false, said: place_no_dial_line(name)}`. Nothing is installed.
- **D3** `lastSeenAt` moves to `at` only where the link answered; an ssh answer does not move it.
- **D4** Written through `change`; where the record went meanwhile, the answer is built from the record as read.
  Answer `{dialled, line: place_dial_line({name, road, linked, dialled}), place: with_cap(view_of(written,
  default_id()), row_ids())}`.

## road (:384-398)

- **RD1** The name is the record's, else the id. Not linked: fail `absent_computer(name, null).sentence`. No
  `report.daemonPort`: fail `place_no_daemon_port_line(name)`.
- **RD2** One forward per link: the first ask opens `open_place_forward(reach, port)` and stores the pending open on the
  live entry; every later ask on that link joins it (spec 03, join). A failed open is taken off the entry (only if the
  entry is still the same link) and the failure passes on, so the next ask opens again. Answers the local port.

## exec (:400-418)

- **E1** Not linked: fail with the engine's `PlaceAbsent` error, `absent_computer(name, null).sentence`.
- **E2** `reach.request("exec", {cmd, timeoutMs?, stdin?})` with `stdin` as base64. A failure where the same link is
  still live and its socket open is the computer's own answer and passes on; any other failure is `PlaceAbsent`.
- **E3** Answer `{exitCode: answer.exitCode else -1, stdout: answer.stdout else "", stderr: answer.stderr else ""}`.

## adds, reportOf, homeOf (:420-423)

- **AD1** `adds()`: every job in insertion order: running ones and the last 20 finished (R16).
- **AD2** `report_of(id)`, `home_of(id)`: the record's report and its `login.HOME`; `None` without a record.

## What survives a restart

The pending add (store), with `step`, `undo`, `login` and `failed`; the record a join wrote. The job list, the code and
`awaiting` are memory: after a restart `adds()` is empty.

## Tests

| Rule | Test |
|---|---|
| A1 | `add_without_installer_refuses` |
| A2, A3 | `add_running_id_refused_as_usage`, `add_id_shape` |
| A4 | `add_second_on_same_address_refused_while_standing`, `add_takes_over_failed_pending_on_same_address` |
| A5, A8 | `add_writes_pending_before_install_and_hands_token`, `add_before_deploy_writes_undo_and_login` |
| A6 | `add_stage_check_moves_pending`, `add_stages_reach_on_stage_and_job` |
| A9, A11 | `add_join_landed_before_install_answer_carries_road`, `add_join_done_note_and_job_done` |
| A10 | `add_join_wait_runs_out_with_box_lines` |
| A13 | `add_without_choices_pending_at_floor` |
| A14 | `add_with_choices_drops_pending_starts_setup` |
| A15 | `add_failure_spends_code_marks_pending_and_job`, `add_failure_host_key_rides_job`, `add_taken_back_forgets_record`, `add_failure_releases_back_without_record` |
| D1-D4 | `dial_unknown_refuses_with_names`, `dial_link_ping_moves_last_seen`, `dial_ssh_answer_keeps_last_seen`, `dial_no_road_line`, `dial_link_bound_is_said` |
| RD1, RD2 | `road_not_linked_absent`, `road_no_daemon_port`, `road_one_forward_per_link_reopens_after_failure` |
| E1-E3 | `exec_not_linked_place_absent`, `exec_stdin_base64_and_defaults`, `exec_failure_on_open_socket_passes_on`, `exec_failure_after_link_went_is_absent` |
| AD1, AD2 | `adds_in_order`, `report_and_home_of` |
| restart | `restart_keeps_failed_pending_drops_jobs` |
