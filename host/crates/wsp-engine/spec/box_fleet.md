# box_fleet (box-backend.ts:788-1044)

Create and its lost answers, replay, get, the listings, snapshots and templates, and backstops; plus the
`MachineBackend` and `Lifecycle` impls. Rust: `src/box_backend/fleet.rs`.

## The backend's declarations (:524-:563)

`box_capabilities()`: liveCloneForks false, pauseMode disk, replacesMachine, previewUrls, signedUrls false,
callbackRelay, diskSnapshots, images, snapshotsAnyLife, snapshotListing, templates, kept false, copies, ownNetwork;
`sizes` the classes the kept read sells in table order (every class with no read), each `{ name, cpu, memMb,
rateUsdPerHour }`. Pricing `BoxPricing`; lifecycle `BoxLifecycle` (budgets `box_budgets()`, a backstop, disk kept
14 days); base templates `base` for both kinds; `root_disk_of` the exact class only (`{ name: type, diskGb }`).
`offers()`: keyed_create, check_key, place_limits, root_disk_of, limits, list_snapshots, templates.

## Create

- `box_create_request(spec)` (:790): `{ type, ttlSeconds, noEnv: true, name, from? }` in that key order, as compact
  JSON. `type` is `box_class_for(spec)`; `ttlSeconds` `ttl_seconds_for(now + idleTimeoutMs)` or the cap with no
  window; `name` `box_name(spec.labels or {})`; `from` is `fromSnapshot` else `template`, left out when absent or
  `base`.
- `post_create(key, bytes)` (:810): POST `/sandboxes` with those bytes, header `Idempotency-Key`, cap `read_ms`. A
  failure is asked again when it is `idempotency_in_progress` (wait 2,000) or a lost answer: nothing answered (no kind
  or status) or a status of 500 and up (wait 0 on the first ask, 5,000 after). Lost words are `road_words` of the
  **first** lost answer. At or past 120,000 ms from the start, or on any other failure, the failure is answered.
- `box_create(spec, waiting, sent)` (:831): `before_start()`; a class the read does not sell is
  `EngineError::Invalid` with `refusal_line(size_refusal(size_word(class), capabilities().sizes), SIZE_PICK_FIX)`, no
  POST. The timer instant is read now. `bytes` is `sent` as given, else a fresh `create_request`. The key is the spec's,
  else a fresh lowercase UUID v4. A post failure goes through `start_refused(e, labels.name)`. `replayed` = no lost
  answer, a status present, and not `provisioning`; a fresh one is `counted()`. A lost answer warns
  `create_answer_lost_line("box" provider key name, lost, id)`. Where the spec has an idle window the backend's timer
  record is `{ until, span_ms }`. `waiting(boat_box_state_line(id, state), { id, state })` for each state as it first
  differs from the last heard, the answer's own included. Then `settle` until running (budget create, `name` for the
  cancelled line), `prove_road`, a PATCH of `{ name }` where the view's name differs from the bytes' name, and
  `land_envs` where envs has at least one key. Any failure there kills the box (its own failure ignored) **unless
  replayed**, then answers the failure. The handle is kind as asked, labels as asked.
- `box_replay_create(key, bytes, made_at)` (:880): where `now - Date.parse(made_at) >= 86,400,000 - 120,000 - 5,000`
  nothing is sent and the answer is `None`. Else `post_create`; a box not gone whose name differs is PATCHed to the
  bytes' name; the handle is kind sandbox, labels `box_labels(name)`, seen `{ state, createdAt? }`, replayed.
- `box_named_after(names)` (:894): every box on both listings whose name is exactly one of `names`, one row per id in
  first-seen order, `{ id, name, state, createdAt? }`.

## Reads

- `box_get(id)` (:903): kind sandbox, labels `box_labels(name)`, seen `{ state, createdAt? }` and, for a paused box
  with a string `snapshotCompletedAt`, `pausedAt` that.
- `pages(path, rows_key, query, enough)` (:917): query then `limit=200`, `cursor` when there is one; pages until no
  cursor or `enough(rows so far)`. `every_box()`: the plain listing then `state=archived`, duplicates kept.
- `box_list(labels)` (:940): one row per id, the first seen; only boxes whose name reads labels; every asked label
  equal; `size { cpu: vcpu, memMb: memoryGB * 1024 }` only with both; listing order.
- `box_list_snapshots(ids)` (:965): the named listing (a page without a `snapshots` array fails `GET /named-snapshots
  answered without a snapshots array`) as `{ id: name, name, sizeBytes or 0, createdAt }`, then the history: none
  when the asked ids hold no UUID; all of it with no ids; else pages until every UUID asked has been seen. History rows
  `{ id, sizeBytes or 0, restoredBytes? (a number contentSizeBytes), createdAt? (completedAt else createdAt), parent?
  (chainId where present, null kept) }`.
