# store

Source: packages/runtime/src/store.ts (264 lines) at 65f6b89c7. Tests: packages/runtime/test/store.test.ts (34 cases,
of which the memory store's 2 and the refusal sentences of 4 port here; the JSON file store's 13 and the import's 5 are
Proposal 3's to drop). Rust tests: tests/store.rs. Expected values: tests/fixtures/store/store.json, written by
oracle/store.oracle.ts off the TS.

## What it does

The persistence port every area writes through (`Store`), the in-memory store tests inject in its place, and the four
sentences a state in another shape is refused with. The trait is async because the TS port is; the SQLite store
answers every call without waiting (rule 2.20). A record is a JSON value; a blob is bytes, one per `(collection, id)`.

## Behaviour

1. `Store` has the TS methods in Rust case: `get put list keys delete get_blob put_blob delete_blob stat_blob`, and
   `transcripts()`, `None` by default (store.ts:9-24). Errors are `HostError` with no kind: the TS throws a plain
   `Error(message)`, which classes as provider.
2. `shape_now(writer)` is `{shape: STATE_SHAPE, wsp, daemon, bin, at}` in that key order, `at` the ISO time of now with
   milliseconds and `Z` (store.ts:43; rule 2.26). `STATE_SHAPE` is 2, the TS host's own (rule 2.19).
3. The refusals, word for word (store.ts:48-74), with `S` this host's `STATE_SHAPE` and `<words>` =
   `<bin> (wsp <wsp>, daemon <daemon>)` (`state_writer_words`, wsp-wire):
   - newer: `<path> was written by a newer wsp (state shape <N>; this wsp reads <S>; written <at> by <words>): run that wsp, or move the file aside`
   - older: `<path> holds state in an older shape (none recorded | shape <N>, written <at> by <words>; this wsp reads shape <S>): move the file aside and this wsp starts a fresh one`
   - unreadable: `<path> does not read as a state file (<why>), so no wsp can read the records in it: move the file aside, or put back a copy a wsp wrote`
   - shape unreadable: `<path> has a $shape that is not a shape document (<JSON.stringify(document)>), so no wsp can say which build wrote it or in which shape: move the file aside, or put back the document a wsp writes`
4. `refuse_other_shape(path, wrote)`: `None` or `shape < S` gives the older sentence; `shape > S` the newer; `S` is Ok
   (store.ts:142-145). The older check runs first.
5. `memory_store()` (store.ts:76-112): `get` of a missing id is `None`; `put` stores a copy; `delete` of a missing id
   does nothing; `list` and `keys` of an unknown collection are empty.
6. The memory store's `keys` and `list` are JavaScript object key order (store.ts:89-92, `Object.keys`): ids that are
   array indexes (`"0"` up to `"4294967294"`, no leading zero) first, ascending as numbers, then the rest in first-write
   order. A re-put keeps an id's place; a delete and a put moves it to the end of its group. Fixture `memKeys`:
   `["1","2","4294967294","b","a","01","4294967295"]`.
7. The memory store's blob mark `at` is a write counter per store, from 1, moved by every `put_blob` of any id
   (store.ts:80,102); `bytes` is the blob's length. `delete_blob` drops the mark. An empty blob is `Some(vec![])` with
   `bytes: 0`.

## Edge cases

- `state_shape_unreadable_line` of the text `not json {` quotes it: `("not json {")` (fixture `shapeUnreadable[3]`).
- `refuse_other_shape(Some(shape 2))` is Ok (a TS host's state); `Some(shape 1)` is the older sentence and `Some(shape 3)` the newer.
- Mark after x, y, x: x `{bytes 5, at 3}`, y `{bytes 4, at 2}` (fixture `marks`).

## Behaviour nobody wrote down

- Rule 2.27: every read is the caller's own copy. The TS memory store hands out the object it holds (store.ts:83), so a
  caller's change would move the store; a Rust `Value` is always a copy, so that aliasing is not ported (decision 2).
- Rule 13.8, null and empty: a TS `put(undefined)` stores `undefined` in the memory store and deletes the row on SQLite.
  `Value` has no undefined, so no Rust caller can make either call.
- What survives a restart: nothing of the memory store.

## Tapes

None: the store serves no op of its own. Every op that writes a record reaches it; the harness diffs rows after a run.

## Decisions

1. `STATE_SHAPE` lives in wsp-wire (`daemon_version.rs`) as 2, the TS host's; this crate re-exports it, so the store
   and the refusals read the one number.
2. The memory store hands out copies (above).
3. Dropped with Proposal 3: `jsonFileStore`, `readStateFile`, `readJson`, `stateNotAnObjectLine` and `jsonKind`, and the
   `Data` type (store.ts:31,63-68,114-264).

## After the port

- The memory store's key order differs from SQLite's for array-index ids (`"2"` before `"b"` whatever was written
  first), so a test on the memory store can pass in an order the real store never answers.
