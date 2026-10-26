# 02 State and storage

What the TS host keeps, where, in what shape, and what it rebuilds at start. The Rust host starts with fresh state and no migrations (the map's locked point 3), so this chapter describes today's layout, says which parts are a contract other processes read, and lists the upgrade code that does not port. Every `file:line` is at `a3b870a6e`.

## The state folder

The path a host is pointed at is still called `state.json`, but it is only an anchor: the records live in `state.db` beside it (`packages/runtime/src/sqlite-store.ts:39`), and every other file is `join(dirname(statePath), name)` (`packages/host/src/host-lock.ts:132-186`), so two state files on one computer are two hosts with nothing shared. Which state file a process uses: `--state`, else `$WSP_HOME/state.json` (an empty `WSP_HOME` counts as unset), else `<cwd>/.wsp/state.json` when the cwd's own `package.json` is named `wsp` (parent folders are not searched), else `~/.wsp/state.json` (`packages/host/src/cli/state.ts:29-55`; `packages/host/src/serving-home.ts:15-24`). The pick is `resolve()`d against the cwd, with no realpath (`cli/state.ts:64-68`). When a named path and a checkout disagree, stderr says `this runs against <chosen>, named by --state|WSP_HOME; the checkout in <cwd> marks <dev>, which this run does not use.` (`cli/state.ts:48-55`).

One writer makes nearly every file: `writeOwn` (`packages/own-file/src/index.ts:18-41`) makes the folder and every folder between it and the target with `mkdir -p` at 0700 and chmods any whose mode is not exactly 0700, writes `<target>.<pid>-<ms>-<up to 6 base36>.tmp` (`Math.random().toString(36).slice(2, 8)`, which can be shorter) with mode 0600, chmods it, and renames it over the target. It never fsyncs. A failed write or rename throws the raw fs error and leaves the `.tmp` behind.

| Name, beside the state file | Holds | Written by | Mode | Read by |
|---|---|---|---|---|
| `state.db`, `-wal`, `-shm` | every record (below) | `sqliteStore` (`sqlite-store.ts:278-384`) | 0600 | the host; `wsp doctor` and `wsp init` open their own connections (`cli/commands.ts:347,368`, `cli/init.ts:251,296`); the GPUI app, read only (rule 2.4) |
| `host.lock` | `{pid, port, address, startedBy, startedAt}`, compact JSON, no newline | `takeLock` by hard link, `rewriteLock` by rename, both `writeFileSync`, not `writeOwn` (`host-lock.ts:259-280,356-360`) | umask | every client's dial; a `sed` in a shell reader (`serving-home.ts:49`); `wsp-mcp/src/aim.rs:57-68`, which needs `pid`, `port` and `startedAt` and takes `address` as optional |
| `host-token` | 24 random bytes, base64url, no newline, new at every start | `cli/serve.ts:311-313`, `writeOwn` | 0600 | every client's dial (`host-lock.ts:143-149`) |
| `host.log` | the host's stdout and stderr when a verb or the service started it | the parent opens it for append (`host-start.ts:73-90`; `service.ts:215-216,299-300`) | umask | `service.ts:594-630` |
| `door-port` | `<port>\n` of the door bound for `--port 0` | `server.ts:594` | 0600 | `server.ts:224-231` |
| `runs/` | one turn's files: `<12 hex>.d/` (the claim), `.sh`, `.in`, `.held`, `.log`, `.pid`, `.exit`, `.agent`, `.pump`, `.late`, `.late.key`, `.late.went` | `runtime/src/local-exec.ts:244-246,705-748` | folder 0700 when this call makes it (an existing one is not repaired), `.d` 0700; `.sh .in .held .log .late .late.key` 0600 created exclusive; `.pid` (`<pid>\n`, `:748`) at the umask; `.exit .pump .agent` written by the script or the agent at the umask (`:716-718,736`) | the host after a restart (06) |
| `roots` | the folders this computer's daemon may serve, one per line; one empty line when there are none | a shell `mkdir -p <dir>; printf '%s\n' <roots...> > roots.next.$$; mv -f` through the machine's exec (`runtime/src/daemon-roots.ts:9-19`) | umask | the daemon (`--roots-path`) |
| `readings/`, `inbox/` | the daemon's readings and landed files | the daemon | | |
| `place-host-key.json` | the host's ed25519 pair, never rotated | `places/this-computer.ts:23-46` | 0600 | `places.ts:33` |
| `relay.json` | the relay record | `account.ts:76-82` | 0600 | `cli/serve.ts:278`, `aim.rs` |
| `.env`, `servers.env`, `servers.owners.json` | provider keys and switches; MCP server values by name | `env-keys.ts:77-160` | 0600 | at start and at every launch, never cached (`cli/keys.ts:158-167`) |
| `slates.secrets.json` | slate secrets marked keep, plaintext | `runtime/src/slate-runs.ts:557-564` | 0600 | once at start |
| `slates/<threadId>/` | a slate's files, `$SLATE_DIR` | `runtime/src/slate-files.ts:20-29` | 0700, 0600 | slate runs |
| `connector.pid`, `cloudflared-<version>` | the relay connector | `connector.ts:62-84,142-223` | umask, 0755 | |
| `thread-bin/`, `docker-notes/` | the `wsp` and `docker` shims a thread's PATH holds, and shell startup files | `shim.ts:14-42`, rewritten at every host build | `wsp` and `docker` 0755 (`:37,39`); the folder and the startup files at the umask (`:35,40`) | turns |
| `icons/`, `project-icons/` | fetched icons | `server-icons.ts:139-146` (a `.tmp` and a rename, not `writeOwn`), `runtime/src/projects/images.ts:95-133` | 0600 | |
| `broker/<day>.jsonl`, `revoked.json` | the broker's log (14 days) and off switch | `broker/log.ts:58-70`, `broker/off.ts:27-50` | 0600 | |
| `recipes/<slug>.toml`, `release.json`, `agent-latest.json` | recipes, the last release read, agent versions | `recipes.ts:117`, `release.ts:195`, `agent-latest.ts:84` | 0600 | |
| `worktrees/<projectId>/<branch>` | worktrees | the daemon's copy road (07) | | |
| `recipe.json`, `history-cache.json`, `golden-import.json`, `golden-recipe.json`, `init.log` | `wsp init`'s working files, not the serving host's | `recipe-file.ts:14,21,59`, `init-import.ts:849`, `init-recipe.ts:262`, `init-log.ts:41` | umask (`writeFileSync`) | `wsp init`; the cut (#2441) decides which survive |
| `state.json.imported`, `.imported.<ms>` | a state file found beside an existing `state.db`, moved aside | `sqlite-store.ts:42-49,333-335` | | nothing; dropped with the import (Proposal 3) |

Outside the state folder: `hosts/<alias>.json`, `relay-client.json`, `device-key.json`, `ssh/` and `ssh_config` under the wsp home (`hosts.ts:52-120`, `account.ts:98-131`, `ssh-files.ts:40-97`); `host-id` and `analytics-id` in the OS config folder, kept off the wsp home on purpose (`runtime/src/host-id.ts:10-37`); agent MCP configs and skill copies, rewritten at start for agents whose wsp entry names this state (`mcp-install.ts:168-276`; the config write is a `/bin/sh` line with `umask 077`, `mktemp`, a `cksum` check and `mv -f`, `catalog/src/config-write.ts:62-96`); the service unit (06); and the local daemon's token folder `<tmpdir>/wsp-local-daemon-<pid>-*` (`local-daemon.ts:185-189`).

2.1. Every file the Rust host writes beside the state file MUST have the name, content and mode in the table above. Files a client or another process reads (`host.lock`, `host-token`, `door-port`, `relay.json`, `hosts/*.json`, the MCP configs, the skill copies) are a contract with programs the port does not change.

2.2. `host.lock` MUST be compact JSON with the keys in the order `pid, port, address, startedBy, startedAt`, no spaces and no trailing newline, with `startedBy` absent (not `null`) when unset: a shell reader takes the pid with `sed -n 's/.*"pid":\([0-9]*\).*/\1/p' "$1/host.lock"` after `[ -f "$1/host.lock" ]`, then `kill -0` (`serving-home.ts:49-50`). The order comes from `{ pid, ...ports, startedAt }` with `ports = { port, address, ...(startedBy) }` (`host-lock.ts:261`; `cli/serve.ts:271`), and the rewrite `{ ...lock, port, address }` keeps it (`cli/serve.ts:356`). 06 has its protocol (hard link, takeover markers, the liveness check).

2.3. The Rust host MUST write through one function with `writeOwn`'s behaviour: every folder from the state folder to the target made or repaired to 0700, a temp file 0600 beside the target, rename, no fsync, the temp file left behind on a failed rename. The files the table marks umask stay at the umask.

2.22. Each JSON file MUST keep its text form, because people and other programs read them (`account.ts:77`, `hosts.ts:119`, `env-keys.ts:80`, `keys/src/index.ts:36-42`, `connector.ts:182`, `release.ts:195`, `agent-latest.ts:84`, `slate-runs.ts:563`, `broker/off.ts:46`):

| Form | Files |
|---|---|
| `JSON.stringify(v, null, 2)` plus `\n` | `place-host-key.json` (keys `publicKey`, base64 SPKI DER, then `privateKeyPem`), `relay.json`, `relay-client.json`, `device-key.json`, `hosts/<alias>.json`, `servers.owners.json` |
| compact, no newline | `host.lock`, `slates.secrets.json`, `revoked.json` (an array, at most 1,024 ids, `broker/off.ts:11`) |
| compact plus `\n` | `release.json`, `agent-latest.json`, `connector.pid` (`{"pid":N,"startedAt":"<ISO>"}`) |
| text | `door-port` `<port>\n`; `host-token` 32 base64url characters, no newline; `host-id` `<8 hex>\n`; `.env` `KEY=value` lines ending in `\n` |

2.23. A start MUST leave the state folder as the TS host leaves it. Measured on a throwaway host (`env -i`, its own `--state`, `--port 0`, umask 077, 2026-10-25): after the first start the folder holds `host-token`, `host.lock`, `place-host-key.json`, `state.db` with `-wal` and `-shm`, and `thread-bin/`, and nothing else; `door-port` appears only when the place door first opens on port 0 (`server.ts:594`), and `release.json` 30 s after start (06, rule 6.26). The fresh database holds two records and nothing more: `owner/id` = `{"id":"h_<8 hex>"}` and `reads/since` = `{"at":<ms>}` (`machines/boot.ts:331-342`). Across a restart on the same state: `host-token` is new (32 characters every time), `place-host-key.json` is kept (a file that does not parse is silently replaced by a new pair, `places/this-computer.ts:34-45`), `door-port` is kept and reused when it holds an integer from 1 to 65535 (`server.ts:224-231`), and `host.lock` is removed by a clean stop and left by a SIGKILL (06).

```text
$ stat -c '%a %n' *        # after the first start
600 host-token
600 host.lock
600 place-host-key.json
600 state.db
600 state.db-shm
600 state.db-wal
700 thread-bin
# select collection, id, json from collections   (python3's sqlite3, read only)
('owner', 'id', '{"id":"h_0a993e4c"}')
('reads', 'since', '{"at":1792923718163}')
```

2.24. `host-token` MUST be read trimmed (`hostTokenFor`, `host-lock.ts:143-149`; `aim.rs:185-189`), and is never deleted, not by a clean stop nor by a failed one (`cli/serve.ts:390-397`). A client that finds a live lock and no token file refuses as auth, exit class 2, with `the host's token file is missing: <path>`; with no live lock, `no wsp host is serving <state>; start one with wsp up --state <state>` (`verbs/client.ts:150,153,166`). An empty token file reads as `""` and fails the auth frame.

## SQLite as the TS host uses it

The TS host opens `state.db` with node's built-in `node:sqlite` (`DatabaseSync`, no options; `sqlite-store.ts:184-185`), synchronously, on the event loop: a `put` is in the WAL when its promise resolves. Node 22.23.4 bundles SQLite 3.53.4.

The whole schema, verbatim (`sqlite-store.ts:28-33,137`; `runtime/src/sqlite-transcripts.ts:10-19`):

```sql
create table collections (collection text not null, id text not null, json text not null, primary key (collection, id));
create table blobs (collection text not null, id text not null, bytes blob not null, at real not null, primary key (collection, id));
create table shape (one integer primary key check (one = 1), json text not null);
create table if not exists migrations (module text primary key, version integer not null);
create table events (workspace text not null, pos integer not null, thread text not null, type text not null, size integer not null, json text not null, primary key (workspace, pos));
create index events_thread on events (workspace, thread, pos);
create table transcript_index (workspace text primary key, json text not null);
```

So the store is a JSON bag: every record is one `collections` row whose `json` is `JSON.stringify(value)`, every byte string one `blobs` row, and only transcript events have columns. The store port over it (`runtime/src/store.ts:9-31`) is `get`, `put`, `list`, `keys`, `delete`, `getBlob`, `putBlob`, `deleteBlob`, `statBlob` and, on SQLite only, `transcripts: TranscriptRows` (`sqlite-transcripts.ts:35-71`).

Pragmas, all of them (`sqlite-store.ts:53-66,92,186,259,311-315`): `busy_timeout = 10000` on every connection; `journal_mode = wal`; `synchronous = normal` on the main connection; `wal_autocheckpoint` = 64 MiB in pages; `journal_size_limit = 16777216`. `foreign_keys`, `cache_size`, `mmap_size` and `user_version` are never set. A worker thread with its own connection runs `pragma wal_checkpoint(passive)` every 2 s when `data_version` moved (`sqlite-store.ts:59,78-131`).

2.4. The table names `collections`, `blobs`, `events` and `transcript_index` and their columns are read by other processes: `wsp doctor`, `wsp init`, and on `gpui/main` the GPUI app, which opens `state.db` with `SQLITE_OPEN_READ_ONLY` and runs `select exists (select 1 from collections where collection = 'places')`, counting a file it cannot read as joined (`gpui/main:apps/desktop-gpui/host-client/src/start.rs:659-661`). Under Proposal 3 they stay; a change to them is a change to those readers in the same ticket.

2.5. `list` and `keys` MUST return rows in `rowid` order, which is first-write order and survives updates; an id deleted and written again takes a new place at the end (`sqlite-store.ts:410-422`). Every listing a client sees that is not sorted on purpose inherits this order.

2.6. A database MUST be switched to WAL on a private file before it takes the name `state.db`: build `state.db.importing-<pid>`, set WAL, close, then `link(2)` it to `state.db`; the loser of a link race returns and opens the winner's file (`sqlite-store.ts:232-273`). Two processes switching one file to WAL at once fail one of them without calling the busy handler (trap #1785; `packages/host/test/state-store.test.ts:146-165`, "ten times in ten").

2.7. Busy handling is `busy_timeout` and nothing else: no retry, no catch on `SQLITE_BUSY` (`sqlite-store.ts:51-53`). A statement that times out rejects to its caller, and most callers log and carry on (`threads/agents.ts:118`, `status.ts:448`).

2.8. Each `put`, `delete`, `putBlob` and `deleteBlob` is one autocommit statement, run at once. Transactions are `begin immediate` in two places only: the migrate-and-stamp step at open (`sqlite-store.ts:316-325`) and `atomically` for transcript writes, where nested calls join the outer one (`sqlite-transcripts.ts:83-99`). (A plain `begin` also runs in `buildDatabase` for the import, `sqlite-store.ts:243`, which does not port; `putIndex` outside a flush is one statement.)

2.9. A blob's `at` is a real number of milliseconds, `performance.timeOrigin + performance.now()`, made strictly increasing by at least 0.001 per write (`stamp = last = Math.max(now, last + 0.001)`, `last` starting at 0 per store; `sqlite-store.ts:281-283`), so `statBlob` tells two writes apart even within one millisecond.

2.25. The store MUST open lazily, at its first call (`ready()`, `sqlite-store.ts:289-337`), in this order: build `state.db` if absent (rule 2.6), else open it with `busy_timeout`; probe for the `shape` table, turning any failure into `stateUnreadableLine` (`:300-306`; `store.ts:60-61`): `<db> does not read as a state file (<sqlite message>), so no wsp can read the records in it: move the file aside, or put back a copy a wsp wrote`. A file that is not a database fails here; a 0-byte file is an empty database and opens as fresh. Then refuse a wrong shape **before** the WAL switch, so a refused file is left untouched; then the pragmas, `begin immediate`, the shape check again, the stamp, `commit`, rolling back and closing on any error, which reaches the caller as SQLite's own message (a read-only file fails at the WAL switch this way). Every later call compares `pragma data_version` with the last one seen and runs the shape check again when another connection wrote, so a later call can refuse too (`:291-296`).

2.26. The shape row MUST be `JSON.stringify({shape, wsp, daemon, bin, at})` in that key order (`store.ts:43`; `host/src/version.ts:16`), measured as `{"shape":2,"wsp":"0.3.8","daemon":167,"bin":"<path to bin.js>","at":"<ISO>"}`. Each process's first write rewrites it with its own writer and a new `at`; reads never write it (`writing()`, `sqlite-store.ts:339-346,421`). The refusals, word for word (`store.ts:48-54,73-74`):
- no row, or a shape below the reader's: `<db> holds state in an older shape (none recorded | shape N, written <at> by <bin> (wsp <wsp>, daemon <n>); this wsp reads shape <S>): move the file aside and this wsp starts a fresh one`;
- a shape above it: `<db> was written by a newer wsp (state shape N; this wsp reads <S>; written <at> by <words>): run that wsp, or move the file aside`;
- a row that is not JSON or not a shape document: `<db> has a $shape that is not a shape document (<its JSON>), so no wsp can say which build wrote it or in which shape: move the file aside, or put back the document a wsp writes`.

The host throws these and goes no further; it never moves a file itself.

2.27. The store's values MUST behave as the TS store's (`sqlite-store.ts:350-422`): `get` of a missing id is `undefined` (Rust `None`); `put` stores `JSON.stringify(value)`, and a value that stringifies to `undefined` (the value `undefined`, a function) deletes the row instead, while `put(null)` stores the text `null`; `put` is an upsert that keeps the rowid; `delete` of a missing row does nothing; `list` and `keys` of an unknown collection are `[]`; `getBlob` and `statBlob` of a missing blob are `undefined`; `statBlob` is `{bytes: <length>, at}`; `putBlob` is an upsert that keeps the rowid and takes a new `at`. Every read parses the stored text afresh, so no two callers share an object, and a record's key order is the order `JSON.stringify` saw when it was written.

2.28. The checkpointer MUST open its own connection with `mode=rw` (it never creates the file), set the same `busy_timeout`, end when the database file is gone, and checkpoint only when `data_version` moved, counting a pass done only when the WAL's frames all went (`log === checkpointed`). A worker that ends early is made again 2 s later, at most twice in a row (three workers in all); one that lived 20 s or more resets the count. Giving up logs `the checkpoints of <db> stopped (<why>), so a commit takes one past 64 MB of WAL` (`sqlite-store.ts:59,78-131`).

2.29. A fresh database MUST end its open with the schema's statements run (the TS host's seven, six under Proposal 3, which drops `migrations`), the shape row written, and no other row; the runtime's start then writes `owner` and `reads` (rule 2.23). (The TS host also writes `migrations` rows `('store', 1)` and `('transcripts', 1)`, `sqlite-store.ts:141-151`; Proposal 3 drops the table.)

## Transcripts

A transcript belongs to a workspace, not a thread: every thread of one project folder shares it. Each event is one `events` row keyed `(workspace, pos)`, with `thread` the event's `threadId` or `''`, and `size` the length of its JSON (`sqlite-transcripts.ts:13-16,104`).

2.10. `pos` MUST be per workspace, stamped `index.pos + 1` at `record()`, and never issued twice: a position a delete or a rewind took away is not reused (`threads/transcripts.ts:677-681`; `types/internal.ts:276-278`). `foldEvent` moves `index.pos` at `append`, synchronously (`internal.ts:377`), and `reindexThreads` never lowers it (`internal.ts:462-466`). Two cases reissue positions, and the port keeps both: the index row stores `pos` at each flush, so after a crash the positions of the events that were never flushed are issued again; and an index row that is missing is rebuilt from the rows (`indexOf`, `transcripts.ts:269-273`), so `pos` becomes the newest remaining row and the positions of deleted newest events come back. A workspace with no rows and no index row starts at `pos` 0.

2.11. `size` MUST be the event JSON's length in UTF-16 code units, not UTF-8 bytes, of the event as kept (rule 2.12), and the stored `json` is that same text (`json = JSON.stringify(kept)`, `size = json.length`; `transcript-reader.ts:60-63`); the caps count it. So `size` depends on `JSON.stringify`'s escapes, number forms, dropped `undefined` and key order (04, rules 4.25 to 4.28): a port that writes the JSON differently stores different sizes and trims at different points. Page reads count true UTF-8 bytes, `Buffer.byteLength(JSON.stringify(shaped)) + 1` per event (`transcript-reader.ts:50`).

2.12. The caps MUST be per thread: 5,000 events (`TRANSCRIPT_CAP`) and 4 MiB of event size (`TRANSCRIPT_BYTES`) (`types/internal.ts:212-215`). Over a cap, `dropOldest` (`threads/transcripts.ts:94-143`) drops a thread's oldest events from before its newest `session.start` first, then the oldest of that newest turn, never the thread's newest event, until it is under both. Only `session.delta` events are clipped (`keptEvent`, `transcripts.ts:659-671`): a `tool_result`'s text, and the text or thinking of a delta with a `parentToolUseId` (a subagent's), longer than 16,384 UTF-16 units (`TOOL_RESULT_KEPT`, `internal.ts:219`) is cut to that many with `.slice`; a `tool_result` cut so also gets `bytes` = the agent's own `event.bytes` if it sent one, else the UTF-8 byte length of the whole text, and a subagent's cut text gets no `bytes`. A `tool_result`'s `patch` is cut by `keptPatch` to the hunks whose lines fit in 16,384 characters and then carries `patchCut: true` (`protocol/src/tool-result.ts:33-56`). The new keys land as `{...event, ...text, ...patch}` writes them: `text` keeps its place, `bytes` and `patchCut` go last.