- `box_delete_snapshot(id)` (:982): a UUID is DELETE `/snapshots/<id>` with `X-Ascii-Confirm-Delete: <id>`; any other
  id DELETE `/named-snapshots/<id>`; a missing answer is done.
- Templates (:1000-:1014): promote answers the snapshot id unchanged, no call; get reads `/named-snapshots/<id>`;
  list maps the named listing; delete is `box_delete_snapshot`.

## Backstops (:1019-:1043)

`box_backstop(id, until, exact)`: without `exact`, nothing while one is being sent for this box or while the last
failure's wait has not passed. Mark sending; `ttl_cap()`; the instant is `until`, at most `now + cap * 1000`; without
`exact`, nothing when a timer is on record and `instant - last.until < 60,000` (an earlier instant included). PATCH
`{ ttlSeconds: max(1, ceil((instant - now) / 1000)) }`, cap `backstop_ms`, `once`. Success records `{ until: instant,
span_ms: instant - now }` and clears the failure. Failure records `{ at: now, wait: 30,000 }` for a first, doubled
from the last up to 300,000, and is answered. Sending is unmarked either way. `forget_backstop(id)` drops the timer and
the failure (a kill does it).

## Rules against tests

| Rule | Tests |
|---|---|
| declarations | `box_plan::says_what_a_box_can_and_cannot_do`, `box_fleet_hard::the_capabilities_list_every_class_before_the_first_read_and_the_plans_after` |
| create bytes, from, name, timer, key | `box_fleet_create::creates_from_the_stock_image_on_the_trial_*`, `box_fleet_create::creates_from_a_named_snapshot_*`, `box_fleet_create::the_create_carries_the_idle_window_as_the_stop_timer_*`, `box_fleet_hard::a_create_with_no_key_sends_a_fresh_uuid_and_one_with_no_envs_sends_no_landing` |
| replayed, envs, failure kills unless replayed | `box_fleet_create::a_create_answered_from_an_earlier_key_reads_replayed_*`, `box_fleet_create::a_create_left_in_error_or_never_ready_*`, `box_fleet_create::a_create_boat_cancels_*`, `box_fleet_create::an_environment_not_landed_*`, `box_fleet_create::a_landing_of_the_environment_runs_detached_*`, `box_fleet_create::a_create_whose_box_reads_ready_and_then_cannot_run_a_command_*`, `box_fleet_create::a_healthy_create_proves_the_road_once_*` |
| the person hears each state once | `box_fleet_hard::the_person_hears_each_state_the_box_passes_through_once_in_order` |
| invalid size | `box_fleet_hard::a_size_the_plan_does_not_sell_is_refused_before_any_create_in_the_runtimes_words`, `box_request::the_first_create_after_a_host_start_checks_the_size_*` |
| lost answers, in progress, 5xx, cap, give up | `box_fleet_lost::*` (6 create cases) |
| replay window, named after | `box_fleet_lost::a_replay_sends_the_attempts_bytes_*`, `box_fleet_lost::a_replay_of_an_attempt_as_old_as_boat_keeps_a_key_*`, `box_fleet_lost::lists_the_boxes_named_exactly_one_of_the_names_*` |
| get, list, both listings, labels, size | `box_fleet_listing::get_reads_the_labels_off_the_name_*`, `box_fleet_listing::a_stopped_boxs_handle_carries_its_stop_*`, `box_fleet_listing::lists_wsps_boxes_alone_off_both_listings_*`, `box_fleet_listing::a_state_the_table_never_learned_reads_running_*`, `box_fleet_hard::the_listing_keeps_wsps_boxes_matching_every_label_asked_with_the_size_in_mb`, `box_fleet_hard::the_unfiltered_listing_keeps_every_box_wsp_named_in_listing_order`, `box_fleet_hard::get_reads_a_box_as_a_sandbox_and_a_paused_ones_last_save_as_its_pause` |
| snapshots and templates | `box_fleet_listing::a_named_snapshot_is_the_template_*`, `box_fleet_listing::lists_every_snapshot_*`, `box_fleet_listing::asked_for_named_snapshots_alone_*`, `box_fleet_listing::deletes_a_snapshot_by_what_its_id_is_*`, `box_fleet_hard::asked_for_ids_that_are_no_history_id_the_snapshot_listing_reads_no_history_at_all`, `box_fleet_hard::a_named_listing_without_its_array_is_a_failure_not_an_empty_list`, `box_fleet_create::checks_the_key_against_the_accounts_limits_and_nothing_else` |
| backstops: slack, exact, cap, backoff, in flight, forget | `box_fleet_listing::pushes_the_providers_stop_timer_*`, `box_fleet_listing::an_exact_backstop_*`, `box_fleet_listing::off_the_trial_the_backstop_*`, `box_fleet_listing::a_failed_backstop_waits_its_backoff_*`, `box_fleet_listing::a_second_backstop_never_starts_*`, `box_fleet_listing::a_backstop_the_provider_never_answers_*`, `box_fleet_listing::a_backstop_that_lands_resets_the_backoff_*` |
