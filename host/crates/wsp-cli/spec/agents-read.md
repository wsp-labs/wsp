# agents_read: the target, the report and the rows of the four lists

Source: `agents-help.ts:1-298` at 65f6b89c7. Rust: `src/verbs/agents_help/read.rs`.
Recorded: `tests/fixtures/cli_c/agents_read.json` (357 cases, probe `record/agents_read.probe.ts`), tests `tests/cli_c/agents_read.rs`,
drivers `drivers_agents_read.rs`, hand tests `agents_read_rules.rs`.

## Scope

`drawRows`, `aimedBothLine`, `aimedUsage`, `agentsTarget`, `ProjectAsked`, `projectUnnamedLine`, `projectOffComputerLine`,
`toolsProjectBareLine`, `projectAsked`, `toolsProject`, `agentsReport`, `reportFacts`, `reportTail` (private), `agentRowLines`,
`scopeWord` (private), `skillRowLines`, `serverRowLines`, `PLUGIN_KIND_WORDS`, `broughtWords`, `pluginRowLines`, `pluginChanged`,
`pluginTurnedWords`, `pluginTurnedLine`, `SERVER_TOOL_COLUMNS`, `toolLines`, `serverToolsOf`, `serverChanged`, `unsetVariableLine`,
`notHeaderLine`, `serverValues`, `serverCommand`, `projectScopeLine`, `serverScope`, `toolsAddedLine`, `addTools`, `signInHere`,
`signedInLine`.

Declared by the tool module, not here (zod inputs and describe text): `AGENTS_FRAME` (119, the report shape less its four row lists),
`PluginIdIn` (186), `PluginAgentIn` (187), `PLUGIN_CHANGE_WORDS` (188-189), `ServerNameIn` (255), `ServerAgentIn` (256), `ServerScopeIn`
(257), `ServerProjectIn` (258-261), `SERVER_CHANGE_WORDS` (262-263), `SERVER_TOOLS_WORDS` (265-266).
`signInHere` is interface only: it needs a pty relay (`target_link`, `relay_sign_in`, a terminal); its words are `signedInLine`'s and
the host's `agents.signInLine` reply.

## Behaviour

- `drawRows` (11-34): without `--watch`, one dial, one frame, `out.emit(value, rows.join("\n"))`, exit 0. With it, `watchOn` is read
  first, before any dial (`--json` refused first, then no terminal); then one dial and `watchBlock` over the frame's rows, handed
  `ctx.signals` where set. A frame's refusal rides out; in a watch the cursor is shown again and the signals taken off first.
- `agentsTarget` (47-52): both refused (`aimedBothLine`, fix `usage`) before any frame; a thread through `threadAt(.., orComputer true)`
  to `{workspaceId}`; a computer through `placeNamed` to `{placeId, project?}`; neither `{placeId: "here"}` with no frame.
- `projectAsked` (93-101), `toolsProject` (105-109): see the function docs.
- `agentsReport` (113-116): target, then `agents.read {target}`; the report parsed as `AgentsReport` (unknown keys dropped, schema order).
- `reportFacts` (122): the facts in the order target, home, user, readAt, stale, pauseMode, refused, projects (absent ones left out).
- Row tables (132-175): `table` of a head and rows, then `reportTail` (the napping line, then `refused:` lines). `agentRowLines` has the head
  alone for no agents; skills, servers and plugins say `no skills`, `no MCP servers`, `no plugins`. Server state: launch beats disabled
  beats off beats auth, then `, not in recipe`; one line per distinct `off` sentence of the enabled servers sits between table and tail.
- `broughtWords`, `toolLines`: see the docs. A tool's description is cut at its first `\n` only.
- `pluginChanged` (178-181), `serverToolsOf` (201-204), `serverChanged` (207-210), `addTools` (271-273): the target, then one op; the
  reply parsed or cut as the docs say. `plugins.toggle {target, agent, plugin, on}`; `servers.tools {target, agent, name, refresh?}`;
  `<op> {target, ...body}`; `agents.addTools {target: {placeId: "here"}, agent}` with no `places.list`.
- `serverValues` (217-232), `serverCommand` (235-241), `serverScope` (247-253): refusals are usage refusals with fix `usage`.
- `signedInLine` (295-298): `<what> is signed in on <where>`, ` (<detail>)` where a detail is set, or `is not signed in`, `: <said>`.

