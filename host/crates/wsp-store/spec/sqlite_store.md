# sqlite_store

Source: packages/runtime/src/sqlite-store.ts (423 lines) at 65f6b89c7, less what Proposal 3 drops. Tests:
packages/runtime/test/store.test.ts (the sqliteStore cases of the round trip, persistence, shape and mode suites),
packages/runtime/test/sqlite-checkpoint.test.ts (5 with the `each`), packages/host/test/state-store.test.ts (the WAL
race, "ten times in ten"). Rust tests: tests/sqlite_store.rs, tests/sqlite_store_modes.rs (its own process: it sets the
umask), tests/sqlite_checkpoint.rs, tests/sqlite_race.rs (child processes). Expected values:
tests/fixtures/store/sqlite_store.json.

## What it does

The host's records and blobs in `state.db` beside the state file, in chapter 02's fresh schema (`SCHEMA`): the TS
host's statements less `migrations`, run once, in one transaction, on a database with no `shape` table. The store
opens at its first call, refuses a database in any other shape, and stamps the shape row at each process's first
write. A checkpointer on a thread of its own copies the WAL into the database; a commit past 64 MiB of WAL does it
itself. The connection belongs to the state owner's thread and runs synchronously (rules 2.20, 3.1): `SqliteStore` is
not `Send`, and every future it returns is ready at its first poll.

## Behaviour

1. `state_db_path(state)` is `dirname(state)/state.db` (sqlite-store.ts:39). The state file is never read, written or
   moved: no import (Proposal 3).
2. **Open** (`ready()`, sqlite-store.ts:289-337; rule 2.25), at the first call of any method, `pragma` and the
   transcript rows included, in this order:
   a. `state.db` missing: `build_database` (3).
   b. Open it; set `busy_timeout = 10000`; probe `select 1 from sqlite_master where type = 'table' and name = 'shape'`.
      An error in the open or the probe becomes `state_unreadable_line(db, <SQLite's message>)` (:300-306). A 0-byte
      file is an empty database and opens as fresh.
   c. With a `shape` table: refuse a wrong shape (7) **before** anything writes, so a refused file keeps its bytes.
   d. `journal_mode = wal`, `synchronous = normal`, `wal_autocheckpoint = ceil(WAL_BOUND_BYTES / page_size)` (16384 at
      4 KiB pages), `journal_size_limit = 16777216` (:311-315).
   e. `begin immediate`; probe again; with a `shape` table refuse a wrong shape, else run `SCHEMA` and insert the shape
      row `shape_now(writer)`; `commit` (:316-321). The second probe is what lets two processes open one fresh file at
      once: the one that waited finds the other's schema and runs none.
   f. Any error in c to e rolls back if a transaction is open, closes the connection and reaches the caller as SQLite's
      own message, unwrapped (:322-329). The next call tries the whole open again.
   g. Start the checkpointer (12). Nothing else (the TS moves a state file found beside the database aside, :335;
      dropped).
3. **Build** (`buildDatabase`, sqlite-store.ts:232-273 less the import): `own_folder(dirname(db))`; remove every
   `state.db.importing-<pid>[-...]` in that folder whose pid is this process's or a dead one (`kill(pid, 0)` failing
   other than EPERM; :216-225); make `state.db.importing-<own pid>` empty at 0600 with `O_EXCL`, an existing one kept as
   it is (:174-181); open it, `busy_timeout`, `journal_mode = wal`, close; `link(2)` it to `state.db`, returning quietly
   on EEXIST; remove the build file on every road out (:271). It writes no table: the open's step e does (rule 2.6).
4. **Values** (rule 2.27; sqlite-store.ts:350-383): `put` stores `json_stringify(value)` (wsp-wire, `JSON.stringify`'s
   text: numbers as JS writes them, index keys first) by upsert, keeping the row's rowid; `put(Null)` stores `null`;
   `get` parses the stored text afresh each time; a missing row is `None`; `delete` of a missing row does nothing.
5. `list` and `keys` are `order by rowid` (rule 2.5): first-write order, kept by a re-put; a deleted id written again
   goes to the end. An unknown collection is `[]`.
6. Blobs: `put_blob` upserts `(bytes, at)`; `get_blob` of a missing blob is `None`, of an empty one `Some(vec![])`;
   `stat_blob` is `{bytes: length(bytes), at}` or `None`. `at` is wall-clock milliseconds with a fraction, strictly
   increasing per store by at least 0.001: `last = max(now, last + 0.001)` (rule 2.9; :281-283). `last` starts at 0 in
   each store and dies with it.
7. **Shape** (sqlite-store.ts:155-170, 284-296; rules 2.19, 2.26): the row `json` is parsed; text that is not JSON is
   taken as the string itself; a value that is not a full `StateShape` gives `state_shape_unreadable_line(db, value)`.
   No row gives the older sentence with `none recorded`; then `refuse_other_shape(db, shape)`. Every call after the open
   reads `pragma data_version` and, when it moved since the last call (another connection committed), runs this check
   again, so a later call can refuse (:291-296). A refusal is returned by every method and changes nothing.
