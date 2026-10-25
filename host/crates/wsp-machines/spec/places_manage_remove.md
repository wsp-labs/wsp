# places/manage, unit remove: holds, remove, update

Port of `packages/runtime/src/places/manage.ts:90-195,494-720` at `65f6b89c7`, into `src/places/manage/remove.rs`.
Tests: `tests/places_manage_remove.rs`. `file:line` is `manage.ts` unless named. Constants: `THREADS_END_MS` = 60,000
(:91); `RUNTIME_CHECKOUTS` = `/wsp/projects` (:94).

## Helpers

- **H1 `runtime_checkouts_script()`** (:97): exactly
  `for d in '/wsp/projects'/*/checkout; do [ -d "$d" ] && printf '%s\n' "$d"; done; true` (the `\n` is the two
  characters backslash and n, for printf).
- **H2 `runtime_project_of(checkout)`** (:101-104): the one name `<n>` of `/wsp/projects/<n>/checkout`; `None` for no
  checkout, any other path, an empty name or a name holding `/`.
- **H3 `reaches_itself(record, login, password)`** (:151-157), over `reached_over` (R22): same is `stands`; not same is
  `elsewhere` with `other` where named; unread with `refused` is `{refused: unread}`; any other unread throws a usage
  refusal `place_login_unchecked_refusal(login.ssh, name, unread)`.
- **H4 `holds_of(id, record)`** (:162-168): `answers` = a live link now. `recording.holds_on(id, answers)`; not answering:
  the holds plus `away: true`, nothing read. Answering and the report saying `takesRuntime: true`: the holds' `unsaved`
  gains `unrecorded_unsaved(id)` after its own lines.
- **H5 `unrecorded_unsaved(id)`** (:184-195): the checkouts recorded on the place (`projects_on`, each `checkout` where
  set); `exec(H1, timeout THREADS_END_MS)`, a failure read as exit -1 with its message as stderr. A non-zero exit is one
  line, `unpushed_unread_line("/wsp/projects", last_line(stderr) else "the listing exited <code>")`. Else each non-empty
  stdout line not recorded is read by `read_unsaved(checkout, checkout, exec)` and its line kept where it answers one.
- **H6 `images_sized(id)`** (:172-182): the held images (`images_on`, none where absent); the provider backend's
  `list_snapshots(ids)` only where its capabilities say `snapshotListing: true`, it offers the method (`offers().list_snapshots`), and there is at least one image (a
  failure lists nothing). Each image is `{id}`, plus `sizeBytes` where its listing row gives one other than 0.

## holds (:560-568)

- **HL1** A record: `holds_of`. No record and not a provider id: `{forks: [], projects: [], unsaved: []}`. A provider:
  `{forks: [], projects: [], unsaved: sub_holds_on(id) else [], images?}` with `images` only where non-empty.

## remove (:570-720)

- **RM1** A provider is an id with no record that `provider_ids()` lists and whose backend this host holds; an id listed
  with no backend falls to RM4.
- **RM2** Provider with `forget`: `forget_records_on(id)` where present, `forget_key(id)` where present, emit
  `place.removed`, answer `{removed: true, swept: [], note?}` with the key's sentence as `note`. Nothing is asked of the
  provider.
