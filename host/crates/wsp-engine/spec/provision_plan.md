# provision_plan (provision.ts:1-250)

The plan off a golden import for a computer somebody owns, and the read of which planned steps that computer already
satisfies. Pure but for `present_steps`, which reads the computer. No state: a plan is rebuilt per job.

## provision_plan_of (:70)

`extras: None` is `{ compiler: true }`; `Some(ProvisionExtras::default())` is `compiler: false`, not the TS default. The plan, field by field:
- `steps`: `agent_steps(installs: imp.agents, node: imp.node, path, prefix)`, then `apt_need_steps` of those, then
  `imp.tools`. `agents` is the count of the first two groups.
- `recipe_at`, `path`, `prefix` as handed; then the extras' fields (they replace `compiler`, and set skills,
  configs, config_tools, plugins, github where given).
- `skipped`: each skipped agent `{ id, label: name, note }`, then each skipped tool `{ id, label, note }`.
- `files`: only when the import has files with at least one landing: `once_per_dest(lands)` (the first landing per
  dest, in order), each marked `once` when its dest is in `once_dests(imp)`; the pack as given.
- `mcp`: the import's, where it has one.

`apt_need_steps(agents, path, prefix)` (:93): per agent step in order, with `agent = agent_of_row(step.id)` (else ""):
each `apt_need_rows(agent)` row through `via_road({ road: apt, packages: [package] }, command, path, prefix)`; a
`Note` answer is dropped; a `Step` is `{ id: need.id, label: package, manager: apt, cmd, shown, bin: need.command,
after: the agent step's id }`. Then each `setup_need_rows(agent)` row as `{ id, label, manager: apt, cmd:
path_line(path, prefix) + "\n" + need.cmd, shown: label, check: need.check, after: need.after }`.

`once_dests(imp)` (:110): the MCP plan's scope files under `guest_home/`, home-relative; plus every recipe file row
marked volatile whose id is an agent's own row (`agent_of_row`). A login's file is never once.

`provision_counts_of` (:122): tool = steps; file = files' landings (0 without); server = per MCP agent its aside rows
plus per scope keep and drop names.

## presence_tests(step, versions) (:149)

In this order, texts byte for byte (oracle `presenceTests`):
1. `( <present or else check> ) >/dev/null 2>&1` where either is set (`present` wins).
2. `command -v <q bin> >/dev/null 2>&1` where `bin` is set.
3. A version read where `pin.read` is set and (`asks` is set or no test came before): `read` is `"$( ( <pin.read> )
   2>/dev/null | head -n 1 | tr -d '[:space:]' )"`. With `Any`, or no `asks`: `[ -n <read> ]`. Else for a row that is not
   an agent's own: `[ <read> = <q asks> ]`; for an agent's row (newer is kept): `printf '%s\n' <q asks> <read> | sort
   -V -C`. With `below` set and `Asked`: `if <node_at_least(below.node)>; then <is(asks)>; else <is(below.asks)>; fi`
   (an absent `below.asks` is `[ -n <read> ]`).
A step with none of these has no test.

## present_steps(machine, steps, path, prefix, versions) (:177)

Only steps with tests are asked, through `paged_reads(machine, rows, line, path_line(path, prefix))`, where row `at`'s
line is `if <tests joined by " && ">; then printf 'wsp-present %s %s\n' <at> "<$(command -v <q bin> 2>/dev/null) where
bin is set, else empty>"; fi`. A page whose exec exited non-zero says nothing is present in it (those steps install
again). In a page, `markers_of(stdout, "wsp-present")` keyed by the row's index within the page; a marked row is
present with `path` the trimmed rest, absent when empty. Answers id to read in step order. A rejected exec rejects.

## present_by_what_waits (:207), present_elsewhere (:221), row_of (:232), row_line (:246)

- `present_by_what_waits(steps, present)`: ids, in step order, of steps with no presence test that at least one step
  waits on (`after == id`) and all of whose waiters are in `present`. The apt index of a real plan is the case.
- `present_elsewhere(step, read)`: `None` unless the read has a path, the step a `bin` and non-empty `bins`; `None`
  when the path starts with `<dir>/` for some dir in `bins`; else `present_elsewhere_line(bin, path,
  road_module_named(manager).words(), bins)`.
