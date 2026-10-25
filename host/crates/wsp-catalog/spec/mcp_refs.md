# mcp_refs (with mcp.rs, the types and the format dispatch)

Source: packages/catalog/src/mcp.ts:15-456 (types, the shared reference helpers, `serversByName`), the format tables
at mcp.ts:1208-1255 and 1737-1757, at a3b870a6e. Tests: mcp-refer.test.ts (88) and the format-level cases of
mcp.test.ts. Rust: tests/mcp_refer.rs (88), tests/mcp.rs (44), tests/oracle_text.rs (canonical entries).

## What it does

Declares the one shape every format reads a server into, dispatches each format method to its module, and holds the
pieces both reference writers share: the variable a header travels under, where a value can stand inside a string,
the guards that refuse a copy still holding a value, and `servers_by_name`, which writes a whole file's servers by name
for a copy on another machine and drops the ones that cannot travel.

## Behaviour

1. `McpFormat` methods dispatch: the three JSON formats to mcp_json / mcp_json_refer, Codex to mcp_toml /
   mcp_toml_refer / mcp_codex. `has_enable` is false only for Claude Code's file (mcp.ts:1163); `trust` is Codex's
   alone; `named` and `arg_ref` are `None` for Codex (CODEX_TOML has neither, mcp.ts:1737).
2. `mcp_header_variable(server, header)` = `WSP_MCP_<server>_<header>` upper-cased (`toUpperCase`, full case
   mapping, rule 8.9), then every character outside `A-Z0-9_` replaced by `_` per UTF-16 unit (mcp.ts:181).
3. `value_forms(v)` = `[v]`, plus the rest after a leading `bearer|basic|token` word and whitespace, any case
   (mcp.ts:190).
4. `places(s, at, start)` (mcp.ts:262): nothing for `""`; else the string itself, then for `--flag=v` the places of
   `v` "the --flag argument", for `Name: v` (value not starting `/`) the places of `v` "the Name header", for each
   scheme form the places of the rest, and for an address's query each pair's raw value, then its decoded value
   where `query_value` changes it, "the <key> parameter of an address". Offsets track the string's own characters;
   a decoded value has none.
5. `still_stands(entries, servers)` (mcp.ts:288): every place of every string in the entries is indexed (first place
   wins for a text); each server value, whole or without its scheme word, is looked up as itself, then as base64 of
   its UTF-8 with padding, without, and url-safe; the first found refuses with "<server>'s value still stands in
   <where>[, as base64] after it was written by name, so the file stays on this computer".
6. `known_in` (mcp.ts:317): each place of `s` whose text equals a known value (first in `known` order) and has an
   offset is a hit; hits sorted by start, then longer first; overlapping ones after the first dropped.
   `known_hits` (mcp.ts:341) drops empty known values, puts the server's own names first, and refuses where another
   server sets that name to another value: "<server> passes the value servers.env holds as <name> in <at>, which
   <other> sets to another value, so the file stays on this computer". `known_stands` refuses via `still_stands`.
7. `header_variables` refuses two headers minting one name: "<server> sends <a> and <b>, which would both travel as
   <n>; rename one" (mcp.ts:383). `codex_headers` (mcp.ts:396): a bearer `Authorization` header is read by
   `<name>_BEARER` and kept as the token alone.
8. `oauth_secret_keys`: keys whose lowercase with `-` and `_` removed is clientsecret, refreshtoken, accesstoken or
   idtoken (mcp.ts:219-222). `oauth_secrets_in` walks any depth, arrays by index, and lists dotted paths of secrets
   under an `oauth` key (table or list) that are not `""` and not a reference (mcp.ts:226-240).
9. `readable(outside, writes, name, in_address)`: the name or its bearer token held outside or written by the file,
   or (outside an address) one of HOME USER LOGNAME PATH PWD SHELL TMPDIR LANG (mcp.ts:207).
