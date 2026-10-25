# errors (errors.ts, 341 lines)

What every road fails with, and the readings callers make of a failure. Rust: `src/errors.rs`, `src/fetch.rs`.

## Shapes

- `ErrorKind` (errors.ts:5): nine words, serialized camelCase (`startLimit`, `snapshotUnavailable`).
- `WspError` (:11): `kind, status, code?, message, requestId?, window?, said?`, serialized in that order, absent fields
  left out. An **empty** request id is kept (`Some("")`); only an absent one is absent (oracle `classify` last case).
- Each TS class is an `EngineError` variant (see the enum's docs). The constructors make the line the TS makes:
  `guest_unusable` (:121) uses wsp-words `guest_unusable_line(provider, machine_id, detail)`; `exec_failed` (:157)
  `exec_failed_line(said)` and `exec_failed: true`; `machine_unreachable` keeps the line it is handed; `stop_refused`
  (:166) and `nap_refused` (:177) their lines; `unreached` (:299) `machine_unreached_line(attempts, elapsed_ms)`.
- `restore_unfinished` and `machine_stopped` (:41, :55) take the cause's **message** as theirs and carry the cause's
  `kind, status, code, requestId` in `Carried`, each `None` where the cause had none (a plain error carries nothing).
- Readers on `EngineError`: `kind()` is a Provider's kind or what a wrapper carried; `status()` is a Provider's,
  a wrapper's carried one, GuestUnusable's and MachineUnreachable's; `code()` a Provider's, a carried one, a Road's or an
  Other's own; `name()` the TS class name (`GuestUnusableError`, `ExecFailedError`, `RestoreUnfinishedError`, `Error` for
  Provider and Other, the RoadError's own name for Road). `is_move_unanswered()` is true of MoveUnanswered and
  ResumeUnanswered; `is_machine_unreachable()` of MachineUnreachable whatever `exec_failed` says.

## classify (:13)

`message` is `error` or `""`. 429 concurrency, 401 auth, 402 and 403 plan, 404 missing, 409 conflict; 502 with the
message exactly `Failed to snapshot sandbox` (case matters) is snapshotUnavailable and is not retried; 502 to 504
transient; every other status unknown, 2xx included. `code` and `requestId` ride as given.

## Retry readings

- `is_missing` (:189): kind missing. `is_account_refusal` (:194): kind auth or plan.
- `should_retry` (:199): transient and `attempt < 3`. `backoff_ms(attempt, r)` (:203): `min(500 * 2^attempt, 8000) +
  floor(r * 250)`. `road_backoff_ms(retry, r)` (:294): `retry * 3000 + floor(r * 250)`. `ROAD_TRIES` 3,
  `PROVIDER_READ_CAP_MS` 20,000.
- `is_network_error` (:213): an Error whose message is exactly `fetch failed`, or whose own code or whose cause's
  code is one of `NETWORK_CODES` (8, errors.ts:210). Every variant counts, `Provider` included: a refusal is thrown
  as `Object.assign(new Error(message), w)`, so a `Provider(w)` whose message is `fetch failed` or whose code is a
  network code is one (rule E-2). Only `classify`'s bare object, which is no Error and no `EngineError`, never is.
- `never_sent` (:222): own or cause code in `UNSENT_CODES` (4), `Provider` included.
- `far_end_answered` (:225): the error carries a kind or a numeric status: Provider, a wrapper with a carried kind or
  status, GuestUnusable, MachineUnreachable, NotFirstLife (kind notFirstLife), VaultTooLarge and Exists and NoProvider
  and Invalid and PlaceAbsent (each has a kind word in the TS). Road and Other never.
- `is_capped` (:235): `name() == "TimeoutError"` and nothing else; an AbortError is not capped.
- `fetch_cap_ms(ms)` (:250): `min(2,147,483,647, max(0, ceil(ms)))` (oracle `fetchCapMs`).
- `abort(cap, signal)` (:261): `None` with neither; with a cap, a **fresh** token per call that fires at
  `fetch_cap_ms(cap)` and marks `timed_out`; with both, one token that fires on either, `timed_out` only for the cap.
- `road_code` (:275): only a Road named `TypeError` with message `fetch failed`. The codes are the cause's `errors`
  (AggregateError) or else `[cause.code]`; answers the **first** code when every one is in `OFFLINE_CODES` (7) and there
  is at least one; otherwise `None`. A missing cause is `[None]`, so `None`. `road_failed` is `road_code().is_some()`.
- `failure_words` (:335): not a fetch failure with an Error cause: the message. Otherwise `[cause.message, ...each
  aggregate member's message]`, empties dropped, joined `"; "`, after `"fetch failed: "`; all empty is `fetch failed`.

## until_reached (:319)

Calls `once` until it answers. A network failure (above) is retried: attempt `n` waits `link_backoff_ms(n, random)`
(wsp-wire limits) while `now - started + wait <= LINK_RETRY_WINDOW_MS` (60,000); past it the answer is
`EngineError::unreached(n, now - started, cause)`. Any other failure is answered at once, after one call and no
sleep. Nothing is kept between calls.

## Rules against tests

| Rule | Tests |
|---|---|
| classify per status, snapshot 502, empty vs absent request id | `errors_oracle::classify_reads_every_status_the_way_the_ts_does_with_each_field`, `errors::never_retries_429`, `errors::retries_generic_502_up_to_3_attempts`, `errors::does_not_retry_the_snapshot_502_signature`, `errors::maps_402_and_404_to_permanent`, `errors::the_request_id_rides_on_the_error_when_the_reply_carried_one`, `errors::the_request_id_is_absent_not_empty_when_the_reply_carried_none` |
| fetch cap rounding and timer bound | `errors_oracle::fetch_cap_ms_rounds_up_never_below_zero_and_never_past_the_timer` |
| sentences of each class | `errors_oracle::each_typed_error_says_the_ts_sentence`, `errors::exec_failed_is_the_unreachable_base_with_that_answers_own_sentence`, `errors::unreachable_keeps_the_sentence_it_was_handed_and_is_no_exec_failed`, `errors_oracle::the_constructors_make_the_ts_lines` |
| backoffs and retry count | `errors_oracle::backoff_doubles_from_half_a_second_to_eight_and_adds_up_to_249_ms_of_draw`, `errors_oracle::road_backoff_is_three_seconds_a_retry_plus_the_draw`, `errors_oracle::should_retry_is_a_transient_answer_before_the_third_attempt_only` |
| network, unsent, answered, capped, road code | `errors_oracle::a_network_failure_is_read_off_the_error_or_its_cause_and_a_gateway_answer_is_not_one`, `errors_pins::*`, `errors::is_network_error_*` (2), `errors_oracle::never_sent_is_only_the_four_codes_of_a_call_that_never_left`, `errors_oracle::the_far_end_answered_where_the_error_carries_a_kind_or_a_status`, `errors_oracle::road_code_names_the_first_offline_code_only_when_every_address_failed_offline`, `errors_oracle::is_capped_reads_the_timeout_name_and_nothing_else`, `errors_oracle::aborted_is_nodes_abort_error_with_no_string_code` |
| failure words | `errors_oracle::failure_words_spell_out_what_fetch_hid_in_its_cause` |
| wrappers carry the cause's fields | `errors_oracle::the_wrappers_carry_the_providers_kind_status_code_and_request_id_and_its_message`, `errors_oracle::missing_and_account_refusals_read_the_kind_alone` |
| until_reached: retry, rethrow, give up | `errors::retries_a_call_nothing_answered_at_the_link_backoff_and_returns_the_answer_that_came`, `errors::rethrows_a_failure_that_is_not_the_network_at_once_after_one_call_and_no_wait`, `errors::gives_up_once_the_next_wait_would_end_past_the_link_window_with_the_count_and_the_time` |
| abort: fresh cap per call, which fired | `errors_oracle::abort_gives_a_fresh_cap_each_call_and_says_which_fired` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `errors`), held by
`tests/errors_pins.rs`.

- E-1 `backoff_ms(attempt, r)` is `min(500 x 2^attempt, 8000) + floor(r x 250)` in f64: any attempt from 4 up is
  8,000 and nothing overflows, attempt 70 and 2,000 included (`backoff_holds_at_eight_seconds_for_any_attempt`).
- E-2 A `Provider` refusal with the words `fetch failed`, or the code `ECONNRESET` or `ECONNREFUSED`, is a network
  error; `bad gateway` is not; `ECONNREFUSED` is also never sent and `ECONNRESET` is not
  (`a_thrown_refusal_with_network_words_or_codes_reads_as_the_network`).
- E-3 `name()`: the class's own name where it sets one (NotFirstLifeError, EnvNotLandedError, DiskGoneError,
  CreateCancelledError, StopRefusedError, NapRefusedError, NoProviderError, besides the five the table above pins),
  and `Error` where it does not: MoveUnanswered, ResumeUnanswered, Unreached and a thrown `Provider` refusal
  (`every_variant_names_itself_as_its_ts_class_does`).
