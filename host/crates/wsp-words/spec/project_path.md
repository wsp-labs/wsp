# project_path
Source: the functions of `packages/protocol/src/project-path.ts` (336 lines) at a3b870a6e; its flat constants (`SSH_ALIAS_PREFIX`, `PLACE_LEFT_LINE`, `PLACE_HELD_LINE`) are `wsp_wire::types::project_path`. Tests: no TS test file of its own (its callers' tests reach it through the host and the engine); `tests/project_path.rs` pins 85 values printed by the TS under node.

## What it does
Paths as wsp reads and writes them: whether a path is inside a project and its repo-relative form, the worktree a folder is in, the key Claude Code files a project's sessions under, folder names, slugs and ssh aliases, the folders a browser hides or guards on a Mac, and every path wsp keeps on a computer it joined.

## Behaviour
1. `under_project(path, root)`: equal, or under `root/` (project-path.ts:16-18). The other names of a folder are its own path and each suffix starting at a "/" that the cwd is under (the `/private/tmp` and `/tmp` twins) (:22-26).
2. `repo_path_of(path, cwd, root)`: the path resolved against cwd (".." pops, "" and "." drop), then cut at the first name of root it sits under; None outside (:32-40).
3. `worktree_of(cwd, worktrees)`: the worktree whose longest name the cwd is under; the first of equal depth wins; None for none (:45-52).
4. `claude_project_key`: every UTF-16 unit outside `[A-Za-z0-9]` as "-" (:60). `claude_memory_dir(home, key)`: home less trailing "/", then `/projects/<key>/memory` (:65).
5. `folder_name`: the last part after trailing "/" go (:69). `folder_slug`: lowercase, runs outside `[a-z0-9]` as "-", ends trimmed, cut at 40, trailing "-" trimmed again, "" reads "work" (:74-77). `ssh_alias`: SSH_ALIAS_PREFIX + slug (:82). `parent_folder_name`: the part before the last, "" with fewer than two (:118-121).
6. `hidden_folder(path, machine)`: a dot-named folder anywhere; on a Mac with a home, `<home>/Library` (:96-101). `guarded_folder`: only on a Mac with a home, the eight MAC_GUARDED folders under it exactly (:110-115).
7. `probe_path(home)`: TOOLS_PATH less empty entries and those under home (:131-134). `roots_path_in`, `work_folder_in` (:139-149).
8. `place_daemon_paths(home)`: the 27 paths of :156-236, keys in that order; `place_login_files`, `place_install_log`, `held_place_script` (each path through `shell_quote`), `place_provision_paths`, `place_owned_paths` (28 paths in the order a leave takes them), `stand_in_records_path`, `stand_in_machine_path` (:242-336).

## Edge cases
- `claude_project_key("é/😀")` is "----": four UTF-16 units, four dashes (rule 8.7). A per-`char` replacement writes three.
- `folder_slug` of 39 letters then "-bcd" cuts to 40 and drops the trailing dash; "---" reads "work".
- `hidden_folder("/h/Library", {home: "/h/", mac: true})` is true: the home's trailing "/" goes first.
- `repo_path_of("..", "/r", "/r")` is None: the path leaves the project.

## Behaviour nobody wrote down
- Rule 8.7: `[^A-Za-z0-9]` and `[^a-z0-9]+` are ported by hand per UTF-16 unit; `toLowerCase` lowercases before the class, so "Ünïcode Name" slugs to "n-code-name".
- Every function trims trailing slashes with `/\/+$/`, so "/" reads as "".

## Tapes
Claude Code session lookups, the folder browser, `wsp add` and `wsp leave` on a box.

## Decisions
1. `FolderMachine` holds `home: Option<String>` (TS null and undefined alike) and `mac: Option<bool>`; the TS default `{}` is `None`.
2. The two path objects are `PlaceDaemonPaths` and `PlaceProvisionPaths`, Serialize with the TS keys.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2, 3 | `pinned::a_path_inside_a_project_reads_through_its_other_names` |
| 4 | `pinned::claude_keys_its_projects_per_utf16_unit` |
| 5 | `pinned::folder_names_slugs_and_aliases` |
| 6 | `pinned::a_mac_hides_and_guards_only_its_home_folders` |
| 7, 8 | `pinned::the_paths_wsp_keeps_on_a_joined_computer` |
