# wire_machine_link
Source: the five sentences of `packages/protocol/src/wire/machine-link.ts` (702 lines) that wsp-wire left to this crate, at a3b870a6e: `placeForgetLine` :347, `placeHoldsLine` :356 (with the private `heldPart` :337 and the private `PlaceHeld` :335), `placeNotAWorkspaceLine` :371, `placeNotAWorkspaceFix` :372 and `localRunsOneLine` :401. The rest of the file is wsp-wire's `wire::machine_link`. Tests: no TS test reaches them; `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` in `tests/view_wire_words.rs` holds every value the TS printed for the same input.

## What it does
What forgetting a computer says about the forks and projects it held, and the refusals of a joined computer named where a workspace was asked for, or of this computer asked for a second workspace.

## Behaviour
1. `held part(rows, noun)` (:337-342): None for no rows; else the noun for one row or "<n> <noun>s" for more, then the names joined by ", "; then " with <plural(threads, "thread")>" where the rows' threads sum to more than 0.
2. `place_forget_line(name, held, ssh)` (:347-352): "its <forks part>" and "its <projects part>" joined by " and ", after "<name> leaves this wsp with ", or "<name> leaves this wsp" when both are None; without ssh, ", and nothing is done on <name>"; with ssh, "wsp tries to take itself off <name> over <ssh> first. Then " before it and ", and nothing more is done on <name>" after; always ending ": whatever of wsp's stays there comes off with wsp leave run on that computer.".
3. `place_holds_line(held)` (:356-361): None when both parts are None; else "its <forks part> deleted" and "its <projects part> out of this wsp" joined by ", and ".
4. `place_not_a_workspace_line`, `place_not_a_workspace_fix`, `local_runs_one_line` (:371, :372, :401): fixed sentences around the name.

## Edge cases
- Two forks with 2 and 1 threads and one project with 1 thread: "spoo leaves this wsp with its 2 forks a, b with 3 threads and its project web with 1 thread, and nothing is done on spoo: ..." (pinned).
- A fork with 0 threads names the fork alone, with no "with 0 threads".
- `place_holds_line` of an empty hold is None.

## Behaviour nobody wrote down
- Rule 8.11: the counts are `String(n)` through `plural`.

## Tapes
`wsp forget` and `wsp remove` on a joined computer; the runtime's refusal of a joined computer named as a workspace (`runtime/src/projects/workspaces.ts`).

## Decisions
1. The TS private `PlaceHeld` is `PlaceHeld { forks, projects }` of `HeldRow { name, threads }`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2, 3, 4 | `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` |
