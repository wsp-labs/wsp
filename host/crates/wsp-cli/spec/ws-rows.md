# ws_rows: the rows a delete, a forget, a thread listing and a create read

Source: `workspaces-help.ts:602-881` at 65f6b89c7, less `branchHere` (871-881, another unit). Rust: `src/verbs/workspaces_help/rows.rs`.
Recorded: `tests/fixtures/cli_c/ws_rows.json` (441 cases, probe `record/ws_rows.probe.ts`), tests `tests/cli_c/ws_rows.rs`,
drivers `drivers_ws_rows.rs`, hand tests `ws_rows_rules.rs`.

## Scope

Dropping: `madeWorktree`, `Dropping`, `onDelete` (private), `dropping`, `deleting`, `forgetQuestion`, `forget`, `forgotLine`,
`deleteQuestion`, `deleteWorkspace`, `deletedLine`. Rows: `FOLDER_WIDTH`, `TITLE_WIDTH`, `shortenedFront`, `shortenedEnd`,
`THREAD_HEAD`, `threadLines`, `diskGoneLines`, `expiredWord` and `subThreadWord` (private), `threadTree`. Places and projects:
`placeNames`, `projectsOf`, `projectOf`, `projectLine`, `NO_PROJECT_YET`. Create: `createFor`, `projectsHere`, `agentsAsked`,
`countAsked`, `sizeChosen`, `sshAsked`, `createdExisting` (not exported by the TS; no recorded case, reached through the verbs'
tapes).

## Behaviour

- `dropping` (615-622): `workspaceOf` (one `workspaces.resolve {ref}`; a workspace that does not read is `otherVersion`). A `local`
  workspace with any worktree record is refused with the worktree words, then any kind that runs in a folder (`local`, `place`)
  with the folder words, both usage refusals and both before `sessions.list`. Else one `sessions.list {workspaceId}` folded into
  threads; its length is the count. A record with no `kind` is a cloud one.
- `deleting` (626-638): `dropping`, then only where the record names a `place`, one `places.list`. Refused list: computer named
  `that computer`. Place not listed or of kind `provider`: no `on`. Else `on = {name, computer: tableName(place)}` (first row with the id).
