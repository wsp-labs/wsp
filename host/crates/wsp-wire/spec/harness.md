# harness

Source: the functions of `packages/protocol/src/views/harness.ts` (280 lines; `noMcpServersLine` :172,
`mcpServersBlocked` :179, `keepsRename` :187, `readsImages` :194, `takesMcpServers` :201, `noFastLine` :248,
`checkedAgainst` :250, `startPicks` :266) and all of `packages/protocol/src/harness-picks.ts` (112), at a3b870a6e.
Tests: `thread-defaults.test.ts` (7 cases on `startPicks`/`markedDefault`, ported in `tests/views_harness.rs`),
`protocol.test.ts` :1051, :1083, :1092 (in `tests/views_protocol.rs`), and `tests/views_harness_rules.rs` (12 tests,
68 TS answers on one catalog with legacy, hidden, per-model efforts, a model default effort, windows and a fast model).
Rust: `src/views/harness.rs`, `src/types/harness_picks.rs`.

## What it does

Reads one harness catalog: which option a list marks default, the efforts and context windows a model takes, every
model a start may name, the picks a thread's rows carry and which of them a resume keeps, and the check a start's picks
meet, filling a new thread's from the marks.

## Behaviour

1. `marked_default`: the first option with `is_default == Some(true)` (harness-picks.ts:9).
2. `efforts_for` (:22): the model's `efforts` filter the catalog's, catalog order kept, a value the catalog lacks
   dropped; no list on the model (or no model) is all of them. With the model's `default_effort`, every option loses
   `is_default` and the one whose value it names gets `Some(true)` (none does if it is not in the list).
3. `every_model` (:30): models, then `legacy_models`, then `hidden_models`. `model_of` (:35): None for no value; the
   first of `every_model` with that value; else `{ value, label: value }` and nothing else.
4. `context_windows_for` (:41): `[]` without a model; else the model's `context_windows` filter as in 2.
5. `listed_pick` (:53): the value where an option carries it, else None.
6. `model_picks` (:65): a model ending in `[1m]` loses it; the window is the one given, else `1m` where the suffix was.
   Keys written `model`, then `contextWindow`.
7. `recorded_picks` (:80): for each of (model and its window from one row), effort, permission mode in that order, the
   latest row naming it. Keys in that order: `model`, `contextWindow`, `effort`, `permissionMode`.
8. `kept_picks` (:98): without a catalog, each named pick else the ran one. With one: the thread's model (only if
   listed) stands when it takes the named effort (or the catalog has no efforts) and the named window; a named model
   wins; effort and window are the named ones else the ran ones where the chosen model's lists carry them. Keys
   `model`, `effort`, `contextWindow`.
9. `start_picks` (:266): model = the named one, else (opening a thread with a catalog) the marked default of
   `every_model`. With a catalog, `checkedAgainst` (:250): a non-empty models list refuses a named model none of models,
   legacy or hidden carries (`crate::wire::helpers::listed`, kind `usage`, its message word for word); a non-empty
   efforts list refuses a named effort `efforts_for(model_of(model))` lacks, subject = the model's label where the
   model has its own efforts, else the harness; same for `permission_modes` ("access mode"); `fast: true` on a model
   (`model ?? runs_on`) whose `fast` is not true refuses with kind `invalid` and `no_fast_line(label)`. Effort and
   permission mode fill like the model, from `efforts_for(model_of(model))` and `permission_modes`. Output keys:
   model, effort, permissionMode, then `fast: true` only where `fast == Some(true)`; nothing else.
10. `mcp_servers_blocked` (:179): None for no servers or an empty map, None where the harness takes them, else
    `no_mcp_servers_line(harness)`. `keeps_rename`/`reads_images`: true without a catalog or for `source: table`, else
    the catalog's flag. `takes_mcp_servers`: only `mcp_servers == Some(true)`.

## Edge cases

- `model_of(cat, Some("nope"))` is a model labelled "nope" with no lists: `efforts_for` of it is all the catalog's.
- A start without a catalog passes every value and fills nothing (24 oracle cases, half without one).
- `fast: false` is dropped from the output. `fast: true` is checked against `model_of(model ?? runs_on)`, where
  `model` is the named or filled one: an unknown slug has no `fast` and refuses; with no model at all (not opening a
  thread, none named, no `runs_on`) `model_of` is None and it passes.

## Behaviour nobody wrote down

- `efforts_for` rebuilds the marked option with `isDefault` last (`{...rest, isDefault: true}`) and the others with no
  `isDefault`: the TS writes `{value, label, description?, short?, isDefault}`, so it returns `Fields<HarnessOption>`,
  which keeps that order and derefs to the option. With no default effort named it returns the catalog's options as
  they stand.

## Decisions

1. `PickRefusal { message, kind }` stands for the TS's thrown error; the words crate turns it into a `HostError`.

## Rules pinned → tests (`tests/views_harness_rules.rs` unless named)

| Rule | Test |
|---|---|
| 1 | `the_marked_default_is_the_first_option_marked_and_none_where_none_is`; `views_harness.rs` 164, 170, 192 |
| 2 | `a_model_s_efforts_narrow_the_catalog_s_in_its_order_and_its_own_default_takes_every_other_mark` |
| 3 | `a_model_is_the_catalog_s_by_value_a_slug_it_lacks_is_named_by_itself_and_none_is_none`, `every_model_is_the_current_ones_then_the_legacy_then_the_hidden_in_that_order` |
| 4 | `a_model_s_context_windows_narrow_the_catalog_s_and_no_model_has_none` |
| 5-8 | `a_kept_pick_stands_only_where_the_list_carries_it`, `a_model_s_1m_suffix_is_its_window_unless_a_window_is_named_and_comes_off_the_model`, `what_a_thread_ran_with_is_each_pick_s_latest_turn_that_named_one_model_and_window_from_one_row`, `a_resume_keeps_the_thread_s_model_only_where_it_takes_what_the_start_names_and_drops_what_the_list_lost` |
| 9 | `a_start_fills_what_opens_a_thread_from_the_marks_and_refuses_a_value_no_list_carries_or_a_fast_no_model_has`; `views_harness.rs` 123, 157, 170, 176, 198 |
| 10 | `servers_named_for_an_agent_that_takes_none_are_refused_in_its_name_and_none_named_pass`, `no_catalog_or_the_runtime_s_table_answers_yes_to_renames_and_images_and_no_to_servers`; `views_protocol.rs` 1051, 1083 |
