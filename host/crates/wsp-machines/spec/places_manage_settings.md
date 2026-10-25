# places/manage, unit settings: list, rows, set, find, pending, choose, setUp, recipes, skip, estimate, setupLog, close

Port of `packages/runtime/src/places/manage.ts:122-145,425-492,722-871` at `65f6b89c7`, into
`src/places/manage/settings.rs`. Tests: `tests/places_manage_settings.rs`. `file:line` is `manage.ts` unless named. The
usage fix `Run wsp computers to read the ones this host holds.` is `NAMED_FIX` below.

## list and rows

- **L1 `list(now)`** (:425-435): the records (R1); each record's `forks_of` (V15), all asked at once; `rows_of` (V20);
  `spent_today` of the provider rows where present (a failure is none). Each row in `rows_of` order gains `forks` where
  its computer answered, `spentTodayUsd` where the spend names it, then goes through `with_limits` (R12).
- **L2 `rows()`** (:437): `rows_of(records)`; no fork room, no spend, no limits.
- **L3 `find(ref)`** (:722): the records, in R1 order, whose id or name is `ref` (two that share a name both).

## set (:439-492)

- **S1** The row: this computer (`here_row`), a record's `joined_row`, or a provider's `provider_row`, each with the mark
  as `mark_held()` else here. None: fail `no_such_place_refusal(id, [here's name, each record's name, each provider's
  provider_key_name])`, kind usage.
- **S2** `renamed` and `ssh` are `also.name` and `also.ssh` trimmed (an empty string after the trim is still given).
- **S3** The settings given are the keys of `set` other than `spawn` whose value is not absent (a `null` nap or turn
  limit is given: no nap, no limit). The call settles settings where it names none of a name, a login or a recipe, or
  where it gives `spawn`, a setting or a reset; a name, a login or a recipe alone sets nothing.
- **S4** Every check runs before any write, in this order, each a usage refusal: settles and `place_set_refusal(row, set,
  reset)`; a rename and `place_rename_refusal(row, renamed, every row)`; a login and `place_ssh_refusal(row, ssh)`; a
  recipe on an id with no record: `no_such_place_refusal(id, record names)`; then, last, a login other than the record's
  own (R19) is read as the same computer (S5).
- **S5 `same_computer_over(record, ssh)`** (:140-145): `reached_over(record, {ssh, keyPath of the record's road}, no
  password)`, no host key passed. Unread: usage `place_ssh_unchecked_refusal(ssh, name, unread)`; not the same computer:
  usage `place_ssh_other_refusal(ssh, name)`. Nothing is written.
- **S6** Settling, in the mark turn (R30): read `settings_of(id)`; `spawn` patches the held `spawn` part by part; the
  next settings are `{...held, ...given, ...spawn patch}` with each reset word taken off by `place_setting_dropped`; an
  empty result deletes the `caps` document, else it is put; `settings_held[id]` = the result. Two sets at once on one
  place both stand.
