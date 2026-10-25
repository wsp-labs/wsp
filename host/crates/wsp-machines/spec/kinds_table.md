# kinds_table

Source: `packages/runtime/src/machines/kinds.ts:317-505` (about 190 of 564 lines) at `65f6b89c7`. Tests: the kind
answers as the runtime tests read them: `local-workspace.test.ts`, `places-fork.test.ts`, `box-folder.test.ts`,
`terminal-folder.test.ts`, `runtime-reach.test.ts`; none tests the table alone. Rust: `src/machines/kinds/table.rs`.

## What it does

The three kind modules, one per `WorkspaceKind`: `cloud` (a fork at a provider or on a computer somebody joined),
`local` (a folder on this computer; absent when the host wired no local road) and `place` (a project folder on a
joined computer; absent without a place door). Each answers the same questions (`KindModule`, a wsp-core trait): its
backend, how a turn's process streams, the folder a thread starts in, agent homes and environment, the daemon road,
scratch folder, imports, the roots file, cloning, and the memory key. Code never compares the kind string; it asks the
module.

## Behaviour

1. `machine_reading` (:321): one shared set of the release calls of every run being read on a machine; the cloud and
   place streams register into it.
2. Cloud (:323-372):
   - `backend(record)`: no place is the host's own backend; else the door's `backend_of(place)`, none failing
     `PlaceForksNowhere(place_forks_nowhere_line(name_of(place)))`.
   - `exec_stream`: `with_gh_door(machine_exec_stream(machine, {reading, ...o}, waiting))`.
   - `folder` is `sub.folder` (none for a non-sub fork); `home` is `cloud_home(id)`; `home_dir` is `GUEST_HOME`;
     `env` is `cloud_env(record, id)`; `login_stands` is `place_login_stands`; `relayed` true; `wsp_mcp` is
     `GUEST_WSP_MCP`; `turn_reach` is `{}`; `turn_road "relayed"`, `keeps_agents` false, `server_values
     "environment"`.
   - `has_daemon(entry)`: not served by its computer and the machine has a preview route. `shared_daemon` false.
     `daemon_road`: `cloud_road` when not served by its computer, else fails `place_serves_daemon_line(name,
     computer_of(entry))`. `scratch` is `GUEST_TMP`; `daemon_version` is `hello_version(entry)`; `dropped` does
     nothing.
   - `deploy_daemon` exists only when the recipe wires a deploy, and is `cloud_deploy`.
   - `import` is `copy_import`; `roots` is `write_roots(entry, dests, None)`; `land_project` is `clone_project`;
     `memory_key` is `project_memory_key(agent, project_held(record.project))`.
3. Local (:373-433), absent without the local wiring:
   - `backend` is the local backend; `exec_stream` the local one; `folder` is `checkout_of(record)`; `home(id)` and
     `home_dir` the local wiring's; `login_stands` is `shared_login_under(local.home(id), id)`; `env` is the local
     environment; `relayed` false.
   - `wsp_mcp`: the agents' `wspMcp` with `SCOPED_MCP_ARG` appended to its args, none without one.
   - `turn_reach`: `{url}` of the agents' `here.url`, none when it is absent or `""`. `turn_road "here"`,
     `keeps_agents` true, `server_values "file"`, `has_daemon` is whether the wiring has a daemon road,
     `shared_daemon` true, `daemon_road` is `local_road`, `restart_daemon` only when wired, `scratch` the local
     backend's folder.
   - `daemon_version`: none without a here daemon, else its `version()`.
   - `import` is `register_import`; `roots` writes to the wiring's `rootsPath`; `land_project` does nothing.
   - `memory_key` (:426-432): a made worktree not gone keys off its own path (`project_state_key(agent, tree.path)`);
     else a project whose git top differs from its path keys off the top; else `project_memory_key`.
