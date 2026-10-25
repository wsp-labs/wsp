# provision_mcp (provision-mcp.ts:1-378)

The recipe's MCP servers merged key by key into the files the agents keep their own servers in, on a computer somebody
owns, and written down one line per key in the list beside the job. What survives a restart is those key lines (and
tombstones) in `place_provision_paths(home).landed`, folded in by the close (files_ledger.md); the next run reads
them back through `landed_servers` to tell wsp's entries from the agent's and the person's.

## Words (:61-100), byte for byte (oracle `mcpLines`)

`their_server_line`: `<name> in <path> is <agent>'s own under that name; wsp does not write over it`.
`key_unreached_line`: `its key in <names joined by ", "> does not reach <agent>'s threads on that computer yet`.
`untrusted_line`: `<agent> reads them only in a folder it trusts, and <folder> could not be marked trusted (<why>)`.
`no_copy_line`: `nothing of this computer's copy of <path> arrived this run`. `server_line`: `<id>\t<d>\t<d>`;
`tombstone_line`: `<id>\t-\t-`. `travelled_path(home, file)`: `<staging>/<file below home/>`, `None` unless the file
starts with `home/` (the home itself is `None`).

## at_home (:45)

The same plan (unchanged) when `home` is its `guest_home`. Else `moved(p)`: `p == guest_home` is `home`; `p` under
`guest_home/` keeps its tail under `home`; anything else stays. Applied to each rewrite's target with its trailing
`/` cut first and put back after, every scope's files, and each project's `to` (never `from`). `guest_home` becomes
`home`; `bin_dirs` and `tools` stay. A lookalike (`/rootx`) never moves.

## headless_keys (:127)

Per MCP agent in catalog order with headless keys and a store: exec (`INLINE_EXEC_MS`) `[ "$(uname -s)" = Linux ] &&
mkdir -p <q store>` for an agent the setup puts there, `[ -d <q store> ]` for any other; a failed exec or non-zero
exit skips it. Read its own config (`own_server_config(agent, home, store)`) through `machine_server_port(...).read`:
unread is `<agent name>: config_unread_line(path, why)`; else `unset_toml_keys(text or "", keys)`; none missing skips;
else write `<key> = <value>\n` per missing key, then the old text, by `land_configs(machine, base, [file] or [],
{ path or files[0]: text })`; its words become `<agent name>: <words>`. Answers those lines in order.

## provision_mcp (:158), step by step

1. `plan = at_home(planned, home)`; stage `("installing-mcp", mcp_opening(plan.agents))`.
2. Per scope in plan order (agents then scopes): `config` is the scope's `own`, else, where the agent is an MCP agent and
   `stores` has it, `own_server_config(agent, home, store)`. `bases`: each config file to its base.
3. Candidates: a scope with a `travelled` text reads its own file alone (`config.files` or the scope's files, no
   travelled path); any other scope pairs each file that has a `travelled_path` with own `config.files[0]` or the file.
4. One read, run unlogged with deadline `READ_MS`: the own candidates of every scope (base `bases[first own]` or home),
   then the travelled candidates of every scope (base home). A rejected run is `refused(message)`. Exit 0 parses;
   anything else is no read and `failure = read_failed(res)`.
5. `own[i]` is the read's i-th; the copy is the scope's `travelled` text (path "") or the read's `scopes + i`-th.
   `travelled[i]` is a copy that was read; `unread[i]` is the first of own and copy that came back unread.
   `target[i]`: own's path, else the first candidate's own (handed-in copy), else the candidate whose travelled path is
   the copy's. `stood`: target to the own text where read.
6. `owned = landed_servers(...)`, its line staged as `installing-mcp`. `arrived_whole(path)`: `landed[path] ==
   Installed`.
7. `merge_all(agents)`, per scope in order, each into the text earlier scopes of the same file left:
   - no target: outcome `{ file: None }`; unread: `{ file, unread }`; both write nothing and plan nothing.
   - `where[id] = target` for every keep and drop name; `standing` = this pass's text for the target, else `stood`.
   - no copy arrived: keep names read `same` when `entry_of(standing)` has a digest, else `missing` (`no_copy[id]`);
     drops read `left`. Nothing is written or planned.
   - else `replace` is every name when the file arrived whole, else the names whose list digest equals the standing
     entry's; merge (`{ text: standing or "" }` when there are no names), then `trust` when the scope and format both
     have one, then `edit` (handed `None` for empty text). A text other than `standing or ""` is kept for the file. All
     names are planned. Results: `same` adds to `same`, `theirs` to `theirs[id] = path`, added/replaced/same to
     `wrote`. A failure is `{ file, error }`; with `trust` it also records `untrusted[agent \0 trust] = message`.
   - `records`: each wrote id to `server_digest(entry_of(final text of its file))`, absent where none; `tombstones`:
     planned ids with no record, in planned order.
