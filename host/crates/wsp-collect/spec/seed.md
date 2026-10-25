# seed

Source: `packages/collect/src/seed.ts` (133 lines) at `a3b870a6e`. Tests: `seed.test.ts` (8 cases). Rust: `src/seed.rs`; tests `tests/seed.rs` (8), `tests/oracle_cases.rs` `seed::*` (1: 3 menus, text and runs against the TS).

## What it does

The seed menu for a folder on this computer: every path git ignores there, sized in one `du`, each judged by the catalog's seed rows, plus the branch, the remote, the commits the remote lacks, the uncommitted edits and Claude Code's memory folder. No file's content is read.

## Behaviour

1. Every git call is `git -C <folder as given> <args>` with `timeout_ms` 120,000 (`CALL_MS`); `du` too. The calls run in this order: the listing; one inside-listing per opened entry; `du` (only when anything was listed); `remote get-url origin`; `rev-parse --abbrev-ref HEAD`; `symbolic-ref refs/remotes/origin/HEAD`; `rev-parse --abbrev-ref @{upstream}`; `merge-base HEAD <against>` and `rev-list --count <base>..HEAD` when reached; `status --porcelain` (seed.ts:24-62, 69-77).
2. Listing (:25): `ls-files -z -o -i --exclude-standard --directory`, split on NUL, empties dropped; an entry ending `/` is a directory.
3. Opened (:27-33): a directory entry is opened when some seed row's `opens` name equals the entry or the entry ends with `/<name>` (`opens`); then `ls-files -z -o -i --exclude-standard -- <entry without trailing slashes>`, keeping only paths `seed_row_for(path, false)` judges.
4. Sizes (:102-118): one `du -sk` over every listed path and every kept inner path, each as `<folder without trailing slashes>/<path without trailing slashes>`; each answer line `<kb>\t<path>` with a finite number and a path under that prefix maps the relative path to `kb × 1024`; any other line is skipped. A path with no size is 0.
5. Rows (:40-47, 93-98): for each listed entry in listing order, first its kept inner paths (not dirs), then the entry itself with its own size less the inner sizes, floored at 0. `row_for`: a `junk` judgement drops the row; no judgement is kind `unknown` with no `row`; else the kind and `row: {id, name}`; `ticked` for `config` and `secret`.
6. Order (:48): rows sorted by `localeCompare` (`wsp_words::js::collate`), so `Zeta` sorts after `b.log`.
7. Plan (:53-63): `source` the folder as given; `remote` the first JS-trimmed line of the origin URL or null; `branch` that of HEAD or `""`; `defaultBranch` the symbolic ref without `refs/remotes/origin/`, or null; `unpushed`; `uncommitted` the count of status lines that are not blank after a trim; `memory`; `files`; `remembered: false`; no `provider` key. Key order is wsp-wire `SeedPlan`'s, which is the TS order.
8. `unpushed` (:69-77): against the upstream (first line) or else `origin/<defaultBranch>`; neither is null; no merge base is null; the count is `Number(first line or "0")`; a non-integer or 0 is null; else `{commits, base}`.
9. `memory` (:82-88): `claude_project_key(folder)`, `claude_memory_dir(state_home, key)`; not a directory is null; else `{key, files: walk(dir).len(), bytes: stat.bytes}`.

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
