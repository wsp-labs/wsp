# sweeps: the orphan reaper, snapshot storage and the names that carry ownership

Source at `a4033dc34`: `packages/engine/src/orphans.ts` (174), `snapshot-storage.ts` (172), `snapshot-names.ts` (31),
`labels.ts` (28). Rust: `src/orphans.rs`, `src/snapshot_storage.rs`, `src/snapshot_names.rs`, `src/labels.rs`, and
`BUILDER_IDLE_MS` (6 h) in `src/golden_seam.rs`. Tests: `tests/sweeps.rs`, hard tests in `tests/sweeps_hard.rs`.
Oracle: `small.json` (`ages`, `snapshotNames`).

## Labels and names

- W1. `is_reserved`: the `poc` key is present, whatever its value. The label constants are the TS values.
- W2. `golden_name(host, golden, v)` = `wsp-<host>-<golden>-v<v>`; `project_snapshot_name` = `wsp-<host>-project-<project>-<stamp>`;
  a host not matching `^[a-z0-9]+$` fails `a snapshot name needs a host of lowercase letters and digits, got <JSON
  string of host>` (empty and `h-1` included). `name_owner`: `^wsp-([a-z0-9]+)-(.+)$`, `.` not matching a newline, so
  `wsp-h1-\nx` has no owner; `wsp-h1-` none; `wsp-h1-a-b-c` is `h1`. Oracle `snapshotNames`.

## orphans.ts

- W3. `whose_machine`: by the `wsp-owner` label only.
- W4. `reap`: empty owner fails first (words in the stub). `older_than_ms` default 600000; `now` read once; the
  listing is the given one or `backend.list(None)` (its failure is the call's failure); `known_ids()` read once after.
  For each row in order, skipped silently unless: state running, not reserved, `wsp` label exactly `1`, id not known.
  builder = `wsp-builder` is `1`; age = now - parse(`createdAt` label) (no label or an unparseable one is no age;
  Date.parse reads ISO 8601, a date-only value as UTC midnight). Backstop: own 60000, else builder `BUILDER_IDLE_MS`,
  else `older_than_ms`. Killed: own with no age or age >= backstop; none (no owner label) with age >= backstop; foreign
  never. Spared rows carry id, labels, builder, whose, owner where labelled, age where known, backstop and the rate of
  the row's size else the pricing's default size.
- W5. A kill is `stop(backend.get(id))`. A missing failure (from get or stop) is skipped, neither reaped nor failed.
  Any other failure is `{id, message: "could not stop: <message>"}` and the sweep goes on. A kill that lands is reaped
  `{id, labels, builder, reason Own or Orphan, age}`. `failed` is `None` when empty. Kills run one at a time in order.
- W6. `lost_workspace(m, owner, now)`: state running or paused; not reserved; `wsp=1`; own; not `wsp-builder=1` or
  `wsp-smoke=1`; and no readable age or age >= 60000.
- W7. `strays(rows, read)`: in order, rows not gone, not known by id, name not asked, created_at parseable and age >=
  600000, each with its age.
- W8. `describe_age`: oracle `ages`: `age unknown`; a negative age is 0; `< 60000` floor seconds `s old`; `< 3600000`
  floor minutes `min old`; else hours with JS `toFixed(1)` (4500000 is `1.3 h old`, 4860000 `1.4 h old`).

## snapshot-storage.ts

- W9. `snapshot_monthly_usd(bytes, p, named)` = `max(0, bytes / 1e9 - free_gb) * usd_per_gb_month + max(0, named -
  free_named or 0) * (usd_per_named_month or 0)`. `billed_by_count`: GB price exactly 0 and a named price present.
  `named_count`: rows with a name.
- W10. `ownership_of`: recorded id is kept; `name_owner(name) != host_id` (no name included) is foreign; else kept
  while `now - parse(created_at) < 60000` or the age is unreadable, orphan past it. `split_by_owner` refuses an empty
  host id first; keeps row order within each group.
- W11. `snapshot_storage`: count, totalBytes (sum), freeGb, usdPerGbMonth, billedFrom, monthlyUsd (with named count),
  `named {count, free (or 0)}` only where billed by count, then kept, orphans, others as `{count, bytes}`.
- W12. `retention_plan(manifest, rows, forked_from, pricing, keep)`: head = `golden_head(manifest)`; a version's parent
  is the version whose snapshot is its `parentSnapshotId` (absent if not in the manifest), or with none recorded the
  highest version below it, `assumed` when one exists and the version has no `base`. The chain walks parents from the
  head, stopping at none or a repeat; `parent_assumed` is the head's own step. keep = first `keep` (default 2) of the
  chain; older = the rest reversed (oldest first); off-chain = versions below the head not on the chain, ascending.
  Each of older then off-chain: a version whose snapshot `forked_from` names workspaces is guarded `{version,
  workspaces}`, else dropped, and also abandoned when off-chain. freed = sum of dropped snapshots' listed sizes (a
  missing row 0). namedFreed = named rows among the dropped. saves = cost now minus cost after (bytes - freed, named -
  namedFreed). No head: everything empty, nothing dropped. Versions above the head are never touched.