- **RM3** Provider: `remove_provider({place_id, backend, owner: opts.owner() else "", known: machines_on(id) else [],
  forget_key?, confirm: opts.killConfirm?, images?})` (places-a's provider_remove), `images` only where both `images_on`
  and `drop_image_on` exist. Where it answers `removed: true`, `release_records_on(id)` and emit `place.removed`. Answer
  as it answered.
- **RM4** No record: `{removed: false, swept: []}`, nothing else.
- **RM5** Read before anything goes: `read` = a live link at this moment; `holds` = `holds_of`.
- **RM6** Refusals, in order, all usage, nothing taken: `forget` on a computer whose link answers:
  `place_forget_answers_refusal(name)`; away with forks or projects (`place_forgets_only`) and no `forget`:
  `place_away_refusal(name, absent_computer(name, null).said)`; unsaved work and no `force`: `place_unsaved_refusal(name,
  unsaved)`.
- **RM7** `reach` = the live link now; `login` = R19; `answers` = a `leave` road, a login, and `login_answers` (R21).
- **RM8** Where it answers, `root_over(login, ask.sudoPassword, remove, name)`. A changed host key
  (`is_place_host_key_changed`) is not a failure: it marks the login elsewhere and no password goes there. Any other
  failure (a sudo that wants a password) fails the remove here, before anything is taken.
- **RM9** Where the key did not change and it answers, `reaches_itself`; with `forget`, its usage refusal reads as
  `{refused: said}` instead of failing. `stands` = it stands; `away` = `place_login_elsewhere(login.ssh, name, other)`
  where elsewhere.
- **RM10** `projects` = each project recorded there whose checkout is under `/wsp/projects` (H2), read now.
  `force` = `ask.force`, or a link read at RM5 and the report naming `takesRuntime` at all.
- **RM11** The forks and projects go first: `forget_on(id)` where away, else `drop_on(id)`; a refusal there fails the
  remove (records already gone stay gone). Then `plugins_off(id, record, linked now, login only where it stands,
  password)` (V10), then `store_files_off` (V9) where linked.
- **RM12** The leave over the login, where there is a `leave` road and a login: a login that does not stand runs nothing
  and notes `{at: login.ssh}`. One that stands runs `leave({placeId, name, report, ssh: login, sudoPassword?, force:
  true only where force, projects})`; its lines are `swept` and `note` is `place_swept_over_ssh_line(name)`. A failure
  notes `{at, said: message}`, or `{at}` alone for a login refusal (T9).
- **RM13** Where no note yet: away sets `note` to `place_login_elsewhere_removed_line(login.ssh, name, other)` unlinked or
  `place_elsewhere_swept_over_link_line(name, away)` linked. Unlinked: `left` = `place_forgotten_line(name, forks went)`
  with `forget`, else `place_still_installed_line(name)`; a note still unset becomes `left`, or `<place_login_road_line(
  name, at, said)>; <left>` where the login road said something. Linked: `unmerged_over` (V7), then
  `exec(thread_cgroups_end_script(), 60 s)` (failure ignored), then `reach.request("place.leave", {force?: true,
  projects})`; `swept` = the servers' lines, the cgroup script's non-empty stdout lines, then the answer's `swept` as
  strings. A login road that did not finish (and the login not elsewhere) sets `note` to `place_swept_over_link_line(name,
  at, said)`. The servers' unread lines join the note with `; `. A failure of any of the three is `note` = `<before>,
  and <failed>` or `<failed>`, `failed` being `<name> was connected but did not finish the sweep: <message>; run wsp leave
  on that computer`, `before` = `<away>, so nothing was changed on the machine it reaches` or the login road line.
- **RM14** `swept` = plugin lines, then the store files, then the rest. Plugins kept add `plugins_kept_line(name, kept)`
  after `; `.
- **RM15** Then, in order: cut the link (`removed from this host`) where linked; release the back where the record has
  one and no other record has the same login `ssh`; drop every pending add naming this place; `forget(id)` (V18).
  Answer `{removed: true, took: what went, swept, note?}`.

## update (:494-558)

- **U1** No record: fail `no_such_place_refusal(id, record names)`, no kind.
- **U2** A setup running there (`setting` holds it and `setting_now(record)` says so): fail with that sentence, kind
  conflict. A sync running (`syncing`): fail `place_syncing_line(name)`, kind conflict. Nothing is sent.
- **U3** `moving` = `report.daemonVersion < DAEMON_VERSION`. No `update` road: moving fails `NO_PLACE_UPDATER` (no kind);
  not moving answers `{name}` with no daemon (U6 still applies).
- **U4** With the road: no link and a login: `root_over(login, ask.sudoPassword, update, name)` first, then
  `reaches_itself`; `refused` fails as a login refusal with its sentence (T9), `elsewhere` fails usage
  `place_login_other_refusal(login.ssh, record, other)`. With a link nothing is read over the login.
- **U5** `update({placeId, name, report, daemon: moving, link?, ssh: login?, sudoPassword?})`, asked behind or not.
  Where it answers a landing: `to` = `until_daemon_version(id, from, opts.updateWaitMs else UPDATE_WAIT_MS)` (R14); where
  `to != from`, `facts_on` (V11) is awaited; `daemon` = `{road, at, kept?, from, to, note?}` with `note` =
  `place_update_slow_line(name, round(wait / 1000))` only where `to < DAEMON_VERSION`.
- **U6** A record following a recipe (`recipe` set and not `NO_RECIPE`) syncs soon (`sync_soon(id, 0)`). Answer `{name,
  daemon?}`.

## What survives a restart

A remove's effects are the store's: the record, its caps and the mark go (V18), the pending adds naming it go. An update
writes nothing of its own; the record moves when the computer dials back.

## Tests

| Rule | Test |
|---|---|
| H1, H2 | `runtime_checkouts_script_text`, `runtime_project_of_cases` |
| H3 | `reaches_itself_unchecked_is_usage` |
| H4, H5 | `holds_away_reads_nothing`, `holds_takes_runtime_adds_unrecorded_checkouts`, `holds_listing_failure_one_line` |
| H6, HL1 | `holds_unknown_id_empty`, `holds_provider_images_sized` |
| RM2 | `remove_provider_forget_asks_nothing` |
| RM4 | `remove_unknown_id_not_removed` |
| RM6 | `remove_refuses_unsaved_without_force`, `remove_away_with_forks_needs_forget`, `remove_forget_on_answering_link_refused` |
| RM8, RM9 | `remove_host_key_changed_runs_nothing_and_goes`, `remove_sudo_refusal_fails_before_anything`, `remove_login_elsewhere_runs_nothing` |
| RM10-RM12 | `remove_over_login_with_link_takes_runtime_forced`, `remove_leave_over_login_failure_then_link_sweeps` |
| RM13 | `remove_unlinked_without_login_still_installed`, `remove_unlinked_forget_forgotten_line`, `remove_link_sweep_lines_in_order`, `remove_link_sweep_failure_note` |
| RM14, RM15 | `remove_plugins_lines_first_and_kept_note`, `remove_releases_back_unless_shared`, `remove_drops_pending_and_record` |
| U1-U3 | `update_unknown_refuses`, `update_refused_while_setup_or_sync`, `update_behind_without_updater_refuses`, `update_current_without_updater_answers_name` |
| U4 | `update_over_login_reads_place_file_first`, `update_login_elsewhere_refused` |
| U5 | `update_waits_for_new_version`, `update_slow_note_after_wait`, `update_link_road_passes_link_and_login` |
| U6 | `update_follower_syncs_soon` |
