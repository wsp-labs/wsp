# places/views: pending adds, what a remove takes off a computer, the link's tunnels, and the rows

Port of `packages/runtime/src/places/views.ts` (431 lines) at `65f6b89c7`, into `src/places/views.rs`. Tests:
`tests/places_views.rs`. `file:line` is `views.ts` unless named.

`PlaceViewsArea::new(ctx, records, setup: Rc<dyn ViewsSetup>, links: Rc<dyn LinkReads>)`. `ViewsSetup` is what views
reads off places-a's setup area (`waiting`, `closed_at`, `woken`, `link_to`); `LinkReads` is what it reads off
`ctx.door` (`backend_of`, `forking_backend`, `folder_computer`), so a test hands fakes for both. It implements
places-a's `LinkViews`.

## Pending adds (`pending-computers`)

- **V1 `pending_records()`** (:48-54): `store.list(PENDING)`, each read as a `PendingRecord` (a `PendingComputer` plus the
  record-only `keyPath`, `hostKey`, `undo`, `login`; unknown keys kept, `.passthrough()`), one that does not parse
  skipped, sorted by `startedAt` ascending, stable.
- **V2 `put_pending(p)`** (:55-58): `store.put(PENDING, p.id, p)` with the record-only fields, then `onSetup`
  `{type: "place.pending", id, pending: pending_view(p)}`, which carries none of them.
- **V3 `drop_pending(id)`** (:59-62): `store.delete`, then `onSetup` `{type: "place.pending", id}` with no `pending`.
- **V4 `floor_while_choosing(pending, id)`** (:66-83): with no provisioner, no record or no `HOME`, one write of
  `{...pending, placeId, step: "choosing"}` and done. Else write `step: "floor"`, run `provision.floor(PlaceMachine(link_to(id),
  record.name, home), home, a stage that drops its lines)`; the failed rows are those with outcome `failed`, and a throw is
  one row `{id: "floor", label: SETUP_STEP_WORDS.floor, outcome: failed, note: first_line_of(e)}`. Then, where the
  pending document is gone from the store (a remove or a set up took it), write nothing. Else write `{...pending,
  placeId, step: "choosing"}` plus `failed: {said: floor_failed_line(rows)}` only where a row failed. The writes spread
  the `pending` passed in, not the stored document (an after-port question, see the report).
- **V5 `floor_once(pending, id)`** (:86-94): the pending id is in `flooring` for the run, removed after it, failure or not.
- **V6 `taken_back(pending)`** (:98-108): `said` = `ADD_NOT_TAKEN_BACK_LINE` where the pending add has no `login`, no
  `undo` script, or the wiring no `undo` road; else `wiring.undo({ssh: login, keyPath?, hostKey?}, undo)` gives
  `ADD_STOPPED_LINE`, or `<ADD_NOT_TAKEN_BACK_LINE>: <first line of the failure>`. Then one write of `{...pending,
  failed: {said, fix: ADD_STOPPED_FIX}}`; the step and the rest stay.

## What a remove takes off a computer

Nothing here fails a remove: each answers empty on any failure.

- **V7 `unmerged_over(id, held)`** (:114-121): no `HOME` answers `{took: [], kept: []}`. Else `unmerge_servers(
  machine_server_port(machine), home, stores_over(id, home))`, bounded by `UNMERGE_MS` (60 s); a failure or the bound is
  no outs. `took` = the lines of each out that read; `kept` = the lines of each out that did not (`unread`).
- **V8 `stores_over(id, home)`** (:124-128): `{}` where `recording.stores_on` is absent or `links.folder_computer(id)` is
  `None`; else `stores_reached(login_of(), stores)`. A `login_of` failure propagates to the bounded caller.
- **V9 `store_files_off(id, held)`** (:133-144): no `HOME` is `[]`. Else `unland_files(machine, home, roots' sources,
  stores)` and each gone relative path written `landed_at(home, roots, rel)`, bounded by `UNMERGE_MS`; failure `[]`.
