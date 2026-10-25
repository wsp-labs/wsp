# own_file

Source: packages/own-file/src/index.ts (41 lines) at a3b870a6e. Tests: none in the package; the TS callers' tests
reach it (host-lock, places, state-store). Rust tests: tests/own_file.rs (12 cases).

## What it does

The one writer for files under the owner's state folder: a host token, a pairing code, a link key, a login file.
Every file lands through a 0600 temporary file renamed over the target, under folders standing at 0700, and a folder
an older build left wider is chmodded back. Synchronous; no caller awaits it.

## Behaviour

1. `own_folder(dir)` makes `dir` with every missing parent (`mkdirSync(recursive, mode 0o700)`, index.ts:19), then
   chmods `dir` alone to 0700 when its mode bits differ (index.ts:20). A parent it made gets 0700 masked by the umask;
   a parent that stood is never touched.
2. `write_own(root, rel, bytes)` joins `rel` to `root` the way node's `path.join` does (index.ts:28): a leading `/` in
   `rel` stays under root, `.` and `..` are resolved textually. Rust's `Path::join` replaces on an absolute `rel`, so
   the port normalises the joined string instead.
3. It runs `own_folder(root)`, then `own_folder` on each folder from root down to the target's folder, one part of
   `relative(root, dirname(target))` at a time, skipping empty parts (index.ts:29-35).
4. The bytes go to `<target>.<pid>-<ms>-<6 base36 chars>.tmp` written with mode 0600, chmodded 0600 again since the
   umask narrows the open mode, then renamed over the target (index.ts:36-40). Nothing else is left in the folder.
5. A link at the target is replaced by the regular file the rename brings; what it pointed at is untouched.
6. Errors are the OS's (`io::Error`); nothing is caught.

## Edge cases

- `rel` = `"/abs/x.json"` writes `root/abs/x.json`, `abs` at 0700 (test `rel_is_joined_as_node_joins_...`).
- `rel` = `"a/../y.json"` writes `root/y.json` and makes no `a` (same test).
- `rel` = `""` names root itself: the rename onto a folder fails with EISDIR (`an_empty_rel_names_root_itself_and_fails`).
- `rel` = `"../side/z"` writes outside root and chmods each folder it climbs through, root's parent included
  (`a_rel_that_climbs_out_of_root_...`), because `relative()` yields `..` and the loop owns `join(root, "..")`.
- A target at 0644 ends at 0600 (`a_file_that_stood_at_0644_...`).

## Behaviour nobody wrote down

- Path text: the TS works on strings; the port takes `&Path` for root and `&str` for rel, and must not let
  `Path::join` drop root (rule 13.8, "how a path is joined where the TS resolves it").
- What survives a restart: only the files themselves; the writer holds no state.

## Tapes

None directly: the host token and link files it writes surface in the host-lock and place tapes of #2443.

## Decisions

1. Temp file name: kept byte-for-byte in shape; the random part may come from any source.
2. The 0600 chmod after the write stays even where the open mode already gives 0600.

## After the port

- A `rel` that climbs out of root writes outside it and chmods root's parent to 0700, and an empty `rel` chmods root's
  parent and leaves a stray `.tmp` beside root (probed on node 22, 2026-10-25). Both look wrong; the port keeps them.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | own_folder_makes_a_missing_folder_and_its_parents, own_folder_repairs_a_folder_an_older_build_left_wider, own_folder_repairs_only_the_folder_named_not_a_parent_that_stood |
| 2 | rel_is_joined_as_node_joins_a_leading_slash_staying_under_root_and_dot_dot_resolved, a_rel_that_climbs_out_of_root_writes_there_and_repairs_the_folders_it_passes, an_empty_rel_names_root_itself_and_fails |
| 3 | write_own_writes_the_bytes_at_0600_under_folders_at_0700, folders_between_root_and_the_file_that_stood_wider_are_repaired_and_nothing_above_root_is |
| 4 | the_write_leaves_no_temporary_file_beside_the_target, a_file_that_stood_at_0644_is_replaced_and_its_old_mode_goes_with_it, write_own_takes_bytes_as_well_as_text |
| 5 | a_link_at_the_target_is_replaced_by_a_file_and_what_it_pointed_at_stands |
