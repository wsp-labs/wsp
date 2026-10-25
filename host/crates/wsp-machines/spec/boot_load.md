# boot_load
Source: `packages/runtime/src/machines/boot.ts:16-326` (about 310 of 1,071 lines) at `65f6b89c7`. Tests: integration
cases in `runtime-golden-import.test.ts`, `runtime-builders.test.ts`, `no-provider-hydrate.test.ts`, `gone.test.ts`,
`runtime-gone.test.ts`, `pause.test.ts`, `status.test.ts`, `runtime-comeback.test.ts`, `sub-queue.test.ts` (under
"Rules against tests"), and the scenes of `oracle/boot_load.oracle.ts`: 6 builder scenes
(`tests/fixtures/oracle/boot_builders.json`), the 35-cell phase table and 13 more hydration scenes
(`boot_hydrate.json`), 4 re-read scenes (`boot_reread.json`). Rust: `src/machines/boot/load.rs`.

## What it does
How a starting host reads its image builders and its workspace records against what the provider says now: each
builder's life (whether it may be reused, sealed or reaped), each workspace's phase, a record held on a stand-in while
nothing can be asked about its machine, and the read again that loads or settles it once something can.

## Behaviour
1. `LOAD_READS_MS` = 1,500 (:18): the one deadline every load read shares.
2. `sealable_at(place, first_life)` (:44): true on first life; else the backend at `place ?? places.wired` (the places
   table, then the place door) has `snapshotsAnyLife`; no backend reads false.
3. `life_of(stored, machine, sealable)` (:49), first match wins: `foreign` when the machine carries a `wsp-owner` label
   that is not `state.owner` (no labels, or no such label, is ours); `held` when `heldBy` is set, is not this host's and
   this process's own hold, its heartbeat is fresh (`now - parse(heartbeat) < HELD_TTL_MS` 900,000; a heartbeat that
   does not parse is fresh), and either another host holds it or its pid is alive; `stale` when not sealable or
   `building`; else `reusable`. (Fixture: 11 stored builders, one per road.)
4. `live_of(record, machine)` (:62): the builder view copies `kind, baseTemplate, setupSha, createdAt, firstLife, size`
   and `import`, `base`, `parentSnapshotId`, `retired` only where set.
5. `admit(stored)` (:78): the backend at `place ?? wired`; none ends quietly. `get(id)` through `observed`; a failure
   of kind `absent` or `noProvider` ends quietly; `missing` reads `read_gone` and deletes `builders/<id>` only on
   `gone`; any other failure is thrown (it stops the listing pass, scene "admit: unreadable places ..."). A closed
   host after the read ends quietly. The state is `seen.state` when the view carries one (no second read), else
   `state()`. `firstLife = stored.firstLife == true && state == running`; `size = stored.size ?? size_built(shape_of(
   machine), at.pricing.defaultSize)`; a first life that dropped is written back; the builder is set; a sealed record
   arms its grace at `sealed.at`.
6. `admit_once` (:111): one read per stored id at a time; a second caller gets the same future; the key is removed when
   it settles either way.
7. `refresh_builders()` (:126): every call starts a numbered pass and becomes `latest`. A pass lists `builders`, and for
   each row, unless a newer pass started (then it answers `latest` and applies nothing more): unknown rows are
   admitted (awaited); known rows that are not `own` re-derive with `firstLife = stored.firstLife == true &&
   builder.firstLife` and `size = stored.size ?? current size`. After the loop, unless superseded, every non-`own`
   builder whose row is gone is removed. `own` builders are never re-read or removed here.
8. `read_within(at, stored, load, held, asked)` (:149): with no load, `asked`. With one, a timer at `max(0, until -
   now)` resolves with `held(Error(late_read_line(machineId)))` and records the read in `load.late`; the read landing
   first cancels the timer and answers itself (or its error).
9. `load_late(id, read)` (:170): marks `rereading`, waits the read out (its failure ignored), reads the stored record,
   hydrates it with the late read when the host is open, warns `workspace <id> was not loaded once its provider
   answered: <why>` on failure, unmarks; then, still open and held away, `reread_held(id, "record load")`.
10. `is_held_away(id)` (:190): no live entry, or an absent stand-in machine on a record that is not `queued`.
11. `reread_held(id, by)` (:200): only a live entry held away and not being re-read. `sight_machine(backend_for(
    record), machineId)`, a failure ending quietly; a closed host or a replaced entry ends quietly. A sighting with a
    machine re-reads the stored record and hydrates it with the sighting, then starts `sync_daemon` on the answer.
    Otherwise `mark_gone(attach(record, dead_machine(m)), m, gone_words(m, {by, at: now, answer?}))`, with the answer
    the load kept for this record. The kept answer is dropped once the record is no longer held away.
