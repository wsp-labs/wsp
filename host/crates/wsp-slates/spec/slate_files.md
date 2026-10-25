# slate_files
Source: packages/runtime/src/slate-files.ts (261 lines) at 65f6b89c7. TS tests: no file of its own; its rules are asserted through `createSlates` in `test/slate-box.test.ts:698-1149` (pins, Always and its refusals) and `test/slate-mcp.test.ts:639-662` (a `then`'s files). Rust: `src/slate_files.rs`, tests `tests/slate_files.rs`. Every expected value below was printed by the TS under node (probe `pfiles.ts`, in the report).

## What it does
The files a slate run reads, and what an approval ("Always in this thread") binds: the slate's own files a command names, and a hash of every script in the thread's folder it names, read on this disk (`pin_here`) or by the thread's computer's daemon (`box_pins`, through `fs.hash`). A pin that cannot bind a script it names says why, and an Always is then refused.

## Behaviour
1. `files_read(doc, decl)` (`:11-16`): `said` = the cmd (empty for a tool or resource) and the `then` (empty when absent) joined by `\n`. When `said` matches `/\$\{?SLATE_DIR\b/`, every file of `doc.files` is read; else a file is read when its name appears in `said` bounded on both sides by start, end or a character outside `[A-Za-z0-9._-]` (dots in the name are literal). Answers `None` when no file is read, else the read files in `doc.files` order. `doc = None` reads nothing.
2. `with_files(doc, decl)` (`:32-35`) answers the declaration unchanged when nothing is read, else the declaration with `files` set (the key goes last).
3. `write_slate_files(dir, files)` (`:20-29`): `mkdir -p dir` with mode 0700 (the mode applies only when made); every entry of `dir` not named in `files` is removed recursively; then for each file, in order: remove what stands at its name (recursively, a link is removed and never followed), create it exclusive (`O_EXCL`) with mode 0600 and write the text. Answers `dir`. Errors pass to the caller.
4. `paths_named(decl)` (`:38-40`): the cmd and the `then` (empty when absent) joined by `\n`, split on runs of `[\s'"` + "`" + `;|&()<>=,]`; kept are the words that are not empty, do not start with `-`, hold no `$`, hold no `://`, and hold a `.` or a `/`. Order and repeats kept. Example: `jq . a.json` gives `.` and `a.json`.
5. `words_at(folder, decl, there)` (`:44-47`): the folder the command runs in, `ran`, is `folder` when `cwd` is absent, else `posix_resolve(folder, cwd)` when `there`, else the host's `resolve(folder, cwd)` (both lexical, `..` folded, an absolute `cwd` wins). Each word of rule 4 is kept when it starts with `/`, else becomes `<ran>/<word>`, and `ran = "/"` gives `/<word>` (never `//`). No normalising of the word itself.
6. `words_of(decl)` (`:119-163`), the shell reading of rule 4's text: split into simple commands on `; | & ( )` and newline outside quotes; words on whitespace outside quotes; `'` and `"` open and close quotes; a `\` outside single quotes takes the next character literally and marks the word escaped. Each word has `text` (quotes and escapes off), `said` (as written), `loose` (escaped, or `text` holds whitespace, or `said` with every `$SLATE_DIR`, `${SLATE_DIR}` and `$SLATE_DIR}` taken out still holds `$`) and `run`. In each command, words of the form `NAME=...` (`/^[A-Za-z_][A-Za-z0-9_]*=/` on the text) are skipped; then while the word is a runner (rule 7) step past it and every following word starting with `-`, noting `inline` when one of those is `-c`, `-e`, `--eval` or `-m`. The word reached is `run` unless `inline`, and unless it is the first word and starts with `/` (an absolute program is a tool of the computer).
7. A runner (`:101-107`): the last `/` segment of the word is in RUNNERS (`sh bash zsh dash ksh fish source . python python2 python3 node nodejs deno bun tsx ts-node ruby perl php lua Rscript env exec nohup sudo time nice command`), or is after removing a trailing run of `[\d.]+` (so `python3.12` is `python`).
8. SCRIPT (`:99`): a name ending, case-insensitively, in `.sh .bash .zsh .fish .ksh .py .js .mjs .cjs .ts .mts .cts .jsx .tsx .rb .pl .pm .php .lua .r .jl .ps1 .awk .tcl`.
9. `pin_of(folder, decl, read, there)` (`:174-191`), in this order:
   - `unpinned` starts with every word of rule 6 that is `loose` and (`run` or SCRIPT on its text), keyed by `said`, why `built`, first one wins;
   - with no `read`, every word of rule 4 not yet keyed is added with why `unread`;
   - then for each word of rule 4 at index `i` with `full = words_at[i]`: `script` = SCRIPT on the word or the word is the text of a `run` word. With a `read`, the why is: `paths` when `there` and `i >= FS_HASH_PATHS_MAX` (256); else none unless `script`; else `past` when `full` is in `read.past`; `special` when in `read.special`; `outside` when the word starts with `~` or the folded `full` is not under the folded `folder` (folded: `posix_normalize` there, `resolve` here; under means starting with `folder` plus `/`, or `folder` itself ending in `/`); `link` when `full` is in `read.outside`. A why is added only when the word is not yet keyed.
   - the pin is `{scripts: read.files}` when that map is not empty (absent otherwise) and `unpinned` in first-keyed order.
10. `keyed(pin)` (`:68-69`): `scripts` (or `None`) when nothing is unpinned, else `scripts` followed by each unpinned `path: why`.
11. `cannot_pin(pin)` (`:87-90`): the unpinned entries other than `unread`, each `<path> <phrase>`, joined by ` and `, then `, so wsp cannot pin it and it asks every time` (`them` when more than one entry is unpinned in all, `unread` ones counted). `None` when only `unread` entries, or none. Phrases (`:71-78`): outside `is outside the thread's folder`; link `is a link out of the thread's folder`; past `is past what one command can pin (32 files, each up to 4 MiB)`; special `is not a regular file`; built `is a script wsp cannot read by name (a space, an escape or a $ in it)`; paths `is past the 256 paths one command can name`.
12. `always_refused(run, pin)` (`:93-97`): `None` when nothing is unpinned; with only `unread` entries, `$<run> runs on the thread's machine and names <paths joined by ", "> there, which its computer could not hash to hold an Always to.`; else `$<run> names a script wsp cannot pin to hold an Always to: <rule 11's joined list>.`
13. `read_here(folder, decl)` (`:197-214`): `None` when `realpath(folder)` fails. For each word of `words_at(folder, decl, false)`: skip when `realpath` or `stat` fails, when it is a folder, or when its relative path to the real folder is already in `files`; relative path starting with `..` (first segment) or absolute goes to `outside` (the word as given); not a regular file goes to `special`; size over FS_HASH_CAP_BYTES (4 MiB) or `files` already holding FS_HASH_FILES_MAX (32) goes to `past`; else `files[rel] =` the first 16 hex of the sha256 of its bytes.
14. `pin_here(folder, decl)` (`:217-220`): `{unpinned: []}` when `folder` is `None` or the declaration is not a cmd; else `pin_of(folder, decl, read_here(..) or an empty read, false)`. A folder that is not there therefore pins nothing and leaves nothing unpinned.
15. `box_pins(hash_on, wait_ms = 10_000)` (`:231-261`) holds one reading per thread, `{at, pins}` keyed by `pin_key = JSON [folder, words_at(folder, decl, true)]`:
   - `read(thread, cmds, asleep)`: takes the next number `at` of a counter shared by all threads; `on` is `None` when `asleep` (no daemon is asked), else `hash_on(thread)`. Asks are the cmds with a folder, a cmd declaration and at least one path named, deduplicated by `pin_key`, each sending `(folder, words_at(...).take(256))`. No asks: the thread's reading is deleted and nothing else happens. Each ask runs at once (all together); one that throws, or does not answer within `wait_ms`, gives no reading for its key; the timer is cancelled once the ask settles. A reply's `files` are cut to their first 16 hex. The new reading replaces the held one only when the held one's `at` is lower (an older read finishing late never overwrites a newer one).
   - `pin(thread, folder, decl)`: `{unpinned: []}` for no folder or a non-cmd; else `pin_of(folder, decl, read, true)` where `read` is an empty read when the command names no path, else the held reading for its key (absent: every named path `unread`).
   - `drop(thread)` deletes the thread's reading.
16. Nothing here survives a restart: `box_pins` is memory only, and a new host reads again before a start (the caller's job, `slates.ts`).

## Edge cases (all pinned from the TS)
- `bash './my evil.sh'`: `'./my evil.sh'` is `built`; `pathsNamed` also yields `evil.sh`, judged as a word of its own.
- `bash ../away.sh; bash ../away.sh`: one entry, `outside`.
- `python3 /opt/acme/stats.py` and `bash ~/away.sh`: `outside`.
- `df -h /`, `sleep 0.5`, `cat ../notes.txt`, `bash -c '...'`, `python3 -m http.server`: nothing unpinned (not scripts, or inline).
- `echo stats.sh` hashes `stats.sh` too: every named regular file inside is bound, script or not.
- `./stats.sh`, `/usr/bin/env bash stats.sh`, `X=1 bash stats.sh`, `sudo -E bash stats.sh`, `bash lib/../stats.sh`: bound by hash, nothing unpinned.
- `cwd="scripts"` binds `scripts/stats.sh`; a box reading made without the cwd does not serve the same command with it (different key), so it reads `unread`.
- 257 paths there: the 257th is `paths`; only the first 256 are sent.
- Folder `/` with `cwd="/opt"`: asks `/opt/a.sh`; folder `/root/acme` with `cwd="../other/./x"` asks `/root/other/x/a.sh`.

## Decisions
1. `HashOn` is a boxed async closure; `box_pins` returns a `BoxPins` struct holding `RefCell` state, borrowed only between awaits (rule 3.3).
2. `Pin.unpinned` keeps first-keyed order (an `IndexMap` in the implementation); `scripts` an `IndexMap` in the reply's order.

## After the port
- A `$SLATE_DIR}` with no opening brace is also taken out by the `loose` test (`:125`), so `bash $SLATE_DIR}x.sh` reads as not loose.
- `files_read`'s name match builds a regex from the name with only `.` escaped (`:14`): a file name holding `+`, `(` or `[` is read as a pattern (and `[` throws in JS). Kept; the Rust port must escape only `.` and treat the rest as the TS regex does, or refuse such names as the TS throws. Pinned: `x.y.z` matches only literally.

## Tests (`tests/slate_files.rs`)
| Rule | Test |
|---|---|
| 1, 2 | `with_files_reads_named_files_bounded_by_non_name_characters`, `with_files_reads_every_file_where_slate_dir_is_named`, `with_files_leaves_a_decl_that_reads_nothing_unchanged`, `with_files_matches_dots_literally`, `with_files_reads_a_tool_runs_then`, `with_files_with_no_doc_reads_nothing` |
| 3 | `write_slate_files_keeps_only_the_documents_files_mode_0600_and_never_follows_a_link` |
| 4 | `paths_named_splits_and_filters_as_the_ts`, `paths_named_drops_flags_urls_and_dollar_words` |
| 5, 15 | `box_pins_joins_relative_words_to_the_folder_and_cwd_as_posix`, `box_pins_sends_at_most_256_paths_and_marks_the_rest_paths` |
| 6, 7, 9 | `pin_here_binds_scripts_by_hash`, `pin_here_marks_each_unpinnable_script_with_its_why` (link, outside twice, absolute, home, special, built three ways, past by size), `pin_here_leaves_tools_inline_code_and_reads_bound`, `pin_here_follows_cwd` |
| 8 | `script_suffixes_are_case_insensitive` |
| 10, 11, 12 | `keyed_cannot_pin_and_always_refused_say_the_ts_words`, `always_refused_for_an_unread_pin_names_the_computer_could_not_hash` |
| 13, 14 | `pin_here_with_no_folder_or_a_missing_folder_pins_nothing`, `pin_here_past_32_files_marks_the_rest_past` |
| 15 | `box_pins_reads_once_per_key_and_cuts_hashes_to_16`, `box_pins_leaves_a_command_unpinned_where_the_daemon_does_not_answer_in_time_and_asks_nothing_asleep` (TS slate-box.test.ts:1132), `box_pins_an_older_read_finishing_late_does_not_replace_a_newer`, `box_pins_with_no_asks_drops_the_reading`, `box_pins_a_failed_ask_reads_unread` |
