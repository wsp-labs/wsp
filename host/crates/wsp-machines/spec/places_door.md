# places.ts: the door's assembly

Port of `packages/runtime/src/places.ts` (50 lines, `makePlaceDoor` at :38-46) at `65f6b89c7`, into `src/places.rs`.
Tests: `tests/places_door.rs`. The file's other lines are the module's export lists (:20-31): the Rust names come out of
`places` by `pub use` (`src/places.rs`), each one the item places-a or places-b ported.

The TS has no test of this file alone: every place test builds the whole door through it. What the places-a and
places-b slices already test at their own seams is not repeated here. This unit pins only what the assembly decides.

## What it does

`makePlaceDoor(opts)` builds the context (`placeDoorContext`), then the areas in one order that each later one needs:
records, setup (over records), views (over records and setup), then the two halves of the door, link and manage, each
over all of them. It spreads the manage half over the link half into one `PlaceDoor` and sets `ctx.door` to it, so an
area that reads `ctx.door` after the assembly (setup reading `ctx.door()` at setup.ts:770, views reading the link half
at views.ts:126, :178, :236-238, :410) finds it. It answers the door.

## Rust shape

- `assemble_place_door(opts) -> Rc<AssembledDoor>` is the assembly; `make_place_door(opts) -> Rc<dyn PlaceDoor>` (the
  signature the rulings keep) is the same door read as the trait. `AssembledDoor { ctx, link, manage }` implements
  `PlaceDoor` by sending each method to the half that owns it (`LINK_HALF`, `MANAGE_HALF`).
- The TS builds views before the link half exists and lets views reach the link half through `ctx.door`. Views and
  manage take `Rc<dyn LinkReads>`: the assembly hands them `DoorLinkReads { ctx }`, which forwards to `ctx.door()` at
  the call, never at construction. That is how the cycle (the link half needs views, views read the link half) breaks.
- The setup area is handed to views as `Rc<dyn ViewsSetup>` and to manage as `Rc<dyn ManageSetup>`; the link half takes
  it as `Rc<PlaceSetupArea>`, records as `Rc<dyn LinkRecords>`, views as `Rc<dyn LinkViews>`. No `impl` of
  `ViewsSetup`, `ManageSetup` or `LinkReads` for the real areas exists in the interface files yet: the implementer of
  `setup.rs` writes `impl ManageSetup for PlaceSetupArea` (which carries `ViewsSetup`) and the implementer of this unit
  writes `DoorLinkReads`.

## Rules

- **D1 One context.** The context is built once from the options. The records area, the setup area, the views area and
  both halves hold that one context (`Rc::ptr_eq`), and so does the door.
- **D2 The door is set last.** After the assembly `ctx.door()` answers the returned door (`Rc::ptr_eq` with
  `AssembledDoor`'s own `Rc`); the context's own rule that it panics before then is places-a's (`places_readings`).
- **D3 Each method on its half.** `LINK_HALF` (33 names) and `MANAGE_HALF` (28) are disjoint and together name every
  method of `PlaceDoor`; the manage half is spread over the link half in the TS, so a name in both would be the
  manage half's. A call through the door answers what its half answers: `host_key` is the devices' key (link),
  `adds` is the records' pending adds, empty on a fresh door (manage).
- **D4 Watchers are the context's.** `on(f)` registers on the context and answers an id; `ctx.emit` reaches `f`; `off(id)`
  removes it (manage's `on` and `off`, over `ctx.watch`).
- **D5 Views reach the link half through the door.** `DoorLinkReads.backend_of(place)` answers what the door's
  `backend_of(place)` answers, read at the call.
- **D6 `make_place_door` is the assembly.** The trait object it answers is the assembled door.

## After the port

None of its own. The TS spreads the manage half over the link half; a name in both would be silently the manage
half's. None is today.

## Rules against tests

| Rule | Test (`tests/places_door.rs`) |
|---|---|
| D1 | `every_area_and_both_halves_hold_the_one_context` |
| D2 | `the_door_is_set_on_the_context_once_the_assembly_is_done` |
| D3 | `the_two_halves_name_every_door_method_once`, `a_link_method_is_answered_by_the_link_half`, `a_manage_method_is_answered_by_the_manage_half` |
| D4 | `a_watcher_the_door_holds_hears_what_the_context_emits_until_it_is_taken_off` |
| D5 | `views_read_the_link_half_through_the_door_at_the_call` |
| D6 | `make_place_door_answers_the_assembled_door` |
