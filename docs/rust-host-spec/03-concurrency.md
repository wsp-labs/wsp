# 03 Concurrency, ordering and seeded replay

Who owns which state, how work interleaves, what orders events, and how races are replayed from a seed. Every `file:line` is at `a3b870a6e`; RT means `packages/runtime/src`.

## How the TS host runs

One Node process, one JavaScript thread. The runtime is one object graph built once: `createRuntime` makes an empty context and fills it with 35 `Object.assign` calls, 33 of them `*Area(ctx)` areas plus the core and the activity door (RT/runtime.ts:433-497); every area reads the others off the context at call time (RT/context.ts:960-962). The only second thread is the SQLite checkpointer, which holds no runtime state (RT/sqlite-store.ts:78-131).

So "atomic" in the TS host means "no `await` between the read and the write". Every op frame starts its own async task (RT/serve.ts:636-638), nothing queues the frames of one socket, and tasks interleave exactly at their awaits. The store is effectively synchronous: `node:sqlite` runs each statement inline, so `await store.put()` only yields a microtask (RT/sqlite-store.ts:350-383), and a store write is a mutation at the call.

The shared state (RT/context.ts:158-238; RT/runtime.ts:824-998):

| State | Holds | Main writers |
|---|---|---|
| `live: Map<workspaceId, LiveWorkspace>` | each workspace's record, machine and the promises of its phase moves (`waking`, `napping`, `adopting`, `deleting`, `checkoutReading`, `prReading`) | create, hydrate, attach, drop (RT/machines/machines.ts:514,537) |
| `sessions: Map<rowId, SessionEntry>` | one row per turn; the latest turn of a thread is the last in iteration order | start's hold row (RT/threads/sessions.ts:520), the re-key when the harness names its session (RT/threads/turns.ts:649-650), boot, drops |
| `threadRecords: Map<threadId, ThreadRecord>` | per thread: access, read, settled, snooze, order | first turn, `mark`, `access`, boot, cap trimming |
| `projectsHeld`, `builders`, `landing`, `preparing`, `copyBuilds`, `execs`, `heldAtClose`, `bootWork` | projects, image builders, fork slots, joins, execs in flight | as named in RT/runtime.ts:824-976 |
| transcript state | indexes, pending events and bytes (the held transcripts, at most 4, are the blob road's alone, rule 2.31) | `record()`, flushes (RT/threads/transcripts.ts:363-714) |
| area-private closures | kept agents, cap waits, owed lines, sub-spawn queue, place sockets, status meters, slate queues | each area (RT/threads/threads.ts:102-384; RT/machines/*.ts; RT/places/context.ts:51-67; RT/status.ts:321-363; RT/slates.ts:295-302) |
| the bus | listeners and the event ring | `emit`, `pass` (RT/types/events.ts:59-98) |

What mutates it: op handlers; bus listeners, which run synchronously on the emitter's stack (44 registrations: 35 `bus.on(`, 8 in the status tracker, 1 `events.on(`); timers; turn output, which is polled from log files, never pushed (06); daemon events; place sockets. There are no file watchers in the runtime.

## Proposal 2: one thread owns the state

> **Proposal, accepted by the owner (26 Oct 2026, #2442)** (ticket decision 2). Pick: all host state lives on one thread, a tokio current-thread runtime with a `LocalSet`; op handlers, bus listeners, timers and process readers are local tasks on it, and reach state through one context handle whose borrows never cross an `.await`. Anything that must leave the thread (blocking file work past a small write, archive packing, the checkpointer) runs elsewhere and comes back as a future. The ticket's form, one actor task that every other part sends a message per read and write, is the alternative, and this spec does not pick it.

Why one thread rather than one actor:

- **The same interleavings.** Local tasks on one thread switch only at `.await`, as Node's tasks switch only at `await`. Every TS guard in this chapter (the same-tick take in `spawnGuard`, "nothing below this loop may await" in `sessions.start`, the join fields on a live entry) works unchanged, and every unguarded gap stays exactly as racy as it is today, which is what parity means for races.
- **Code that reads like the TS.** With an actor, every `ctx.live.get(id)` becomes a message and every read-await-write handler becomes a hand-made state machine; the order of TS awaits is lost or rebuilt by hand. The pilot's cheap models passed because the Rust looked like the TS (13). A port where each state access is a message cannot be checked line by line.
- **Seeded replay comes free.** A single-threaded executor whose only inputs are seams can be driven in any order a seed chooses (below).
- **It is what the Rust side already does.** Every tokio runtime in `daemon/` is current-thread (`wsp-daemon-bin/src/main.rs:31-35`). Node is single-threaded too, so there is no throughput to lose.

The cost: one thread does all the work, as in Node; a slow synchronous step (a large JSON parse, a big transcript read) stalls every client, as it does today. Moving such a step off the thread is an after-port ticket per case, never a porting choice.

Rules, holding under Proposal 2:

3.1. All host state MUST be owned by one thread: the runtime's maps, the store connection (02, rule 2.20), the bus and its ring, the area closures. No `Mutex` or `RwLock` guards host state; no state is `Send`.

3.2. Each op frame MUST start its own local task, as RT/serve.ts:636-638 does (rule 4.16). Bus listeners MUST run inline on the emitter's call, not as tasks.

3.3. A borrow of shared state MUST NOT be held across an `.await`, nor while calling out: an `emit`, another area's method, an injected trait (a road's `kill`, a store write) or a callback. The context's accessors take a closure (`ctx.with(|s| ...)` returning a value), never hand out a guard, and `clippy::await_holding_refcell_ref` is denied workspace-wide (12). The large-module pilot's ports called a road's kill while holding their state, which deadlocks on a synchronous end; in the one-thread model the same mistake is a `RefCell` double borrow, a panic the first test that re-enters catches.

3.4. A port MUST keep the TS await points between a read and a write of shared state: an `.await` where the TS awaits, and none where the TS has none in such a stretch. Adding an await inside a TS synchronous stretch opens a race the TS host does not have; removing one closes a race the tapes may depend on. The stretches the TS code names as synchronous on purpose include `sessions.start`'s loop exit to `runTurn` (RT/threads/sessions.ts:584-589,742-780), `spawnGuard`'s count and take (RT/account/rules.ts:290-334), `workspaces.create`'s place take (RT/projects/workspaces.ts:144-157), `events.subscribe`'s replay and attach (RT/serve.ts:1178-1197) and every join that checks a field and assigns it with no await between.

3.5. A panic in a task MUST be caught at the task's edge, logged as the TS host logs an uncaught error (06, rule 6.14), and the host keeps serving. State the task half-changed stays half-changed, as after a JS exception mid-mutation.

## The bus

`eventBus()` (RT/types/events.ts:59-98) is not Node's EventEmitter.

3.6. `emit(event)` MUST, in order: stamp `seq = ++head`, push on the ring, trim the ring to 5,000, then call every listener of the event's type in registration order, then every `"*"` listener in registration order, inline (events.ts:81-88). It catches nothing: a listener's failure stops the later listeners and reaches the caller of `emit`. In Rust a listener returns `Result<(), HostError>` and `emit` returns the first error.

3.7. A listener MAY emit; the nested event MUST be delivered at once, depth first, with a larger `seq`, so a later listener of the outer event sees the inner one first. Sockets are `"*"` listeners (RT/serve.ts:1188), so a nested emit from a typed listener reaches a socket before the outer event, and a socket can see `seq` go down. The host works around this where it matters: `session.row` pushes wait until the moved event is out, "since a push inside that event's own emit would reach a socket before it" (RT/machines/boot.ts:803-806). The listener set MUST be iterated live: a listener added during delivery is reached, one removed before its turn is skipped (JavaScript `Set` iteration). A Rust `Vec` iterated by index with tombstones for removals gives this; iterating a snapshot does not.

3.8. `pass(event)` MUST deliver to every listener with `seq` equal to the current head (absent when nothing was emitted yet), and put nothing in the ring (events.ts:91-96). `record(event)` (RT/threads/transcripts.ts:677-684) MUST stamp `at` (an `session.earlier` keeps its own), stamp `pos`, append to the transcript, then `emit`, then for a thread's `session.start` or `session.end` emit `thread.head`. So a transcript row has two numbers: `pos`, dense per workspace and never reissued, and `seq`, global per process.

3.20. The bus MUST hold exactly this state and no more (RT/types/events.ts:60-98): a map from type to a listener set, the ring, a stream id made once per bus by `randomUUID()`, and `head = 0`. An empty set is never removed, there is no cap on listeners, and `on` returns a function that deletes that one listener. `emit` builds `{...event, seq: ++head}`, so `seq` is the event's last key unless the event already carried one, which keeps its place; the same object goes into the ring and to every listener, with no copy. `emit` takes a `seq` and fills the ring even when no listener is attached. The ring is trimmed with `splice` once it passes 5,000, so it holds the events `head - 4999` to `head`.

3.21. `pass` MUST stamp `seq: head` only when `head > 0`; before any `emit` it delivers the event object as given, with no `seq` key (events.ts:92). Its users: a cost tick whose figures did not move (RT/status.ts:831), `session.context` (RT/threads/turns.ts:490), `aside.text` (RT/threads/sessions.ts:1204) and `session.row` (RT/machines/boot.ts:836-837). A passed row's `seq` is the head when it is delivered, not when the row moved.

3.22. `since(after, stream)` MUST answer every case as below (events.ts:74-80), with `oldest = head - ring.length + 1` and `foreign` meaning a `stream` was given and differs from this bus's, the empty string included:

| `after` | Answer |
|---|---|
| absent | no replay, no gap; `stream` is not read at all, so a foreign one is no gap |
| set, and `foreign` | gap, nothing replayed |
| greater than `head` | gap |
| less than `oldest - 1` | gap (so `after: 0` once the ring has wrapped past 5,000 is a gap) |
| `oldest - 1` to `head` | every event after `after`, oldest first; `after === head` replays nothing and is no gap |

The schema takes `after` as a non-negative integer and `stream` as any string (`protocol/src/wire/runtime.ts:198-207`); a negative or fractional `after` is a schema refusal (05). The reply is `{id, ok: true, seq: <head>, stream, gap?}` in that key order (RT/serve.ts:1193): `seq` is the head, never the cursor, and `gap` is absent rather than `false`. At most 5,000 events are replayed. On a fresh host, `after: 0` is no gap, while `after: 1` and a foreign `stream` with `after` set are:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up","protocol":1}}
{"in":{"id":2,"op":"events.subscribe","after":0}}
{"out":{"id":2,"ok":true,"seq":0,"stream":"48b5cb17-faf7-49a5-9eaf-5b1b83fd4a17"}}
{"in":{"id":3,"op":"events.subscribe","after":1}}
{"out":{"id":3,"ok":true,"seq":0,"stream":"48b5cb17-faf7-49a5-9eaf-5b1b83fd4a17","gap":true}}
{"in":{"id":4,"op":"events.subscribe","after":0,"stream":""}}
{"out":{"id":4,"ok":true,"seq":0,"stream":"48b5cb17-faf7-49a5-9eaf-5b1b83fd4a17","gap":true}}
```

With one event in the ring, `after` equal to the head replays nothing and is no gap, and the schema refuses a negative or a fractional cursor:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up","protocol":1}}
{"in":{"id":2,"op":"preferences.set","patch":{"theme":"dark"}}}
{"out":{"id":2,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":3,"op":"events.subscribe","after":1}}
{"out":{"id":3,"ok":true,"seq":1,"stream":"7f8d5baa-71d5-44c9-bfea-8925e1cf5cf9"}}
{"in":{"id":4,"op":"events.subscribe","after":-1}}
{"out":{"id":4,"ok":false,"error":"[\n  {\n    \"code\": \"too_small\",\n    \"minimum\": 0,\n    \"type\": \"number\",\n    \"inclusive\": true,\n    \"exact\": false,\n    \"message\": \"Number must be greater than or equal to 0\",\n    \"path\": [\n      \"after\"\n    ]\n  }\n]"}}
{"in":{"id":5,"op":"events.subscribe","after":1.5}}
{"out":{"id":5,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"integer\",\n    \"received\": \"float\",\n    \"message\": \"Expected integer, received float\",\n    \"path\": [\n      \"after\"\n    ]\n  }\n]"}}
```

With no `after`, a foreign `stream` is not read and the answer is no gap:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up","protocol":1}}
{"in":{"id":2,"op":"preferences.set","patch":{"theme":"dark"}}}
{"out":{"id":2,"ok":true,"preferences":{"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}}}
{"in":{"id":3,"op":"events.subscribe","stream":"00000000-0000-0000-0000-000000000000"}}
{"out":{"id":3,"ok":true,"seq":1,"stream":"9fd64943-697d-4574-b452-19d22d5fd9d2"}}
```

## The serialization helpers

The TS host has six kinds, each a few lines repeated per site. `wsp-core` gives one generic helper per kind; each site uses the helper of its kind.

| Kind | Semantics | TS sites |
|---|---|---|
| **chain** | FIFO: `next = (tail ?? resolved).then(work)`; the stored tail swallows errors so one failure does not block the next. Two forms differ in what the caller gets: its own result and rejection, or the swallowed promise (so it never sees a failure) | caller gets its own result: the transcript queue, the only one that orders reads too (RT/threads/transcripts.ts:292-297); a thread's slate (RT/slates.ts:298-302); the roots file per machine (RT/machines/daemon.ts:72-80); git writes per workspace (RT/projects/folders.ts:203-212); a computer's record (`change()`, RT/places/records.ts:336-354); a turn stream's hand-overs and parked lines (RT/machine-exec.ts:352-356,663,680-710). Caller gets the swallowed promise: `persistSessions` (`:617-657`, the snapshot taken at call time); kept images (`:24-37`); sub-machine life (`serial`, RT/machines/sub-life.ts:170-178). One chain for the whole host, not per key: preferences (RT/account/preferences.ts:17,72-83); devices (RT/devices.ts:171-177); the usage ledger (RT/usage.ts:211-215); held-start writes (RT/threads/threads.ts:376-379). Also: cost histories; shell tags; `session.row` pushes (`rowsOut`, RT/machines/boot.ts:835-844) |
| **barrier** | per key, the work starts at once, not after the last; the key holds a promise that settles when every holder so far has settled (`Promise.all([prior, mine])`), so a waiter waits for all of them and a second holder cannot release the first's hold | the host's writes into a harness session file (`hostWrites`, RT/threads/threads.ts:173-195); a thread's next turn held behind a stop (`holdNext`, RT/threads/sessions.ts:47-68) |
| **join** | a second caller gets the first caller's promise | a live entry's `waking`, `napping`, `adopting`, `deleting`, `checkoutReading`, `prReading`; `syncDaemon` per machine; `oneFolder` per project and branch; `preparing`; `copyBuilds`; `ready()`; `bootWork`; backend facts per place; warm launches; a usage reset (RT/usage-reset.ts:82); sign-ins per target; a flush burst; the host's orphan sweep and restart (`??=`) |
| **refuse** | a second caller gets an error | `forking` names (RT/machines/create.ts:212,274-278); a setup per computer (RT/places/setup.ts:161-186); a doctor per computer (RT/serve.ts:401, refusal at `:1003-1012`) |
| **drop** | a second caller returns at once with nothing done and no error | `rereading` in `adoptGone` (RT/machines/machines.ts:729-740) |
| **take and release** | take a claim in the same synchronous step as the check, release in `finally` | `hold(purpose)` for keyed creates and the sweep's replay (RT/machines/machines.ts:43-54); `claiming()` and `ctx.inflight` (`:183-230`); the idle refcount (RT/idle.ts:62,101-110); the thread cap (RT/threads/threads.ts:374-522); the spawn slot (`spawnGuard`); the sub-spawn queue's `claimStart` |

3.9. Each helper MUST keep its kind's semantics exactly, including those that look like bugs: `hold(purpose)` is not FIFO (waiters loop and re-check on resume, so a caller arriving between a release and a waiter's resume takes the key first, RT/machines/machines.ts:43-54); a chain of the first form hands its caller its own rejection while the key's tail swallows it, and one of the second form never shows its caller a failure; a barrier starts every holder's work at once. A site keeps its own form: moving `persistSessions` to the first form would surface store failures the TS host swallows.

3.10. Of the handlers that read, await and write (28 were traced for this chapter), most re-check after their awaits (`refuse()` in `sessions.start`, `stillLive()` in `proven`, the generation stamps in `pollTick`). Five are not guarded in the TS host and MUST stay unguarded in the port, each filed as an after-port ticket (14): two opening starts can pick the same thread id before either holds it (RT/threads/sessions.ts:290 to 523); `sessions.access` writes the thread record from a copy read before three awaits (RT/threads/sessions.ts:1032-1044); `mark` does the same (RT/machines/boot.ts:928-941); `sessions.forget` deletes the rows it saw before its awaits (RT/threads/sessions.ts:1453-1480); two deletes of a worktree workspace can both pass the join check across `refuseChanged` (RT/projects/workspaces.ts:621-630).

## Ordering

3.11. Map iteration order MUST be insertion order wherever the TS iterates a `Map` or `Set`: `foldThreads` takes the last row of a thread as its latest (`protocol/src/views/session.ts:380-392`), `pushRows` takes `held.at(-1)` (RT/machines/boot.ts:811-826), `latestOn` gives ties to the later row (RT/threads/threads.ts:91-97). `Map.set` on an existing key keeps its place; delete then set moves it to the end (the re-key at RT/threads/turns.ts:649-650). In Rust these are `IndexMap`s, and every removal is `shift_remove`: `IndexMap::remove` is `swap_remove`, which moves the last entry into the hole and silently reorders.

3.12. Events on one socket MUST arrive in emit order (rule 4.21 covers replies). Exceptions the clients live with: a nested emit reaches later listeners first (3.7); `pass` events share the head's `seq`; `session.row` pushes are collected per tick, queued from a microtask (`queueMicrotask(pushRows)`, RT/machines/boot.ts:849) onto the `rowsOut` chain, and passed after `await placedRows(rows)` (`:835-844`), so a row can arrive after a later ringed event and after a reply sent in the same synchronous run; only a refused spawn and a queued start that failed wait for their rows before replying (`rowsSent()`, RT/machines/sub-spawn.ts:563, RT/machines/sub-queue.ts:272). In Rust the microtask is a task spawned at the end of the current one.

3.13. There is no per-workspace or per-thread op lock beyond the helpers above. What gates: every op naming a workspace awaits `ready()` and that workspace's `bootWork` through `entryOf`, except with `now: true`, which skips the `bootWork` wait (RT/machines/boot.ts:628-641); phase moves join; a thread runs one turn at a time by `sessions.start`'s loop ("Two processes on one harness session corrupt its transcript", RT/threads/sessions.ts:584-589). Two sends into different threads of one project folder run side by side and share one `pos` space.

3.14. Another process on the same `state.db` (`wsp init`, `wsp doctor`) is ordered only by SQLite's busy wait and the builders' `heldBy` stamps with a pid and a heartbeat (RT/images/builders.ts:142-148; 15 minutes and 5 minutes, RT/types/internal.ts:875-877).

3.23. `events.subscribe` MUST run with no await, in this order (RT/serve.ts:1178-1197): read `since`; build the socket's filter and `costMoved` closure; attach the listener to the bus's `"*"` and to the forwards, init, doctor and release doors present; send the reply; replay each event through the filter; with `creates: true`, pass each create in flight's last stage. Nothing can come between these steps. Each call attaches a new listener with its own `costMoved`, so a second call doubles every later event (04, rule 4.22); there is no `events.unsubscribe`, and only the socket's close detaches. Replayed events carry their ring `seq`; create stages and passed frames carry the head at the time, or none.

3.24. An event frame's key order MUST be the emitter's object literal, then `seq` last (rule 3.20). For a transcript event, `record` spreads the event and then sets `at` and `pos`, so each keeps its place when the event already had it and otherwise lands after the event's own keys, before `seq` (RT/threads/transcripts.ts:680). A `session.row` frame is `type, workspaceId, threadId?, id, row`, then `seq`; a row the host stopped holding is `{type: "session.row", workspaceId, threadId?, id}` (RT/machines/boot.ts:835-844). A batch of rows that fails warns `a moved row was not pushed: <why>` and goes again with the next batch; the set of rows already told is pruned once it passes `2 * sessions.size + 64` (`:840-844`).

## Sockets, close and restart

3.25. Sending MUST apply no backpressure: `send` drops the frame silently when the socket is not open and otherwise writes it (RT/serve.ts:624-628). There is no check of buffered bytes, no per-socket cap, no drop of old frames and no cut of a slow reader; the outgoing buffer is the websocket library's and unbounded. A Rust host on tokio MUST keep an unbounded queue per socket, because a bounded one either blocks the owner thread (rule 3.1) or drops frames the TS host would have sent.

3.26. A socket's close MUST run every detach in the order it was registered and then forget them (RT/serve.ts:577-581): bus and door listeners, daemon channels (`:572-575`), slate holds, sign-in waits, and its status watch. When the last status watcher goes, the status poll and the cost tick stop and their remembered figures are cleared (RT/status.ts:876-888); nothing else stops them, `rt.close()` included.

3.27. `rt.close()` MUST leave the bus alone: it removes no listener, clears no ring, and `emit` has no closed guard (RT/runtime.ts:1225-1283). It stops the roots recheck, the sweep, idle, alerts, slates, the grace, snooze and resume timers, the pull request poll and the gone watch, flushes every transcript (02, rule 2.33), and awaits what 06's close order lists: copies moving, a sweep in flight, the disk watch, the daemon syncs, the kept writes and the agent asks, the kept agents for at most 2 s, the local wiring, the place door, a status tick, the builders' writes, and the transcript queue and the index flushes as they stand at that moment (`runtime.ts:1230-1282`). Every other chain (git writes, roots writes, preferences, devices, usage, slates, host writes, held-start writes, sub-machine life) is neither awaited nor cancelled; its waiters run on until the process exits. 06 has the whole close order.

3.28. Nothing about the bus survives a restart: a new process has a new `stream` and `head = 0`, so its first event is `seq` 1, and a transcript row never carries `seq` (`packages/runtime/test/serve-replay.test.ts:234-241`). A client that comes back with its old cursor and its old `stream` gets a gap, with nothing replayed (`:219-221`). One that comes back with an old cursor and no `stream` gets whatever the new process's ring holds after that number, read as if it were the old stream's, unless the cursor is past the new head, which is a gap (`:176-184,227-229`).

> **After the port.** A cursor sent without its `stream` is trusted across a restart, so a client that drops the stream id can be replayed another process's events under its old numbers. The port keeps it. No client today sends one without the other: the web client sends `after` and `stream` together once it has both (`apps/web/src/protocol/client.ts:372-390`), and the command line and `wsp-mcp` send no cursor at all (`packages/host/src/verbs/client.ts:379`; `daemon/crates/wsp-mcp/src/client.rs:306`).

What no tape here shows, and why: the checker sends each frame only after the previous reply, so a tape cannot interleave two ops, a nested emit, or a `session.row` against a reply; a ring past 5,000 events needs 5,001 events, each a frame, which is minutes of tape. Those are pinned by `serve-replay.test.ts:83-243` and by the seeded replay below.

## Time and ids

The TS clock is `Clock { now(); schedule(fn, ms, {unref}) => cancel }` (RT/clock.ts:4-23): no interval, every repeating timer re-arms itself. It reaches the idle windows, the transcript debounce, the status tickers and the pause and wake budgets (205 lines with `clock.now()` in `runtime/src`), but 44 `Date.now()` calls and 8 `Date.now` references passed as functions (`opts.now ?? Date.now`), 11 `new Date()` and the raw timers (the turn log polls, the daemon link's heartbeat and backoff, the place `seen` interval, slate timers, the host's sweep and release timers) bypass it. Tests fake the clock with `fakeClock()` (`packages/runtime/test/fake-clock.ts:8-39`: `advance` runs each due timer at its own time) in 47 runtime test files and 1 host test file, and `vi.useFakeTimers` in 13 (8 runtime, 5 host; 15 across every package) where the code uses raw timers. Counted with `grep -rl` over each package's `test/`.

3.15. Under Proposal 2b, every time read and every timer in the Rust host MUST go through the clock trait, the TS's raw sites included, so a replay controls all of them. The real clock reproduces the TS timing; this changes no behaviour and is one of the two seams the port adds on purpose (rule 1.4). `unref` has no meaning in Rust (the host exits by its close road, 06) and is dropped.

3.16. Under Proposal 2b, ids MUST be made through one `Ids` trait (UUIDs, random hex of N bytes, random base64url tokens), the second seam the port adds (rule 1.4), with the shapes of rule 2.15, so a replay can seed them. The real implementation uses the OS's randomness, as `node:crypto` does. `Math.random()` appears only in backoff jitter and temp names (`protocol/src/wire/limits.ts:104`; `engine/src/errors.ts:204,295`; `host/src/analytics.ts:109`; `host/src/relay.ts:316`; the `writeOwn` temp name, 02) and goes through the same trait.

3.17. Timers that matter to clients, with their values, MUST be kept:

| Timer | Value | Where |
|---|---|---|
| status poll, while any socket subscribed | 15 s, first run at once, re-armed before its body so slow ticks overlap | RT/status.ts:266,761-887 |
| cost tick | 5 s, the first one interval after the watch starts, not at once; and on `workspace.created/woken/upgraded/napped/gone`. A tick emits `workspace.cost` when the figures moved and `pass`es it when they did not | RT/status.ts:340,830-831,840-847 |
| idle window per workspace | 20 minutes by default (`NAP_AFTER_MS`); a nap the provider refuses starts a full window again, the backstop where the refusal stands (`machines.ts:796,811-818`), and only a nap whose move went unanswered is tried again after 15 s (`retryMs = POLL_INTERVAL_MS`, `idle.ts:89-91`); provider backstop twice the window or 6 h | RT/idle.ts:14-18; RT/machines/machines.ts:825; `protocol/src/place-state.ts:54` |
| transcript flush | 250 ms after the first non-delta event (02, rule 2.32) | RT/threads/transcripts.ts:710-713; `TRANSCRIPT_FLUSH_MS`, RT/types/internal.ts:554 |
| kept agent | expires 30 minutes, warm 3 minutes | `protocol/src/wire/limits.ts:45,66` |
| owed lines | 30 s doubling to 10 minutes, for 60 minutes | RT/types/internal.ts:99-101 |
| pull request poll | 3 minutes watched, 15 idle, 30 s for a slate's pending check (07, rule 7.19) | RT/account/pull-requests.ts:223-240; `PR_POLL_MS`, `PR_POLL_IDLE_MS` in `protocol/src/pull-request.ts:118,121`; RT/types/events.ts:23 |
| worktree sweep | 10 minutes while a made worktree stands | RT/projects/folders.ts:274-297 |
| turn log poll | local 100 ms (1 s at rest); machine 1.5 s, 200 ms when hurried (for 3 s after a write, `HURRIED_FOR_MS`, and 30 s from launch, `OPENING_HURRY_MS`), 10 s at rest | RT/local-exec.ts:68-71; RT/machine-exec.ts:196-211,301 |
| daemon link | ping every 10 s (one dial path passes its own `heartbeatMs: answersMs`, RT/machines/reach.ts:224,255), no pong by the next beat ends it; redial 500 ms doubling to 10 s plus `floor(random * 250)` ms | RT/reach.ts:56,117-184; `limits.ts:101-105` |
| place `seen` write | 60 s (`SEEN_EVERY_MS`, RT/places/helpers.ts:232) | RT/places/link.ts:184-186 |
| orphan sweep | at start (waited 2 s), then every 10 minutes, never overlapping | `host/src/server.ts:168-172,711-745` |
| release check | 30 s, then 6 h, never twice within 10 minutes unless a person asks | `host/src/release.ts:20-23` |

## Proposal 2b: seeded replay

> **Proposal, accepted by the owner (26 Oct 2026, #2442)** (the map's harness item 8, and ticket decision 2's "seeded replay for races"). Pick: a replay build of the Rust host whose every input is a seam (frames, child output, daemon frames, timers, ids, the clock), driven by a scheduler that, at each step, picks which ready input is delivered next from a seeded generator, runs the owner thread until no task can run, and repeats. The seed is printed on failure and a run with that seed reproduces it exactly.

How it works:

1. The host is built with the fake clock, the seeded `Ids`, fake processes (a turn whose log lines are scripted), the fake daemon built from `daemon/fixtures/contract`, and a tape as its client input.
2. The driver keeps a set of pending inputs: the tape's next frame, each fake process's next chunk, each fake daemon reply, each timer due at the current virtual time. At each step the seed picks one, the driver delivers it, and the owner thread runs to quiescence. When nothing is pending and nothing can run, virtual time jumps to the next timer.
3. Inputs keep the order their source imposes (a socket's frames stay in order, a process's chunks stay in order); the seed only interleaves sources.
4. A run fails when an invariant breaks or when a tape's `out` frames come out in an order outside the allowed windows (#2443's normalizing).

What the oracle is for races: not the TS host, which has no such hook. The TS host gives the outcomes of the orders its tapes recorded; the replay checks that every other order keeps the invariants the TS code states, and these are written as checks:

- a thread has at most one running row (RT/threads/sessions.ts:584-589);
- a restarted turn writes no row twice (runtime invariant 2);
- `spawnGuard` never lets a root exceed its machine count (`packages/runtime/test/spawn-guard.test.ts:1203`);
- two preference patches both land (`packages/runtime/test/preferences.test.ts:132`);
- a computer's record keeps every field through concurrent writers (runtime invariant 6);
- a create in flight is never swept (#2255) and a replay and a create of one key make one box (#2278);
- a transcript `pos` is never issued twice (`thread-head.test.ts:99`);
- `events.subscribe` with a cursor loses and doubles nothing (`serve-replay.test.ts:100`).

3.18. The replay build MUST run in the comparison harness's seeded mode (`cargo xtask parity --seeds <n>`), and a race fix in the Rust host MUST come with the seed that showed it.

3.19. The five unguarded gaps of rule 3.10 will fail some seeds. Until their after-port tickets land they are listed in the harness's accepted list with the seed that shows each, so a new failure is never hidden behind a known one.
