# wiring-helpers: the functions wiring.ts shares

TS: `packages/runtime/src/types/wiring.ts:510-531, 799-1060` (about 270 lines) at `65f6b89c7`. Rust:
`src/types/wiring.rs`. Tests: `tests/wiring_helpers.rs` (17), against `tests/fixtures/core_types/wiring.json`. Strings
are counted and cut as JavaScript does, in UTF-16 units; none of these functions cuts mid-character.

## wired_place (wiring.ts:522-531)

One row over one backend. `WiredId::Fixed(id)` or `WiredId::Read(f)`: a read id is asked again on every call of
`wired`, `backend` and `list`, since a host started with no key swaps its provider in later. `backend(place)` is the
backend for exactly the current id and None for any other. `list()` is `[id]`. Pinned by
`wired_place_reads_its_id_at_each_call`, which checks the same `Arc` comes back (`Arc::ptr_eq`).

## settled, at_provider (wiring.ts:811, 953)

`settled` is true for `Settled` and `Moot` only. `at_provider(None, _)` is false; `Some(id)` is true exactly when
`places.list()` contains it, the empty name included. Pinned by `settled_is_settled_or_moot`, `at_provider_reads_the_list`.

## folder_named (wiring.ts:829-838)

The TS reads `homedir()` and `process.cwd()`; here they are the `home` and `cwd` arguments.

1. `"~"` or a word starting `"~/"`: `join(home, rest)`, where join is node's `path.join` (normalizes `.` and `..`,
   collapses slashes, and keeps a trailing one: `~/x/` joins to `<home>/x/`).
2. Else an absolute word (starts with `/`): **the word as written**, not normalized.
3. Else node's `path.resolve(cwd, word)`: normalized, absolute, a trailing slash dropped; the empty word is `cwd` itself; `"~foo"` is relative.
4. Then `realpath` of that; where realpath fails (missing, unreadable), the step 1 to 3 path itself.

Pinned by `folder_named_reads_home_cwd_and_links` and
`folder_named_keeps_an_absolute_path_that_does_not_resolve_as_written`.

## same_source (wiring.ts:842)

Two `git` sources compare their `url` strings exactly, two `folder` sources their `path` strings exactly (`/a` and `/a/`
differ). Any other pair is false, **two equal `github` sources included**, since the rule names only git and folder.
Pinned by `same_source_compares_the_url_or_the_path` and `same_source_never_matches_two_github_sources`; the github
case is an after-port question in the report.

## last_line_of (wiring.ts:849)

