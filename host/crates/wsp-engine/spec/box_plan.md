# box_plan (box-backend.ts:23-345)

Boat's classes, prices, budgets, the account's limits, the state table and the names that carry labels. Rust:
`src/box_backend/plan.rs`. Pure: nothing kept, nothing survives.

## Classes and prices

- `BOX_CLASSES` (:39), in this order: small 2 cpu 4096 MB 12 GB $0.018/h; default 4/8192/50/$0.036; large
  8/16384/125/$0.072; xlarge "extra large" 16/32768/251/$0.200. `box_root_disk_gb` (:49): small 75, default 75,
  large 160, xlarge none (never measured, never offered as more room).
- `box_class_for(size)` (:52): the class matching both figures exactly; else the first class in order whose cpu and
  memory are both at least the asked ones (an absent figure asks 0); else xlarge. `{}` is small; `{ cpu: 3 }`
  default; `{ cpu: 64, memMb: 1 }` xlarge (oracle `boxClassFor`). `class_of_type(word)` (:58) by the provider's
  word, `None` for any other (the person's word "extra large" is not a type).
- `BoxPricing` (:64): the rate of `box_class_for(size)`; default size 2 and 4096; storage free 0 GB at $0 billed from
  `""` with 10 named snapshots free and $1.70 a month each past them; no builder disk.
- `box_budgets()` (:76): `{ wakeAttempts: 1, daemonAnswersMs: 900000 }`, no `resumeAsks`, in that key order. The TS
  freezes one shared object; the Rust answers a fresh value each call, which no caller can change for another.
- Constants (:90-:130, :225-:240, :250, :277, :336) are the TS numbers (oracle `constants`): pause, create and
  snapshot 600,000 ms; resume 300,000; poll 2,000; restore 120,000; key kept 86,400,000; lost answer 120,000 asked
  every 5,000; in progress 2,000; backstop slack 60,000, retry 30,000 to 300,000, cap 20,000; trial TTL 7,200 s;
  limits kept 10,000; refusal log quiet 60,000; firewall hold 30,000 watched every 5,000; env landing 60,000; PUT
  5,242,880 bytes; GET 4,194,304; inline 30,000; `BOX_WORK_DIR` `/home/user`; name 120; snapshot name 63.

## The account's limits

- `plan_sizes(trial, usd)` (:175): trial: small and default, whatever the dollars. Else xlarge joins only at
  `usd >= 100`; an absent figure sells three.
- `limits_of(raw)` (:193), `raw` any JSON. A number field counts only when it is a finite JSON number; a text field
  only when a non-empty string. `planKey` = `sandboxPlanKey`. `trial` = `accessTier == "trial"` or `planKey ==
  "trial"`. `planUsd` absent on the trial; else `sandboxPlanDollars`, else the matching tier's `dollars` (by key, from
  `sandboxPlanTiers`), else the digits of `^box_(\d+)$` in the key. A window counts where its `limit` is a number;
  its `used` defaults to 0. `starts` is set only when none of the three is missing and `starts.unlimited` is not
  `true`. `canStart` is false only for a JSON `false` (`"no"` reads true). Output keys in this order, absent ones left
  out: trial, planKey, planUsd, machinesAtOnce, activeBoxes, starts, sizes, canStart, startBlockedReason,
  creditBalanceHours, billingStatus (oracle `limitsOf[].keys`).
- `place_limits_of(l)` (:182): `None` without `machinesAtOnce`. Else `plan { key?, trial, machinesAtOnce, sizes,
  running? }` with the sold classes as wsp-words `size_word` in table order (`2x4`), `running` = activeBoxes; then
  `starts` copied where present; then `creditHours` where present.

## States and names

- `state_map` (:256): init, provisioning, provisioned, cloning → starting; ready, idle, running → running; archiving,
  archived → paused; error, cancelled → gone. Any other word, `""` and `constructor` included, has no entry, and
  `state_of` reads it **running**, so a delete never reads a word it does not know as gone.
- `box_name(labels)` (:301): `wsp`, then one `;k=v` per label except `wsp` itself. wsp's keys shorten to a letter in
  rank order owner o, workspace w, createdAt t, builder b, smoke s, doctor d, host h, golden g, name n; any other key
  ranks after them under its own name, those ordered by `localeCompare` (wsp-words `collate`). Values encode `%`, `;`,
  `=` as `%25`, `%3b`, `%3d` (lowercase hex). Pairs go in while the name stays at most 120 UTF-16 units; the first that
  does not fit ends the name, and nothing after it goes in, even a shorter one.
