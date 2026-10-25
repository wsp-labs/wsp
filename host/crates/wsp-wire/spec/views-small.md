# views-small

Source, at a3b870a6e: the functions of `views/golden-image.ts` (`onNpmBin` :97, `goldenHead` :103, `holdsNothing`
:110, `goldenImage` :116, `customRows` :245, `commandCheck` :250, `recipePins` :355, `agentOfRow` :445,
`toolRowPrefix` :448, `toolRowId` :451, `packageOf` :456), `views/place.ts` (`withPlaceStage` :114, `signInRowId` :256,
`signInOfRow` :258, `provisionCounts` :261, `macKindOf` :502, `placeForgetsOnly` :702), `views/session.ts`
(`threadKeyOf` :368, `threadRan` :374), `views/session-events.ts` (`agentStartingLine` :321, `hostFromEnv` :334,
`wspPackageIn` :355, `wspBinIn` :356, `turnTokenOf` :365, `copiedFromOf` :665, `isSessionEvent` :673),
`views/sub-row.ts` (:58, :93, :97), `views/workspace.ts` (`roadOf` :297, `scopeOf` :300, `agentsFrom` :450),
`shell-quote.ts` (25 lines), and the TS-only interfaces the host reads (`desktop-bridge.ts` BootPayload,
ContextMenuItem, HostsView; app-ports, conversations, placeholders, project-import, recipe-file, start, sub-thread,
thread-defaults, usage, workspace-look, workspace-state, app-address). Tests: `protocol.test.ts` (golden, session,
recipe cases in `tests/views_protocol.rs`), `shell-quote.test.ts` (5 of 6 in `tests/types_shell_quote.rs`),
`places.test.ts` :240 (`tests/views_session_events.rs`), and `tests/views_small_rules.rs` (28 tests, 140 TS answers).

## Behaviour

1. `on_npm_bin(None, p)` is `p`; else `"/opt/wsp-agents/bin:<bin>:<p>"`, an empty bin included (:97).
2. `golden_head`: the version whose number equals `head`; None without a manifest or a match (:103).
   `holds_nothing`: no head and no workspaces (:110). `golden_image`: a template id gives `Template`, no marks; else
   `FromSnapshot(snapshot_id)`, marks `["volatile"]` (:116). `custom_rows`: the recipe's `custom` or `[]`.
3. `recipe_pins` (:355): one pin per tick with a `pin`, `{ id: key(tick.id), ...pin, road? }`, sorted by id with
   `String.prototype.localeCompare` (`crate::text::collate`, root locale): case, accents and digits order as ICU does
   (`_x`, `10`, `9`, `Ab`, `e`, `é`, `zz` is not a byte sort).
4. `agent_of_row`: `agents/<id>` gives `<id>` (empty for `agents/`), never under `agents/mcp/`. `package_of`: the id's
   segments after the second `/` joined by `/` (`tools/npm` gives `""`). `tool_row_id(m, p)` is `tools/<m>/<p>`.
   `command_check(b)` is `command -v ` + `shell_quote(b)`.
5. `with_place_stage` (:114): the line is `{ step, state, note?, ms? }`; it replaces the job's line of that step in
   place, else goes last. A done `join` sets state `done` and the event's `place_id` where it has one; a `failed` line
   sets state `failed` and, only where the job has no `said`, `said` = the note; otherwise the job keeps its state.
6. `sign_in_of_row`: `signins/<x>` gives `<x>`, `github` gives `gh`, else None. `provision_counts`: counts by kind (no
   kind counts as `tool`), keys in the order first met. `mac_kind_of`: whitespace removed, lower-cased, then prefixes
   `macbook`, `imac`, `macmini`, `macstudio`, `macpro`. `place_forgets_only`: `away == true` and a fork or a project.
7. `thread_key_of`: thread id else id. `thread_ran`: a turn running, or one with a harness session id and no refusal.
8. `host_from_env`: url, token and key each trimmed with JavaScript's `trim` (U+FEFF and U+00A0 included,
   `crate::types::check::js_trim`); None without a url or a token after trimming; no key where it is empty. `turn_token_of`:
   untrimmed, None for missing or empty. `is_session_event(t)`: `t` is one of `SESSION_EVENT_TYPES`. `copied_from_of`:
   the event's `copied_from`, for every type but `session.slate`, which has no such key (23 of 24). `wsp_package_in(d)` is `<d>/wsp`, `wsp_bin_in(d)`
   `<d>/wsp/dist/bin.js` (no slash folding: `/a/` gives `/a//wsp`).
9. `sub_spawning`: begun, no `ready_at`, no `failed`. `sub_all_pushed`: no `read_failed` (an empty one is a failure),
   nothing unpushed, nothing changed. `sub_turn_gone(released, started)`: released, and a start unknown or strictly
   before it.
10. `agents_from(held, patch)`: the held setting, else `AGENTS_ON` (spawn, 3 machines, depth 2), each field the patch
    names over it. `road_of`/`scope_of`: a road alone has no scope.
11. `shell_quote(v)`: `'` + v with each `'` as `'\''` + `'`. `shell_line`: a word of one or more of
    `A-Za-z0-9_@%+=:,./-` is bare, any other (the empty word too) quoted; joined by spaces. `in_folder(None, c)` is
    `cd ~ && c`; `in_folder(Some(f), c)` is `cd '<f>' && c`, an empty folder quoted as `''`.

## Behaviour nobody wrote down

- `shell_line` and `shell_quote` work on bytes sh reads back: the ported tests run `/bin/sh -c` on the line.
- The TS-only interfaces keep their declared key order, the order the TS code builds them in where it builds them in
  one literal; `HostsView.current` is null, not absent, where the window is on no listed host.

## Decisions

1. Web-only functions are not ported (spec/shapes.md Decisions 2).

## Rules pinned → tests (`tests/views_small_rules.rs` unless named)

| Rule | Test |
|---|---|
| 1, 2 | `golden_image_rows::an_npm_bin_goes_between_the_agents_bin_and_the_path_and_none_leaves_the_path`, `..._the_head_is_the_version_it_names...`, `..._a_version_boots_from_its_template...`, `holds_nothing_only_with_no_head_and_no_workspaces`, `custom_rows_are_the_recipe_s_own_or_none`; `views_protocol.rs` 1421, 1470-1478 |
| 3 | `golden_image_rows::pins_are_one_per_tick_that_has_one_sorted_by_id_as_locale_compare_orders_them`; `views_protocol.rs` 121 |
| 4 | `golden_image_rows::an_agents_row_names_its_agent...`, `a_tools_row_s_package_is_everything_after_its_manager_slashes_kept`, `a_command_check_quotes_the_binary_for_sh`; `views_protocol.rs` 2030 |
| 5, 6 | `a_place_s_add_and_rows::*` (5 tests) |
| 7, 8 | `sessions::*` (6 tests); `views_session_events.rs`; `views_protocol.rs` 513 |
| 9 | `sub_thread_rows::*` (3 tests) |
| 10 | `a_workspace_s_agents::*` (2 tests) |
| 11 | `shell_quoting::*` (3 tests); `types_shell_quote.rs` (5) |
