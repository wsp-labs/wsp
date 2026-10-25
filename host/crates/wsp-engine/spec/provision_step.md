# provision_step (provision.ts:252-551)

One step of a setup on a computer somebody owns. `SetupRun` carries what installed and what the files round landed
from step to step within one setup; nothing of it survives a host restart (a resume starts a fresh run, which reads
every server in a file already there as the agent's own). What survives is on the computer: the two lists
(files_ledger.md, files_outside.md).

## provision_step(machine, plan, step, run, stage, on) (:386)

1. Every row the step reports, staged or returned, carries `step: step.setup_step()`.
2. Only a plan with a `prefix` is bracketed: `walked = outside_roots(plan, step)`; when non-empty, exec
   `outside_before_script(step word, walked, "")` (timeout `OUTSIDE_MS` 120,000, failure ignored) first and
   `outside_after_script` the same way last, also when the step failed part way.
3. Fails only when the computer stopped answering (an `Err` the step's own calls did not catch).
4. The `SharedRun` lock is held only to read or change the run, never across a call to `stage`, a hook, the machine
   or `provision_mcp`: a stage closure may lock the run itself (the CLIs-on-the-run test does, and would hang).

`outside_roots` (:422): for each row `step_installs(plan, step)` gives (floor: `base_installs({}, path, prefix,
left_off(compiler))`; agents: the first `agents` steps; clis: the rest; github: `github`; configs: `config_tools`;
others: none), the road's `roots` and the row's `bins`; each clipped to `/usr/local` or `/opt` (a dir inside a top is
kept, a top inside a dir becomes the top), anything under `/opt/wsp` dropped; unique, sorted, and none inside another.

## The steps (:436)

- **floor**: `install_base(stage lines: each detail, path, prefix, left_off(compiler))`; stage its `line`; `run.base` is
  its tools; rows are only its failed rows `{ id, label, outcome: failed, note }`.
- **agents**: each skipped row as `{ id, label, outcome: skipped, note }`, staged with its row; then the tools step over
  the first `agents` steps.
- **clis**: the tools step over the rest. **github**: over `github`. **configs**: over `config_tools`, then, with
  `configs`, a files round labelled `your configs`, closed.
- **mcp**: with `files`, a files round labelled `your agents' files` left open (`run.landed` is its owned paths). With
  `mcp`: stage `MCP servers: <plural(agents, "agent")>`; `provision_mcp(home, landed: run.landed, tools: run.tools,
  path, held, stores, stage: each detail, into: records file to names)`. Then for each `once` landing whose file got
  names, the row `files/<dest>` that reads present gains `merged_into_line(names)` (joined to an existing note by
  `; `). Stage `provision_servers_line(installed count, all)` and each server row. With `stores`, stage each
  `headless_keys` line for the agents among the plan's agent steps. Always: `close_agent_files(home,
  once_paths_of(files' lands), stores)` and clear `run.landed`.
- **skills**: with `skills`, a files round labelled `your skills`, closed.
- **plugins**: per plugin in order, run `path_line(path, prefix) + "\n" + cmd` (deadline `PLUGIN_MS` 300,000; a
  rejected run is exit -1 with the message on stderr). Exit 0: `installed` with `ms`; else `asked(stdout)` when given and
  `Some`: `skipped` with that note; else `failed` with `last_words(stderr or stdout)` or `exit <n>`.
- **context**: `apply_machine_context(result: { recipe_hash: "", base: run.base, tools: run.tools, agents: [], files
  only when files were skipped }, path, shells: none)`; stage `machine context: <summary>`; no rows.

A files round (:354): stage `<label>: <plural(lands, "path")>`; `provision_files(home, lands, pack, say: stage, stores)`;
stage each row with it; append its skipped to `run.skipped_files`; closed rounds call `close_agent_files(home,
once_paths_of(lands), stores)`.

## The tools step (:296)

1. Nothing to do: no rows, no call.
2. `present_steps(path, prefix)`, plus `present_by_what_waits` ids read as `{}`.
3. `disk_use_size(prefix)`; floor `owned_floor_bytes(size)`.
4. `install_tools(steps, present ids, keep caches, floor, path, prefix)`; each row as the loop reaches it: pushed on
   `run.tools` at once (so a step beside reads it), then its hook (below), staged with `{ label, index, of }` of the next
   step (`None` after the last) and the row.
5. After the loop, each final result replaces its entry in `run.tools` by id (or is appended): `run.tools` holds each
   id once. Rows are the final results through `row_of`; a row whose hook failed stays failed though the loop's check
   read installed. A row whose final line differs from what was staged is staged again (line only).

### The loop's order, pinned (T-rules, held to the TS's run in oracle/strengthen.oracle.ts)

- **T1.** `at()` is `steps[done]`, the step by *count* of rows landed, not the row's own step. A detail line from
  `install_tools` is staged with the step under way (`1/3: bat` while bat installs). A row is staged after `done`
  has counted it, so it carries the *next* step (`bat: installed` is staged with `2/3: Git LFS`), and the last row
  carries `None`. Staging a row with its own step is the wrong choice both pilot ports made.
- **T2.** The loop's own check lines (`bat: the check did not pass: <check>`) and its tally line come after the last
  row, with `None`; then each row whose final line differs from what was staged for it is staged again, line only
  (`None`, no row), in final row order.
- **T3.** A failed hook (`<on>: <last_words>`, here `git lfs install --system: git: 'lfs' is not a git command`, the
  last line of stderr) fails the row as it lands; the final row is that failed row (the loop's result read
  `installed`), so its line matches what was staged and it is not said again.
- **T4.** `run.tools` holds the loop's `ToolResult`s, never the hooked rows: a row whose hook failed still reads
  `installed` there with no note, and the context step reads it so. A row the check corrected reads its final
  result (`failed` with the check's note). Every row returned and staged carries `step`.
- **T5.** `last_words`: trim the whole text, take its last line, cut to 300 UTF-16 units; inner lines are not
  trimmed, so `"  first\nlast of stdout  \n"` gives `last of stdout`. stderr is used unless it is empty (`""`, not
  blank), then stdout; an empty result is `exit <n>`.
- **T6.** A plugin run that rejects reads as exit -1 with the error's message as stderr, so its note is the last line
  of that message (`connection reset`). `asked` is read only on a non-zero exit. Each plugin is staged
  `<label>: <outcome>[ (<note>)]` with its row as it lands, in plan order.

`hooked` (:540): only an `installed` row whose catalog tool entry has a hook (`hook_of`): exec `path_line + "\n" +
on` (timeout `HOOK_MS` 60,000; rejection is exit -1). Non-zero: `failed` with note `<on>: <last_words(stderr or
stdout) or exit <n>>`. A row the box already had (present) runs no hook.

`last_words(text)` (:551): last line of the trimmed text, its first 300 UTF-16 units, `None` for empty.
`project_servers_step` (:367): `provision_mcp` with an empty `landed` and no tools, stage each row, then
`close_agent_files(home, [], stores)`.

## Rules against tests

| Rule | Tests |
|---|---|
| order: floor, presence, steps, context last; no shell on the context probe; floor lines as read | provision_step.rs: the first three "run on the computer" cases |
| one row per step and per skipped row; rows as the checks left them, said again | provision_step.rs: the two rows cases |
| `of`/`index` while it runs; a computer that stops answering throws after its rows | provision_step.rs: the progress and stops-answering cases |
| files and servers on the same run: order, counts, stage lines, left-out rows, failed pack, list keys | provision_step.rs: the "person's own files" describe |
| PATH on every script; no home dir; knobs under the prefix; a planted binary never runs | provision_step.rs: the PATH cases with a shell |
| hook after install only, failed hook fails its row, row said after its hook | provision_step.rs: the two hook cases |
| run.tools: on the run as each lands, once at the end | provision_step.rs: "are on the run the moment each lands ..." |
| plugin asked is skipped with the sha, installed reads installed | provision_step.rs: the plugin case |
| T1 to T4: next step on a row, check lines and re-said rows last, run.tools keeps the loop's result | provision_step_hard.rs: `the_tools_loop_says_each_row_with_the_next_step_and_keeps_the_loops_result_on_the_run` |
| T5, T6: plugin notes, the rejected run, the 300 cut | provision_step_hard.rs: `a_plugins_note_is_the_last_trimmed_line_of_what_it_said_cut_at_300_else_its_exit` |
| compiler only when needed | provision_step.rs: the floor case |
| bracket and walked folders | files_outside.rs: the three bracket cases; provision_pins.rs: `the_folders_a_step_walks_are_each_roads_roots_clipped_to_usr_local_and_opt_without_wsps_own` |
| last words and within | provision_pins.rs: `the_last_words_are_the_trimmed_last_line_cut_at_300_and_nothing_for_blank`, `within_is_the_folder_itself_or_under_it_never_a_lookalike` |