4. Place (:434-504), absent without a place door:
   - `backend(record)`: `place_folder_backend(<the folder computer's machine>, shape)`.
   - `admit_folder(record)`: the machine's login must be `root`, else fails `place_login_not_root_line(name, user)`
     with kind `usage`.
   - `exec_stream`: with a `thread` option on a `PlaceFolderMachine`, the launch goes through
     `machine.in_cgroup(thread_cgroup(thread), paths.guestBin)`; options `{reading, runDir: paths.runDir, ...o,
     launchOn?}` (the caller's keys override `runDir`, never `launchOn`).
   - `port_reach(entry, port, {standing})`: the door forwards the port with `{pane: {workspaceId, name}, standing?}`;
     answers `{url: "http://localhost:<local port>/", expiresAt: now + PANE_HOLD_MS}`.
   - `end_thread` and `end_owed` are the thread-ends area's, on the record's place.
   - `folder` is `checkout_of`; `home(id)` is `place_agent_home(place, <its home>, id)`; `home_dir` the folder
     computer's home; `env` is `place_env`; `login_stands` is `place_login_stands`; `relayed` true.
   - `wsp_mcp`: `GUEST_WSP_MCP` only where the folder computer reports `tools: true`.
   - `turn_reach` `{}`, `turn_road "relayed"`, `keeps_agents` true, `server_values "launch"`, `has_daemon` false,
     `shared_daemon` true; `daemon_road` fails `place_serves_daemon_line`; `scratch` is the place paths' `putDir`.
   - `daemon_version`: the door's report's `daemonVersion`, none without a place or a report.
   - `import` refuses with kind `usage`: `<name> is a project folder on <computer>; a folder from this computer goes
     there as a project of its own, with wsp add <folder> --on <computer>`.
   - `roots`: the roots script for `roots_path_in(home)` run by the door's `exec` on the place (`INLINE_EXEC_MS`);
     non-zero fails `could not make <last dest> browsable on <place name>: <stderr last 200>`.
   - `land_project` does nothing; `memory_key` is `project_memory_key`.

## Edge cases

- `local` and `place` modules are absent where the host wired neither; `module_of` then fails `no_kind_line(kind)`.
- A cloud record whose place forks nowhere fails with the typed `PlaceForksNowhere` error, which callers match on.

## Behaviour nobody wrote down

- Rule 3.3: the modules hold no state of their own beyond `machine_reading`; every method reads the context afresh.

## Decisions

1. The table is three structs implementing wsp-core's `KindModule`, held in `KindsArea` as
   `Option<Rc<dyn KindModule>>` per kind.

## After the port

- None found.

## Not in this slice

- `exec_stream` is not on the `KindModule` stand-in: it hands back wsp-core's `ExecStreamFactory`, which is not ported yet. The
  cloud, local and place streams (rules 2-4) and the cgroup launch go in when wsp-core's machine-exec lands; no test here
  covers them.
- `admit_folder` reads the login through the context's `door_login_user(place)`, the stand-in for the folder computer's
  `PlaceFolderMachine.loginOf()`.

## Tests (tests/kinds_table.rs)

| Rule | Test |
|---|---|
| 2 | `cloud_backend_is_the_hosts_own_or_the_places_and_refuses_one_that_forks_nowhere` |
| 2 | `cloud_answers_its_fixed_facts` (relayed, roads, server values, scratch, home dir) |
| 2 | `cloud_daemon_road_refuses_a_workspace_its_computer_serves` |
| 3 | `local_wsp_mcp_carries_the_scoped_arg_and_turn_reach_the_here_url` |
| 3 | `local_memory_key_follows_a_made_worktree_then_the_git_top` |
| 3 | `local_import_registers_in_place` |
| 4 | `place_admits_only_a_root_login` |
| 4 | `place_exec_launches_in_the_threads_cgroup` |
| 4 | `place_port_reach_forwards_and_holds_for_the_pane_hold` |
| 4 | `place_import_refuses_with_the_add_line` |
| 4 | `place_wsp_mcp_only_where_the_report_says_tools` |
| 1, 21 | `module_of_refuses_a_kind_the_host_did_not_wire` |
| 3, 21 | `module_of_refuses_the_local_kind_on_a_host_with_no_local_road` |