- **S7** A nap given (even `null`) or reset: `opts.napChanged(id)`.
- **S8** A rename or a login changes the record through `change` (name; `road.ssh` over the road's other fields). A
  rename renames every pending add naming this place. A new login on a record that dials back through a forward
  releases the old login's forward unless another record holds that login, and holds the new one (R23).
- **S9** A recipe: `follow_to(id, recipe)` (F1).
- **S10** Answer and emit `place.changed` with `with_limits(with_cap(the row off the written record, else the row from
  S1, row_ids()))`; settings are read fresh there, so the answer carries what was just set.

## follow, recipes

- **F1 `follow_to(id, recipe)`** (:124-135): through `change`, `recipe` set; following `NO_RECIPE` also takes `sync` off,
  following a recipe keeps it. Then: `NO_RECIPE` where the record had a `sync` sends `place.sync` `{placeId}`; any other
  recipe `sync_soon(id, 0)`; `recipes_moved` for the old slug then the new one, each once, neither for `NO_RECIPE` or
  none.
- **F2 `follow(id, recipe)`** (:757-761): no record: fail `no_such_place_refusal(id, names)`, usage. Answer
  `view_of(written else held, default_id())`.
- **F3 `recipe_changed(slug, after_ms)`** (:763-781): `recipes_moved(slug)` first. For each record following `slug`, in
  R1 order: the recipe is resolved once for all (`opts.recipes()?.resolve(slug)`, a failure none; the TS asks again
  for the next record while it is none); `sync_of(record, resolved)`. None: skip. No changes: `sync_soon(id, after_ms)`
  and skip. Changes: the name is answered; unless a setup or a sync runs there, `mark_sync(id, {state: behind, changes:
  each change's key, since: the record's `sync.since` else now as ISO})`; `sync_soon(id, after_ms)`.
- **F4 `followers()`** (:783-787): slug to names, slugs in first-seen order over R1, names in R1 order, `NO_RECIPE` and
  none left out.
- **F5 `unfollow(slug)`** (:839-847): each record following `slug` is moved to `NO_RECIPE` through `change` (re-checked
  inside the turn); its name is answered where written. `sync` stays. Then `recipes_moved(slug)`.
- **F6 `picks_of(id)`** (:849): the record's `picks`.

## pending, choose, setUp

- **P1 `pending()`** (:724): `pending_records()` (V1) through `pending_view`.
- **P2 `choose(ref, choices, recipe)`** (:726-733): the first pending add (V1 order) whose id, address or place id is
  `ref`. None: usage `no_pending_refusal(ref)`, fix `Run wsp computers to read the adds still pending.`. Else write it
  with `choices` replaced and `recipe` replaced by the one given (taken off where none is given) and answer its view.
- **P3 `set_up(ref, o)`** (:735-755), in order:
  1. `add_id` = `o.addId`, else `a_` and 12 hex;
  2. the pending add: first in V1 order whose id, address or name is `ref`, or whose place id is `ref`, or whose place's
     kept record is named `ref`;
  3. one not joined yet (no place id): usage `pending_not_joined_line(its name else address)`, fix
     `pending_not_joined_fix(address)`;
  4. the computer: the pending add's place, else the records whose id or name is `ref`; two: usage
     `two_places_refusal(ref, ids)`, fix `Name it by its id.`; none: usage `no_such_place_refusal(ref, names)`, `NAMED_FIX`;
  5. a setup running there with a `setup` on the record and neither choices nor recipe given: answer `{addId:
     record.setup.addId, place: view_of(record, default_id()), setup}` and start nothing;
  6. choices = `o.choices`, else the pending add's where `recipe_counts` shows any row, else none; none and the record
     has no `picks`: usage `place_no_picks_line(name)`, fix `CHOOSE_FIX`;
  7. drop the pending add;
  8. recipe = `o.recipe`, else the pending add's where no choices were given;
  9. `start_setup(id, add_id, choices and recipe or none, o.row)`; answer `{addId, place: view_of(record now, default_id()),
     setup?, said?}` as it answered.

## skip (:789-813)

- **SK1** No record: usage `no_such_place_refusal(id, names)`, `NAMED_FIX`.
- **SK2** A sign-in waiting live (`skippers["<id>/<row>"]`): run its skip and nothing else here.
- **SK3** Else the row must wait (`setup.waiting`) or have failed (applied, outcome `failed`), or usage
  `nothing_to_skip_line(row, name)`, fix `Skip a row the computer's page shows waiting or failed.`. Through `change`:
  the row is taken out and put back last as `{id, label: the failed row's, else the wait's, else the id; outcome:
  skipped, note: SKIPPED_FOR_NOW, step?}` (`step` off the failed row, else any row with that id); the wait goes from
  `setup.waiting`; `applied` keeps its `hash`, `at` and `items`, or is `{hash: "", at: now}` where there was none. A setup
  no longer running then sends `place.setup` `{addId, placeId, ...setup_outcome(setup, applied)}`.
- **SK4** Answer `view_of(record now, default_id())`.

## estimate, setupLog

- **ES1 `estimate(ref, choices)`** (:815-824): no `provision.estimate`: fail `NO_ESTIMATE_LINE`. The place: the first
  pending add whose id, address, name or place id is `ref`, its place; else a record whose id or name is `ref`. Answer
  `{neededBytes: weight.bytes, keptBytes: owned_floor_bytes(report.diskSizeBytes), unmeasured, freeBytes?}`, `freeBytes`
  where the report has one; with no place, `keptBytes` is `owned_floor_bytes(None)`.
- **LG1 `setup_log(id, step)`** (:826-837): no record: usage `no_such_place_refusal`, `NAMED_FIX`. No `HOME`: `[]`. Else
  `exec("tail -c <SETUP_LOG_TAIL_BYTES> <shell_quote(log path)> 2>/dev/null", LOG_READ_MS)` on `PlaceMachine(link_to(id))`
  (unlinked: the absent refusal); the non-empty lines, less the first where stdout's UTF-8 length reached
  `SETUP_LOG_TAIL_BYTES` (a cut line), and only those holding ` [<step>] ` where a step is named.

## on, close

- **C1 `on(f)`**: adds a watcher, answers its id; `off(id)` removes it. `emit` reaches watchers in the order added.
- **C2 `close()`** (:856-870), in order: cancel and clear the sync timers; `back.close()`; cut every live link with
  `this host is stopping`; wake every waiter with `false`; for each forward, destroy its connections, clear them, close
  its listener, run its `stop`, remove it; clear `awaiting`. Records, caps and pending adds stay in the store.

## What survives a restart

Settings (`caps`), the record's name, road, recipe and `applied` rows, and the pending adds with their choices. The
`settings_held` copy is rebuilt by `load` (places-a).

## Tests

| Rule | Test |
|---|---|
| L1, L2 | `list_forks_spend_and_limits_on_rows`, `rows_have_no_fork_room` |
| L3 | `find_by_id_or_shared_name` |
| S1 | `set_unknown_place_names_every_row` |
| S3, S4 | `set_name_alone_settles_nothing`, `set_refused_setting_writes_nothing`, `set_rename_taken_refused`, `set_recipe_on_here_refused` |
| S5 | `set_login_reaching_other_computer_refused`, `set_login_unread_refused` |
| S6 | `set_patch_spawn_and_reset`, `set_reset_all_deletes_caps`, `set_null_nap_is_given`, `set_two_at_once_both_stand` |
| S7 | `set_nap_calls_nap_changed` |
| S8 | `set_rename_renames_pending`, `set_login_moves_back_forward` |
| S10 | `set_answers_and_emits_fresh_row` |
| F1, F2 | `follow_none_drops_sync_and_frames`, `follow_recipe_keeps_sync_syncs_soon_moves_both_slugs`, `follow_unknown_refused` |
| F3 | `recipe_changed_marks_behind_and_names`, `recipe_changed_busy_not_marked`, `recipe_changed_no_changes_syncs_soon_unnamed` |
| F4-F6 | `followers_by_slug`, `unfollow_keeps_sync_names_written`, `picks_of_record` |
| P1, P2 | `choose_replaces_choices_and_recipe`, `choose_unknown_refused` |
| P3 | `set_up_not_joined_refused`, `set_up_two_names_refused`, `set_up_running_followed`, `set_up_uses_pending_choices`, `set_up_no_picks_refused`, `set_up_drops_pending_and_starts` |
| SK1-SK4 | `skip_live_sign_in_runs_skipper`, `skip_failed_row_goes_last_skipped`, `skip_waiting_row_leaves_wait_and_frames`, `skip_nothing_refused` |
| ES1 | `estimate_without_reader_refused`, `estimate_reads_floor_and_free` |
| LG1 | `setup_log_drops_cut_first_line_and_filters_step`, `setup_log_no_home_empty` |
| C1, C2 | `on_off_watchers`, `close_cuts_links_wakes_waiters_clears` |
| restart | `restart_keeps_caps_and_pending_choices` |
