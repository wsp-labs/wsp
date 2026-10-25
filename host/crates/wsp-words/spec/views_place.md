# views_place
Source: `computerKindWord` (:140-143) and `provisionCountWord` (:222-225) of `packages/protocol/src/views/place.ts` (762 lines) at a3b870a6e. The rest of the file is wsp-wire's: its zod shapes and tables (`PlaceKind`, `PlaceView`, `PlaceProvisionKind`, `PROVISION_KIND_WORDS`, `SETUP_STEP_CLASS`, `SETUP_STEP_WORDS`, ...) and `withPlaceStage` :114, `signInRowId` :256, `signInOfRow` :258, `provisionCounts` :261, `macKindOf` :502 and `placeForgetsOnly` :702 (wsp-wire's spec/views-small.md); `placeAddSheetWord` :53 is web-only and ported by neither crate (wsp-wire's spec/shapes.md, Decision 2). Tests: `packages/protocol/test/places.test.ts:520` (`provisionCountWord({})`) and the count `setupWord` reads at :509-518, both inside the case at :505, ported in `tests/place_setup.rs`. No TS test reaches `computerKindWord`. No pinned module.

## What it does
The word the KIND column and the app give one computer's row: this computer by its platform's name, a cloud, or a box. And a count of what a recipe put on a computer by kind, as one short phrase.

## Behaviour
1. `computer_kind_word(place, platform)` (:140-143): a place whose id is HERE_PLACE_ID ("here") reads `this_computer(platform)` ("this Mac" on darwin, else "this computer") whatever its kind; any other place reads "cloud" for kind provider and "box" for every other kind.
2. `provision_count_word(counts)` (:222-225): walks the kinds in PlaceProvisionKind's order (tool, file, server), never the map's order; a kind whose count is above 0 says `plural(count, word)` with the words tool, file and MCP server; the parts join with ", "; with no count above 0 it reads "0 tools".

## Edge cases
- An empty map is "0 tools" (pinned).
- One row of each kind is "1 tool, 1 file, 1 MCP server" (pinned through `setup_word`).
- A count of 0, below 0 or NaN leaves its kind out, since the test is `> 0` (:223).
- A map holding server before tool still reads tools first.

## Behaviour nobody wrote down
- Rule 8.11: each count is `String(n)` through `plural`, so `js::number_string`; `plural` is singular only for exactly 1.

## Tapes
The KIND column of `wsp computers` (host/src/verbs/client.ts:527) and the `computers` tool; the TOOLS column's "<count> ready" through `setup_word` (spec/views_place_setup_words.md).

## Decisions
1. `Pick<PlaceView, "id" | "kind">` is the whole `&PlaceView`; the `"darwin" | "linux"` union is `&crate::words::computer::Platform`.
2. `Partial<Record<PlaceProvisionKind, number>>` is `&IndexMap<PlaceProvisionKind, f64>`; a kind missing from the map reads as 0, and the map's own order is ignored.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | none: no TS test reaches `computerKindWord` |
| 2 | `place_setup.rs: places::the_setup_on_a_computer_you_own::says_on_the_tools_cell_what_is_under_way_where_it_stopped_or_what_stands` |
