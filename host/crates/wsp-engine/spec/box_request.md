# box_request (box-backend.ts:347-786)

The wire, how a refusal is read, one request, the limits the backend keeps, and the two waits on a box. Rust:
`src/box_backend/request.rs`.

## State, and what survives a restart

`BoxState` (:572-581): the last answered `/limits` read and its time, the read out now, when each refusal was last
logged, the stop timer last sent per box, the boxes a backstop is being sent for, and per box the last failed
backstop. **All memory only.** A restarted host starts with none: every class sold until the first read, no timer
on record for a resume (so a resume arms the account's cap), no refusal logged. The backend is a cheap clone over one
`BoxShared`; every clone is the same state.

## Wire views (:349-405)

Read with serde, every field but `id`, `state` (box) and `name`, `status`, `createdAt` (named snapshot) optional; a
JSON `null` reads as absent, except a snapshot's `chainId`, where a present null is kept (`Some(None)`) and passed on as
`parent: null`. Unknown fields are ignored. `stop_refusal_of` (:409): the box's non-empty `error`; else `its latest
snapshot <status>[ at <attempt>]` or `it reported no snapshot attempt`, then `, and the last that completed was at
<completed>` or `... was none`. `template_row_of` (:415): `{ id: name, name, status, error?, createdAt }`, status ready,
failed, or building for any other word; `error` only on a failed one that has one.

## Reading a refusal

- `box_refusal(e)` (:459), in this order: code `IP_RATE_LIMIT_EXCEEDED` is transient with wsp-words'
  `BOAT_REQUEST_LIMIT_LINE` as message and Boat's words in `said`; codes `rate_limited` and `daily_limit_reached` are
  startLimit, the daily one with `window: day`; codes `limit_reached` and `member_limit_reached`, or a message
  matching `/concurrent (box|sandbox)/i`, are concurrency; any other 429 keeps classify's reading and puts its own
  message in `said`; codes `billing_required`, `team_member_cap_reached`, `out_of_capacity` take wsp's line with
  Boat's words in `said`; anything else stands as classify read it.
- `request_limit_wait_ms(body)` (:446): `body.retryAfter ?? body.error.details.retryAfter` (only a null or absent top value falls through, so a top `"7"` or 0 waits the minute even with a number below); a positive number
  of seconds times 1000, at most 60,000; anything else 60,000.
- `still_restoring(e)` (:472): never the request limit; any transient; a conflict with code `sandbox_restoring` or
  `sandbox_starting`. snapshotUnavailable is not.
- `road_words(e)` (:480): `<status> <message>` where the error has a status; else `<message>: <cause code>` where the
  cause has a code; else the message.
- `fail(w)` (:487): `EngineError::Provider(w)` with its message `w.message`, kept even when empty: the TS's
  `<kind> (<status>)` fallback is dead, since `Object.assign` copies the refusal's own `message: ""` over it.

## request(method, path, opts) (:603)

1. Headers in this order: `Authorization: Bearer <key>`, then `opts.headers` in order, then `Content-Type:
   application/json` where a body goes. The body is `opts.bytes` as given, else `JSON.stringify(opts.body)`.
2. Each send gets `abort(cap, signal)`: the cap is `opts.cap_ms`, else `budgets.read_ms` for a GET, else none (a
   write waits as long as its road does).
3. A fetch that rejects: a `road_code` failure is sent again after `road_backoff_ms(n)` and a warn line
   `provider_road_retry_line("<METHOD> <path>", code, n + 1, 3)`, three sends in all (one with `once`); any other
   rejection (a cut connection, a cap, a cancel) is the caller's at once.
4. A 2xx answers the JSON of the body, `{}` for an empty body.
5. Otherwise the body is read as JSON where it parses (an envelope `{ code, message, requestId }` or
   `{ error: { code, message, details } }`); `code` from the top or the envelope's error; Boat's words from `message`,
   the error's message, else the body's text. The refusal is logged (below), read by `classify` then `box_refusal`.
6. The request limit: with `once`, or on its third meeting, it fails; otherwise warn
   `boat_request_limit_wait_line("<METHOD> <path>", wait, n + 1, 3)`, sleep the wait, answer `aborted()` if the signal
   fired meanwhile, and send again. Its count is its own, apart from the gateway's.
7. Any other refusal: with `once` or where `should_retry(e, attempt)` is false it fails; else sleep
   `backoff_ms(attempt)` and send again (three sends at most for a transient).
- `log_refusal` (:653): key `"<call> <status> <code or empty>"`; a key logged under 60,000 ms ago is skipped; before
  a write every key at or past 60,000 is pruned; the line is wsp-words `boat_refused_log_line(call, status, code,
  request_id, said)`.

## Limits kept (:664-:749)

- `limits()`: the kept read while under 10,000 ms old, else `read_limits()`. `read_limits()`: one read out at a time,
  shared by every caller (a `Shared` future); a success is kept with its time; a failure is not kept and the next call
  reads again. `place_limits()`: `place_limits_of` of `limits()`, `None` on any failure.
- `before_start()`: `limits()`, and on failure the kept read where there is one, else the failure.
- `counted()`: one more `used` in each window of the kept read; nothing without a kept read or without starts.
- `start_refused(e, what)` (:709): not startLimit: `e`. A fresh `read_limits()` (failure: `e`); no starts: `e`. With
  no window on `e` and neither the hour nor the minute spent (`used >= limit`), `e`. Else the window is `e`'s, else
  hour if spent, else minute: a Provider error with message `starts_used_line(window, used, limit,
  boat_plan_words(trial, plan_usd), what)`, `e`'s kind, status, code and request id, that window, and `said` = `e`'s
  message.
- `ttl_cap()`: `Some(7200)` on the trial, else `None`, off the kept read or a fresh one. `ttl_seconds_for(instant)`:
  no instant is the cap; else `max(1, ceil((instant - now) / 1000))`, at most the cap.
- `check_key()`: `read_limits()`, never the kept one. `view(id)`: `GET /sandboxes/<id URI-encoded>`, its `sandbox`.

## The waits

- `settle(id, done, budget, what, signal, name, seen)` (:757): loop: a fired signal answers its reason, `<what> of <id> was stopped by its caller` for a reason that is
  not an error, `aborted()` with none (README.md's cancel rule); read the
  view; `seen(view)`; `done` answers it; state `error` fails `box <id> reads error during <what>[: <error>]`, the suffix only for a non-empty error; state
  `cancelled` is `CreateCancelled { why: error or "", line: boat_cancelled_line(name, error) }` on the first such read;
  at or past the budget fails `<what> of <id> still reads <state> after <round(budget / 1000)> s`; sleep the poll.
- `while_restoring(call, signal)` (:774): a fired signal answers its reason, `the call was stopped by its caller` for one that is not an error,
  `aborted()` with none; the call's answer, or its failure where
  not `still_restoring`; a restoring failure at or past `restore_ms` from the start is answered as itself; else sleep
  the poll and call again.

## Rules against tests

| Rule | Tests |
|---|---|
| refusal reading per code, request limit wait, restoring, road words, fail, template rows, stop refusal | `box_request_oracle::*` (8) |
| headers order, bytes, empty answer, text refusal | `box_request_hard::a_request_sends_authorization_first_then_its_own_headers_then_the_content_type`, `box_request_hard::bytes_win_over_a_body_and_a_call_with_neither_sends_no_content_type`, `box_request_hard::an_empty_answer_reads_as_an_empty_object`, `box_request_hard::a_refusal_that_is_not_json_says_its_text_and_reads_its_status_alone` |
| gateway retries, once, road retries | `box_request_hard::a_gateway_answer_is_sent_three_times_on_the_engines_backoff_and_once_where_asked_once`, `box_request_hard::a_road_that_never_left_is_tried_three_times_in_all_and_any_other_road_failure_is_the_callers_at_once`, `box_machine_exec::a_box_answering_502_to_its_first_commands_*` |
| request limit | `box_request::an_address_past_boats_request_limit_waits_the_seconds_boat_names_and_asks_again`, `box_request::the_request_limit_is_asked_three_times_at_most_then_fails_in_its_own_line_not_as_the_cap` |
| refusal codes through a create | `box_request::limit_reached_and_member_limit_reached_are_the_machine_cap`, `box_request::a_429_with_a_code_boat_never_listed_keeps_todays_reading_and_boats_own_words`, `box_request::a_402_and_a_503_out_of_capacity_print_boats_lines_503_only_once_the_creates_asks_run_out`, `box_fleet_create::a_refusal_is_the_providers_own_envelope_*`, `box_fleet_create::the_concurrent_box_cap_is_the_concurrency_kind_*` |
| refusal log, quiet minute per key | `box_request::every_refused_call_goes_in_the_host_log_with_boats_code_and_request_id`, `box_request::two_boxes_refused_in_turn_each_keep_their_own_quiet_minute_in_the_log`, `box_request_hard::the_same_refusal_of_the_same_call_is_logged_once_a_minute_and_a_new_code_logs_at_once` |
| limits kept, shared read, failure not kept, counted | `box_request::keeps_a_read_10_s_reads_before_every_create_and_resume_past_that_and_counts_each_start`, `box_request::reads_the_backends_own_kept_read_a_start_counted_on_it_and_nothing_once_a_read_fails`, `box_request::asks_box_for_the_accounts_limits_again_once_a_read_of_them_ended_at_its_cap`, `box_request_hard::callers_of_limits_while_a_read_is_out_share_it_and_a_failed_read_is_not_kept`, `box_request_hard::a_counted_start_adds_one_to_every_window_on_the_kept_read_and_nothing_without_one`, `box_request_hard::a_failed_read_before_a_start_goes_on_with_the_kept_one_and_fails_with_none_kept` |
| start windows | `box_request::daily_limit_reached_is_the_day_*`, `box_request::rate_limited_is_the_hour_or_the_minute_*`, `box_request::a_start_with_no_name_*`, `box_request::rate_limited_keeps_boats_words_*`, `box_request::a_resume_that_meets_a_start_limit_says_so_too` |
| trial and plan reads | `box_request::reads_the_owners_answer_of_17_06_utc_*`, `box_request::a_trial_offers_small_and_default_*`, `box_request::a_trial_is_read_off_the_plan_key_too_*`, `box_request::the_first_create_after_a_host_start_checks_the_size_*` |
| read caps | `box_request::reads_at_most_the_measured_default_by_default`, `box_request::ends_each_of_boxs_reads_of_a_machine_the_fleet_and_the_snapshots_at_the_reads_cap`, `box_request::leaves_a_write_uncapped_where_its_own_road_gives_it_none` |
| ttl seconds | `box_request_hard::the_seconds_to_a_stop_are_rounded_up_at_least_one_and_capped_on_the_trial` |
| settle and while_restoring | `box_request_hard::settle_reads_until_done_then_fails_naming_the_state_after_its_budget_in_whole_seconds`, `box_request::cancelled_reads_as_gone_and_ends_a_creates_wait_on_the_first_read_with_boats_reason`, `box_machine_exec::a_box_that_reads_ready_but_refuses_commands_while_its_disk_streams_in_*` |

## Pins (wave 2)

- B-R1 `fail(w)` keeps an empty message empty (`box_request_oracle::fail_keeps_every_field_and_an_empty_message_stays_empty`).
- B-R2 `request_limit_wait_ms` falls through to the details only for a null or absent top value
  (`box_request_oracle::the_request_limit_waits_the_seconds_boat_names_never_past_its_minute`).
- B-R3 `stop_refusal_of` skips only an empty `error`: an error of white space alone is the words, and an empty status,
  attempt or completion is written as it is, so `its latest snapshot  at , and the last that completed was none`
  (`box_request_pins::stop_refusal_writes_empty_strings_as_they_are`, from `units.json`).
- B-R4 `ttl_cap` and the capabilities read the kept limits of any age, never asking again because 10 s passed; only
  `limits()`, `before_start` and `place_limits` hold the 10 s window
  (`box_fleet_scenes::backstop_after_an_hour_reads_the_kept_limits_of_any_age`).