- **V10 `plugins_off(id, held, linked, login, password)`** (:150-171): names = `made_by_setup(held, plugins)`. Answers
  `{off: [], kept: []}` without `HOME`, without picks, with no names, or with no `provision.undo`. Else plan
  `undo(held.picks, names as plugins rows, home, no stores)` (failure: no plan). `kept` starts as the names with no
  planned entry keyed `plugins/<name>` that carries a `cmd`. Then for each planned entry with a `cmd`, in plan order:
  run it over the link (`PlaceMachine.exec(cmd, UNDO_MS)`) where `linked`, else over the login with `runOver(login,
  cmd, UNDO_MS, password)`, else not at all; exit 0 pushes `plugin_off_line(label)` to `off`, anything else (a throw,
  no road) pushes the label to `kept`. A plugin the computer had before wsp (row `present`) is never in names.

## Facts, backends and tunnels

- **V11 `facts_on(id, held)`** (:177-189): races `links.forking_backend(id)` then a fresh `record_of(id)` (else `held`;
  a failure is `held`) against `ADD_FACTS_MS` (2 s) answering `held`. A record whose facts are already on it answers as
  fast as `forking_backend` does.
- **V12 `backend_from(id, facts)`** (:193-197): `LinkBackend::of(link_to(id), facts)`, put in `ctx.backends` under `id`
  (replacing), and answered.
- **V13 `tunnelled(id, event)`** (:201-216): `false` for any event but `tunnel.data` and `tunnel.end`, and where no
  forward whose key starts `<id>:` holds a connection under the event's `tunnelId` (first match in insertion order).
  Else `tunnel.data` writes the base64-decoded bytes to that connection and `tunnel.end` ends it; `true`.
- **V14 `cut(id, reason)`** (:220-228): nothing where no link is live. Else remove it from `ctx.live` first, then cancel
  its `seen` timer, close its forward (failure swallowed, not awaited), close the reach, close the socket with code 1000
  and `reason`.
- **V15 `forks_of(record)`** (:234-256): `None` where the computer is not linked; where linked with no backend yet, start
  `forking_backend(id)` in the background (not awaited, failure swallowed) and answer `None`; `None` where the backend
  does not offer `capacity` (`offers().capacity` false). Else `capacity()` bounded by `CAPACITY_MS` (5 s, what `machine.capacity on <name>`): `running` =
  `machines.running` (a napping fork is not counted), `room` = `fork_room(capacity, min(defaultSize.memMb,
  machineMemMb), largest image sizeBytes or None where it is 0)`. A failure or the bound is `None`.

## Rows