2.13. Events MUST NOT be written one by one. `record()` buffers them; a flush runs at once on `session.end` or past 1 MiB pending, and otherwise 250 ms after the first non-delta event (`PENDING_FLUSH_BYTES`, `TRANSCRIPT_FLUSH_MS`; `transcripts.ts:704-713`). A `session.delta` alone starts no timer, so a crash loses the deltas since the last flush. A flush is one `begin immediate` transaction that also writes the workspace's `transcript_index` row (`transcripts.ts:401-446`).

2.14. A transcript whose index row does not read MUST NOT stop later turns from being written (runtime invariant 5). On the rows road, which is the only one the Rust host has, `loadRowsIndex`'s catch (`transcripts.ts:275-281`) warns `the transcript of <workspaceId> does not read (<why>), so search and a send leave it out until it reads` (`transcriptUnreadLine`, `internal.ts:293`), sets the index to an empty one whose `pos` is the newest row's (so the next flush does not collide with kept rows), and marks the workspace unread: flushes still append rows but skip `putIndex` until a reindex rebuilds the whole index from the rows and clears the mark (`transcripts.ts:431-439`). The blob road's set-aside (`transcripts-unparsed/<workspaceId>.<ms>`, `transcripts.ts:179-199`) and the one-time `moveBlobs` (`sqlite-transcripts.ts:191-203`) read transcripts kept as blobs, which Proposal 3 drops; neither ports.

