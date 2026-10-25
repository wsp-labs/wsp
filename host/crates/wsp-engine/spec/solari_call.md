# solari_call: the Solari backend and its one HTTP call

Source: `packages/engine/src/solari-backend.ts:1-380` at `a4033dc34` (the dead-code cut, #2441). Rust:
`src/solari_backend.rs` (`SolariBackend`, the constants, `preview_token_expiry`, `state_of`). Tests:
`tests/solari_call.rs`, `tests/key_check.rs` (the key check runs through this backend), hard tests in
`tests/solari_hard.rs`. Oracle: `tests/fixtures/oracle/solari.json`.

## Construction (solari-backend.ts:191)

- C1. `base_url` defaults to `https://api.getsolari.com`; every call's URL is `base_url + path`, nothing inserted.
- C2. Budgets: each `MoveBudgetsGiven` field that is `None` takes `PAUSE_MS` 240000, `RESUME_CAP_MS` 30000,
  `STATE_READ_MS` 30000, `PROVIDER_READ_CAP_MS` 20000 (`read_ms`). Given ones are used as given, fractions too.
- C3. `capabilities()` is, in the wire struct's order: liveCloneForks true, pauseMode memory, replacesMachine true,
  previewUrls, signedUrls, callbackRelay, diskSnapshots, images true, snapshotsAnyLife false, snapshotListing, templates
  true, kept false, copies, ownNetwork true, sizes `[{cpu 2, memMb 4096, rateUsdPerHour rate(2, 4096)}]`. The JSON the
  TS writes is in `solari.json` `capabilities` (rate `0.11000000000000001`).
- C4. `rate_usd_per_hour = cpu * 0.035 + (mem_mb / 1024) * 0.01`, evaluated in that order in f64.
- C5. `pricing()`: that rate, default size 2 x 4096, snapshot storage `{freeGb 10, usdPerGbMonth 0.05, billedFrom
  "2026-10-01"}` with no named-count fields, `builder_disk_gb` `Some(20)` (`tool_sizes::BUILDER_DISK_GB`).
- C6. `lifecycle()` is `Some`, its budgets `{wakeAttempts 2, daemonAnswersMs 30000, resumeAsks {everyMs 60000, forMs
  1800000}}`, the same value from every backend; nothing can change it (the TS freezes it). No backstop, no quiet read.
- C7. `offers()`: check_key, list_snapshots, templates true; every other optional method false.

## The call (solari-backend.ts:201-252)

- R1. Headers in this order: `Authorization: Bearer <key>`, then `Idempotency-Key` where keyed, then `Content-Type:
  application/json` only when there is a body. The body is the JSON of the value, `serde_json` with key order kept.
- R2. The abort: `cap_ms` where given, else `read_ms` for a GET, else none; combined with the caller's signal
  (`errors::abort`). A fresh cap per send.
- R3. A fetch that rejects with a road code (`errors::road_code`: TypeError "fetch failed" whose cause code, or every
  code of an AggregateError cause, is one of ENOTFOUND EAI_AGAIN EAI_FAIL ENETUNREACH ENETDOWN EHOSTUNREACH EHOSTDOWN)
  is sent again, keyed or not, up to `ROAD_TRIES` 3 sends in all for the whole request. Before each resend one warn line
  `provider_road_retry_line("<METHOD> <path>", code, retries + 1, 3)` (so "try 2 of 3", "try 3 of 3") and a sleep of
  `road_backoff_ms(retries, clock.random())` = `retries * 3000 + floor(random * 250)`. The third road failure is
  thrown as it came. The road budget is never reset by an answer.
- R4. Any other rejection: thrown at once unless keyed and not yet resent; a keyed call is resent once after
  `backoff_ms(attempt, clock.random())`; a second rejection is thrown. ECONNREFUSED, ECONNRESET, ETIMEDOUT are not road codes: one send.
- R5. A 2xx answer: the body text parsed as JSON, `{}` for an empty body. The reply is kept for its headers.
- R6. A non-2xx answer: body trimmed; parsed as JSON `{code?, error?}`, else (not JSON) `{error: first 300 chars}`,
  empty text `{}`. `classify(status, code, error, header x-request-id)`. Retried only while `should_retry` (kind
  transient, i.e. 502 to 504 except a 502 "Failed to snapshot sandbox", and attempt < 3), after
  `backoff_ms(attempt, clock.random())` = `min(500 * 2^attempt, 8000) + floor(random * 250)` (the TS draws
  `Math.random()`; the port draws from the clock so tests are exact).
  `attempt` counts answers only, starting at 1, so a road drop before the first answer leaves three answer tries.
- R7. A refusal that is not retried fails as `EngineError::Provider(WspError)` whose message is the provider's `error`,
  or `<kind> (<status>)` where it said nothing (`unknown (500)`), and `request_id` from the header.

## create (solari-backend.ts:256-301)

- K1. Body keys, each present only when truthy (0 and "" are absent), in this order: kind, template, fromSnapshot, cpu,
  memMb, diskGb (camelCase), envs, metadata (the spec's labels), lifecycle `{onTimeout}` (from onIdle), timeoutMs =
  `min(idleTimeoutMs, IDLE_TIMEOUT_MAX_MS 21600000)`. Byte-exact in `solari.json` `bodies`.
- K2. Keyed by the spec's idempotency key on the first ask, a fresh random UUID where it has none and on every later
  ask; the one key rides every resend of that ask.
- K3. A refusal that is missing (404) with the message exactly `Snapshot not found` on a spec with `fromSnapshot`: the
  listing is read (a failed listing reads as not listed). Not listed: the refusal is thrown as it came. Listed: with
  `waited = now - started`, where `waited + 15000 > 180000` the refusal is thrown with kind `transient` and message
  `snapshot_listed_refused_line(snapshot, waited)`, status and the rest kept; else warn
  `snapshot_listed_wait_line(snapshot, 15000, waited)`, tell `waiting` `image_served_wait_line("Solari", 15000)` with
  no box, sleep 15000, ask again under a fresh key.
- K4. The handle: id `sandboxId`, kind from the reply else the spec's, stream URL from the reply, labels the spec's,
  `seen` only where the reply carried `createdAt` (state `state_of(reply.state or "running")`), `replayed` when the
  reply header `Idempotent-Replayed` is exactly `true`.

## Reads

- G1. `get(id)`: `GET /sandboxes/<encodeURIComponent id>`; the handle's id is the view's `sandboxId` else the asked id,
  kind the view's else sandbox, no stream URL, labels the view's metadata, seen `{state_of(view.state), createdAt?}`.
- G2. `state_of`: starting, running, paused as themselves; archived, releasing, gone are gone; any other word running.
- L1. `list(labels)`: `GET /sandboxes` with query `metadata.<k>=<v>` per label in order, then `cursor` where one is
  held, form-encoded as URLSearchParams does (space as `+`); no `?` when the query is empty. Pages followed while
  `nextCursor` is a non-empty string. A page without `sandboxes` is an empty page. Rows in order: id, `state_of`,
  labels (metadata or `{}`), size only where both cpu and memMb are present.
- S1. `list_snapshots`: `GET /snapshots`; a reply whose `snapshots` is not an array fails with `GET /snapshots answered
  without a snapshots array`. Rows: id, name only where a non-null string, sizeBytes, restoredBytes equal to sizeBytes,
  createdAt where present, parent where the key is present (null kept as `Some(None)`).
- T1. Templates: `promote_snapshot` POSTs `{name}` to `/snapshots/<id>/promote` and answers `templateId`;
  `get_template` and `list_templates` map `{templateId, name, status, error?, createdAt?}` to `{id, name, status, error,
  createdAt}`, a null error or createdAt absent. A list reply whose `templates` is not an array fails with `GET
  /templates answered without a templates array`. `delete_template` and `delete_snapshot` DELETE their path.
- T2. `check_key` lists templates and answers `Value::Null`; its failure is the listing's.
- E1. `preview_token_expiry(token, now)`: for cut in `[last index of ".", len]`, skipping a cut <= 0, decode
  `token[..cut]` as base64url the way node does (invalid characters skipped, decoding stops at `=`) to UTF-8 (lossy)
  and read the first `"exp":(\d+)`; else `now + 3600000`. Cases in `solari.json` `expiry`.

## Null, empty, caps

- Empty body on 2xx is `{}`; an empty error body names `<kind> (<status>)`. `IDLE_TIMEOUT_MAX_MS` 21600000;
  `SOLARI_INLINE_MAX_MS` 26000; `SNAPSHOT_LISTED_MS` 180000; `SNAPSHOT_LISTED_EVERY_MS` 15000; `ROAD_TRIES` 3;
  three answer tries for a transient status.
- Nothing survives a restart: the backend holds no state beyond its options.

## After-port notes (not changed in the port)

- A `cpu` of 0 is dropped from the create body as falsy (K1), so a caller cannot ask for zero; harmless today.
- `list` never stops on a provider that repeats a cursor.

## Rules and their tests

Files: `solari_call.rs` (call), `solari_hard.rs` (hard), `key_check.rs` (key), `solari_machine.rs` (machine).

| Rule | Tests |
|---|---|
| C1, C3, C4, C5 | call `declares_the_measured_solari_capability_truth`, `offers_no_size_its_forks_have_not_come_up_at_a_2x8_ask_is_refused_naming_2x4`; hard `capabilities_write_the_ts_json_byte_for_byte` |
| C2 | machine `ships_four_minutes_for_a_pause_half_a_minute_for_a_resume_and_a_read_and_takes_shorter_for_tests`; call `reads_at_most_the_measured_default_by_default` |
| C6 | call `declares_its_lifecycle_and_its_resume_cap_shared_by_every_backend_built_here` |
| R1, K1 | hard `create_body_writes_the_ts_json_byte_for_byte`; call `idle_timeout_above_six_hours_is_sent_as_six_hours`, `maps_on_idle_and_idle_timeout_to_lifecycle_and_timeout_omitting_both_otherwise`, `sends_the_disk_as_camel_case_disk_gb_when_asked_for_and_leaves_it_otherwise`, `passes_the_specs_envs_as_the_create_bodys_envs_and_sends_none_when_none` |
| R2 | call `ends_each_of_solaris_reads_of_a_machine_the_fleet_and_the_snapshots_at_the_reads_cap`; machine `a_resume_carries_its_cap_a_pause_half_its_budget_and_a_read_the_reads_cap_into_the_fetch` |
| R3 | call `tries_a_call_that_never_left_this_computer_three_times_and_logs_one_line_per_retry`, `gives_up_after_the_third_try_and_throws_what_the_road_threw`, `reads_a_road_out_of_an_aggregate_error_one_error_per_address_tried`, `every_road_takes_it_create_snapshot_pause_poll_and_promote_survive_one_dropped_lookup` |
| R4 | call `never_retries_a_refused_reset_or_timed_out_connection`, `retries_a_create_once_under_the_same_key_when_the_fetch_itself_throws`, `lets_a_second_thrown_fetch_propagate_and_never_retries_a_thrown_exec` |
| R6 | call `a_road_that_flapped_before_the_first_answer_leaves_the_gateway_retries_whole`, `never_retries_an_answer_a_409_is_the_callers_on_the_first_reply`, `surfaces_snapshot_unavailable_without_retrying`; hard `a_transient_status_is_answered_three_times_then_thrown` |
| R7 | call `keeps_a_plain_text_error_body_as_the_refusals_words_instead_of_an_empty_one`, `names_the_class_and_status_where_the_provider_said_nothing_at_all`, `a_refused_calls_error_carries_the_request_id_the_replys_header_named` |
| K2 | call `sends_the_specs_key_on_create_and_nothing_on_the_posts_the_provider_ignores_it_for`, `holds_one_key_across_the_retries_of_a_create_and_mints_one_when_the_caller_sent_none` |
| K3 | call `asks_again_under_a_fresh_key_while_the_listing_holds_the_snapshot_and_says_each_wait`, `is_missing_at_once_where_the_listing_does_not_hold_the_snapshot`, `is_not_missing_where_the_listing_still_holds_the_snapshot_once_the_wait_runs_out` |
| K4 | call `reads_idempotent_replayed_off_the_create_reply_onto_the_handle` |
| G1, G2 | hard `a_state_the_provider_names_that_the_table_never_learned_reads_running`; call `creates_a_sandbox_and_url_encodes_ids_on_follow_up_calls` |
| L1 | call `list_reads_labels_and_size_per_row_follows_the_cursor_and_filters_by_label`; hard `list_encodes_labels_as_url_search_params_does` |
| S1, T1 | call `lists_every_snapshot_on_the_account_with_billed_size_and_the_name`, `promotes_a_snapshot_to_a_template_and_reads_lists_and_deletes_templates`, `a_templates_reply_of_another_shape_is_a_failure_never_an_empty_registry`, `a_listing_reply_of_another_shape_is_a_failure_never_an_empty_account` |
| T2 | key `asks_the_template_table_the_cheapest_read_the_key_opens_and_nothing_else` and the other five key tests |
| E1 | call `reads_the_ms_exp_claim_from_the_real_token_shape`, `falls_back_to_the_measured_60_min_ttl_when_the_token_is_opaque`; hard `preview_token_expiry_reads_tokens_as_node_decodes_them` |
