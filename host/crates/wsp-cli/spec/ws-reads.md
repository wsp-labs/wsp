# ws_reads: listings, picking, rows, waking, the image, ssh

Source: `packages/host/src/verbs/workspaces-help.ts` at 65f6b89c7, lines 111-309 and the two helpers `threadRows`
(158-184) and `branchHere` (869-881). Rust: `src/verbs/workspaces_help/reads.rs`. Recorded in
`tests/fixtures/cli_c/ws_reads.json` (279 cases, probe `record/ws_reads.probe.ts`), drivers
`tests/cli_c/drivers_ws_reads.rs`, hand tests `tests/cli_c/ws_reads_rules.rs`.

## Scope

`workspaces` 111, `workspaceStatuses` 118, `threads` 122, `workspaceOf` 130, `pickThread` 138 (private), `threadOf` 148,
`threadsOf` 153, `ThreadRow` 158, `threadRows` 164, `stateLine` 189, `wokeLine` 196, `nap` 202, `resize` 209, `rebuild`
218, `rebuiltLine` 229, `imageView` 234, `buildImageAt` 242, `imageLines` 268, `HOST_SIDE_VAULT` 279,
`SSH_PIPES_HERE_LINE` 282, `sshWorkspaceOf` 286, `pipeBytes` 296, `branchHere` 871 (private).
`ThreadRow` is `ThreadRowOut` of `io.rs` (agreed with the `ws_rows` writer). `threadRows` calls `projects_here` of `rows.rs`.

## Behaviour

- `workspaces`, `workspaceStatuses`: one frame, `workspaces.list` or `status.list`, the array as the host wrote it. The TS does
  not parse either; only `workspaceOf` does.
