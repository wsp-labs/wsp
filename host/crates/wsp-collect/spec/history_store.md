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
7. `read_store` (index.ts:43-69): `reader.files`; up to `FILES_AT_ONCE` (4) reads at once over one shared index, each taking the next file: a cache hit (`get(path, stamp)`) is used as is, else `reduce_calls(reader.read(...))` and `set(path, stamp, buckets)`; each finished file stores `{holder(root, path), buckets}` at its own index and calls `on_file(read_so_far, total)`. The first failure stops new files from starting (the index jumps to the end); reads already open finish; then that error is returned. Otherwise `usage_of(one_holder(held in file order), folders)`.
8. `read_histories` (:93-116): per agent in the given order: no `history` gives state `no-reader` with zeros and an empty usage; else `read_store(history_reader(format), expand(home, root), ...)` with `on_progress({agent, read, files})` per file; an error gives `unreadable` with zeros; zero sessions gives `empty`; else `read` with the usage's sessions and calls. `on_agent` is told each answer as it is made. After the last agent the cache is saved once; a save error is the function's error (the TS throws out of `save`).
9. Cache file (cache.ts:35, 41-51): `{"version":1,"parse":2,"files":{"<path>":{"stamp":"...","buckets":[...]}}}`. It is read once when the cache is made: missing, unreadable, not JSON, another version, another `parse`, or any field off its shape (a negative or fractional count, a non-string session) gives an empty cache, never a partial one.
10. `get` (:57-62): a stored entry with the same stamp is a hit and is kept for the next save; a different stamp or no entry is a miss and keeps nothing. `set` keeps the new entry (replacing, never merging) and marks the cache changed.
11. `save` (:67-76): nothing is written when nothing was set and the kept count equals the stored count. Otherwise the folder is made (recursive), the text `JSON.stringify(file) + "\n"` (one line) is written to `.<ms>-<6 base-36 chars>.tmp` in the same folder and renamed over the path. Files nothing asked about are dropped.
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
