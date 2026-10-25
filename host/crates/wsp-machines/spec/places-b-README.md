# wsp-machines, places slice b: specs

The TS host is the oracle: `packages/runtime/src/places/` at `65f6b89c7` (the dead-code cut, #2441). Every `file:line`
is at that commit and relative to that folder unless rooted. The slice is `types.ts`, `records.ts`, `views.ts` and
`manage.ts` (2,485 lines). Slice a (`places-a-README.md`) holds the rest of `places/` and `places.ts`'s assembly.

| Unit | Rust | TS | Spec | Tests |
|---|---|---|---|---|
| types | `src/places/types.rs` | types.ts (808) | places_types.md | `tests/places_types.rs` |
| records | `src/places/records.rs` | records.ts (374) | places_records.md | `tests/places_records.rs` |
| views | `src/places/views.rs` | views.ts (431) | places_views.md | `tests/places_views.rs` |
| manage_add | `src/places/manage/add.rs` | manage.ts:197-423 | places_manage_add.md | `tests/places_manage_add.rs` |
| manage_remove | `src/places/manage/remove.rs` | manage.ts:90-195, 494-720 | places_manage_remove.md | `tests/places_manage_remove.rs` |
| manage_settings | `src/places/manage/settings.rs` | manage.ts:122-145, 425-492, 722-871 | places_manage_settings.md | `tests/places_manage_settings.rs` |

`src/places/manage.rs` holds `ManageDoor`, its constructor and `ManageSetup`, which every manage unit uses.

## How the tests stand

The TS tests are whole-host tests (`createRuntime` plus `serveRuntime` over a websocket). Each case is ported at slice
b's own seam: `tests/places_b_common/mod.rs` builds the four areas on `place_door_context(opts)` with fakes for the
store, the devices, the clock, the recording, places-a's setup area (`FakeSetup` behind `ViewsSetup` and
`ManageSetup`), the link reads (`FakeLinks` behind `LinkReads`), a daemon reach, the socket, a backend and the roads.
Expected values come from the TS: `oracle/places_b*.oracle.ts` run the real TS door on fakes and write
`tests/fixtures/oracle/places_b*.json`; run one from the repository root with
`pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 <file>`.
An implementer never edits a fixture, an oracle or a test.

Every test of this slice goes red on `todo!()` until places-a's `context.rs` (`place_door_context`, `emit`, `watch`) is
implemented too: the fixture builds the context first. That is 75 TS lines and the first unit to implement.

## Rules every unit shares

places-a's README states them for the whole crate and this slice keeps each: one thread (`Rc`, `RefCell`,
`#[async_trait(?Send)]`, no borrow across an `.await` or a call out); insertion order (`IndexMap`, `shift_remove`);
every time read and timer through `ctx.clock` (the TS reads `Date.now()` raw at manage.ts:266,291 and `setTimeout`
at manage.ts:275, records.ts:154, views.ts:185); every failure a `wsp_words::exit::HostError` compared by message and
kind. Two of this slice's own:

- **A TS optional method is an engine `offers()` flag or an `Option` answer.** `at.placeLimits !== undefined` is
  `backend.offers().place_limits`, `backend.capacity` is `offers().capacity`, `listSnapshots` is
  `offers().list_snapshots`; a `PlaceRecording` method marked "absent" answers `None`.
- **The TS error classes are marks** on `HostError` (`types.md` T3): `is_place_login_refused(e)` where the TS asks
  `e instanceof PlaceLoginRefusedError`.

## Runtime invariants this slice holds (`packages/runtime/AGENTS.md`)

