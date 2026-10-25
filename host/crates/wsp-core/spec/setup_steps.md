# setup_steps
Source at 65f6b89c7: packages/runtime/src/setup-graph.ts (72) and recipe-sync.ts (88). Tests: setup-graph.test.ts
(7), recipe-sync.test.ts (5). Rust: `src/setup_graph.rs`, `src/recipe_sync.rs`; tests `tests/setup_graph.rs` (13),
`tests/recipe_sync.rs` (12). Fixture: `tests/fixtures/core_rest/recipe_sync.json`. Callers: wsp-machines'
`places/setup` (spec places_setup_run.md rule 5, places_setup_area.md), which hold `RecipeChange` already.

## The graph (setup-graph.ts)
- **G1** (:23-33) A step is ready when every name in `after` has ended or is no step's name and no step's mark, none of
  its lanes is held by a running step, and it is light or fewer than `width` non-light steps run. `width` 0 starts no
  heavy step.
- **G2** (:34-61) One pump walks the waiting steps in the order given and starts every ready one at once (each start
  can make a later one not ready: a lane, the width), removing it from waiting. When no step runs after the walk, the
  run answers: the first failure if any, else the names still waiting, in the order given. No steps answers `[]` at
  once.
- **G3** (:43-45) Starting a step calls its `run` immediately, in the pump, with its release; `on_move` hears the
  running names (start order) right after each start.
- **G4** (:45-48) `release(mark)` does nothing for a mark the step does not hold or one already ended; else it ends the
  mark and pumps on a later turn (`queueMicrotask`), not inside the call.
- **G5** (:62-69) A step's end: it leaves the running set, its name and every one of its marks end, a `Stop` stops the
  run, `on_move` hears the running names, then the pump runs. A stopped run starts nothing more and lets the running
  end.
- **G6** (:50-55) A failure records only the first error and stops the run like `Stop`; the answer waits for every
  running step to end, then is that error.

## Recipe sync (recipe-sync.ts)
- **R1** (:36-50) Kinds in RECIPE_KINDS order (agents, mcp, clis, skills, plugins, folders, configs); for each, every
  row of `after` in its own order: absent before → `added`; its row serialized differs from before's, or the kind is
  `mcp` and `copyKeys === true` differs between before and after (both present) → `changed`; else the items differ →
  `edited`. Then every row only `before` has → `removed`. `before = None` reads every row as added and never as moved
  keys.
- **R2** (:24-31, :46) A row's items are every item key equal to `<kind>/<name>` or starting `<kind>/<name>/`, sorted
  by key, compared as `[key, value]` lists. `before_items = None` (a computer set up before the sync) reads a row as
  edited where `after_items` holds any item of it. `mcp/linear2` is not an item of `mcp/linear`.
- **R3** (:19-21) A row whose value is absent is not a row. Rows compare by their serialized JSON, so key order counts
  as in the TS's `JSON.stringify` (wsp-wire's struct order equals zod's parse order for these shapes).
- **R4** (:56-87) Steps, in the order first added: a removal adds `mcp` for mcp and `context` for agents and clis, and
  nothing for the rest. agents: edited → `mcp`; changed → `signins`; added → agents, signins, mcp, skills, context.
  clis → clis, context, plus `floor` where added and the after row names `needs`. configs → `github` for the row named
  github, else `configs`. mcp, skills, plugins, folders → their own step. Any `folders` adds `folderServers` last.

## Decisions
1. Step and mark names share one type `N`, as the TS's `after: (S | M)[]`; wsp-machines picks `N`.
2. A step's failure is a `HostError`; the run answers `Result<Vec<N>, HostError>`.
3. Steps answer `IndexSet<PlaceSetupStep>` so the TS's `Set` order holds.

## Tests
| Rule | Test |
|---|---|
| G1, G2 | `start_together_up_to_the_width_each_once_what_it_waits_on_ended_never_two_on_one_lane`, `one_pump_starts_every_ready_step_in_the_order_given_and_skips_one_on_a_busy_lane`, `width_zero_starts_only_light_steps_and_answers_the_rest_as_never_started`, `no_steps_answers_nothing_never_started_at_once`, `runs_one_at_a_time_at_width_one_and_a_light_step_takes_no_room` |
| G3, G5 | `on_move_hears_every_start_and_every_end_with_the_running_steps_in_start_order`, `starts_nothing_more_once_a_step_says_stop_lets_the_running_end_and_answers_what_never_started` |
| G4 | `starts_a_step_waiting_on_a_mark_once_the_step_holding_it_lets_it_go_while_that_step_still_runs`, `a_release_after_its_step_ended_starts_nothing_twice` |
| G5 | `lets_a_steps_marks_go_at_its_end_where_it_never_let_them_go_itself_and_a_mark_no_step_holds_counts_as_ended`, `answers_a_step_waiting_on_a_mark_as_never_started_where_the_step_holding_it_never_ran` |
| G6 | `throws_the_first_error_once_every_running_step_ended`, `the_first_failure_is_the_answer_whatever_ends_after_it` |
| R1 | `what_moved_is_nothing_where_nothing_moved`, `names_each_row_added_changed_in_the_recipe_edited_on_this_computer_and_taken_out`, `the_one_answer_about_the_servers_keys_moving_is_every_server_that_stays_changed_carried_by_the_servers_step`, `rows_moved_in_the_recipe_match_the_ts_changes_and_steps`, `rows_taken_out_match_the_ts_changes_and_steps`, `rows_added_or_changed_match_the_ts_changes_and_steps` |
| R2 | `reads_every_row_with_an_item_as_edited_once_on_a_computer_that_kept_no_items`, `first_apply_and_a_computer_that_kept_no_items_match_the_ts_changes_and_steps`, `rows_moved_only_in_the_items_match_the_ts_changes_and_steps` |
| R4 | `carries_each_move_by_the_steps_that_put_it_on_and_a_removal_by_none_but_the_servers`, `the_steps_of_a_hand_built_list_of_changes_match_the_ts_rows` |
| all | `every_fixture_row_is_in_one_group` |
