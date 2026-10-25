# context
Source: packages/runtime/src/context.ts (959 lines, types only) at 65f6b89c7, with the values it is filled with at packages/runtime/src/runtime.ts:433-497 and :986-990. Tests: none of its own; `pipe.test.ts:551`, `proc-scopes.test.ts:44,174-176` and `deleted-workspace-state.test.ts:23,41,66` build partial contexts. Rust: `src/context.rs`, tests `tests/context.rs`.

## What it does
The runtime's one shared object. `createRuntime` makes it empty (`{} as RuntimeContext`), then fills it with the core and 33 areas in file order (`Object.assign`, runtime.ts:433-497); every area reads the others off it at the moment it calls them, never copying a member while areas are still being built (context.ts:957-958). `RuntimeState` holds the lets more than one area reads or writes (:157-175). In Rust this is the context handle of Proposal 2: state reached only through `with(|s| ...)` (rule 3.3), areas through late-filled slots.

## Behaviour
1. `RuntimeState` fields and their first values (context.ts:157-175, runtime.ts:986-990): `say_stops: None`, `reads_since: 0`, `roots_recheck: None`, `owner: ""`, `copies_moving: None`, `sweep_timer: None`, `sweeping: None`, `sweep_stopped: false`, `beat: None`, `closed: false`, `closing: false`, `ticking: None`, `preferences_held: None`. `RuntimeState::default()` is exactly these. The interval in `roots_recheck` and the timer in `sweep_timer` are clock `Cancel`s (rule 3.15).
2. `StateCell<S>` is the handle over shared state: `with(|s| ..) -> R` lends `&mut S` for the closure only and hands back the closure's value; there is no way to hold a borrow past the call (rule 3.3). A `with` inside a `with` on the same cell panics (a `RefCell` double borrow): the mistake the pilot made, caught by the first test that re-enters. Clones share one state.
3. `Late<T>` is one member of the context filled after construction: `fill(v)` once; `get()` hands back a clone of what was filled (`Rc` for an area); `get()` before `fill` panics with `the runtime read <name> before it was built`, as calling an `undefined` member throws in the TS; a second `fill` panics with `<name> was built twice`. `is_filled()` reads without panicking.
4. A member is read when called: code that captured the context before an area was filled reaches that area once it is (runtime.ts:433-497 fills areas after earlier areas captured `ctx`).
5. `RuntimeContext` holds `state: StateCell<RuntimeState>` and one `Late` slot per area of context.ts:959, named in snake case: `activity`, `kinds`, `rules`, `transcripts`, `slates`, `channels`, `proc_scopes`, `records`, `reach`, `pull_requests`, `daemon`, `machines`, `boot`, `create`, `sub_spawn`, `sub_queue`, `sub_life`, `folders`, `start_from`, `workspaces`, `agents`, `threads`, `turns`, `sessions`, `builders`, `golden`, `image`, `projects`, `project_icons`, `usage`, `conversations`, `waits`, `status`, `preferences`, and `core` for `RuntimeCore`. A test fills only the slots its unit calls, as the TS tests pass partial objects.
6. Each area is a trait named as its TS interface (`KindsArea`, `RulesArea`, ...) with the TS member names in snake case (rule 1.3). This unit declares them; each area's members are added by that area's packet writer when its unit is ported, with the types they name (most live in `types/internal.ts` and `types/wiring.ts`, the core-types unit). `StatusArea` is this slice's: `status() -> Rc<StatusTracker>` and `adopt_lost(..)`, the latter added with `status.ts`'s caller in `wsp-machines`.
7. `ShownCreate`, `DetachedSend`, `CapHeld` and `Placed` (:542, :721-739, :656-662) are plain data types of this file; they move here with the types they name (`WorkspaceView` exists in `wsp-wire`; `ThreadCapWait`, `SessionUncappedEvent` likewise). `DetachedSend.outcome` is one of `steered`, `queued`, `started`, `held` (:722).

## Edge cases
- `with` that panics inside the closure leaves the cell usable afterwards (the borrow is released on unwind), as a JS exception mid-mutation leaves the object (rule 3.5).
- Reading `state` from inside an area's method that was itself called from a `with`: a double borrow, by design a panic.

## Behaviour nobody wrote down
- Rule 3.1: the context and everything in it is `!Send`; one thread owns it.
- Rule 3.3: no guard leaves `with`; `clippy::await_holding_refcell_ref` stays denied.
- Rule 1.5: area traits live here so a lower area crate can call a higher one through the context.

## Decisions
1. Late slots are `OnceCell`s; the TS's "read at call time" falls out of every access going through the slot.
2. The area traits ship empty in this slice; filling them is each area's port (Behaviour 6). Nothing in this slice calls an area member.

## After the port
- Nothing: the file holds no behaviour of its own.

## Tests
| Rule | Test (`tests/context.rs`) |
|---|---|
| 1 | `runtime_state_starts_as_create_runtime_fills_it` |
| 2 | `with_hands_back_the_closures_value_and_clones_share_the_state`, `with_inside_with_panics_as_a_double_borrow`, `a_panic_inside_with_leaves_the_cell_usable` |
| 3 | `a_late_slot_read_before_it_is_filled_names_itself`, `a_late_slot_filled_twice_panics`, `is_filled_reads_without_panicking` |
| 4 | `a_member_is_read_when_called_so_an_area_filled_later_is_reached` |
| 5 | `a_partial_context_serves_the_slots_a_test_filled` |
