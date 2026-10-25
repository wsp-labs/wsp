# mcp_toml

Source: packages/catalog/src/mcp.ts:1257-1594, 1737-1757 (about 360 lines) at a3b870a6e. Tests: mcp.test.ts (the
Codex cases), hooks.test.ts (the TOML line helpers through codex-hooks). Rust: tests/mcp.rs, tests/oracle_text.rs
(toml_lines), tests/hooks.rs.

## What it does

Codex's config read and edited line by line, only in the table form Codex documents and `codex mcp add` writes, so
every comment, blank line and other key stays byte for byte.

## Behaviour

1. `uncomment_toml`: a `#` outside a basic or literal string ends the line; a backslash in a basic string skips the
   next character; an unterminated string runs to the end (mcp.ts:1263).
2. `toml_header`: `[mcp_servers.<name>]` or `[mcp_servers.<name>.<sub>]`, the name bare (`[A-Za-z0-9_-]+`), `"..."`
   (no `"` inside) or `'...'`, the sub bare, spaces allowed around dots (mcp.ts:1277).
3. `toml_strings`: every `"..."` (decoded: `\b \t \n \f \r \" \\`, `\uXXXX`, `\UXXXXXXXX`; any other escape kept as
   written) and `'...'` (as written), in order (mcp.ts:1284-1308).
4. `toml_value`: a `[` value is its strings, a `{` value its `key = "string"` pairs (keys bare or quoted), a quoted
   value its first string, anything else nothing (mcp.ts:1316).
5. `read_codex` (mcp.ts:1339): lines split on `\n`; each uncommented and trimmed; a `[` line sets the current table
   (`toml_header` or none); `key = value` with a multi-line array joined while it is open; `enabled = false` in the
   main table marks disabled; main-table values by key, sub-tables `env`, `http_headers`, `env_http_headers` take
   string values. Per table in first-seen order: a non-empty `command` is stdio (args, env = inline `env` then the
   sub-table), else a non-empty `url` is http (headers likewise); `envRefs` = bearer var then env_http_headers values;
   `reads` present only where env_vars, env_http_headers or the bearer is. Scope always `user`.
6. `place_codex` (mcp.ts:1407): header `[mcp_servers.<name>]`, the name JSON-quoted unless bare; lines `command`,
   `args = [...]`, `env = { "k" = "v" }` where any, `tool_timeout_sec = n` where set, or `url`, `http_headers` where
   any; strings written as `JSON.stringify`. A missing or blank file is the table plus a newline; a file without the
   header gets a blank line and the table appended (a newline added first where missing); an existing header's lines
   up to the next `[` line are replaced, one blank line kept before a following header.
7. `codex_owners`: each line belongs to the last `[mcp_servers.<name>...]` header above it (sub-tables included);
   another header ends ownership.
8. `codex_editor`, `codex_merge`, `remove_codex`, `enable_codex`, `only_codex`, `entry_of_codex` follow
   mcp.ts:1484-1594 and 1741-1750: a kept server's lines have their strings rewritten by `rewrite_toml_line` (the
   program on a `command =` line); merge appends after the file's trailing blank lines are trimmed (`[...kept, "",
   ...block, ""]`), replaces wsp's own in place, compares entries as uncommented trimmed non-blank lines; enable
   writes `enabled = false` as the first line under the header or deletes it; "the file names no server called
   <name>" where no main table is found.
9. `rewrite_toml_line`: strings outside the comment rewritten; a literal string written back `'...'` (as is), a basic
   one re-encoded only where its value changed (escapes for `\ " \b \t \n \f \r` and `\uXXXX` for other control
   characters and U+007F); the comment kept as written (mcp.ts:1423).

## Edge cases

- `entry_of_codex` and `only_codex` drop a block's trailing blank lines; `remove_codex` takes them with the block.
- A name quoted in the header reads the same as bare; a sub-table header of an unknown sub is ignored.

## Behaviour nobody wrote down

- Rule 4.26: tables keep file order; headers and env keep line order with inline values before sub-table values
  (mcp.ts:1379-1381 spread order).
- Lines split on `\n` only: a CRLF file keeps its `\r` at the end of each line, inside the trim.

## Tapes

servers.add, servers.remove, servers.toggle on Codex configs (#2443).

## Decisions

1. `TomlValue` replaces the TS `unknown` of `tomlValue`.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | oracle_text: toml_lines::a_comment_ends_a_line_only_outside_quotes |
| 2, 4, 5 | mcp: read::codexs_toml_mcp_servers_tables_with_their_env_header_and_bearer_sub_keys_quoted; mcp_refer: a_definition_s_references_read_back_as_the_agent_starts_the_server::gives_a_codex_command_its_env_vars_that_have_a_value_and_sends_env_http_ |
| 3 | oracle_text: toml_lines::every_string_on_a_line_is_read_basic_ones_decoded |
| 6 | mcp: place_group::codexs_toml_* (4), place_every_transport_a_person_adds::codex_writes_a_commands_variables_and_an_addresss_headers_as_inline_tables_every |
| 7, 8 | mcp: merge::codexs_toml_* (4), remove_group::codexs_toml_the_named_tables_go_with_their_sub_tables_and_the_persons_trust_tabl, enable_group::codex_turns_a_server_off_with_an_enabled_line_in_its_own_table_and_on_by_taking |
| 9 | oracle_text: toml_lines::a_rewrite_touches_only_the_strings_it_changes_and_leaves_the_comment; hooks: codex_s_notify::* |
