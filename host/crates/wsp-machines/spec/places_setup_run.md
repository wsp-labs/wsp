# places_setup_run: one run of a computer's setup or sync

Source: `setup.ts:79-111` (`ROW_FOLLOWS`, `row_step`, `signs_in_again`), `:246-372` (the run's frame: rows, applied,
push, lines, steps, landing), `:746-765` (`end`), `:808-922` (the order, the graph, the catch). Module
`places::setup::run` (`SetupRun`, built by the area for one run; the sign-ins and the folders are its other two
files). Shared rules: `places-a-README.md`. TS cases: `test/places-setup.test.ts` groups "a computer added with its
picks" (:646-1180), "a host that stops in the middle of a setup" (:1265-1396), "one row of a setup that ended, run
again" (:1535-1631), and the sync group's ownership cases (:2124-2217), at this unit's seam: `start_setup` or
`sync_place` on the area with the fake provisioner, fake sign-ins and a fake link (`tests/common/places.rs`).

## Inputs

`run_setup(place, add_id, picks, done: steps, started: PlaceSetup, rerun: waits, sync: Option<SyncJob>, only:
Option<OnlyRow>)`. The record must exist (the area checked); `home` is its `HOME`; `machine = PlaceMachine(link_to(place),
{ id: record.name, home })`.

- **Header** of the log: `wsp {host_name} {set up|synced} {record name} {from|to} {picks.name} at {started.startedAt
  for a setup | now for a sync}`.
- **resolved** = the sync job, else (no recipe or `NO_RECIPE`) `recipes.resolve_file(picks)`, else
  `recipes.resolve(recipe)`; a failure or no shelf is none. `hash = resolved.hash` else `picks_hash(picks)`.
- **before** = the applied rows. **put** = the engine's rows (`engine_row`) that read `installed`; **made** = the
  `folders` rows with a project. **carried** (a setup, not a sync) = put and made whose step is not in `done`, each
  marked `earlier: true`: what an earlier run put on stays wsp's through a resume and through a step that threw.
- **rows** = a sync: every row of before. A setup: before's rows that stand (`done` holds their step, or a
  `signins` row whose agent `signs_in_again` is false), then carried.
- **ours(got)**: a `present` row under an id `put` holds reads `installed` with `earlier: true`.

## The record as the run writes it

- `applied()`: `hash` and `items` from the record's applied (hash `""` where none) while a sync has not ended; else
  `hash` and `resolved.items`. `at` = now. `rows` = one per id, at the place of its first, holding its last value
  (a `Map` from the rows); a `failed` row with no `fix` gets `setup_row_fix(row, record name)`. Keys `hash, at,
  rows, items?`.
- `push(next)`: `held = next`, then one `write_setup(setup: next, applied: applied())`, chained after every earlier
  write, its failure ignored. A frame said "after the write" is chained on the same chain: a client reading the
  record on a frame never reads it behind the frame.
- `line(l)`: the running set moves; a setup pushes `held` with that step's line replaced in place or appended; a
  sync pushes `held` as it stands (the setup's lines are not the sync's). Log line `[{step}] {SETUP_STEP_WORDS}:
  {state}` plus ` ({note})` where noted. Frame: a sync's `place.sync { placeId, sync: job state, line }` at once; a
  setup's `place.setup { addId, placeId, line, running: [steps running now] }` after the write.
- `step(name, work)` (:330): passed over (answering none) where `done` holds it or a sync's steps do not. Else a
  running line stamped `startedAt`, the work, each row stamped `step: name`, and that step's carried rows dropped.
  A work that fails with `PlaceAbsent` passes the failure up; any other failure is one row `{ id: "{name}/stopped",
  label: SETUP_STEP_WORDS[name], outcome: failed, step, note: first line }` and the carried rows stay. Then a `done`
  or `failed` line with `ms = max(0, round(now - began))` and, where any failed, `note: "{n} of {m} failed"`.
- `land_row(r)` (:352): the row under `r.id` goes and `r` is appended; pushed; a frame `{ addId, placeId, landed:
  r.id }` after the write, carrying `outcome()` too where the steps ended, no wait stands and no folder waits on
  GitHub; and then (a setup) the run lets go of the place's lander.
- **The lander** (`landers[place]`, :361) lands a row from outside the run: `landed_on(rows, r)`'s row under that id,
  else false. It answers true only once that row's write is on the record. It is let go at the end where nothing
  waits, at the last landing that clears the waits, and in the `finally` of a run that never ended.

## The order (:808-922)

1. A GitHub pick signed in from the vault with no `GH_TOKEN` held asks `wiring.github_token()` first, its failure
   ignored. Then the sync's removals (`places_setup_folders`), then the plan: `provisioner.setup(picks, { home,
   stores?, recipe: picks.name where the record follows a recipe }, sync's steps)`. A sync keeps only the plugins it
   moved.
