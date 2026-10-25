# account_slates
Source: packages/runtime/src/account/slates.ts (176 lines) at 65f6b89c7. TS tests: none of its own; every runtime slate test goes through it (`slate-mcp.test.ts`, `slate-box-*.test.ts`, `slate-here-http.test.ts`, `slates.test.ts`, `lazy-slates.test.ts`). Rust: `src/account/slates.rs`; tests `tests/account_slates.rs`.

## What it does
The slate area's wiring: what `createSlates` (slates-a's) is handed, each answer read off the runtime's context per call. It holds no state of its own beyond the slates handle and one bus listener.

## The context it reads
A trait of its own, `SlatesAreaCtx` (ruling 6401022701 rule 4), implemented on wsp-core's context handle at the runtime join. `Entry` is wsp-machines' (`Rc<RefCell<LiveWorkspace>>`), so the record's `id`, `name`, `place`, `project` and `phase`, the entry's `machine`, `checkout` and `pr`, and the kind's words (`folder_on_joined`, `is_local_workspace`, in wsp-words) are read where they are, not through the context. What stays on the context: `latest_on(thread)`, `thread_record(thread)`, `running_turn(thread)` (the `turnId` of a running session of the thread, first in the sessions map), `live(workspace)`, `box_of(thread)`, `folder_of(workspace, session)`, `runs_in(entry, ran_in, fallback)`, `kind_folder(entry)` (the kind module's `folder(record)`), `checkout_of(record)`, `is_local(record)`, `path_exists(path)`, `home_dir()`, `computer_of(entry)`, `root_of(thread)`, `tool_asks(harness, mode)`, `boot_work(workspace)`, `wake(workspace)`, `ready()`, `settled_now(thread, ms)`, `settle_after()`, `tree_under(lead)`, `thread_of_token(token)`, `own_process(thread, pid, kind)`, `with_daemon_image(entry, path)`, `with_daemon_hash(entry, root, paths)`, `agents_server(target, agent, name)` (the box road), `agents_reader_server(projects, agent, name)` (this computer; none when the host reads no config), `stdio_road(entry, min_version)`, `place_name_here()`, `at_provider(place)`, `agent_label(agent)`, `rows_on(thread)`, `transcript(workspace)`, `usage_account_key(entry, harness)`, `usage_accounts()`, `project_name(project)`, `rate_usd_per_hour(record)`, `accrued(workspace)`, `start_session(...)`, `on_queued(request_id, fn)`, `poll_pull_request(entry)`, `local_env()`, `process_env()`, `state_folder()` (none without a state path).

## Behaviour
1. `harness_of(thread)` (`:16`): the latest row's harness, else the thread record's.
2. `servers_target(box)` (`:18-19`): `{placeId, project}` when the record has a place and its kind keeps a folder on a joined computer (`folder_on_joined`), else `{workspaceId}`.
3. `machine_on_box(box)` (`:22`): the record has a place, its kind is not a folder on a joined computer, and the place is not at a provider.
4. `thread(thread_id)` (`:28-53`): the workspace is the latest row's, else the record's; none when neither. `running` = a session of this thread with status running. `ran_in` = the latest row's cwd, else `folder_of(workspace, claudeSessionId)` when the row has one. `asks = tool_asks(harness, record's permissionMode else latest's)`. With no live entry: `{workspaceId, rootThreadId, sessionId: latest.claudeSessionId else latest.id else thread, turnId?, toolAsks}`. Else `folder = runs_in(entry, ran_in, kind_folder(entry) else checkout_of(record))`; `hostFolder` = the folder for a local workspace, else the checkout when it exists on this disk, else the home folder; answer `{workspaceId, rootThreadId, sessionId, turnId?, folder, hostFolder, computer, toolAsks}` in this key order.
5. `machine_of(thread)`: `box_of(thread)`'s machine. `image_on(thread)`: none without a box, else `{name: computer_of(box), read: path => fs.image through the daemon, parsed as FsImageReply}`. `hash_on(thread)`: none without a box, else `(root, paths) => fs.hash through the daemon, parsed as FsHashReply`. `asleep(thread)`: a box whose record's phase is not `running`. `wake(thread)`: nothing without a box; else await its boot work, then `wake(workspace)` when the phase is still not running.
6. `settled(thread)`: `settled_now(thread, SETTLE_MS[settle_after])` (15m 900000, 1h 3600000, 2h 7200000, 1d 86400000, never null).
7. `mcp_server(thread, name)` (`:83-102`):
   - workspace as rule 4; no live entry or no harness fails `this host holds no folder for thread <thread>`;
   - a box: `machine_on_box` fails `slate_machine_on_box([name], record.name, computer)`; else `agents_server(servers_target(box), harness, name)`; no `login` fails `<computer> did not say who its lines run as`; the spec (less `login`) gains `open = pipe_on(computer, login, () => stdio_road(box), mark)` with `mark = {WSP_THREAD (THREAD_ENV): thread, SLATE_DIR: box_slate_dir(thread)}` in that order, and `post = post_on(computer, () => stdio_road(box, HTTP_POST_DAEMON_VERSION), spec.signIn)`;
   - here: no reader fails `this host reads no agent's MCP config`; `agents_reader_server({kind: here, projects: [{id: workspace, name: "thread", path: checkout_of(record)}]}, harness, name)`; when its sign-in has a `read`, the spec gains `post = post_here(computer, sign_in)`.
