# run_block
Source: the functions of `packages/protocol/src/run-block.ts` (142 lines) at a3b870a6e; its constants (`SHELL_FENCES`, `RUN_OUTPUT_MAX_CHARS`, `RUN_WORDS`, `RUN_SENT_LINES` and the rest) are `wsp_wire::types::run_block`. Spec chapter 13 carries this unit as its worked example. Tests: `packages/protocol/test/run-block.test.ts` (12 cases; 1 ignored as constants alone), plus `pinned` in `tests/run_block.rs` (every value printed by the TS under node).

## What it does
A reply's shell block that a person can run where it stands: which labels run, the command a block holds, the tail of a run's output, the lines a terminal's bytes leave, and the words a finished run is said in.

## Behaviour
1. `runnable_command(language, code)`: the label is `js_trim` then lower-cased and must be sh, bash, zsh, shell or console; `\r\n` becomes `\n`; any HIDDEN char (U+0000-0008, U+000B-001F, U+007F, U+00AD, U+061C, U+200B-200F, U+202A-202E, U+2060-2064, U+2066-2069, U+FEFF) refuses the block; a console block runs the lines that start with `$ ` (their rest), or the whole text when none does; the command is `js_trim`med; "" is None (run-block.ts:15-28).
2. `run_output_tail(text, max)`: the text when <= max units; else the last max units, then everything after the first "\n" in them where there is one (run-block.ts:38-43).
3. `terminal_text(raw)`: walk UTF-16 units: ESC `[` skips to the first unit in 0x40-0x7E; ESC `]`, `P`, `_`, `^` skip to BEL or ESC `\`; any other ESC skips intermediate bytes 0x20-0x2F; `\r` moves to column 0 (later text overwrites); `\n` ends a line; `\b` moves back one and truncates the line there; other controls below space except tab are dropped. Trailing lines blank by `js_trim` are dropped, each line's trailing JS whitespace is cut, lines joined by "\n" (run-block.ts:48-100).
4. `run_ended_words`: moved, lost, running each a fixed line; a signal `Stopped by signal N`; else `Exited <exit code or 0>` (run-block.ts:120-126). `run_output_message`: `it exited N` for an exit with no signal, else the ended words lower-cased; blank output says it printed nothing; else the last 60 lines in a `text` fence one backtick longer than the longest run inside, at least three (run-block.ts:133-141).

## Edge cases
- `"  $ ls"` in a console block is not a prompted line: the whole text runs, `$ ls`. A tab is not hidden.
- `run_output_tail("abc", 0)` is "abc" (`slice(-0)` is the whole string, then no newline); `utf16::tail` is not the right helper for max 0.
- `terminal_text("progress 10%\rprogress 99%\rdone\n")` is "doneress 99%": `\r` overwrites, it does not clear.
- A cut or a `\b` inside a surrogate pair: node keeps the lone half; the port drops it (rule 8.5, `the_tail_starts_at_a_line_and_counts_units`, `a_terminals_bytes_become_the_lines_a_person_saw`).

## Behaviour nobody wrote down
- Rules 8.4, 8.5, 8.6: units, never half a pair, JavaScript's whitespace.

## Tapes
`sessions.run` and the thread's run records; the reply blocks every client draws.

## Decisions
1. HIDDEN is a char match over BMP ranges, the same set as the TS's unit regex.
2. `RunEnd { state, exit_code, signal }` with `RunState` stands for the TS run object.

## After the port
- `run_ended_words` says "Exited 0" for an exited run with neither code nor signal.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `which_of_a_reply_s_code_blocks_a_person_can_run::*`, `only_a_shell_fence_with_no_hidden_character_runs` |
| 2 | `the_tail_starts_at_a_line_and_counts_units` and the TS tail cases |
| 3 | `a_terminals_bytes_become_the_lines_a_person_saw` and the TS terminal cases |
| 4 | `how_a_run_ended_and_what_send_output_says` and the TS message cases |