- `forgetQuestion`, `forgotLine`, `deleteQuestion`, `deletedLine`: pure; the machine half comes from `onDeleteOf` (a made, not gone
  worktree; a create that made no machine; a copy on a joined computer; else the kind's words). `forget` sends `workspaces.forget`,
  `deleteWorkspace` sends `workspaces.delete`, each `{workspaceId}` alone, and answer nothing.
- `shortenedFront`/`shortenedEnd` (675-682): length in UTF-16 units; over the width the front is cut behind `…`, or the end before it.
- `threadLines` (688-693): one row, then one per subagent. State cell precedence: expired word, sub-thread word, thread state word.
  Expired: nothing waits, state not `failed`, `diskKeptUntil` at or before now. Sub-thread word: none if no `sub`, if it asks or is
  waited on; `waiting`, `starting`; `released` only while `completed` or `interrupted`; else `Not pushed` when not running and work is held.
  The folder is home-shortened (`~`) then front-cut at 40; titles end-cut at 60. `home` and `now` are the TS's `homedir()` and
  `Date.now()`, handed in so a test can pin them.
- `threadTree` (716-734): depth first in listing order; a parent not in the listing (or `""`) leaves the row at the top; rows left by a
  loop of parents (and a row that is its own parent) stand flat at depth 0 in listing order; a row whose id was drawn is not drawn again.
- `createFor` (775-812): see the doc on the function: copy-takes-no-size refusal before any frame; `workspaces.landing {project, sized?}`;
  size check; frames held, `events.subscribe`; `workspace.creating` frames of this name emitted and streamed (held first); `workspaces.create`
  keys `project, name, cpu, memMb, agents, parent`; line `created <name> <id>, a copy of <project> at <folder or project path>`.
- `projectsHere` (817-823): `projects.list`; any refusal or a reply with no `projects` falls back to the workspaces' projects, once per
  id, first position and last name kept; an empty list is an answer.
- `agentsAsked`/`countAsked` (827-838): `--max-machines` (least 0) before `--max-depth` (least 1); the word is JS `Number(word)`.
- `sizeChosen` (843-847), `sshAsked` (851-859): see the doc comments; the port is read before the key.

## Limits and caps

Folder 40 and title 60 UTF-16 units: under, at and over, for the folder (39, 40, 41, 42; home shortened first at 39/40/41 after the
tilde), title (59 to 62), subagent title, project path; widths 1, 2, 5, 40, 60 on the two shorteners. Port 1 and 65535, 0 and 65536.
Counts 0 and 1 against their least. Sizes: an offered one, half a gigabyte, one past the offer.

## Null and empty inputs

No rows, one row, no worktree, empty ref, empty places, empty sessions, no projects key, empty size word, empty name, empty notice,
empty port and key, empty HOME-relative folder, empty subagent list, empty `diskGone`.

## Order

Frames: dropping `workspaces.resolve`, `sessions.list`; deleting then `places.list`; createFor `workspaces.landing`, `events.subscribe`,
`workspaces.create`; projectsHere `projects.list`, `workspaces.list`. Lines: subagents in listing order under their thread; tree depth first.

## Invariants held

- Invariant 7 (cloud off, no line names a cloud): the lines of the kinds that exist with the cloud off (local, place) never say cloud.
  Test `invariant_7_the_lines_of_local_and_place_workspaces_name_no_cloud`. The cloud-kind sentences are unreachable without a cloud.
- Guest rule (a caller on a machine never has this computer's disk read): not reached; no function here takes `elsewhere` or reads a
  path (`sshAsked` runs `absolutePath` on a string, no disk). `branchHere`, which reads files, is another unit's.

## What the TS leaves unclear (after-port tickets, not fixed here)

- `deleting` throws a `TypeError` (`reading 'find'`) on a `places.list` answer with no `places` key (workspaces-help.ts:632); the Rust
  cannot match that text, so no case is recorded. `projectsOf` returns `undefined` on a reply with no `projects` (744).
- `threadTree` drops the second of two rows with one id although its comment says every row prints (732).
- `shortenedFront`/`shortenedEnd` can cut a surrogate pair in two (676, 681), writing a lone surrogate; cases keep emoji off the cut.
- `countAsked`/`sshAsked` read JS `Number(word)`: `0x10`, `0b11`, `1e2`, `+3`, `""` as 0, blank as 0.
- `createFor` casts the landing's `capabilities` instead of parsing; the recorded reply carries `sizes` only, so Rust reads `sizes` alone.
- `createFor`'s `events.subscribe` runs outside its `try`, so a refused subscription never stops the held frames (harmless).

## Rule to test

| Rule | Tests (recorded `ws_rows.rs` unless named) |
|---|---|
| refusals come before any `sessions.list` | `dropping__a_local_*`, `dropping__a_box_folder_is_refused`, `deleting__a_local_*` |
| `places.list` only for a record with a place | `deleting__no_place_on_the_record_reads_no_places`, `deleting__a_workspace_on_a_joined_computer_*` |
| unnamed computer, provider, unknown place | `deleting__places_list_is_refused_*`, `deleting__the_place_is_a_provider_*`, `deleting__the_place_is_not_in_the_list` |
| delete and forget words per kind | `delete_question__*`, `deleted_line__*`, `forget_question__*`, `forgot_line__*`; `ts_verbs_lifecycle_*` |
| caps under, at, over | `shortened_*`, `thread_lines__a_folder_of_*`, `thread_lines__a_title_of_*`, `project_line__a_path_of_*`; `shortened_cells_never_exceed_their_width` |
| UTF-16 units | `shortened_front__an_emoji_*`, `shortened_end__accents_*`, `thread_lines__a_title_with_an_emoji_*` |
| state word precedence | `thread_lines__*` (expired, sub-thread, asking); `ts_verbs_threads_the_state_cell_*` |
| tree order, missing parent, loops, duplicates | `thread_tree__*`; `thread_tree_prints_each_row_once_with_parents_first`; `ts_verbs_folders_settings_*` |
| subagent rows repeat the thread's cells | `thread_lines_put_one_row_per_subagent_under_the_thread` |
| `createFor` frame order and keys | `create_for__*`; sent-or-not: `a_refused_size_leaves_no_subscription_and_no_create` |
| stages held, live, after the reply | `create_for__stages_*`, `create_for__a_stage_heard_after_the_reply_is_dropped` |
| size on a copy refused first | `create_for__a_size_on_a_copy_*`, `create_for__an_empty_size_on_a_copy_*` |
| `projectsHere` fallback | `projects_here__*` |
| number words | `count_asked__*`, `agents_asked__*`, `ssh_asked__*` |
| `placeNames` names, last wins | `place_names__*`; `ts_places_place_names_*` |
| both cloud settings | `delete_question__a_cloud_with_the_cloud_on`, `deleted_line__a_cloud_with_the_cloud_on` (cloud-kind words do not read `CLOUD_ON`) |
| invariant 7 | `invariant_7_the_lines_of_local_and_place_workspaces_name_no_cloud` |
