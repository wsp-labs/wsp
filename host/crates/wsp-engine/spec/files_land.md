# files_land (provision-files.ts:1-301, 400-510)

The person's agent files landed in the agents' homes on a computer somebody owns. A file already there with other
content is never written over; a file the list beside the job says wsp left, and that nobody changed since, is
replaced when this computer's copy changed. What survives a host restart is that list
(`place_provision_paths(home).landed`, written by the close in files_ledger.md): this unit only reads it.

## Which files may land (:60-79)

- `covers(dest, rel)`: `rel == dest` or `rel` starts with `dest + "/"`. Never a lookalike (`.codexx` is not under
  `.codex`). The one rule for every reader here.
- `agent_paths(agent)` (:65): `~/<stateHome>`, `skills_dir_of(agent)`, each config path, each MCP file, in that
  order; only those starting `~/`; trailing slashes cut.
- `agent_state_file(id, source, home)` (:74): the agent is `agent_of_row(id)`, else `parse_mcp_id(id)`'s agent; not in
  the catalog (`catalog_agents()`) is false; true when one of its paths, as `<home>/<path without ~/>`, covers
  `source`. A dotfile, a login's store, another agent's path, `agents/mcp/mcp-remote` and an unknown agent are false.

## Stores (:113-153)

- `store_roots(home, stores)` (:118): per catalog agent in catalog order with a store, `(rewritten_rel(stateHome),
  store)`, except a store equal to `<home>/<that rel>` (Claude Code's own folder moves nothing). Empty stores: empty.
- `root_of(home, roots, rel)` (:127): the first root whose folder is `rel` or holds it gives `{ root: store, sub: rel
  after the folder and its slash ("" for the folder itself), moved: true }`; else `{ root: home, sub: rel, moved:
  false }`. `landed_at` is `root` for an empty `sub`, else `root/sub`.
- `once_paths_of(lands)`: the dests marked once, in plan order. `lands_once(rel, once)`: some once dest covers `rel`.
- Shell helpers, exact lines (oracle): `key_test()` (`wsp_key`: true for an id starting with the quoted
  `MCP_ID_PREFIX`), `once_test(once)` (`wsp_once`: a `case` of each quoted dest and `<dest>/*`, joined by `|`; no case
  line at all for no dests), `dest_test(roots)` (`wsp_dest`: one case arm per root setting `root`, `sub`, `top`, then
  the home arm; then `dest="$root/$sub"`).

## What already stands there (:164-243)

- `standing_script(home)` (:184): the oracle's text. It writes the paths from stdin to `asked`, hashes them all with one
  `xargs -0 sha256sum -z --`, prints `wsp-stand\t<digest>\t<path>\0` per path that is there, then the list's first line
  per asked path as `wsp-ledger\t<path>\t<third field>\0`, and removes `asked`. A missing path prints nothing.
- `parse_standing(stdout)` (:219): split on NUL, then tabs; fewer than 3 fields is no record. `wsp-stand`: `at[fields
  2.. joined by tab] = field 1` (a path keeps its tabs). `wsp-ledger`: `listed[field 1] = field 2`. A later record for
  a key overwrites its value and keeps its first place.
- `standing_digests(machine, home, paths)` (:237): no paths is `None` with no call. Exec the script with timeout
  `STAND_MS` (120,000) and stdin each path followed by NUL. `None` where the exec failed, exited `EXEC_DEADLINE_EXIT`
  (124) or `NO_SHELL_EXIT` (127), or both maps came back empty. Any other exit is read. The script ends `exit 0`: a missing path prints nothing and is not an exit of its own (no 123 leaves it); only its three `|| exit 1` lines (mkdir, cat, cd) exit non-zero.
  No path appears in the command text.

## The landing (:250-301, :400-510)

`land_files_script(home, once, roots)` (:250), the oracle's text. Per file under the staging tree (`find . -type f`):
`installed` where nothing is at the dest or an empty file is (a workspace's mount point); else `present` for a once
path; else `present` where the bytes match; else, where the list's first line for the path names the dest's current
bytes as what wsp left, `present` when this computer's copy is the one that travelled then, `installed` otherwise;
else `kept`. `installed` copies with `cp -p` (failure is `failed`). A moved store removes the copy an earlier run left
under the home when it still holds the bytes the list says, then empty folders up to `top`. Each path prints
`wsp-land\t<act>\t<digest>\t<rel>`; each installed or present path that is not once appends `<rel>\t<digest>` to
`landing`. Paths go to awk through `ENVIRON`, so a backslash in a path is read as itself.

- `parse_landed(stdout)` (:401): per line in order: the mark first, at least 4 tab fields, a known outcome; the path is
  fields 3.. joined by tab (it may be empty).
- `owner_of(rel, lands)` (:412): the longest covering dest; the first of equal length wins.
- `files_rows(lands, landed, home, roots)` (:442): one row per planned entry in plan order, over the landed paths it
  owns; then one row per landed path no entry covers, each alone, in landing order, labelled by its path alone.
  Per row: `failed` when any path failed (note `could not be written: <list>`), else `installed` when any installed,
  else `skipped` when none travelled (note `nothing of it travelled`) or all are kept, else `present`. The note
  otherwise: for a once entry that reads present, `already there before this run, so it stands as it was`; else the
  parts joined by `; `: `<i> of <n> files` when more than one path and any installed, then the kept part (`already
  there with other content: <list>` when all are kept, `<k> already there with other content: <list>` otherwise). An
  empty note is no note. `<list>` names 3 paths, then ` and <n> more`. `kind` is `file`; the label is `<label>
  <landed_at>` (files_land rows of a moved store read the store's path).
- `land_agent_files(machine, o)` (:490): exec `rm -rf <q staging>` (`INLINE_EXEC_MS`; its failure fails the call);
  `import_into(tar, staging, overlay, timeout LAND_MS, tmp_dir: place_provision_paths(home).dir, on_part: say
  provision_shipped_line)` (never the shared `/tmp`); run the landing with deadline `LAND_MS` (300,000); exit not 0:
  `the agents' files did not land on <machine id> (exit <n>): <last 300 of stderr>`. Say `provision_landed_line(walked
  count)`. The `stood` paths join the walked ones as `present`. `owned`: each installed or present path at its
  `landed_at`, in order (a repeat keeps its place, takes the later value). `skipped`: each kept path as `{ id: its
  owner's id or files/<rel>, path: landed_at, note: "already there with other content; wsp did not write over it" }`.

## Pinned since (F-rules, values from running the TS)

- F1. Roads and bounds: `standing_digests` is an exec under `STAND_MS` 120,000 with the paths on stdin
  (`<p>\0` each, duplicates kept); the staging sweep an exec under `INLINE_EXEC_MS` 20,000; the import a run under
  `LAND_MS`; the landing a run under `LAND_MS` 300,000, `unlogged` false. The box guest runs exec and run alike, so
  only a recording machine tells them apart.
- F7. The sweep's exit is not read (exit 1 goes on); only an `Err` fails `land_agent_files`, and then nothing else runs.
  A landing exit other than 0 is `the agents' files did not land on <id> (exit <n>): <last 300 UTF-16 units of
  stderr>` (ends `: ` when stderr is empty); a cut inside a surrogate pair writes U+FFFD for the half kept.
- F8. Every parser here (`parse_landed`, `parse_landed_servers`, `own_marks`, `outside_marks`, unland's) splits on
  `\n` alone: a trailing `\r` stays part of the path. `str::lines()` would strip it.
- F9. `file_label(Some(""), ..)` is `" <landed at>"` (an empty label is a label). Two lands on one dest are two rows,
  each counting the same landed paths.

## Rules against tests

| Rule | Tests |
|---|---|
| F1 | files_hard.rs: `each_call_rides_the_road_and_bound_the_ts_gives_it` |
| F7 | files_hard.rs: `the_sweep_fails_the_landing_only_by_err_and_the_landing_error_keeps_the_last_300_units` |
| F8 | files_hard.rs: `a_carriage_return_stays_in_the_path` |
| F9 | files_hard.rs: `an_empty_label_is_kept_and_two_lands_on_one_dest_are_two_rows` |
| may land: own row, catalog path; never dotfile, store, other agent, unknown | files_land.rs: the two "which files may land" tests; `an_agent_file_is_its_own_rows_path_and_never_a_lookalike_folder` |
| rows: installed, present, skipped, failed, loose path, notes, labels | files_land.rs: the two rows tests; `the_rows_name_three_paths_then_how_many_more_put_failed_first_and_read_a_moved_store`; `no_planned_path_and_nothing_landed_is_no_row` |
| landing: missing lands, same stands, person's file kept; empty file written over | files_land.rs: "lands what is missing ...", "writes over an empty file ..." |
| restart: a second run reads the list the first close wrote (present, replaced, kept) | files_land.rs: "lands its own copy again when this computer's file changed ...", "leaves the list as it was after a round that landed nothing" |
| a backslash path read off the list | files_land.rs: "reads its own copy of a path holding a backslash ..." |
| once path lands where missing, stands after, never in the list | files_land.rs: "lands the file an agent keeps for itself only where it is missing ..." |
| what travels lands under wsp's folder, not /tmp | files_land.rs: "puts what travels under wsp's own folder ..." |
| standing read: one xargs pass, NUL records, first list line, paths on stdin; read that did not happen sends all | files_land.rs: the four "what already stands" tests |
| exact scripts and parsers | `the_standing_read_is_the_ts_text_byte_for_byte_with_a_quote_in_the_home`, `the_landing_is_the_ts_text_byte_for_byte_with_and_without_once_paths_and_stores`, `the_standing_records_keep_tabs_in_a_path_skip_short_records_and_let_a_later_record_win`, `nothing_printed_is_nothing_standing`, `the_landing_lines_need_the_mark_four_fields_and_a_known_outcome` |
| stores and once | `a_store_moves_only_its_own_folder_and_a_store_at_the_home_folder_moves_nothing`, `a_path_lands_once_under_a_once_folder_and_not_beside_it`, `the_merged_line_says_none_or_names_each_server` |
