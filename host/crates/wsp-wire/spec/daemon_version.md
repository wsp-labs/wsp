# daemon_version
Source: packages/protocol/src/wire/daemon-version.ts (838 lines, of which 170 are the DAEMON_CONTENTS table and 535 the version register's doc comment) at a3b870a6e. Tests: places.test.ts (4 cases), broker-contract.test.ts (1), protocol.test.ts (1); 4 more pinned here against what the TS module printed. cut-daemon-version.test.ts tests the landing's script, not this module, and is not ported. Rust: `src/wire/daemon_version.rs`; tests `tests/daemon_version.rs` (10).

## What it does
The daemon's protocol version and the content hash of what a deploy installs, the gates that name the first daemon serving each op, the state file's shape record, and the sentences that say a place's daemon is behind, too old for a kill or a slate's server, or already current.

## Behaviour
1. `DAEMON_VERSION` is `DAEMON_CONTENTS.length` (daemon-version.ts:716), 167 at a3b870a6e; the Rust reads `wsp_frames::numbers::DAEMON_VERSION`, the same number the landing's cut writes into numbers.rs. `DAEMON_CONTENT_SHA` is the table's last entry (:730), 64 lower-case hex. `DAEMON_ROOTS_PATH` is `/root/.wsp/roots` (:735), the value, never recomputed.
2. `place_daemon_behind(v)` (:766-769): None when v is None or `v >= DAEMON_VERSION`; else `daemon <v>, host <DAEMON_VERSION>`. 0 is a version (behind), not unknown.
3. `named_watch_gap(v)` (:793-796): None reads `daemon version unknown, host <N>`; below 136 is `place_daemon_behind(v)`; else None.
4. `leave_asks(v)` is `v.unwrap_or(0) >= 141`; `leave_takes(v)` is `>= 145` (:811, :818). They read the version a computer's own wsp says it was built with, never its daemon's.
5. `kill_too_old_of_line(name, procs)` (:829-830): the processes as `<pid> (<command>)` joined by ` and `, `it` for exactly one and `they` otherwise (an empty list writes nothing between "to" and "alone" and says they).
6. `StateShape` (:746-756): `shape` and `daemon` integers, `wsp`, `bin`, `at` strings, all required; `STATE_SHAPE` is 2. `state_writer_words` is `<bin> (wsp <wsp>, daemon <daemon>)`.
7. The gates are copied: STDIO 154, HTTP_POST 155, PIPE 159, NAMED_WATCH 136, SYS_UNWATCH 152, FS_FOLDERS 73, FORK_BRANCH 150, LEAVE_ASKS 141, LEAVE_TAKES 145; each is at most DAEMON_VERSION.
8. Every sentence is a plain format of its arguments, copied byte for byte (daemon-version.ts:822-838).

## Edge cases
- `place_daemon_behind(Some(DAEMON_VERSION + 1))` is None: a newer daemon is never behind.
- `kill_too_old_of_line("box", [])` is `TERM went to  alone: ...they started...` (two spaces).

## Behaviour nobody wrote down
- The register comment above `DAEMON_VERSION` is the protocol's history; the Rust keeps none of it, since the version is wsp-frames' number and the TS file stays the register until cutover.

## Tapes
None of its own: the version rides `daemon.hello` (DaemonEvent) and `places.list` rows. Held out: none.

## Decisions
1. `DAEMON_VERSION` is an i64 const off wsp_frames' u32, so the two cannot drift; `DAEMON_CONTENT_SHA` is copied as text.
2. The functions take the one field they read (`Option<i64>`) where the TS takes an object with that optional field.

## After the port
- The landing's cut script (scripts/cut-daemon-version.mjs) writes the TS table and wsp-frames' numbers.rs; once the TS host is gone it must write `DAEMON_CONTENT_SHA` here too, or the Rust host names a stale content hash.
