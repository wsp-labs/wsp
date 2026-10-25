# workspace_state
Source: the functions and tables of `packages/protocol/src/workspace-state.ts` (665 lines) at the cut, 383708c78 (681 lines at a3b870a6e); its flat constants (`COMPUTER_LEFT`, `UNNAMED_COMPUTER`, `START_DAEMON_WORD`, `WAITED_ROW_MS`, `REPORTED_WORD`, `BACK_OVER_SSH`, `SEND_BLOCK_WORDS`, `CLOUD_SIGN_IN_ROAD`, `FORGET_NEEDS_GONE`) are `wsp_wire::types::workspace_state`. Tests: `packages/protocol/test/workspace-state.test.ts` (42 of 45 ported) and `absent-computer.test.ts` (23 of 25 ported) in `tests/workspace_state.rs`, plus `pinned` there. Skipped: workspace-state.test.ts:238 (the kind table has three keys: a Rust `match` on `WorkspaceKind` is total, so the compiler holds it), :333 and :344 (`screenCommandTyped`, `screenCommandsOf`: `views/harness.ts`, wsp-wire's), absent-computer.test.ts:183 (`placeAddSheetWord`: `views/place.ts`), :190 (compares two constants of `words::places`, no function of this unit).

## What it does
The one state word a workspace reads as, off its phase, machine state and reach; the words each kind of workspace takes (machine word, reading roads, what a delete does); what a computer that is not answering says and the road back to it; and the refusals of a send or an action in each state.

## Behaviour
1. `workspace_kind(kind)`: the record's kind, cloud where it carries none (workspace-state.ts:15). `is_local_workspace`, `turn_spend_word` (LIST_PRICE_WORD on a local workspace, else None, and None without a view) (:21-28).
2. `kind_words(kind)` is the WORKSPACE_KIND_WORDS row (:110-114), whose `projectSources` are git, github and gitlab for cloud and those three and folder for local and place (the row has no `agents` field); `serves_reading`, `reading_road`, `machine_word`, `thread_place` (the computer's name where the kind runs in the folder, else the machine word) read it (:116-144).
3. `on_delete_of(kind, copy, machine_id, on)`, in order: machine id "" (a create that failed before any machine) is NO_MACHINE_MADE; no copy and a joined computer named is "copy on <computer> is deleted"; no copy is the kind's row; a copy is "worktree at <path> is removed and the project folder is left as it is" (:207-214). `delete_notice` and `delete_copies_notice` wrap it; copies that all say the same are one sentence, else one per copy and the records sentence capitalised (:148-172).
4. `unpushed_line`: None for a checkout holding nothing; parts in order commits, uncommitted files, stashes, each an "it could not count/read" word where the counts are unknown; joined "a, b and c" (:175-188).
5. `workspace_state(input)`: released wins; a gone machine is gone; then by phase: pausing, napping is paused, waking, gone, queued is waiting; running reads the machine (paused, starting is waking) then the reach (unreachable, no-daemon, zombie are unreachable) (:224-252). `workspace_state_of` passes the status's machine state and reach, phase alone without a status (:261-266).
6. `workspace_word`: the memory words under pause mode memory where the state has one, else the word (:283-285). `where_word`: named, else provider, else the kind's `where`, else the machine id; THIS_COMPUTER becomes `computer_word(os)` (:294-297).
7. `absent_computer(name, away_ms)`: away "no answer", line "no answer <offlineFor(ms)>, is it on?" (no time with None), the said, will and sentence (:345-351). `own_daemon_down`, `composer_held_line`, `waited_line`, `away_ms_of` (None for no or unparsable lastSeenAt), `last_known` (:361-401).
8. The road back (:442-510): `ssh_road_of` (None for an empty address), `back_url(port)`, `dials_back_word`, `linked_over` ("over ssh" when the dialled address is the back URL, "at <address>" when it was handed out, else None), `absent_road`, `place_dial_line`, `place_dial_road` (link when present, ssh with an ssh road, else None).
9. `action_refusal(state, action, gone_words, name)`: None running; per state as :512-543; waiting refuses everything but a send. `send_refusal`: a block reads SEND_BLOCK_WORDS, a state reads `action_refusal(state, "send", ..)` (:558-560).
10. `sign_in_road(kind)`: "sign in from a terminal on this computer" for local, "... on the computer the thread runs on" for a kind that runs in the folder, else CLOUD_SIGN_IN_ROAD; `sign_in_refusal_line` adds ", then send again" (:574-584). `signed_out_line`, `screen_command_line` (:588-603).
11. `daemon_silent` (unreachable, no-daemon, zombie), `reach_shown` (a slow probe holds the last probe's word where that was slow or answering), `is_billing`, `start_failed` (gone with machine id ""), `needs_rebuild` (gone, zombie, or a wake refused) (:605-636). `gone_road_refusal`, `gone_refusal` (`words` "" reads as none) (:641-665).

## Edge cases
- `on_delete_of(kind, None, Some(""), Some(on))` is NO_MACHINE_MADE: the empty id wins over the joined computer (pinned).
- `last_known(value, Some(0.0), None)` and `waited_line(4999)`: see pinned values.
- `away_ms_of(Some("not a date"), now)` is None.

## Behaviour nobody wrote down
- Rule 8.17: `away_ms_of` reads `lastSeenAt` with `Date.parse`'s rules, which `src/date_parse.rs` carries.
- `null` and `undefined` read alike in WorkspaceStateInput (`?? null`), so one Option holds both.

## Tapes
The state word on every row and in `wsp ls`; the Delete confirmation; the composer's refusals; the absent-computer strip.

## Decisions
1. A function taking `Pick<WorkspaceView, "kind">` takes the view's own `kind: Option<&WorkspaceKind>`; `turn_spend_word` takes `Option<Option<&WorkspaceKind>>` for the TS null view.
2. `MachineOnDelete.done` is a String read through `done_for(machine_id)`, since a Rust row cannot hold the TS closure; `kind_words` answers an owned row and WORKSPACE_KIND_WORDS is no constant.
3. Structural TS parameters become structs: `CopyAt`, `StandsOn`, `PlaceRoad`, `Dialled`, `WhereRecord` (Deserialize under the TS keys), and `away_ms_of` takes `lastSeenAt` alone; `GoneRoad` and `SendRefusalKind` replace the string unions.

## After the port
- `screen_command_line` and the two screen-command tests panic inside wsp-wire's `nullable` deserializer today, and four tests inside its JsNumber; they reach this unit's stubs once wsp-wire is filled in.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `workspace_state_test::what_a_workspace_s_kind_changes_about_its_words::*`, `pinned::each_kind_says_its_own_machine_and_what_a_delete_takes` |
| 3 | `workspace_state_test::what_a_workspace_s_kind_changes_about_its_words::*` (the delete sentences), `pinned::each_kind_says_its_own_machine_and_what_a_delete_takes` |
| 4 | `workspace_state_test::what_a_copy_holds_that_its_remote_lacks::*` |
| 5 | `workspace_state_test::workspace_state::*`, `pinned::the_state_reads_off_phase_machine_and_reach` |
| 6 | `workspace_state_test::where_a_workspace_runs_in_the_person_s_words::*`, `the_line_a_verb_that_moved_a_workspace_prints::*` |
| 7, 8 | `absent_computer_test::*`, `pinned::an_absent_computer_reads_its_road_and_its_last_word` |
| 9 | `workspace_state_test::workspace_state::*` (the refusals), `pinned::each_state_reads_one_word_and_refuses_in_it` |
| 10 | `pinned::the_sign_in_road_and_its_refusal_follow_the_kind`, `workspace_state_test::a_slash_command_that_works_only_in_the_cli_s_own_terminal::*` |
| 11 | `workspace_state_test::is_billing::*`, `reach_shown::*`, `pinned::the_state_reads_off_phase_machine_and_reach`, `pinned::each_state_reads_one_word_and_refuses_in_it` |
