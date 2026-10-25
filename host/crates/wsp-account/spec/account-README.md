# wsp-account, the account slice: shared rules

Slice of #2448: `packages/runtime/src/account/` at `65f6b89c7`, less `slates.ts` (wsp-slates) and less the
projects files (the other wsp-account writer). Every `file:line` below is at `65f6b89c7` under
`packages/runtime/src/`.

| Unit | TS | Rust | Spec |
|---|---|---|---|
| rules | `account/rules.ts` (334) | `src/account/rules.rs` | `rules.md` |
| status | `account/status.ts` (182) | `src/account/status.rs` | `status.md` |
| preferences | `account/preferences.ts` (156) | `src/account/preferences.rs` | `preferences.md` |
| pull_requests | `account/pull-requests.ts` (300) | `src/account/pull_requests.rs` | `pull_requests.md` |
| box_panels | `account/box-panels.ts` (263) | `src/account/box_panels.rs` | `box_panels.md` |
| proc_scopes | `account/proc-scopes.ts` (210) | `src/account/proc_scopes.rs` | `proc_scopes.md` |
| usage | `account/usage.ts` (344) | `src/account/usage.rs` | `usage.md` |
| channels_daemon | `account/channels.ts:1-280` | `src/account/channels/daemon.rs` | `channels_daemon.md` |
| channels_ptys | `account/channels.ts:280-580` | `src/account/channels/ptys.rs` | `channels_ptys.md` |
| channels_copy | `account/channels.ts:580-934` | `src/account/channels/copy.rs` | `channels_copy.md` |

## How an area is ported (every unit)

- **A1. One struct per TS area.** `xxxArea(ctx)` becomes `XxxArea::new(ctx: Rc<dyn XxxCtx>) -> Rc<XxxArea>`. The
  closure's `let` and `Map` state become fields in `Cell`/`RefCell` (`IndexMap` where the TS iterates a `Map`, so
  insertion order holds). Chapter 03: one thread, `Rc`, and no `RefCell` borrow held across an `.await` (rule 3.3).
- **A2. One context trait per unit** (ruling 6401022701, item 4). `XxxCtx` names exactly the `ctx.` members the TS
  file reads, one method each, with the TS member in its doc line. The join implements them on the
  context handle; the tests drive fakes. `join.md` lists what the join replaced. Nothing else of the runtime is reachable from a unit.
- **A3. Channels is one area in three files.** `channelsArea` is one closure whose parts call each other
  (`copyChannel` calls `ownDaemonChannel` and `servedChannel`, `servedChannel` opens the box panels). In Rust it is one
  `ChannelsArea` struct in `channels.rs` with one `ChannelsCtx`, and `impl ChannelsArea` blocks in `channels/daemon.rs`,
  `channels/ptys.rs` and `channels/copy.rs`.
- **A4. Errors.** `new Error(line)` is `HostError::new(line)`: no kind (exit class provider). `Object.assign(new
  Error(line), { kind: "usage" })` is `HostError::with_kind(line, Kind::Usage)`. `refusal(happened, fix, kind)`
  keeps `fix`. A test compares `message`, `fix` and `kind` whole, never a substring.
- **A5. Sentences come from wsp-words.** Every sentence a unit builds calls the wsp-words function of the same name.
  The tests hold the TS's own words, written by the oracle (`oracle/*.oracle.ts`, run under the repository's vitest;
  `tests/fixtures/oracle/*.json`), so a wsp-words drift shows as a red test here.
- **A6. Fire and forget.** `void p.catch(e => console.warn(line))` becomes `ctx.spawn(fut)` with the warning through
  `ctx.warn(line)`. A test that pins such a path asserts the warn line whole.
- **A7. Time.** Every `clock.now()`, `Date.now()` and `setTimeout`/`setInterval`/`clock.schedule` goes through the
  context's clock (`now()`, `schedule(ms, f) -> Cancel`). Tests drive a fake clock and never sleep. A TS `Date.now()`
  read beside `clock.now()` is the same fake reading in Rust (noted where the TS mixes them).
- **A8. Frames.** A daemon frame is a `serde_json::Map` built in the TS literal's key order (`preserve_order`); tests
  compare frames as serialized JSON strings, so key order is pinned.
- **A9. Object identity.** Where the TS keys a `WeakMap` by an object (a link, a pane, a caller), Rust keys by the
  `Rc` pointer of the handle (`Rc::as_ptr` cast to `*const ()`), and the entry goes when the handle's last owner
  drops it or the TS deletes it. Noted per unit.

## What survives a host restart

Only what a unit writes to the store: the project map (`projects`, rules), the preferences record (`preferences`,
id `PREFERENCES_ID`), the `unrecorded` and `dropped` collections (status), the `pty-threads` and `pty-tokens`
documents (channels_ptys, channels_copy), and the workspace record's `pr`, `prSent`, `branchThreads`, `tree` fields
(pull_requests, through `ctx.persist`). Every other map in these files is process memory and starts empty: the
rate-limit holds, the page cache entries, the sign-in hold, the plan read stamps, the landing slots, the port roots,
the pty owners, the shell tokens' in-memory copy (re-read from the store), the origin reads.

## Runtime invariants these files hold (packages/runtime/AGENTS.md)

- **3, origin off the token** (rules): every rule reads the caller's stamped `ThreadScope` (`scopeOf(caller)`) and
  never a thread or workspace id from the request's own words. Test `invariant_3_the_rules_read_the_stamped_scope_alone`.
- **4, nothing posts as the person through a copy's gh** (pull_requests): `postAsPerson` asks only this computer's
  daemon and never the copy's, even where this computer has no command line. Tests
  `invariant_4_post_as_person_never_takes_the_copy_road` and `invariant_4_post_as_person_drops_the_held_page`.

Invariants 1, 2, 5 and 6 live in other areas.

## Implementation order inside the slice

`rules`, `box_panels` and `proc_scopes` first (no other unit of the slice under them), then `channels_daemon`,
`channels_ptys`, `channels_copy` (copy opens the box panels), then `pull_requests`, `preferences`, `status`, `usage`.
