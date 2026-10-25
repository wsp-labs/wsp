# provision_plan (provision.ts:1-250)

The plan off a golden import for a computer somebody owns, and the read of which planned steps that computer already
satisfies. Pure but for `present_steps`, which reads the computer. No state: a plan is rebuilt per job.

## provision_plan_of (:70)

`extras: None` is `{ compiler: true }`. The plan, field by field:
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

## Rules against tests

| Rule | Tests |
|---|---|
| exact tests per case, asked and any | provision_pins.rs: `each_presence_test_is_the_ts_text_in_its_order_for_asked_and_for_any` |
| check passes, command answers, version read; a version-only step; formula by its link | provision_plan.rs: the presence describe's first, version-only and formula cases |
| a plan's every step present on a second run; nothing answered is nothing present; nothing readable asks nothing | provision_plan.rs: "is every step of a plan the computer already answers ...", "is nothing at all when the computer answers nothing ..." |
| waits: the apt index alone, every waiter needed | provision_plan.rs: the two waits cases; provision_pins.rs: `a_step_with_nothing_to_read_has_no_test_and_present_by_waiters_needs_every_waiter` |
| elsewhere note: named outside, silent inside, kept on a present row | provision_plan.rs: the two elsewhere cases; provision_pins.rs: `a_command_outside_its_roads_folders_is_named_and_a_lookalike_folder_is_outside` |
| bubblewrap and the bwrap profile rows after Codex | provision_plan.rs: the three bubblewrap cases |
| the PATH line and knobs on every script | provision_plan.rs: the PATH describe's pure cases |
| counts and one landing per dest | provision_pins.rs: `the_counts_are_steps_landings_and_every_server_aside_kept_and_dropped`, `one_landing_per_dest_keeps_the_first_row_that_named_it` |
