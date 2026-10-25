# agents_read_targets: where a read runs, the report, and every write there

TS: `agents-read.ts:1-311, 351-485, 597-647`. Rust: `src/agents_read.rs` (the types, `AgentsReadOptions`, the ports
`AgentsReader`, `AgentsActs`, `SkillsActs`, `ServersActs`, `PluginsActs`, `ServerIcons`, `CallbackForwards`,
`AgentsPlaces`) and `src/agents_read/targets.rs` (methods of `AgentsReads`). TS tests: agents-read.test.ts (25 cases;
the reads, skills, servers and projects groups here, the sign-ins in agents_read_sign_ins), places-agents,
agents-brokered, box-folder. Tests: `tests/agents_read_targets.rs`.

The TS options object is `AgentsReadOptions`: every optional member is an `Option`, every function an `Rc<dyn Fn>`,
the ports `Rc<dyn Trait>`. `places` answers the door each call (it can change). `AgentsPlaces` names exactly the place
door calls this file makes (`list`, `report_of`, `logins_again`, `sign_ins_at`, `exec`, `gh_there`, `key_landed`,
`login_landed`); the join implements it over `PlaceDoor`. `Caller` is `wsp_wire`'s `Caller`, passed through untouched.

## Rules

A1. `changed(target)` (agents-read.ts:286): no target, or a workspace target, is passed as it is; a computer target is
passed as `{placeId}` alone, so a write in one project is the computer's change.
A2. A missing port refuses with `NO_AGENTS_READER`, no kind, before anything else is read: the reader for `read`,
`tools`, `server`; the acts for `addTools`, `key`, `signIn`, `signInLine`; the skills for every skill road but icons;
the servers for the three server writes; the plugins for `pluginsToggle`; `setupWrite` for `setup`; and
`reader.server` for `server`.
A3. Where a read runs (agents-read.ts:388-425). This computer (`placeId == "here"`): `{kind: here, stores?, projects?}`,
`stores` from `stores_here()` (a failure read as none, an empty map left out). A computer: no door fails
`NO_PLACE_DOOR`; a place the door's `list(now)` does not hold refuses `no_such_place_refusal(id, <rows as
table_name>)` and a place that is not a joined computer refuses `provider_agents_refusal(table_name(row))`, both kind
`usage`; then `report_of(id)`, then (for a read only) `logins_again(id)`, then `sign_ins_at(id, <every catalog agent
id>)`. The answer is `{kind: box, name, machine (exec through door.exec), login {HOME?, PATH?} off the report,
signIns?, versions? (the report's agentVersions), setupRows? (the row's applied rows), logins?, stores?
(placeStores(id, HOME) where both stand), relayed? (true where a registered relay reaches the target), projects?}`. A
workspace: `workspace(id, origin)`; napping answers `{napping: name, pauseMode?}` and asks nothing more; on this
computer `{kind: here, projects: [project], stores?}`; else `{kind: machine, machine, projects: [project], relayed?
(o.relayed() true), brokered?, stores?, seeds?}`.
A4. The projects a computer target covers (agents-read.ts:377-386): an act naming no project covers none and asks
nothing; else `projects(placeId)` (none read as `[]`). No project named: all of them. Named: the one whose id it is,
else the one project of that name; none refuses kind `usage` with `shared_agents_project_refusal(said, computer)` where
two or more share the name, else `no_such_agents_project_refusal(said, computer)`, `said` the name with control
characters taken out and `computer` `THIS_COMPUTER` or the row's name.
A5. `read(target, origin)` (agents-read.ts:441-457): a napping workspace answers its last report with `stale:
"napping"` and its `pauseMode`, or refuses `napping_agents_refusal(name, pauseMode)` kind `usage` where none was read;
the reader is never asked. Else the reader reads with `latest` from `latest_on()` (none: true), each agent row of a
computer target gets `setup` from `setup_of(placeId, row id)` where it answers one, and the report is
`AgentsReport` with `target`, `readAt` (`now()` as ISO) and `reach` (A6); `runAs` never reaches the report. A
workspace's report is kept as its last.
A6. `page_reach_of(on, run_as)` (agents-read.ts:44): `here` on this computer; `relay` where the target is relayed and
the lines run as nobody else; else `none`.
A7. `setup(placeId, agent, change)` (agents-read.ts:461-470): the write, after which the computer's report changed
(even when the write failed); then a fresh read; the agent's row, or `there is no agent <agent> in the catalog` kind
`usage`.
A8. `tools(target, ask)` (agents-read.ts:471-476): a napping workspace refuses `napping_tools_refusal`; the reader is
asked with `key` the target's JSON (`{"workspaceId":"w"}`, `{"placeId":"p","project":"x"}` in that key order) and the
ask's `agent`, `name`, `refresh`. `server(target, ask)` the same refusal, then `reader.server(on, ask)`.
A9. `forget(workspace)` drops that workspace's last report (agents-read.ts:485).
A10. Skills (agents-read.ts:611-625): search asks with the limit given, else `SKILLS_SEARCH_LIMIT` (20); get, preview,
add, remove and toggle; a napping workspace refuses `napping_skills_refusal` before the port is asked; add, remove and
toggle say the target changed after, whether the write held or failed.
A11. Servers (agents-read.ts:358-366, 626-629): add, remove and toggle refuse a napping workspace with
`napping_servers_refusal` and say the target changed after, held or not. `servers_icon(host, refresh)` answers null
where no icons port is wired, else the port's answer.
A12. Plugins (agents-read.ts:630-634): napping refuses `napping_plugins_refusal`; the write says the target changed
after, held or not; answers `{plugin}`.
A13. `add_tools(target, agent)` (agents-read.ts:635-642): napping refuses `napping_sign_in_refusal`; the target is
said changed only after the write held.
A14. `key(agent, key, way)` (agents-read.ts:606-610): the acts write it, then every report changed (`changed(None)`),
then the door's `key_landed(agent)` where a door stands.
A15. `forwards(relay)` (agents-read.ts:643-646) registers a relay and answers its removal.

## After the port

- A5: `readAt` reads `o.now()` but A3's place listing reads `door.list(o.now())` separately; two clock reads.
- A3 asks `logins_again` before the reader runs, so a read of a computer always costs a login listing.

## Tests

| Rule | Test |
|---|---|
| A1 | `a_write_in_one_project_is_the_computers_change` |
| A2 | `every_road_refuses_without_its_port_before_reading` |
| A3 | `a_workspace_reads_off_its_machine_with_its_project`, `a_workspace_on_this_computer_reads_as_this_computer`, `a_joined_computer_reads_through_the_door_with_its_login_and_sign_ins`, `a_computer_the_door_does_not_list_is_refused_usage`, `a_provider_place_is_refused_usage`, `no_door_fails_no_place_door`, `this_computers_stores_are_left_out_when_empty_or_failing` |
| A4 | `a_read_of_a_computer_covers_every_project_held_there`, `an_act_names_one_project_by_id_or_by_a_unique_name`, `a_shared_name_and_an_unknown_project_are_refused_with_control_characters_out` |
| A5 | `a_napping_workspace_answers_its_last_report_stale`, `a_napping_workspace_never_read_is_refused`, `the_report_carries_each_agents_setup_on_a_computer`, `the_report_asks_no_vendor_where_the_person_said_no` |
| A6 | `the_report_says_where_a_sign_in_page_reaches` |
| A7 | `setup_writes_reads_again_and_answers_the_row`, `setup_refuses_an_agent_not_in_the_report` |
| A8 | `tools_are_asked_on_the_workspaces_machine_keyed_by_the_target`, `tools_and_server_refuse_a_napping_workspace` |
| A9 | `forget_drops_the_last_report` |
| A10 | `skills_search_asks_twenty_unless_told`, `a_skill_write_says_the_target_changed_held_or_not`, `skills_never_touch_a_napping_workspace` |
| A11 | `a_server_write_says_the_target_changed_held_or_not`, `servers_never_touch_a_napping_workspace`, `servers_icon_is_null_without_icons` |
| A12 | `plugins_toggle_answers_the_row_and_says_changed` |
| A13 | `add_tools_says_changed_only_after_it_held` |
| A14 | `a_key_changes_every_report_and_lands_on_the_door` |
| A15 | `a_relay_registers_and_unregisters` |