- `box_labels(name)` (:319): `None` for no name and for a name whose first `;` part is not exactly `wsp`. Else
  `{ wsp: "1" }` then each part with `=` past index 0: the letter mapped back to its key, the value `%XX`-decoded (any
  case of hex; a `%` not followed by two hex digits stays). A part with no `=` or an empty key is skipped; a repeated
  key keeps its first place and its last value.
- `box_snapshot_name(name)` (:337): lowercase, every UTF-16 unit outside `[a-z0-9-]` a `-` (an emoji is two), leading
  dashes cut. Empty after that fails with `"<JSON of name>" leaves nothing a named snapshot can be called`. Up to 63
  units it stands; longer keeps its first 54 units, trailing dashes cut, then `-` and the first 8 hex of sha256 of the
  **original** name.
- `is_uuid` (:345): the 8-4-4-4-12 hex shape, any case, whole string.

## Open points

1. `box_name` lets a non-wsp key hold `=` (`a=b`), which `box_labels` reads back as key `a` (oracle `boxName` case
   3). After-port ticket, not a port change.

## Rules against tests

| Rule | Tests |
|---|---|
| class table, root disks, words | `box_plan_oracle::the_class_table_and_root_disks_are_the_ts_tables_in_order`, `box_plan_limits::the_class_table_is_boats_four_sizes_xlarge_at_0_200_and_named_snapshots_past_10_cost_1_70` |
| constants | `box_plan_oracle::every_constant_is_the_ts_value`, `box_plan::an_inline_command_is_bounded_under_the_edges_cut_and_above_the_detached_roads_own_execs` |
| budgets and pricing | `box_plan_oracle::the_budgets_and_the_pricing_are_the_ts_objects`, `box_plan::prices_by_class_small_to_xlarge_a_stopped_box_nothing_named_snapshots_past_the_free_ten`, `box_plan::declares_its_lifecycle_one_wake_attempt_fifteen_minutes_no_asking_again_a_backstop` |
| size onto class | `box_plan::reads_a_size_onto_the_class_table_the_match_else_the_smallest_that_holds_it_else_the_largest`, `box_plan_oracle::a_size_reads_onto_the_class_it_matches_else_the_smallest_that_holds_it_else_the_largest` |
| plan sizes, limits_of, key order, place limits | `box_plan_oracle::plan_sizes_sell_two_on_the_trial_and_xlarge_from_100_dollars`, `box_plan_oracle::limits_of_reads_each_raw_answer_into_the_ts_object_with_its_key_order`, `box_plan_limits::reads_the_owners_plan_machines_at_once_sizes_sold_and_starts_used_in_each_window`, `box_plan_limits::sells_16x32_from_the_100_plan_up_and_two_sizes_on_the_trial`, `box_plan_limits::carries_nothing_for_a_read_that_names_no_machine_cap` |
| state table, unknown reads running | `box_plan_oracle::the_state_table_reads_every_word_and_an_unknown_one_reads_running` |
| names: rank, locale order, encoding, the 120 cap | `box_plan::a_machines_labels_ride_in_the_box_name_shortened_and_read_back_whole`, `box_plan::a_name_past_the_apis_cap_keeps_the_pairs_in_rank_order_up_to_the_first_that_does_not_fit`, `box_plan_oracle::box_name_ranks_wsps_keys_then_the_rest_by_locale_order_and_stops_at_the_first_pair_that_does_not_fit`, `box_plan_oracle::box_labels_reads_back_only_names_wsp_made_with_their_keys_in_order`, `box_plan_oracle::a_name_and_its_labels_round_trip_through_the_encoding` |
| snapshot names, UTF-16 units, hash | `box_plan::a_snapshot_name_is_lowered_onto_the_providers_pattern_and_a_long_one_ends_in_a_hash`, `box_plan_oracle::a_snapshot_name_is_lowered_cleaned_and_a_long_one_ends_in_the_hash_of_the_whole`, `box_plan_oracle::a_character_outside_utf16s_basic_plane_is_two_dashes` |
| uuid | `box_plan_oracle::a_uuid_is_a_history_id_in_any_case` |
