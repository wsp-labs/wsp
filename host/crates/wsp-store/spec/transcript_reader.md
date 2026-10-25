# transcript_reader

Source: packages/runtime/src/transcript-reader.ts (81 lines) at 65f6b89c7. Tests: none of its own; the threads
area's head and history tests reach it (thread-head.test.ts, folder-threads.test.ts, rewind.test.ts). Rust tests:
tests/transcript_reader.rs. Expected values: tests/fixtures/store/transcript_reader.json.

## What it does

The read of one thread's events that a thread's head and a history page share, over a list or over rows; the one rule
for an event's stored JSON and the size the transcript caps count; and the shape an event takes in a head.

## Behaviour

1. `read_thread(events, t, o)` (transcript-reader.ts:33-40): `total` counts events whose `threadId` equals `t`; the
   pick walks the list from its end and is `pick_newest`.
2. `pick_newest(newest, o)` (:44-56), for each event newest first: skip it (and go on) when `before` is set and
   `pos` (0 when absent) `>= before`; `shaped = o.shape(e)` or a copy; `size` = UTF-8 bytes of `JSON.stringify(shaped)`
   plus 1; stop when `limit` events are picked, or when `strict` or at least one is picked and `bytes + size > o.bytes`;
   else take it. The answer is oldest first. Without `strict` the newest event always comes, so a page back always
   moves.
3. `event_json(e)` (:60-63): `json` is `json_stringify(serde_json::to_value(e))` and `bytes` is its length in UTF-16
   units (rule 2.11). `event_bytes(e)` is that `bytes`.
4. `head_shape(e)` (:77-81): an event that is not a `session.delta` of kind `tool_result` is returned as it is. A
   tool result whose text is longer than `HEAD_RESULT_CHARS` (2048) UTF-16 units gets the first 2048 units and
   `cut` = the whole length; one with a `patch` gets `kept_patch(patch, 2048)` (wsp-words), whose `patchCut: true`
   only where something was cut.

## Edge cases

- Six events of fixture `reads` (thread `a` at pos 1, 3, none, 6; `b` at 2; no thread at 4): limit 2 gives
  `[unnumbered, 6]`; limit 0 `[]`; before 6 `[1, 3, unnumbered]`; before 1 `[unnumbered]` (no pos reads 0); before 0
  `[]`; bytes 0 `[6]`; bytes 0 strict `[]`; a budget of exactly the newest two `[unnumbered, 6]`, one byte short `[6]`;
  strict with exactly the newest `[6]`, one short `[]`. `total` is 4 each time.
- `read_thread(events, "")` finds none: an event with no `threadId` is not thread `""`.
- Head: 2049 units cut to 2048 with `cut: 2049`; exactly 2048 unchanged with no `cut`; a text delta of 5000 unchanged;
  a patch of 40 lines of 100 units keeps 20 with `newLines: 20` and `patchCut: true`.
- `event_json` of a delta whose text is `😀 é \u0001`: the text the fixture holds, 116.

## Behaviour nobody wrote down

- Rule 8 (text units): `bytes` and the head cut count UTF-16 units; the page budget counts UTF-8 bytes. Three units
  in one file.
- The cut lands inside a surrogate pair when unit 2048 is the first half of one: the TS keeps the lone half
  (`zz\ud83d`). A Rust `String` cannot, so the half is dropped: 2047 units, `cut` still the whole length (2053), as the
  core-types slice ruled for `firstOf`.
- Key order is the `SessionEvent` struct's (wsp-wire), not the order the event was built in: `cut` and `patchCut` sit
  where the struct puts them, where the TS appends them last. Sizes do not change with key order.

## Tapes

`sessions.head` and `sessions.history` (the threads area's tapes).

## Decisions

1. `ThreadRead.limit` is `usize`, `usize::MAX` for `Infinity`; `shape` is a plain `fn`, `head_shape` being the only one.
2. `numbered` is dropped: it numbered events an older build wrote with no `pos` (Proposal 3).

## After the port

- A head cut can split an emoji (above).
