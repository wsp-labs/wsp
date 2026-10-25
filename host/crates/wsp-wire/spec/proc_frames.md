# proc_frames
Source: packages/protocol/src/wire/machine-link.ts lines 510-702 (193 lines) and the process rows of daemon.ts (ProcEntry, ProcOwner, ProcScope, byProcColumn, pipeOpenOf: daemon.ts:85-108, 323-460) at a3b870a6e. Tests: proc-entry.test.ts (2 cases), daemon-ws.test.ts (1 assertion); 10 more pinned here against what the TS module printed for the same inputs. Rust: `src/wire/machine_link.rs` (from DaemonErrorCode down), `src/wire/daemon.rs` (the process rows); tests `tests/proc_frames.rs` (12).

## What it does
How the host reads what a daemon says about its computer's load and processes: the readings sample and its history, the process snapshot and the changes that follow it, the containers and cgroups riding them, the reply envelope every daemon answer comes in, and the pipe's opening frame. Reading leniently is the point: a row a newer daemon writes must never drop the frame it rides in.

## Behaviour
1. `ProcEntry.kind` (daemon.ts:382, `ProcKind.catch("other").optional()`): absent stays None; a known word is that kind; null, a number, or any unknown word is `Some(Other)`. The read never fails on `kind`. `proc_kind_or_other` is that reader.
2. `ProcEntry.owner` (:380, `.optional().catch(undefined)`): absent, null, or any owner that fails to read (an unknown `by` rung, a wrong type) is None; the row stands.
3. `ProcContainer.placedBy` (machine-link.ts:647): the same catch; an unknown word reads None.
4. Integers: `pid`, `ppid`, `pgid` and a frame's `daemon`, `total`, `seq`, `base`, every `gone` pid are i64; a fraction refuses the frame with the issue at that field's path (`["procs", 0, "pid"]`). `cpu`, `rss`, `startedAt`, `energyMw`, a frame's `at` are plain numbers (JsNumber) and keep fractions.
5. `apply_proc_changes(snapshot, changes)` (machine-link.ts:696-702): None unless `changes.base == snapshot.seq` (a snapshot with no seq takes no changes). Otherwise rows keyed by pid from the snapshot; every pid in `gone` removed; then every row of `changes.procs` set, whole; the result's procs sorted by pid ascending; `at`, `daemon`, `total`, `seq` are the changes'; `cost` is the changes' or absent (never the snapshot's); `containers` the changes' when present (an empty list included), else the snapshot's, else absent; `cgroups` the changes' or absent. Field order as written: type, at, daemon, total, procs, seq, cost, containers, cgroups.
6. `by_proc_column(sort, a, b)` (daemon.ts:414-419): Cpu compares `b.cpu - a.cpu`, Mem `b.rss - a.rss` (heaviest first); a tie is `a.pid - b.pid`. NaN reads as equal (Ordering::Equal) then falls to pid, as a JS comparator's NaN does.
7. `pipe_open_of(text)` (daemon.ts:99-108): JSON-parse the text; read it as PipeOpen (`op` the literal `pipe.open`, `door` http|gh, `heldMs` a non-negative integer, unknown keys dropped); anything else, invalid JSON and a daemon.hello included, is None.
8. `DaemonResponse` (machine-link.ts:531-544) through `parse_daemon_response`: `ok: true` keeps every key in order (passthrough); `ok: false` needs a string `error`, reads `code` (DaemonErrorCode), `kind` (MachineErrorKind) and an integer `status` when present; `id` is a RequestId or null.
9. `SysSample` (:551-558) is a member of DaemonEvent and holds no `type`; where it stands alone inside `workspace.sys` and `place.sys` it is a `SysSampleFrame`, which writes and needs `"type": "sys.sample"` (an absent type refuses at ["sample","type"]).
10. The shapes held to wsp-frames' types (`SysPoint`, `SysHistoryReply`, `UsageLogsReply`, `TranscriptsListReply`, `TranscriptsReadReply`, `ProcCost`, `CgroupMemory`, `ProcListReply`, `PipeOpen`) are wsp-frames' and read every bound while deserializing; their Check is `Ok(())`.

## Edge cases
- A pid both in `gone` and in `procs` of one frame stands, with the new row (gone first, then set).
- `pipe_open_of` with `heldMs: -1` or `door: "ssh"` is None; an extra key is dropped from the frame it answers.
- `ProcEntry` with `kind: null` writes back `"kind": "other"`.

## Behaviour nobody wrote down
- Rule 4.26: the snapshot apply_proc_changes builds is written in the field order above, which is the order the host sends a page.
- The CLAUDE.md trap (#1720): `seq` on a snapshot stays optional, and a frame missing it is a whole list with no changes to follow.

## Tapes
`proc.watch` streams, `sys.watch`, `sys.subscribe` (workspace.sys, place.sys) and every daemon reply; the daemon contract fixtures for `proc.snapshot`, `proc.changes`, `sys.sample` (tests/contract_fixtures.rs). Held out: none counted here.

## Decisions
1. `apply_proc_changes` returns a `ProcSnapshot` struct, which writes its `type` only inside `DaemonEvent::ProcSnapshot`, where the TS object carries it itself.
2. `by_proc_column` answers `std::cmp::Ordering` for `sort_by`, where the TS answers a number for `Array.sort`.

## After the port
- `apply_proc_changes` drops the snapshot's `cost` and `cgroups` when a changes frame carries none, while it keeps the snapshot's containers: two rules for three lists.
