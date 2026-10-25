# shell_and_text
Source: at a3b870a6e, `packages/protocol/src/quote.ts` (12), `regexp.ts` (4), `ps-time.ts` (17), `tool-result.ts` (56), the functions of `oom.ts` (44), `thread-shell.ts` (31), the sentence and script functions of `thread-cgroup.ts` (85; `cgroupMemory` and `inCgroup` are `wsp_wire::types::thread_cgroup`) and `docker-shim.ts` (130): 379 lines. Their flat constants are `wsp_wire::types::<file>`. Tests: `tool-result.test.ts`, `oom.test.ts`, `docker-shim.test.ts` and `format.test.ts:328` (19 cases; 1 ignored as constants alone; the docker-shim cases run the script under `/bin/sh`), plus `pinned` in `tests/shell_and_text.rs` (every value printed by the TS under node).

## What it does
Small text and shell helpers the runtime and the daemon share: one line and a fence for quoted words, a regex-escaped word, ps's cpu column, an edit's hunks and the part of them that fits a count, the oom and nice line a turn's processes start under, the shell startup files that keep a thread's folder first on PATH, a thread's cgroup and the script that ends it, and the docker that tags a thread's containers.

## Behaviour
1. `one_line`: every run of JS whitespace becomes one space, then `js_trim` (quote.ts:6). `fence_for`: backticks one longer than the longest run in the text, at least three (quote.ts:9-12). `escape_reg_exp`: a backslash before each of `.*+?^${}()|[]\` (regexp.ts:4).
2. `ps_cpu_seconds(time)`: days from the text before the first "-" (`Number`, "" is 0), then each ":"-part of the rest folded `s*60 + Number(part)`; a total that is not finite or is negative reads 0 (ps-time.ts:10-17). `Number` is JavaScript's: JS whitespace trimmed, "" is 0, a decimal or exponent literal, anything else NaN.
3. `whole_file_hunk(content, added)`: lines are content less one trailing "\n", split on "\n" ("" has none); start is 1 with lines, 0 without; each line signed "+" or "-" (tool-result.ts:8-15). `unified_hunks(diff)`: one trailing "\n" off; a line matching `^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@` opens a hunk (a missing count is 1); other lines join the open hunk; lines before the first head are dropped (:20-28).
4. `kept_patch(patch, chars)`: whole hunks while the running count of line units fits; the hunk that overruns keeps its lines up to the one that would pass the count, its counts recomputed (oldLines: lines not starting "+" or "\"; newLines: not "-" or "\"), kept only if it has a line; then the patch stops with `patchCut: true`; files keep their other fields (movedTo) (:31-56).
5. `work_score_line(adj)`: `{ echo <adj> > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1`, adj 500 by default; `work_argv(file, args)`: `/bin/sh -c "<line>; exec \"$0\" \"$@\"" file args...` (oom.ts:27-37).
6. `thread_shell_files(dir, env)`: the four zsh files in order `.zshenv .zprofile .zshrc .zlogin`, each body the eight lines of thread-shell.ts:20-24 with the person's own ZDOTDIR (else `$HOME`), then `.bash_env` sourcing the person's BASH_ENV first where one is set and is not this file; an empty value reads as unset (thread-shell.ts:16-31).
7. `thread_cgroup(id)`: THREAD_CGROUPS, "/", the id with every UTF-16 unit outside `[A-Za-z0-9_-]` as "_" (thread-cgroup.ts:9-11). `cgroup_join_line`, `cgroup_end_script(cgroup, remove)` and `thread_cgroups_end_script(under)` are the TS scripts word for word; the end function's text is copied by script into the stub as `END_FUNCTION` (:44-85).
8. `docker_shim(bin, notes)`: the script with `{bin}` and `{notes}` replaced by `shell_quote` of each, first occurrence only (docker-shim.ts:23-25); the script text is copied by script into the stub as `DOCKER_SHIM`.

## Edge cases
- `ps_cpu_seconds`: "" and "abc" read 0; "-1:00" reads 60 (days ""); "99:99" reads 6039; " 1:00" reads 60 (`Number` trims).
- `thread_cgroup("é😀")` is `/wsp-threads/___`: three units, three underscores (rule 8.7). A per-`char` replacement would write two.
- `kept_patch` counts UTF-16 units: a line "+😀" is 3.

## Behaviour nobody wrote down
- Rule 8.7: the hunk-head regex and the cgroup character class are ported by hand; the cgroup class works per UTF-16 unit.

## Tapes
None directly; the scripts reach machines through the runtime's spawns.

## Decisions
1. `whole_file_hunk` takes `added: bool` for the TS sign; `thread_shell_files` takes the environment as an ordered map and answers (name, body) pairs.
2. `cgroup_end_script` takes `remove: bool` for the TS options object.

## After the port
- `ps_cpu_seconds("1:2:3:4")` reads 223384: a fourth field is not refused.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `quoting_and_escaping_text` |
| 2 | `the_cumulative_cpu_ps_prints::*`, `ps_cpu_reads_every_column_form_and_nothing_else_as_zero` |
| 3, 4 | `an_edit_s_hunks::*`, `hunks_are_built_parsed_and_cut_by_units` |
| 5, 6 | `who_the_kernel_takes_first::*`, `oom_and_shell_startup_lines` |
| 7 | `a_threads_cgroup_is_named_per_utf16_unit_and_ended_by_one_function` |
| 8 | `the_docker_a_thread_finds_first_on_its_path::*`, `the_docker_shim_quotes_its_two_paths` |
