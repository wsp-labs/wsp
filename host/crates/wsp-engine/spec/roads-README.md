# Engine roads: rules every unit shares

The nine units here port six files of `packages/engine/src` read at `a4033dc34` (the dead-code cut's branch; these
six files and their tests are byte-identical to `a3b870a6e`). A `file:line` with no folder is under
`packages/engine/src`; a test cited as `ssh-backend.test.ts:40` is under `packages/engine/test`.

| Unit | Rust module | TS source | TS lines |
|---|---|---|---|
| ssh_dial | `src/ssh_backend/dial.rs` | ssh-backend.ts:1-375 | 375 |
| ssh_keys | `src/ssh_backend/keys.rs` | ssh-backend.ts:377-615 | 239 |
| ssh_machine | `src/ssh_backend/machine.rs` | ssh-backend.ts:617-969 | 353 |
| exec_put | `src/exec_detached/put.rs` | exec-detached.ts:1-227 | 227 |
| exec_run | `src/exec_detached/run.rs` | exec-detached.ts:229-399 | 172 |
| link_backend | `src/link_backend.rs` | link-backend.ts | 403 |
| fake_backend | `src/fake_backend.rs` | fake-backend.ts | 334 |
| place_machine | `src/place_machine.rs` | place-machine.ts | 263 |
| local_backend | `src/local_backend.rs` | local-backend.ts | 225 |

Tests: each TS test file is ported one to one (`tests/ssh_backend.rs`, `ssh_back_forward.rs`, `ssh_bytes.rs`,
`ssh_sudo.rs`, `link_backend.rs`, `exec_detached.rs`, `fake_backend.rs`, `place_machine.rs`, `local_backend.rs`), and
each unit has hard tests in `tests/<unit>_hard.rs` that compare against the TS code's own answers, recorded by
`oracle/roads.oracle.ts` into `tests/fixtures/roads/` (link_backend's three are at the end of `tests/link_backend.rs`). Shared stand-ins are in `tests/roads_common/`.

## Text

- Every cap on words (`SSH_LINE_CAP` 300, `boxWord`'s 48, `missingKnownHostsLine`'s 48 and 72, `HELD_TAIL` 4096,
  the `slice(-N)` tails) counts UTF-16 code units, as JavaScript's `slice` does. A cut never panics on a char
  boundary: where the cut would split a surrogate pair, the lone half is dropped (JavaScript would keep a lone
  surrogate, which a Rust `String` cannot hold; an accepted difference).
- `"…".trim()` is JavaScript's: it strips Unicode white space and line terminators (U+0009 to U+000D, U+0020,
  U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F, U+3000, U+FEFF). `str::trim` differs on U+FEFF
  and U+0085; use a JS-trim helper (wsp-words has `js`).
- A regex is JavaScript's: `\s` holds U+00A0 and U+FEFF, not U+0085; `\d` is ASCII; `\w` is `[A-Za-z0-9_]`.
- `split("\n")` keeps a trailing empty piece; `lines()` does not. Port `split` as `split('\n')`.
- `Buffer.toString("utf8")` replaces a bad sequence with U+FFFD: use `String::from_utf8_lossy`.
- `Number(x)` reads a decimal, `0x` hex, an exponent and surrounding white space, `""` as 0 and anything else as
  NaN. Where a unit reads a port or a count with `Number`, its spec says which of these matter.

## JSON

- serde_json with `preserve_order`. An object a unit writes keeps the TS insertion order, named in each spec.
- An optional field the TS leaves out with `...(x !== undefined ? { x } : {})` is absent from the JSON, never
  `null` (`skip_serializing_if = "Option::is_none"`).

## Errors

- Every refusal's words are exact; a test compares the whole sentence where the TS test does, and a substring where
  the TS test does.
- A refusal that the TS throws as a plain `Error` is `EngineError::Said(String)` (the machines writer names the
  type); one with a kind (`absent`, `missing`, the guest's) keeps that kind so `is_place_absent` and `is_missing`
  read it.

## Time

- Every bound is milliseconds as the TS gives it. A test may shorten a poll; nothing here sleeps in a test longer
  than the TS test does.

## What survives a host restart

Only what is on disk: the fake backend's records file (fake_backend), the ssh control folder and its sockets
(ssh_dial), the files `exec_put` and `exec_run` leave on a guest until cleanup, and what ssh itself writes
(known_hosts). Every cache in memory starts empty: `SshMachine`'s last warning, `LocalMachine`'s OS name,
`PlaceFolderMachine`'s login.
