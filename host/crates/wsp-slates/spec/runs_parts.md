# runs_parts
Source: slate-runs.ts:17-242 (types), 363-482 (records and key helpers), 484-489 (`asText`), 985-987 (`timeoutOf`) at
65f6b89c7. Tests: slate-runs.test.ts `stopAll quiet ... restart and rewind rewrite running records` (record half),
`then ... is part of the key`; oracle `oracle/helpers.out` (`key.*`, `stableJson`, `asText`, `timeoutOf`, `spent`).

## What it does
The shapes the runs table and the slate area pass between them, and the pure functions over them: the run record
and what a start keeps of the last one, what a restart and a rewind make of a running record, the start budget, the
approval key of a command, and how a value becomes a command's text.

## Behaviour
1. **The record is an ordered JSON object** (`RunRecord`, :20-37), `serde(transparent)` over `serde_json::Map`, so
   each road writes its keys in the TS's order (rule R4). Getters read `state`, `runs` (0 when absent), `why`,
   `startedAt`. Every literal below is the TS's order; a key in brackets is present only when the TS spreads it in.
   - held: `state, why, runs` (:711). Asleep tick: `[result fields], state, why, runs` (:889).
   - failed by `fail`: `state, why, exit: null, runs, endedAt` (:717).
   - running: `state, runs, startedAt` with no kept result; with one, `[result fields], state, runs, startedAt,
     refreshing: true` (`runningRecord`, :375-377).
   - finished: `state, [why], exit, err, [json], out, [lines], startedAt, endedAt, ms, runs, [cut: true]` (:791-809,
     819); `why` sits after `state` whether it came with the fields or from the timer's streak.
   - cancelled by `stop`: `state, why, exit: null, runs, [startedAt, ms], endedAt`, the two only when it was running
     and had a start time (:848-855).
   - ended from outside (`endedRecord`, :380-382): `state, why, exit: null, runs, [startedAt], endedAt`.
2. **Result fields** (`RESULT_FIELDS`, :366): `exit, out, err, json, lines, endedAt, ms, cut`, always in this order,
   whatever the record's own order; a key holding JSON `null` counts as present (`exit: null` is kept).
3. `last_result(record)` (:369-372): a running record answers its result fields only when `refreshing` is true; any
   other answers them only when it has `startedAt` and reads `done`, `failed` or `cancelled`; else none. A held,
   idle or never-started record keeps nothing.
4. `restarted_record(record, now)` (:385-387): a running record becomes `failed`, why `the host restarted while it
   ran`, its kept result dropped; any other record comes back unchanged. `rewound_record` (:390-392) the same with
   `cancelled` and `the thread was rewound`.
5. `spent(times, at)` (:462-465): keeps the stamps younger than 60,000 ms; 12 or more kept answers none (past the
   budget); else the kept stamps and `at` appended.
6. `stable_json(value)` (:467-474): arrays in order; objects with keys sorted by UTF-16 code units (JS `<`), each
   `JSON.stringify(key):value`; a leaf as `JSON.stringify`, absent as `null`. Numbers are written as JS writes them
   (rules 8.11 and 4.25): `20`, `2.5`, `1e+21`, never `20.0`.
7. `run_key(decl)` (the table's `key`, :662-668): the first 32 hex characters of the sha256 of the JSON array
   `[kind, cmd, [[name, stable_json(expr)] for env sorted by name], [stable_json(a) for args], stdin ? stable_json :
   null, on ?? "thread", cwd ?? "", stream === true, every ?? null, always === true, timeout ?? 60, once === true,
   ...then?, ...stable_json(files)?, ...stable_json(scripts)?]`. `confirm` is not in it. `stdin: null` (a literal
   null) differs from no stdin.
8. `as_text(value)` (:484-489): null is empty; a string itself; a number or bool as JS's `String`; else
   `JSON.stringify` in the value's own key order.
9. `map_strings(value, fn)` (:477-482): every string, in arrays and object values, through `fn`; keys untouched.
10. `timeout_of(decl)` (:985-987): `min(max(timeout ?? 60, 1), 600)` seconds, fractions kept (`2.5`).
11. Constants: `NO_FOLDER`, `HELD_APPROVAL` (= `SLATE_HELD_APPROVAL`, `needs your approval`), `HELD_BUDGET`
    (`started 12 times in a minute; press to run it again`), `HELD_PRESSED` (`pressed 12 times in a minute; it can
    start again in a minute`), held busy `4 runs are already running`, held asleep `the box was asleep, so this tick
    did not wake it; press to run it now`, `BACKED_OFF`, `SECRET_DOTS` (`••••`), `STARTS_PER_MINUTE` 12, running at
    once 4, lines kept 500, timer floor 10 s, backoff after 5 failures for 300,000 ms, close grace 1,000 ms, default
    timeout 60 s, env name `^[A-Za-z_][A-Za-z0-9_]*$` (:363-415).

## Edge cases
- `restarted_record({state:"running", runs:2, startedAt:1}, 9)` is exactly
  `{"state":"failed","why":"the host restarted while it ran","exit":null,"runs":2,"startedAt":1,"endedAt":9}`;
  a done record is the same record (test `stopAll quiet ...`).
- `spent` of 12 stamps at 0..11 answers none at 59,999 and keeps 1..11 plus 60,000 at 60,000 (oracle).
- Keys pinned against the TS (oracle): `{kind:"cmd", cmd:"true"}` is `6144e0b9c343b38f3a9a13eb194e63bf`;
  timeout 60 and `on: "thread"` give the same key as none; `confirm` does not move it; the full declaration of the
  oracle is `4fd3f52da753cc4eadb19e8406b302e1`.
- `as_text`: `[1,"a"]` is `[1,"a"]`, `{b:1,a:2}` keeps `b` first, `1e21` is `1e+21`, `-0` is `0`.
- `timeout_of`: 0 and -5 give 1, 0.5 gives 1, 601 gives 600.

## Behaviour nobody wrote down
- Rule 3.11: the env names are sorted for the key, the record's keys are not.
- Rule 8.8: the sort compares UTF-16 units, which differs from Rust's `str` order only past U+FFFF.

## Decisions
1. `RunRecord` and `RunResult` are ordered JSON maps, not structs: every road's order differs (rule R4), and the
   slate area stores and pushes them as JSON. The alternative, a struct with accepted-difference lines for eight
   roads, is named in the slice's report.
2. `CmdRunDecl` holds its expressions as `serde_json::Value` and keeps `stdin: null` apart from no stdin.
3. `RunInput` is an enum `Value(Value)` / `Secret(String)`.

## After the port
- None in these lines.
