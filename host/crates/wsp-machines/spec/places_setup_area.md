# places_setup_area: the waits for a link, the link a machine is driven over, the start and the sync

Source: `setup.ts:112-245` and `:924-1093`. Module `places::setup` (the area struct `PlaceSetupArea`; the run is
`setup/run.rs`, specs `places_setup_run.md`, `..._sign_ins.md`, `..._folders.md`). Shared rules:
`places-a-README.md`. TS cases from `test/places-setup.test.ts` (the start, retry, sync and resume groups) and
`test/places-image.test.ts:204` (a frame with a key waits out a gap), at this unit's seam: a fake records area
(`SetupRecords` over a memory map), a fake link per place (`tests/common/places.rs`), a fake provisioner, and the
run itself (`setup_run`) behind `start_setup` and `sync_place`.

## The records it reads (`SetupRecords`, slice b implements)

`record_of(place) -> Option<PlaceRecord>`, `change(place, move) -> Option<PlaceRecord>` (one at a time per place,
`move` from the record as it stands; `None` where `move` answered `None` or the record is gone), `login_listed(place,
agent)`.

## State, memory only

`channels: IndexMap<place, IndexMap<id, reader>>`, `waiting: IndexMap<place, IndexMap<id, wake>>`, `closed_at:
IndexMap<place, ms>`, `setting: IndexSet<place>`, `syncing: IndexSet<place>`, `signing_in: IndexMap<"place/agent",
{ mute, leave }>`, `skippers: IndexMap<"place/row", async fn>`, `landers: IndexMap<place, async fn(LandedRow) ->
bool>`, `again: IndexSet<place>`, `sync_timers: IndexMap<place, Cancel>`.

## The waits

- `woken(place, back)` (:131): takes `waiting[place]` out whole, then calls each wake with `back`, in order.
- `dials_back(place, until) -> bool` (:140): false at once where a live link's socket is open (a closing socket is
  not up: the wait waits for the next one), or where `until - now <= 0`. Else a wake joins `waiting[place]` and a
  timer at `until - now` wakes it false. Woken either way, it removes itself from the set it joined and cancels its
  timer. A wake woken after `woken` took the set does not touch the newer set.
- `setting_now(record) -> Option<String>` (:185), the one busy reading: `place_provisioning_line(name, the running
  step's name if any)` where `setting` holds the place or `record.setup.state == running` (a setup a stopped host
  left), else `None`.
- `folders_of`: the place whose setup's own folder add the current call runs under (TS `AsyncLocalStorage`, here a
  tokio task-local scoped over the add's future). `current_folders_of() -> Option<String>`; `None` outside.
- `write_setup(place, patch)` (:193): one `change` that sets each field the patch carries over the record as it
  stands; `sync: Some(None)` removes `sync`. A field another writer set meanwhile survives.
- `recipes_moved(slug)`, `sync_frame(e)`, `setup_frame(e)`: `opts.on_setup` with `recipes.changed { slug }`,
  `place.sync { ... }`, `place.setup { ... }`; nothing where `on_setup` is not wired.

## The job's log on the computer (`provision_record`, :213)

`log = provision_record(machine, home, header)`. Paths: `place_provision_paths(home)` (`.log`, `.result`).
- Every line is stamped as written: `provision_log_line(now, line)`. The header is the first line and waits with
  the rest (no frame of its own).
- `write(line)`: queued; at 50 queued lines a flush at once; else, where no timer stands, one at 2,000 ms.
- `flush()`: cancels the timer, sends every queued line as one append of `lines.join("\n") + "\n"` to the log,
  chained after the previous send; a failed send is ignored; nothing queued sends nothing.
- `close(setup)`: a flush, then the result file written whole as `JSON.stringify(setup, null, 2) + "\n"`, its
  failure ignored. Nothing here fails the job.

## `start_setup(place, add_id, given?, row?) -> Result<{ setup?, said? }>` (:927)

1. No provisioner wired, or no record: `{}`.
2. `setting` holds it: `Err(place_provisioning_line(name, running step), Conflict)`. `syncing` holds it:
   `Err(place_syncing_line(name), Conflict)`.
3. No `HOME`: `{ said: place_no_home_line(name) }`. No picks given or kept: `{ said: place_no_picks_line(name) }`.
4. A row given where the setup ended (not running, present): `only = { step: row_step(record, row), row }`;
   `row_step` is the step by its own name, else the applied row's step, else a waiting row (`github` reads
   `github`, a sign-in `signins`); none is `Err(usage_refusal(no_setup_row_refusal(row, name), "Run again a row the
   computer's page shows."))`. A row given while running or with no setup is ignored: a whole run.
5. `runs = { only.step, ROW_FOLLOWS[only.step], context }` with `ROW_FOLLOWS = { clis: [mcp, folderServers],
   folders: [folderServers], github: [folders, folderServers], agents: [signins] }`.
6. `setting` takes the place, then: `resuming` = the record's setup where nothing was given and it reads running;
   `kept` = the ended setup's lines whose step is not in `runs`, else the resumed setup's `done` lines, else none;
   `done` = their steps; `rerun` = the waits standing on the record whose step is not run again: `github` where
   `done` holds github, a sign-in where `done` holds signins or `signs_in_again(only, agent, rows)` is false.
7. `setup = { state: running, addId, startedAt: resuming's or now, steps: kept, waiting: [] }` written with `picks`,
   and `recipe: given.recipe or NO_RECIPE` only where picks were given; then the run starts without being awaited,
   and `{ setup }` is answered. When the run ends, `setting` lets go, and a place in `again` is synced at once.
   A failure before the run starts lets `setting` go and passes through.
