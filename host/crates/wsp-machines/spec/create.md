# create
Source: `packages/runtime/src/machines/create.ts` (294 lines) at `65f6b89c7`. Tests: integration cases through
`workspaces.create` in `runtime-golden-update.test.ts`, `spawn-guard.test.ts`, `runtime.test.ts` and the host's
`mcp-record` suite (under "Rules against tests"), and 25 scenes plus a table of small facts in
`oracle/create.oracle.ts` (the landing road, the branch of record and the engine's kill are recorded fakes; `Date` is
pinned to the scene clock), written to `tests/fixtures/oracle/create.json`. Rust: `src/machines/create.rs`.

## What it does
The create area: `create_staged` makes one workspace record on the project's computer, forks its machine (stopping a
kept image builder when the provider is at its machine cap), asks the route and the daemon, reads the memory, lands the
project and writes the record, reporting each stage. Around it: the name rules, the failed and shown creates the windows
list, the last stage of each create in flight, and the join that makes one folder record per key.

## Behaviour
1. `create_staged(o, project, id, report, spawned?, landed?, child_base?)` (:13), in order:
   a. `landing_place(project.computer)`; `place_refuses(placeId)`; `at = landing_backend(placeId)`;
      `road = project_landing(landing_kind(project.computer, at))`; `deps = landing_deps(project.computer).deps`.
   b. `road.refusal(project, deps)`: a sentence throws it with kind `invalid` before anything is written (:38).
   c. `said = road.places({project, memoryKey, deps})`; a `memoryDir` that differs from the project's is remembered
      with `remember_project({...project, memoryDir})` (:40). `binds = road.workspace_binds(project_held(project.id))`.
   d. `copy = {from: project.checkout, at: project.path}` when the project has a checkout.
   e. `from_image = keeps_images(at)`; with it, `golden = copy_for_fork(o.golden ?? image_head(), placeId, building)`,
      where `building(where, rate)` reports `fork-requested` with `copy_first_line(where, o.name, rate)`; without it
      `golden = ""` and no image is read. `inherited = image_of(golden).version.size` only with an image.
   f. `brokered = brokered_start(ctx, road.kind == "provider")`; `carried = child_base.is_none() && checkout.is_some()`;
      `branchOfRecord = o.branchOfRecord ?? named_of_record({remote, name: o.name, project: project.name, on:
      place_name(placeId ?? places.wired), base: child_base ?? project.base, carried})` only when brokered.
   g. `opensThread = o.opensThread ?? (brokered && spawned.is_none() ? Ids::uuid() : None)`.
   h. The record, keys in this order (rule 4.26, fixture `persist` rows): `id, name, kind: "cloud", machineId: "",
      phase: "running", golden, createdAt: ISO(clock.now()), project: project.id, spec: {envs?, labels?, binds? (only
      non-empty), copy?}, idleWindowMs?, place?, size: {cpu: o.cpu ?? inherited.cpu ?? at.pricing.defaultSize.cpu,
      memMb: likewise}, firstLife: true, ...tree_of(spawned), parentWorkspaceId?, base? (child_base ?? project.base),
      branchOfRecord?, agents?` where `agents = agents_from(spawn_at(placeId ?? places.wired), o.agents)` only for a
      create no thread asked for (`spawned` absent) that named agents.
   i. A size the caller named (`names_size(o)`: cpu or memMb given) that `offered_size(at.capabilities.sizes, size)`
      does not offer throws `refusal_line(size_refusal(size_word(size), sizes), SIZE_PICK_FIX)`, kind `invalid`, with
      nothing forked and nothing reported (fixture: `3x1 is not a size ... Ask for one of those instead.`).
   j. `where = places.wired` with no place, else `place_door_of().name_of(placeId)`; report `fork-requested`
      `starting_line(name, where)`.
   k. `fork(record, bind, None, report, false)`. `bind(m)` sets `record.machineId`, `attach(record, m)`, marks the
      entry `creating`, sets `opensThread` and `ofRecordHeld` (only when `o.branchOfRecord` was given), then calls
      `landed()`, all in one synchronous step (rule 3.4).
   l. A refusal that is not the cap (`kind != "concurrency"`) is thrown as it is. At the cap: `refresh_builders()`,
      then for each builder, in map order, whose life is `own` or `reusable` and whose record is `sealed`: cancel and
      forget its grace timer, `kill_until_gone(backend_at(place ?? wired), machine, opts.killConfirm)`,
      `forget_builder(id)`, push the notice `Stopped the builder kept from image v<n> to make room at the machine cap.`,
      warn `workspace <id>: <notice with first letter lowered and last character dropped> (<builder id>)`, report
      `fork-requested` `<starting line> again` with the notice, and fork again; a success stops the loop; a cap refusal
      moves to the next builder; any other failure is thrown. With none left the create throws
      `machine_cap_refusal(names of live records that hold a slot, names of the builders left, refusal.said)` with kind
      `concurrency`, the last refusal as its cause, and the refusal's numeric `status` when it has one.
   m. `machine.notice` joins the notices.
   n. Not served by its computer, and a machine with a `previewUrl` or `daemonAnswers`: with a preview url,
      `until(ws.daemon_reach(), now + lifecycle.budgets.daemonAnswersMs, "preview route")` reports `preview-route`
      `Preview route to the daemon minted.`, or on failure `No preview route to the daemon.` with notice `preview route
      for <machine id> not minted (<why>)`; then `fault = ping_daemon(entry)`, report `daemon-answering` `Daemon
      answered.` or `Daemon did not answer.` with `{notice: fault}` (a present-but-undefined key: the fixture shows
      `{}`), and start `sync_daemon(entry)` without waiting.
   o. With no place: a fault warns `workspace <id>: memory not read: the daemon did not answer`; else
      `read_memory(record, machine, sizes)` and, when it read and the size moved, the notice and warn
      `size_got_line(asked, record.size, names_size(o))`.
   p. `module_of(kind).land_project(entry, project, report)` (a failure ends the create: the entry stays in `live`
      with `creating` set, and nothing is persisted); `write_daemon_roots(entry)`; `persist(record)`;
      `place_door.mark_used(placeId)`; clear `creating`; report `ready` `ready`; emit `workspace.created` with the view;
      answer the view, with `notice` = the notices joined by one space when there are any.
