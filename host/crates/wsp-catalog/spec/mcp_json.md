# mcp_json

Source: packages/catalog/src/mcp.ts:470-904, 1089-1255 less the referrer (about 420 lines) at a3b870a6e. Tests:
mcp.test.ts (44, most of them), turn-servers.test.ts, mcp-launch.test.ts. Rust: tests/mcp.rs, tests/turn_servers.rs,
tests/mcp_launch.rs, tests/skills.rs (the disabled read).

## What it does

The three JSON formats' reads and edits, all in place through json_edit: Claude Code's `mcpServers` (with per-folder
`projects`), Gemini CLI's settings (`httpUrl`, `$X`, `mcp.excluded`) and OpenCode's `mcp` (`{env:X}`, `enabled`).

## Behaviour

1. `read` (mcp.ts:1118): text that does not parse or is not an object defines none. Servers under the key in key
   order; an entry with neither a command nor a url (or not an object) is none. mcpServers: a non-empty string
   `command` is stdio with `args` (strings only), `env` (string values only), `cwd` where a non-empty string; else
   `httpUrl` then `url` is http with `headers` (strings only) and `envRefs` the variables its url and header values
   name (mcp.ts:1189-1201). OpenCode: `type: "remote"` with a url is http, envRefs from header values only; else the
   first of `command` (an array) is the command, the rest args, `environment` its env; `enabled: false` reads
   disabled (mcp.ts:1241-1252). Gemini: a name in `mcp.excluded` reads disabled. A format with `projects` reads
   `projects[home]`'s servers after the file's own, scope `home`. An OAuth secret's references are added to envRefs
   for formats that expand them there (mcp.ts:736-741).
2. `place` (mcp.ts:1131): a missing or blank file becomes `{"<key>": {"<name>": entry}}` pretty plus a newline; else
   the entry is set at `[key, name]` where the key holds an object, else `[key]` is set to `{name: entry}`.
   Entries: mcpServers stdio `{command, args, env?}`, http `{type: "http", url, headers?}` for Claude Code and
   `{httpUrl, headers?}` for Gemini; OpenCode stdio `{type: "local", command: [command, ...args], enabled: true,
   environment?}`, http `{type: "remote", url, enabled: true, headers?}`; an empty env or headers is left out
   (mcp.ts:462, 1203, 1253).
3. `edit` (mcp.ts:850): kept servers' strings rewritten in place by `lib` (`command` keys and a command array's first
   element are the program), a folder's servers moved from `projects[from]` to `projects[to]`, dropped ones removed;
   a kept name missing at the source is `written` where the destination already holds it, else `missing`; every
   dropped name is reported `dropped` whether or not it was there.
4. `merge` (mcp.ts:773): definitions from `travelled_servers`; per kept name: `missing` if the copy lacks it; `same`
   where the agent's entry is canonically equal to the rewritten one; `theirs` where it differs and is not in
   `replace`; else written (`added` or `replaced`). Per dropped name: `left` unless it stands and is in `replace`,
   then `dropped`. No edits gives the own text as it was (`""` where none). With no own file the result is the
   edits applied to `{}`, pretty plus a newline.
5. `remove` (mcp.ts:819): only names the table holds are taken; none taken gives the text unchanged; the table key
   itself stays even when emptied; a Gemini name in `mcp.excluded` is flipped back on as it goes.
6. `enable` (mcp.ts:1169): "the file names no server called <name>" where the entry is missing; OpenCode sets
   `<entry>.enabled`; Gemini appends to `mcp.excluded` (making it `[name]` where it is not an array) to turn off and
   removes every occurrence, last first, to turn on; Claude Code has none.
7. `only` (mcp.ts:1138): the named servers that exist, from the file's own table or `projects[from]`, written pretty
   plus a newline under `{key: ...}` or `{projects: {to: {key: ...}}}`.
8. `entry_of` (mcp.ts:1145): the canonical text of the entry, nothing for unparseable text or a missing name.
9. `resolve` (mcp.ts:1093): each reference in stdio env values and in an http url and header values replaced by its
   value (pushed to `values` in order of appearance), else its default `${X:-d}`, else left and the first such name
   is `Missing`. Arguments and the command are not touched.

## Edge cases

- The refs regexes: Claude `\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^}]*)?\}`; Gemini also bare `$X`; OpenCode
  `\{env:([A-Za-z_][A-Za-z0-9_]*)\}` (mcp.ts:1208, 1212, 1223).
- A file with the servers key twice is refused by the in-place check, never edited into a guess (mcp.test.ts).

## Behaviour nobody wrote down

- Rule 4.26: a placed entry's keys are in the order listed in rule 2; a merged new file keeps the edits' order.
- Every write goes through json_edit, so a comment outside what moves stands or the write is refused.

## Tapes

servers.add, servers.remove, servers.toggle on a computer's agent configs (#2443).

## Decisions

1. Functions take the format and panic on Codex's; the dispatch in mcp.rs never sends Codex here.

## After the port

- None found.

## Rules to tests

| Rule | Tests (mcp unless named) |
|---|---|
| 1 | read::* (5); skills: a_server_its_file_switches_off::reads_opencode_s_enabled_false_and_codex_s_enabled_false_as_off_and_every_other; turn_servers: a_claude_code_turns_servers::* |
| 2 | place_group::mcpservers_json_claude_code_gemini_cli_a_fresh_file_holds_the_server_an_existing, ::opencodes_json_the_server_is_a_local_mcp_entry_whose_command_is_one_array_enable, place_every_transport_a_person_adds::* (5) |
| 3 | edit::every_json_format_edits_a_jsonc_file_in_place_each_comment_outside_what_it_took |
| 4 | merge::mcpservers_json_* (6), merge::every_json_format_merges_into_a_jsonc_file_in_place_each_comment_it_did_not_take |
| 5 | remove_group::mcpservers_json_the_named_keys_go_under_the_user_scope_and_the_home_folder_alike, ::opencodes_json_the_named_keys_go_from_under_its_own_key_every_other_byte_of_the, ::gemini_cli_takes_a_removed_servers_name_out_of_mcp_excluded_too_so_a_later_serve |
| 6 | enable_group::opencode_turns_a_server_off_and_on_by_its_enabled_field_and_the_read_says_so, ::gemini_cli_turns_a_server_off_by_its_name_in_mcp_excluded_and_on_by_taking_it_ou, ::opencode_and_gemini_cli_flip_a_switch_in_place_so_a_jsonc_file_keeps_its_comment, remove_group::gemini_cli_turns_a_server_on_and_takes_a_removed_one_out_of_mcp_excluded_by_that |
| 7 | turn_servers: what_a_projects_servers_travel_in::is_a_copy_of_the_file_holding_the_named_servers_alone_moved_under_the_folder_the |
| 8 | oracle_text: editing_json_in_place::an_entrys_canonical_text_sorts_keys_by_utf16_at_every_depth; mcp_launch: a_claude_code_launchs_servers::* |
| 9 | mcp_refer: a_definition_s_references_read_back_as_the_agent_starts_the_server::fills_each_format_s_own_syntax_in_headers_command_variables_and_the_addr, ::takes_a_written_default_for_a_variable_with_no_value_and_names_the_first |
