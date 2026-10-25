# places_setup_folders: the folders, their servers, and a sync's removals

Source: `setup.ts:633-744` (folders, their servers) and `:767-806` (`stores_here`, the sync's `undo`). Module
`places::setup::folders` (methods of `SetupRun`). Shared rules: `places-a-README.md`. TS cases:
`test/places-setup.test.ts` :470-497 (the servers step's inputs), :1030, :1067-1126, :1983, :2069-2259, :2333, at
the run's seam (`places_setup_run.md`).

## Recording roads used

`folder_remote(folder) -> remote?`, `add_folder(place, key, folder, move?, said) -> HeldRow`, `projects_on(place)
-> [{ id, path }]`, `folder_look(id, old pick, new pick)`, `remove_folder(place, project id)`, `stores_on(place,
home)`. The provisioner's `project_servers(machine, picks, key, path, stage, on)` and `undo(before, removed, on)`.

## Folders

- `needs_github(folder)` (:635): the folder's remote, read by `folder_remote` (a failure is none), as a GitHub address;
  none is false. Else `GIT_TERMINAL_PROMPT=0 git -c credential.helper= ls-remote --quiet '{address}' HEAD >/dev/null
  2>&1` on the machine, timeout 30,000: any exit but 0, or a failed exec, is true.
- `add_folder(key, folder, move?)` (:645): no road: `{ id: "folders/{key}", label, outcome: failed, note:
  NO_FOLDER_ROAD }`. Else the road runs under `folders_of = place` (so a fork gate it reaches lets it through) with
  each line said to the `folders` stage as `"{label}: {line}"`; a failure is a failed row with its first line.
  `label` is `folder.name` else the key.
- **The step** (`folders`, :698), each picked folder in order; a sync passes over one it did not move:
  1. A project an earlier run made from this pick (`made` row `folders/{key}` with a project and a pick) and still
     standing there (`projects_on`) is the claim. With the same source, name and kept files, it is `installed` with
     that project and the new pick: `earlier: true` where icon, hue and image match too, else after `folder_look`
     (a failure: `failed` with its first line and the old pick).
  2. Else, where GitHub settled true or the folder needs no GitHub, `add_folder` with the claim.
  3. GitHub settled false: `failed`, `NEEDS_GITHUB_LINE`.
  4. Else `skipped`, `WAITS_ON_GITHUB_LINE`, and one more folder waits: once GitHub settles it is added (or fails
     with `NEEDS_GITHUB_LINE`) and landed; then, once every step ended, its servers are carried and each landed; then
     the count drops and the row lands again, which is the landing that may say the end.
  A row that did not land keeps the claim's project, pick and `createdAt`, so the next run moves it.
- A landed folder (`installed` with a project) is remembered for its servers. A resume whose folders step ended
  remembers every `folders/` row of it.

## Their servers (:664-693)

- `agent_files_there` starts true only where `done` holds `mcp` and the started setup's mcp line reads done; after
  the mcp step it is true where the step was passed over or none of its rows is a failed `files/` row or
  `mcp/stopped`.
- `carry_servers(key, row)`: marks it carried; no project or no `project_servers` road: nothing. Files not there:
  `skipped` `"{label} servers"` with `FOLDER_SERVERS_WAIT_LINE`. The project's path off `projects_on`; none: nothing.
  Else `project_servers(machine, picks, key, path, stage "folderServers", { home, held: the vault's server value
  names, stores?, recipe: picks.name where the record follows one })`; a failure is one failed row; through `ours`.
- The step carries every landed folder not carried yet, in landing order.

## `stores_here()` (:769)

`recording.stores_on(place, home)` and the folder computer's login (`ctx.door().folder_computer(place).machine
.login_of()`); either missing is none; else the engine's `stores_reached(login, stores)`.

## A sync's removals (`undo`, :777)

Nothing without a sync, removed changes, or an undo road. Else `provisioner.undo(before, removed, { home, stores? })`,
and for each planned removal in order:
- `owned` = the outcome of the row under `u.owner`; `ours` = owned is `installed`.
- `present`: a `skipped` row with `was_there_line(name)` (a tool the box had before wsp is never taken off).
- Files (`u.dests`): `unland_files(machine, home, dests, stores)`; none answered is `failed` with `UNLAND_FAILED_LINE`;
  kept files are `skipped` with `edited_there_line(name, kept)`; the stage says how many went and stayed.
- A folder with its owner's project: `remove_folder(place, project)`, a failure said to the stage only.
- No command, a note and ours: `skipped` with the note. A command and ours: run with timeout 300,000; a non-zero
  exit is `failed` with `"not taken off: {last line of stderr or stdout, else exit n}"`.
- The rows under `u.ids` and `u.key` go, then the rows left above are appended.

## Pins

| Rule | Test | TS case |
|---|---|---|
| servers step's inputs: held names, stores, a login not root | `the_servers_step_is_handed_held_names_and_store_folders` | :470, :481, :489 |
| folders wait on GitHub and clone once through | `a_private_folder_waits_for_github_and_clones_once_through` | :1084 |
| GitHub skipped or failed: needs GitHub, nothing cloned | `a_private_folder_reads_needs_github_where_github_is_skipped`, `..._failed` | :1067, :1115, :1983 |
| a public folder clones with no wait | `a_folder_github_lets_anyone_read_clones_at_once` | |
| the end is said once, by the last folder | `the_last_waiting_folder_says_the_end_once` | :1084 |
| a waiting folder's servers go after every step | `a_late_folders_servers_go_after_every_step` | |
| an unchanged claim is earlier and asks nothing | `a_folder_made_before_from_the_same_pick_stands_as_wsps` | :2124 |
| a look change puts the look on | `a_folder_whose_look_moved_gets_the_look` | |
| a failed add keeps the claim | `a_folder_that_did_not_land_keeps_its_claim` | |
| files not there: servers wait | `servers_wait_where_the_agents_files_did_not_land` | |
| a sync lands a folder with no GitHub wait | `a_sync_lands_a_folder_with_no_wait_on_a_github_step_it_does_not_run` | :2333 |
| removals: before-wsp kept, wsp's own taken off | `a_sync_never_takes_off_what_the_box_had_before_wsp` | :2106, :2217 |
| an edited file stays and its row says so | `a_file_edited_there_stays_and_its_row_says_so` | :2088 |
| a skill removed by the ledger | `a_removed_skill_comes_off_by_the_ledger` | :2088 |
| an undo command that fails names why | `an_undo_that_fails_says_what_was_not_taken_off` | |
| Codex's files under the logins folder | `a_dropped_codexs_files_come_off_under_the_logins_folder` | :2234 |
| a project made by a removed folder is removed | `a_removed_folders_project_goes_with_it` | |
