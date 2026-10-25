# 02 State and storage

What the TS host keeps, where, in what shape, and what it rebuilds at start. The Rust host starts with fresh state and no migrations (the map's locked point 3), so this chapter describes today's layout, says which parts are a contract other processes read, and lists the upgrade code that does not port. Every `file:line` is at `a3b870a6e`.

## The state folder

The path a host is pointed at is still called `state.json`, but it is only an anchor: the records live in `state.db` beside it (`packages/runtime/src/sqlite-store.ts:39`), and every other file is `join(dirname(statePath), name)` (`packages/host/src/host-lock.ts:158-164`), so two state files on one computer are two hosts with nothing shared. Which state file a process uses: `--state`, else `$WSP_HOME/state.json`, else `<checkout>/.wsp/state.json` inside a checkout of wsp, else `~/.wsp/state.json` (`packages/host/src/cli/state.ts:29-55`; `packages/host/src/serving-home.ts:192-201`).

One writer makes nearly every file: `writeOwn` (`packages/own-file/src/index.ts:27-41`) makes or repairs the folder to 0700, writes `<target>.<pid>-<ms>-<6 base36>.tmp` with mode 0600, chmods it, and renames it over the target. It never fsyncs.

| Name, beside the state file | Holds | Written by | Mode | Read by |
|---|---|---|---|---|
| `state.db`, `-wal`, `-shm` | every record (below) | `sqliteStore` (`sqlite-store.ts:278-384`) | 0600 | the host; `wsp doctor` and `wsp init` open their own connections (`cli/commands.ts:347,368`, `cli/init.ts:251,296`); the GPUI app, read only (rule 2.4) |
| `host.lock` | `{pid, port, address, startedBy, startedAt}`, compact JSON | `takeLock` by hard link, `rewriteLock` by rename (`host-lock.ts:259-280,356-360`) | umask | every client's dial; a `sed` in a shell reader (`serving-home.ts:225-228`); `wsp-mcp/src/aim.rs:57-68` |
| `host-token` | 24 random bytes, base64url, no newline, new at every start | `cli/serve.ts:311-313`, `writeOwn` | 0600 | every client's dial (`host-lock.ts:143-149`) |
| `host.log` | the host's stdout and stderr when a verb or the service started it | the parent opens it for append (`host-start.ts:73-90`; `service.ts:215-216,299-300`) | umask | `service.ts:594-630` |
| `door-port` | `<port>\n` of the door bound for `--port 0` | `server.ts:594` | 0600 | `server.ts:224-231` |
| `runs/` | one turn's files: `<12 hex>.d/` (the claim), `.sh`, `.in`, `.held`, `.log`, `.pid`, `.exit`, `.agent`, `.pump`, `.late*` | `runtime/src/local-exec.ts:244-246,705-748` | 0700 folder, 0600 files except `.pid` and `.exit` | the host after a restart (06) |
| `roots` | the folders this computer's daemon may serve | a shell `printf ... > roots.next.$$; mv -f` through the machine's exec (`runtime/src/daemon-roots.ts:9-19`) | umask | the daemon (`--roots-path`) |
| `readings/`, `inbox/` | the daemon's readings and landed files | the daemon | | |
| `place-host-key.json` | the host's ed25519 pair, never rotated | `places/this-computer.ts:23-46` | 0600 | `places.ts:33` |
| `relay.json` | the relay record | `account.ts:76-82` | 0600 | `cli/serve.ts:278`, `aim.rs` |
| `.env`, `servers.env`, `servers.owners.json` | provider keys and switches; MCP server values by name | `env-keys.ts:77-160` | 0600 | at start and at every launch, never cached (`cli/keys.ts:158-167`) |
| `slates.secrets.json` | slate secrets marked keep, plaintext | `runtime/src/slate-runs.ts:557-564` | 0600 | once at start |
| `slates/<threadId>/` | a slate's files, `$SLATE_DIR` | `runtime/src/slate-files.ts:20-29` | 0700, 0600 | slate runs |
| `connector.pid`, `cloudflared-<version>` | the relay connector | `connector.ts:62-84,142-223` | umask, 0755 | |
| `thread-bin/`, `docker-notes/` | the `wsp` and `docker` shims a thread's PATH holds | `shim.ts:14-42`, rewritten at every host build | 0755 | turns |
| `icons/`, `project-icons/` | fetched icons | `server-icons.ts:138-145`, `runtime/src/projects/images.ts:95-133` | 0600 | |
| `broker/<day>.jsonl`, `revoked.json` | the broker's log (14 days) and off switch | `broker/log.ts:58-70`, `broker/off.ts:27-50` | 0600 | |
| `recipes/<slug>.toml`, `release.json`, `agent-latest.json` | recipes, the last release read, agent versions | `recipes.ts:117`, `release.ts:195`, `agent-latest.ts:84` | 0600 | |
| `worktrees/<projectId>/<branch>` | worktrees | the daemon's copy road (07) | | |

Outside the state folder: `hosts/<alias>.json`, `relay-client.json`, `device-key.json` and `ssh/` under the wsp home (`hosts.ts:52-120`, `account.ts:98-131`, `ssh-files.ts:40-97`); `host-id` and `analytics-id` in the OS config folder, kept off the wsp home on purpose (`runtime/src/host-id.ts:10-37`); agent MCP configs and skill copies, rewritten at start for agents whose wsp entry names this state (`mcp-install.ts:168-276`; the config write is a `/bin/sh` line with `umask 077`, `mktemp`, a `cksum` check and `mv -f`, `catalog/src/config-write.ts:62-96`); the service unit (06); and the local daemon's token folder `<tmpdir>/wsp-local-daemon-<pid>-*` (`local-daemon.ts:185-189`).

2.1. Every file the Rust host writes beside the state file MUST have the name, content and mode in the table above. Files a client or another process reads (`host.lock`, `host-token`, `door-port`, `relay.json`, `hosts/*.json`, the MCP configs, the skill copies) are a contract with programs the port does not change.

2.2. `host.lock` MUST be compact JSON with the keys in the order `pid, port, address, startedBy, startedAt` and no spaces: a shell reader takes the pid with `sed 's/.*"pid":\([0-9]*\).*/\1/p'` (`serving-home.ts:225-228`). 06 has its protocol (hard link, takeover markers, the liveness check).

2.3. The Rust host MUST write through one function with `writeOwn`'s behaviour: folder 0700, a temp file 0600 beside the target, rename, no fsync. The files the table marks umask stay at the umask.

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

2.4. The table names `collections`, `blobs`, `events` and `transcript_index` and their columns are read by other processes: `wsp doctor`, `wsp init`, and on `gpui/main` the GPUI app, which opens `state.db` read only to ask whether a `places` row exists (`gpui/main:apps/desktop-gpui/host-client/src/start.rs:659-661`). Under Proposal 3 they stay; a change to them is a change to those readers in the same ticket.

2.5. `list` and `keys` MUST return rows in `rowid` order, which is first-write order and survives updates; an id deleted and written again takes a new place at the end (`sqlite-store.ts:410-422`). Every listing a client sees that is not sorted on purpose inherits this order.

2.6. A database MUST be switched to WAL on a private file before it takes the name `state.db`: build `state.db.importing-<pid>`, set WAL, close, then `link(2)` it to `state.db`; the loser of a link race returns and opens the winner's file (`sqlite-store.ts:232-273`). Two processes switching one file to WAL at once fail one of them without calling the busy handler (trap #1785; `packages/host/test/state-store.test.ts:146-165`, "ten times in ten").

2.7. Busy handling is `busy_timeout` and nothing else: no retry, no catch on `SQLITE_BUSY` (`sqlite-store.ts:51-53`). A statement that times out rejects to its caller, and most callers log and carry on (`threads/agents.ts:118`, `status.ts:448`).

2.8. Each `put`, `delete`, `putBlob` and `deleteBlob` is one autocommit statement, run at once. Transactions are `begin immediate` in two places only: the migrate-and-stamp step at open (`sqlite-store.ts:316-325`) and `atomically` for transcript writes, where nested calls join the outer one (`sqlite-transcripts.ts:83-99`).

2.9. A blob's `at` is a real number of milliseconds, `performance.timeOrigin + performance.now()`, made strictly increasing by at least 0.001 per write (`sqlite-store.ts:281-283`), so `statBlob` tells two writes apart even within one millisecond.

## Transcripts

A transcript belongs to a workspace, not a thread: every thread of one project folder shares it. Each event is one `events` row keyed `(workspace, pos)`, with `thread` the event's `threadId` or `''`, and `size` the length of its JSON (`sqlite-transcripts.ts:13-16,104`).

2.10. `pos` MUST be per workspace, stamped `index.pos + 1` at `record()`, and never issued twice: a position a delete or a rewind took away is not reused (`threads/transcripts.ts:677-681`; `types/internal.ts:273-277`).

2.11. `size` MUST be the event JSON's length in UTF-16 code units, not UTF-8 bytes (`transcript-reader.ts:330-335`); the caps count it. Page reads count true UTF-8 bytes, `Buffer.byteLength(JSON.stringify(shaped)) + 1` per event (`transcript-reader.ts:320`).

2.12. The caps MUST be per thread: 5,000 events (`TRANSCRIPT_CAP`) and 4 MiB of event size (`TRANSCRIPT_BYTES`) (`types/internal.ts:212-215`). Over a cap, `dropOldest` (`threads/transcripts.ts:94-143`) drops a thread's oldest events from before its newest `session.start` first, then the oldest of that newest turn, never the thread's newest event, until it is under both. A tool result's text, and a subagent's text or thinking, is kept to 16 KiB characters with `bytes` set to the whole length (`TOOL_RESULT_KEPT`, `internal.ts:219`; `transcripts.ts:659-671`).

2.13. Events MUST NOT be written one by one. `record()` buffers them; a flush runs at once on `session.end` or past 1 MiB pending, and otherwise 250 ms after the first non-delta event (`PENDING_FLUSH_BYTES`, `TRANSCRIPT_FLUSH_MS`; `transcripts.ts:704-713`). A `session.delta` alone starts no timer, so a crash loses the deltas since the last flush. A flush is one `begin immediate` transaction that also writes the workspace's `transcript_index` row (`transcripts.ts:401-446`).

2.14. A transcript blob that does not parse MUST be set aside, never thrown away or written over: copied to `transcripts-unparsed` as `<workspaceId>.<ms>`, the original deleted, and the transcript starts again empty with a warning; a blob that cannot be read at all refuses with `transcriptUnreadLine` and nothing is written (runtime invariant 5; `transcripts.ts:179-199`). With a fresh store the only source of such a blob is a crash mid-write, so the rule holds for the Rust host's own data.

## The records

There is no thread collection and no turn collection: both live in the per-workspace `sessions` document. The collections, keyed by the id shown (`types/internal.ts:44-158,841-845` and the sites named):

| Collection | Key | Shape | Owner of writes |
|---|---|---|---|
| `workspaces` | `ws_<8 hex>` | `WorkspaceRecord`, about 55 fields (`types/wiring.ts:341-425`); a project folder, a local workspace and a worktree are the same record | `images/records.ts:99-106` (`persist`) |
| `workspace-names` | workspace id | `{workspaceId, name, project?}` | `records.ts:105` |
| `projects` | `pr_<8 hex>` | `ProjectView`, 16 fields (`protocol/src/views/workspace.ts:184-237`) | `account/rules.ts:17` |
| `seed-choices` | absolute folder path | `SeedChoice` | `projects/projects.ts:244,356` |
| `sessions` | workspace id | `{workspaceId, viewed?, sessions[], threads?}`: one row per turn (`SessionView`, 52 fields, plus 16 turn fields) and one `ThreadRecord` per thread, 21 fields (`internal.ts:772-837`) | `persistSessions` (`threads/transcripts.ts:617-657`), through a promise chain per workspace |
| `attachment-keys`, `notify-owed`, `held-starts`, `sub-starts`, `thread-ends`, `pty-threads`, `pty-tokens` | as named in `internal.ts`, `threads/threads.ts:31`, `machines/thread-ends.ts:14`, `account/channels.ts:378,717` | small records of turns in flight | their modules |
| `places` | `p_<16 hex>` | `PlaceRecord`, 15 fields (`places/types.ts:69-100`); "this computer" and each provider are rows, not records | `keep()` and `change()` (`places/records.ts:271-276,349`), runtime invariant 6 |
| `caps`, `place-default`, `pending-computers`, `agentSetups`, `agent-lists` | place id, `default`, `a_<12 hex>`, `<placeId>:<agent>`, `<machineId>:<harness>` | place settings and setups | `places/manage.ts`, `agent-setup.ts:119-122`, `threads/agents.ts:118` |
| `devices` | `d_<16 hex>` | `{id, name, tokenHash, createdAt, lastSeenAt, scope?, road?, aside?, here?, via?}`; a thread's token is a device with `scope` | `runtime/src/devices.ts:45-198` |
| `pairings`, `revoked-keys` | the code, the key fingerprint | `{code, expiresAt, here?}`, `{fingerprint}` | `devices.ts:67-78` |
| `goldens`, `golden-recipes`, `project-goldens`, `images`, `builders` | `<place>/<name>`, `<place>/<name>@v<n>`, snapshot id, name, builder id | image records (`protocol/src/views/golden-image.ts`) | `images/*.ts`; the cut (#2441) decides which survive |
| `preferences` | `default` | `Preferences`, about 42 keys | `account/preferences.ts:72-79` |
| `reads`, `owner` | `since`, `id` | `{at}`, `{id: "h_<8 hex>"}` | `machines/boot.ts:331-342` |
| `usage-index`, `limits`, `usage-accounts`, `usage-computers`, `usage-reset-pending`, `cost-histories` | as in `usage.ts:42-47,143` | usage and cost, kept 90 days | `usage.ts`, `usage-reset.ts`, `status.ts:439-499` |
| `slates` | thread id | `SlateRecord` (`slates.ts:100-122`), only `schema: 2` loaded | `slates.ts:303-306` |
| `creates`, `dropped-machines`, `unrecorded-machines` | purpose, machine id | the durable half of a create in flight | `machines/machines.ts:64-148,360` |

Blob collections: `attachments` (`<threadId>.<requestId>.<index>`), `vaults` (a napped machine's files, up to 200 MiB), `image-vaults`, `sub-sessions`, `usage-days`, `prices` (`litellm`), `transcripts-unparsed`.

2.15. Ids MUST keep their shapes: `ws_`, `pr_` and `h_` with 8 random hex characters, `p_` and `d_` with 16, `a_` with 12; thread, turn and steer ids are `randomUUID()` (`projects/workspaces.ts:159`, `projects/projects.ts:311`, `places/link.ts:88`, `devices.ts:186`, `places/manage.ts:201`, `threads/launch.ts:85`). Clients and the harness's normalizer key on these prefixes.

2.16. Each record's JSON MUST be what the TS host writes for it: the Rust record types are serde structs held to the TS types' JSON, field for field, under 13's type rules. A record is read back with no schema check in the store (`sqlite-store.ts:352,361`); validation belongs to the module that owns it.

## What lives only in memory

Lost at every restart, on purpose (`runtime/src/context.ts`, the sites named):

- The event ring, its head and its stream id (04, rule 4.19).
- Unflushed transcript events (rule 2.13) and the in-memory transcript index, rebuilt from its row.
- Creates in flight: `ctx.inflight`, `holding`, `forking`, `failedCreates`, `createStages` (`machines/machines.ts:45-52,184-229`; `context.ts:550-556`); only the `creates` row is durable.
- Starts held on a computer's thread cap (`threads/threads.ts:374-393`); only `held-starts` is durable, and each is failed at the next start.
- Kept and warm agent processes (`threads/threads.ts:102`).
- Slate press and reaction buckets, write budgets, secrets not marked keep, "run once" approvals, MCP connections (`slates.ts:470-474`; `slate-runs.ts:527-589`; `slate-mcp.ts:212-225`).
- The GitHub read cache (`github-cache.ts:18-25`), the preferences cache, per-machine daemon tokens (random per process), and the timers re-armed from stored fields at start.

2.17. The Rust host MUST keep the same split: what the list above holds in memory MUST NOT be persisted, because the TS host's restart behaviour (a held start fails, a slate run reads "the host restarted while it ran") depends on its loss.

## What a start rebuilds

`ready()` in `runtime/src/machines/boot.ts:328-619`, in order: the owner id; `reads/since`; places into memory (`places/link.ts:291-315`); projects; setups; each workspace hydrated against its provider under one shared 1,500 ms deadline (`LOAD_READS_MS`, `boot.ts:18`); the transcript index of each workspace; the `sessions` documents into memory, with snooze and limit-resume timers armed; then, per workspace in the background, each running turn re-opened by its stored run handle (06), runs the host does not keep swept, and the daemon synced; the `sub-starts` queue resumed; each `held-starts` row failed with `capRestartedLine`; every thread token whose turn is not running revoked; owed notices delivered; builders admitted; the slates loaded, with every run that read `running` rewritten as failed by a host restart in one batch (`slates.ts:427-462`).

2.18. Hydration MUST be lazy and MUST run before the lock is taken on the `wsp up` road (`cli/service.ts:60` before `:62`), as `servesNothing()` forces it; on the desktop's in-process road it runs at the first verb or the start sweep (`cli/serve.ts:216-224`). A start that hydrates in another order changes what the first client sees.

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

2.19. The Rust host MUST stamp the `shape` row with its own shape number, one past the TS host's `STATE_SHAPE = 2` (`protocol/src/wire/daemon-version.ts:742`), and MUST refuse a database of any other shape with the TS host's refusal and its fix, move the file aside and start fresh (`store.ts:51-54`). It never reads a TS host's `state.db`. Whether the owner's threads are exported is the map's open question; with the layout unchanged an export is a copy of rows.

2.20. The Rust host MUST run SQLite on the state owner's own thread, synchronously, as node does, so a write is durable in the WAL before the next task runs (03, rule 3.1). The passive checkpointer keeps its own connection on its own thread.

## Secrets at rest, unchanged

The port keeps every secret where it is today; changing where a secret rests is a security ticket after the port, not a porting choice. In plaintext inside `state.db`, protected by its 0600 mode and the 0700 folder: running turns' tokens (`sessions[].turnToken`), pairing codes, agent setup env values (`agent-setup.ts:3-6`), image vault blobs. Hashed: device tokens, as sha256 hex (`devices.ts:90,181`). In 0600 files: `host-token`, `.env`, `servers.env`, `relay.json`, `hosts/*.json`, `slates.secrets.json`, the key pairs. The one encrypted artifact is an image export: scrypt (N 2^15, r 8, p 1) then AES-256-GCM (`engine/src/vault-cipher.ts:25-66`).

2.21. The Rust host MUST NOT log, print or send any value from the list above, and MUST blank the request fields `token key rows code passphrase env envs headers sudoPassword` one level deep in its refusal log lines, withholding the sentence past 256 secret values (`protocol/src/wire/runtime.ts:1033`; serve.ts:359-374).
