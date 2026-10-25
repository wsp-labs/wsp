# history_store

Source: `packages/collect/src/history/tally.ts` (171), `index.ts` (124), `cache.ts` (78) at `a3b870a6e`. Tests: `history.test.ts` `describe("readHistories")` (2), `describe("the session cache")` (7), and every reader case through `readStore`. Rust: `src/history/{tally,cache,mod}.rs`; tests `tests/history_store.rs` (9), `tests/history_hard.rs` `tally::*` (3), `store::*` (7).

## What it does

Reduces each session file's calls to per-session buckets of names and counts, keeps them in a cache file so a second run reads only the files whose stamp moved, picks one holder per session where a session's files were copied, and adds a store's buckets up into a `Usage`. `read_histories` does this for every catalog agent and says whether each store was read, empty, unreadable or has no reader.

## Behaviour

1. `reduce_calls` (tally.ts:95-123): one bucket per `session + "\0" + (folder or "")`, in first-call order; a new bucket is `{session, folder?, calls: 0, commands: {}, installs: {}}`; each call adds 1 to `calls`; `at` is set to the call's time when the bucket has none or the call's is greater (so `at` is written last, rule 12); a shell call adds 1 per `command_names` entry to `commands[name]` and 1 per `install_names` entry to `installs[install_key]`, keys in first-seen order. The first `Err` item is returned and nothing after it is polled.
2. `install_key` is `"<via> <name>"`; `installed_name` is the text after the first space.
3. `one_holder` (:127-143): per session, the newest `at` across each holder's buckets (no `at` is minus infinity); the kept holder is the first (in `held` order, then bucket order) whose newest is strictly greater than the kept one's; a tie keeps the first. Answer: every bucket, in held order then bucket order, whose holder is its session's kept holder.
4. `in_folders` (:87-90): a bucket with a folder that `under_project(folder, f)` (`wsp_words::project_path::under_project`: equal, or starts with `f + "/"`) for some `f`; no folder is never in.
5. `usage_of` (:147-171): skip buckets not `in_folders` when `folders` is given (an empty list skips all); `sessions` is distinct session ids; `calls` is the sum of `calls`; commands and installs tally per name `{sessions: distinct sessions, calls: sum}`; tools tally `catalog_tool_for(name)` for each command and `catalog_tool_for(installed_name(key))` for each install, by tool id. Each map is sorted most sessions first, then most calls, then name by UTF-16 code units (`wsp_words::utf16::cmp`).
6. `meets_used_floor` (:22-25): `is_heavy(size)` (size over `HEAVY_BYTES`, 300 MiB) picks `HEAVY_USED_FLOOR` (3 sessions, 20 calls) else `USED_FLOOR` (2, 5); both counts must reach the floor.
7. `read_store` (index.ts:43-69): `reader.files`; up to `FILES_AT_ONCE` (4) reads at once over one shared index, each taking the next file: a cache hit (`get(path, stamp)`) is used as is, else `reduce_calls(reader.read(...))` and `set(path, stamp, buckets)`; each finished file stores `{holder(root, path), buckets}` at its own index and calls `on_file(read_so_far, total)`. The first failure stops new files from starting (the index jumps to the end); reads already open finish, and each still calls `set` on the cache and `on_file`, so their files are kept in the cache after the store is said unreadable. The four reads are four workers, numbered 0 to 3 in start order, each taking files one after another; the error returned is the failure of the lowest-numbered worker that failed, not the first in time (`Promise.allSettled` then `find`): with file 3 failing before file 1, the store answers file 1's error. Otherwise `usage_of(one_holder(held in file order), folders)`.
8. `read_histories` (:93-116): per agent in the given order: no `history` gives state `no-reader` with zeros and an empty usage; else `read_store(history_reader(format), expand(home, root), ...)` with `on_progress({agent, read, files})` per file; an error gives `unreadable` with zeros; zero sessions after the folders are weighed gives `empty` (a store full of sessions none of which ran at a named folder is `empty` with zeros); else `read` with the usage's sessions and calls. `on_agent` is told each answer as it is made. After the last agent the cache is saved once; a save error is the function's error (the TS throws out of `save`).
9. Cache file (cache.ts:35, 41-51): `{"version":1,"parse":2,"files":{"<path>":{"stamp":"...","buckets":[...]}}}`. It is read once when the cache is made: missing, unreadable, not JSON, another version, another `parse`, or any field off its shape (a negative or fractional count, a non-string session, `null` for the optional `folder` or `at`) gives an empty cache, never a partial one: one bad entry refuses every entry. Unknown keys anywhere are accepted and dropped from what the next save writes.
10. `get` (:57-62): a stored entry with the same stamp is a hit and is kept for the next save; a different stamp or no entry is a miss and keeps nothing new (a path already hit this run stays kept after a later miss on it). `set` keeps the new entry (replacing, never merging) and marks the cache changed.
11. `save` (:67-76): nothing is written when nothing was set and the kept count equals the stored count. Otherwise the folder is made (recursive), the text `JSON.stringify(file) + "\n"` (one line) is written to `.<ms>-<6 base-36 chars>.tmp` in the same folder and renamed over the path. Files nothing asked about are dropped: a run over a subset of the agents, or over agents that have no reader, drops every other agent's entries (a run over no-reader agents alone empties a filled file to `"files":{}`). With nothing stored and nothing asked no file and no folder is made; a refused file (corrupt, another version) stays on disk untouched while nothing is set.
12. Key order (rule 4.26): a bucket is `session, folder?, calls, commands, installs, at?`, which is the TS order for a freshly read bucket. The `files` map is in the order entries were kept: hits and sets as they happened.

