# seed

Source: `packages/collect/src/seed.ts` (133 lines) at `a3b870a6e`. Tests: `seed.test.ts` (8 cases). Rust: `src/seed.rs`; tests `tests/seed.rs` (8), `tests/oracle_cases.rs` `seed::*` (1: 3 menus, text and runs against the TS).

## What it does

The seed menu for a folder on this computer: every path git ignores there, sized in one `du`, each judged by the catalog's seed rows, plus the branch, the remote, the commits the remote lacks, the uncommitted edits and Claude Code's memory folder. No file's content is read.

## Behaviour

1. Every git call is `git -C <folder as given> <args>` with `timeout_ms` 120,000 (`CALL_MS`); `du` too. The calls run in this order: the listing; one inside-listing per opened entry; `du` (only when anything was listed); `remote get-url origin`; `rev-parse --abbrev-ref HEAD`; `symbolic-ref refs/remotes/origin/HEAD`; `rev-parse --abbrev-ref @{upstream}`; `merge-base HEAD <against>` and `rev-list --count <base>..HEAD` when reached; `status --porcelain` (seed.ts:24-62, 69-77).
2. Listing (:25): `ls-files -z -o -i --exclude-standard --directory`, split on NUL, empties dropped; an entry ending `/` is a directory.
3. Opened (:27-33): a directory entry is opened when some seed row's `opens` name equals the entry or the entry ends with `/<name>` (`opens`); then `ls-files -z -o -i --exclude-standard -- <entry without trailing slashes>`, keeping only paths `seed_row_for(path, false)` judges.
4. Sizes (:102-118): one `du -sk` over every listed path and every kept inner path, each as `<folder without trailing slashes>/<path without trailing slashes>`; each answer line `<kb>\t<path>` (split at the first tab) whose `kb` reads as a finite number by JavaScript's `Number()` (`0x10`, ` 4`, `""` as 0, `1e1`) and whose path, with only trailing slashes trimmed (no whitespace trim: a `\r` or a space leaves it unmatched), is under that prefix maps the relative path to `kb × 1024`, kept as an `f64` (`4.3` kB is 4403.2); a later line for the same path wins; any other line is skipped. A path with no size is 0.
5. Rows (:40-47, 93-98): for each listed entry in listing order, first its kept inner paths (not dirs), then the entry itself with its own size less the inner sizes, floored at 0. The inner paths subtracted include those `seed_row_for` judges `junk` or `never`, though they get no row. The sort of rule 6 is stable, so rows with equal names keep this order. `row_for`: a `junk` judgement drops the row; no judgement is kind `unknown` with no `row`; else the kind and `row: {id, name}`; `ticked` for `config` and `secret`.
6. Order (:48): rows sorted by `localeCompare` (`wsp_words::js::collate`), so `Zeta` sorts after `b.log`.
7. Plan (:53-63): `source` the folder as given; `remote` the first JS-trimmed line of the origin URL or null; `branch` that of HEAD or `""`; `defaultBranch` the symbolic ref without `refs/remotes/origin/`, or null (an answer that is exactly `refs/remotes/origin/` gives `""`, which is not null); `unpushed`; `uncommitted` the count of status lines that are not blank after `js_trim` (a line of U+00A0 or U+FEFF alone is blank); `memory`; `files`; `remembered: false`; no `provider` key. Key order is wsp-wire `SeedPlan`'s, which is the TS order.
8. `unpushed` (:69-77): against the upstream (first line) or else `origin/<defaultBranch>`; neither is null; no merge base is null; the count is `Number(first line or "0")` (JavaScript's rules: `0x3` is 3, `""` is 0); a non-integer, a negative or 0 is null; else `{commits, base}`.
9. `memory` (:82-88): `claude_project_key(folder)`, `claude_memory_dir(state_home, key)`; not a directory is null; else `{key, files: walk(dir).len(), bytes: stat.bytes}`.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/seed.json` (`oracle/w2-seed.oracle.ts`, 45 menus with every call), replayed by
`tests/seed.rs` `w2_oracle::every_menu_is_the_plan_the_ts_wrote_with_the_same_calls` (plan, key order and calls).

- W1. du parsing of rule 4, the inner junk and never paths subtracted without rows (rule 5), a blank first line of a
  git answer is nothing and never falls through to the second, `defaultBranch` `""`, the commit count by `Number()`,
  blank status lines by `js_trim`, the memory key and file count, `localeCompare` order.
- W2. The fractional-kilobyte case is skipped: the TS keeps `4403.2`, and wsp-wire's `SeedFile.bytes` is an `i64`.
  `du -sk` prints whole kilobytes, so no real answer reaches it; flagged for the wsp-wire owner.

## Edge cases

- A folder git answers nothing for: no `du`, empty files, branch `""`, every nullable null (oracle 1).
- A trailing slash on the folder stays in `source` and in `git -C`, and the du prefix is built without it (oracle 2).
- A rev-list count of `two` is no unpushed commits (oracle 2).

## Rules pinned

| Rule | Test |
|---|---|
| 1, 2, 3, 4, 5 | `shows_every_path_git_ignores_sized_once_*`, `names_the_row_that_judged_each_path_*`, oracle menus (runs column) |
| 6 | oracle menu 2 |
| 7, 8 | `reads_the_branch_the_remote_s_default_*`, `counts_the_commits_the_remote_does_not_have_*`, `answers_no_remote_*`, oracle menus 2 and 3 |
| 9 | `carries_the_agent_s_memory_folder_*`, `carries_no_memory_row_*` |
| no content read | `reads_no_file_s_content_anywhere` |
