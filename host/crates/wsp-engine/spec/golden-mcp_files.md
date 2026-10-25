# mcp_files (golden-mcp.ts:1-335)

Server row ids, the plan off a recipe, the one read of every config off a machine and its parse, the string rewrites,
the edit of every scope, and the landing of what changed. Nothing survives a restart on the host; what lands is the
agents' own config files on the machine, which the next read finds as they stand.

## Ids (:93-115)

- `mcp_row_id(agent, at, name)`: `agents/mcp/<agent>/` then `@<encodeURIComponent(folder)>/` for a folder, `home/`
  for the home, nothing for the user scope, then the name. An empty folder is still a folder (`@/`).
- `scope_row_id(agent, project, folder, name)`: the folder where set (even empty), else home where `project` is set,
  else the user scope.
- `parse_mcp_id(id)`: `None` unless it starts `agents/mcp/` and is not `agents/mcp/mcp-remote`. The rest split on
  `/`: fewer than two parts is `None` (the agent may be empty: `agents/mcp//name` is agent ``). Three or more parts
  with the second starting `@`: folder = `decodeURIComponent` of the rest of it (a bad escape is the TS's `URIError`,
  here `None`), name = the parts after joined `/`. Else `home` when there are exactly three parts and the second is
  `home`, name the third; else the parts after the agent joined `/`.

## The plan (:117-156)