## Edge cases

- A store with no files: usage all zero, `on_file` never called (`store::a_store_with_no_files_is_empty_and_tells_no_progress`).
- A save with nothing asked of a filled cache writes `"files":{}` (`store::a_save_with_nothing_asked_of_a_filled_cache_empties_it`).
- A cache whose folder is a file fails the save, so `read_histories` fails (`store::a_failed_save_is_the_error_read_histories_answers`).

## What survives a restart

The cache file only. A new `file_history_cache` over the same path trusts every entry whose stamp still matches (`store::a_new_cache_over_the_same_file_trusts_what_the_last_run_kept_as_after_a_restart`); counts are never kept in memory between runs.

## Decisions

1. A bucket the TS kept from the cache is rewritten in zod's shape order (`at` third); the Rust struct writes `at` last always. Only this program reads the file, and either order parses.
2. `files` order follows read completion in the TS; the Rust order follows whichever completion order the four reads give. The hard test compares the entries as a map.
3. The `PARSE_VERSION` fingerprint hashes the six Rust files; its fixture `tests/fixtures/parse-fingerprint.txt` reads `unset` and the implementer writes the digest the test prints once the bodies are final. That one fixture is the implementer's to write.

## After the port

- `on_agent` is told an agent's result before the cache is saved, so a failed save throws after every agent was announced as read. Kept.
- Integer-like command names (`"7"`) would sort first in a JS object; Rust keeps insertion order. Only the cache file sees it.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/history_store.json` (`oracle/w2-history_store.oracle.ts`), replayed by
`tests/history_hard.rs` `w2_oracle::*`; the worker traces are `tests/history_hard.rs` `workers::*`, their expected
values printed by a gated reader in the TS.

- W1. Four workers in start order over one shared index; a worker takes the next file as soon as its last one lands
  (`workers::with_no_failure_every_file_is_read_four_at_a_time_in_file_order`).
- W2. The lowest-numbered failing worker's error wins, not the first in time; no file starts after a failure; reads
  open at the failure still land, are told and are cached (`workers::the_lowest_numbered_worker_s_failure_*`,
  `workers::a_later_file_on_worker_0_beats_*`, `workers::a_failed_store_keeps_the_reads_that_landed_*`).
- W3. The cache's `files` map is in the order entries were kept: a hit when asked, a read when it lands; across a
  restart a moved stamp goes to the end (`workers::the_cache_keeps_files_in_the_order_they_were_kept_across_a_restart`).
- W4. Cache shape: 29 shapes, each with the answer of `get` and the file after (`file_history_cache` cases named
  `shape: ...`): unknown keys accepted and dropped on the next write; `null` folder or at, a fractional or negative
  count, a string count, `installs` as an array, a numeric stamp, missing `buckets`, `files` not an object, version
  `"1"`, `parse` missing or `1`, a numeric session: the whole file refused. A BOM before the JSON, or text after it,
  refuses it (JSON.parse); blank lines after it do not; a duplicate key keeps the last value; `1.0`, `2.0` and `1e3`
  are the integers they write.
- W5. Bookkeeping: save with nothing asked and no file makes no file and no folder; over a garbage file leaves it; a
  set over a garbage file replaces it whole; a set makes the missing folder; set twice replaces in place; hits and a
  miss with nothing set write nothing; a hit then a miss on one path keeps the hit; escapes in text survive
  (`file_history_cache`). No temp file is left in the folder after any save.
- W6. `read_histories`: first and second runs, a later mtime or a grown file re-reads one file, a gone file leaves the
  cache, a half-written or other-version cache reads as none, a run over a subset or over no-reader agents drops the
  rest, `folders` matching nothing says `empty`, an unreadable file among three leaves the other two cached, the
  states for every catalog agent, a cleared copy under one holder counted twice, two holders resolved by newest time
  (`read_histories`, with `told`, `progress`, the host's calls where one store ran, and the cache file after).
- W7. Tally: `reduce_calls` keeps the greatest `at` (0 beats -3) and the first-seen key order; `one_holder` merges a
  holder's files and keeps the first on a tie; `usage_of` sorts sessions desc, calls desc, then UTF-16 name, and the
  tools tally adds commands and installs; `in_folders` with `/`, `""`, `/ab` and a trailing slash; the
  `meets_used_floor` and `is_heavy` boundaries (`reduce_calls`, `one_holder`, `usage_of`, `in_folders`,
  `meets_used_floor`, `is_heavy`, `installed_name`, `install_key`).
- W8. Not copied from the TS: a command named `toString`, `constructor` or `valueOf` is an ordinary name in Rust (the
  TS object turns its count into text), and `__proto__` is an ordinary name (the TS drops it). Integer-like names keep
  first-seen order in Rust (the TS lists them first); the fixture's cases named `[JS key order ...]` are compared as
  maps.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `tally::reduce_calls_makes_one_bucket_per_session_and_folder_*`, `tally::reduce_calls_hands_back_the_first_error_*` |
| 3, 5 | `tally::one_holder_keeps_each_session_*`, `counts_a_session_once_when_two_project_folders_hold_its_files_*` |
| 4 | `counts_only_the_sessions_that_ran_at_a_named_folder_*`, `counts_nothing_from_a_session_whose_store_never_recorded_a_folder` |
| 7 | `ends_a_store_at_a_file_it_cannot_read_*`, `counts_every_session_file_off_as_it_lands_*`, `store::a_store_with_no_files_*` |
| 8 | `names_every_catalog_agent_read_with_counts_empty_unreadable_or_no_reader_*`, `store::a_failed_save_*` |
| 9 | `drops_a_session_file_that_is_gone_*`, `reads_everything_again_when_another_parse_version_*`, `store::a_cache_file_of_another_version_or_shape_*` |
| 10 | `re_reads_the_one_file_whose_stamp_moved_*`, `a_second_run_over_an_unchanged_history_*`, `store::a_new_cache_over_the_same_file_*` |
| 11 | `puts_the_new_cache_in_place_with_a_rename_*`, `store::a_run_that_changed_nothing_leaves_the_file_as_it_was`, `store::a_save_with_nothing_asked_*` |
| 12 | `store::the_cache_file_holds_what_the_ts_wrote_for_the_same_history` |
| fingerprint | `bumps_parse_version_when_the_code_that_fills_a_bucket_changes` |