2.30. An event with no `threadId` MUST be stored with `thread = ''` and read back with `threadId` absent (`sqlite-transcripts.ts:104,136`). It counts as one thread named `''` for the caps (`transcripts.ts:108,419`), pushes no `thread.head` (`:683`), adds no words, starts, cut or taken (`internal.ts:386-402`), is never marked trimmed (`:152-153`), and no page read returns it, since reads filter on `threadId`; it still moves `index.pos` and the index's facts.

2.31. The caps MUST be checked exactly so (`transcripts.ts:94-143,401-446`): over means strictly more than 5,000 events or strictly more than 4,194,304 of `size`; at a flush only the threads with pending events are checked, against the row counts and sizes kept (`rows.sizes`, `:419-422`); the byte bound applies only when the total passed in is already over 4 MiB, otherwise only the count is enforced (`:105`); a thread with no `session.start` drops simply oldest first, keeping its newest event. After a drop the thread is marked trimmed, the kept images of dropped starts are released, and the thread is reindexed. Reads (`openRows`, `openThread`, `:327-343`) trim in memory and delete no row. Three more caps hold beside these: `SESSION_INDEX_CAP` = 200 turn rows per workspace (`internal.ts:551`; `capSessions`, `transcripts.ts:590-609`), over which the oldest finished rows by `startedAt` go, never a thread's newest row nor a running row (so the count can pass 200), and a thread whose row went gets `ended: <status>` on its record; `WORDS_HELD` = 2 MiB of search words held in memory (`internal.ts:233`); `TRANSCRIPTS_HELD` = 4 applies to the blob road only, and the rows road holds no transcript in memory (`runtime.ts:953-955`), so the Rust host has no held-transcript map.

