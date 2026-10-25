# status_probe
Source: packages/runtime/src/status.ts:1-332 (the free functions, constants and option types) at 65f6b89c7. Tests: packages/runtime/test/status.test.ts, the probe cases ("maps a prompt 502...", "the probe tells this computer's road...", "the caller's bound is the whole ask", "a guest that takes longer than the prompt reads slow") and the socket filter cases of "serveRuntime status.subscribe". Rust: `src/status.rs` (types, constants, free functions), tests `tests/status_probe.rs`.

## What it does
The pure half of the status tracker: what a runtime phase means as a provider state, one reach probe over HTTP or over a machine's own road, what a failed probe says, the facts backoff, the provider's words for a row, and the per-socket filter that keeps unchanged cost figures off a socket.

## Behaviour
1. Constants (:257-272): `PROBE_TIMEOUT_MS 10_000`, `PROMPT_MS 2_500`, `RECONCILE_MIN_MS 300_000`, `ZOMBIE_WINDOW_MS 180_000`, `ZOMBIE_PROBE_TIMEOUT_MS 20_000`, `POLL_INTERVAL_MS 15_000` (public), `ZOMBIE_PROBE_CMD "echo ok"`, `FACTS_BACKOFF_MAX_MS 300_000`, `COST_INTERVAL_MS 5_000` (:340), `DAEMON_ANSWER 426` (:64).
2. `machine_state_of(phase)` (:20-38): running and pausing give running; napping paused; waking and queued starting; gone and released gone.
3. `phase_leaving_gone(read)` (:43-45): running gives running, paused gives napping, anything else `None`.
4. `provider_said(e)` (:49-53): the message is `said` of a machine-unreachable error (`is_machine_unreachable`, ExecFailed included), else the error's message; with a status (`e.status()`), `<status> <message>`.
5. `missed(e)` (:79-81): `{state: unreachable, from_daemon: false}`, plus `offline: true` only when `road_failed(e)`.
6. `probe_reach(url, o, now)` (:88-99): `started = now()`; one GET through the platform fetch (`wsp_engine::fetch::platform_fetch`) capped at `o.timeout_ms`; the body read and its failure ignored; `from_daemon = status == 426`; `now() - started > o.prompt_ms` (strictly) gives `slow` with that `from_daemon`; else 502 gives `no-daemon`, every other status `reachable` (401, 403, 404, 500 included). A fetch that fails is `missed(e)`.
7. `ask_daemon(answers, o, clock)` (:104-117): `started = now`; `up = bounded(answers(Some(o.timeout_ms)), o.timeout_ms)`; past the prompt (strictly) gives `slow` with `from_daemon = up`; else `reachable` or `no-daemon` by `up`, `from_daemon = up`. A failure, the bound's included, is `missed(e)`.
8. `bounded(fut, ms, clock)` (:290-298): the future's own answer if it comes first; else after `ms` on the clock an error `timed out after <ms> ms` (`EngineError::Other`, no code, so never offline). The future is not cancelled: it is spawned and runs to its end on its own, as the TS leaves the promise to settle.
9. `facts_backoff_ms(misses)` (:278-280): `min(300_000, 15_000 * 2^max(0, misses - 1))`: 15 s, 30 s, 60 s, 120 s, 240 s, then 300 s.
10. `CostMoved::admit(e)` (`costMoved`, :320-332), one per socket: `workspace.deleted` forgets that workspace's figures and is let through; any event that is not `workspace.cost` is let through; a cost event is let through only when its figures `<phase> <rate> <accrued>` (numbers in JavaScript's formatting, `wsp_wire`'s JS formatter, rule 4.25) differ from the last let through for that workspace, which it then records.
11. Option types (:55-171): `ProbeOptions {prompt_ms, timeout_ms}`; `StatusReader` `app` or `table`; `StatusListOptions {probe_timeout_ms, reader, prompt_ms, reconcile: Always | OnFailure, zombie_probe}`, every field optional; `StatusWatchOptions {cost_interval_ms, poll_interval_ms, probe_timeout_ms, prompt_ms, reconcile_min_ms, zombie_window_ms, zombie_probe_timeout_ms}`, every field optional.

## Edge cases
- A 426 that took longer than the prompt is `slow` with `from_daemon: true` (Behaviour 6).
- A 502 answered after the prompt is `slow`, not `no-daemon` ("maps a prompt 502 to no-daemon, a late answer to slow").
- No DNS or no route (`ENOTFOUND`, `ENETUNREACH`): `unreachable` and `offline`; a timeout, a refused port (`ECONNREFUSED`) or a dropped request: `unreachable`, not offline ("the probe tells this computer's road from the machine's silence").
- `ask_daemon` whose `answers` never settles: `unreachable` after `timeout_ms` on the clock, not offline.
- `facts_backoff_ms(0)` is 15_000, as `max(0, -1)` is 0.
- A cost event figure `0.30000000000000004` is formatted as JavaScript prints it, so `0.1 + 0.2` and `0.3` are different figures.

## Behaviour nobody wrote down
- Rule 3.15: `bounded`'s timer and `ask_daemon`'s elapsed read use the clock; `probe_reach` takes `now` as the TS does.
- Rule 4.25: figures and the `<status> <message>` line format numbers as JavaScript does.

## Decisions
1. `bounded` and `ask_daemon` take the clock (the TS's raw `setTimeout` goes through the clock, rule 3.15).
2. Errors the record's roads answer are `EngineError`; `missed` and `provider_said` read them with `wsp_engine::errors`.

## After the port
- `probe_reach` reads every status but 426 and 502 as `reachable`, so an edge's 401 or 404 shows a healthy row until the provider is asked (the poll does ask, status_poll Behaviour 9).

## Tests
| Rule | Test (`tests/status_probe.rs`) |
|---|---|
| 2, 3 | `machine_state_of_maps_every_phase`, `phase_leaving_gone_reads_running_and_paused_only` |
| 4 | `provider_said_quotes_the_status_and_the_providers_own_words` |
| 5, 6 | `maps_a_prompt_502_to_no_daemon_a_late_answer_to_slow_silence_to_unreachable_and_any_other_answer_to_reachable`, `a_426_is_the_daemons_own_answer`, `the_probe_tells_this_computers_road_from_the_machines_silence` |
| 7, 8 | `a_guest_that_takes_longer_than_the_prompt_reads_slow_and_one_inside_it_reads_reachable`, `the_callers_bound_is_the_whole_ask`, `a_guest_that_says_its_port_is_dead_reads_no_daemon`, `bounded_leaves_the_work_running_past_its_bound` |
| 9 | `facts_backoff_doubles_from_one_poll_to_five_minutes` |
| 10 | `cost_moved_lets_a_figure_through_once_and_forgets_a_deleted_workspace`, `cost_moved_lets_every_other_event_through`, `cost_moved_formats_figures_as_javascript_does` |
