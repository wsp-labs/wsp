# runtime_of: the public runtime, the sweep and the close

runtime.ts:1017-1277 and index.ts at `65f6b89c7`. Rust: `src/runtime/public.rs` (`AssembledRuntime`, which
implements wsp-core's `Runtime` and this crate's `ServedRuntime`), `src/lib.rs` (index.ts's re-exports). Tests:
`tests/runtime_of.rs`, over `PublicCtx` (the calls `runtimeOf` makes on the core and the areas), faked.

## The public members (runtime.ts:1022-1104)

- U1. `events` is the bus; `backend`, `workspaces`, `projects`, `sessions`, `slates`, `golden`, `image`,
  `preferences`, `usage`, `conversations`, `activity` are the areas' own; `devices` the core's device door;
  `places` only where a place door was built; `recipes` only where `opts.recipes` was given.
- U2. `recipes.options(folders)`: the shelf's options, then this computer's homes (a failure reads as none), then
  each agent kept where this computer's login serves it; `servable_options(options, vault() or {}, kept)`.
- U3. `here_channel(on_event)`: this computer's own channel (`channel_over(local_road(), THIS_COMPUTER, ..)`), held
  to the owner of `HERE_PLACE_ID`, scoped to this computer's process scopes, kill-guarded as this computer.
  `place_kills(channel)` kill-guards a joined computer's channel.
- U4. `watch_place_sys(place, f)`: another place with no place door fails `no_such_place_refusal(place, [])`, else
  the door's; this computer with no sampler fails `this host reads nothing of the computer it runs on`.
- U5. `status.list(o, origin)`: the probes run first; which rows this caller is served is read after them, off
  `listed_for(origin)`, so a record dropped while the probes ran leaves both lists. Order is the probe list's.
- U6. `status.history(id, origin)`: `refuse_named(id, origin)` first (an id this host no longer holds is still
  answered), then the history.
- U7. `harnesses.list(workspace, origin, at)`: the entry is the workspace's (read with the origin unless the caller
  talks to that tree), else the project's home, else the computer's, else none. Each catalog:
  `shape_models(defaults_on(c, prefs, project).catalog, own.models)`, then `unshaped` (`models`, `legacyModels`?,
  `efforts`, `permissionModes`) only where the person set this agent's defaults or the project's, then
  `isDefault: true` only on the default agent.
- U8. `owner()` waits on `ready`, then answers the state's owner.

## The sweep (`reap`, runtime.ts:1105-1216)

- U9. A `say` given becomes the state's `say_stops`. Waits on `ready`, refreshes the builders, then expires the
  grace; a grace that fails adds `grace sweep: <message>` and reaps nothing for it.
