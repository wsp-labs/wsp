# boot_entries
Source: `packages/runtime/src/machines/boot.ts:623-672,1014-1065` (about 110 of 1,071 lines) at `65f6b89c7`. Tests:
`runtime-create.test.ts:210`, `spawn-guard.test.ts` (523, 565, 1471), `tree.test.ts:457`, `box-lead.test.ts` (276, 446),
and the scenes `entry of`, `child of`, `entry of row`, `tree records`, `computer rows` (2), `image head` in
`oracle/boot_rows.oracle.ts` (`tests/fixtures/oracle/boot_rows.json`). Rust: `src/machines/boot/entries.rs`.

## What it does
The doors every verb comes through: the live entry a verb names, with the origin rule read once; a lead's child; which
rows a thread caller reaches; the thread records of a caller's tree; the computers a project can live on; the image a
workspace forks by default; the place a project's workspace lands.

## Behaviour
1. `entry_of(id, origin, {now, act, thread, being_made})` (:628): awaits `ready()`. No entry, or one `creating` without
   `being_made`, refuses `not_found_refusal(...)` (kind `not-found`): `no such workspace` for a thread caller,
   `no such workspace: <id>` otherwise. The named thread is passed on only when its record is on this workspace and the
   caller drives it. `refuse_relayed(record, origin, act, thread)`. With `now`, the entry at once; else await the
   workspace's `boot_work`, then answer the live entry under that id if one stands (a load may have replaced it), else
   the one read before.
2. `child_of(lead, ref, origin, {lead, child})` (:643): `workspaces.resolve(ref, origin)`, then `entry_of` of it (with
   the child thread when named). A child whose `lead.workspaceId ?? parentWorkspaceId` is not the lead's id refuses
   `not_the_leads_child_refusal(label_of(kid, child), label_of(lead, lead))` with kind `invalid` (`label_of` is the
   record name, or `thread <first 8 of the id>`).
3. `reaches_row(row, caller, talk)` (:656): `drives_thread(row.threadId, caller)` first; then a row on no live record
   reaches; else `talk` with `talks_to_its_tree(caller)`, or `acts_on(record, caller)`.
4. `entry_of_row(row, origin, talk, {being_made})` (:665): a caller that is no thread reads through `entry_of`. A thread
   caller: after `ready()`, no entry, a creating one, or one it does not reach answers `None`; else await `boot_work`
   and answer the live entry as rule 1 does.
5. `tree_records(caller, talk, ws)` (:1017): thread records in map order whose root (`rootThreadId ?? id`) is the
   caller's root (any root for a caller that is no thread), on `ws` when named, on a live workspace, that the caller
   reaches.
6. `tree_stands_on(ws, caller, talk)` (:1024): false for a caller that is no thread; else a row on that workspace the
   caller reaches, or a tree record there.
7. `talks_to_tree_on(ws, caller)` (:1029): `talks_to_its_tree(caller) && tree_stands_on(ws, caller, "send")`.
8. `computer_rows()` (:1038): with no place door, `[{id: "here", name: hostname(), kind: "computer"}]`; with one, its
   rows as `{id, name, kind}`. The wired provider is appended as `{id: wired, name: wired, kind: "provider"}` when no row
   has its id.
9. `name_of_computer(computer, rows)` (:1047): the row's name, else the id.
10. `image_head_or_none()` (:1051): `golden_head(copy_of(image_place("default"), "default"))?.snapshotId`;
    `image_head()` (:1054) throws `NO_IMAGE_YET` ("no image yet") for none.
11. `landing_place(computer)` (:1062): `workspace_lands(computer, wired)`; `place` answers
    `place_door_of().place_for(place)`; here and wired answer `{}`.

## Edge cases
- `entry_of` of a creating entry with `being_made` answers it (scene).
- `computer_rows` names this computer by its host name, read at each call.

## Behaviour nobody wrote down
- Runtime invariant 3 (AGENTS.md): the origin is the token's; `entry_of` reads the rule once through `refuse_relayed`
  and never trusts a client's word for its scope.
- Rule 3.13: `entry_of` with `now` skips the boot work wait; nothing else does.

## Rules against tests
| Rule | Tests (`tests/boot_entries.rs`; scenes replayed from `boot_rows.json`) |
|---|---|
| 1 | scene `entry_of_refusals_by_caller_a_create_still_landing_and_the_boot_work_waited`; `until_ready_a_creating_workspace_is_neither_listed_nor_reachable` |
| 2 | scene `child_of_a_leads_own_child_by_lead_or_by_parent_else_refused_naming_both` |
| 3, 4 | scene `entry_of_row_a_thread_caller_reaches_only_its_tree_and_answers_nothing_otherwise`; `an_id_this_host_does_not_hold_reads_as_one_outside_the_callers_tree` |
| 5-7 | scene `tree_records_a_threads_tree_by_root_on_live_workspaces_it_reaches` |
| 8-11 | scenes `computer_rows_*` (2), `image_head_none_sealed_refuses_with_no_image_yet`; `landing_place_names_a_place_only_for_a_joined_computer` |
