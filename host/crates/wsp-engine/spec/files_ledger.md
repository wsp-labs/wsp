# files_ledger (provision-files.ts:303-398, 512-647, 798-909)

The list beside the job, `place_provision_paths(home).landed`, is what survives a host restart: one line per path or
server key wsp owns in the agents' homes, `<rel or row id>\t<digest that travelled>\t<digest standing>`. Every reader
takes a path's first line. A key line (row id starting `MCP_ID_PREFIX`) carries the server entry's digest in both
fields; a tombstone carries `-` in both. This unit writes the list (the close), reads it (servers, ownership), takes
from it (unland, unmerge), and runs the whole files round.

## The scripts (byte for byte; oracle `landedServersScript`, `landedFilesScript`, `closeFilesScript`, `unlandScript`)

- `landed_servers_script(home)` (:310): no list exits 0 with nothing; per line whose id passes `wsp_key`, prints
  `wsp-server\t<third field>\t<id>`.
- `landed_files_script(home)` (:334): per line whose `<home>/<rel>` is a file whose sha256 is the third field, prints
  `wsp-own\t<rel>`. With `{home}` plus a newline it is `daemon/fixtures/contract/landed-files.sh`.
- `close_files_script(home, once, roots)` (:357): first `rm -f "$log".a[0-9a-f]*` (the markers older roads left; `log`
  and `log.appended` stay), then exit 0 when there is no `landing` file, leaving the list exactly as it was. Otherwise
  build `landed.new`: each landing line, key lines as they are, file lines (by `wsp_dest`) only where a file stands,
  as `<rel>\t<from>\t<sha256 now>`; then each old line except a once path's; then keep the first line per path and drop
  any whose third field is `-` (`awk '!seen[$1]++ && $3 != "-"'`); move it over the list; remove `landed.new`, the
  staging tree and `landing`.
- `unland_script(home, dests, roots)` (:852): no list exits 0. Each list line outside the dests (a `case` of each quoted
  dest and `<dest>/*`) is kept; a line inside with a `..` part is dropped untouched; else, by `wsp_dest` (falling back
  to the copy under the home when the store has none), a dest that resolves to itself (no link on the way), is a file,
  and holds the line's bytes is removed (`wsp-unland\tgone\t<rel>`), anything else stays (`wsp-unland\tkept\t<rel>`);
  every line inside the dests leaves the list either way. Then per dest with a non-empty sub, `find -depth -type d
  -empty -delete` under it, only where it resolves to itself; for a moved store also under the home copy. Never a store
  itself, never an agent's own folder.

## Readers and writers

- `server_digest(entry)` (:515): `None` for `None`; else sha256 hex of the UTF-8 bytes. One rule for the round that
  writes a key line and the leave that reads it back.
- `parse_landed_servers(stdout)` (:519): per `\n` line: the mark first and more than 2 tab fields; id is fields 2..
  joined by tab, digest field 1; a repeated id takes the later digest, keeps its first place.
- `landed_servers(machine, home, say)` (:531): run (deadline `LAND_MS` 300,000); failed or non-zero exit is empty and
  says nothing; else says `provision_list_read_line(count)` once.
- `own_marks(stdout)` (:640): lines with the mark first; path is fields 1.. joined by tab; drop empty, absolute (`/`
  first) and any with a `..` part (`a/..b` is kept).
- `append_landing(machine, home, lines)` (:802): no lines, no call. Else one exec (`INLINE_EXEC_MS`) of `mkdir -p <q
  dir> || exit 1` and `printf '%s\n' <q each line> >> <q landing>`, joined by `\n`; its failure is swallowed.
- `close_agent_files(machine, home, once, stores)` (:907): one run (deadline `LAND_MS`) of the close with
  `store_roots(home, stores)`; failure and exit ignored.
- `unland_files(machine, home, dests, stores)` (:889): no dests is `Some` of two empty lists, no call. Else exec
  (`timeout LAND_MS`); failed or non-zero exit is `None`. Lines split on tab as `mark, how, rest`; `gone` and `kept`
  collect `rest` joined by tab; other `how` words are ignored; both lists sorted.

