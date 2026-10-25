# link_backend: one computer's machines driven from another, a frame at a time

Source: link-backend.ts at `a4033dc34`. Module `link_backend`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/link_backend.json` (every frame and option, byte for byte).

## The link

- `LinkAsk { timeout_ms, idempotency_key }`. `MachineLink` (:61): `request(op, params, ask)` answering a JSON
  object or an error; optional `dials_back()` (`has_dials_back()` says whether the link has one); `forward(place_port)`
  answering `{ local_port }`.
- `LINK_MARGIN_MS = 5_000`; `LINK_PROMPT_MS = 6_000`; `CREATE_MS = 605_000`; `UPLOAD_ATTEMPTS = 3` (:82, :87, :399).
- `PlaceAbsent` (:91): an error of kind `absent`; `is_place_absent(e)` is true of exactly that kind. A refusal the far
  side's backend gave keeps its kind and status, so `is_missing` reads a lost container as missing here.
- `ask(reply, op, params, opts)` (:273): one `request`; with a reply shape, the answer is parsed into it (a parse
  failure is an error); with none (the bare ok envelope) the answer is dropped.

## `LinkMachine` (:105)

- Built from a `MachineHandle`: id, kind, and stream URL, labels, seen, replayed, notice, daemon supervisor where the
  handle has them. `takes_stdin` is true.
- Offers exactly the roads the handle names: `preview_url`, `daemon_answers` (then `daemon_answers_prompt_ms` is
  6,000), `put_bytes`, `describe`, `facts`, `metrics`; and always `daemon_frame`. Never `framed`.
- Every machine frame carries `machineId` last, after the call's own params.
- `exec(cmd, opts)` (:152): `timeout = opts.timeout_ms or 20,000`; `machine.exec` with `{ cmd, timeoutMs, stdin? }`
  where stdin is base64 of the bytes (field absent without bytes, never inside `cmd`); the link waits `timeout +
  5,000`; the idempotency key rides only when the caller gave one. Answers the reply's `result`.
- `run(script, opts)`: `exec_detached(self, script, opts)` over this machine's exec; no `machine.run` frame exists.
- `snapshot(..)`: fails with `NO_IMAGES_HERE` and sends nothing.
- `pause()`, `kill()`: `machine.pause` / `machine.kill`, bare envelope.
- `resume(signal)` (:181): sends `machine.resume` once. With a signal, answers whichever comes first: the reply, or the
  signal's cancel (already cancelled fails at once, with the frame still sent once). The cancel fails with
  `errors::aborted()` ("This operation was aborted"); the TS rejects with the signal's own reason, which a
  `CancellationToken` cannot carry (an accepted difference).
- `state()`, `download_url(path)`, `upload_url(path)`: `machine.state` (reply `state`), `machine.downloadUrl` /
  `machine.uploadUrl` with `{ path }` (reply `url`).
- `preview_url(port)` (:214): `machine.previewUrl { port }`; the reply's `reach.url` port (WHATWG URL; an absent port
  is `""`) must be an integer >= 1, else fails with `{id} answered {url}, which names no port this host can be
  carried to`. Then `forward(port)`, and answers `{ url: "http://127.0.0.1:{local}", token: "", expiresAt:
  9007199254740991 }`.
- `daemon_frame(frame)` (:225): `op` taken out, the rest plus `machineId` sent as that op with no ask options. Success
  answers `{ ok: true, ...reply }`. A failure answers, never throws, `{ ok: false, error: message, code }` with `code`
  only where the error carried a string code.
- `daemon_answers(timeout)` (:235): `machine.daemonAnswers { timeoutMs }` (default 20,000), link waits `+ 5,000`;
  answers `answers`.
- `describe` / `facts` / `metrics`: `machine.describe` (reply `shape`), `machine.facts` (reply `facts`),
  `machine.metrics` (bare).
- `put_bytes(path, bytes, timeout)` (:247): `timeout` default 20,000. Parts = max(1, ceil(len /
  `MACHINE_PUT_PART_BYTES`)) (4 MiB; zero bytes is one empty part). Per attempt a fresh `uploadId` (16 hex from 8
  random bytes); parts in order as `machine.putBytes { path, uploadId, seq, last, data (base64), timeoutMs }`, link
  wait `timeout + 5,000`, no key on any part. A part that fails: when this was attempt 3, or the link has no
  `dials_back`, or `dials_back()` answers false, the upload fails with that part's own error; otherwise every part
  goes again from seq 0 under a new id. Answers `None` (the far side counts nothing back).

## `LinkBackend` (:289)

- `open(link)`: one `machine.backend` frame with no params and no options, parsed as `BackendFacts`. `of(link,
  facts)` sends nothing.
- From the facts: capabilities as sent; `logins`, `projects` where sent; `names_workspace` true; base templates where
  sent; lifecycle only where the facts carry one, with its budgets and a `quiet_for_ms` (has_quiet_for_ms true).
- Pricing: `rate_usd_per_hour(size)` is the rate of the offer in `capabilities.sizes` with that exact cpu and memMb,
  else 0; `default_size`, `snapshot_storage` (`free_named`, `usd_per_named_month` none), `builder_disk_gb` where sent.
- Offers `on_computer`, `check_key`, `capacity`; never snapshots, templates, keyed create, limits. `list_snapshots`
  and the template calls are not offered at all.
- `quiet_for_ms(machine)` (:344): `machine.metrics { machineId }`, reply `reading`; answers `reading.quietForMs`,
  `None` where the reading has none (never 0).
- `create(spec)` (:355): the spec as given (the TS keeps the caller's key order; the Rust struct writes the schema's,
  an open point), with `shares: shares_in(logins)` added as its last key only where the
  backend has logins and the spec's labels carry no `BUILDER_LABEL`. Frame `machine.create { spec }`, link wait
  605,000, the spec's idempotency key as the ask's key where it has one. Answers a `LinkMachine` of the reply.
- `get(id)`: `machine.get { machineId }`. `list(labels)`: `machine.list` with `{ labels }` or `{}`; answers
  `machines`.
- `delete_snapshot(..)`: fails with `NO_IMAGES_HERE`, sends nothing. `check_key()`: `machine.checkKey`, answers
  JSON null. `capacity()`: `machine.capacity`, parsed as `PlaceCapacity`.
- `on_computer(cmd, timeout)` (:391): frame `exec { cmd, timeoutMs }` naming no machine, timeout default 20,000, link
  wait `+ 5,000`, parsed as `DaemonExecReply`; answers its exit code, stdout and stderr.

Nothing is cached past the facts; nothing survives a restart.

## Pins

| Rule | Tests |
|---|---|
| open asks once; names workspace | `asks_once_and_answers_with_what_the_far_side_said` |
| no image calls, NO_IMAGES_HERE sends nothing | `carries_no_snapshot_and_no_template_call_at_all` |
| pricing off offers | `prices_a_size_off_the_offers_the_far_side_sent` |
| spec whole, roads named | `sends_the_spec_whole_and_carries_exactly_the_roads_the_handle_named`, `leaves_out_a_road_the_handle_did_not_name` |
| notice | `carries_the_sentence_the_computer_answered_the_create_with` |
| shares, builder, no logins | `fills_a_create_with_the_logins_that_computer_signs_in_once`, `shares_nothing_where_the_computer_names_no_logins_directory` |
| exec bound, stdin, keys | `execs_with_the_callers_bound`, `sends_an_execs_input_as_base64_beside_the_command`, `names_an_exec_the_caller_says_lands_the_same_twice` |
| run over exec frames | `runs_a_script_over_exec_frames_and_sends_no_run_frame` |
| daemon frame and refusal reply | `answers_one_daemon_frame_for_this_machine` |
| missing and absent pass through | `reads_a_refusal_the_far_sides_own_backend_gave`, `passes_a_place_that_is_not_connected_through` |
| parts, order, retry under a new id, failures | `sends_a_file_in_parts_under_one_upload`, `carries_no_key_on_a_part_and_sends_the_whole_upload_again`, `fails_an_upload_with_the_parts_own_answer`, `sends_one_part_for_one_byte`, `gives_up_after_three_attempts_on_a_link_that_keeps_flapping` |
| preview route, bad port | `turns_the_far_sides_own_loopback_route_into_one_on_this_computer`, `a_route_with_no_port_is_refused` |
| daemon answers | `asks_the_machine_itself_whether_its_daemon_answers` |
| resume cancel | `stops_waiting_on_a_resume_the_caller_gave_up_on` |
| quiet figure | `reads_how_long_the_machine_has_been_quiet` |
| capacity, on_computer | `reads_the_capacity_of_the_computer_on_the_far_side`, `frames_match_the_ts_byte_for_byte` |

## Pins (wave 2)

Scenes in `oracle/roads_scenes.oracle.ts` (`links`), replayed by `tests/link_backend_scenes.rs::<name>`: every frame
after the machine was got, its params and ask byte for byte, then the answer.

- L-1 pause, kill, state, `downloadUrl { path }`, `uploadUrl { path }`, describe and metrics each send their op with
  `machineId` last and no ask options; the backend's `get` sends `{ machineId }`, `list(None)` sends `{}` and
  `list(Some({}))` sends `{"labels":{}}` (`frames_of_the_roads_no_other_test_sends`).
- L-2 An empty stdin is bytes: `"stdin":""` goes between `timeoutMs` and `machineId`
  (`an_empty_stdin_is_sent_empty`).
- L-3 daemon_frame: `{ ...params, machineId }` overwrites a frame's own `machineId` in its place and appends it
  otherwise; `{ ok: true, ...reply }` lets a reply's own `ok` override the value, the key staying first; a refusal is
  `{ ok: false, error: message }` (an empty message stays empty) plus `code` only where it is a string
  (`a_daemon_frame_keeps_its_own_machine_id_place_and_its_replys_ok`).
- L-4 The place's port is WHATWG `URL.port` read with `Number`: the scheme's own port (`http://h:80`) reads `""`, so
  0, so `{id} answered {url}, which names no port this host can be carried to`; `foo://h:80` keeps 80; `[::1]:8080`
  is 8080; a URL the reader refuses (`http://h:99999`) fails `Invalid URL` and nothing is forwarded
  (`a_route_on_the_schemes_own_port_names_no_port`, `a_route_on_a_scheme_with_no_default_keeps_its_port`,
  `a_route_on_an_ipv6_loopback_reads_its_port`, `a_route_the_url_reader_refuses_is_invalid_url`).
- L-5 A failing upload asks `dials_back` only below its third attempt, so twice, sends each attempt from seq 0 under
  a new id with no wait, and fails with the last part's own error
  (`a_failing_upload_asks_whether_the_link_dials_back_twice_and_sends_three_times`).
- L-6 `create(spec, waiting, sent)` ignores `waiting` and `sent`, and `pause(wait_ms)` ignores the wait: the TS takes
  neither (link-backend.ts:357, :174).
