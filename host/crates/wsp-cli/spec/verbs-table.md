# verbs-table

Source: `packages/host/src/verbs.ts` (764 lines, of which 1-364 are imports and re-exports and 440-604 the `FLAG_WORDS` data, copied by script) at 65f6b89c7, plus node 22's `util.parseArgs` as the line calls it. Tests: `parity.test.ts` (29; 8 stand on this file alone), `pages.test.ts` (14; 2), `refusals.test.ts` (9), `contract.test.ts` (19; the pre-dial refusals), `cloud.test.ts` (17; 4), `project-flag.test.ts` (7). Rust: `src/verbs.rs`, `src/parse_args.rs`; tests `tests/verbs_oracle.rs`, `tests/run_verb.rs`, `tests/table_laws.rs`.

## What it does
Joins the seven groups into `ALL_VERBS`, cuts the cloud's verbs, flags and spans for a process with the cloud off (`VERBS`), finds the verb a line opens with, prints a verb's help, and runs a verb: parse, help, state, aim, the lazy dial, the run, the one failure line, the close.

## Behaviour
1. `all_verbs` (366-374): computers, agents, projects, workspaces, threads, conversations, activity. 86 entries, 81 tools after the cut.
2. `verbs_for(on)` (376-399): on keeps all; off drops every `cloud` entry and runs `without_cloud_flags`; then `in_cloud_words` cuts `<!-- cloud -->` spans in usage, about, the tool's description and (a Rust decision) each input's describe. 78 entries and 74 tools off, 86 and 81 on.
3. `without_cloud_flags` (379-387): only Cli and CliOnly entries with cloud flags: drop each from options; drop from the usage the first ` [--<flag>\b[^\]]*]` match; drop tool inputs whose name with `_` as `-` is a dropped flag.
4. `cloud_line_of(argv, on, built)` (404-412): None when on or not built; the longest-named entry of `all_verbs` whose words open argv; a cloud entry is `wsp <name>`; else the first cloud flag typed as `--f` or `--f=...` is `wsp <name> --f`.
5. `find_verb` (418-423): line entries only (a ToolOnly name like `projects add` falls to `projects`); longest name in characters first, ties in table order; words compared exactly (case kept).
6. Help (426-681): `HELP_WIDTH` 80; `usage_lines` wraps each usage line on its own with spaces inside brackets held as U+00A0; `verb_help` prints usage lines at four spaces and the about at six; `help_page` pads flag words to the widest plus 4, wraps under width+2; `verb_page` lists own flags (`flag_says` or ""), then --json, --state, --host with the words handed in; `verb_usage` joins `usage: <usage>` of every verb whose first word matches.
7. `flag_says(verb, name, on)` (608-611): the `"<verb> <name>"` row, else the `name` row, cloud-cut.
8. `parse_refusal` (685-692): an unknown `--x` read by other line verbs is `foreign_flag_line("--x", ["wsp v"...], "wsp this")`; else node's message; fix `usage: <usage>`; usage class.
9. `run_verb` (694-764), in order: parse `optional_values(argv after the verb's words)` against COMMON plus the verb's options, positionals allowed, then `absolute_folder(--cwd)`; any failure is `failed(io, json_asked(argv), parse_refusal)` with no prefix. `--help` prints `verb_page` (hostSide words where the entry has `host_side`) and returns 0 before the state is picked. Then `state_path_of(--state)` once; `aimed_host(state, {host, env})` once, a failure printed behind `wsp <verb>: `. A host-side line aimed elsewhere refuses `host_side_only_line(name, aim_name)` + `host_side_only_fix(host_side)` with no prefix. The state note (`state_ignored_line(aim_name)`) goes to stderr once, at the first dial, only for an aim elsewhere with `--state` given.
10. The dial (746-755): the first plain `client()` dials with `{aim, say: io.error, any_release if the entry has one, start unless starts_no_host}`; later plain calls answer the same socket; `client(Some(again))` dials anew with `deadline_ms = within_ms` and no start, and becomes the socket. `deps.dial` overrides `dial_host`.
11. The run (757-763): its exit code stands; an error prints `failed(io, json, host_schema_refusal(e, usage) ?? e, "wsp <verb>: ")`; the socket, if one was opened, is closed on every road out.
12. The parser (`parse_args`): node 22's strict parse with positionals; values keyed in first-met order, last value wins, `multiple` collects; `--` ends options; node's messages byte for byte (see `src/parse_args.rs` and `parse_args.json`, 33 cases).

