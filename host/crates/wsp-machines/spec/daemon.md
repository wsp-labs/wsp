# daemon
Source: `packages/runtime/src/machines/daemon.ts` (363 lines) at `65f6b89c7`. Tests: integration cases in
`runtime-reach.test.ts`, `runtime-comeback.test.ts`, `runtime.test.ts`, `sub-queue.test.ts` and the host's
`deploy-token`, `doctor` and `place-machines.runtime` suites (listed under "Rules against tests"), and 41 scenes in
`oracle/daemon.oracle.ts`, which drive the real TS area with a fake context and write every call, event, warning and
record in order to `tests/fixtures/oracle/daemon.json`. Rust: `src/machines/daemon.rs`.

## What it does
The daemon area: what the runtime settles with a machine's daemon whenever it can reach it (the roots file of folders it
may browse, then replacing a daemon older than this build), the revive of a daemon that stopped answering, the hour a
machine that said it lacks something is left alone, and two ways to send a message without waiting for its turn.

## Behaviour
1. `send_detached(ws, {prompt, thread?}, origin)` (:14): mints a `requestId` (the `Ids` trait, a UUID), listens for
   `session.queued` with that id **before** calling `sessions_api.start(ws, {prompt, thread?, requestId, onHeld}, origin)`,
   and answers whichever comes first: the start's own answer (`{outcome: "steered" | "started", threadId: view.threadId
   ?? view.id, harness}`, any outcome but `steered` reading `started`), the queued event (`{outcome: "queued", threadId:
   o.thread, harness: <event's>}`), or `onHeld` (`{outcome: "held", threadId: view.threadId ?? view.id, harness,
   capped: wait}`). A start that rejects first rejects the send. After a queued or held answer, a later rejection of
   the start only warns `a message waiting in thread <threadWord(thread ?? "")> was not sent: <why>`. The listener is
   removed in every case (`finally`).
2. `to_first_thread(ws, prompt, origin)` (:48): of the rows on that workspace with a `threadId`, the one with the least
   `startedAt` (absent reads 0), ties to the earlier in `sessions` order (a stable sort); none sends with no thread.
3. `note_daemon(entry, note)` (:54) sets or clears `daemon_notes[ws]` then awaits `push_status(entry)`;
   `flash_daemon(entry, note)` (:63) sets it, pushes, and clears it with no second push.
4. `roots_write(machine, work)` (:72) is a chain per machine id (03, "chain", first form): each write starts after the
   last settled, its caller gets its own result or rejection, the tail swallows failures, and the key is dropped once
   the tail it holds settled and is still the newest.
5. `write_daemon_roots(entry, strict = false)` (:88), inside `roots_write(entry.record.machineId)`: a closed host
   (`state.closed`) writes nothing; a workspace served by its computer that is not a folder kind writes nothing; else
   the dests are, over every live entry with the same `record.machineId` in `live` order, `[project path,
   checkout_of(record), ...record.landed]`, deduped keeping the first; then `module_of(kind).roots(entry, dests)`.
   `strict` hands a failure back; otherwise it warns `browsable folders for <ws> not written on <machine.id>: <message
   cut to its last 200 UTF-16 units>` and resolves.
6. `turn_runs(ws)` (:111) counts any `running` row, a queued one too; `turn_on_machine` (:114) leaves out rows with
   `queued`. `when_no_turn_runs(ws)` resolves at once with no turn on the machine, else on the first `session.done` or
   `session.end` (any workspace) after which none is; it listens to those two types only.
7. `can_deploy_daemon(entry)` (:135): not served by its computer, the kind's module has `deploy_daemon`, and
   `lands_bytes(backend_for(record).capabilities, machine)` (signed urls or the machine's own byte road).
8. `deploy_daemon_on(entry, deploy)` (:147): success clears `record.daemonRefusedAt` (persisting only if it was set) and
   answers the deploy's detail. A failure marked "lacks" (`machine_lacks_line`) sets `daemonRefusedAt = {machineId:
   machine.id, at: ISO(clock.now()), why: <line>}` and persists; one marked "never answered" changes nothing; any other
   clears the mark as success does. The failure is then rethrown.
9. `lacks_said(entry)` (:174): the record's mark when it names the entry's current machine; a mark of another machine is
   deleted, the record persisted, and `None` answered.
10. `sync_daemon(entry)` (:190), joined per `machine.id` (03, "join"; the key is dropped when this run settles and is
    still the one held). Order, each step pinned by the scenes:
    a. served by its computer: a folder kind writes roots; either way the sync ends.
    b. `placing = !module.has_daemon(entry)` is read **before** anything else (the scene log shows `hasDaemon` first).
    c. `lacks_said`; a mark under `DAEMON_LACKS_AGAIN_MS` (3,600,000) old ends the sync with nothing written.
    d. `write_daemon_roots(entry)`; then no deploy road ends it.
    e. `module.daemon_version(entry)`; `None` sets `unread_at[ws] = {machineId, at: now}`, warns `daemon on <m>
       (workspace <ws>): version not read within <daemonHelloTimeoutMs / 1000> s; asking again at the next reach
       probe` and ends; a version read deletes `unread_at[ws]`; a rejection rejects the sync.
    f. `version >= DAEMON_VERSION` ends it.
    g. a turn on the machine warns `daemon on <m> (workspace <ws>): update waits for the running turn`; then
       `when_no_turn_runs`; a phase not `running` ends it with no note.
    h. `note_daemon(installing | updating)`; loop `while turn_on_machine: when_no_turn_runs`; a phase not `running` now
       clears the note and ends.
    i. `deploy_daemon_on(entry, module.deploy_daemon)`, `write_daemon_roots(entry)`, clear the note. A failure clears the
       note; a phase not `running` ends silently; else warns `daemon on <m> (workspace <ws>) not installed|not updated:
       <why>` and flashes `machine_lacks_line(e)` or `DAEMON_INSTALL_FAILED | DAEMON_UPDATE_FAILED`. The sync resolves.
11. `daemon_settled(entry)` (:262): no sync held for the machine, or a turn on the machine, resolves at once; else the
    sync with its failure swallowed.
12. `unreached_of(entry)` (:274): the line of the mark on the entry's own machine; a mark of another machine is deleted.
13. `revive_daemon(entry, reach)` (:292), synchronous, starts work and returns: only for `no-daemon` on a `running`
    record that can deploy and has no `unreached_of` line; one revival per machine at a time (join map, dropped as the
    sync's is); not again within `DAEMON_REVIVE_AGAIN_MS` (300,000) for the same machine, counted from the last start
    (`now - at < window` refuses, equal passes). It sets `revived_at` before the work starts. The work: note
    `DAEMON_RESTARTING`; a record no longer running clears and ends; `deploy_daemon_on`, roots, clear; a failure clears,
    ends silently if no longer running, else warns `daemon on <m> (workspace <ws>) not restarted: <why>` and flashes
    `DAEMON_RESTART_FAILED`. It waits out no turn.
14. `offer_daemon_again(entry, polled)` (:343): a record with `daemonRefusedAt` and a status carrying `facts` starts a
    sync (detached); nothing else.
15. `read_version_again(entry, polled)` (:349): no `unread_at` mark does nothing; a mark of another machine is deleted;
    else a sync starts only when `polled.phase == "running"`, `polled.reach.state == "reachable"` and the mark is at
    least `DAEMON_REVIVE_AGAIN_MS` old.

## Edge cases
- `write_daemon_roots` for an entry already dropped from `live` still writes, from the other entries on its machine
  (a record gone from the machine is left out, :86 doc).
- A deploy hung by the scene: a second `sync_daemon` answers the same future; `daemon_syncs` holds one key; after it
  ends the map is empty (scene "two at once on one machine").
- A daemon refusal of the old machine is deleted and persisted before the roots write (scene "another machine's").

## Behaviour nobody wrote down
- Rule 3.9: `roots_write` is a chain whose caller sees its own rejection; `sync_daemon` and the revival are joins whose
  map key is cleared only by the run it holds.
- Rule 3.3: `module.roots`, `deploy_daemon`, `push_status` and `persist` are calls out; no borrow of `live`,
  `sessions` or the note maps is held across them.
- Rule 8.3: the 200-unit cut of a roots failure counts UTF-16 units.
- Restart: `daemonRefusedAt` is on the record and survives; `daemon_notes`, `unread_at`, `revived_at`, `unreached`,
  the join and chain maps are memory only and start empty.

## Tapes
None directly; `workspaces.daemon` and the status poll reach it in their units.

## Decisions
1. `deploy` and the module calls are trait methods of the kind module (`KindModule` in wsp-core); a missing
   `deploy_daemon` is `None`.
2. The wait in `when_no_turn_runs` is a bus listener pair, removed when it resolves, as the TS removes it.

## After the port
- `send_detached`'s queued answer trusts `o.thread!`: a queued start with no thread answers `threadId: undefined`,
  which the wire then drops.
- `turn_runs` counts a queued row and `turn_on_machine` does not; the two readers can disagree for a held spawn.

## Rules against tests
| Rule | Tests (`tests/daemon_scenes.rs` replays a scene by name; `tests/daemon.rs`) |
|---|---|
| 1 | scenes `send_detached_started`, `_steered`, `_queued_answers_at_the_event_with_this_requests_id`, `_held_answers_the_wait`, `_a_refused_start_rejects` |
| 2 | scenes `to_first_thread_*` (3) |
| 3, 14, 15 | scenes `offer_again_only_with_a_refusal_and_facts`, `read_again_after_the_window_reachable_and_running_same_machine` |
| 4, 5 | scenes `roots_*` (4); `roots_writes_of_one_machine_land_in_call_order` |
| 6, 11 | scenes `sync_waits_out_a_running_turn_then_deploys`, `sync_a_queued_row_is_no_turn`, `turn_runs_counts_a_queued_row_daemon_settled_does_not_wait_beside_a_turn` |
| 7 | scenes `sync_no_road_to_deploy_asks_no_version`, `sync_no_byte_road_and_no_signed_urls_asks_no_version`, `sync_a_byte_road_with_no_signed_urls_deploys` |
| 8, 9 | scenes `sync_a_deploy_refused_for_what_the_machine_lacks_*`, `sync_a_deploy_the_machine_never_answered_*`, `sync_any_other_deploy_failure_*`, `sync_another_machines_refusal_*`, `sync_the_hour_out_*`, `sync_a_machine_that_said_it_lacks_something_inside_the_hour_*` |
| 10 | every `sync_*` scene (17) |
| 12, 13 | scenes `revive_*` (4), `unreached_of_a_replaced_machines_mark_is_dropped` |
| restart: the refusal survives | `a_lacks_refusal_on_the_record_holds_the_sync_after_a_restart` (runtime-reach.test.ts, the hour kept across a host) |
