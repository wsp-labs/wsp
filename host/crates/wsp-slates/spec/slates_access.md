# slates_access
Source: slates.ts:294-381 (the queue, save, the approvals the runs read, `moved`, `timed`), 426-462 (`ready`,
`recover`), 476-555 (`refuseSubThread`, `targetOf`, `byOf`, records, `checkVersion`, `announce`, `push`,
`keepProblems`), 1095-1116 (`settled`, `close`, `get`), 1281-1288 (`shown`), 1450-1480 (`subscribe`), 1495-1563
(`turnEnded`, `rewound`, `forget`, `watchesPr`, `closeServers`, `napped`) at 65f6b89c7. Tests: slates.test.ts
cases on restart, rewind, timers, sub-threads, delete; lazy wiring through `ready`.

## What it does
Who may touch which slate, the one-at-a-time chain per thread, the store, start-up recovery, the transcript events
and pushes, and the thread's lifecycle: a turn's snapshot, a rewind, a forget, a window's hold.

## Behaviour
1. `serial(thread, run)` is rule R2's chain (:298-302). `save(r)` stamps `updatedAt = now` then puts the record
   whole in `slates` under the thread id (:303-306).
2. The approvals the runs read (:314-329): `has` reads the record's approval as `allowed`; `allow` (no record does
   nothing) writes `{state:"allowed", at: now, ...approval_names(r, key)}` and saves without waiting; `revoke`
   (only when there) deletes and saves without waiting; `list` the allowed keys in order.
3. `moved(thread, run, record)` (:332-335): the record of the run a start is capturing is the start's own and is
   dropped; any other queues `run_moved` on the thread's chain; a failure logs `the slate of thread <word> lost a
   record of $<run>: <message>` at warn.
4. `timed(thread, run)` (:371-381), on the chain: a run the document no longer declares does nothing; with no
   window showing and `settled(thread)` true it joins `skipped` and starts nothing; else one batch `by: timer` with
   `starts: [{run, by: timer}]`. Failures are swallowed.
5. `ready()` (:427-435), once for the area's life: `deps.loaded()`, then every stored record with `schema` 2 that is
   not held yet is held with `revision ?? version`; then each held record, one after another, recovers on its chain.
6. `recover(r)` (:440-462): secrets refreshed; no document stops there (nothing saved); each declared run reading
   running becomes `restarted_record` and all go in one batch `by: run`, else the record is saved; timers armed; each
   run held for approval (`why` absent or `needs your approval`) is started again `by: person` so the runs table
   holds it for the sheet; when the runs table then holds any, a later task on the chain re-holds each whose key
   moved (`held_again`) and saves and pushes them.
7. `target_of(p, caller, write)` (:485-500): a sub-thread token (scope root differs from thread) refuses `Z803
   sub-thread: ...` (kind usage, code Z803). A token names its own thread when `p.thread` is absent or equal; a write
   to another refuses `Z800 thread-not-yours: a thread writes its own slate, and thread <word> is not this one`; a
   read of a thread not under it refuses not-found `no thread <word>` with code Z801. With no token: a thread id is
   taken when the runtime knows it or a record exists, else not-found; a turn token is resolved by
   `thread_of_token`; else usage `a slate belongs to a thread, and this line names none and runs inside no turn.`
   with fix `Name the thread by id, as wsp threads lists it.` `by_of` is `agent` with a token, else `person`.
8. `record_of` awaits `ready` then the held record. `need_record` refuses usage `Z802 no-slate: this thread has no
   slate yet; write one with slate_write` when there is none or it has no document at version 0. `fresh_record`
   needs the runtime's facts (not-found `no thread <word>` without) and is the record of model 1 at version 0,
   `empty: "none"`, not held until written.
9. `check_version(r, if_version)` refuses conflict `V750 version-behind: the slate is at version <v>, not <n>; read
   it and write again` when given and different (:519-521).
10. `announce` (:523-537) records `{type:"session.slate", workspaceId, sessionId (the facts' or the thread id),
    [turnId], threadId, cause, version, by, pieces (first 20), [run]}`. `push(r, names)` (:539-542) emits
    `{type:"slate.values", workspaceId, threadId, version, revision, values}` with `$name` for each distinct name
    the record holds, in the order given; nothing when none. `keep_problems` appends and keeps the last 20.
11. `turn_ended` (:1495-1507): after `ready`, on the chain: no record does nothing; the snapshot of this revision is
    kept once (a copy); the turn moves to the end of `turns` as `{at, version, revision}`; trimmed; saved.
12. `rewound` (:1509-1526): after `ready`, on the chain: the snapshot the turn points at (a copy), or with none
    `{document: null, values: <as they are>, empty: "rewound-before"}`, is put back (`become`); the cut turns go;
    trimmed; version and revision + 1; saved; `announce(restore, host)`; every value pushed.
13. `forget(thread, keep)` (:1528-1546), on the chain: the runs and MCP tables dropped (`keep` leaves running ones),
    pins dropped, the record let go, the slate's folder removed (recursive, missing is fine), its folder on the
    thread's machine handed to the ledger without waiting, holds, skipped, windows, write budget, send stamps, event
    bucket, views and `started_by` entries dropped, then the stored record deleted. It does not await `ready`.
14. `subscribe(p)` (:1450-1480): each named source's hold counted; the windows count up; both tables told shown;
    each skipped timer run fired now and the skipped set emptied; `pr` among the sources asks `watch_pr` for the
    record's workspace. The release, once: windows down (not below 0), each source's hold down (gone at 0), and
    when no source is held at all, both tables told not shown.
15. `shown(thread)` (:1281-1288): on the chain, sets `shownOnce` and saves, once.
16. `settled()` (:1098-1105): waits on every chain's tail until a round finds them unchanged, at most 50 rounds.
17. `close()` (:1107-1111): the runs table, the MCP table and the ledger closed (rule R8). `get(thread)` is the
    view, or null with no record (:1113-1116).
18. `watches_pr(ws)` (:1548-1555): a record of the workspace with a `pr` hold, or whose document's JSON names
    `\bpr\.checks\b`. `close_servers(thread)` and `napped(ws)` close the MCP servers of the thread, or of each
    record of the workspace.

## Edge cases
- Restart mid-run: the run reads `failed`, `the host restarted while it ran`, its `done` reaction fired once; a
  memory secret reads unset; other values stand; a third start finds nothing running and fires nothing.
- A thread's windows count only one hold per subscribe, whatever its sources; a subscribe with no sources still
  counts as shown.
- A release called twice does nothing the second time.
- A settled thread's timers, `always` ones too, start nothing until a window subscribes; then each starts at once.

## Behaviour nobody wrote down
- Invariant 2 (one `done` per run end) and invariant 3 (the thread off the token) live here; rule R7 is `recover`.
- Rule 3.4: `ready` is awaited before every read of a record but `forget`'s.

## Decisions
1. The chain is a map of thread id to the tail future; the caller awaits its own result (`futures::Shared` tail).

## After the port
- `allow` and `revoke` save without waiting, so a failed save is lost silently (:320, 326).
- `requests` (the press ids) are not dropped by `forget`.