12. `hydrate_workspace(raw, seen, load, asked)` (:229), in order:
    a. `queued`: `attach_queued(stored)`, answer `None`.
    b. `released`, or `sub.cloudGone`: `attach(copy, dead_machine)`, answer `None`, nothing read.
    c. `module_of(kind).backend(stored)`; a failure warns `workspace <id> is left as it was: <why>` and answers `None`
       with nothing attached.
    d. `golden_kind`: `sandbox` for `golden == ""` (no image read); else `image_of(golden).version.kind`, a failure or
       none reading `sandbox`.
    e. The machine: the sighting's, else `read_within(...)`. A failure that is not `missing` holds the record away on
       an absent stand-in of `golden_kind` that refuses every call with that error. `missing` on a record not `gone`
       keeps `provider_said(e)` as the held answer and holds it away with `gone_unconfirmed_line(m, said)`; on a `gone`
       record it is a dead machine and `missing = provider_said(e)`.
    f. `at_provider`: `gone` when missing; none when held away; else the sighting's state, the view's `seen.state`, or
       one `state()` read (a failure: none).
    g. The phase table (pinned cell by cell in `boot_hydrate.json`): none keeps the stored phase; a stored `gone`
       leaves only for `running` (running) or `paused` (napping); provider `gone` is gone; `paused` napping; `running`
       running; any other provider state turns `pausing` into napping and keeps the rest.
    h. The record: the stored one with the phase and `screen: {streamUrl}` when the machine has one. Gone keeps
       `stored.gone ?? gone_words(m, {by: "record load", at: now, answer?: missing})`; any other phase deletes `gone`.
       Napping deletes `diskGone`, and takes `nappedAt` from a parseable `seen.pausedAt` that differs from the stored.
    i. `attach(record, machine)`; a moved `nappedAt` with the phase unchanged persists; a changed phase warns
       (`gone_log_line(id, gone)` for gone, else `workspace <id> was left <stored> and its machine is <provider> at
       the provider; the record hydrates <phase>`) and persists.
    j. Only a `running` record not held away answers its entry: a stored `napping` or `waking` marks it `unchecked`,
       and the idle window is touched.

## Edge cases
- A builder whose read fails with an unexpected error stops the pass at that row; rows after it are not admitted.
- A record held away at the load deadline is reported by `is_held_away` and kept until `reread_held` or `load_late`.
- `is_held_away` of an id with no entry is true.

## Behaviour nobody wrote down
- Rule 3.4: `refresh_builders` re-checks `pass == passes` after each await; `admit_once` takes its key in the same
  step it checks it.
- Rule 3.15: `Date.now()` in `life_of` is the clock's now.
- Restart: builders rows and workspace records are the only state read; the held answers, the admit map and the pass
  counter start empty.

## Decisions
1. `held(e)` is a closure building the absent stand-in; `asked` is a shared future so `load.late` and the timer share
   one read.

## After the port
- A builder read that fails another way stops the whole refresh pass, so one bad provider hides every builder after it.

## Rules against tests
| Rule | Tests (`tests/boot_load_scenes.rs` replays a scene by name; `tests/boot_load.rs`) |
|---|---|
| 2-5 | scenes `admit_every_life_a_stored_builder_can_read_as`, `admit_a_provider_whose_copy_*`, `admit_unreadable_places_*`, `admit_a_host_closed_*` |
| 6, 7 | scenes `refresh_own_builders_stay_*`, `refresh_two_admits_of_one_row_share_one_read`; `an_older_pass_neither_drops_nor_admits_after_a_newer_one_listed` (runtime-golden-import.test.ts:515) |
| 8, 9 | scene `the_loads_deadline_holds_a_late_read_*`; `a_late_read_that_fails_holds_the_record_on_the_providers_words`, `a_late_gone_answer_settles_gone_with_the_loads_words` (runtime-gone.test.ts:183,190) |
| 10, 11 | scenes `a_held_record_*` (3), `a_record_not_held_*` |
| 12g | scenes `phase_<stored>_provider_<answer>` (35) |
| 12 a-f, h-j | scenes `a_queued_spawn_*`, `a_released_record_*`, `a_sub_whose_cloud_left_*`, `a_kind_with_no_module_*`, `a_desktop_golden_*`, `a_folder_record_*`, `an_image_that_does_not_read_*`, `a_nap_the_provider_read_*`, `the_same_napped_at_*`, `a_nap_clears_*`, `a_stream_url_*`, `a_running_record_that_napped_or_woke_*`, `a_found_machine_*` |