8. `away(thread)`: none without a box; else `{computer, host: place_name_here(), machine: record.name only when machine_on_box}`.
9. `missing_on(thread, servers)` (`:107-116`): no box or no harness: `{agent: {id: harness or "", name: harness or ""}, thread, missing: []}`. Else every server asked at once through `agents_server`; `off` = the messages of failures that are server-off errors, in server order; `missing` = the servers whose failure is a not-set-up error; a read that failed otherwise refuses nothing; agent `{id, name: agent_label(id)}`.
10. `sources(thread, workspace)` (`:117-144`): `now` read once at build; `rows()` = `rows_on(thread)` sorted by `startedAt` (none 0) ascending, stable; `results()` = the workspace transcript's `session.done` results of this thread in order; `account()` = none without a live entry or harness, else the usage account whose key is `usage_account_key(entry, harness)`; `workspace()` = none without a live entry, else `{computer, project (name, when the record has one), folder (checkout), checkout?, pr?, rateUsdPerHour (0 when the record has no size), accruedUsd (0 when none)}`.
11. `deliver({thread, workspace, prompt, request_id})` (`:145-164`): listen for `session.queued` with this request id before starting; start a session `{prompt, thread, requestId, startedBy: "person", via: "slate"}`; whichever comes first: the start's answer gives `{outcome, turnId}`; the queued event gives `{outcome: "queued"}`, and a later failure of that start is logged as a warning `a press waiting in thread <thread_word(thread)> was not sent: <message>` and never surfaces. The listener is removed in every case.
12. `watch_pr(workspace)`: polls the pull request of a live entry; nothing otherwise. `run_env()`: the local wiring's environment, else the process environment.
13. With a state path: `secrets_file = <state folder>/slates.secrets.json`, `slates_dir = <state folder>/slates`; without one, neither key.
14. On `workspace.napped`, `slates.napped(workspace_id)`.

## Invariants (named tests)
- A box thread's MCP server is started there with that computer's config, never this computer's: `mcp_server_on_a_box_reads_that_computers_config_and_marks_the_thread`.
- A machine wsp made on a joined computer gets no tool run: `mcp_server_refuses_a_machine_on_a_box`.
- The thread's mark reaches the server's start: `mcp_server_on_a_box_marks_the_thread_and_slate_dir`.

## Decisions
1. The area answers a `SlatesWiring`, which implements `SlatesDeps` (slates.rs): its methods are the closures above, `store`, `clock`, `record` and `emit` read the context, and `runs` is left to the area's default road. `lazy_slates(Rc::new(wiring), Box::new(create_slates))` is the TS's `lazySlates({...})`. `slates_area(ctx, napped)` registers the nap listener.
2. Everything the TS reads through `ctx` goes through `SlatesAreaCtx`, so the tests drive a fake.

## After the port
- `missing_on` asks every server at once on the box, one daemon read per server; kept.
- `deliver`'s queued answer drops the start's later failure into the log only; the person sees nothing. Kept.

## Tests (`tests/account_slates.rs`)
| Rule | Test |
|---|---|
| 1, 4 | `thread_without_a_live_entry_names_the_session`, `thread_on_a_box_runs_in_its_folder_there_and_here_in_the_checkout_or_home` |
| 2, 3, 7 | `mcp_server_on_a_box_reads_that_computers_config_and_marks_the_thread`, `mcp_server_refuses_a_machine_on_a_box`, `mcp_server_on_a_box_without_a_login_fails`, `mcp_server_here_adds_post_here_only_with_a_sign_in_read`, `mcp_server_with_no_folder_or_no_reader_fails` |
| 5 | `box_roads_answer_none_for_a_thread_here`, `asleep_and_wake_follow_the_phase` |
| 6 | `settled_reads_the_settle_preference` |
| 8, 9 | `away_names_both_computers_and_the_machine_on_a_box`, `missing_on_splits_missing_and_off_and_ignores_other_failures` |
| 10 | `sources_sort_rows_and_read_this_threads_results` |
| 11 | `deliver_answers_queued_when_the_press_waits`, `deliver_answers_the_started_turn` |
| 13 | `secrets_and_slates_dir_sit_in_the_state_folder` |
| 14 | `a_nap_reaches_the_slates` |