2.32. A flush MUST behave so (`transcripts.ts:546-585,690-714`): it runs at once when a `session.end` arrives or when `pending + size` is strictly more than 1 MiB; otherwise a non-delta event arms one 250 ms timer per workspace if none stands, and later events do not move it. Flushes are queued on a per-workspace promise chain and asks coalesce; a flush with nothing pending does nothing (`:563`). A flush that fails (a `begin immediate` that times out after `busy_timeout`, a constraint error) rolls back, is swallowed with no log line (`flush.catch(() => {})`, `:585`), and leaves its events pending, since pending is cleared only after commit (`:442-443`); no retry is armed, and the next trigger tries again. A rewind's `done()` flushes too (`sessions.ts:1292`).

2.33. Inside a flush's transaction the steps MUST run in this order (`settleRows`, `transcripts.ts:401-446`): append the pending events less those a cut drops; remove the cut rows; check the caps; reindex; `putIndex` (skipped while the workspace is unread, rule 2.14); commit. After the commit: release images, clear pending, swap in the new index, balance the words. Closing the host flushes every workspace with a timer or pending events, then awaits the transcript queue and the index flushes as they stand (`runtime.ts:1281-1282`). Deleting a workspace cancels its flush, awaits its queue, and clears its events and index row in one transaction (`machines.ts:548-550,565-575`; `rows.clear`).