## Edge cases
- Pre-dial refusals of the thread verbs, exit codes and dial counts: `run_verb.json` (37 rows). `run --size 2x4 hi` dials before it refuses (see threads-a).
- `exec --help` and `exec -h` print the page; `thread read x --help --nope` is a parse refusal (the parse runs before help).

## Laws (named tests)
Tool names unique under the naming rule; every tool has output fields; every own flag has a help row and no row for a flag nobody reads; every verb that takes something away reads `--yes` alike; every verb page fits 80 columns; every list input is a `multiple` flag or says why; `--notify` once or twice is a list; with the cloud off no line, page or tool names the provider or keeps a mark (packages/host/AGENTS.md invariant 7).

## Behaviour nobody wrote down
- Rule 10.1 (longest match after the shared flags are lifted), 10.3 steps 1-4, 10.21 (no signal handler: nothing here installs one).

## Decisions
1. The cloud switch is a parameter (`verbs_for(on)`, `cloud_line_of(argv, on, built)`); `verbs()` reads `crate::cloud` once for this process.
2. Input describes keep their cloud spans in the table and are cut in `in_cloud_words`, where the TS cuts them at module load: `VERBS` reads the same either way; `ALL_VERBS`' describes differ (raw in Rust), which no output shows.
3. `run_verb` takes the table, since the parse refusal reads the other verbs.

## After the port
- A parse refusal is printed without the `wsp <verb>: ` prefix while every later refusal has it.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `verbs_oracle::the_table_has_the_fixtures_entries_in_order` and the five `every_*` field tests, `all_verbs_is_every_group_in_order...`, `table_laws::a_cloud_verb_keeps_its_place...` |
| 3 | `the_cloud_off_table_holds_no_cloud_verb_and_no_cloud_flag`, the field tests on `verbs.json` |
| 4 | `cloud_line_of_names_the_cloud_line...`, `cloud_line_of_is_none_in_a_public_build...` |
| 5 | `find_verb_answers_the_longest_name_the_line_opens` |
| 6, 7 | `verb_page_is_the_whole_help_of_every_line_verb_byte_for_byte`, `verb_help_*`, `verb_usage_*`, `usage_lines_*`, `help_page_*`, `about_lines_*`, `flag_says_*`, `own_flags_of_*`, `flag_words_is_the_raw_table...` |
| 8 | `run_verb_answers_the_exit_code...` (`--ssh`, `--recipe` rows) |
| 9 | `run_verb_answers_the_exit_code...`, `run_verb::help_prints_the_verb_s_page...`, `help_on_a_line_that_runs_at_its_own_host...`, `a_line_that_runs_at_its_own_host_refuses_any_other_aim...`, `an_empty_host_is_refused...`, `a_state_named_for_a_line_aimed_elsewhere...`, `the_shared_flags_bind_wherever_they_sit...`, `words_after_the_double_dash...` |
| 10 | `a_line_dials_once_lazily...`, `the_starter_s_line_goes_to_stderr...`, `a_line_aimed_elsewhere_dials_that_aim...`, `a_line_that_starts_no_host_dials_without_the_starter...` |
| 11 | `the_socket_is_closed_once_the_run_returns...`, `a_failure_is_one_stderr_line...`, `a_sentence_that_opens_with_the_verb_s_name...`, `a_refusal_the_host_s_validator_wrote...` |
| 12 | `parse_args_answers_node_values_in_order_and_positionals`, `parse_args_refuses_with_node_codes_and_messages` |
| Laws | `table_laws::*` (9) |
