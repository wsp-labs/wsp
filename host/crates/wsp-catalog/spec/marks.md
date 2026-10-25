# marks

Source: packages/catalog/src/marks/*.ts (about 300 lines over 25 files, mostly svg data) at a3b870a6e. Tests:
marks.test.ts (3), brand-marks.test.ts (5), editor-marks.test.ts (2), provider-marks.test.ts (3). Rust:
tests/marks.rs, brand_marks.rs, editor_marks.rs, provider_marks.rs (13), tests/oracle_data.rs (every mark, and the
server mark of 7 transports).

## What it does

The brand, provider and editor marks drawn inline where a CLI, an MCP server, a cloud or an editor is named; never
fetched. Lookups by tool, by provider, by editor, and a server's mark off its address or its command line.

## Behaviour

1. `brand_marks`: node, github, cloudflare, linear; `provider_marks`: solari then boat, boat alone in a public build
   (marks/index.ts:39); `editor_marks`: the fourteen of marks/index.ts:41 in order (Cursor's is its agent's mark).
2. `editor_mark(id)`: the first mark naming the editor; `provider_mark`; `tool_mark` by a mark's `tools`.
3. `server_mark` (marks/index.ts:72): http by `host_under(host, domain)` over each mark's hosts; stdio by each word of
   the line in order: an `http(s)://` word by its URL host, else the word's package (a trailing `@version` after the
   first character, then a trailing `:tag` with no `/` after it, cut) in a mark's packages.

## Edge cases

- `@scope/name` keeps its leading `@`; `localhost:5000/img` keeps its port (the tag cut stops at a `/`).

## Behaviour nobody wrote down

- Rule 4.26: the mark lists keep the TS order, which the app shows.

## Tapes

agents.read (servers, CLIs) and the places views (#2443).

## Decisions

1. The public build is the crate feature `public-build`, for the bundler's `__WSP_CLOUD__` constant.
2. The svgs are data, copied by script from tests/fixtures/marks.json.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | oracle_data: the_brand_provider_and_editor_marks_are_the_ts_marks_in_order; marks: agent_marks::* (3); brand_marks: brand_marks::every_mark_is_simple_icons_path_at_a_pinned_release_under_cc0, ::a_mark_names_only_catalog_clis_each_under_one_mark, ::every_catalog_cli_has_a_registered_mark_or_takes_the_terminal_glyph_on_purpose_n; editor_marks: * (2); provider_marks: * (3) |
| 2 | editor_marks: editor_marks::every_editor_the_host_opens_has_its_mark_finder_s_among_them_each_editor_under_o; provider_marks: provider_marks::every_provider_a_person_can_add_has_its_mark_one_module_each_found_by_the_provid |
| 3 | oracle_data: a_servers_mark_is_the_one_the_ts_finds_by_host_package_or_address, a_command_words_package_is_the_word_without_its_version_or_tag; brand_marks: brand_marks::a_server_reached_over_an_address_takes_the_mark_of_the_company_that_serves_it_by, ::a_command_server_takes_the_mark_of_the_package_it_runs_or_of_an_address_on_its_l |