2.34. The `transcript_index` row MUST be `indexBytes(index, undefined, false)` (`internal.ts:510-511`): keys `of` (absent when undefined), `words`, `starts`, `cut`, `facts`, `taken`, `children`, `trimmed`, `pos`, in that order; every Map and Set as an array of pairs or values, `children` as `[thread, [[task, child], ...]]`; `words` in the cold form `{lines: [], last, size, first?, opened?, cold: true}`. A row that does not parse, or lacks `taken`, `children` or `pos`, reads as absent and the index is rebuilt (`:512-525`). After a restart every thread's words are cold and are read back from the rows one thread per `setImmediate` tick, within the 2 MiB budget (`threads/transcripts.ts:472-505`).

2.35. `sessions.history` with a `threadId` MUST page as the TS host does (`pickNewest`, `transcript-reader.ts:44-56`; `transcripts.ts:353-358`; `protocol/src/wire/runtime.ts:565-574`): `before` is an integer above 0 and `limit` an integer from 1 to 1,000 (`HISTORY_PAGE_MAX`), each refused by the schema otherwise; the defaults are 200 events (`HISTORY_PAGE_EVENTS`) and 400 KiB (`HISTORY_PAGE_BYTES`, `protocol/src/views/preferences.ts:311,315`). It walks newest first over pending events then rows with `pos < before`, stops at `limit` events or when the next event would pass the byte budget (checked from the second event on), and answers oldest first as `{events, total, pos, trimmed?}` in that key order, `total` being rows plus pending, `pos` the index's (0 when none) and `trimmed: true` only for a trimmed thread. A page past the oldest event answers `events: []` with `total` and `pos` filled. A thread outside the caller's tree answers `{events: [], pos, total: 0}` (`sessions.ts:948`). The head read is the same walk with no event limit, strict from the first event, a budget of 64 KiB less the facts' JSON less 100, and each tool result's text cut to 2,048 characters with `cut: <full length>` (`sessions.ts:960-963`; `headShape`, `transcript-reader.ts:77-81`).

## The records

There is no thread collection and no turn collection: both live in the per-workspace `sessions` document. The collections, keyed by the id shown (`types/internal.ts:44-158,841-845` and the sites named):