8. **Stamp** (sqlite-store.ts:339-346): the first `put`, `delete`, `put_blob` or `delete_blob` of each store upserts
   the shape row with `shape_now(writer)` before its own statement; reads never write it. A fresh database's open
   already wrote one (2e); the first write writes it again with a new `at`.
9. Each write is one autocommit statement (rule 2.8). Busy handling is `busy_timeout` alone, no retry (rule 2.7): a
   write waits up to 10 s on another connection's lock, then fails with SQLite's message.
10. A fresh database holds exactly the `SCHEMA` objects (tables `blobs collections events shape transcript_index`, index
    `events_thread`, no `migrations`) and one row, the shape (rule 2.29). The table and column names are a contract with
    the doctor, init and the GPUI app (rule 2.4).
11. Modes (rules 2.1, 2.3): `state.db` is 0600 because the build makes it so; `-wal` and `-shm` take its mode; the
    folder is 0700 when the build made or repaired it. Folders above it are not touched.
12. **Checkpointer** (sqlite-store.ts:73-131; rule 2.28): `checkpointer(db, options)` spawns through
    `options.checkpointer`. The real thread (`checkpoint_loop`) ticks every 2 s: file gone ends it Ok; it opens the file
    read-write without creating it (`SQLITE_OPEN_READ_WRITE`, no `CREATE`), sets `busy_timeout`, and runs
    `pragma wal_checkpoint(passive)` only when `data_version` differs from the last pass that copied every frame
    (`log == checkpointed`). A spawn that fails, or a thread that ends while the file is there, is made again 2 s later;
    after `CHECKPOINTER_TRIES` (3) in a row it gives up and calls `options.warn(checkpoints_stopped_line(db, why))` once,
    `why` the spawn error, the thread's `Err` text, or `the thread ended`. A thread that lived 20 s resets the count.
    Nothing the checkpointer does blocks the store's thread.
13. The WAL bound: with no checkpointer running, a commit that takes the WAL past 64 MiB checkpoints itself, and once a
    checkpoint has copied the WAL a commit cuts the file back to 16 MiB or less (`journal_size_limit`).

## Edge cases

- A `state.db` of text bytes: every call fails with `<db> does not read as a state file (file is not a database), ...`
  and the bytes stay (store.test.ts:408-416).
- A TS host's `state.db` (its seven tables, `migrations` rows, shape 2): the older sentence naming shape 2 and
  `this wsp reads shape 3`, bytes kept.
- Shape row `3`, `null`, `{"shape":3}`, or text `not json {`: the shape-unreadable sentence with that value.
- A `shape` table with no row: the older sentence, `none recorded`.
- A database with a `collections` table and no `shape` table: the open's `SCHEMA` fails with SQLite's
  `table collections already exists`, rolled back; the table's rows stay.
- `put` of `{"z":1,"a":2,"2":0,"n":1.5,"big":1e21,"small":1e-7,"neg0":-0,"whole":10.0}` stores
  `{"2":0,"z":1,"a":2,"n":1.5,"big":1e+21,"small":1e-7,"neg0":0,"whole":10}` (fixture `raw`).
- A string with U+2028, `\u0001`, `/` and quotes is stored as `"héllo   \u0001 / \"q\""` with U+2028 raw (fixture).
- Keys after put a, b, c, a again, delete b, put b: `[a, c, b]` (fixture).
- A state file `state.json` beside the database is left byte for byte and never read.

## Behaviour nobody wrote down

- Rule 3.3: the checkpointer and the warn sink run off the store's thread and hold nothing of it.
- What survives a restart: every row, blob and the shape row. What does not: `last` (blob stamps), `stamped` (each
  process stamps again), the seen `data_version`, the checkpointer.
- A row whose `json` does not parse fails that `get` or `list` with an error (the TS throws `SyntaxError`; the text
  differs and is not pinned).

## Tapes

None directly. Rule 2.23's measured fresh start (two records after boot) is the runtime's.

## Decisions

1. The checkpointer is a std thread watched by a supervisor thread; `CheckpointerSpawn` is the seam the TS tests
   reach by mocking `node:worker_threads`. `warn` is the seam for `console.warn`.
2. `pragma(name)` is added for tests to read the per-connection pragmas; the TS tests had no way to and did not.
3. Dropped (Proposal 3): `STORE_MODULES`, `StoreModule`, `migrate`, `stateMigratedByNewerLine`, the `migrations` table,
   `importedStatePath`, `moveAside`, `fileBlobs`, `writeOwnLeftover`, the import inside `buildDatabase`, and the move
   aside of a state file found beside the database.

## After the port

- The open probes `shape` before the WAL switch but a database with other tables and no `shape` fails on `SCHEMA` only
  after the WAL switch rewrote its header, so that file is changed though nothing was written to it.
- A stalled checkpointer thread (alive, never copying) is never replaced; only the WAL bound saves the file.
