# codex_limits
Source: packages/adapter-codex/src/limits.ts (64 lines), reset.ts (89 lines) at a3b870a6e.
Tests: packages/adapter-codex/test/reset.test.ts (12 cases), the `creditsOf` describe of adapter.test.ts (5 cases). Rust:
`src/codex/limits.rs`, `src/codex/reset.rs`; tests `tests/codex_limits.rs`.

## What it does
Reads a Codex plan's windows, its banked resets and the reset a limit-stopped turn waits on, off app-server answers; and the two scripts
that read and spend a banked reset outside any turn, through wsp-catalog's `app_server_script`, with their parsers.

## Behaviour
1. `reset_ms(v)` is `v * 1000` when `v < 1e11`, else `v` (limits.ts:51): seconds and ms epochs both read as ms.
2. `credits_of(answer)` (limits.ts:56): `rateLimitResetCredits.availableCount` must be a whole number at or above zero (`2.0` counts,
   `1.5`, `-1`, `"2"`, `null` do not), else the whole field is `None`. A `credits` that is not an array gives `{count}` alone. Each credit
   that is an object with a string `id` (the empty string counts here, fields.ts `str`) becomes `{id, status, expiresAt?}`; its status is
   the given one only when `resetType == "codexRateLimits"` and the status is one of `available`, `redeeming`, `redeemed`, `unknown`, else
   `unknown`; `expiresAt` is `reset_ms` of a finite number, absent otherwise. An empty list is `{count, credits: []}`.
3. `stopped_until(windows, now)` (limits.ts:74): the windows whose `resetsAt` is set and after `now`, sorted by `usedPercent` descending,
   then `resetsAt` descending; the first one's `resetsAt`, or `None`.
4. `USAGE_LIMIT_CODE` is `usageLimitExceeded`.
5. `snapshot_of(answer)` is `rateLimitsByLimitId.codex` when an object, else `rateLimits` when an object (limits.ts:84).
6. `limit_of(snapshot, account, credits)` (limits.ts:89): `primary` (fallback 300 minutes) then `secondary` (fallback 10080); a slot is read
   only when it is an object with a finite `usedPercent`; its kind is `limitKindOfMinutes(windowDurationMins or the fallback)` (≤300
   session, ≤10080 week, else month); `resetsAt` is `reset_ms` of a finite number. No window is `None`. Key order: `windows`, `plan`
   (the snapshot's string `planType`, the empty string included, else the account's plan), `status` (`ok` when `rateLimitReachedType` is
   absent or null, else `reached`), `account` (`{id, label?}` where the account names an id, else its label as the id), `credits`.
7. Scripts (reset.ts:334): `cd ~ && export K='v' ...` for every pair of `build_env({base: road.env, home: road.home})`, in its order,
   space-joined, then a newline and the server script run under `programWord("codex", launch)`. `build_env`'s refusal of a relative home
   stands.
8. `reset_read_command`: one stage, `initialize` (id 1), `initialized`, `account/read` (id 2), `account/rateLimits/read` (id 3) with no
   params, waiting for 3 answers. `reset_credit_command`: stage one is `initialize`, `initialized`,
   `account/rateLimitResetCredit/consume` (id 4) with `{idempotencyKey, creditId?}`, both slugs, waiting for 2; stage two is the limits
   read, sent only once the consume answered.
9. `limit_in(answers, sign_in)` (reset.ts:357): no limits result is `None`; email and plan are non-empty strings of the sign-in; the id is
   the answer's non-empty `accountId`, else the email; `limit_of(snapshot or {}, {id?, label?: email, plan?}, credits)`, and where that
   is `None`, `{windows: [], account?: {id, label?}, credits?}`.
10. `parse_reset_read` (reset.ts:375): no answer to id 1 is `None`. A sign-in (id 2's `account`) that is an object with `type: "apiKey"`
    is `{keyed: true}`; else `{keyed: false, limit?}`.
11. `parse_reset_credit` (reset.ts:385): no answer to id 4 is `{answered: false}`; an error is `{answered: true, refused: <message>,
    tooOld: code == -32601}`; a result is `{answered: true, outcome: <non-empty outcome, else "unknown">, limit?}` with no sign-in.
12. `CODEX_PLAN_RESETS.too_old` is `is older than 0.141 and has no resets`.

## Edge cases
- `credits_of({rateLimits: {}})` and `rateLimitResetCredits: null` are `None`; `NaN` cannot reach Rust (serde_json holds none).
- A credit with no id is skipped; the count is still the server's, not the list's length.
- A server that died before answering the consume sends no limits read and reads `{answered: false}`.

## Behaviour nobody wrote down
- Two `str` readers: limits.ts uses fields.ts's (accepts `""`), reset.ts its own (refuses `""`). Keep them apart.

## Decisions
1. `count` is written as an integer (`u64`), as the TS writes `2` for `2.0` (rule 4.25).

## After the port
- Nothing.

## Tests (tests/codex_limits.rs, 17; TS 12 + 5)
- Rules 1, 2: `credits_of::reads_each_credit_s_id_status_and_expiry_seconds_made_ms_and_an_empty_list_as_de`,
  `credits_of::fails_closed_a_count_that_is_not_a_whole_number_at_or_above_zero_reads_the_whole`,
  `credits_of::never_reads_a_credit_of_another_kind_or_one_in_a_state_it_does_not_know_as_avail`,
  `credits_of::reads_the_count_alone_where_the_details_are_null_or_not_a_list`,
  `credits_of::reads_nothing_where_the_answer_has_no_reset_field_as_a_codex_older_than_resets_a`.
- Rules 3, 4: through the turn, `codex_turn_more.rs` `a_turn_the_plan_s_usage_limit_stopped::*` (4 cases).
- Rules 5, 6: `reading_the_banked_resets::asks_the_sign_in_and_the_limits_in_full_under_the_login_s_own_home_and_path_and`;
  the turn's `a_codex_account_s_plan_limits_on_the_app_server::*` cases (windows by length, month, reached, buckets).
- Rule 7: `reading_the_banked_resets::refuses_a_relative_home_as_the_turn_env_does`, `spending_one::kills_only_the_pid_the_shell_recorded_for_the_server_and_waits_on_no_clock`.
- Rule 8: `spending_one::consumes_under_the_caller_s_key_and_the_credit_it_named_reads_the_limits_again_o` (the read sent after the
  consume answered: `after == 2`), `spending_one::leaves_the_credit_to_the_backend_s_pick_where_none_is_named`,
  `spending_one::refuses_a_credit_id_or_a_key_that_is_not_one_plain_word_before_it_reaches_the_sh`.
- Rules 9, 10: `reading_the_banked_resets::reads_a_sign_in_by_api_key_as_keyed_which_has_no_reset_to_spend`, `reads_nothing_where_the_server_never_answered`.
- Rule 11: `spending_one::hands_every_outcome_on_as_codex_said_it_one_it_does_not_know_included`,
  `spending_one::reads_a_server_that_has_no_such_method_as_one_too_old_for_resets_and_any_other_r`,
  `spending_one::says_the_consume_went_unanswered_where_the_server_died_before_answering_it_and_s`.
- Rule 12: `spending_one::is_the_agent_s_one_registered_set_of_reset_roads`.