- **V16 `view_of(record, defaulted)`** (:258-314), a joined computer's row, keys in this order where present: `id`,
  `kind: computer`, `name`, `default` (= `defaulted == id`), `os`, `mac` (`mac_kind_of(model)` where the report names a
  model), `shape`, `diskFreeBytes`, `copies`, `present` (= a live link), `takesRuntime: true` (only where the report says
  `takesRuntime: true` and `leave_asks(report)`), `joinedAt`, `lastSeenAt`, `daemonVersion`, `behind` (`{word:
  place_daemon_behind(report), fix: place_update_line(name), act: update}` where behind), `agents`, `agentVersions`,
  `signIns` (`sign_ins_of(report, vault or {}, applied rows)`, absent where the report lists no `logins`), `logins` (off
  `backendFacts`), `takesForks: true`, `road` (`ssh`, `from`, `back` alone, never `keyPath` or `hostKey`; `{}` where the
  record's road holds none of the three), `home`, `uptimeMs`, `reportedAt`, `dialled`, `blocked` (`place_blocked`),
  `toolsBlocked` (`place_no_tools_line`), `setup`, `applied` (`applied_view`, no `pick` or `createdAt`), `picks`,
  `recipe`, `sync`. Key order on the wire is wsp-wire's `PlaceView` field order (see report).
- **V17 `joined(id, pending, req, expect, from, at)`** (:323-357), the join's half of the prove, in this order:
  1. remove `id` from `joining`, whatever follows;
  2. `verify_place_bytes(pending.publicKey, expect, req.signature)` false: refusal `PLACE_KEY_REFUSAL`;
  3. `taken_report(req.report)` throws: refusal with its message;
  4. `place_blocked(taken.name, taken)`: refusal with that sentence;
  5. `req.code` absent or `devices.spend(code, at)` false: refusal `PLACE_CODE_REFUSAL`;
  6. where an install awaits that code, note `place_id = id` on it before the record's first write;
  7. `keep({id, name: taken.name, publicKey, joinedAt, lastSeenAt, reportedAt: all ISO(at), report: taken})`;
  8. `mark_default(id)`: the last computer joined is the default;
  9. where `req.client` names a client, `devices.admit(client.name, at)`;
  10. emit `place.joined` `{place: view_of(record, id), from}` (so `default: true`); answer `{report: taken, device?}`.
  A refusal at 2 to 5 leaves no record, no mark, no event, and the code unspent at 2 to 4.
- **V18 `forget(id)`** (:360-375), in order: cut a live link with `removed from this host`; drop the backend; in the
  record turn, drop `kept[id]` and `store.delete(PLACES, id)`; `store.delete(CAPS, id)`; drop `settings_held[id]`; in the
  mark turn, delete the mark only where it names `id`; `woken(id, false)`; drop `closed_at[id]`; emit `place.removed`
  `{placeId: id}`. Forgetting an id with no record still deletes and emits.
- **V19 `here_row(marked)`** (:378-401): `id: here`, `kind: computer`, `name` and, where given, `label`, `mac`, then
  `default` (= `marked == here`), `os`, `shape`, `diskFreeBytes`, `home` off `wiring.here()`; `daemonVersion` =
  `hereDaemon.held()` where it answers; `behind: {word, fix: hereDaemon.fix, act: install}` where that version is behind;
  `present: true`, `takesForks: false`, `buildsImages: false` always.
- **V20 `rows_of(held)`** (:404-408): marked = `mark_held()` else here; rows = here, each record's `joined_row`, each
  provider's `provider_row`, in that order, each through `with_cap(row, rows)`. Exactly one row is default, and it is
  this computer when the mark names a row no longer here.
- **V21 `joined_row(record, marked)`** (:410): `view_of` plus `image_facts(id, links.backend_of(id))`.
- **V22 `provider_row(id, marked)`** (:411-424): `id`, `kind: provider`, `name: id`, `default`, `takesForks: true`,
  `rateUsdPerHour` where known, `sizes` where non-empty, then `image_facts(id, provider_backend(id))`.

## What survives a restart

The pending adds (store); `joining` and `flooring` are memory. A pending add left at `step: "wsp"` with an `undo` script
is what places-a's `load` hands to `taken_back`.

## Tests

| Rule | Test |
|---|---|
| V1-V3 | `pending_records_sorted_and_passthrough`, `put_pending_frame_hides_record_fields`, `drop_pending_frame_has_no_pending` |
| V4, V5 | `floor_while_choosing_no_provisioner_goes_choosing`, `floor_failed_row_says_on_pending`, `floor_throw_is_one_floor_row`, `floor_pending_gone_writes_nothing` |
| V6 | `taken_back_without_undo_road`, `taken_back_undo_ok`, `taken_back_undo_fails_first_line` |
| V7-V9 | `unmerged_over_no_home_is_empty`, `store_files_off_failure_is_empty` |
| V10 | `plugins_off_only_installed_rows`, `plugins_off_link_then_login_roads`, `plugins_off_kept_names_and_labels` |
| V11 | `facts_on_answers_held_after_add_facts_ms` |
| V12 | `backend_from_replaces_kept_backend` |
| V13 | `tunnelled_other_place_is_false` (the data path needs a `Forward`, whose `LoopbackListener` binds a real port: covered once places-a's pane ports test one) |
| V14 | `cut_removes_link_then_closes_1000` |
| V15 | `forks_of_unlinked_none`, `forks_of_room_and_running`, `forks_of_capacity_bound_is_none` |
| V16 | `view_of_full_row`, `view_of_road_hides_key_path_and_host_key`, `view_of_takes_runtime_needs_leave_asks`, `view_of_sign_ins_absent_without_logins` |
| V17 | `joined_writes_record_marks_default_emits`, `joined_bad_signature_spends_no_code`, `joined_blocked_spends_no_code`, `joined_spent_code_refused_no_record`, `joined_notes_awaiting_install_road`, `joined_admits_client_device` |
| V18 | `forget_clears_record_caps_mark_and_emits`, `forget_keeps_mark_on_other_place` |
| V19-V22 | `here_row_fields_and_behind_install`, `rows_of_order_and_one_default`, `rows_of_mark_on_gone_row_falls_to_here`, `joined_row_adds_image_facts`, `provider_row_rate_sizes` |
