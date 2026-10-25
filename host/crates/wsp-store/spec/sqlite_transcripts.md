# sqlite_transcripts

Source: packages/runtime/src/sqlite-transcripts.ts (270 lines) at 65f6b89c7, less the blob move. Tests:
packages/runtime/test/sqlite-transcripts.test.ts (24 cases: the module's own 4 port here; the blob move's 4 drop with
Proposal 3; the 16 runtime cases are the threads area's, `threads/transcripts.ts`). Rust tests:
tests/sqlite_transcripts.rs. Expected values: tests/fixtures/store/sqlite_transcripts.json.

## What it does

A workspace's transcript as rows of `events`, one per event, keyed `(workspace, pos)`, and its index as one
`transcript_index` row of text the threads area writes and reads. A flush appends; a read of one thread walks the
`(workspace, thread, pos)` index. Every call is one statement or one transaction, so a reader on another connection
sees a flush whole or not at all. The trait is synchronous, as the TS is.

## Behaviour

1. Every call reaches the connection through the store's `ready()` (sqlite-transcripts.ts:74-80), so it opens the
   database at a first call and refuses as the store refuses (sqlite_store 7).
2. `append(ws, events)` inserts each event in order inside `atomically` (sqlite-transcripts.ts:100-106,145):
   `(ws, pos, threadId or '', type, size, json)` with `{json, bytes: size} = event_json(event)`. An event with no `pos`
   is refused (the TS: `Provided value cannot be bound to SQLite parameter 2.`) and a `pos` the workspace keeps is
   refused (`UNIQUE constraint failed: events.workspace, events.pos`); either undoes the whole append.
3. `all(ws)` and `thread(ws, t)`: events `order by pos` and `size` summed, 0 for none (:109-120).
4. `newest(ws, t, before)`: the thread's events `pos < before` (`before` absent is 9007199254740991,
   `Number.MAX_SAFE_INTEGER`), `order by pos desc`, handed one at a time; `Break` ends the walk (:122-124).
5. `count(ws, t)`; `total(ws, None)` over every event, `total(ws, Some(t))` over one thread's; `size` 0 where there is
   none (:125-131).
6. `newest_pos(ws)`: `max(pos)`, 0 for none (:107).
7. `sizes(ws, t)`: `{pos, threadId, type, size}` oldest first, `threadId` absent where the stored `thread` is `''`
   (:133-140).
8. `word_events(ws, t)`: the thread's `session.start`, `session.steer`, `session.earlier`, and `session.delta` whose
   JSON `kind` is `text`, oldest first (`WORDS`, :246, 251).
9. `at(ws, positions)`: for each position in the order asked, the event there or nothing; a position asked twice comes
   twice (:141-144).
10. `remove(ws, positions)` deletes each in one transaction; `clear(ws)` deletes the workspace's events and index row in
    one transaction, no other workspace's (:146-157).
11. `index(ws)` is the stored text or `None`; `put_index(ws, json)` upserts it, verbatim: the store never parses an
    index (:158-163).
12. `atomically(f)` (:82-99): at depth 0, `begin immediate`, run `f`, `commit` on Ok, `rollback` on Err and return the
    Err; the depth goes back down on every road. At depth above 0 it only runs `f`: an inner call joins the outer
    transaction, so an inner Err the outer `f` catches leaves the inner writes in the outer commit (fixture `nested`).
13. A stored row is parsed back with no check beyond the `SessionEvent` type; a row that does not parse fails the read.

## Edge cases

- Ten events across threads `a`, `b`, none and `""` (fixture): `newest(w, a)` gives `[20,10,9,8,7,6,5,1]`,
  `newest(w, a, Some(7))` `[6,5,1]`, `newest(w, a, Some(0))` `[]`; `word_events(w, a)` `[1,5,6,8,20]`;
  `at(w, [4,4,99,1])` `[4,4,1]`.
- An event with no `threadId` and one with `threadId: ""` are both thread `''`: `count(w, "")` is 2,
  `thread(w, "")` gives both, `sizes` names no thread for either; read back, the first has no `threadId` and the second
  `threadId: ""` (rule 2.30).
- Sizes are UTF-16 units: the delta whose text is `😀 é` is 109 (fixture `raw`).
- `total` and `newest_pos` of a workspace with no rows: `{0, 0}` and 0.

## Behaviour nobody wrote down

- Runtime invariant 2 (no row written twice): the primary key refuses a position twice and the append lands nothing,
  so a replay that miscounts fails loudly rather than doubling a turn.
- Runtime invariant 5 (a transcript that does not parse is set aside and later turns are written): an index that is not
  JSON is kept and handed back verbatim, and appends and `put_index` go on beside it; the threads area decides it does
  not read (`loadRowsIndex`, rule 2.14).
- Rule 3.3: `newest`'s callback runs while the connection is borrowed and must not call the store.
- What survives a restart: rows and index rows. `depth` starts at 0 in each process.

## Tapes

`sessions.history` and `sessions.head` read through here; their tapes are the threads area's.

## Decisions

1. `newest` takes a callback answering `ControlFlow` in place of the TS lazy iterator; `atomically` takes
   `FnMut() -> Result<(), HostError>` (a trait object cannot be generic) and a caller carries its value out by capture.
2. Dropped (Proposal 3): `moveBlobs`, `MovedBlobs`, `indexOfBlob`, the `transcripts` and `transcript-index` blob names,
   and the statements only they used (`blobbed`, `blob`, `putBlob`, `dropBlob`).

## After the port

- An inner `atomically` whose error the outer one catches keeps its writes (no savepoint), so a caller that recovers
  from an inner failure commits half of it.