10. `servers_by_name` (mcp.ts:414-456): refer once with every known name and held name as `outside`; per server in
    order drop with the first reason of: `unread`; a value under a catalog row's variable (`row_variable_line`); a
    value clashing with `held`, with a value kept earlier in this file, or with servers.env's value unless that value
    is this server's alone (`clash_line`); a value crossing lines. A server that passes keeps each of its values not
    already held. Dropped servers are removed (by project where they sit under one); then refer again and drop, in
    rounds, every server that now reads what a dropped one wrote, unkeeping their values. Then `still_stands` over the
    final entries against the first refer's servers, and any OAuth secret in the entries that is not a reference to a
    written name refuses with "a server still carries an OAuth secret in <path less its first part> after it was
    written by name, so the file stays on this computer". Only on success are the kept values put into `held`.
11. A file whose first refer finds no server returns the text as it was and nothing dropped (mcp.ts:438).

## Edge cases

- `clash_line` names the setters joined by " and ", "set" for two or more, "sets" for one, or servers.env's own
  sentence when no server is recorded (mcp.ts:403-406).
- The dropped list carries `{name, reason}` only; the project used for removal is not returned (mcp.ts:455).
- `query_value` keeps the raw text where `decodeURIComponent` throws (`%E0%A4%A`).

## Behaviour nobody wrote down

- Rule 4.26: `servers` keeps the file's table order, projects' tables after the file's own; `held` keeps insertion
  order, which is file order.
- Rule 3.x does not apply: no state outlives a call except `held`, which the caller owns.
- The TS writers are `async` only to load smol-toml lazily; the port is synchronous throughout.

## Tapes

servers.add and the project import's copy of agent configs (#2443 tapes that carry an MCP server with a header).

## Decisions

1. `servers_by_name_with` takes the reference writer as a function so the TS tests' stand-in format
   (`{ ...FORMAT, refer }`, mcp-refer.test.ts:277, 405, 647, 688) ports one for one; `servers_by_name` calls it with
   the format's own `refer`.
2. `RewriteString` is a `&dyn Fn(&str, bool) -> String` for the TS `McpEditLib` object.

## After the port

- None found in this unit.

## Rules to tests

| Rule | Tests (mcp_refer unless named) |
|---|---|
| 1 | mcp: enable_group::claude_code_keeps_no_switch_a_person_turns_per_server_so_its_format_has_none_and, the_catalogs_mcp_configs::are_registered_from_the_agent_entries_that_have_one_each_naming_its_format_modul |
| 2 | the_name_a_header_s_value_travels_under::is_wsp_mcp_server_header_in_capitals_every_other_character_an_underscore |
| 4, 5 | what_a_reference_writer_refuses_rather_than_guess::refuses_the_copy_where_a_value_still_stands_in_a_server_s_entry_whole_or_after_i, ::refuses_the_copy_where_a_value_still_stands_in_an_argument_or_an_address_as_a_fl, ::copies_an_ordinary_config_whose_unrelated_keys_and_history_hold_the_same_short_w |
| 6 | a_value_servers_env_holds_written_back_as_its_reference_on_the_copy::* (16) |
| 7 | what_a_reference_writer_refuses_rather_than_guess::two_headers_of_one_server_that_would_travel_under_one_name, codex_s_reference_writer::refuses_two_headers_that_would_travel_under_one_name_naming_the_headers_as_the_p |
| 8 | an_mcp_server_s_oauth_secrets::* (7), an_oauth_secret_s_reference_read_as_one_the_definition_reads::* |
| 9 | a_server_s_arguments_and_command_by_the_same_rule_as_its_headers::a_name_every_machine_defines_* (7) |
| 10 | a_whole_file_s_servers_by_name::* (3), the four `the_copy_s_last_check_*` tests through `servers_by_name_with` |
| 11 | claude_code_s_reference_writer::names_one_server_alone_when_asked_and_stands_byte_for_byte_where_nothing_holds_a |
