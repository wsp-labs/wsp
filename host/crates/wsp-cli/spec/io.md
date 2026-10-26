# io

Source: `packages/host/src/verbs/io.ts` (463 lines) at 65f6b89c7. Tests: `project-bundle.test.ts` (plan words, 30; the plan reads are the host's), `host-folders.test.ts` (17; host side), `verbs-exec-host.test.ts` (exec, 5), `parity.test.ts` (SEND_MEETS, CAP_MEETS). Rust: `src/verbs/io.rs`; tests `tests/io_oracle.rs` (from `tests/fixtures/io.json`), `tests/io_roads.rs`.

## What it does
The three roads that follow frames the host pushes (exec, export, import), the plan of an import as lines and the person's ticks, the one confirmation every line that takes something away asks, the tool door's output shapes and result builders, the shared tool inputs several verbs reuse, and small flag readers.

## Behaviour
1. `exec_on` (68-86): `PushedFrames` made before `workspaces.exec {workspaceId, argv, cwd?}` is sent, so frames pushed ahead of the reply are held; after the reply, frames with type `exec.output` or `exec.exit` and the reply's `execId` go to `on` in order; the first exit ends it and later frames reach nobody; raced against the socket (`until_settled`); the hold stops on every road out. `ran_in` is the reply's `cwd`, absent where it named none.
2. `export_project` (99-121): hold, `events.subscribe` (once), then follow `project.export` frames whose `workspaceId` and `dest` match; the request `{workspaceId, source, dest, replace?, agents?}` (each only where given, `false` included) raced against the socket; answers `exported`.
3. `import_project` (130-143): as export, matching `workspaceId`, `source` and `dest`; request `{workspaceId, ...req}`.
4. `secrets_chosen` (147-156): keep paths, then cut paths, each must be a secret the plan lists; the first that is not is refused `<path> is not a secret-shaped file in the plan, which lists none.` or `...; the plan lists a, b.` with `Name one the plan lists, or drop the flag.`; then default ticks plus keep less cut.
5. `agents_chosen` (159-166): None is `default_agents`; each named agent must travel (`can_travel`), else refused naming the travelling ones.
6. `plan_lines` (171-185): the table of facts in that order; `plural`, `fmt_bytes`, `secret_signals_line`, `secret_offer(s, ticked).full`.
7. `with_replace_hint` (194-197): kind `exists` only gains `\nRun again with --replace to overwrite it.`, kind kept.
8. `agents_flag` (200-208): split `,`, trim, drop empties; none left refused; the first unknown id refused with `unknown_agent_line(id, catalog ids)`.
9. `confirmed_at` (217-223): `--yes` true; off a terminal usage refusal `<first line of the question> There is no terminal to answer on.` with `Pass --yes to say yes.`, nothing asked; at a terminal one `ask`, "yes" true, anything else `<name> kept` on stderr and false.
10. Output shapes (231-269): `TurnOut` keys threadId, workspaceId, harness, text?, outcome, capped?, afterCut?, notice?; `RunOut` adds `sub` last; `ThreadEndOut`, `WaitOut`, `ThreadRowOut` (the row then projectName, folder, branch, computerName).
11. `as_json` text is `json_line(v, 2)`; `as_text` the text given; both carry the value as `structuredContent`.
12. `turn_text` (279): notice, AFTER_CUT_LINE when afterCut, text or "", the absent ones dropped, joined by newlines. `noticed` sets `notice` only when given.
13. `detached_out` (285-288): `[said, opened_thread_line(id, opened, cwd)]` joined by newline, plus `\n<cap_wait_line>` on a held start with `capped`; value is the turn view with `sub` and `notice` where given.
14. `wait_answer` (305-308): an end is `{finished: end_view}` and `notify_line(id, result, length)`; a timeout `{timedOut: true}` and `wait_timed_out_line(ids, ms)`. `end_view` (290-300) carries duration and cost only where present, `reply` = `notify_tail(result)`, `result` = the sub's.
15. `timeout_flag` (311-316): JS `Number`, finite and above 0, times 1,000; "" and " " are 0 and refused.
16. `turn_out` (319-323): the turn's refusal (`turn_refusal`, auth for a sign-in refusal) else its view.
17. Shared inputs (328-402): each field's name, kind, optionality and describe as `io.json` `shared` records; `RunProjectIn` and `ProcessesIn` keep their cloud span until `in_cloud_words` cuts it.
18. `host_folders` (434-446): `on` looked up first (one `places.list`); `here` (the place id `here`) reads the folder through the caller's `here`; another computer needs an absolute folder (`folder is a path on <name>`), refused before the listing; the request carries `dir` where a folder was given, `hidden`/`repos` only when true, `on` only for another computer.
19. `init_setup` (449-452): `init.get {}`, parsed against `InitSetup`; a shape it refuses is an error.
20. `folder_lines` (456-461), `scheme_flag` (408-413), `projects_flag` (416, node `path.resolve` against the caller's folder), `print_table` (423-428), `progress` (419), `REASON_FLAG` (463).

## Edge cases
Every row of `io.json`: empty plans, an agent with an error, ticks over keep and cut, every timeout word, agent lists with blanks and unknowns, the five confirmation roads, six turns (held, steered after a cut, failed, sign-in refusal), four turn texts, five waits.

## Behaviour nobody wrote down
- Rule 10.8: a line that takes something away asks only after the host's own refusals passed; the question is refused off a terminal.
- Rule 10.20: every follower races the socket, so a host going away is an error, never a hang.

## Decisions
1. `projects_flag` takes the folder to resolve against; the TS reads `process.cwd()`.
2. The sets `secrets_chosen` and `agents_chosen` answer are ordered sets; the TS `Set` keeps insertion order, which no caller prints.

## After the port
- None found.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `io_roads::exec::*` (5), `threads_lines::exec::*` (5) |
| 2, 3 | `io_roads::export_and_import::*` (5) |
| 4, 5 | `secrets_chosen_takes_the_plans_default_ticks...`, `agents_chosen_takes_the_default...` |
| 6, 20 | `plan_lines_are_the_oracles_lines`, `folder_lines_are_the_oracles_lines`, `scheme_flag_*`, `projects_flag_*`, `reason_flag_*` |
| 7, 8, 15 | `with_replace_hint_*`, `agents_flag_*`, `timeout_flag_*`, `project_folders_*` |
| 9 | `confirmed_at_asks_once_at_a_terminal_and_refuses_off_one`, `threads_lines::taking_away::*` |
| 10-14, 16 | `turn_out_*`, `detached_out_*`, `turn_text_and_noticed_*`, `wait_answer_*`, `as_json_and_as_text_*`, `the_words_a_start_and_an_import_say` |
| 17 | `every_shared_tool_input_is_the_field_the_oracle_recorded`, `pick_inputs_*`, `send_inputs_*` |
| 18 | `io_roads::folders::*` (5) |
| 19 | `io_roads::setup::*` |
