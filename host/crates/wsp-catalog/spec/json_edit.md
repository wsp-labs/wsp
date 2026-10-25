# json_edit

Source: packages/catalog/src/mcp.ts:458-768 (about 310 lines) at a3b870a6e. Tests: reached through mcp.test.ts,
hooks.test.ts and mcp-launch.test.ts. Rust: tests/oracle_text.rs (editing_json_in_place, 25 edit cases and 7
canonical cases from the TS), tests/mcp.rs (edit, merge, remove groups).

## What it does

The one editor every road that changes an agent's JSON config goes through: each edit is made on the text so every
comment and byte it does not touch stays where it was, and an edit whose result would not parse to exactly the edited
value is refused rather than written as a guess.

## Behaviour

1. `edit_json(text, edits)` reads the text (`read_jsonc`), refuses a non-object with "the file is not a JSON object",
   and runs `edit_jsonc` (mcp.ts:724).
2. `edit_jsonc` walks the edits in order; a run of two or more edits that each put a new key into the same object that
   holds none of them goes in with one parse (`newKeys`, mcp.ts:685, then `insertMany`, mcp.ts:670); every other
   edit is `edit_once`. After each step `set_at` applies it to `root`. At the end the text must parse (jsonc) to a
   value whose `json_canonical` equals `root`'s, else "the file could not be changed in place, which keeps its
   comments; change it by hand" (mcp.ts:702-719). `LOST_COMMENT` thrown inside passes through as itself; any other
   error inside becomes the not-in-place refusal.
3. `edit_once` (mcp.ts:625): finds the deepest node of the path that exists. Removing (`None`) a path that does not
   exist returns the text unchanged. Removing a member cuts `member_cuts`. Setting with a missing key in an object, or
   `-1` in an array, inserts the wrapped rest of the path after the container's last member (`inserted`). Anything
   else replaces the node's own text, laid out on lines under the node's indent where the node spanned lines.
4. `member_cuts` (mcp.ts:560): a member sharing its line with other text is cut alone, with the spaces after its
   comma; otherwise the cut runs from the comment lines right above it (a blank line ends them) through its comma and
   the comment ending its line; the last member also takes the comma before it.
5. `inserted` (mcp.ts:597): in an empty container on one line, the entry goes right after the bracket; in an empty one
   laid out on lines, on a new line indented two more than the bracket's line; after a last member that ends its line,
   on a line of its own under that member's indent (a trailing comma kept trailing); after one that shares its line,
   `, entry` on the same line. CRLF text inserts CRLF. A value laid out on lines is `JSON.stringify(v, null, 2)` with
   each newline followed by the indent.
6. `keeps_comments` (mcp.ts:662): every comment outside the span the edit took out must stand, in order and text,
   after it; else `LOST_COMMENT`.
7. `set_at` (mcp.ts:490): own keys only, so `__proto__` is a key; a missing object on the way is made `{}`; a number
   last on an array holder sets, appends (-1) or splices out the element.
8. `json_canonical` (mcp.ts:764): keys sorted in UTF-16 order (rule 8.8) at every depth, written compactly.

## Edge cases (all from the TS, tests/fixtures/text.json)

- Removing `arr[1]` of `[1, 2]` leaves `[1 ]` (the space before the cut comma stays).
- A file with a key twice is refused: the tree finds the first, the parse keeps the last.
- Removing a path whose parent is missing (`missing.x`) is refused: the text stands but `set_at` made `missing: {}`.
- A new nested path in a one-line file is written compact: `"y": {"z":{"w":"deep"}}`.
- Editing a JSON array file is "the file is not a JSON object".

## Behaviour nobody wrote down

- Rule 4.26: inserted keys go after the last member, in the edits' order; nothing is re-sorted.
- Rule 8.8: `json_canonical` sorts by UTF-16 units: `"\u{1F600}"` sorts before `"\u{FFFF}"`, the reverse of byte order.

## Tapes

The servers.add, servers.remove, servers.toggle and plugins.toggle tapes of #2443 write through this editor.

## Decisions

1. Offsets are byte offsets; the TS's are UTF-16 units, but they only ever slice the same text.

## After the port

- Removing a key under a missing parent is refused instead of being a no-op (mcp.ts:636 with set_at's object making).

## Rules to tests

| Rule | Tests |
|---|---|
| 1-7 | oracle_text: editing_json_in_place::every_edit_lands_as_the_ts_editor_lands_it_or_is_refused_with_its_sentence (25 TS cases, each with its whole output text or refusal) |
| 2 (one parse for a run of new keys) | mcp: merge::mcpservers_json_many_servers_merged_at_once_land_in_order_after_the_persons_own |
| 4, 6 | mcp: remove_group::a_remove_takes_out_the_servers_own_lines_its_comment_lines_above_it_and_the_comm, remove_group::a_jsonc_file_keeps_its_comments_when_a_key_comes_out_of_it, edit::every_json_format_edits_a_jsonc_file_in_place_each_comment_outside_what_it_took |
| 5 | mcp: place_group::an_add_goes_on_a_line_of_its_own_after_the_last_server_so_the_comment_at_the_end |
| 2 (refusal) | mcp: place_group::a_file_that_holds_the_servers_key_twice_is_refused_rather_than_edited_into_somet |
| 8 | oracle_text: editing_json_in_place::an_entrys_canonical_text_sorts_keys_by_utf16_at_every_depth; mcp: merge::mcpservers_json_a_keys_shape_is_read_whatever_order_the_agent_wrote_it_back_in_a |