8. With a read: first pass; then per scope, keep names move to drops with a reason, first match wins: folder untrusted
   (`untrusted_line`), theirs (`their_server_line`), no copy (`no_copy_line`). Then `missing = absent_commands(...)`,
   `agents = without_absent(...)`, second pass, `failure = land_configs(machine, base of path or home, own, texts)`.
9. `present`: with no failure and a second pass, the `same` ids whose `where` file did not arrive whole.
10. `rows = mcp_rows(machine, plan, { agents, missing, present, stage, report: second pass outcomes, failure })`.
11. `unreached` (no failure, a second pass, `held` given and non-empty): per kept name of each scope, the server read
    from the final text (`format.read(text, project.to or home)`) whose name matches and whose scope is `home` exactly
    when the scope has a project; not disabled; `launch_misses(agent, format, server, held)` non-empty sets
    `key_unreached_line(label, misses)`.
12. `into` (given, with a second pass): every distinct `where` file in first-seen order, each with the names of its
    recorded ids (none on a failed round), called once per file.
13. No failure and a second pass: `append_landing` of every record as `server_line` (records order) then every
    tombstone.
14. Rows, in mcp_rows' order: outcome `skipped` stays skipped; `present` ids read `present`; everything else
    `installed` (`fetched-on-first-use` included). Note: mcp_rows' note unless present, then the unreached line unless
    skipped, joined by `; `, absent when empty. `label` `<agent> <name>`, `kind` `server`, `keys` where mcp_rows gave
    them.

## Rules against tests (tests/provision_mcp.rs)

| Rule | Tests |
|---|---|
| a server lands as the copy carries it, keys kept, values still names | `lands_each_server_as_the_copy_carries_it_every_key_kept_and_every_value_still_a_name` |
| restart: the next run reads its own keys present, rewrites only where the copy changed | `merges_into_the_agents_file_reads_present_next_run_and_rewrites_only_where_the_copy_changed` |
| an agent's rewrite stays and is named; an absent command is skipped | `leaves_a_server_the_agent_rewrote_since_and_skips_one_whose_command_is_not_there` |
| no read through a link or past the cap; a link inside the home is merged through | `reads_no_config_through_a_link_or_past_the_cap_and_writes_neither`, `merges_into_a_config_linked_inside_the_home_and_the_leave_takes_its_servers_back_out`, `the_leave_takes_nothing_out_of_a_config_that_became_a_link_or_grew_past_the_cap` |
| tombstones for an unticked server | `writes_down_the_keys_it_no_longer_owns_so_the_list_stops_naming_an_unticked_server` |
| home-folder servers, held keys not reached, jsonc comments | `reads_a_home_folders_server_in_a_copy_whose_folder_keys_the_pack_already_moved`, `says_on_a_servers_row_where_its_key_does_not_reach_that_computers_threads`, `merges_into_a_jsonc_config_in_place_so_the_persons_comments_stand` |
| stores: merge outside the home, headless keys once, the merged-into note, files under the store, moves and leaves | `merges_an_agents_servers_into_the_store_config_outside_the_home_and_takes_them_out_on_a_leave`, `writes_the_key_codex_keeps_a_servers_sign_in_by_into_the_store_config_once`, `says_on_the_row_of_a_store_config_that_stood_which_of_this_computers_servers_merged_into_it`, `lands_the_rest_of_an_agents_own_files_under_the_store_and_none_under_the_home`, `moves_the_copies_an_earlier_run_left_under_the_home_into_the_store`, `takes_a_dropped_skill_off_under_the_home_on_a_box_whose_claude_reads_a_store`, `takes_the_copies_left_under_the_home_off_with_a_codex_dropped_before_any_run_moved_them`, `moves_an_earlier_copy_into_the_store_without_taking_the_agents_own_folder`, `takes_every_file_landed_in_a_store_off_by_the_agents_folder_and_leaves_the_store` |
| a config on no computer is the merge's to skip | `leaves_a_config_on_no_computer_to_the_merge_which_skips_its_servers` |
| a round whose files never left reads present and claims nothing | files_land.rs: "reads a round whose files never left this computer ..." |
| words and at_home | provision_pins.rs: `the_servers_words_are_the_ts_sentences`, `a_plan_moves_with_the_home_every_path_under_it_and_nothing_beside_it`, `a_file_travels_only_from_under_the_home` |

Unclear in the TS: step 11 reads a home-scoped server for a scope with a project of any folder (`scope === "home"`
against `project !== undefined`), not that project's own scope; ported as written.
