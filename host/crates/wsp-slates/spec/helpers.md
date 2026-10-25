# helpers
Source: slate-read.ts (37), slate-message.ts (27), slate-writes.ts (25), slate-refused.ts (19) at 65f6b89c7. Tests: none
direct in TS; the slates.test.ts cases that reach them (`a read carries each run's json`, `a refused write names
every error`, `a read asked for a run or a value without its $`). Oracle: `oracle/helpers.out`, the TS run under node.

## What it does
Four pure leaves of the slate area. `slate_read` cuts a run's record for a read and names a path that can only read
null. `slate_message` makes a carried value safe for a send's `slate:` line and finds and cuts its longest string.
`slate_writes` is the agent's write budget per thread. `slate_refused` builds the one-line refusal of a write the
parser or validator refused.

## Behaviour
1. `is_run_record(v)`: an object (not null, not an array) whose `state` is a string and `runs` a number (read:6-7).
2. `READ_OUTPUT_CHARS` = 4,000 UTF-16 units (read:10).
3. `run_shown(rec, name, named)` (read:14-28): not a record answers `rec`, or `null` when absent. `named`: only the keys
   `state, why, exit, ms, runs, stale, refreshing, endedAt`, in the record's own key order. Otherwise every key in
   order, except `out` is left out when the record has the key `json` at all (a `json: null` counts); a string
   `out` or `err` over 4,000 units becomes its first 4,000 units, then `… (N characters; values ["$name.field"]
   reads it whole)` (U+2026, a space, N the whole length); `lines` keeps its last 50; anything else is kept as is
   (a non-string `out` too).
4. `misread(doc, path)` (read:31-37): a path not matching `^[A-Za-z_]\w*(\.\w+|\[\d+\])*$` answers none (so any
   `$`-path, `len(...)`); its root is the text before the first `.` or `[`. In this order: a run of the document
   gives `X401` with message `<path> is not a source: $<root> is a run, read as $<path>; its record is also under
   runs.$<root>` and fix `$<path>`; a value or a derived value gives `<path> is not a source: $<root> is a value,
   read as $<path>` with the same fix; a root that is no `SLATE_SOURCES` name gives `<root> is not a source and not
   this slate's; a slate's own names start with $` with no fix; else none. With no document only the last check
   runs. The problem is `slate_problem("X401", ...)`: keys `code, name ("path-unknown"), message, fix`.
5. `defanged(value)` (message:6-11): in every string, at each line start (multiline), optional whitespace, `slate`,
   optional whitespace, `:` that is followed by optional whitespace and `{` becomes the leading whitespace, `slate`
   and U+FF1A `：`; the whitespace between `slate` and the colon goes. Strings inside arrays and object values are
   walked; keys and non-strings are not touched.
6. `longest(value, at)` (message:15-19): a string answers its own path and its length in UTF-16 units; an array or
   object answers the longest of its children in order, the first one on a tie; a number, bool, null, an empty
   array or object answers none.
7. `cut_at(value, at, keep)` (message:21-27): at the end of the path a string becomes its first `keep` units then
   ` (cut)`; anything else is unchanged. In an array only the element whose index equals the head is walked; in an
   object the key keeps its place.
8. `write_budget(now)` (writes:2-25): per thread a bucket `{tokens: 20, at, hour: []}` made at the first spend.
   `spend` refills `tokens = min(20, tokens + (at - was) / 1000 * 5)`, moves `at`, drops hour stamps 3,600,000 ms
   old or older, keeps the bucket, then: tokens below 1 answers `ceil((1 - tokens) / 5)` seconds; else 600 stamps in
   the hour answers `ceil((3,600,000 - (at - oldest)) / 1000)`; either way nothing is taken. Otherwise one token is
   taken, `at` stamped, and none is answered. `delete(thread)` answers whether a bucket was there.
9. `invalid(errors, warnings)` (refused:8-19): a `HostError` with kind `invalid`, `errors` the first 20 as JSON,
   `warnings` all, no fix, message `slate refused: N error[s]: ` then the one error said alone, or each of the first
   20 as `i) <said>` joined by a space, then ` And M more.` past 20. `said` is `[line L][, piece[.prop]]: ` (only
   the parts present, joined by `, `, the colon only when either is) then `code name message` then
   `. Did you mean <fix>?` when a fix is given.

## Edge cases
- `run_shown(undefined, ..)` is `null`; `{state:"x", runs:"1"}` is not a record and comes back whole (oracle).
- An `out` of exactly 4,000 units is not cut; 4,001 is (oracle `runShown.cut4000`, `runShown.cut`).
- `cut_at("abcdef", [], 0)` is ` (cut)`; `cut_at(5, [], 3)` is `5` (oracle).
- `longest({a:"xx", b:"yy", c:["zzz","q"]})` is `c[0]`, 3 (oracle).
- Budget at one instant: 20 spends pass, the 21st and 22nd answer 1; 100 ms later still 1 (half a token); 200 ms
  later a whole token is back and it passes (oracle `budget.*`). One spend a second: the 601st answers 3,000, and at
  3,600 s the oldest has aged out and it passes.
- `invalid([], [])` reads `slate refused: 0 errors: ` with the trailing space (a write whose parse gave no document
  and no error calls it so).

## Behaviour nobody wrote down
- Rule 8.21: `READ_OUTPUT_CHARS`' cut writes U+2026 here and three ASCII dots at slates.ts:1246; both kept.
- Rule 8.7: the three regexes are ported by hand; none relies on a surrogate half.

## Decisions
1. `defanged`, `longest` and `cut_at` take and answer `serde_json::Value`; object order is kept (`preserve_order`).
2. A path segment is `PathSeg::Key(String)` or `PathSeg::Index(usize)`.

## After the port
- `cut_at` on an object whose key is missing writes the key as `undefined`, which JSON drops; only `longest`'s own
  paths reach it, so it never happens.
