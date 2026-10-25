# vault_export (vault.ts:1-287)

A folder or a set of paths off a machine as one gzipped tar: archived on the guest into a temp file, size-checked,
brought back over the provider's signed URL (or base64 through exec), and the temp files removed. Nothing survives a
restart; the guest keeps nothing once a call returns, except where noted.

## Bringing an archive back

`download(machine, path, sink, opts, total)` (:70):
1. `opts.read_road == Exec` goes to `exec_read` and nothing below runs.
2. `url = machine.download_url(path)`; `fetch_stream` a GET of it on `opts.fetch` or `platform_fetch()` (no headers,
   no body). A rejected fetch is its `RoadError` (`EngineError::Road`).
3. Not ok (outside 200..300): error `vault export download failed: HTTP <status>`. This is read before the body.
4. `length` is `Number(content-length)`: a missing header is `Number(null)`, 0. `known = total`, else `length` where it
   is finite and above 0, else unknown.
5. No body: error `vault export download failed: HTTP <status> with no body`.
6. `on_progress({ bytes: 0, total: known or 0 })` once before any chunk; then per chunk in arrival order: write it whole
   to `sink` (awaiting the write, which is the TS's drain), add its length, then `on_progress({ bytes, total: known or
   bytes })`. A chunk is in the sink before its own progress call. A stream error is that error.

`exec_read` (:92): `bytes = total`, else `guest_file_size`. Over `EXEC_READ_CAP` (16,777,216; equal passes):
`VaultTooLarge { bytes, message: "the archive on the machine is <bytes> bytes and this provider mints no download URL,
so it comes back through one command, which is capped at 16777216" }`, before any read. Then `on_progress(0, bytes)`;
exec `base64 < <q path>` with timeout `opts.timeout_ms` or 120,000; exit not 0: `vault export read failed (exit <n>):
<last 500 of stderr>`. Decode stdout with every whitespace removed; a decoded length other than `bytes`: `vault export
read failed: the machine held <bytes> bytes and <n> came back`. Write it, then `on_progress(n, bytes)`.

`download_buffer` (:107) collects the chunks in order. `download_file` (:115) streams into a new file at `into` and
answers its size on disk once closed; on any error it closes the file, removes it (missing is fine) and fails with
the download's error, so a failed download leaves no file, not even an empty one.

`guest_file_size` (:200): exec `wc -c < <q path>` with `INLINE_EXEC_MS`. The answer is `Number(stdout.trim())`: an
empty stdout is 0, `0x10` is 16, `1e3` is 1000. Exit not 0, or not a finite number: `the archive's size on the
machine is unknown: <last 200 of stderr>`.

## Archiving (archive_of, :209)

1. `tmp = guest_tmp_path("wsp-vault", None) + ".tgz"`; `run(script(tmp))` with deadline `opts.timeout_ms` or 120,000.
   A script function that fails (kept_paths' refusal) fails here, before any call to the machine.
2. Exit not 0: `vault export tar failed (exit <n>): <last 500 of stderr>`. This is thrown before the cleanup is armed:
   a failed tar sends no `rm`.
3. Then, always followed by the cleanup: with `max_bytes`, read `guest_file_size(tmp)`; above it (equal passes):
   `VaultTooLarge { bytes, message: vault_over_cap_line(bytes, max) }` (e.g. `the export was 12 B, over the 11 B
   cap`), and nothing is downloaded. Else bring the archive.
4. Cleanup: exec `rm -f <q tmp> <q tmp.list> <q tmp.keep>` with `INLINE_EXEC_MS`; its failure or exit is ignored.

`guest_tmp_path(what, dir)` (:177): `<dir, or GUEST_TMP, with trailing slashes cut>/<what>-<Date.now() ms>-<1 to 6
chars of a random base-36 fraction>`. Two calls never collide in practice; nothing reads the tail.

## The three exports

- `export_paths(machine, paths, opts)` (:251) into memory, `total` unknown. Script: no paths is
  `empty_archive_script` (`tar czf <q out> --no-recursion --null -T /dev/null`: never a tar with no operand, never a
  find with no start, which from `/` would walk the whole filesystem); no `exclude` is `group_archive_script([{ root:
  "/", paths }], out, leave)`; with a rule, `excluding_archive_script("/", kept_paths(paths, rule) each with one
  leading / cut, rule, out, leave)`.
- `export_paths_into(machine, groups, into, opts)` (:265): `group_archive_script(groups, out, [])` (no leave), into a
  file, `total` unknown. Answers the bytes written.
- `export_folder(machine, dir, rule, into, opts)` (:275): probe exec `test -d <q dir> && echo yes || echo no`
  (`INLINE_EXEC_MS`); exit not 0 or a trimmed stdout other than `yes`: `<dir> is not a folder on the machine`, with
  no other call. `tmp = guest_tmp_path("wsp-out", None) + ".tgz"`; run `folder_export_script(dir, rule, tmp)` with
  deadline `opts.timeout_ms` or 600,000; exit not 0: `packing <dir> on the machine failed (exit <n>): <last 500 of
  stderr>`; `excluded` is stdout split on `\n`, empties dropped, sorted; then `guest_file_size(tmp)` and
  `download_file` with that size as `total`. The `rm -f` cleanup runs after the probe whatever happens (also after a
  failed pack). `max_bytes` is not read here.
- `kept_paths` (:235): drop a path whose last name (trailing slashes cut, then after the last `/`) is in `rule.dirs`
  or `rule.files` by whole name, whatever it is on disk; none kept: `vault export: every path named is a cache under
  the rule (<paths joined by ", ">), so there is nothing to archive`.
- `dest_exists(dest, files)` (:270): `Exists`, `<dest> already exists on this computer with <plural(files, "file")>;
  export with replace to overwrite it`.

## The scripts (byte for byte; tests/fixtures/oracle/provision.json)

- `excluding_archive_script(cwd, targets, rule, out, leave)` (:145): eight lines joined by `\n`: `set -eo pipefail`,
  `cd <q cwd>`, the cache-root find (`-mindepth 1`, `.git` pruned, the cache test pruned and printed) into `<out>.list`,
  `printf '%s\0'` of the targets into `<out>.keep`, the keep find (`.git` and everything under it printed, caches
  pruned, the rest printed, NUL-separated) appended, `tar czf <q out><leave flags> --no-recursion --null -T <q
  keep>`, `sed 's|^\./||' <q list>` (the excluded roots on stdout), `rm -f` of list and keep. The cache test is
  `\( -type d \( -name A -o ... \) \)` for dirs, the same with `-type f` for files, one `\( -type d -exec test -f
  '{}/<marker>' \; \)` per marker, joined by ` -o `, wrapped in `\( ... \)`; with no names at all, `\( -false \)`.
- `folder_export_script(dir, rule, out)` is `excluding_archive_script(dir, ["."], rule, out, [])`.
- `group_archive_script(groups, out, leave)` (:196): `tar czf <q out><leave flags from "/">` then per group ` -C <q
  root> <q tar_path(root, p)>...`.
- `tar_path(root, p)` (:189): root `/` cuts one leading `/` (a relative path stays as it is); any other root is posix
  `path.relative(root, p)`. `leave_flags(root, leave)` (:193): ` --exclude=<q tar_path(root, p)>` per path, each
  before the operands (GNU tar reads an exclude only for names after it).

## Pinned since (V-rules, values from running the TS)

- V1. The exec road: `run` the tar (deadline `timeout_ms` or 120,000), `exec` `wc -c < <q path>` (20,000), `exec`
  `base64 < <q path>` (`timeout_ms` or 120,000; a `Some(0)` is 0, not the default), `exec` the `rm -f` (20,000). No
  URL is minted. Progress `{0, size}` before the read and `{size, size}` after.
- V2. `EXEC_READ_CAP` is inclusive: 16,777,216 is read; 16,777,217 is `VaultTooLarge { bytes, message: "the archive
  on the machine is <n> bytes and this provider mints no download URL, so it comes back through one command, which is
  capped at 16777216" }` with no base64 exec and no progress, the cleanup still sent. A read that comes back short is
  `vault export read failed: the machine held <n> bytes and <m> came back`; a failed read is `vault export read failed
  (exit <n>): <last 500 UTF-16 units of stderr>`.
- V3. base64 decodes as node's `Buffer.from(s, "base64")`: characters outside the alphabet are skipped, padding is
  optional, `-` and `_` read as `+` and `/` (`-_-_` is `fb ff bf`). A strict decoder fails three of the four cases.
- V4. `guest_file_size` is JS `Number(stdout.trim())`: `0x10` is 16, `1e3` is 1000, blank is 0; `abc`, `Infinity`
  and two lines are `the archive's size on the machine is unknown: <last 200 units of stderr>`, as is a non-zero exit.
  (A fraction or a negative reads in the TS; `u64` cannot hold them: after-port note.)
- V5. Every chunk is a progress call, an empty one included. The total is the caller's `Some(total)` (0 included,
  which then stays 0 while bytes grow), else content-length as `Number` where finite and above 0, else the bytes so
  far. A status that is not ok is `vault export download failed: HTTP <status>` before any progress. The request is a
  bare `GET`: no header, no body, `abort: None` (a hung download waits, as in the TS).
- V6. `download_file` opens `into` truncating (a 27-byte file is 7 bytes after a 7-byte download) and removes it on any
  failure, so a file that was there before a failed download is gone, not kept. Writing to a temp file and renaming
  would keep it: wrong.
- V7. Exports always use `GUEST_TMP` for their own archive: `tmp_dir` is an import option and is not read here.

## Rules against tests

| Rule | Tests |
|---|---|
| V1 to V6 | vault_hard.rs: `the_exec_road_reads_the_size_then_one_base64_exec_and_mints_no_url`, `sixteen_mib_exactly_is_read_and_one_byte_more_is_refused_before_the_read`, `base64_is_decoded_as_leniently_as_node_and_a_failed_read_names_its_exit`, `the_guests_size_reads_as_js_number_reads_it`, `progress_counts_every_chunk_and_a_total_of_zero_stays_zero`, `a_failed_download_removes_the_file_that_was_there_and_a_good_one_truncates_it` |
| exact export scripts, commands and order; tar run detached, size read and cleanup inline | `each_export_sends_the_ts_commands_in_order_and_refuses_with_the_ts_words` (provision_pins.rs); vault_export.rs: the run/exec split, exact script spellings |
| no paths: an empty archive, no find, no walk | same, case `none`; vault_export.rs: an export naming no path |
| every path a cache: refused before any call | same, case `allCaches`; `kept_paths_judge_the_last_name_and_refuse_when_every_path_is_a_cache` |
| over the cap: removed, not downloaded, `VaultTooLarge` with bytes; equal passes | same, case `capped`; vault_export.rs: size cap |
| a failed tar surfaces its exit and stderr | vault_export.rs: a failing tar |
| folder export: probe first, sorted excluded roots, nothing left on the guest | vault_export.rs: exportFolder cases |
| progress only rises, total known up front, chunk on disk before its line, progress while the body is open | vault_export.rs: the 6 MB body, progress, lands-as-it-arrives cases |
| a failed or bodiless download leaves no file | vault_export.rs: exportPathsInto failure case |
| groups from their own roots | vault_export.rs: exportPathsInto groups |
| tmp names: folder, clock, base-36 tail | `a_guest_tmp_path_is_the_folder_without_trailing_slashes_the_name_the_clock_and_six_base36` |
| tar paths from `/` and other roots | `a_tar_path_from_the_root_cuts_one_slash_and_keeps_a_relative_path` |
| exists refusal words and kind | `an_existing_destination_is_kind_exists_with_the_ts_words` |

After-port notes: a tar that fails leaves its temp file on the guest (the cleanup is armed after the check); an empty
`wc -c` answer reads as a 0-byte archive.