## The files round: `provision_files` (:814)

Never fails. `say: None` says nothing. With `once = once_paths_of(lands)`, `roots = store_roots(home, stores)`:
1. `pack(Some(leave_out))`. `leave_out(staged)`: `standing_digests(machine, home, the staged dests whose root_of root
   is the home)`; `None` leaves nothing home; else leave home each staged dest that is not once, whose standing digest
   and listed digest both equal its staged digest. Order is the staged order.
2. Say `provision_packed_line(lands count, { bytes, files, stood: stood count })` (files and stood only where the pack
   gave them), e.g. `packed on this computer: 6 paths, 3 files stand there already, 3 files packed, 512 B`.
3. `land_agent_files(home, tar, lands, stood, say, roots)`.
4. Rows: the landing's, then one per pack skip as `{ id: left-out/<i>, label: its path, outcome: skipped, kind: file,
   note }`; skipped: the pack's skips then the landing's.
5. Any failure in 1 to 3: one row per planned landing, `{ id: files/<dest>, label: file_label(...), outcome: failed,
   kind: file, note: the error's first line }`, owned and skipped empty. The list is not touched: the close that follows
   finds no landing and leaves it, so the keys wsp wrote stay its own.

## Servers back out: `unmerge_servers(port, home, stores)` (:586)

Keys come from `port.run(landed_servers_script(home))`. Per MCP agent in catalog order with keys (`parse_mcp_id`):
the agent's own config (`own_server_config`) gets two groups, the user-scope keys (no home, no folder) and the home
keys under project `home`; each folder's keys (first-seen order) go to the file the agent's launch road `carry`s that
folder to, under its folder. Files are keyed by their candidate list; per file in insertion order: `port.read` none is
skipped, unread is `ServersOut::Unread`; else per group, the names whose `entry_of` digest in the current text equals
the key's digest are removed with `remove(text, names, project)`; nothing taken, or the text unchanged, writes nothing;
else `port.write` then `ServersOut::Took { path, names }` in taking order. Any error on a file (read, remove, write)
skips that file; the other files and agents go on. A key whose entry the agent rewrote stays.

## Rules against tests

| Rule | Tests |
|---|---|
| exact scripts; the daemon's twin | `the_list_readers_and_the_close_are_the_ts_text_byte_for_byte`, `the_unland_is_the_ts_text_byte_for_byte_and_never_sweeps_a_store_or_an_agents_own_folder`; files_ledger.rs: landed-files.sh fixture |
| restart: the close keeps first lines, carries key lines, leaves the list when nothing landed, sweeps markers | files_land.rs: "closes a round that landed only keys ...", "leaves the list as it was ...", "sweeps the markers ..." |
| a failed pack: every path failed with the reason, nothing owned, the list untouched | files_land.rs: "reads a round whose files never left this computer ..." |
| a round whose copy never travelled takes no key out | files_land.rs: "takes no key out of the list ..." |
| stage lines in order: packed, shipped, landed, list read | files_land.rs: "says what it packed, what went over and what the landing walked ..." |
| unland: wsp's copy goes, an edited file and a link stay, no climb, the row's lines go, emptied folder goes | files_ledger.rs: the two unland tests; `no_dests_asks_the_computer_nothing_and_takes_nothing`, `an_unland_that_did_not_run_answers_nothing_rather_than_an_empty_take`, `the_unland_answer_is_sorted_whatever_order_the_file_system_listed` |
| unmerge: both scopes, both formats, jsonc comments kept, nothing written where nothing is wsp's | files_ledger.rs: the three unmerge tests |
| parsers and digest | `the_key_lines_need_three_fields_keep_tabs_in_an_id_and_let_a_later_line_win_in_the_first_place`, `the_ownership_marks_drop_empty_absolute_and_climbing_paths`, `a_server_digest_is_the_sha256_of_the_entrys_utf8_and_none_for_no_entry`, `a_leave_line_names_what_came_out_or_why_the_file_was_left_unread` |