| Collection | Key | Shape | Owner of writes |
|---|---|---|---|
| `workspaces` | `ws_<8 hex>` | `WorkspaceRecord`, about 55 fields (`types/wiring.ts:341-425`); a project folder, a local workspace and a worktree are the same record | `images/records.ts:99-106` (`persist`) |
| `workspace-names` | workspace id | `{workspaceId, name, project?}` | `records.ts:105` |
| `projects` | `pr_<8 hex>` | `ProjectView`, 16 fields (`protocol/src/views/workspace.ts:184-237`) | `account/rules.ts:17` |
| `seed-choices` | absolute folder path | `SeedChoice` | `projects/projects.ts:244,356` |
| `sessions` | workspace id | `{workspaceId, sessions[], threads, viewed?}`, in that key order (`transcripts.ts:651`): one row per turn (`SessionView`, 52 fields, plus 16 turn fields) and one `ThreadRecord` per thread, 21 fields (`internal.ts:772-837`) | `persistSessions` (`threads/transcripts.ts:617-657`), through a promise chain per workspace |
| `attachment-keys`, `notify-owed`, `held-starts`, `sub-starts`, `thread-ends`, `pty-threads`, `pty-tokens` | as named in `internal.ts` (`notify-owed` at `:94`), `machines/thread-ends.ts:14`, `account/channels.ts:378,717` | small records of turns in flight | their modules |
| `places` | `p_<16 hex>` | `PlaceRecord`, 15 fields (`places/types.ts:69-100`); "this computer" and each provider are rows, not records | `keep()` and `change()` (`places/records.ts:271-276,349`), runtime invariant 6 |
| `caps`, `place-default`, `pending-computers`, `agentSetups`, `agent-lists` | place id, `default`, `a_<12 hex>`, `<placeId>:<agent>`, `<machineId>:<harness>` | place settings and setups | `places/manage.ts`, `agent-setup.ts:119-122`, `threads/agents.ts:118` |
| `devices` | `d_<16 hex>` | `{id, name, tokenHash, createdAt, lastSeenAt, scope?, road?, aside?, here?, via?}`; a thread's token is a device with `scope` | `runtime/src/devices.ts:45-198` |
| `pairings`, `revoked-keys` | the code, the key fingerprint | `{code, expiresAt, here?}`, `{fingerprint}` | `devices.ts:67-78` |
| `goldens`, `golden-recipes`, `project-goldens`, `images`, `builders` | `<place>/<name>`, `<place>/<name>@v<n>`, snapshot id, name, builder id | image records (`protocol/src/views/golden-image.ts`) | `images/*.ts`; the cut (#2441) decides which survive |
| `preferences` | `default` | `Preferences`, about 42 keys | `account/preferences.ts:72-79` |
| `reads`, `owner` | `since`, `id` | `{at}`, `{id: "h_<8 hex>"}` | `machines/boot.ts:331-342` |
| `usage-index`, `limits`, `usage-accounts`, `usage-computers`, `usage-reset-pending`, `cost-histories` | as in `usage.ts:42-47,143` | usage and cost: `usage-days` kept 90 days (`usage.ts:50`); an ended workspace's `cost-histories` document deleted at load once its last point is before the current month (`status.ts:441-449`) | `usage.ts`, `usage-reset.ts:40`, `status.ts:439-499` |
| `slates` | thread id | `SlateRecord` (`slates.ts:100-122`), only `schema: 2` loaded | `slates.ts:303-306` |
| `creates`, `dropped-machines`, `unrecorded-machines` | purpose, machine id | the durable half of a create in flight | `machines/machines.ts:64-148,360` |

Blob collections: `attachments` (`<threadId>.<requestId>.<index>`), `vaults` (a napped machine's files, up to 200 MiB), `image-vaults`, `sub-sessions`, `usage-days`, `prices` (`litellm`), `transcripts-unparsed`.

2.15. Ids MUST keep their shapes: `ws_`, `pr_` and `h_` with 8 random hex characters, `p_` and `d_` with 16, `a_` with 12; thread, turn and steer ids are `randomUUID()` (`projects/workspaces.ts:159`, `projects/projects.ts:311`, `places/link.ts:88`, `devices.ts:186`, `places/manage.ts:201`; thread `threads/sessions.ts:290`, unless the caller named one; turn `:476`, `o.as?.turnId ?? randomUUID()`; steer `:714,1005`; a warm launch's thread `threads/launch.ts:85`). Tokens: a turn's token 16 random bytes as 32 hex (`launch.ts:101`, `sessions.ts:423`), a machine's daemon token 24 bytes as 48 hex (`transcripts.ts:524`), a device token 24 bytes base64url, the host token 24 bytes base64url (32 characters); a sign-in id is `si_` and 12 hex (`agents-read.ts:503`). Clients and the harness's normalizer key on these prefixes.

2.16. Each record's JSON MUST be what the TS host writes for it: the Rust record types are serde structs held to the TS types' JSON, field for field, under 13's type rules. A record is read back with no schema check in the store (`sqlite-store.ts:352,361`); validation belongs to the module that owns it.

2.36. The `sessions` document MUST be written as `persistSessions` writes it (`threads/transcripts.ts:617-657`): nothing for a workspace not in `live` (`:618`); rows with `launch` set (held and unlaunched starts) left out (`:627`); before the rows are built, a settled row that did work sets `worked: true` on its thread's record, then `capSessions` runs (rule 2.31). Rows go in the order of the in-memory `sessions` map filtered to the workspace (at a start, `store.list` order, then array order), each with the view's own keys, then `turnId, notify, notifyBy, notifyRoad`, then, only while the row's status is `running`, `reply, told, steered, waited, waitingSince, run, from, asked, turnToken, scopeDeviceId`, then `snapshot` (`:628-646`). `threads` is the thread records in map order, keyed by thread id with `workspaceId` stripped; `viewed` is written only when non-empty; a workspace with no turns still writes `{workspaceId, sessions: [], threads: {}}`. Writes go through one chain per workspace, so an earlier snapshot never lands after a later one; a failed `put` is swallowed (`:652-655`). At a start, a document for a workspace not in `live` is skipped and not deleted (`boot.ts:396`); one with no `sessions` array warns `sessions document for <id> has no rows array, read as empty` and is skipped (`:433-436`); a string `notify` reads as `[notify]`; an opening prompt past `OPENING_KEPT` is cut and the document written once more (`:442-445,500`).

> **After the port.** A start re-reads each thread record without its `filesIn` and `clouds` fields (`boot.ts:403-428`), so the next persist after a restart drops them. The port keeps the loss.

2.38. Taking a thread or a turn away MUST keep the TS order: a delete removes the in-memory rows and the thread record first, then drops the thread from the transcript, then persists the sessions document, then forgets the slate (`sessions.ts:1446-1449`); a forget removes the rows and the record, then forgets the slate, then drops the thread from the transcript, then persists (`:1476-1479`); a rewind drops the cut turns from the transcript, tells the slates (`slates.rewound`), then persists, flushes and emits `thread.rewound` (`:1290-1294,1388-1396`). Transcript drops run inside the workspace's transcript queue (`transcripts.ts:385-396`).

## What lives only in memory

Lost at every restart, on purpose (`runtime/src/context.ts`, the sites named):

- The event ring, its head and its stream id (04, rule 4.19).
- Unflushed transcript events (rule 2.13) and the in-memory transcript index, rebuilt from its row.
- Creates in flight: `ctx.inflight`, `holding`, `forking`, `failedCreates`, `createStages` (`machines/machines.ts:45-52,184-229`; `context.ts:550-556`); only the `creates` row is durable.
- Starts held on a computer's thread cap (`threads/threads.ts:374-393`); only `held-starts` is durable, and each is failed at the next start.
- Kept and warm agent processes (`threads/threads.ts:102`).
- Slate press and reaction buckets, write budgets, secrets not marked keep, "run once" approvals, MCP connections (`slates.ts:464,470-474`; `slate-runs.ts:527-589`; `ThreadMcp.conns`, `slate-mcp.ts:589-600`).
- The GitHub read cache (`github-cache.ts:18-25`), the preferences cache, per-machine daemon tokens (random per process), and the timers re-armed from stored fields at start.

2.17. The Rust host MUST keep the same split: what the list above holds in memory MUST NOT be persisted, because the TS host's restart behaviour (a held start fails, a slate run reads "the host restarted while it ran") depends on its loss.

## What a start rebuilds

`ready()` in `runtime/src/machines/boot.ts:328-621`, in order:

1. the owner id, then `reads/since` (`:331-342`); `migrateCopies` (`:343`; upgrade code, dropped under Proposal 3);
2. places into memory (`places/link.ts:291-315`), which also rewrites them: a setup left waiting becomes `expired`, a `sync.state` of `running` becomes `behind` (`link.ts:296-303`);
3. projects (and the `git` backfill, dropped); setups;
4. each workspace hydrated against its provider under one shared 1,500 ms deadline (`LOAD_READS_MS`, `boot.ts:18`);
5. the transcript index of every workspace that has none yet (`:387`);
6. the `sessions` documents into memory, with snooze and limit-resume timers armed as each loads (rule 2.36);
7. the `recut` persists, the unread-card reads, `rereadHeld` for every workspace; then, per workspace in the background, each running turn re-opened by its stored run handle (06), runs the host does not keep swept, and the daemon synced. A re-open that answers `cannot` settles the turn with `host restarted while the agent was working` (`RESTARTED_REASON`, `internal.ts:163`) and `cut by a host restart after <duration>` (`restartCutLine`, `internal.ts:208`); one that answers `gone` settles with `the machine no longer holds this turn's run, so nothing of it can be read back` (`RUN_GONE_LINE`, `protocol/src/words/thread.ts:108`); an unreachable machine is asked again after 60 s (`LINK_RETRY_WINDOW_MS`);
8. the `sub-starts` queue resumed (`resumeQueue`);
9. each `held-starts` row failed (rule 2.37);
10. every thread token whose turn is not running revoked (`boot.ts:597-599`), then `deviceDoor.revokeAsides()`;
11. `armSubs()`, then owed notices delivered;
12. builders admitted, under the same 1,500 ms deadline; `armSweep()`;
13. the slates loaded, not awaited (`void slates.ready()`), so hydration resolves before they load: every run record that read `running` is rewritten in one batch as `{state: "failed", why: "the host restarted while it ran", exit: null, runs, startedAt?, endedAt: <now>}` (`restartedRecord`, `slate-runs.ts:380-387`; `slates.ts:427-462`), a literal in the code and not a words file.

A snooze timer whose time is past is not armed; a limit-resume timer whose time is past fires at once; both are clamped to `MAX_TIMER_MS` = 2^31 - 1 ms and re-armed on the way (`runtime.ts:872-908`).

2.18. Hydration MUST start as the TS host starts it and MUST be done before the lock is taken on the `wsp up` road (`cli/service.ts:60` before `:62`), as `servesNothing()` forces it. When a broker is wired, `pipeArea` already calls `void ctx.ready()` while the runtime is built (`pipe.ts:404,454`, reached from `runtime.ts:496`); with none, the first op that needs it calls it (`ctx.ready()` at `runtime.ts:616` and others). A start that hydrates in another order changes what the first client sees.

2.37. A start MUST fail every `held-starts` row, each in this order (`boot.ts:584-594`): delete the row; skip it when its workspace is not live; with `notify` set, send the end as failed with the reason; then `record` `{type: "session.end", workspaceId, sessionId, turnId, threadId, exitCode: null, sawResult: false, reason, unstarted: true, prompt?}` in that key order, with `reason` = `the host restarted while this turn waited for a slot on <place>; nothing started, so send it again` (`capRestartedLine`, `protocol/src/place-state.ts:358`). No turn row is made. A `held-starts` record is `{workspaceId, threadId, turnId, sessionId, harness, place, prompt, notify?, notifyBy?, notifyRoad?}` in that key order, keyed by turn id, written through one chain whose failures only warn, and deleted when the start goes through or is given up (`threads/threads.ts:448-454,518`).

## Proposal 3: the store

> **Proposal, awaiting the owner's ruling** (ticket decision 3). Pick: SQLite through `rusqlite` with the bundled SQLite, the same seven statements above minus `migrations`, run once on an empty file; the same pragmas; the same JSON-bag layout. Fresh means no import, no shape upgrade and no migration code, not a new layout.

Why the same layout and not typed tables:

- The TS host is the oracle and the records are JSON documents its modules read and write whole. A typed table per record kind is a redesign of every module's persistence, and its bugs would not show on a tape until a restart.
- Other processes read the tables (rule 2.4). Keeping them keeps those readers working without a coordinated change.
- Parity of a record is then one check: the Rust host's row JSON against the TS host's row JSON for the same tape, which the harness can diff after a run.
- A typed schema can come after the port, as one ticket, when the record shapes stop moving.

What the Rust host drops (every one is in today's code, `sqlite-store.ts`, `store.ts`, `machines/boot.ts`, `images/records.ts`, `threads/transcripts.ts`, `transcript-reader.ts`):

- the `migrations` table and the per-module migration runner;
- the `state.json` import, `state.json.imported`, the JSON store and `WSP_STATE_STORE=json`;
- transcripts kept inside the state file or as blobs, and their move to rows (`moveBlobs`);
- events without `pos`, indexes missing `taken`, `children` or `pos`;
- golden records under a bare name, the `owned` list, image records backfilled from a copy, `ProjectGolden.project`;
- session rows with `notify` as a string, no `notifyBy`, `resumeAt`; workspaces with no `kind` or `base`; scoped devices with no `road`; slate records of schema 1; projects with no `git`;
- `moveOldCopies` and `copies-kept.txt`.

2.19. The Rust host MUST stamp the `shape` row with its own shape number, one past the TS host's `STATE_SHAPE = 2` (`protocol/src/wire/daemon-version.ts:742`), and MUST refuse a database of any other shape with the TS host's refusals of rule 2.26, whose fix the person carries out: the TS host throws and moves nothing (`refuseOtherShape`, `store.ts:142-145`, called at `sqlite-store.ts:286,293,310,318`). A TS host's `state.db` (shape 2) so reads as the older-shape refusal, `... this wsp reads shape 3): move the file aside and this wsp starts a fresh one`. It never reads a TS host's `state.db`. Whether the owner's threads are exported is the map's open question; with the layout unchanged an export is a copy of rows.

2.20. The Rust host MUST run SQLite on the state owner's own thread, synchronously, as node does, so a write is durable in the WAL before the next task runs (03, rule 3.1). The passive checkpointer keeps its own connection on its own thread.

## Secrets at rest, unchanged

The port keeps every secret where it is today; changing where a secret rests is a security ticket after the port, not a porting choice. In plaintext inside `state.db`, protected by its 0600 mode and the 0700 folder: running turns' tokens (`sessions[].turnToken`), pairing codes, agent setup env values (`agent-setup.ts:3-6`), image vault blobs. Hashed: device tokens, as sha256 hex (`devices.ts:90,181`). In 0600 files: `host-token`, `.env`, `servers.env`, `relay.json`, `hosts/*.json`, `slates.secrets.json`, the key pairs. The one encrypted artifact is an image export: scrypt (N 2^15, r 8, p 1) then AES-256-GCM (`engine/src/vault-cipher.ts:25-66`).

2.21. The Rust host MUST NOT log, print or send any value from the list above, and MUST blank the request fields `token key rows code passphrase env envs headers sudoPassword` one level deep in its refusal log lines, withholding the sentence past 256 secret values (`protocol/src/wire/runtime.ts:1033`; `runtime/src/serve.ts:359-374`). The fields' string values are taken, or the string values one level into an object or an array (`runtime.ts:1035-1042`); each is replaced by `<redacted>`, longest first, both as written and trimmed, skipping any under 4 characters once trimmed (`protocol/src/words/thread.ts:402-407`). The line is `refused <op> kind=<kind>: <text>`, `op` being `frame` unless the op is declared and `kind` `none` when absent, with only the sentence's first line, cut at 400 characters as `... (cut N characters)` (`SAID_LINE_CHARS`, `exit.ts:92,96`); past 256 distinct values the text is `(sentence withheld, N secret values)`.

## Empty, missing and fresh

What a fresh host answers, and what each read answers for an id it does not hold. The tapes are recorded from a TS host at `a3b870a6e` (`node docs/rust-host-spec/checker/check.mjs --record`).

2.39. A fresh host MUST answer every listing empty, never refused: `workspaces.list`, `sessions.list` and `devices.list` answer `[]`, and `account.get` answers `{signedIn: false}` when no relay account is linked.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"workspaces.list"}}
{"out":{"id":2,"ok":true,"workspaces":[]}}
{"in":{"id":3,"op":"sessions.list"}}
{"out":{"id":3,"ok":true,"sessions":[]}}
{"in":{"id":4,"op":"devices.list"}}
{"out":{"id":4,"ok":true,"devices":[]}}
{"in":{"id":5,"op":"account.get"}}
{"out":{"id":5,"ok":true,"account":{"signedIn":false}}}
```

2.40. A read of a missing id MUST answer as the op does today, which is not uniform: `sessions.list` filtered to a workspace it does not hold answers `[]`; `workspaces.get` and `sessions.history` refuse with `kind: "not-found"` and `no such workspace: <id>`; `sessions.head` and `sessions.forget` of an unknown thread refuse with `kind: "not-found"` and `no thread <first 8 characters of the id>`. A list op filters and a one-record op refuses: a builder MUST NOT make one into the other.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"sessions.list","workspaceId":"ws_00000000"}}
{"out":{"id":2,"ok":true,"sessions":[]}}
{"in":{"id":3,"op":"workspaces.get","workspaceId":"ws_00000000"}}
{"out":{"id":3,"ok":false,"error":"no such workspace: ws_00000000","kind":"not-found"}}
{"in":{"id":4,"op":"sessions.history","workspaceId":"ws_00000000"}}
{"out":{"id":4,"ok":false,"error":"no such workspace: ws_00000000","kind":"not-found"}}
{"in":{"id":5,"op":"sessions.head","threadId":"00000000-0000-4000-8000-000000000000"}}
{"out":{"id":5,"ok":false,"error":"no thread 00000000","kind":"not-found"}}
{"in":{"id":6,"op":"sessions.forget","threadId":"00000000-0000-4000-8000-000000000000"}}
{"out":{"id":6,"ok":false,"error":"no thread 00000000","kind":"not-found"}}
```

2.41. A record's JSON MUST keep the key order of the record, not of the request that changed it (rules 2.16, 2.27). `preferences.set` merges the patch into the stored preferences and answers them whole in their own order: a patch naming `sendWith` before `theme` still answers `theme` first, and an empty patch answers what is stored, unchanged (`account/preferences.ts:72-83`).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"preferences.set","patch":{"sendWith":"mod-enter","theme":"dark"}}}
{"out":{"id":2,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"mod-enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":3,"op":"preferences.set","patch":{}}}
{"out":{"id":3,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"mod-enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
```

What survives a restart has no tape: a tape runs against one host process from its first frame to its last, and the checker starts a fresh host for each. Rule 2.23 holds the measured restart of a throwaway host; the restart rules (2.17, 2.18, 2.36, 2.37) are pinned by `packages/runtime/test/local-exec.test.ts`, `subagents.test.ts`, `transcript-index.test.ts` and `serve-replay.test.ts`, which a port carries over (13).