`mcp_plan_for(rows, opts)`, `guest_home` = `opts.guest_home` or `GUEST_HOME`. Per agent of `opts.agents` in its order:
its server rows (`parse_mcp_id(row.id).agent == agent`, a folder row counted with the user scope); none: no plan entry.
Agent row `agents/<agent>` not ticked: every row set aside `{ id, name, reason: "<label> is not ticked, so its config
did not travel" }`, no scopes. Else one scope for the user rows, then one for the home rows (only those that have
rows), each `{ files: source.files, format: source.format, keep, drop }`, the home scope with `project: { from:
opts.home, to: guest_home }`. A ticked row that is not withheld (`consent == true` and `choice != copy`) is kept by
name; the rest dropped with the row's `reason` or `unticked` when unticked, `credential-shaped; not copied without your
answer on its row` when withheld. No agent: `None`. The plan: `rewrites` = `[<home>/ → <guest_home>/, /opt/homebrew/ →
<BREW_PREFIX>/]`; `bin_dirs` = `[<home>/.local/bin/, ~/.local/bin/, ...(opts.bin_dirs or MAC_BIN_DIRS)]`; `tools` = the
tools rows matching `^tools/(brew|go|cargo|npm|pnpm|bun|pipx|uv|hand)/`, `{ id, ticked, reason }`.

## The read (:158-253)

- `READ_MS` 120,000; `MCP_READ_MARK` `wsp-mcp`; `MCP_READ_END` `wsp-mcp end`. `refused(words)` is exit -1, stderr the
  words. `read_reason(res, t)` is `reason_of` with stdout blanked (no line of a config ever reaches a reason), `t`
  defaulting to 120; `read_failed(res)` = `the config edit did not run (<read_reason(res)>)`.
- `read_configs_cmd(scopes)`: the TS's script byte for byte: the `wsp_mcp_read` function (a file is read at its real
  path, only a plain file inside the scope's base; `-plain` otherwise; over `CONFIG_READ_MAX_BYTES` (32 MiB) is
  `-cap`; else the bytes base64 on one line; no file is `- -`), one call per scope `wsp_mcp_read <i> <quoted base>
  <quoted files>`, then `printf '%s\n' 'wsp-mcp end'`. No scopes is the function and the end line alone.
- `read_whole(stdout)`: some line trims to `wsp-mcp end`.
- `parse_configs(stdout, scopes)`: one `None` per scope to start. Each `\n` line JavaScript-trimmed and split on single
  spaces (`a  b` has an empty word); a line counts only when its first word is the mark and it has three or more
  words; the read answered once one does. The scope and file indexes are read with JavaScript `Number` (an empty word
  is 0, `x` is NaN and finds nothing); a line whose indexes name no file sets nothing. `-plain` and `-cap` are unread
  with `CONFIG_NOT_PLAIN` and `CONFIG_PAST_CAP`; anything else is decoded as Node's lenient base64 (an absent word is
  empty bytes; characters outside the alphabet skipped, padding optional), the text as UTF-8 with U+FFFD for bad
  bytes, `sum` = `config_sum(bytes)`. A later line for the same scope wins. No line answered: `None`.

## Rewrites and the edit (:255-312)

- `rewrite_string(plan, s, command)`: a command right under a bin dir (no `/` after it) is the rest of it, the first
  dir in order that fits. Else a leading `--?[\w-]+=` (ASCII word characters) is kept as the head and the rest is
  tried against each rewrite in order; the first prefix that fits is replaced; else `s` unchanged.
- `edit_scopes(plan, agents, read)`: `texts` starts as every read file's text, in read order. Per scope in plan order
  with the next read entry: none is `{ file: None }`; unread is `{ file, unread }`; else the format's `edit(rewrite,
  { keep, drop: names, project }, texts[path])`, its text replacing the file's (two scopes of one file: the second
  edits what the first left), outcome `{ file, results }`; an `Err` is `{ file, error: <message> }`.

## Landing (:314-335)

`land_configs(machine, base, read, texts)`: the files whose text differs from what was read (a file never read
always differs), in `texts`' order, each with a landing `config_landing(path, <12 random hex digits>)`. None: `None`,
no call. Each lands in turn by `land_bytes(machine, landing, utf8, timeout READ_MS)`; one failing sweeps. Then one
exec (20,000): per file `(\n<config_write_line({ file, base: base(path), bytes, from: landing, sum: <read sum when
read> })>\n) || exit $?`, newline-joined; a rejected exec reads as `refused`. Exit 0: `None`. Else sweep with
`config_refusal(res, first path, identity)`, else `reason_of(res, 20)`. The sweep: exec `rm -f <every landing quoted,
space-joined>` (its failure ignored), then `the edited config did not land (<why>)`. Never fails.

## Rules against tests

| Rule | Tests (`golden_mcp_files_hard.rs`, `golden_mcp.rs`) |
|---|---|
| marks and times | `the_read_marks_and_its_time_are_the_ts` (green: constants) |
| row ids, folders encoded, empty folder | `row_ids_spell_the_user_scope_the_home_and_an_encoded_folder` |
| parsing ids: home, folders, mcp-remote, too short, empty agent | `parse_mcp_id_reads_the_agent_scope_and_name_and_nothing_for_a_row_that_is_not_a_server` |
| the plan: keep, drop, withheld, aside, home scope, tools rows, options | `mcp_plan_for_keeps_drops_and_sets_aside_as_the_ts_and_is_nothing_with_no_server_row`, `keeps_ticked_drops_unticked_sets_aside_unticked_agent_and_moves_the_home_scope`, `is_nothing_when_the_recipe_has_no_mcp_rows` |
| refused, reasons with no output | `a_refused_call_its_reason_and_the_failed_read_say_the_ts_words_and_never_a_line_of_output`, `a_read_whose_leader_was_killed_says_so_by_its_exit_code_and_no_reason_is_built_from_its_output` |
| the read script; empty scopes | `read_configs_cmd_is_the_ts_script_byte_for_byte`, `read_whole_finds_the_last_line_trimmed` |
| parsing the read: empty, no mark, unread words, bad indexes, double space, lenient base64, later wins | `parse_configs_reads_each_scope_and_nothing_when_no_line_carried_the_mark`, `a_config_that_is_on_the_machine_and_empty_did_not_parse_it_is_not_one_that_is_not_there` |
| rewrites: bin dirs, flags, prefixes in order | `rewrite_string_names_a_bin_dir_command_and_moves_a_laptop_prefix_past_a_flag`, `writes_kept_definitions_rewritten_drops_unticked_leaves_unknown_and_names_each_on_stage`, `opencode_config_rewrites_the_kept_local_server_drops_the_dropped_one_and_keeps_the_rest` |
| landing: nothing changed, byte road then writes, sweep on each failure | `land_configs_lands_what_changed_then_writes_each_and_sweeps_every_landing_on_a_failure`, `a_write_the_machine_refuses_leaves_no_landing_beside_the_config`, `a_config_with_a_second_hard_link_is_left_as_it_is_and_one_linked_inside_the_home_stays_a_link` |
