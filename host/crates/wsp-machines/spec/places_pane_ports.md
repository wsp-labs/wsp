# places_pane_ports: the ports a Browser pane opens here for a computer's port

Source: `pane-ports.ts` (134) and `listenNear` (`helpers.ts:256-281`). Module `places::pane_ports`; `listen_near`
in `places::helpers`. Shared rules: `places-a-README.md`. TS cases: `test/box-panels.test.ts:375-435` (three), at
this unit's seam.

## Constants

`PANE_HOLD_MS = 120_000`, `PANE_QUIET_MS = 3_600_000`, `PANE_BUSY_RECHECK_MS = 15_000`, `NEAR_PORTS = 20`
(private), `FORWARD_MAX_PER_TARGET = 16` (wsp-wire). Lines from wsp-wire: `pane_forward_floor_line`,
`..._cap_line(name, cap)`, `..._quiet_line`, `..._stopped_line`, `..._gone_line`.

## `pane_ports(deps)` (:43)

`deps = { forwards: shared IndexMap<String, Forward> (the context's), now, schedule(ms, f) -> Cancel }`. State, all
memory only: `held: IndexMap<key, Held>` (insertion order is `list()`'s order), `ended: IndexMap<key, which line>`,
listeners. `Held = { forward, view: PortForward, held_at, used_at, due: Cancel }`.

- **`reach(key, port, asker { workspace_id, name }, on_conn) -> Result<u16 local port>`** (:78), in this order:
  1. A held forward under `key`: `held_at = now`, re-armed (the old timer cancelled), its local port answered. No
     floor, cap or `ended` check: a standing forward is held again whatever the number.
  2. An `ended` line under `key`: removed, and `Err(line(port), Kind::Usage)`. The ask after that one opens again.
  3. `port` not an integer in 1024..=65535: `Err(pane_forward_floor_line(port), Usage)`. 80, 1023 and 65536 refuse;
     1024 and 65535 pass the floor.
  4. Held forwards whose `view.workspace_id == asker.workspace_id` number 16 or more:
     `Err(pane_forward_cap_line(asker.name, 16), Usage)`. Another workspace's forwards do not count; an `ended` one
     does not count.
  5. `listen_near(port, ...)`; its error passes through. Each connection calls `on_conn(conn, used)`, where `used()`
     sets `used_at = now` (the link's tunnel calls it at accept and again at close).
  6. `held[key] = { view: { workspaceId, port: local, startedAt: ISO(now), name, kind: "url" }, held_at = used_at =
     now }`, armed, `forwards[key] = forward`, then `forward.open` with the view emitted, then the local port.
     `view.port` is the local port, not the asked one, and `stop` matches on it.
- **The arm** (:64): at `conns > 0 ? now + 15_000 : min(held_at + 120_000, used_at + 3_600_000)`, scheduled after
  `max(0, at - now)`. When it fires: nothing if `held[key]` is no longer this `Held`. With a connection open, or
  both `now - held_at < 120_000` and `now - used_at < 3_600_000`, it arms again. Else it closes, and where `now -
  used_at >= 3_600_000` and `now - held_at < 120_000` (the pane still asks, the hour ran out) `ended[key] = quiet`.
  A forward whose pane stopped asking closes with no `ended` line: the pane's next ask opens afresh.
- **`close(key)`** (:53): removes `held[key]`, cancels its timer, removes `forwards[key]` only where it is this
  forward (a newer forward under the key stays), stops the forward (every connection destroyed, every listener
  closed: the port no longer connects), emits `forward.close { workspaceId, port }`.
- **`standing(key, port)`**: the held forward's local port, or `Err(pane_forward_gone_line(port), Usage)`. Opens
  nothing and touches neither `held_at` nor `ended`.
- **`list()`**: every held view in insertion order.
- **`stop(workspace_id, port) -> bool`** (:122): the first held entry whose view matches both: closed; and where
  `now - held_at < 120_000`, `ended[key] = stopped`. False and nothing changed where none matches.
- **`on(f) -> unsubscribe`**: listeners in insertion order; the answer removes it.

## `listen_near(port, on_conn) -> Result<(listeners, u16)>` (helpers.ts:262)

For `at` from `port` to `min(port + 20, 65_535)`: listen on `127.0.0.1:at`; on any failure try the next `at`. Then on
`[::1]:at`: success answers both and `at`; a failure other than address-in-use (no IPv6 here) answers the IPv4
listener alone and `at`; address-in-use closes the IPv4 listener and tries the next `at`. None: `Err("no port from
{port} to {port + 20} is free on this computer")`, with `port + 20` even where the walk stopped at 65,535.

## Pins

| Rule | Test | TS case |
|---|---|---|
| opens at the asked number and answers it; view and open event | `opens_at_the_asked_number_and_says_so` | box-panels:376 |
| a pane asking every minute holds it past ten quiet minutes; stops asking, gone at the hold, no ended line | `stands_while_the_pane_keeps_asking_and_goes_once_it_stops` | box-panels:376 |
| quiet hour while asked: closed, told once, opens again | `goes_after_a_quiet_hour_tells_the_pane_once_and_opens_again` | box-panels:397 |
| an open connection defers the close by 15 s rechecks | `an_open_connection_holds_it_past_the_hold` | |
| a use moves the quiet hour | `a_connection_moves_the_quiet_hour` | |
| floor 1024 to 65535 | `refuses_a_port_below_1024_or_above_65535` | box-panels:411 |
| cap of 16 per workspace, others not counted | `refuses_the_seventeenth_for_one_workspace_and_not_another` | box-panels:411 |
| list order, stop by local port, stopped told once, standing gone, reopen | `the_ports_list_stops_one_and_its_pane_is_told_once` | box-panels:411 |
| stop of an unheld port | `stop_of_a_port_nobody_holds_is_false_and_changes_nothing` | |
| a stop after the hold ran out leaves no line | `a_stop_after_the_hold_leaves_no_line` | |
| close leaves a newer forward under the key | `close_keeps_a_newer_forward_under_the_same_key` | |
| a standing forward is held again with no floor or cap check | `a_held_forward_is_held_again_without_the_checks` | |
| a stale timer does nothing | `a_timer_for_a_closed_forward_does_nothing` | |
| listen_near next free number, both families | `listen_near_takes_the_next_free_number`, `listen_near_holds_both_loopbacks` | box-panels:314 |
| listen_near none free | `listen_near_says_no_port_is_free` | |
| unsubscribe | `on_answers_an_unsubscribe` | |