2. `name_given(name)` (:207) is `js_trim(name)` (NBSP and BOM are trimmed, fixture `nameGiven`).
3. `name_refusal(name)` (:271): a blank name after `name_given` is `BLANK_NAME_REFUSAL`; else the first live entry whose
   record name equals `name` exactly (not trimmed) refuses with `name_deleting_refusal` while it is deleting, else
   `name_taken_refusal`; a name in `forking` refuses as taken; else `None`.
4. `failed_view(...)` (:229): `{id, name, machineId: "", phase: "gone", kind, golden, createdAt: ISO(began), project:
   ref_of(project), gone: said}` in that order.
5. `supersede_failed(name)` (:241): every failed create of that name is removed and `workspace.deleted` emitted for it,
   in map order.
6. `stage_reporter(id, name, began, spawned)` (:251) emits `workspace.creating` `{workspaceId, name, stage, message,
   elapsedMs: now - began, notice?, detail?, waiting: true only when true, askedBy?: {threadId, rootThreadId}}`.
7. The `*` listener (:221): a `workspace.creating` is held by workspace id without its `seq`; `workspace.created`
   drops the held stage of `workspace.id`; `workspace.deleted` drops `workspaceId`'s.
8. `show_create(view)` (:216) holds it in `shown_creates` and emits `workspace.created` with it.
9. `folders_of(project)` (:279): live entries of a folder kind on that project, in `live` order.
10. `one_folder(key, make)` (:283): a join per key; the key is freed when the make settles, success or failure.

## Edge cases
- `name_refusal(" taken")` is `None` though `taken` exists: the comparison is exact (fixture).
- A cap refusal with no sealed own or reusable builder skips the loop and words the holders at once.
- A brokered create with a named `branchOfRecord` reads no branch and keeps `ofRecordHeld`.

## Behaviour nobody wrote down
- Rule 3.4: `bind`'s attach, marks and `landed()` run with no await between; the slot the guard took is handed back in
  the same step the record enters `live`.
- Rule 3.15: `createdAt` is the clock's now; the TS reads `new Date()`.
- Restart: only the persisted record survives; `forking`, `failed_creates`, `shown_creates`, `create_stages` and the
  folder joins are memory only (02, rule 2.17).

## Tapes
`workspaces.create` tapes belong to the projects area that calls this; none here.

## Decisions
1. `project_landing`, `brokered_start` and `named_of_record` are calls through the unit's runtime trait, so tests fake
   them as the oracle does.

2. In the TS the live entry's record and the record `fork` and `read_memory` write are one object (`bind` attaches the
   very record `create_staged` holds). The Rust holds two: `bind` attaches a copy with `machineId` set; `create_staged`
   sets `machineId` on its own record and writes it onto the live entry after the fork and after `read_memory`, before
   anything reads the entry's record or persists, so the entry and the persisted record read as the TS's one object.

## After the port
- A create whose `land_project` fails leaves its entry in `live` marked `creating` for the caller to drop.
- `name_refusal` compares untrimmed names while `name_given` trims.
- `report("daemon-answering", ..., {notice: undefined})` carries an absent notice key.

## Rules against tests
| Rule | Tests (`tests/create_scenes.rs` replays a scene by name; `tests/create.rs`) |
|---|---|
| 1a-1h | scenes `a_provider_fork_lands_and_is_ready`, `an_offered_size_is_kept`, `a_memory_folder_*`, `a_project_with_a_checkout_*`, `a_computer_that_keeps_no_image_*`, `a_named_golden_at_a_place_*`, `a_spawned_fork_*`, `a_brokered_fork_*` (2) |
| 1b, 1i | scenes `the_road_refuses_before_anything_is_written`, `a_size_not_offered_is_refused_before_any_machine` |
| 1k | `bind_attaches_marks_and_hands_the_slot_back_in_one_step` |
| 1l | scenes `at_the_machine_cap_*`, `at_the_cap_with_builders_refused_*`, `at_the_cap_with_nothing_to_stop_*`, `a_fork_that_fails_another_way_*`, `a_refusal_after_a_builder_stopped_*` |
| 1m-1o | scenes `a_route_and_a_daemon_*`, `a_route_that_is_not_minted_*`, `a_daemon_that_does_not_answer_*`, `a_served_workspace_asks_neither`, `memory_*` (3), `a_machine_notice_joins_the_answer` |
| 1p | scene `a_clone_that_fails_ends_the_create_before_it_is_persisted` |
| 2-10 | `name_given_trims_as_javascript_does`, `name_refusal_*`, `failed_view_*`, `supersede_failed_*`, `stage_reporter_*`, `the_stage_listener_*`, `show_create_*`, `folders_of_*`, `one_folder_*` (fixture `facts`) |