## Limits and caps

None numeric. Column widths are the widest cell in UTF-16 units (emoji names, accents, a name wider than the head). Counts of zero, one
and a dozen in `broughtWords`. A table of no rows (the head alone) and of one.

## Null and empty inputs

No agents, skills, servers, plugins (key absent and empty list), no `refused`, no `projects`, empty pause mode; empty thread, computer,
project, usage, variable, header, tool name; no line for `serverCommand`, an empty line, a line of spaces; no environment; empty
description; empty `off` sentence; empty detail and said.

## Order

Frames: `agentsTarget` frames (thread: `sessions.list`, `workspaces.get`; computer: `places.list`) then the op. Lines: table, off
sentences, napping line, refused lines. Keys: `plugins.toggle` and the server ops as listed above. `serverValues`: variables before headers.

## Invariants held

- Invariant 7: with the cloud off a word naming a machine is no thread (`invariant_7_cloud_off_a_machine_word_is_no_thread`) and the row
  functions add no cloud word of their own (`invariant_7_the_row_functions_add_no_cloud_word_of_their_own`).
- Guest rule: not reached; these helpers take no `elsewhere` and read no path. `addTools` always targets `here` through the host.

## What the TS leaves unclear (after-port tickets, not fixed here)

- `serverValues` builds its maps with `Object.fromEntries`, so names that are whole numbers come first in numeric order (223-231);
  the Rust must order the same way (cases `a name that is a whole number...`). Probably nobody means it.
- `projectAsked` takes a `thread` it never reads (93); `agentsTarget` drops `project` for a thread (49).
- `scopeWord` reads a skill's `local` scope that `SkillScope` cannot hold (140).
- `agentsTarget` with an empty `on` is a word naming no place, refused after a `places.list` (a usage sentence naming the places).
- Rust cases with `cloud: true` need a per-call cloud switch; `thread_at` reads the process's cloud setting.

## Rule to test

| Rule | Tests (recorded `agents_read.rs` unless named) |
|---|---|
| thread and computer refused before any frame | `agents_target__both_are_refused_*`; `a_thread_and_a_computer_together_send_no_frame_by_any_road` |
| a project word is no thread | `agents_target__a_word_that_is_a_project_s_name*`, `agents_report__a_project_word_*` |
| cloud off, no machine word | `invariant_7_*` (three) |
| watch refused before any dial | `a_watch_with_nowhere_to_redraw_is_refused_before_any_dial`, `draw_rows__watch_*` |
| one dial for a watch | `a_watch_holds_one_dial_over_every_frame` |
| non-watch path | `draw_rows__one_frame_*`, `draw_rows__under_json_*`, `draw_rows__no_rows_*` |
| tables aligned to the widest cell, whole columns | `agent_row_lines__*`, `skill_row_lines__*`, `server_row_lines__*`, `plugin_row_lines__*`, `tool_lines__*` |
| no line ends in white space; tail last | `row_tables_never_end_in_white_space_and_the_tail_comes_last` |
| server state order | `server_row_lines__a_launch_row_*`, `server_row_lines__disabled_*`, `server_row_lines__off_*`; `ts_agents_reader_*` |
| report facts only | `report_facts__*`, `report_facts_never_carry_rows_or_reach` |
| frames sent per write, key order | `plugin_changed__*`, `server_tools_of__*`, `server_changed__*`, `add_tools__*`; `ts_agents_signin_add_tools_*` |
| variables and headers | `server_values__*`, `server_values_order_whole_number_names_first` |
| command words and scope | `server_command__*`, `server_scope__*` |
| `--project` refusals | `project_asked__*`, `tools_project__*`, `project_refusals_are_usage_and_carry_the_lines_usage` |
| words and consts | `const_*`, `aimed_usage__*`, `project_off_computer_line__*`, `plugin_turned_*`, `signed_in_line__*`, `tools_added_line__*`, `not_header_line__*`, `unset_variable_line__*`, `project_scope_line__*` |
| both cloud settings | `agents_target__*_the_cloud_on`, `agents_target__a_word_that_is_a_machine_s_name_the_cloud_off` (the rows read no cloud setting) |