- `started_or_said(...)`: the same with any refusal as `{ said: first_line_of(e) }`.

## The sync (`:967-1038`)

- `sync_of(record, given?)`: `None` with no recipe slug or `NO_RECIPE`, no resolver, no picks, a setup not `done`,
  or a resolve that fails. Same hash as applied: no changes. Else `recipe_changes(picks, applied.items, file, items)`.
- `mark_sync(place, sync)`: nothing with no record or with nothing to clear; else written, and a `place.sync` frame
  (`sync` left off where cleared).
- `sync_place(place)` (:996): a place `setting` or `syncing` holds joins `again` and nothing else happens. Else
  `syncing` takes it until the end: no record or no provisioner, or `sync_of` none: nothing. No changes: where the
  applied hash differs, applied takes the new hash and items and picks the resolved file; then the sync mark is
  cleared. Changes: `state = { running, changes: keys, since: the record's own since or now }`; not linked or no
  `HOME`: marked `behind`, nothing run. Else marked running and the run awaited with the sync job (`steps_for`,
  `moved` = keys not removed, `before` = the record's picks). At the end `syncing` lets go and a place in `again` is
  synced at once.
- `sync_soon(place, ms)`: cancels the place's pending sync and schedules one after `ms` (a burst is one sync); a sync
  that fails is a warning, never a failure.

## `link_to(place) -> Rc<dyn MachineLink>` (:1053)

- `request(op, params, o)`: loop:
  - No live link: `PlaceAbsentError(absent_computer(name or id, null).sentence)` at once, unless the frame has an
    idempotency key, a socket of this place closed (`closed_at`), and `dials_back` until `until` answers true; then
    again. `until` is fixed at the first gap: `closed_at + relink_wait_ms` here, or `now + relink_wait_ms` below.
  - Else `bounded(reach.request(op, params), o.timeout_ms or frame_wait_ms, "{op} on {name or id}")`. A failure is
    passed through where the frame has no key, or where the same link is still live and open (the place answered).
    A different link already live (it dialled back while this failed) and still inside `until`: again at once. Else
    `dials_back(until)`; false passes the failure through, true goes again.
- `dials_back()`: `dials_back(place, now + relink_wait_ms)`. `forward(port)`: `ctx.door().forward(place, port)`.

## Pins

| Rule | Test | TS case |
|---|---|---|
| setting_now is the one busy reading, setting or a running record | `setting_now_is_the_one_busy_reading` | places-setup:1127 |
| woken takes the set; a late wake leaves the newer set | `woken_wakes_every_wait_and_a_late_one_leaves_the_next_set` | |
| dials_back false on an open link, false at once past until, true on a wake, false at the timer | `dials_back_answers_by_the_link_and_the_clock` | |
| a frame with no key fails at once on no link | `a_frame_with_no_key_fails_at_once_on_an_absent_computer` | |
| a keyed frame waits out a gap and goes on the next link, once | `a_keyed_frame_is_sent_again_on_the_link_that_comes_back` | places-image:204 |
| a keyed frame past the wait fails with its own words, the wait counted from the close | `a_keyed_frame_fails_with_its_own_words_once_the_wait_runs_out` | places-image:204 |
| a place that never closed a socket is refused at once, key or not | `a_keyed_frame_to_a_computer_never_here_is_refused_at_once` | |
| a refusal on the live open link passes through, keyed or not | `the_places_own_refusal_is_never_waited_out` | |
| bound and its words | `a_frame_is_bounded_by_its_own_timeout_or_the_frame_wait` | |
| the log: stamped lines, 50 at once, 2 s batches, header first, result file | `the_log_goes_up_in_batches_every_line_stamped`, `the_log_close_writes_the_result_beside_it` | places-setup:1165 |
| a failed log write fails nothing | `a_log_the_computer_will_not_take_fails_nothing` | |
| write_setup keeps another writer's field; sync None removes it | `write_setup_keeps_a_field_another_writer_set_meanwhile` | invariant 6 |
| start: no provisioner, no record, busy, syncing, no home, no picks | `start_refuses_or_says_in_the_ts_order` | places-setup:1196, :1127 |
| start writes the running setup and picks, recipe only where given | `start_writes_the_running_setup_before_the_run` | places-setup:647 |
| resume keeps done lines and startedAt, reruns waits whose step ended | `a_resume_keeps_the_ended_steps_and_asks_again_for_a_waiting_sign_in` | places-setup:1290 |
| a row of an ended setup runs its step, its followers and context | `a_row_retry_runs_its_step_its_followers_and_the_context` | places-setup:1536, :1608 |
| an unknown row refused | `a_row_the_setup_holds_nothing_under_is_refused` | places-setup:1622 |
| setting lets go at the end; again syncs | `the_end_of_a_run_lets_the_computer_go_and_syncs_a_change_that_waited` | places-setup:2406 |
| started_or_said turns a refusal into words | `started_or_said_says_a_refusal` | |
| sync_of none cases; same hash no changes | `sync_of_reads_nothing_where_nothing_is_followed` | places-setup:2302 |
| in step: hash moved, picks follow, mark cleared, nothing run | `a_recipe_that_moved_no_row_runs_nothing` | places-setup:2277 |
| away: behind, nothing run; dial back catches up once | `a_computer_away_reads_behind` | places-setup:2318 |
| a sync during a setup or a sync waits and runs once at its end | `a_change_mid_sync_runs_once_more_at_its_end` | places-setup:2406 |
| sync_soon: a burst is one sync | `sync_soon_folds_a_burst_into_one` | |
| a mark with nothing to clear writes nothing | `mark_sync_writes_nothing_with_nothing_to_clear` | |
