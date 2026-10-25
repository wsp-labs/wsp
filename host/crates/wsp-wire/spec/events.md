# events
Source: packages/protocol/src/wire/events-union.ts (110 lines) and daemon-events.ts (92 lines) at a3b870a6e. Tests: protocol.test.ts (19 cases), places.test.ts (2), port-forwards.test.ts (2), callback-relay.test.ts (2), init-job.test.ts (2), agents-report.test.ts (1), preferences.test.ts (1), broker-contract.test.ts (1); 3 more pinned here against what the TS module printed; the daemon event contract fixtures in tests/contract_fixtures.rs. Rust: `src/wire/events_union.rs`, `src/wire/daemon_events.rs`; tests `tests/events.rs` (33).

## What it does
The two event streams the host reads or writes: `EventUnion`, every event the host's bus carries to subscribed sockets (77 types, each a views or top-level event schema plus an optional `seq`), with the reply `events.subscribe` answers first; and `DaemonEvent`, the 21 types a daemon pushes on its socket, which the host reads and relays to a page.

## Behaviour
1. `EventUnion` (events-union.ts:18-96) is internally tagged on `type`, one variant per member in the TS order (`workspace.creating` first, `aside.text` last), each `Sequenced<E>`: the event's own fields (the views writer's struct, which holds no `type`) then `seq`. Written: `type` first, the event's fields in its TS order, `seq` last and only when present.
2. `Sequenced.seq` (helpers.ts:81): absent, or a positive integer; 0 is `too_small` at ["seq"] ("Number must be greater than 0"), a fraction `invalid_type` at ["seq"].
3. `parse_event(value)` reads the union as zod does: an unknown or absent type is ONE `invalid_union_discriminator` at ["type"] with `detail.options` = `EVENT_TYPES` (77, in order); a known type reads its event (the views type's own issues), then `seq`.
4. `EventsSubscribeReply` (:105-109): `seq` a non-negative integer (0 before the first event; -1 is `too_small`), `stream` optional, `gap` only the literal `true` (false is `invalid_literal` at ["gap"]). Written `seq`, `stream`, `gap`.
5. `DaemonEvent` (daemon-events.ts:119-201) through `parse_daemon_event`, the same discriminator rule with `DAEMON_EVENT_TYPES` (21, `daemon.hello` first, `guest.closed` last). `daemon.hello.version` an integer (a string refuses); `pty.mode.mode` line|raw with `echo` and `foreground` required; `browser.open.url` passes `is_http_url` (custom "http or https URL") and `port` is a RelayPort; `callback.port` and `localhost.url` a RelayPort (80, 1023, 65536, 1.5, -1, "8123" refuse); `port.close` flattens PortCloseDetail; `stdio.exit.code` an optional integer; `guest.opened` needs `session`, `life`, `kind` (mcp|cli), `token`, `argv`, `cwd`; `guest.message.message` any JSON, kept as sent.
6. `sys.sample`, `proc.snapshot`, `proc.changes` are the newtype variants of proc_frames' structs.
7. Every accept event of the daemon contract fixtures reads and writes back as the same value; every reject event refuses (tests/contract_fixtures.rs).

## Edge cases
- An event with no `seq` writes none (never `"seq": null`).
- `{"seq": 0}` is a valid subscribe reply and an invalid event.

## Behaviour nobody wrote down
- Rule 4.19-4.20: `seq` counts from 1 per host process and `stream` names the process, so nothing here survives a host restart: after one, a client's cursor names another stream and the reply carries `gap: true`. The reader holds no cursor; the bus (runtime) does.
- Rule 4.26: an event is written type first, fields in the TS order, seq last; the views structs declare their fields in that order.

## Tapes
`events.subscribe` and every op whose event reaches a subscribed socket before its reply (rule 4.21); chapter 04's replay tapes. Held out: none counted here.

## Decisions
1. Every event struct leaves its `type` out and carries `pub const TYPE`; the union writes and reads the tag (agreed with the views writer).
2. `EventsSubscribeReply.seq` and `Sequenced.seq` read as i64 so that a negative or zero value is zod's `too_small` from Check, not a serde type error.

## After the port
- A second `events.subscribe` on one socket attaches a second listener (rule 4.22); that is the bus's, kept for parity.