- **6. A computer's record changes only in its turn, through `change()`** (records.ts:336-354). `change` is defined
  here; `keep` is its one write. Named tests: `records::change_one_at_a_time_per_place_no_field_lost` (three writers
  over a yielding store, no field lost), `change_gone_record_writes_nothing`, `change_move_none_writes_nothing`,
  `change_chain_survives_a_failure`. Every record write in manage (dial, set, follow, unfollow, skip, the add's rewrite)
  goes through `change`; `forget`'s delete takes the same turn (views V18).
- Invariants 1 to 5 (turn logs, rows written once, origin off the token, gh as the person, transcripts) are not held
  by these files.

## What the TS leaves unclear

1. **Row key order.** `viewOf`, `hereRow`, `providerRow` and `withCap` build a row's keys in literal order
   (views.ts:262-313, records.ts:101-114); wsp-wire's `PlaceView` serializes in its struct order. The tests compare rows
   as JSON maps, which ignore order, so nothing here pins the wire order of a row. The harness's tapes are where it
   shows; ruling needed on whether struct order is accepted.
2. **The stored record's key order and unknown keys.** The TS spreads a record, so a key it does not know survives
   every write; serde drops it on the next write (types.md T4).
3. **`localeCompare` on ISO dates** (records.ts:42, views.ts:54) is read as a plain string compare; true for every date
   this host writes.
4. **`Number(answer.exitCode ?? -1)`** (manage.ts:417) on an exec answer whose `exitCode` is not a number gives `NaN`;
   the port's `i64` cannot. Pinned only for numbers and absence.

## After-port questions (look wrong in the TS; not changed in the port)

1. **A choice made during the floor is lost.** `floorWhileChoosing` ends by writing `{...pending}` from the record it
   was handed (views.ts:82), so `choose` called while the floor runs is overwritten with the old choices.
2. **`unfollow` keeps `sync`, `follow(NO_RECIPE)` drops it** (manage.ts:843 against :129): a computer taken off a
   recipe by the recipe's removal can still read Behind.
3. **`recipeChanged` asks again for every follower while the resolve fails** (manage.ts:770, `??=` on `undefined`).
4. **`set` reads a new login without the kept host key** (manage.ts:141): every other login road pins the key the add
   kept; this one would read a place file from a machine whose key changed.
5. **The reset word crash.** `placeSetRefusal` throws `Cannot read properties of undefined (reading 'words')` for a
   reset word outside `PlaceSettingWord` (found by the oracle). The wire schema refuses such a word first; a caller that
   skips it crashes `set`.
6. **The wired cloud on a host with no provider table** is listed (`providerIds`) but `remove` answers `{removed:
   false}` for it, since no backend answers (manage.ts:572; oracle `/remove/providerForget`).
7. **`dial` names the held computers off memory** (`kept`, manage.ts:346) where every other refusal reads the store.
8. **An add whose code cannot be issued blocks its address.** `devices.issue` throws outside the add's `try`
   (manage.ts:239), after the pending add was written at `connect` (:222): it never reads failed, so every later add
   of that address is refused as one still standing (A4) until a host restart or a remove.
9. **The installer's offered host key has no Rust home.** The TS reads `e.hostKey` off an installer error of kind
   `PLACE_HOST_KEY_KIND` (manage.ts:321); `HostError` carries no such field, so the job cannot get the key the person is
   asked to trust. `add_failure_host_key_rides_job` pins the kind and that no key is made up; the field needs a ruling
   (a `HostError` extra, or an installer error type of its own).
10. **The cut renamed a setting word that wsp-wire still spells the old way.** `PlaceSettingWord` at `65f6b89c7` is
    `max-machines` where it was `spawn` at `a3b870a6e` (packages/protocol/src/views/place.ts:468, changed by the cut,
    383708c78); wsp-wire's `PlaceSettingWord` (src/views/place_more.rs:585) follows `a3b870a6e`. A reset of
    `max-machines` cannot be written until wsp-wire moves; the settings tests reset `max-depth` alone.
11. **A changed host key never names the other computer** (manage.ts:639): the remove marks the login elsewhere with
    no `other`, even where another record holds that login.
12. **`force` reads `takesRuntime !== undefined`** (manage.ts:655): a report saying `takesRuntime: false` forces the
    leave too. It reads like `=== true` was meant.
13. **An update's login refusal keeps the sentence and drops the fix** (manage.ts:520, `PlaceLoginRefusedError(reached.refused)`).
14. **`unrecordedUnsaved` reads each checkout in turn** (manage.ts:190-193), each read bounded by 60 s, so a slow
    computer with many old checkouts holds the remove for minutes.

