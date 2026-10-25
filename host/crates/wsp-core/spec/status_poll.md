# status_poll
Source: packages/runtime/src/status.ts, the row builder and the poller of `createStatusTracker`: :173-228, :300-314, :334-342, :502-787, :849-889, :925-929 at 65f6b89c7. Tests: status.test.ts "status.list", "a machine that answers for its own daemon", "status.watch provider calls", "the status poll and a machine's uptime", "the status ticks", "status zombie at rest", "a machine the provider answered it cannot reach", "the reach word a row shows". Rust: `src/status/poll.rs`, an `impl` block of `StatusTracker`; tests `tests/status_poll.rs`.

## What it does
Builds a `WorkspaceStatus` row per record: the runtime's phase plus one reach probe, the provider asked only after a failed reach (once per reconcile window) or on an explicit list. A row's reach word turns dark only on the second silence in a row, per reader. A machine slow or unreachable past the zombie window, still running at the provider, earns one exec probe; a failed probe marks it zombie. While a watcher holds it, a poll every 15 s pushes each row that changed.

## Behaviour
1. `list(opts)` (:753) builds rows for `records()` with `opts`; `reconcile` defaults to `always`, `reader` to `app`, `prompt_ms`/`probe_timeout_ms` to the tracker's defaults. Every record is built concurrently on the one thread; the answer keeps the records' order (:648).
2. Per record, first of all (:658): with `facts`, the facts read (Behaviour 13), for every phase. The base row is the record's view, then `size`, `rateUsdPerHour`, `idleAt` where set, `facts` where read (:659); the record's `reason`, `checkout`, `pr`, `tree`, `wake_ask` ride in it.
3. `done(state, reach)` (:661-664): state `gone` calls `on_gone(id, machine_id, gone_reasons[id] or gone_words(machine_id, None))` and the row's `machineState` is `machine_state_of(phase)` (one 404 is not gone); else `state`.
4. Phases, in order (:667-691): `queued`: suspects dropped; `starting`, reach `napping`. `gone`/`released`: suspects dropped; `gone`, reach `gone`, `reason` = the view's `gone` where set. `away` set: suspects dropped; running reads `running`/`unreachable`, any other `paused`/`napping`; `reason` = away; nothing asked. Not running (napping, waking, pausing): suspects dropped; `done(machine_state(r, reconcile, false), napping)`.
5. Running, the road (:692-711): with `daemon_answers`, the route (`daemon_reach`, a failure read as none) then `ask_daemon` with the record's own `daemon_answers_prompt_ms` when set; else with `daemon_reach`, the route and `probe_reach(route.url)`, any failure `missed(e)`; with neither, `done(machine_state(r, reconcile, false), unsupported)`. The row carries `url`, `expiresAt` of a route it got.
6. Offline (:715): the reach word is the reader's last shown (or `unreachable` when `unreached` is set), `offline: true`, the route kept; `done(machine_state(r, reconcile, false), ..)`; reason `unreached` where set. The reader's probe memory, suspects and the zombie window are untouched.
7. Otherwise (:716-719): an answer from the daemon is `saw_awake`; `shown = reach_shown(memory.last, probed.state)`, then `memory.last = probed.state`, `memory.shown = shown`.
8. `unreached` set (:721-731): the suspect is taken (made at now when absent or for another machine); unless `zombie_probe == false` an exec probe is started on it if none is in flight (not awaited here); then `judge(r, {unreachable, route})`; `memory.shown` = the judged word; the row is `done(judged)` with `reason: unreached` only when the judged state is running (a zombie's own reason is not added).
9. A route the reach needed and did not get (no `daemon_answers`, `daemon_reach` failed, :735): `done(machine_state(r, reconcile, true), shown)`, no route on the row.
10. `reachable` or `no-daemon` (:737-743): suspects dropped; `done(machine_state(r, reconcile, !from_daemon), {state: shown, url, expiresAt})`.
11. `slow` or `unreachable` (:745-748): `judge`; the row's word is `zombie` when judged so, else `shown`; `memory.shown` = that word; the judge's reason is added where it gave one.
12. `judge(r, reach, opts)` (:592-624): `elapsed = now - suspect.bad_since` read first. No verdict and inside the window: `machine_state(r, reconcile, reach.state == unreachable)`. Else `provider = machine_state(r, reconcile, true)`; not running: suspects dropped, answer it. A standing verdict: zombie with its reason. `zombie_probe == false`: the reach as it is. Else join the probe in flight or start one; a probe that answered sets `bad_since = now` (the window starts over); a failed one sets the verdict `<machine> reports running at the provider; reach <state> since <iso(bad_since)> (<js_round(elapsed/1000)> s); exec probe "echo ok" <fault>`, warns `zombie on <machine> (workspace <id>): <reason>` once, and answers zombie.
13. `facts_read` (:630-642): a dial record for the same machine whose `next_at` is still ahead skips the read (no facts). A read that answers clears the record. A failure counts a miss and sets `next_at = now + facts_backoff_ms(misses)`; a record for another machine starts from 0.
14. `ask_provider(r)` (:523-538): `provider_state()`; a failure that is not `is_missing` answers `machine_state_of(phase)` and records nothing; missing is `gone` with `provider_said(e)` as the answer. Running first runs the metrics check (15). Then `reconciled[id] = {state, at: now}`; running is `saw_awake`; gone stores `gone_words(machine_id, {by: "status poll", at: now, answer})`.
15. Metrics check (:505-519): no `metrics`, nothing; it answers: the doubt is cleared; a failure that is not missing: nothing; missing: warned once per machine `host metrics for <machine> (workspace <id>) answered <provider_said>; the state read says running and the record follows the state read`.
16. `machine_state(r, reconcile, reach_failed)` (:540-547): `always` asks; no failure gives `machine_state_of(phase)`; a reading for the same state of the phase younger than `reconcile_min_ms` is reused; else ask.
17. `probe_exec(r)` (:550-561): `exec("echo ok", timeout zombie_probe_timeout_ms)` bounded by the same; exit 0 and trimmed stdout `ok` answers none; else `failed after <ms> ms (exit <code>[: <stderr js_trim, first 200 UTF-16 units>])`, or `failed after <ms> ms (<error message>)`. One probe in flight per suspect; it clears itself from the suspect when done.
18. Probe memory (:566-574) per reader per workspace, fresh `{shown: reachable}` for a new machine; suspects (:577-583) per workspace, fresh for a new machine.
19. `watch(opts)` (:867-889): the first watcher starts the cost timer (`opts.cost_interval_ms` or the default) and the poll timer (`opts.poll_interval_ms` or the default), each re-armed **before** its body so slow ticks overlap, the first one interval away, and runs one poll tick at once. Later watchers' options are ignored. The release is idempotent; the last one stops both timers and clears last-emitted rows and sent figures. `watched()` is `watchers > 0`.
20. `poll_tick(only)` (:849-865): records (filtered), rows with `reconcile: on-failure` (reader `app`, probes allowed); records listed again; a row whose stamp `<generation> <machine_id> <unreached or ""> <away or "">` changed or whose record left is dropped; `on_polled` gets the kept rows; each kept row whose key differs from the last emitted for it is emitted as `workspace.status` and remembered. A failing tick warns `status poll tick failed` and the next runs.
21. Row key (:778-779): the row as JSON with `facts` removed, `reach.url` and `reach.expiresAt` removed, and every key named `uptimeMs` or `readAt`, at any depth, removed. Every `workspace.status` on the bus, the tracker's own and the runtime's pushes, sets the last-emitted key (:784-786).
22. `poll(ids)` (:925-927): only while watched and with ids, one poll tick over those ids, failures warned.

## Edge cases
- First silence after an answer keeps the earned word; a second in a row reads `unreachable`; an answer clears it at once; the raw miss still sends the poll to the provider ("the reach word a row shows").
- The `table` reader's probes never spend the `app` reader's silence.
- A provider that says paused while reach is slow: no exec probe, the suspect dropped ("a machine the provider says is paused gets no probe").
- `zombie_probe: false` past the window: the last settled word, no probe.
- An `unreached` machine probed every tick holds one probe in flight while the provider hangs.

## Behaviour nobody wrote down
- Rule 3.17: poll 15 s, first run at once, re-armed before its body; cost 5 s, first after one interval.
- Rule 3.26: only the last watcher's release stops the timers; `close()` of the runtime does not.
- Rule 3.3: `records`, `emit`, `pass`, `on_gone`, `on_polled` and every record closure are called with no borrow held; `emit` re-enters the tracker through its own `workspace.status` listener.

## Decisions
1. Row equality compares the serialized struct, whose key order is fixed; the TS compared `JSON.stringify` text, whose order followed each builder's spread.
2. The extra record fields the runtime puts on a record (`checkout`, `pr`, `tree`, `wake_ask`) are explicit `StatusRecord` fields.
3. `StatusApi` is a trait the tracker implements (the runtime holds `Rc<dyn OriginStatusApi>`); the tracker keeps a `Weak` of itself for the listeners and timers it registers.

## After the port
- A zombie verdict's reason is dropped from the row when the record also carries `unreached` (Behaviour 8).

## Tests
See `tests/status_poll.rs`; each test names its rule in its first comment line. Rules 1-5: `enriches_views_*`, `maps_napping_and_gone_*`, `an_explicit_list_asks_the_provider_once_*`, `a_queued_*`, `an_away_*`, the seven "answers for its own daemon" cases. 6-7: `a_request_that_never_leaves_this_computer_marks_no_row`. 8: the nine "provider answered it cannot reach" cases. 12, 17: the five "zombie at rest" cases. 13: `a_workspace_that_goes_takes_its_facts_backoff_with_it`. 14-16: `asks_the_provider_nothing_across_healthy_polls`, `asks_the_provider_once_after_a_failed_reach_*`, `a_missing_metrics_read_is_warned_once_per_machine`. 19-22: `emits_workspace_status_when_*`, `does_not_send_a_row_again_*` (2), `a_tick_that_throws_*`, `a_row_built_on_a_record_that_moved_is_dropped`, `the_last_release_stops_the_ticks`. 18, 7: the seven "reach word a row shows" cases.