- `row_of(result, present, step)`: outcome `present` where the result installed and the id has a read, else the
  result's; a present row's note is `present_elsewhere(step, read)` (the landing's own note is dropped), any other row
  keeps the result's note; `ms` where the result has it. No `kind`, no `step`.
- `row_line(row)`: `<label>: <outcome word>` plus ` (<note>)` where a note is set.

## Pinned since (R-rules, values from running the TS)

- R1. `files` only where the import has files and its `lands` is non-empty (`lands: []` is `None`, so the mcp step runs
  no files round). The lands are the first per dest in first-seen order; a dest is once by exact equality with a
  server file under `mcp.guest_home` (`.claude` is not once because `.claude/settings.json` is) or a volatile agent
  row's dest; a landing that came with `once: true` keeps it (`once = came || in set`, never `once = in set`).
- R2. `steps` is every agent install (node first), then every need row of those installs, then `imp.tools`; codex's
  need rows follow claude when claude comes after codex. `agents` = installs + need rows exactly: the agents and clis
  steps slice `steps` there.
- R3. `skipped` is the skipped agents then the skipped tools. No extras: `compiler: true`, no optional field set. Given
  extras: their `compiler`, and only the fields they set (`github: Some([])` stays `Some([])`). `server_tools` is never
  set here.
- R4. `present_steps` is one exec per page of `READS_PER_EXEC` (8) reads, `timeout_ms` 20,000 (`INLINE_EXEC_MS`), the
  text the TS's exactly: `path_line(path, prefix)`, then per step that has a test `if <tests joined by " && ">; then
  printf 'wsp-present %s %s\n' <i> "<$(command -v <q bin> 2>/dev/null) or empty>"; fi`, the `\n` the two characters.
  `<i>` counts only steps with a test, from 0 on each page. No step with a test: no exec.
- R5. A page whose exec exits non-zero reads nothing present in it, whatever it printed; the next page is still read; an
  exec that rejects rejects the call and keeps nothing.
- R6. Markers: each line trimmed, then split on single spaces: the mark, the index, the rest rejoined by single spaces
  (an inner double space survives) and trimmed; a line not starting with the mark, a non-numeric index or one outside
  the page is ignored; the last marker for an index wins; the answer is in step order (a repeated step id keeps its
  first place and its last read).
- R7. `present_by_what_waits` reads only the set it is given (one pass, never its own answers): a chain of two steps
  with no test is not satisfied end to end. Each id once.
- R8. `row_of`: `present` only over an `installed` result with a read; the result's own note is then dropped (the
  elsewhere note or none); a `failed` or `skipped` result stays as it is. `note: Some("")` and `ms: Some(0)` are kept.
  `row_line` prints `<label>: <outcome>` then ` (<note>)` whenever note is `Some`, `()` for an empty one.
- Presence tests, four more branches now in the oracle's case list (`presenceTests`, 24 cases): `below` with no `asks`
  of the step's own (the then-branch is `[ -n <read> ]`); the agent form on both sides of `below`; `below` without
  `asks` on an agent row (the else-branch is `[ -n <read> ]`); agent-ness is `agent_of_row(id)` (`agents/node` and
  `agents/codex/bubblewrap` are agent rows, `agents/mcp/claude/x` is not); empty strings are values (`check: ""` is
  `(  ) >/dev/null 2>&1`, `bin: ""` is `command -v ''`, `asks: ""` compares to `''`); a pin with no `read`, or no pin,
  adds no version test; a `read` holding a newline is spliced as it is.

## Rules against tests

| Rule | Tests |
|---|---|
| R1 to R8 | provision_plan_hard.rs: `the_plans_files_are_one_per_dest_once_by_exact_dest_and_none_when_empty`, `need_rows_follow_every_agent_install_not_their_own_agent`, `skipped_rows_then_extras_overlay_only_what_they_set`, `the_presence_read_is_one_exact_exec_whose_index_counts_only_steps_with_a_test`, `a_failed_page_reads_nothing_the_next_page_counts_from_zero_and_a_rejection_rejects`, `markers_are_trimmed_the_last_wins_and_the_answer_is_in_step_order`, `the_waiters_rule_is_one_pass_over_the_set_given`, `row_of_reads_present_only_over_an_install_and_row_line_prints_an_empty_note` |
| presence branches (24 oracle cases) | provision_pins.rs: `each_presence_test_is_the_ts_text_in_its_order_for_asked_and_for_any` |
| exact tests per case, asked and any | provision_pins.rs: `each_presence_test_is_the_ts_text_in_its_order_for_asked_and_for_any` |
| check passes, command answers, version read; a version-only step; formula by its link | provision_plan.rs: the presence describe's first, version-only and formula cases |
| a plan's every step present on a second run; nothing answered is nothing present; nothing readable asks nothing | provision_plan.rs: "is every step of a plan the computer already answers ...", "is nothing at all when the computer answers nothing ..." |
| waits: the apt index alone, every waiter needed | provision_plan.rs: the two waits cases; provision_pins.rs: `a_step_with_nothing_to_read_has_no_test_and_present_by_waiters_needs_every_waiter` |
| elsewhere note: named outside, silent inside, kept on a present row | provision_plan.rs: the two elsewhere cases; provision_pins.rs: `a_command_outside_its_roads_folders_is_named_and_a_lookalike_folder_is_outside` |
| bubblewrap and the bwrap profile rows after Codex | provision_plan.rs: the three bubblewrap cases |
| the PATH line and knobs on every script | provision_plan.rs: the PATH describe's pure cases |
| counts and one landing per dest | provision_pins.rs: `the_counts_are_steps_landings_and_every_server_aside_kept_and_dropped`, `one_landing_per_dest_keeps_the_first_row_that_named_it` |