## Restart

Nothing is kept; every rule reads labels and stamps the provider holds, so a host restart reads the same.

## Rules and their tests

Files: `sweeps.rs` (ported; the TS `it.each` of 25 rows is 25 tests made by its `verdict!` macro), `sweeps_hard.rs` (hard).

| Rule | Tests |
|---|---|
| W1, W2 | ported: the six snapshot-names tests (`a_golden_versions_snapshot_and_its_template_read_the_same_name_and_the_host_reads_back_off_it` to `a_host_outside_the_class_the_provider_takes_is_refused_rather_than_written_into_a_name_no_parser_reads_back`); hard `names_match_the_ts_oracle` |
| W3, W4 | ported `kills_only_unknown_wsp_labeled_running_machines_older_than_10_min`, the 25 `verdict!` rows, `describes_a_spared_machine_with_whose_it_is_its_age_its_backstop_and_the_rate`, `fetches_no_machine_it_leaves_alone_a_spared_row_is_judged_from_the_listing`, `honors_a_custom_age_threshold_for_workspaces_not_for_builders`, `kills_nothing_when_the_listing_fails`, `reads_the_claimed_ids_after_the_listing_returns_so_a_create_that_lands_during_it_is_claimed`, `refuses_an_empty_owner_before_listing_anything` |
| W5 | ported `skips_a_listed_machine_that_is_gone_by_the_time_it_is_fetched_or_killed_and_still_processes_the_rest`, `a_row_whose_kill_fails_with_anything_else_is_reported_by_id_and_the_rest_of_the_pass_still_runs` |
| W6, W7 | ported: the `verdict!` rows for lost and stray machines |
| W8 | ported `describes_an_age_for_a_person_never_negative_floored_hours_with_one_decimal`; hard `describe_age_matches_the_ts_oracle` |
| W9 to W11 | ported `counts_every_snapshot_on_the_account_and_bills_the_gb_past_the_free_tier_at_the_published_rate`, `inside_the_free_tier_the_monthly_cost_is_zero_never_negative`, `splits_the_sum_by_who_made_each_snapshot_so_the_count_on_the_line_says_why_the_account_holds_them`, `a_row_anything_records_is_kept_whatever_its_name`, `this_hosts_mark_with_nothing_recording_the_id_is_an_orphan`, `no_mark_of_this_host_is_foreign_whether_another_hosts_a_persons_own_or_a_name_from_before_the_mark`, `the_one_split_serves_snapshots_and_templates_alike`, `a_marked_row_the_provider_listed_inside_the_grace_is_kept`, `without_this_hosts_mark_there_is_no_split_at_all_rather_than_one_that_claims_every_unmarked_row`; hard `storage_view_writes_named_only_when_billed_by_count` |
| W12 | ported: the eleven retention tests from `keeps_the_head_and_its_recorded_parent_and_offers_the_older_ancestors_oldest_first_with_what_they_hold_and_save` to `a_snapshot_the_listing_does_not_carry_counts_as_holding_nothing`; hard `retention_keeps_versions_above_the_head` |