2. `plan = cli_first(planned, [gh's row where gh is a picked CLI, every CLI with a hook, the server tools])`.
3. Each wait in `rerun` runs again first: GitHub's sign-in, or the agent's (`places_setup_sign_ins`).
4. `floor`; a failed floor ends the run `failed` with `floor_failed_line(rows)` and nothing after it runs.
5. The graph (`run_graph`, wsp-core), width `setup_width(report.shape.memMb)`, the steps `done` holds left out:

   | Step | After | Lanes | Notes |
   |---|---|---|---|
   | agents | | installs | every agent row failed: the run stops with `no_agent_line(rows)` |
   | signins | agents | none, light | |
   | skills | | files | |
   | github | `gh` mark where gh is a CLI | installs where the plan has a GitHub step | |
   | clis | agents | installs | releases marks `gh`, `hooks`, `servers` as their rows are said; an empty set releases at once |
   | mcp | agents, `servers` | files | sets whether the agents' files are there |
   | configs | | installs and files where config tools exist, else files | |
   | plugins | mcp | none | |
   | folders | github, `hooks` | none | |
   | folderServers | mcp, plugins, folders | files | |

6. `context`, then the end. A stop ends `failed` with its line.
7. **The catch**: `PlaceAbsent`, or a sync that failed before any step started: nothing more is written for a setup
   (it stays running on the record and resumes at the next link); a sync is written `behind` and said. Any other
   failure ends the run `failed` with its first line. **Finally**: the steps read ended, a run that never ended lets
   go of its lander, and the log closes with the setup as held.

## `end(failed?)` (:746)

A sync: its rows written with `picks` and `sync` removed, the lander let go, `place.sync { placeId, applied:
applied_view }`. A setup: `state` done or failed, `finishedAt`, `said` where failed; once written, the lander goes
where nothing waits; a done run with folders still waiting on GitHub says nothing yet (the last folder says the end);
else `place.setup { addId, placeId, end, said? }` from `setup_outcome` or `{ end: failed, said }`.

## Pins

| Rule | Test | TS case |
|---|---|---|
| order of ran steps and lines; rows on the record; needs-you then ready | `runs_the_floor_and_agents_first_and_writes_every_step` | :647 |
| a late sign-in said with its row while running | `a_sign_in_landing_after_its_step_is_said_with_its_row` | :693 |
| frames only after the record holds them | `a_frame_is_said_only_once_the_record_holds_it` | :711 |
| running line stamped startedAt on record and frame | `a_running_step_is_stamped_with_when_it_started` | :733 |
| a failed floor stops everything | `a_failed_floor_stops_the_run_and_says_why` | :860 |
| every agent failed stops; one ok goes on | `every_agent_failed_stops_and_one_installed_goes_on` | :872 |
| a failed CLI stands with its fix, ready, n rows did not install | `a_failed_cli_row_stands_and_the_computer_reads_ready` | :882 |
| width by memory, two lanes | `runs_as_wide_as_the_memory_allows` | :922 |
| one at a time under 4 GB | `runs_one_step_at_a_time_on_a_small_box` | :938 |
| marks: github after gh's row, servers after theirs, plan order | `never_starts_a_step_before_what_it_needs` | :950 |
| github held while clis runs and gh has not landed | `holds_github_until_gh_is_said` | :1015 |
| folders held until a hook's row | `holds_the_folders_until_the_hook_row_is_said` | :1030 |
| servers and plugins held until the server tool | `holds_the_servers_until_their_tool_is_on` | :1052 |
| the log lines carry their step | `each_log_line_names_its_step` | :1148 |
| resume redoes no step that ended | `a_resume_redoes_no_ended_step` | :1290 |
| a step that throws is one stopped row; carried rows stay | `a_step_that_throws_is_one_failed_row_and_keeps_what_earlier_runs_put_on` | :2158 |
| an absent computer leaves the setup running | `a_computer_that_went_away_leaves_the_setup_running` | :2136 |
| a tool wsp put on stays wsp's through a run again | `a_tool_wsp_put_on_stays_wsps_through_a_run_again` | :2124 |
| a sync's failed start reads behind | `a_sync_that_failed_before_any_step_reads_behind` | :2419 |
| a sync cut partway keeps the old hash | `a_sync_cut_partway_holds_the_hash_it_started_from` | :2465 |
| the lander lands among the run's rows and survives its next write | `a_sign_in_landed_while_the_run_writes_survives_its_next_write` | :1840 |
| a frame listener that throws does not stop the run | `a_frame_listener_that_throws_does_not_stop_the_run` | :1682 |
| applied rows one per id, fix stamped on a failed row | `applied_keeps_one_row_per_id_and_stamps_a_fix` | :904 |
| retry of one row runs only its steps | `a_row_retry_runs_only_its_steps_and_keeps_the_rest` | :1536 |
| a stopped setup's never reached steps run on a retry | `a_retry_runs_the_steps_a_stopped_setup_never_reached` | :1594 |
