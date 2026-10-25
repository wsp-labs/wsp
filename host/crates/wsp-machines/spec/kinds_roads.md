# kinds_roads

Source: `packages/runtime/src/machines/kinds.ts:26-316, 506-563` (about 350 of 564 lines) at `65f6b89c7`. Tests:
`packages/runtime/test/projects.test.ts:148-320` (clone here and on a machine, 11), `terminal-folder.test.ts` (2),
`agents-brokered.test.ts` (2), `places-agents.test.ts:153-220` (the agent homes on a joined computer, 2),
`sub-codex-models.test.ts` (the broker read, 5). Rust: `src/machines/kinds/roads.rs`; the area `KindsArea` is
declared in `src/machines/kinds.rs`.

## What it does

The reads and roads every kind of workspace shares: git on this computer (`git_here`, `git_top_of`, `branch_at`,
`remote_here`), a clone into a folder the person picked (`clone_here`), the clone and install inside a fork
(`clone_project`), the roots file write, the agent homes and environments of each kind, what the host's broker holds
and the launch a brokered workspace carries, and the one rule for the folder a turn or a command starts in
(`thread_folder`, the one home the runtime's notes name). `module_of` picks a kind's module (`kinds_table`).

## Behaviour

1. `BROKER_READ_MS = 5_000` (:27). `project_of(dest, size)` (:32): `{name: folder_name(dest), dest, importedAt: <now
   ISO>, size}`.
2. `clone_on_machine(project, computer)` (:36-43): `clone_lines({source: project_source(kind), remote, checkout:
   project.path, computer, branch?: project.base})` joined with `\n`; no `--branch` without a base.
3. `clone_project(entry, project, report)` (:46-76): a project with a `checkout` (the computer copied it in) clones
   nothing and installs nothing; with a branch of record it lands on it (`land_on_record(entry, project, None)`).
   Otherwise report `project-cloned` `Cloning <remote> into <path>.`; `computer = place_name(record.place ??
   places.wired)`. With a branch of record: `land_on_record` with the clone lines of `{..., branch?: record.base ??
   project.base, brokered: {ofRecord, origin: owner_repo_of(remote) ?? "", held: entry.ofRecordHeld}}`. Without: exec
   the clone with `CLONE_MS`; non-zero fails `clone_on_machine_failed_line({repo: owner_repo ?? remote, on: computer,
   exitCode, said: stderr || stdout, limitMs: CLONE_MS})`. Then `ls -A '<path>'` (`INLINE_EXEC_MS`), each line
   `js_trim`med; the first `project_installs(names, path)` row; none installs nothing; else report `project-cloned`
   `<command> in <path>.` and run `install_script(install, {dir: path, log: <scratch>/install-<project id>.log})` with
   `INSTALL_MS`; non-zero fails `<command> in <path>: <last line of stderr, else of stdout, else "exit <n>">; its whole
   output is <log> on the machine`.
4. `remote_here(path)` (:80-90): one command on the local machine, `git -C <path> remote get-url origin || echo;
   git -C <path> symbolic-ref --short refs/remotes/origin/HEAD || echo` (argv through `shell_line`), with
   `INLINE_EXEC_MS`; the first two lines `js_trim`med (missing lines read `""`); the default branch drops a leading
   `origin/`.
5. `git_here(cwd, args, timeout_ms = INLINE_EXEC_MS, stdin)` (:93-96): `shell_line(["git", "-C", cwd, ...args])` on the
   local machine, with stdin bytes when given. `git_top_of(path)`: `rev-parse --show-toplevel`, the trimmed output
   on exit 0 when not empty. `branch_at(path)`: `symbolic-ref --quiet --short HEAD`, the same rule.
6. `state_folder()` (:112-115): the folder of the resolved state path; no state path fails `this runtime was given no
   state file, so it keeps no worktrees or copies` with kind `invalid`.
7. `clone_here(word, source, into)` (:124-167):
   - `clone_url_refusal(word)` refuses with kind `invalid`.
   - `picked = folder_named(into)`; `name = project_name_of(source)`; the picked folder is the clone's own when its
     basename is `name`, or `name-<digits>` (`^[0-9]+$` on the rest); else the clone goes in `<picked>/<name>`.
   - Standing reads `missing` (ENOENT), `file` (ENOTDIR), `empty` or `full` (directory entries); any other error is
     thrown as is. A not-own picked folder that is a file refuses `clone_into_file_line(home_shortened(picked))`
     (kind `invalid`). A destination that is `full` or a `file` refuses with the next free `<name>-<n>` beside it,
     `n` from 2 up while that path exists: `clone_into_file_line(shown, free)` or `clone_into_taken_line(shown, free)`,
     both home-shortened (kind `invalid`).
   - The folders above the destination that do not exist are remembered, nearest first, up to the root.
   - Runs `shell_line(["env", "GIT_TERMINAL_PROMPT=0"]) + " " + project_source(kind).clone_command({remote:
     project_remote(source), dest})` on the local machine with `CLONE_MS`. A failure removes the remembered folders
     with `rmdir`, nearest first, stopping at the first that fails, and refuses `clone_failed_line(stderr || stdout)`
     (kind `invalid`). Answers `folder_named(dest)`.
8. `register_import(o, report)` (:171-178): report `landing` `REGISTERING_LINE`; the plan; answers `{result: {dest,
   files, bytes, parts: 0, cut: [], rewritten: [], agents: [], project: project_of(dest, bytes)}, done:
   registered_line(dest)}`.
9. `write_roots(entry, dests, roots_path)` (:181-184): `write_daemon_roots_script(dests, roots_path)` on the machine
   with `INLINE_EXEC_MS`; non-zero fails `could not make <last dest> browsable on the machine: <stderr last 200>`.
10. `cloud_home(id)`: the catalog's guest home for the agent, else fails `the catalog has no home for <id>`.
    `cloud_env(record, id)`: `login_env_on(place, npmBin)` merged with the agent's `guest_env`, the agent's keys last.
11. `cloud_road(entry)` (:197-201): `{url, expiresAt}` of `entry.ws.daemon_reach()` and `daemonToken` when
    `daemon_token_of(machine, DAEMON_TOKEN_PATH)` gives one. `local_road()`: the local wiring's daemon road, else
    fails `this host wired no daemon for this computer, so nothing on it can be dialled`.
12. `cloud_deploy(entry)` (:209-213): the recipe's `deploy_daemon(machine, <token held>)`, then the held token is
    forgotten and minted again (`daemon_tokens.remove`, `daemon_token_of`).
13. `shared_login_under(home, agent)`: the catalog's shared login file for the agent exists under `home`.
    `place_door_of()` fails `no_kind_line("place")` without a door. `place_of(record)` (:227-232): the door's folder
    computer for `record.place`; none while the door says the place is joined fails `place_home_refusal(None)`; none
    otherwise fails `<name> stands on <place ?? "no computer">, which this host no longer holds`.
14. `place_agent_home(place, home, id)` (:235-240): the agent's shared login and the place's `logins` folder give
    `login_home_in(logins, shared)`; else `under_login_home(home, cloud_home(id))`.
15. `vault_on(entry)`: the vault (`opts.vault()` or empty), filtered by `vault_at(place, vault)` on a record with a
    place where the door answers.
16. `brokered(record)` (:248-249): the broker answers ready and `is_brokered(record, at_provider(place) ? {kind:
    "provider"} : None)`.
17. `read_broker()` (:256-270): nothing when the broker has no `status`. Races the read against `BROKER_READ_MS` on the
    clock: status (a failure reads none), then the Codex claims only when `status.codex` is `chatgpt` or
    `access-token` (a failure reads none). Afterwards (late or not) the held status and claims are both replaced by
    what was read, both none when late. State kept: `broker_said`, `claims_said`, memory only.
18. `brokered_launch(entry, harness, token)` (:278-287): none when not brokered. `codex = codex_broker_of(said,
    claims, opts.broker.codexModels)` for a codex harness. No token answers `{env: {}, vault: {}, codex?}`. With one:
    `unbrokered_refusal(harness)`, else for codex with no codex broker `codex_login_unread_refusal()`, is thrown; else
    `{env: brokered_env(token, record.branchOfRecord, public_marketplaces(ctx)), vault, claude, codex?}` from
    `placeholders_of(said, token)`.
19. `know_broker(entry, harness)` (:291-295): only for codex on a brokered record: reads the broker when no codex
    broker is known, then refuses `codex_login_unread_refusal()` when there is still none.
20. `place_env(entry, home_of)` (:304-316): the place login's variables without `USER` and `PATH`, then `HOME`, then
    for every catalog agent in catalog order its store variable (`stateHomeEnv`, else its shared login's `homeEnv`)
    at `home_of(agent)` and `secret_env_of(vault, agent, place_login_stands(entry, agent))`, then `GITHUB_TOKEN` only
    where the vault holds it and the door says GitHub comes from the vault on that place. Later keys override.
21. `kind_of(computer)`: `kind_for_computer(computer, door.joined(computer))`. `module_of(kind)`: the table's module,
    else fails `no_kind_line(kind)`.
22. `backend_for(record)` (:514): `RELEASED_BACKEND` for phase `released` or a `sub.cloudGone`; else the module's
    backend. `backend_of_kind(kind, place)` builds a record of just those two facts. `keeps_images`,
    `pause_keeps_disk` (`pauseMode == "disk"`), `names_workspace` read the backend. `lifecycle_of(entry)` fails `<kind>
    machines declare no lifecycle` without one.
23. `exec_factory_for(entry, o, waiting)` (:539-540): the module's exec stream with `opts.machineExec` under `o` (the
    caller's keys win) when wired.
24. `project_memory_key(agent, project)` (:546-547): `project.memoryKey` when the agent's catalog row names a
    `projectKeyEnv`; else `project_state_key(agent, project.path)`.
25. `thread_folder(entry, {cwd})` (:556-558): `cwd`, else the module's `folder(record)`, else the project's path.

## Edge cases

- `lab-2` and `kit-7` picked are the clone's own folders; `lab-4-shelf` is a parent (projects.test.ts:169).
- A full `lab` offers `lab-2`, then `lab-3` once `lab-2` stands (:196).
- A failed clone into `gone/code` leaves no `gone` (:237).
- `remote_here` on a folder with no origin answers two empty strings, not a branch (rule 4).
- `read_broker` past 5 s leaves both facts none, even if one was held before.

## Behaviour nobody wrote down

- Rule 3.15: the broker race's timer is the clock's.
- Rule 4.26: `place_env` key order is the TS spread order (login, HOME, agents, GitHub); later keys win in place.
- The trap "No code compares the kind string": everything asks `module_of`.

## Decisions

1. The filesystem reads of `clone_here` (`read_dir`, `exists`, `rmdir`, `realpath`) go through `std::fs` directly, as
   the TS uses `node:fs` directly (01: the filesystem is not a seam outside collect).

## After the port

- `clone_project`'s install failure names the command and the last line but no exit code when a line exists.

## Tests (tests/kinds_roads.rs)

| Rule | Test (TS case) |
|---|---|
| 7 | `clones_an_https_url_into_the_folder_named_by_argv_with_no_prompt` (projects :148) |
| 7 | `clones_owner_repo_through_gh_into_an_empty_folder_of_its_name` (:159) |
| 7 | `clones_into_a_folder_of_the_repos_name_inside_a_picked_folder` (:169) |
| 7 | `refuses_a_full_folder_of_the_repos_name_offering_the_next_free` (:196) |
| 7 | `refuses_a_file_a_url_no_clone_should_run_cloning_nothing` (:218) |
| 7 | `says_gits_last_line_when_the_clone_fails_and_leaves_no_folder_it_made` (:237) |
| 2, 3 | `clones_the_repo_into_a_fork_before_ready_with_its_base` (:285) |
| 2 | `with_no_base_the_clone_takes_the_remotes_default` (:301) |
| 3 | `a_clone_that_fails_ends_with_gits_last_line` (:308) |
| 3 | `installs_the_lockfiles_row_once_and_names_the_log_on_failure` (new) |
| 25 | `thread_folder_takes_cwd_then_the_kinds_folder_then_the_project` (terminal-folder :32, :45) |
| 18 | `a_brokered_launch_carries_the_token_env_and_placeholders` (agents-brokered :28) |
| 18, 19 | `a_codex_launch_refuses_where_the_broker_named_no_login` (sub-codex-models :38-89, the read rule) |
| 17 | `read_broker_past_five_seconds_leaves_both_facts_none` (new) |
| 14, 20 | `a_joined_computers_agent_homes_and_env_follow_its_login` (places-agents :153, :190) |
| 4 | `remote_here_reads_two_lines_and_drops_origin` (new, real git) |
| 6 | `state_folder_refuses_without_a_state_file` (new) |
| 22 | `backend_for_a_released_or_cloud_gone_record_is_the_released_backend` (new) |
| 24 | `memory_key_follows_the_catalog_row` (new) |
