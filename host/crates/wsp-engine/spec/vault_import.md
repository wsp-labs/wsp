# vault_import (vault.ts:289-539)

A gzipped tar onto a machine: sent up in parts of at most 32 MiB, reassembled and checked against its sha256 on the
guest, then extracted (replacing or overlaying); and a folder landed whole in one rename. Nothing survives a restart:
the parts are removed on every exit of the guest script, and on any upload failure.

## tar_of (:386) and fits_tar (:380)

`tar_of(entries)` is `tar_of_at(entries, floor(now_ms / 1000))`. Per entry, in order:
1. Body: a file's content bytes; a dir or link has none. Over `TAR_MAX_FILE_BYTES` (0o77777777777): `<path> is <n>
   bytes, over what a ustar header holds`. A link target over 100 UTF-8 bytes: `<path> links to a target too long for
   a ustar header`. These two are checked before the name.
2. Name: the path with every leading `/` cut, split by `split_name` (:363): whole when at most 100 UTF-8 bytes; else
   the last `/` (scanning right to left, a `/` at index 0 never a cut) leaving a prefix of at most 155 bytes and a
   name of at most 100. No cut fits: `<path with leading slashes cut> does not fit a ustar header`.
3. A 512-byte zero header: name at 0 (100), mode at 100 as 7 octal digits and NUL (0o777 for a link, the entry's mode
   otherwise), uid at 108 and gid at 116 `0000000\0`, size at 124 as 11 octal digits and NUL, mtime at 136 likewise,
   checksum at 148, type at 156 (`2` link, `5` dir, `0` file), link target at 157 (100), `ustar\0` at 257, `00` at
   263, `root` at 265 and at 297, prefix at 345 (155). The checksum is the byte sum with the field as 8 spaces,
   written as 6 octal digits, NUL, space. Then the body, padded with zeros to a multiple of 512.
4. After the last entry, two zero blocks (1,024 bytes); no entry is those two blocks alone. Then gzip (flate2's
   framing; the tar inside is the TS's byte for byte, `tests/fixtures/oracle/provision.json` `tarOf`).

`fits_tar(path, target)`: the path with leading slashes cut splits, and the target (when given) is at most 100 bytes.

## import_into (:424)

1. An empty tar: `vault import: empty archive, nothing to import`, before any call.
2. `tmp = guest_tmp_path("wsp-vault-in", opts.tmp_dir) + ".tgz"`; `parts = ceil(len / UPLOAD_PART_BYTES)`; one part
   is `tmp` itself, more are `tmp.part0` .. `tmp.part<n-1>`. 32 MiB exactly is one part; one byte more is two, the
   second of one byte.
3. Per part in order: where `has_byte_road(machine)`, `land_bytes(machine, path, bytes, default)` and its `pieces`;
   else `put_part(fetch, machine.upload_url(path), bytes, i + 1, parts)` with `opts.fetch` or `platform_fetch()`.
   Then `on_part({ part: i + 1, parts, bytes: end of this part, total: len, pieces })` (`pieces` `None` on the URL road
   and where the byte road said nothing).
4. Any failure in step 3 (an upload URL, a part, a byte road): exec `rm -f` of every part path, quoted, with
   `INLINE_EXEC_MS`, that failure ignored; then fail with the original error. Every path, not only those that landed.
5. The script (exact text in the oracle `importInto`), joined by `\n`: `set -eo pipefail`; with nothing left out
   `trap "rm -f <parts>" EXIT`, else `kept_aside`'s `where`, its `back` holding the `rm -f`, and `trap back EXIT`;
   ``sum=$(cat <parts> | sha256sum | cut -d' ' -f1)``; `test "$sum" = <sha256 hex of the whole tar> || exit 65`;
   `mkdir -p <q dest>`; `kept_aside`'s `aside` line when there is one; `cat <parts> | tar xzf -<leave_flags(dest,
   leave)> -C <q dest> <flag>` where the flag is `--no-same-owner` for an overlay, `--recursive-unlink` otherwise.
   `kept_aside` (:478) takes a second `guest_tmp_path("wsp-vault-keep", tmp_dir)` as the folder things wait in, and
   `None` for an empty `leave`.
6. `run` with deadline `opts.timeout_ms` or 120,000. Exit 65: `vault import: the uploaded archive (<len> bytes in <n>
   part|parts) did not match its hash on the machine` (`part` for one). Other non-zero: `vault import untar failed
   (exit <n>): <last 500 of stderr>`. A part missing on the guest fails `cat` under pipefail, which is exit 1 with
   cat's words, never 65. Answers `Imported { parts }`.

## put_part (:315) and upload_failure (:304)

Up to three attempts of `PUT url` with the bytes as body. A rejected fetch on attempt 3 fails `vault import upload
failed<where>: <the error's message> after 3 attempts`; before that, sleep `backoff_ms(attempt, random)` and retry.
An ok answer returns. Otherwise read the body text (unreadable is `""`), `classify(status, None, error, None)` where
`error` is the body's JSON `error` when it is a string; `should_retry(kind, attempt)` false fails with
`upload_failure(status, body, part, parts, attempt)`, else sleep and retry. Only `transient` retries: a 502 to 504
retries unless its JSON `error` is exactly `Failed to snapshot sandbox` (that 502 is snapshotUnavailable, final at
once); 500, 501, 505, 413, 429 and any other 4xx are final on the first answer. Parts already up are not sent again; the retry is per part.

