# detect_turn_servers

Source: `packages/collect/src/detect/turn-servers.ts` (26), `mcp-project.ts` (77) at `a3b870a6e`. Tests: none in collect; `packages/host/test/project-servers.test.ts` reaches them through the host. Rust: `src/detect/{turn_servers,mcp_project}.rs`; tests `tests/detect_turn_servers.rs` (6 on these files, against `tests/fixtures/oracle/turn-servers.json`).

## What it does

The MCP servers a turn of an agent gets in a folder, read off one Host by the catalog's resolver; and of those, the ones a project carries to another computer because its checkout does not bring them, each with the scan's row and why it may stay.

## Behaviour

1. `read_checkout` (turn-servers.ts:9): `run("git", checkout_args(folder))` with no options, then `checkout_of(answer, folder)`.
2. `read_turn_servers` (:15-26): the checkout is the one handed in, else `{key: folder}` with `own`, else `read_checkout`. `turn_server_files(agent, folder, home, stores[agent.id], top)`. The own file is the first `files.user` candidate `read_text` answers (later ones not read). Without `own`, every `files.projects` path is read, in order, all of them, keeping those that answer. Then `turn_servers(agent, {user, projects, folder, key})`. The TurnAgent is the entry's id and its `mcp` format, files and project files; an entry with no `mcp` gets `[]`.
3. `detect_project_mcp` (mcp-project.ts:55-77): `mcp_remote_tokens(host)`, then `read_checkout(folder)` once. For each agent in order that has a launch road (`launch_server_road(id)`): its turn servers in the folder with that checkout, minus `user`-scope ones. An agent with any `project`-scope server goes in `read`. `brought`: the distinct files of its project servers, those inside the checkout top (or the folder), relative to it, asked of `git -C <top> ls-files -- <rel...>` (no call when none), plus those whose path relative to the folder is in `keep`; absolute again.
4. Per server name, first one wins: a project server whose file is brought is skipped; else the row is `mcp_server_row(agent, server with scope user, tokens)`, and its reason is `SWITCHED_OFF_HERE` for a disabled server, else `outside_project_line(paths)` when any outside path, else none; with a reason the row's default is `skip`.
5. Outside paths (:32-39): none for http; for stdio, the command (with `~/` expanded) when under home and not under `~/.local/bin`, as a tilde path, then the catalog's `home_paths(server, home)`, distinct, keeping those (expanded) neither inside the top nor equal to it.
6. `outside_project_line` (:27-28): `runs against <paths joined by ", ">, outside the project, which nothing carries to that computer`.

## Edge cases

- Outside git, the checkout is `{key: folder}` with no top (`the_checkout_is_what_git_names_*`).
- A tracked project file's servers stay out of the answer; an untracked one's are carried (oracle).

## Decisions

1. `read_turn_servers` and `detect_project_mcp` take `&AgentEntry` (TS `McpAgent`) so the detect writer's `mcp_server_row(&AgentEntry, ..)` and these agree.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `the_checkout_is_what_git_names_for_the_folder_or_the_folder_alone_outside_git` |
| 2 | `a_turns_servers_are_read_off_the_agents_own_file_*`, `own_reads_the_agents_own_file_alone_*`, `a_checkout_handed_in_is_used_*` |
| 3, 4, 5 | `a_project_carries_the_servers_its_checkout_does_not_bring_*` (rows, read, checkout and every call) |
| 6 | `the_outside_line_names_every_path_*` |