- `threads(workspaceId?)`: `sessions.list`, with `{workspaceId}` when one is given (an empty string is given), else no
  params. Folded by `foldThreads` (turns of one thread key into one row, rows in the order of each thread's first turn).
- `workspaceOf`: `workspaces.resolve {ref}`; the `workspace` is parsed (schema key order, unknown keys dropped). A record
  that does not read is the plain error `otherVersion("workspaces.resolve")`. The ref is sent as typed, empty too.
- `pickThread` (private, reached through `threadOf`, `threadsOf`, and hand-tested in Rust): an exact id wins whole, then a
  prefix that names one; several is the plain error `<n> threads start with <ref>; give more of the id` (no kind, so the
  provider's exit class); none is the not-found refusal `no thread <ref>`. The empty ref is a prefix of every id. A row
  is matched by its thread key (the thread id, else the row id).
- `threadOf`, `threadsOf`: one `sessions.list` and no more, whatever the number of refs; refs are picked in the order
  named; the first that picks nothing ends the call. No refs is still one read.
- `threadRows(within?)`: `workspaces.list`, then `sessions.list {}`. With `within` naming no project any session row
  carries, `projects_here` is asked (`projects.list`, and when that is refused `workspaces.list` again) and a name found
  nowhere is `refusal("no project <w>", "Run wsp projects to read the names.", not-found)`. Rows are the folded
  threads, only those whose session row's project id or name is `within` when given. Each row is the thread, then
  `projectName` and `computerName` (from the session row, empty if absent), `folder`, `branch`.
  `folder` is `threadCwd` of the row and workspace, else the workspace's folder, else its project's path, else empty.
  `branch` is `branchHere(folder)` where the workspace is `local`, else the worktree's branch unless that worktree is
  gone, else empty (`branchHere` has no say for a non-local workspace: it is never called).
- `branchHere(folder)`: nearest folder up the tree holding a `.git`; a `.git` file's `gitdir:` line (trimmed, relative to
  the root) points at the git dir; the first line of its `HEAD` that reads `ref: refs/heads/<name>`, trimmed. Anything
  unreadable, a detached head, a ref outside `refs/heads`: nothing, so the worktree record answers.
- `stateLine`: `workspaceStateLine(name, workspaceState({phase}), pauseMode)`: the phase alone. `rebuiltLine`: that line
  plus ` on <machineId>`, with no pause mode (it reads the record it was handed, not a listing).
- `wokeLine`: `status.list`, the entry of the workspace's id read as the listing's own phase, machine state and reach,
  else the phase given; so a running record whose listing says paused reads `stopped`. An unreachable reach reads
  `<name> is up and not answering yet`.
- `nap`: `workspaceOf` then `workspaces.nap {workspaceId}`; a record that does not read asks no nap. `resize`: `threadOf`
  (a thread ref, not a workspace one) then `workspaces.resize {workspaceId (the thread's), size}`.
- `rebuild`: `workspaceOf`, `status.list`, and the refusal (a plain error, no kind) unless `needsRebuild`: the state is
  gone, or the reach is a zombie, or `wakeRefused` is set (an empty string is set). Else `workspaces.rebuild`.
- `imageView`: `image.get {}`, `view` parsed. `imageLines`: `no image yet` or the record line and one line per copy.
- `buildImageAt`: `places.list`, `events.subscribe`, `image.build {place: <word as typed>, force?}`, `image.get`, in this
  order. The stage lines stream while `image.build` runs, one per `golden.stage` frame of that place's id.
  The image is checked before the build record is parsed, so no image is `no image yet` even for an unreadable build.
- `sshWorkspaceOf`: a ref starting `wsp-` reads `workspaces.list`; one workspace by `sshAlias(name)` is answered as the
  list gave it, two or more are a usage refusal naming each `name (id)`, none falls to `workspaceOf` (a second read).
- `pipeBytes`: input to the socket until it ends, the socket to the output until it closes, the output never ended.

## Limits and caps

The title cut in a folded row is 48 characters (`openingTitle`), pinned at 47, 48 and 49 through `threads`. A folder
slug is cut at 40 characters with trailing dashes dropped (`sshWorkspaceOf`, 40 and 39 plus a dash). Sizes of a copy:
`1023 B`, `1 KB`, `1024 MB`, `1 GB` through `imageLines`.

## Null and empty inputs

Empty list, one, and three for `workspaces`; no sessions; an empty workspace id; an empty ref (all and one thread);
no refs; `within` empty; an absent `within`; no listing for `wokeLine`; no copies, no vault, no hash, no size; an
empty alias list; the empty name and an empty machine id in the lines; HEAD with no ref line.

## Order

Frames: `workspaces.list` before `sessions.list` in `threadRows`; `status.list` before `rebuild`; `places.list`,
`events.subscribe`, `image.build`, `image.get` in `buildImageAt`; the list before the resolve in `sshWorkspaceOf`.
Lines: stage lines in the order the frames came. Rows: fold order. Copies: the host's order.

## Invariants held

Host invariant 7 (cloud off names no cloud): this unit says no word that moves with the setting; every case that could
differ is also recorded with the cloud on, and the twins are equal. Named test `invariant_7_...`. The folder rule: the
host's guest callers never have this computer's folders read for a workspace that is not on this computer:
`threadRows` reads a HEAD file only for `local` workspaces. Named test `folder_rule_...`. The TS has no `elsewhere`
check inside these helpers; the callers hold that rule.

## What the TS leaves unclear

- `workspaces`, `workspaceStatuses`, `nap`, `resize`, `rebuild` and `sshWorkspaceOf` (an alias hit) answer the host's
  objects without parsing them; `workspaceOf` and `imageView` parse. Unknown keys and key order differ between the
  two kinds. The Rust types keep schema order, so replies in the tests are in schema order.
- `ThreadView` as the wire struct writes it does not have the key order `foldThreads` builds (`asking` ... `failure`
  come before `turns` and `ran` there). One case records it (`threads / the key order of a folded row with asking and
  a cost`, named test `fold_keeps_the_ts_key_order_...`); the Rust fold must serialise in the TS order.
- `wsp-words` holds `NO_SEALED_IMAGE` as `no image yet; run wsp init to build one`; the TS at the oracle says `no image yet`.
  The tests follow the oracle.
- `imageView` and `buildImageAt` throw zod's own message for a view or build that does not parse; `workspaceOf` says
  `otherVersion`. Not recorded (zod's text is not reproducible).

## Rule -> test table

| Rule | Test |
|---|---|
| each helper's frames, params and order | the recorded case of each helper (`threads / a workspace named`, `buildImageAt / built, with the stages streamed`, ...) |
| a refusal sends no later frame | `nap / no such workspace asks no nap`, `rebuild / no such workspace asks no status`, `resize / no such thread asks no resize` |
| null and empty inputs | `threads / an empty workspace id is still sent`, `threadOf / the empty word ...`, `threadsOf / none named`, `threadRows / within the empty word` |
| caps at, under, over | `threads / the title is ...` (47, 48, 49), `sshWorkspaceOf / a long name ...`, `imageLines / a copy of ...` |
| pick rules, both refusal kinds | `pick_thread_exact_prefix_and_the_two_refusals`, `pick_thread_matches_the_thread_key_not_the_session_row` |
| one read for many refs | `threadsOf / one listing for many refs` |
| branch read off HEAD | `threadRows / a thread on a local folder ...` and the 12 cases after it |
| branch not read off this disk for a non-local workspace | `folder_rule_a_workspace_off_this_computer_has_no_folder_read` |
| the listing's word, not the phase | `woke_line_is_read_off_the_listing_not_the_phase_it_wrote`, `ts_pause_line_...` |
| rebuild refusals are plain errors | `rebuild_refusals_carry_no_kind` |
| the stream stops when the build returns | `build_image_at_streams_nothing_after_it_returns`, `ts_boat_add_names_boat_never_box_on_every_stage_line` |
| ssh alias | `ssh_alias_shared_by_two_workspaces_is_a_usage_refusal_with_its_fix` |
| both cloud settings | `invariant_7_the_cloud_setting_moves_no_word_of_ws_reads` (and every `(cloud)` twin) |
| pipe | `pipe_bytes_both_ways_until_the_socket_closes_and_never_ends_the_output`, `pipe_bytes_fails_where_nothing_listens` |
| the recording is whole | `the_recording_holds_every_helper_of_the_unit` |