`upload_failure`: `vault import upload failed`, then ` on part <p> of <n>` unless one part, `: HTTP <status>`, then
` <reason>` unless empty (the JSON `error` string, else the trimmed body's first 300 UTF-16 units), ` after <n>
attempts` only past one, `; the upload takes at most <limit> bytes per PUT` only when JSON `limit` is a number.

Pinned since (V-rules, values from running the TS):

- V8. Sleeps: `backoff_ms(1)` (1000 to 1249 ms) after the first try, `backoff_ms(2)` (2000 to 2249) after the second,
  none after the third. Each PUT is method `PUT`, the part as body, no header, no deadline (`abort: None`); the URL is
  minted once per part, not per attempt.
- V9. A 502 `{"error":"Failed to snapshot sandbox"}`, a 500 and a 429 are each one PUT then the error.
- V10. The reason: a string `error` is taken whole, even `""` (then no reason, and the body's other words are not
  used) and even past 300 units; the 300 cut is for a body whose JSON has no string `error` (an array, a quoted
  string, `null`, non-JSON), taken as its trimmed text. `limit` is printed as JS prints a number (`1.5`, `0`); a
  string `limit` adds nothing.
- V11. `<path> links to a target too long for a ustar header` and the size refusal name the path as given (leading
  slashes kept); `<path> does not fit a ustar header` names it with its leading slashes cut; a link is judged before
  its name.
- V12. `split_name` takes the rightmost slash that leaves a prefix of at most 155 and a name of at most 100 bytes:
  `a*50/b*50/c*50/d` is prefix `a*50/b*50/c*50`, name `d` (the cut after `b` also fits and is wrong).
- V13. `tar_path(root, p)` is posix `path.relative`: `("/root", "/root")` is `""`, a path outside climbs (`../etc/x`,
  and the lookalike `/rootx/y` is `../rootx/y`), trailing and doubled slashes and `.`/`..` segments normalise
  (`/root/a/../b` is `b`). A relative input resolves against the host's current folder, as node's does.

## land_bundle (:510)

1. `target` is `dest` with trailing slashes cut. `dest` not starting with `/`, or `target` empty (`/`): `destination
   must be an absolute path below /, got <dest>`, before any call.
2. Probe exec `test -e <q target> && echo yes || echo no` (`INLINE_EXEC_MS`). Exit not 0: `could not look at <target>
   on the machine: <last 200 of stderr>`. `yes` without `replace`: `Exists` (`exists_on_machine(target)`), before any
   byte goes up.
3. `staging = <target>.wsp-in-<ms>-<base36>`; `import_into(machine, tar, staging, overlay: true, tmp_dir, fetch,
   timeout_ms, on_part)` (no `leave`). Its failure: exec `rm -rf <q staging>` (`INLINE_EXEC_MS`, ignored), then fail.
4. `on_landing()`, then run (deadline `timeout_ms` or 120,000): `set -e`, `trap "rm -rf <q staging>" EXIT`, `mkdir -p
   <q parent of target, "/" for a top-level target>`, `rm -rf <q target>` with replace or `test ! -e <q target> || exit
   66` without, `mv <q staging> <q target>`. Exit 66: `Exists`; other non-zero: `landing at <target> failed (exit
   <n>): <last 500 of stderr>`. Answers the import's parts.

## Rules against tests

| Rule | Tests |
|---|---|
| V8 to V13 | vault_hard.rs: `a_transient_part_sleeps_the_backoff_between_its_three_tries_and_not_after`, `a_502_that_names_the_snapshot_a_500_and_a_429_are_final_at_once`, `an_upload_refusal_reads_the_body_as_the_ts_does`, `tar_refusals_spell_the_path_as_the_ts_does`, `a_long_name_is_cut_at_the_rightmost_slash_that_fits`, `tar_path_is_posix_path_relative` |
| tar bytes: split names, link, dir, UTF-8, modes, checksum | `the_tar_inside_the_gzip_is_the_ts_bytes_with_a_split_name_a_link_a_dir_and_utf8`, `no_entry_is_two_zero_blocks` (provision_pins.rs); vault_import.rs: tarOf cases |
| what fits, byte counts, refusals | `what_fits_a_ustar_header_counts_bytes_cuts_leading_slashes_and_splits_at_a_slash`, `a_path_or_link_ustar_cannot_hold_is_refused_with_the_ts_words` |
| the import script, hash, flags, leave, empty archive refused before any call | `each_import_sends_the_ts_script_checks_the_hash_and_refuses_an_empty_archive_before_any_call`; vault_import.rs: importInto, overlay, leave cases |
| part boundary 32 MiB / +1 byte; 40 MB in two parts byte-identical; progress per part; pieces | vault_import.rs: parts cases |
| retry 502 per part with a bound; three attempts then words and every part removed; 413 names its limit | vault_import.rs: retry, give-up and refusal cases; `an_upload_refusal_names_the_part_the_reason_the_attempts_and_the_limit` |
| missing part is cat's error; failed extract leaves no part; hash mismatch refused before extracting | vault_import.rs: those three cases |
| kept-aside paths come back even when the extract fails | vault_import.rs: replacing import keeps, extract-fails cases |
| landing: parents made, one rename, no staging left; exists refused before upload; relative and root refused | `a_landing_refuses_a_relative_path_and_the_root_and_moves_the_staging_in_one_rename`; vault_import.rs: landBundle cases |
| exists words and kind | `an_existing_destination_is_kind_exists_with_the_ts_words` |
