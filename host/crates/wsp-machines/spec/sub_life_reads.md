# sub_life_reads

Source: `packages/runtime/src/machines/sub-life.ts:1-154` (154 of 597 lines) at `65f6b89c7`. Tests:
`packages/runtime/test/sub-result-read.test.ts` (2 cases), and the row and keep halves of `sub-results.test.ts`,
`sub-disk-keep.test.ts` and `sub-resume.test.ts` (rows asserted through `sessions.list`). Rust: `src/machines/sub_life/reads.rs`.
Oracle: `oracle/machines_b_reads.oracle.ts` writes `tests/fixtures/machines_b/sub_life_reads.json`.

## What it does

The pure half of a sub-thread's life: the constants the timers use, the one shell command a done runs to read what the
machine holds, the parse of its answer into a `SubResult`, the two shell commands that pack a session file on a machine
and land it on a new one, the row a sub-machine's record shows (`SubRow`), and the day a cloud may delete a napping
machine's disk. Nothing here touches state, the clock or a process.

## Behaviour

1. Constants (sub-life.ts:16-30): `SUB_SESSIONS = "sub-sessions"` (a blob collection), `SUB_SESSION_MAX_BYTES =
   67_108_864`, `SUB_STOP_AFTER_DELETE_MS = 600_000`, `SUB_DELETE_RETRY_MS = 600_000`, `SUB_LINE_WAIT_MS = 300_000`,
   `PUSH_READ_EVENTS = 400`, `PUSH_READ_BYTES = 2_000_000`, `DONE_NAP = "its sub-thread is done"`. The first five are
   `pub` (other modules read them); the last three are `pub(crate)`.
2. `sub_result_command(folder, branch, base_sha)` (:36-46) is six lines joined by `\n`, each argument through
   `shell_quote`, byte for byte as the fixture's `commands`. The branch goes in as `refs/remotes/origin/<branch>`, the
   base as `<sha>^{commit}` on line 4 and bare on line 6.
3. `sub_result_of(stdout, branch, read_at)` (:49-75): split on `\n`, each line `js_trim`med (so `\r` and U+00A0 go),
   empty lines skipped; the key is everything before the first space (a line with no space is a key with value `""`),
   the value everything after it, `js_trim`med. A later line with the same key replaces the earlier (Map.set).
4. A `norepo` key anywhere wins over every count (:56): `{branch, pushedSha: null, commits: 0, unpushed: 0, changed: 0,
   norepo: true, readAt}`.
5. A count (:57-60) is JavaScript's `Number(value)`: present, not `""`, an integer and `>= 0`. So `0x10` is 16, `1e1` is
   10, `3.0` is 3, `-0` is 0; `-1`, `1.5`, `abc` and a missing key are no count. `unpushed` and `changed` are both
   required, else the answer is `None` (:63). `commits` defaults to 0. The stub's `js_number(&str) -> f64` is
   JavaScript's `Number()` on a string: `js_trim`, then `""` is 0, `0x`/`0o`/`0b` prefixes, decimal with exponent,
   `Infinity` with a sign, anything else NaN; never `str::parse` alone (it takes `+inf`, `nan`, `1_0` differently).
6. `pushedSha` (:68) is `None` (JSON `null`) for a missing key, `""` and `none`; any other word is kept as it is.
   `onBranch` (:72) is present only for a non-empty `branch` value (`HEAD` on a detached checkout is kept).
7. Key order of every result is `SubResult`'s struct order in wsp-wire (branch, pushedSha, commits, unpushed, changed,
   onBranch, norepo, readFailed, readAt, pushRefused), which is the order every TS literal writes.
8. `sub_session_pack_command(file, to)` (:79-86) and `sub_session_land_command(file, path, from)` (:90-97): four and
   three lines joined by `\n`, `-maxdepth` is `depth + 1`, `-name` is `*<name>`, byte for byte as `packs` and `lands`.
   The land command's folder is `path` up to its last `/` (`path.slice(0, path.lastIndexOf("/"))`).