- U10. Builders, in the order held: an own one gets its heartbeat. Its age is the wall clock less its
  `CREATED_AT_LABEL` (else its record's `createdAt`); an unreadable age is none. A stale one, and a reusable one
  with no age or an age of at least `BUILDER_IDLE_MS`, is stopped at its place (the wired place where it names
  none); a stop that fails adds `{id, message: "could not stop: <message>; stays recorded, retried next sweep"}`
  and keeps it; a stopped one is forgotten and reaped as `{id, builder: true, reason: "unfinished"}` (stale and
  still building), `"recorded"` (stale), or `"expired"` with `ageMs` where known.
- U11. Lost creates are asked again before any listing; a failure adds `asking lost creates again: <message>`.
- U12. One listing per place: the wired one first, then every other in `places.list()` order. The wired one
  failing adds its message and the sweep answers at once with nothing reaped and nothing spared (the builders and
  grace reaped so far stay). Another failing adds `<place name>: <message>` and the sweep goes on without it.
- U13. Every live record not creating, napping, waking or deleting: an absent machine is re-read (`sweep`); one
  the listings carry under a record marked gone is recovered; a gone record not listed is left; any other not
  listed is read once, and a read the provider answers missing adopts it gone with
  `gone_words(machineId, {by: "sweep", at: now, answer?})`; any other read failure leaves it.
- U14. The ids claimed (every record's machine ids, the builders, the in-flight creates, every id reaped so far and
  the gone watch's) are read once; each listing adopts its lost machines; then each listing is reaped by the
  engine (`stop` through the gone watch, `knownIds` the claimed ids read again with the claimed set), then the
  strays named after this host's images are swept (a failure adds `machines named after this host's images:
  <message>`). A failure anywhere in that stretch adds its message and answers the adopted ones with nothing more.
- U15. The answer: `{reaped, spared, failed?, adopted?}`, `failed` and `adopted` only when not empty, `reaped` the
  builders and grace first, then the replays, the adoptions, the engine's and the strays.

## The close (runtime.ts:1217-1275)

- U16. In order: `closing` set; the roots recheck cleared; every write held for the close run once and cleared;
  the copies moving awaited; the sweep stopped and its timer cancelled, the sweep in flight awaited; the disk
  watch stopped; idle, alerts and slates closed; the agents reader closed; every daemon sync, kept write and agent
  ask settled; every kept agent reaped at once and its exit awaited, at most `KEPT_CLOSE_WAIT_MS`; every machine
  reading stopped and cleared; the pipes closed; the local wiring closed; the place door closed; `closed` set; the
  heartbeat cancelled; the grace, snooze and resume timers cancelled and cleared; every pull request poll stopped;
  the gone watch closed; the tick awaited; every own builder holding a heartbeat written back without it; every
  transcript with a timer or pending events flushed; every transcript write and index flush awaited.
- U17. The turns running on machines are not ended by a close: their readings stop, and a later host re-opens them
  (invariant 1 is the threads area's; this is the half that holds it, by leaving them).

## index.ts

- U18. `lib.rs` re-exports what index.ts exports that this crate owns (`create_runtime`, `serve_runtime` and their
  types); the rest of index.ts's list are wsp-core's and the engine's own crates, read from there. The
  `LOOPBACK` and `WS_PATH` re-exports point at wsp-words' and wsp-wire's (serve.ts:137).

## Rules against tests (`tests/runtime_of.rs`)

| Rule | Tests | TS case |
|---|---|---|
| U1 | `places_and_recipes_only_where_wired` | runtime.ts:1030-1044 |
| U2 | `recipe_options_keep_agents_this_computer_serves` | places-settings.test.ts |
| U4 | `watch_place_sys_refusals` | serve-sys.test.ts:204 |
| U5 | `status_list_filters_after_the_probes`, `a_record_dropped_mid_probe_leaves_the_list` | status.test.ts |
| U6 | `status_history_refuses_off_the_named_rule` | turn-costs.test.ts |
| U7 | `harnesses_list_marks_the_default_and_unshaped_only_where_set`, `harnesses_list_reads_a_project_or_computer_home` | thread-defaults.test.ts, runtime-history.test.ts |
| U8 | `owner_waits_on_ready` | runtime.ts:1101 |
| U9-U15 | `reap_order_grace_builders_replays_listings`, `reap_a_stale_builder_reasons`, `reap_a_reusable_builder_expires_at_six_hours`, `reap_a_builder_stop_that_fails_stays_recorded`, `reap_the_wired_listing_failing_answers_at_once`, `reap_another_listing_failing_goes_on`, `reap_a_listed_gone_record_is_recovered`, `reap_an_unlisted_missing_machine_is_adopted_gone`, `reap_answer_omits_empty_failed_and_adopted` | runtime.test.ts:444, runtime-builders.test.ts, sweep-places.test.ts, reconcile.test.ts |
| U16, U17 | `close_runs_its_steps_in_order`, `close_waits_at_most_the_kept_wait_for_a_kept_agent`, `close_frees_own_builders_heartbeats`, `close_leaves_machine_turns_running` | runtime-comeback.test.ts, thread-keeps-process.test.ts, runtime-builders.test.ts |
| U18 | `lib_re_exports_the_entry_points` | index.ts |
