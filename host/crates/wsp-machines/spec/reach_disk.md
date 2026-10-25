# reach_disk

Source: `packages/runtime/src/machines/reach.ts:1-197, 374-379, 476-482` (about 210 of 483 lines) at `65f6b89c7`.
Tests: `packages/runtime/test/disk-watch.test.ts:67-209` (7 cases; the resize half is another module's),
`gone.test.ts:1126, 1229` (the refused nap and stop on the row), `status.test.ts` (the status push). Rust:
`src/machines/reach/disk.rs`; the area struct `ReachArea` is declared in `src/machines/reach.rs`.

## What it does

The status a workspace's row is pushed with outside the 15-second poll, and the three refusals and the one reading
that ride on it: a pause the provider refused outright, a stop it refused while the disk's snapshot fails, and the
disk of a machine whose stop snapshots it (Boat), read at each turn's end and every 2 minutes while such a machine
runs. Past 90% full, once per crossing, what wsp made on the machine is cleared, the disk read again, and every
thread on the machine is told how full it is and the first size up whose disk is bigger.

## State (memory only, all lost at a restart)

`polled_reach: ws -> {machine_id, reach}` (:31), `nap_refusals` and `stop_refusals: ws -> {machine_id, said}` (:34,
:48), `disks: ws -> {machine_id, pct, told}` (:55), `disk_reads: ws -> shared future` (:140), one disk timer (:142).
Each held fact is for one machine: a fact about another machine id reads as none.

## Behaviour

1. Constants: `DISK_READ_MS = 120_000`, `DISK_CLOSE_WAIT_MS = 2_000` (pub).
2. `nap_refused_of(entry)` (:35-41): none without a refusal; the refusal's `said` when its machine is the entry's;
   otherwise the stale refusal is removed and none. `nap_refused_reason` is `nap_refused_line(said)` of it.
3. `stop_refused_reason(entry)` (:49-52): `stop_refused_line(said)` only for the entry's own machine; a stale one is
   left in the map (not removed, unlike the nap's).
4. `disk_reason(entry)` (:56-59): `disk_full_line(pct)` when the held reading is the entry's machine and `pct >
   DISK_FULL_PCT` (90); else none. Exactly 90 is none.
5. `watches_disk(entry)` (:60): the backend's pause keeps the disk (`pause_keeps_disk(backend_for(record))`) and the
   phase is `running`.
6. `disk_read(entry)` (:63-74): `DISK_USE_CMD` with `INLINE_EXEC_MS`; an exec error, a non-zero exit or an answer
   `disk_use_of` cannot read is none; so is a machine no longer watched by the time it answers. `told` stays set only
   for the same machine while `pct > DISK_REARM_PCT` (85); `crossed = !told && pct > 90`; the reading is stored with
   `told || crossed`. When `disk_reason` changed by this reading the status is pushed at once with `reach_of(entry)`
   and `row_reason(entry)`. Answers `{use, crossed}`.
7. `read_disk(entry)` (:78-87): resolves at once for an unwatched machine. Arms the watch (rule 9). One read per
   workspace at a time: a second caller joins the one in flight, which is removed from the map when it settles.
8. `read_disk_now(entry)` (:88-106): the first reading; nothing more unless it crossed and the host is not closing.
   Then `sweep_leftovers(machine)` (a failure warns `what wsp made on <name> was not cleared: <message>`), a second
   reading (the first stands when the second reads none), and `move_for(entry, used + free)`. The words: `machine:
   name`, `on: provider_key_name(provider_of(record) ?? "the provider")`, `pct`, `usedBytes`, `sizeBytes: used + free`,
   `freedBytes: max(0, first.used - after.used)`, `machineGoes: record.sub is set`. Warn `<name>: <disk_mark_line(
   words)>`, then `tell_threads(id, |thread| disk_mark_line({...words, move: {...to, thread}}))`, the move only where
   one was found.
9. `arm_disk_watch()` (:143-154): nothing while armed, while closing, or with no watched live machine. Else one timer
   at `DISK_READ_MS`: it clears itself, starts `read_disk` for every live entry (each failure warns `disk of <id> not
   read on the watch: <message>`) without waiting, and arms again. One machine's slow clearing never holds the beat:
   the next beat skips only the workspace whose read is still in flight (rule 7's join).
10. `stop_disk_watch()` (:155-159): cancels the timer, then waits for every read in flight to settle or
    `DISK_CLOSE_WAIT_MS` through the clock, whichever is first.
11. `move_for(entry, read_bytes)` (:116-126): none when the backend offers no `root_disk_of`. The backend's `limits()`
    is awaited first where offered, its failure ignored (it refreshes the sizes). The capability sizes with more
    memory than the record's, sorted by memory ascending (stable); the first whose root disk is known and `diskGb *
    1e9 > read_bytes * 1.1` gives `{name: disk.name, cpu, memMb, diskGb}`.
12. `tell_threads(ws, line)` (:107-112): for the latest row of each thread on the workspace (rule 13), `record` a
    `session.notice` `{type, workspaceId, sessionId: claudeSessionId ?? turnId, turnId, threadId, text: line(thread)}`,
    `thread` being the row's `threadId`, else its row id.
13. `last_turns(ws)` (:128-137): rows of the workspace keyed by `threadId ?? id`; a later row with `startedAt >=` the
    held one's (absent reads 0) replaces it, so ties go to the later row in map order; answered in first-key order.
14. `reach_of(entry)` (:165-171): `unreachable` while `unreached_of(entry)` holds a line; the polled reach when it is
    for this machine; else `reachable` when the kind module says `has_daemon(entry)` or the machine has a
    daemon-answers road, else `unsupported`.
15. `emit_status(entry, reach, reason)` (:172-191) emits `workspace.status` with `status` keys in order: the record's
    view, `machineState: machine_state_of(phase)`, `reach: {state}`, `size`, `rateUsdPerHour:
    pricing.rate_usd_per_hour(size)`, `reason?`, `wakeAsk?`, `idleAt?` (only while `running`, from `idle.idle_at`),
    `checkout?`, `pr?`, `tree?` (all off the live entry).
16. `says_waking(entry, words)` (:194-197): `entry.wakeSaid = words`, then `emit_status(entry, "napping", words)`.
17. `push_status(entry)` (:376-379): only for a `running` record, with `reach_of`. `status_now(entry)` (:477): with
    `reach_of` when running, else `napping`, no reason.

## Edge cases

- 88% then 91% after a told crossing says nothing again; 84% re-arms; 91% then is a second crossing
  (disk-watch.test.ts:68).
- A machine on the 12 GB disk moves to default (75 GB); one on 75 GB skips default (same disk) for large; large is
  offered nothing (xlarge has no measured disk) (:178).
- A machine whose stop keeps memory is never read (:197).
- A reading for the record's previous machine is ignored by every reason (rules 2 to 4).

## Behaviour nobody wrote down

- Rule 3.3: `disk_read` re-reads `disks` after the exec; nothing borrowed across `emit_status`, `sweep_leftovers`,
  `record`.
- Rule 3.17: the beat is 2 minutes and re-arms itself; it is armed only while a watched machine stands.
- Rule 3.27: `stop_disk_watch` is one of the waits `close()` makes, bounded at 2 s.

## Decisions

1. The bounded wait in rule 10 uses the context clock (rule 3.15), where the TS uses a raw `setTimeout`.

## After the port

- A stale stop refusal is never removed from the map (reach.ts:49-52), unlike a stale nap refusal; it leaks one entry
  per replaced machine until a pause lands.

## Tests (tests/reach_disk.rs)

| Rule | Test (TS case) |
|---|---|
| 6, 8, 9 | `is_read_on_the_beat_cleared_past_90_and_told_again_only_after_falling_5_under` (disk-watch :68) |
| 7, 9 | `a_machine_still_clearing_holds_back_no_other_machines_read` (:112) |
| 10 | `a_host_closing_waits_on_a_read_still_clearing_only_so_long` (:148) |
| 11 | `offers_the_first_size_up_that_grows_the_disk_and_none_on_large` (:178) |
| 5, 7 | `reads_nothing_on_a_machine_whose_stop_keeps_memory` (:197) |
| 8 | `tells_a_sub_threads_machine_it_goes` (:162) |
| 2 | `a_nap_refusal_stands_for_its_machine_and_drops_for_another` (gone :1126) |
| 3 | `a_stop_refusal_stands_on_the_row_for_its_machine` (gone :1229) |
| 4 | `disk_reason_is_none_at_exactly_90` (new) |
| 12, 13 | `tell_threads_writes_under_the_latest_turn_of_each_thread` (new) |
| 14 | `reach_of_takes_unreached_then_polled_then_the_kinds_claim` (new) |
| 15 | `emit_status_carries_the_entrys_facts_in_order` (new, JSON text) |
| 16, 17 | `status_now_and_push_status_read_the_phase` (new) |
