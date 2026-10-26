# jsonc (with jsonc_tree, js_regex, mcp_bearer, mcp_travelled)

Source: packages/catalog/src/jsonc.ts (47), mcp-bearer.ts (22), mcp-travelled.ts (17) at a3b870a6e, and the three
reads mcp.ts makes of the `jsonc-parser` package (`parseTree`, `findNodeAtLocation`, `visit` onComment). Tests:
packages/catalog/test/jsonc.test.ts (2). Rust: tests/jsonc.rs (2), tests/oracle_text.rs (reading_jsonc,
writing_json_as_javascript, bearer_names, the travelled test).

## What it does

Reads JSON with comments and trailing commas in one pass that copies spans, writes JSON the way `JSON.stringify` does,
parses jsonc into a tree with offsets for the in-place editor, keeps a sign-in row's regex as data, names the variable
Codex reads a bearer header by, and picks one scope's servers out of a copy that travelled.

## Behaviour

0. `json_parse::parse_json` is `JSON.parse`: the one reader of JSON text in the crate (the other crates that read
   `~/.claude.json` call it, and `read_jsonc` is it after the comments and commas are taken out). A failure carries
   V8's words, including the position and `(line L column C)`; `oracle/json_parse.oracle.mjs` writes 3447 texts with
   node's answer and `tests/json_parse.rs` checks them.
1. `read_jsonc` walks the text once (jsonc.ts:12-42): a `"` skips to the closing quote, a backslash skipping the next
   unit; `//` drops to the newline (kept) and `/*` to after `*/` (an unterminated one drops the rest); a `,` is
   remembered while only whitespace and comments follow, and dropped when a `}` or `]` comes next. The rest is
   `JSON.parse`d; any parse error is an error.
2. A key named twice keeps its first place and its last value (JSON.parse into an object; serde_json's map with
   `preserve_order` does the same on insert).
3. `json_object` is the value where it is an object, else nothing (jsonc.ts:47).
4. `js_stringify` and `js_stringify_pretty` are `JSON.stringify(v)` and `JSON.stringify(v, null, 2)`: numbers as
   JavaScript prints a double (`1.0` → `1`, `1e21` → `1e+21`, `-0` → `0`, `12345678901234567890` →
   `12345678901234567000`, rule 4.25), strings escaped as JavaScript escapes (`\u0007`, not `\a`; U+2028 and `/`
   unescaped), an empty object `{}`, an empty array `[]`.
5. `parse_tree` gives the jsonc-parser node shape: object, array, property (key node then value node), string,
   number, boolean, null, each with offset and length; trailing commas allowed. `find_node_at_location` follows keys
   into objects (first property of that name) and indexes into arrays; `find_member` gives a member's container and
   its index there. `comments_of` lists `//` comments (to the newline, not including it) and `/* */` comments whole.
