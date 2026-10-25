# import_agents (golden-import.ts:1348-1403, 1422-1534)

The ticked agents' installers, the one Node step their floors ask for, and the agents as steps of the tools loop.
Pure; `now` is passed in as milliseconds since the epoch (the provision wrapper reads the clock).

## Installers (:1368-1403)

- An agent's installer: `name`, `install` = `install_line(entry)` (a catalog agent with no line is a catalog fault:
  the TS throws at load, the port panics), `smoke` = `smoke_of(entry)`, `road` = its road's name, `node` = its engines
  floor where set, `pin` = `pin_read_of(road, bin, None)`.
- `agent_installers(agents)`: each agent's by id in the slice's order, then `aider`: name `Aider`, install `UV_INSTALL`,
  newline, `uv tool install --force --python 3.12 --with pip aider-chat==0.86.2`, smoke `aider --version`, road `uv`,
  no node, no pin.
- The npm line `^npm install -g (?:--ignore-scripts )?(\S+?)@\S+$` (multiline) and the uv line `^uv tool install
  .*?([\w.-]+)==[\w.-]+$` name the package an install puts on.

## Node (:1374-1384, 1499-1503)

- `CURRENT_LTS` is 22. `node_major_for(floor, now)`: the pinned majors ascending (`NODE_RELEASES`: 20, 22); `None` when
  `floor` is above the last; else the first at or above `floor` whose `now < Date.parse("<eol>T23:59:59Z")`, else 22.
  Floor 0 reads as any major.
- `node_floor_check(floor)`: `major="$(node --version 2>/dev/null | sed 's/^v//; s/\..*//')"; [ "${major:-0}" -ge <floor> ]`.

## agent_installs_for_rows(entries, agents, now) (:1452-1494)

1. Rows that are ticked, `rung == "agents"` and not an MCP row (`agents/mcp/`), in recipe order; the agent id is the id
   after its first `/`. A catalog agent installs by `pinnedInstaller`: a row pin that is not a latest mark fixes the
   road at the pin's tag through the module's `at` (when the module has none, the catalog road stands); otherwise the
   catalog road. An id the slice does not hold takes `agent_installers`' own (that is `aider`). Neither: skipped `no
   installer known`.
2. On node: a catalog agent whose road's module waits on a manager's toolchain (npm's node); an installer outside the
   catalog whose line is an npm global.
3. An agent with a floor `node_major_for` cannot meet is skipped `needs Node <floor>, none pinned` and leaves the
   installs; skips keep the order they were found in (no installer first, in recipe order, then the floors).
4. `floor` = the highest floor among the installs left, else the catalog node row's major as a number (0 if none).
   When an install has a floor or runs on node and `node_major_for(floor, now)` answers: `node = { floor, version:
   release.version, agents: <their names, recipe order>, cmd: node_install_script(floor, release) }`.

## agent_steps_with(plan, agents, path, prefix) (:1505-1534)

1. With a node install: `agents/node`, label `Node <version>`, manager `script`, cmd the script, shown `Node <version>
   for <agents joined ", ">`, check `node_floor_check(floor)`, bin `node`.
2. Each install in order: id, label `name`, manager `road`, cmd `path_line(path, prefix)`, `NODE_PATH_LINE`, the
   install line (newline-joined), shown `shown_of(install.split("\n"))`, check `smoke`, `after: agents/node` when a node
   step exists and the road's module waits on a manager's toolchain; for an agent the slice holds: `bin`,
   `road_reads(road, bin, prefix)` and `asks_version(road)` off the catalog road (not the pinned one); `pin` where set.
   The plan's skips make no steps. No installs and no node is no steps.

## Rules against tests

| Rule | Tests (`golden_import_agents_hard.rs`, `golden_import_agents.rs`) |
|---|---|
| installers: lines, checks, roads, floors, pins, aider last | `every_installer_is_the_ts_line_check_road_floor_and_pin_read_with_aider_last`, `every_known_agent_has_a_pinned_installer_its_version_check_and_its_documentation`, `the_hermes_and_aider_installers_bring_uv_by_a_checksummed_release_binary_and_pin_source` |
| the major for a floor at three dates, above every pin | `node_major_for_picks_the_lowest_supported_pinned_major_else_the_current_lts` |
| the floor check, under a real shell | `node_floor_check_reads_the_major_the_way_the_install_script_does`, `reads_that_floor_the_way_the_install_script_reads_it_under_a_real_shell`, `the_node_script_keeps_a_guest_between_the_floor_and_the_pin_else_installs_the_pinned_release` |
| ticked only, recipe order, unknown skipped, MCP and shell rows ignored, pins, latest marks | `agent_installs_for_plans_the_ts_installs_skips_and_node_step_for_every_recipe_and_date`, `installs_only_the_ticked_agents_in_recipe_order_each_from_the_catalogs_table` |
| one Node step, the agents that asked, a floor no pin meets | `asks_for_node_once_at_the_lowest_supported_pinned_major_meeting_every_ticked_floor`, `an_agent_whose_floor_no_pinned_major_meets_is_set_aside_with_a_note` |
| the steps: node first, waits, reads, both paths | `agent_steps_are_the_ts_steps_for_every_plan_on_both_paths`, `puts_the_node_step_first_and_every_agent_whose_road_runs_on_node_after_it`, `names_the_node_step_after_the_release_it_installs_and_reads_the_floor_its_script_reads`, `keeps_an_agent_a_computer_has_at_the_pin_or_newer_and_installs_over_an_older_one`, `carries_no_node_step_when_nothing_ticked_runs_on_node_and_nothing_for_no_agent` |
| the provision wrapper | `the_wrapper_provision_calls_plans_provisions_own_rows_as_the_ts` |