`said.trimEnd()` (JavaScript's whitespace, `\r` and `\t` included), split on `\n`, the last piece, trimmed both ends;
"" for an empty or all-whitespace input. `"a\r\nb"` is `"b"`. Pinned by `last_line_of_is_the_last_trimmed_line`.

## outcome_words, home_outcome (wiring.ts:851-907)

`outcome_word`/`home_word` give the two tables' words, None for `Failed`. `outcome_words(a)`:

- failed: `failed: <error, or "no reason given">`; a note is not said.
- moved with `rows` present: `moved, <plural(rows, "row")> merged` or, at 0 rows, `moved, its rows already there`,
  then ` (<note>)` where a note is present.
- moved with no rows: `moved`, and the note is not said.
- transcript-only: the word, then ` (<note>)` where present.
- carried, nothing: the word alone; a note or rows are not said.

`home_outcome(a)`: `<name>`, then ` (<plural(sessions, "session")>)` where `sessions` is present (0 included), then
a space, then the failed sentence or the home word, then `, <plural(skipped, "indexed rollout")> not under sessions/
skipped` where `skipped` is present and not 0. Pinned by `outcome_words_match_the_ts` and `home_outcome_matches_the_ts`
(11 and 8 cases from the TS).

## merge_on_machine (wiring.ts:873-892)

1. `dir` = the script path up to its last `/`.
2. `machine.run("python3 " + shell_quote(script), deadline 120 000 ms)`.
3. Whatever step 2 did, then `machine.exec("rm -f <q script>; rmdir <q dir> 2>/dev/null")`, its result and its
   failure ignored. Only then does a failed step 2 return its own error, unchanged (an `EngineError`).
4. A non-zero exit: the agent with `outcome: failed` and `error: the merge on the machine failed (exit <n>): <why>`,
   `<why>` the last line of stderr after trimEnd, or `no output` where that is empty. Other fields kept, the note too.
5. Exit 0: the engine's `parse_merge_output(stdout)`. A parse failure is `outcome: failed` with its message as `error`.
   `Waiting` keeps the outcome and sets `note` to the waiting words. `Merged` sets `outcome: moved`, `rows: merged`,
   and `note` only where the output had one (else the agent's own note stays).

The result is the input agent with those keys changed in place, so its JSON keeps the input's key order and appends new
keys at the end. Pinned by `merge_on_machine_matches_the_ts` (8 cases, each also checking the two commands and their
order).

## until (wiring.ts:917-930)

`ms = max(0, deadline - clock.now())`, then `clock.schedule(fire, ms)` once, before anything is polled. If the work
settles first, cancel the timer and answer as it settled (`UntilError::Failed(e)` for its error). If the timer fires
first, answer `UntilError::Deadline(DeadlineError("<what> timed out after <ms> ms"))` with the `ms` armed; the work is
dropped unpolled, which in Rust cancels it where the TS left it running: a caller that needs it to run on must spawn
it first. Pinned by `until_fails_on_the_clock_it_was_given`, `until_answers_the_work_and_cancels_the_timer` and
`until_past_its_deadline_waits_zero`, which check the clock's log: `schedule <ms>` then `cancel`, or no cancel when
the deadline won.

## shape_fault (wiring.ts:936-944)

For `cpu` then `memMb`, where both sides have the key and the values differ: `<key> <got> != <want>` with numbers
written as JavaScript writes them (`JsNumber::js_string`: `1.5`, `4096`). Joined with `, `; None where none differ.
Disk and createdAt never count. Pinned by `shape_fault_names_cpu_and_memory_only`.

## hostname_for, set_hostname (wiring.ts:956-977)

`hostname_for`: JavaScript `toLowerCase` (Rust `to_lowercase` agrees on every case pinned: `İ` becomes `i` plus a
combining dot, which is then replaced), every run of characters outside `[a-z0-9-]` becomes one `-`, leading and
trailing `-` removed, cut to the first 63 characters, trailing `-` removed again, `wsp` where nothing is left. Runs of
`-` already in the name are kept. Pinned by `hostname_for_matches_the_ts` (14 cases).

`set_hostname(machine, name)`: `host = hostname_for(name)`; one `exec("hostname <host> && echo <host> > /etc/hostname")`.
Exit 0 answers `{host}`. Otherwise `refused = "hostname <host> on <machine id> failed: <stderr trimmed>"`, where an exec
that failed reads as exit -1 with its error's message as stderr; the line is logged as a warning and answered, never
returned as an error. Pinned by `set_hostname_answers_the_refusal_rather_than_failing`.

## Constants

`PROVIDER_READ_MS` 30 000, `GONE_CONFIRM_MS` 5 000, `WAKE_LATE_READ_MS` 180 000, `DAEMON_HELLO_TIMEOUT_MS` 5 000,
`PORT_PROBE_TIMEOUT_MS` 10 000, `CLONE_MS` 300 000, `MERGE_DEADLINE_MS` and `LISTING_DEADLINE_MS` 120 000,
`PORT_PROBE_BODY_CAP` 2048, `VAULT_CAP_BYTES` 209 715 200, `VAULTS` "vaults", `VAULT_SKIP` [.local, .cache, .npm],
`GRACE_MS` 600 000, `DAEMON_REVIVE_AGAIN_MS` 300 000, `DAEMON_LACKS_AGAIN_MS` 3 600 000, the four usage timings, and the
two sentences. Pinned in `tests/wiring_records.rs` (`constants_*`).
