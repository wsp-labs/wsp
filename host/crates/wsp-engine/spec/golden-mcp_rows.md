# mcp_rows (golden-mcp.ts:337-566)

Which kept servers' commands the machine lacks, the plan with those dropped, every server as a row with the words a
person reads, and the builder's whole round (`apply_mcp`). Nothing here fails and nothing survives a restart.

## Commands (:346-437)

- `fetchers()`: `npx` → `npx`, `uvx` → `uv`, `uv` → `uv`, in that order. A command's basename is what follows its
  last `/`.
- `mcp_tally(rows)`: `<n> installed`, `<n> skipped`, `<n> on first use` (fetched-on-first-use), zero counts left out,
  joined `, `; `None` when all are zero.
- A row's named commands: its own `bin`, its catalog entry's bin (`catalog_entry(catalog_id_of_row(id))`) and, for a
  tool, its `brings`' bins. `command_row(command, rows)`: the first row whose named commands hold the basename, else the
  first binary row (`^tools/(brew|go|cargo|npm|pnpm|bun|pipx|uv|hand)/`) whose last id segment is the basename.
- `server_needs(command, rows, ready)`: nothing for a fetcher, a name in `ready`, or a floor bin or brought bin; else
  the command's row; else every binary row or `tools/custom/` row with no named command, in order.
- `absent_reason(command, plan, tools)`: `command not on the machine`, then where `command_row(command, plan.tools)`
  finds one: `; <id> was not ticked` with ` (<reason>)` where it has one; or, where that row's tool result is skipped
  or failed, `; <id> was skipped` / `; <id> failed` with ` (<note>)`; else `; <id> was ticked, but nothing by that
  name is on PATH`.
- `as_run(plan, command)`: a leading `~/` becomes `<guest_home>/`; nothing else changes (`~x` stays).
- `absent_commands(machine, plan, read, path)`: every distinct command of every result that has one, `as_run`, in
  first-met order; none asks nothing. One exec (20,000): `export PATH=<path>` then per command `if command -v <q c>
  >/dev/null 2>&1; then echo <q "ok c">; else echo <q "no c">; fi`, newline-joined. A rejected exec is `refused`.
  Answer: every stdout line starting `no `, less those three characters, in order.

## The plan less what is missing (:439-460)

- `without_absent(plan, agents, read, missing, tools)`: per scope in plan order with the next read outcome: a kept name
  whose result has a command, whose basename is no fetcher, and whose `as_run` is in `missing`, leaves `keep` and joins
  the end of `drop` with `absent_reason(command, plan, tools)`, in keep order. A scope with none is unchanged.
- `mcp_opening(agents)`: `<label> <kept + dropped + aside>` per agent, joined `, `.

## mcp_rows (:462-534, 566)

1. Per agent, per scope with the next outcome of `report`: the words every kept server is skipped with are `failure`
   when set; else, no outcome or a `None` file: `<label>'s config is not on the machine`; unread:
   `config_unread_line(file, unread)`; an error: `<label>'s config on the machine did not parse (<error>)`.
2. Each kept name: skipped with those words, or with `not in the config that travelled` when there are none and the
   result is absent or `missing`. Else `fetched-on-first-use` with the note `<fetcher> fetches the package on first
   use` (short `package fetched on first use by <fetcher>`) when the command's basename is a fetcher, else `installed`.
   Then each drop, skipped with its reason and its `keys`. Then, per agent, its aside rows. Row ids are
   `scope_row_id`; an aside row keeps its own id.
3. uv: the rows whose command's basename is `uv` or `uvx` and whose `as_run` is in `missing`. With any: stage
   `installing-mcp`, `uv for <names joined ", ">`; one run of `guarded_road(script, UV_INSTALL)`, deadline
   `guard_deadline_ms(road_limit_s(script))`, each line staged `uv: <line>`; a rejected run is `refused`. Exit 0 puts
   `uv installed for it` (short `uv installed`) first on each; else `uv did not install (<reason_of(res, limit)>)` and
   the note `<that>; the server starts once it is installed there`, also first.
4. Every other row with a command whose `as_run` is in `missing`: short `<basename> is not on the machine`, note `<that>;
   the server starts once it is installed there`, appended.
5. Closing stage `installing-mcp`: `closing(summary, free_note(machine, None))`, a rejected df leaving the second part
   off. The summary, `; `-joined, `nothing to do` when empty: `<names> already there` for the rows in `present`; `<names>
   installed` for installed rows with no short and not present; per distinct joined shorts in first-met order,
   `<names>: <shorts joined ", ">` (skipped and present rows left out); each skipped row `<name> skipped (<first note>)`.
6. The rows: `{ id, agent: label, name, outcome, note: notes joined "; " where any, keys }`.

## apply_mcp (:536-564)

Stage `installing-mcp`, `mcp_opening(plan.agents)`. One unlogged run of `read_configs_cmd` (every scope, base the plan's
guest home), deadline `READ_MS`, rejected is `refused`. Exit not 0: `failure = read_failed(res)`. Exit 0 and
`parse_configs` answers nothing: also `read_failed(res)`, which then reads `exit 0`. Else edit once, ask
`absent_commands` on that edit (on `TOOLS_PATH`), `without_absent`, edit again, `report` = the second edit's outcomes,
`failure = land_configs(machine, guest_home, read, second texts)`. Then `mcp_rows` with no `present`.

## Rules against tests

| Rule | Tests (`golden_mcp_rows_hard.rs`, `golden_mcp.rs`) |
|---|---|
| fetchers, tally | `the_fetchers_are_npx_uvx_and_uv_in_the_ts_order`, `mcp_tally_counts_by_outcome_and_leaves_out_zeros`, `the_tally_counts_servers_by_outcome_zero_counts_left_out_and_nothing_for_an_empty_plan` |
| command rows and needs | `command_row_and_server_needs_name_the_row_that_puts_a_command_on_as_the_ts`, `names_the_row_that_puts_a_command_on_and_otherwise_a_server_waits_on_every_unnamed_row` |
| the PATH check: distinct, as run, refused, none asked | `absent_commands_asks_once_for_every_distinct_command_as_run_and_reads_the_no_lines`, `a_path_check_the_machine_refuses_drops_no_server_the_definitions_land_and_the_stage_closes` |
| reasons and as-run | `as_run_and_absent_reason_say_what_the_ts_said`, `the_reason_names_the_tool_row_that_would_have_brought_the_command_and_what_became_of_it` |
| dropping what is missing, fetchers never dropped | `without_absent_moves_each_missing_kept_server_to_the_drops_with_its_reason_in_plan_order`, `a_server_whose_command_is_not_on_the_machine_is_taken_out_and_skipped_with_the_reason`, `a_server_that_runs_through_npx_is_written_even_when_npx_is_not_on_the_machine_yet` |
| opening count | `mcp_opening_counts_every_server_of_each_agent` |
| rows, uv, missing notes, closing words, failure, unread, error | `mcp_rows_writes_every_row_and_the_closing_words_as_the_ts_in_every_scenario`, `a_failed_uv_install_is_named_on_every_server_that_needed_it_and_the_definitions_still_land`, `a_server_whose_command_is_on_the_machine_is_written_and_installed` |
| the round and its failure roads | `apply_mcp_runs_the_whole_round_and_every_failure_road_as_the_ts`, `a_machine_that_refuses_the_script_skips_every_server_touches_no_config_and_does_not_throw`, `running_twice_leaves_the_files_as_after_the_first_run_and_reports_the_same`, `a_format_the_catalog_gains_is_one_module_the_stage_runs_its_editor_with_no_format_of_its_own` |
