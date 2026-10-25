# tools_disk (golden-tools.ts:42-235, 278-285)

df and the disk the loop keeps free, the guard every install runs under, the reason a failed run is named by, the road
a source install reports, the cache sweep and the PATH check. Nothing here survives a restart (golden-README.md).

## df (:44-69, 112-131)

- `df_kb_cmd(columns, path)`: `df -Pk <path> | awk 'NR==2{print <cols>}'`, `<cols>` the asked columns as `$2` (size),
  `$3` (used), `$4` (free), joined `, `, in the order asked; no columns is `print }`. `path` is written as given,
  unquoted; `None` is `/root`. `free_kb_cmd()` is `[Free]`, `used_kb_cmd()` `[Used]` (`df -Pk /root | awk 'NR==2{print $4}'`).
- `prefix_volume(p)`: `"$(p=<shell_quote(p)>; until [ -e "$p" ]; do p=$(dirname "$p"); done; echo "$p")"`. The empty
  prefix is `''`, quoted.
- The read (dfRead): one exec of `df_kb_cmd(columns, prefix.map(prefix_volume))`, timeout `INLINE_EXEC_MS`. `printed`
  is stdout JavaScript-trimmed, `kb` its `split(/\s+/)` read with `Number`. It answered when exit is 0 and `printed`
  is not empty (the pipe exits as awk does, so a failed df exits 0 printing nothing). A reading needs as many numbers
  as columns, every one finite and >= 0, and the size column, where asked, > 0: then `bytes = kb * 1024` each, kept
  as JavaScript numbers (`0x10` is 16 KB, `1e3` 1000 KB; a full disk reads free 0 and an empty one used 0, both good
  readings). Anything else: `df answered <printed cut to 160 UTF-16 units>` when it answered, else `df failed:
  <reason_of(res, 20)>`. A rejected exec is the `Err`.
- `free_bytes(machine, prefix)` is the free column (`Err` when the exec rejects). `disk_use(machine, prefix)` is
  `[Used, Size]` and never fails: a rejected exec reads `Unknown { reason: "df failed: <reason_of({exit 1, stdout
  "", stderr: <its words>}, 20)>" }`. `disk_use_size` is `Some(size)` of a `Use`.
- `free_note`: `<fmt_bytes(free)> free`, `None` for an unknown reading; a rejected exec is the `Err`.
- `owned_floor_bytes(size)`: `max(TOOLS_DISK_FLOOR, floor((size or 0) / 10))`. `TOOLS_DISK_FLOOR` = 2048 MiB.

## Reasons and roads (:90-110, 151)

- `reason_of(res, timeout_s)`: exit 124 is `timed_out_line(timeout_s)` whatever the output. Else lines are
  `split("\n")`, each JavaScript-trimmed, empty ones dropped; the last stderr line starting `Error:`, else stderr's
  last line, else stdout's, else `exit <code>`; then cut to 160 UTF-16 units (a cut inside a pair writes U+FFFD).
- `road_of(stdout)`: the last match of `^WSP_ROAD (release|go) (\S+)(?: ([0-9a-f]{64})(?: (\S+)(?: (x86_64|aarch64))?)?)?`
  with JavaScript's multiline `^` (a line starts after `\n` and after `\r`). No end anchor: a trailing word is ignored,
  65 hex digits keep the first 64 as the sum and no tag, `x86_64x` reads `x86_64`, uppercase or 63 hex digits is no sum
  and so no tag either. `kind` is the road word; absent groups are absent fields. `WSP_ROAD go` alone is no road.
- `closing(parts)`: the parts that are neither `None` nor empty, joined `; `.

## The guard (:74-80, 186-235)

- `guarded(script, timeout_s)`: the TS's script byte for byte (TREE_FN, `setsid bash -c <quoted script> &`, the poll
  every second to `timeout_s`, TERM then KILL to `-$p` and the tree with a 10 s grace each, `exit 124`, `wait $p`).
- `road_limit_s(road)` is `road_step(road).limit_s`; `guarded_road(road, cmd)` is `guarded([...env, cmd].join("\n"),
  limit)`. `guard_deadline_ms(t)` = `(t + 60) * 1000` (`GUARD_SLACK_S` 60: two graces and 40 s of polls).

## The sweep and the PATH check (:153-172, 278-285)

- `sweep_caches(machine, path)`: `free_bytes(machine, None)` (always /root, never a prefix volume), then one run of
  `guarded(<sweep script>, 300)` with deadline 360,000; the script is `set -euo pipefail`, `export PATH=<path>`, `go
  clean -cache -modcache` when go is there, `rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build
  /root/.cache/node-gyp`, `apt-get clean` when apt-get is there. Exit not 0: `cache sweep failed (<reason_of(res,
  300)>)`, no second read. Else read again: both free and more after is `caches swept, <fmt_bytes(after - before)>
  back`, anything else `caches swept`. A rejected exec or run is the `Err`.
- `missing_commands(machine, bins, path, prefix)`: one exec, timeout 20,000, of `<path_line(path, prefix)>\nfor b in
  <bins each shell-quoted, space-joined>; do command -v "$b" >/dev/null 2>&1 || echo "missing $b"; done`. `failed` is
  `reason_of(res, 20)` when exit is not 0; `missing` is every stdout line starting `missing `, the rest
  JavaScript-trimmed, in order, read whatever the exit. A rejected exec is the `Err`.

## Rules against tests

| Rule | Tests |
|---|---|
| df lines, constants | `the_df_lines_and_the_constants_are_the_ts_byte_for_byte`, `df_kb_cmd_writes_the_columns_in_the_order_asked_and_the_path_unquoted` |
| prefix volume, empty prefix | `prefix_volume_quotes_the_prefix_and_walks_up_to_a_folder_that_exists` |
| df readings: pair, full, size 0, too few or many, negative, junk over 160, hex, tabs, empty, exit 1/124/2, rejected | `disk_use_reads_df_as_the_ts_and_never_fails_even_when_the_exec_rejects`, `free_bytes_and_free_note_read_df_as_the_ts_and_a_rejected_exec_is_an_error`, `disk_use_size_is_the_size_of_a_reading_and_none_for_an_unknown_one` |
| owned floor, absent size | `owned_floor_bytes_keeps_the_larger_of_the_floor_and_a_tenth_of_the_disk_floored`, `holds_back_a_tenth_of_the_disk_on_a_computer_somebody_owns_where_that_is_more_than_the_floor` |
| reason: 124, Error line, last lines, empty, 160 cut, CRLF | `reason_of_names_a_failure_as_the_ts_does_for_every_result_and_timeout`, `a_cut_inside_an_emoji_writes_the_replacement_character_for_the_half` |
| road line: last, partial groups, `\r`, 65 hex, arch prefix | `road_of_reads_the_last_road_line_and_only_the_fields_that_match` |
| guard script, road limits and deadlines | `guarded_is_the_ts_script_byte_for_byte_with_a_quote_and_a_zero_timeout`, `every_road_has_the_ts_limit_deadline_and_guarded_script` |
| sweep: freed, nothing, failed, unknown after | `sweep_caches_runs_the_sweep_under_the_guard_and_says_what_came_back` |
| PATH check: quoting, trimmed names, failed exit still read | `missing_commands_sends_one_line_on_the_path_and_reads_missing_off_stdout_even_when_it_failed` |
| closing | `closing_joins_the_parts_and_leaves_out_the_empty_ones` |