9. `sub_row_of(record)` (:100-130): `None` with no `sub`. State: phase `released` or `gone` reads `released`; `queued`
   reads `waiting` when `record.queue` is set, else `starting`; `running` reads `starting` while `sub_spawning(spawn)`
   (a spawn with neither `readyAt` nor `failed`), else `running`; every other phase (`napping`, `creating`, ...) reads
   `napping`. Fields in order: `state, on, machine` (the record's name), `size {cpu, memMb, name?}` (name off
   `sub.sizeName`), `rateUsdPerHour?`, `branch`, `result?`, `held {files, commits}` (the record's `at` dropped),
   `deleteAt?`, `releasedAt?`, `cloudGone` only when exactly `true`, `sessionNotKept?`, `spawn?`.
10. `sessionNotKept` (:133-137): none for no session or `kept` not `null` (a number). With `kept: null`: `bytes >
    SUB_SESSION_MAX_BYTES` gives `sub_session_too_large_line(fmt_bytes(bytes))`; otherwise
    `sub_session_not_kept_line(why ?? "no reason was given")`. Exactly 64 MiB is the second.
11. `disk_kept_ms(ctx, record)` (:140-146): `backend_for(record).lifecycle().disk_kept_ms()`; `None` when `backend_for`
    errs, when the backend has no lifecycle, or the lifecycle names no keep.
12. `disk_kept_until(ctx, record, stopped_at)` (:150-154): from `nappedAt` when the phase is `napping` and `nappedAt`
    is set, else from `stopped_at`; `None` when there is no from or no keep; otherwise `from + ms`. A keep of 0 is a
    keep (gives `from`); `stopped_at` 0 is a from.

## Edge cases

- Empty stdout gives `None`; `"norepo\n"` gives the norepo result (fixture `parsed`).
- `"unpushed\t1"` is one key with no value, so `None`: only a space splits (fixture).
- `"branch feat x y"` keeps `feat x y`; `"unpushed  7 "` is 7 (fixture).
- A land path with no `/` cuts its last character for the folder: `nofolder.jsonl` gives `install -d -m 0700
  'nofolder.json'` (fixture `lands[2]`). Kept; see After the port.
- A `cloudGone: false` on a record (never written by the TS, but readable) is not carried on the row (fixture).

## Behaviour nobody wrote down

- Rule 8.6: every `trim()` here is `js_trim`, which takes U+00A0 and U+FEFF (fixture's last two `parsed` rows).
- Rule 4.26: key order. The row is compared as JSON text against the fixture, not as a struct.
- Rule 4.25: counts are `JsInt`; a parse of `-0` writes `0`.

## Decisions

1. `disk_kept_ms` and `disk_kept_until` take a `&dyn BackendFor` (one method, `backend_for(&WorkspaceRecord) ->
   Result<Arc<dyn MachineBackend>, HostError>`) in place of `Pick<RuntimeContext, "backendFor">`.

## After the port

- A land path with no `/` makes a folder named after the path less its last character (sub-life.ts:91). No caller
  passes one today (paths come from the pack's `find`, always absolute), so it is latent.
- `subResultOf` reads a count with `Number()`, so a machine answering `0x10` counts 16 commits. Harmless with git's
  output; a strict integer parse would be clearer.

## Tests (tests/sub_life_reads.rs)

| Rule | Test |
|---|---|
| 1 | `constants_hold_the_ts_values` |
| 2 | `result_command_is_byte_for_byte_the_ts_line` (fixture `commands`) |
| 3, 4, 5, 6, 7 | `parses_every_answer_as_the_ts_does` (fixture `parsed`, 17 rows, compared as JSON text) |
| 2, 3 | `counts_only_the_commits_made_on_the_machine_not_the_history_it_started_from` (real git, port of sub-result-read.test.ts:57) |
| 2, 3 | `still_answers_on_a_new_machine_that_fetched_its_pushed_branch_and_not_the_commit` (real git, :74) |
| 8 | `pack_and_land_commands_are_byte_for_byte_the_ts_lines` (fixture `packs`, `lands`) |
| 9, 10 | `rows_read_as_the_ts_rows` (fixture `rows`, 17 records, JSON text) |
| 11, 12 | `disk_keep_reads_as_the_ts_reads` (fixture `keeps`, 11 rows) |