6. `JsRegex` matches as the JavaScript regex of its source and flags; `is_whole_match` is
   `new RegExp(`^(?:${source})$`)` (signin.ts:180, mcp.ts:915). The `m` flag starts a line after `\r`, U+2028 and
   U+2029 as well as `\n`; the `i` flag folds neither U+017F nor U+212A to ASCII; a pattern fancy-regex refuses
   panics at its first use (every sign-in row's patterns are compiled by a test) and never reads as no match; a
   pattern that opens with a lookbehind of unbounded width (the doppler code) is held as the lookbehind's body and the
   rest, tried at each match.
7. `bearer_of(name)` is the token for `<token>_BEARER` where the token matches `^WSP_MCP_.+_AUTHORIZATION$`; else
   nothing (mcp-bearer.ts:13). `with_bearer_variables` puts each header token's `<name>_BEARER` = `Bearer <value>`
   ahead of every value, a value of the same name keeping the bearer's place and its own value (mcp-bearer.ts:19).
8. `travelled_servers` reads the copy (errors as `read_jsonc` errors), refuses a non-object with "the copy that
   travelled is not a JSON object", and gives the object under `key` at the top, or for a folder under
   `projects[from]`, else `projects[to]`, else `{}`; a key that holds no object gives `{}` (mcp-travelled.ts:11-17).

## Edge cases

- `""` is an error; `"  [ ]  "` is `[]`; `{"a":1,,}` is an error (only the last comma before a bracket is dropped).
- A comma followed by a comment and then `}` is still dropped (`{"a":1, // c\n}` reads `{a:1}`).
- `"WSP_MCP__AUTHORIZATION_BEARER"` has an empty middle and is no bearer name (`.+`).
- jsonc.test.ts reads a 10 MB config in one pass under 5 s: no quadratic string building.

## Behaviour nobody wrote down

- Rule 4.26: JSON object key order is the text's; every map here is an `IndexMap` or a `preserve_order` map.
- Rule 8.6 and 8.7: the TS `/\s/` in the comma tracker is JavaScript's whitespace (U+FEFF included).
- Offsets in the tree are byte offsets into the same text; they never leave the crate, so UTF-16 units are not needed.
- A JavaScript object orders integer-like keys ("1", "42") ahead of the others, ascending. Reads keep the text's order
  (an accepted difference: nothing lists a file's servers by it), but every write does not: `js_stringify` and
  `js_stringify_pretty` list the array-index keys first (`plugins::js_entries`), so a server or an env key named for a
  number is written where `JSON.stringify` writes it.
- A lone surrogate escape (`"\ud83d"`, which `JSON.stringify` writes for a string cut inside an emoji) is read by
  `JSON.parse`, and is read here: `json_parse::parse_json` holds it as a char in U+10F800..U+10FFFF and `js_string`
  writes that char back as its escape, so an edit keeps it byte for byte. A file holding one of those chars itself
  writes it as an escape too.

## Tapes

None directly: config text arrives through agents.read and servers.* ops, whose tapes are #2443's.

## Decisions

1. Numbers that `JSON.parse` reads as infinity (`1e400`) read as `null`, which is what `JSON.stringify` writes for
   infinity (the review found the first draft's error left the whole file unread). Nesting past 1000 levels is
   refused ("Maximum call stack size exceeded") where V8's parser has no limit but memory; serde_json's 128 is gone.
2. `JsRegex` holds `&'static str`: every regex is a catalog literal.

## After the port

- `JSON.parse` (jsonc.ts:41) reads any depth, and the port refuses past 1,000 levels (ruling 2 of #2448): a `serde_json::Value` clones recursively (dies at 1,183 levels) and drops recursively (6,509), so a tree kept deeper could abort the host wherever it is cloned or dropped.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2 | oracle_text: reading_jsonc::takes_out_comments_and_trailing_commas_outside_strings_and_errs_where_json_parse_would, reading_jsonc::a_key_named_twice_keeps_its_first_place_and_its_last_value; jsonc: reading_json_with_comments::drops_comments_and_trailing_commas_outside_strings_and_keeps_everything_inside_t, ::reads_a_config_of_tens_of_megabytes_in_one_pass |
| 3, 8 | oracle_text: the_travelled_copy_gives_the_scopes_servers_or_its_refusal |
| 4 | oracle_text: writing_json_as_javascript::* (3) |
| 5 | through json_edit: oracle_text: editing_json_in_place::every_edit_lands_as_the_ts_editor_lands_it_or_is_refused_with_its_sentence |
| 6 | oracle_signin: a_paste_is_the_token_only_where_the_whole_trimmed_paste_is_its_shape; signin_state: the_token_sign_in_and_the_vault::the_token_in_a_paste_is_the_whole_of_it_and_nothing_around_it_trimmed |
| 7 | oracle_text: bearer_names::* (2); oracle_signin: the_vault_keeps_row_variables_out_of_server_values_and_names_their_rows |
