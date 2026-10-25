# mcp_json_refer

Source: packages/catalog/src/mcp.ts:906-1087 (about 180 lines) at a3b870a6e. Tests: mcp-refer.test.ts (most of its
88). Rust: tests/mcp_refer.rs.

## What it does

The JSON formats' reference writer: every literal header, command variable, OAuth secret and argument default of each
server written in place as the agent's own reference syntax, the values handed back per server for the vault, and the
file refused where a value would still travel.

## Behaviour

1. A format that keeps servers elsewhere refuses the whole file when no single server is asked: OpenCode's
   `mcp.servers` envelope ("the file keeps servers under mcp.servers, whose values wsp does not write by name, so the
   file stays on this computer"), unless that object is empty or reads as one server called servers (a `type` or
   `enabled` that is not an object), mcp.ts:1236-1240, 960.
2. Tables: the file's own (an absent or null key is empty) and, unless `only`, each folder's under `projects`; a
   table or an entry that is not an object refuses with `shape_refusal` of its path (mcp.ts:925-942). `only` limits
   to one name in the file's own table.
3. Before weighing, `writes` is every name the file's literals will travel under, so a reference to a name another
   server of the file writes stands whatever the order (mcp.ts:969).
4. Per server, headers then the env key then OAuth secrets: an empty value is skipped; a whole reference to a
   readable name with no default stands; one with a default to a readable name is rewritten without its default;
   one with no default to an unreadable name is weighed after every server (`lost`); one with a default to an
   unreadable name travels as the default; a value that names a reference inside other text refuses: "<server>'s
   <key> mixes a value with a variable, so it cannot travel by name; make it one or the other". A literal travels as
   the minted header variable (`Bearer ` kept outside for a bearer header) or the env key itself; a literal `""`
   default writes `""` (mcp.ts:979-1006).
5. OAuth: an `oauth` that is neither absent, false nor an object refuses with the entry's shape refusal; a secret that
   is not a string, or a key the format reads no variable in, refuses: "<server> keeps an OAuth secret in <path>,
   and its agent reads no variable there, so the file stays on this computer" (mcp.ts:1008-1024).
6. Arguments, command and address (`arg_strings`): each reference to a readable name stands bare; with no default to
   an unreadable one is weighed later; with a default in an address makes the server `unread`; else the default
   moves under `mcp_header_variable(server, path[.n])`, refusing a name the server already sets (mcp.ts:1026-1055).
7. After weighing, a lost reference whose name is still unreadable makes the server `unread`; an unread server keeps
   its text, travels no values, and is listed with `unread` (mcp.ts:1058-1070).
8. Known values: in each written argument and address of the servers that travel, a known value standing whole is
   replaced by its reference (`spliced`) and recorded in that server's values; `known_stands` then refuses any left
   (mcp.ts:1071-1081).
9. The edits are made once (`edit_jsonc`), then `still_stands` over every definition of the written text
   (mcp.ts:1082-1085). A file with no edit is returned byte for byte.

## Edge cases

- `servers` lists each weighed server with `project` only for a folder's table (mcp.ts:1062).
- A held name referenced with a default in an address is unread; in an argument it travels its default.

## Behaviour nobody wrote down

- Rule 4.26: `servers` is table order, the file's own first; `values` is insertion order: headers, env, secrets,
  argument defaults, known hits.
- The OAuth table's shape and the header-name clash are refused before any value is read (mcp.ts:1008-1013), so a
  server with two faults gets the TS's refusal: `oauth_plan` runs before `put_entries`.

## Tapes

The project import and copy roads that carry a Claude Code, Gemini CLI or OpenCode config (#2443).

## Decisions

1. Synchronous: the TS `async` only matched the Codex writer's lazy import.

## After the port

- None found.

## Rules to tests

| Rule | Tests (mcp_refer) |
|---|---|
| 1 | opencode_s_mcp_servers_envelope::* (3), an_mcp_server_s_oauth_secrets::opencode_a_secret_under_the_mcp_servers_envelope_it_also_reads_keeps_the_file_on |
| 2 | what_a_reference_writer_refuses_rather_than_guess::every_format_a_server_entry_or_server_table_that_is_not_an_object_is_refused_by |
| 3 | a_reference_with_a_default_in_an_address_or_by_name_in_codex::whether_a_reference_names_what_the_file_writes_does_not_rest_on_the_order_of_i |
| 4 | claude_code_s_reference_writer::* (3), the_other_json_formats_write_their_own_syntax::gemini_cli_writes_name_opencode_env_name, a_literal_that_reads_like_a_reference::* (8), what_a_reference_writer_refuses_rather_than_guess::a_value_that_mixes_a_literal_with_a_variable_which_cannot_travel_whole_by_name |
| 5 | an_mcp_server_s_oauth_secrets::refuses_an_oauth_secret_the_agent_reads_no_variable_for_naming_the_key_so_the_fi, ::gemini_cli_writes_name_for_oauth_clientsecret_which_it_expands_and_hands_the_val, ::opencode_writes_env_name_for_oauth_clientsecret_and_for_the_snake_case_client_se |
| 6 | a_server_s_arguments_and_command_by_the_same_rule_as_its_headers::* (20), a_reference_with_a_default_in_an_address_or_by_name_in_codex::a_reference_in_an_address_to_a_name_nobody_holds_drops_its_server |
| 7 | a_literal_that_reads_like_a_reference::*_drops_its_server_* |
| 8 | a_value_servers_env_holds_written_back_as_its_reference_on_the_copy::* |
| 9 | claude_code_s_reference_writer::keeps_every_other_key_and_comment_of_the_file_where_it_was, ::names_one_server_alone_when_asked_and_stands_byte_for_byte_where_nothing_holds_a |
