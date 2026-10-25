# internal-machines: stand-in machines, the create fingerprint, bodies and session files

TS: `packages/runtime/src/types/internal.ts:694-738, 864-1066` (about 250 lines) at `65f6b89c7`. Rust:
`src/types/internal.rs`. Tests: `tests/internal_machines.rs` (12), against `tests/fixtures/core_types/internal.json`.

## fingerprint (internal.ts:867-873)

The sha256, lowercase hex, of the JSON of the spec with `idempotencyKey` removed, the `createdAt` label removed from
`labels`, and `labels` then written **last**, as `{}` where the spec had none. The other keys keep the spec's order,
which in Rust is the wire `MachineSpec`'s serde order (zod's); numbers are written as JavaScript writes them. Two
attempts of one request (another key, another createdAt) give one hash; a spec read with labels first gives the hash
of labels last. Pinned by `fingerprint_matches_the_ts_hash`, against the TS's hashes. The hash is stored in
`PendingCreate.body` and compared across attempts, so a mismatch makes a replay a new create.

## pid_alive (internal.ts:881-888)

`kill(pid, 0)` succeeds, or fails with EPERM: alive. Any other failure (ESRCH): not alive. Pinned by
`pid_alive_reads_signal_zero` (this process; a reaped child). It sends no signal, so it needs no process group of its
own.

## The stand-in machines (internal.ts:966-1031)

- `dead_machine(id)`: kind sandbox, no stream url. `exec`, `run`, `snapshot`, `pause`, `resume`, `download_url`,
  `upload_url` fail with `EngineError::Provider(WspError { kind: Missing, status: 404, message: "machine <id> is gone",
  .. })`, which is what a wake settles gone on; `kill` succeeds; `state` answers `gone`. Every optional method keeps
  the engine's default refusal. Not absent. Pinned by `a_dead_machine_answers_gone_and_refuses_the_rest`.
- `absent_machine(id, kind, away)`: every call, `kill` and `state` included, fails with a fresh `away()`, asked once
  per call; `is_absent_machine` is true for it and false for every other handle (the TS marks it with a symbol; Rust
  may use a marker type and `Any`, or a flag the trait can read). Pinned by
  `an_absent_machine_refuses_every_call_with_the_away_error`.
- `released_backend()`: the engine's `NoProviderBackend` with `RELEASED_LINE`. A record whose machine was deleted at
  its cloud stands on it, so nothing asks that cloud anything.

`OPTIONAL_BACKEND_CALLS` and `forwardedCalls` (internal.ts:1055-1066) bind a JS module's prototype methods onto a
handle. The Rust `MachineBackend` trait carries the optional calls as default methods, so a handle that wraps a backend
forwards them by delegating each; there is no `forwarded_calls` function. The constant is kept for the names.

## read_body_up_to (internal.ts:1034-1051)

Pull chunks while fewer than `cap` bytes have come; stop pulling at once when the count reaches `cap` (a chunk past it
is never pulled), when the body ends, or when a chunk fails (the failure is swallowed). Answer the bytes cut at exactly
`cap`, decoded as UTF-8 with each broken sequence replaced (`ab` plus half an emoji reads `ab\u{FFFD}`). `cap` 0 pulls
nothing. The TS also cancels the reader; in Rust dropping the stream does that, and the caller owns it. Pinned by
`read_body_up_to_matches_the_ts` (7 cases) and `read_body_up_to_pulls_no_chunk_past_the_cap`.

## Session files (internal.ts:694-738)

- `find_session_file(folder, name, depth)`: None where the folder does not list. Depth 0: the first file of the
  listing whose name ends with `name`, in the order the directory lists. Deeper: the sub-folders sorted **descending by
  JavaScript `localeCompare`** (ICU root collation, not byte order: `Z9, z9, z10, f, é, e, B, b, A, a, 2026-9-30,
  2026-10-01, _z`, punctuation lowest, case and accents only breaking ties, digits compared as characters), the first
  found winning. Pinned by `find_session_file_walks_folders_in_locale_order` and
  `find_session_file_follows_node_locale_order`, which walks node's whole sorted list. Depth 0 takes the directory's
  own order, as node's `readdirSync` does; the tests give it one match.
- `size_of(path)`: its size, -1 where it does not stat (`size_of_is_minus_one_for_a_missing_file`).
- `stamp_session_file(at, known)`: None where the agent names no file; else the path (`known`, never searched for, or
  the one found) with its size, or `{path: None, size: -1}` where none was found.
- `same_session_file(at, stamp)`: true where either is None; else stamp again (with the stamp's path as `known`) and
  compare path and size. A file appearing where none was is a change. Pinned by
  `a_session_file_stamp_holds_while_path_and_size_hold`, `a_known_path_is_stamped_without_a_search`.
