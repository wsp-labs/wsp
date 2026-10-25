# mcp_toml_refer (with mcp_codex)

Source: packages/catalog/src/mcp.ts:1596-1735 (140 lines) and mcp-codex.ts (83) at a3b870a6e. Tests: the Codex cases
of mcp-refer.test.ts, turn-servers.test.ts and mcp-launch.test.ts. Rust: tests/mcp_refer.rs (codex_s_reference_writer,
the Codex refusals), tests/turn_servers.rs, tests/mcp_launch.rs.

## What it does

Codex's reference writer and Codex's own reads: the file is read whole by a TOML parser so every spelling of a
server's headers and variables reads the same; values move to `env_http_headers` and `env_vars`; the line editor's
text is kept where it parses to the same tree, else the tree is written out. Also the resolve Codex does at start
and its trust table for a folder.

## Behaviour

1. `refer_codex` (mcp.ts:1661): a parse error refuses "the file is not valid TOML (<first line of the parser's
   message>)"; no `mcp_servers` gives the text unchanged with no servers; `mcp_servers` that is not a table, a server
   that is not a table, `http_headers` or `env` that is not a table of strings, or `oauth` that is not a table
   refuses with `shape_refusal`. A server's OAuth secret refuses: "<name> keeps an OAuth secret in
   mcp_servers.<name>.oauth.<key>, and Codex reads no variable there, so the file stays on this computer".
2. `writes` = every kept header name and env key of the asked servers; a `bearer_token_env_var` or an
   `env_http_headers` value naming an unreadable variable makes the server `unread` with no values
   (mcp.ts:1688-1693).
3. A server with headers or env: each header travels as `codex_headers` says (a bearer `Authorization` read by the
   `_BEARER` name, kept as the token alone), each non-empty env value under its key; the server's new table drops
   `http_headers` and `env`, merges into `env_http_headers`, and appends new keys to `env_vars` (existing first).
4. Arguments, command and address are checked for known values: a hit refuses "<name> passes the value of <var> in
   <at>, and Codex reads no variable there, so the file stays on this computer"; then `known_stands`.
5. Nothing changed gives the text as it was. Else the line editor's text (`refer_codex_lines`) is kept where its
   parse is canonically the wanted tree; otherwise (or where the lines throw) the wanted tree is written out by the
   TOML writer. `still_stands` runs over the written file's servers.
6. `refer_codex_lines` (mcp.ts:1606): per server read by `read_codex` with headers or env: header and env lines
   (inline or sub-tables) go; `env_http_headers` is extended inline, or its sub-table gets lines after its last
   non-blank line, or a new inline line goes under the header; `env_vars` is rewritten with existing names first,
   or added under the header.
7. `resolve_codex` (mcp-codex.ts:12): no `reads` gives the transport and no values; stdio sets each env var that has a
   value; http sends each env_http_headers header and then the bearer (`Bearer <v>` as `Authorization`), and the
   first one with no value is `Missing`.
8. `codex_trusts` reads `projects[folder].trust_level == "trusted"`; false where the text does not parse.
   `trust_codex` (mcp-codex.ts:57): trusted already gives the text unchanged; a `[projects."<folder>"]` table gets
   `trust_level = "trusted"` set or inserted first; else the table is appended after a blank line (none for an empty
   file); a folder named in another shape refuses "the trust of <folder> is written in a shape wsp does not edit"; a
   result that does not parse refuses "the trust of <folder> cannot be added to a file that writes its projects in a
   shape wsp does not edit".

## Edge cases

- Seven TOML spellings of one server (inline, bare under `[mcp_servers]`, dotted, quoted sub-table, multi-line
  string) read as one tree (mcp-refer.test.ts, "every TOML spelling").
- `[[mcp_servers.x]]` (an array of tables) is a shape refusal at its path.

## Behaviour nobody wrote down

- Rule 4.26: the tree keeps key order (the `toml` crate with `preserve_order`); appended `env_vars` keep first-seen
  order.

## Tapes

Project import and copy roads carrying a Codex config (#2443).

## Decisions

1. The TS writes the fallback with smol-toml's `stringify`; the port uses the `toml` crate's writer. The two differ
   in spacing and quoting; no test pins the fallback's bytes (the TS tests parse it back), and this is an accepted
   difference for the harness, class `toml-layout`. A value type is not layout: JavaScript cannot tell `20.0` from
   `20`, so the tree the parser gives holds a whole number as an integer and the fallback writes `20`.
3. The file is read by `smol_toml::parse_toml`, a port of smol-toml 1.9.0's parse (TOML 1.1: multi-line inline
   tables, a trailing comma in one, `\e` and `\xHH`, a time without seconds; an integer past 2^53 refused), not the
   `toml` crate, so a file reads as the same tree or fails with the same message (the first line in
   `refer_codex`'s "the file is not valid TOML (...)", the whole message with its code block where the TS lets it
   through). `oracle/toml_parse.oracle.mjs` writes 5329 texts with smol-toml's answer; `tests/toml_parse.rs` checks
   them. Differences: nesting is refused past 256 levels (smol-toml 1000), and V8's lenient legacy date reader is not
   ported (a date shape TOML has reads as it does).
4. `decode_toml` throws as `String.fromCodePoint` throws on a `\u` or `\U` without its digits ("Invalid code point
   NaN") or past U+10FFFF; `read_codex` reads that as no server, the editors, `refer` and the Codex hooks carry the
   error.
2. Synchronous: smol-toml was loaded lazily only to save the host's memory.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | mcp_refer: what_a_reference_writer_refuses_rather_than_guess::codex_refuses_a_server_shape_it_cannot_map_so_the_file_stays_on_this_computer_ra, ::codex_reads_no_variable_for_an_oauth_client_secret_so_the_file_stays_on_this_com, a_whole_file_s_servers_by_name::throws_the_format_s_own_words_where_a_file_is_not_the_format_so_a_caller_never_c |
| 2 | mcp_refer: a_reference_with_a_default_in_an_address_or_by_name_in_codex::codex_a_bearer_or_header_variable_by_name_that_nobody_holds_drops_its_server_o, codex_s_reference_writer::reads_a_name_another_server_s_bearer_header_is_read_by_as_held_the_token_alone_k |
| 3, 6 | mcp_refer: codex_s_reference_writer::writes_env_http_headers_and_env_vars_in_place_of_the_values_every_other_line_as, ::adds_to_the_names_a_table_already_reads_rather_than_writing_a_key_twice, what_a_reference_writer_refuses_rather_than_guess::codex_every_toml_spelling_of_a_server_s_headers_and_variables_reads_as_one_tree |
| 4 | mcp_refer: a_server_s_arguments_and_command_by_the_same_rule_as_its_headers::codex_a_value_servers_env_holds_in_the_command_refuses_the_copy_since_code, a_value_servers_env_holds_written_back_as_its_reference_on_the_copy::codex_reads_no_variable_in_an_argument_or_an_address_so_the_copy_refuse |
| 7 | mcp_refer: a_definition_s_references_read_back_as_the_agent_starts_the_server::gives_a_codex_command_its_env_vars_that_have_a_value_and_sends_env_http_ |
| 8 | turn_servers: what_a_projects_servers_travel_in::marks_a_folder_trusted_in_codexs_own_file_in_place_where_its_table_is_there_and; mcp_launch: a_codex_thread_starts_config::reads_a_folders_trust_however_the_toml_spells_it_a_dotted_key_and_an_inline_tabl |
