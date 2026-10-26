# commands
Source: `commands.ts` (185 lines) at a3b870a6e. Tests: `test/commands.test.ts` (10 cases). Rust:
`tests/detect_commands.rs`. Calls `crate::history::commands::{command_words, is_version_check, split_commands,
without_heredocs}` (the other half of collect): its tests go red with them until both are implemented.

## What it does
`called_commands` lists the commands an rc file calls, for the pack to guard the ones the machine lacks.
`guard_commands` prepends one block defining a guard function per missing name, plus notes; `drop_plugins` takes
plugins out of oh-my-zsh's `plugins=( )` lists. All three are pure over text.

## Behaviour
1. `called_commands(text, Fish)` is `[]` (commands.ts:87).
2. Own names (never listed): every FUNCTION_HEAD match over the whole text, multiline, group 1 or 2 (commands.ts:93);
   each `name=` of an `alias` command; each non-`-` word after `autoload` (commands.ts:96-111).
3. The text goes through `without_array_literals`, then the history parser's `without_heredocs`, then `split_commands`;
   each command is split on JS `\s+` into words (commands.ts:94-95). An `alias` command: for each word `i` (1-based in
   the slice) matching `^([\w.+-]+)=(.*)$`, the body is group 2 then the following words up to the next such token,
   joined by a space, and its call is `call_of(body)` (commands.ts:96-106). An `autoload` command adds own names only.
   Every other command adds `call_of(cmd)`.
4. `call_of` (commands.ts:74-80): the history parser's `command_words`; no head, a version check, a head not matching
   WORD, or a head in RC_BUILTIN is no call; a command any of whose `\s+` words ends with `/<head>` is no call (a path).
5. Calls are kept once each in first-call order, then every own name is filtered out, wherever it was defined.
6. `without_array_literals` (commands.ts:33-71): outside an array, a backslash outside single quotes copies itself and
   the next unit; quotes toggle; an unquoted `#` at index 0 or after one of `\s ; & | (` copies the rest of the line
   (the comment stays, unparsed) and resumes at the newline; a `(` right after `=` whose previous unit matches `[\w\]+]`
   opens an array at depth 1 and is copied. Inside, nothing is copied: a backslash outside single quotes skips the next
   unit, quotes toggle, an unquoted comment skips to just before the newline, `(` deepens, the `)` that brings depth to 0
   is copied. An array left open drops the rest of the text.
7. `drop_plugins` (commands.ts:125-156): line endings are `\r\n` when the text holds one, else `\n`; lines split on
   `\r?\n`. A line outside a list that does not match `^\s*plugins\+?=\(` is kept. In a list line, `code` runs up to
   and including the whitespace before the first `#` that starts a word (`(^|\s)#`), `comment` is the rest; the list
   stays open while `code` holds no `)`. `code` splits on `(\s+|[()])` keeping the separators, empties dropped; each
   part matching WORD with `drop(part)` true is removed with the whitespace part after it, or else the one before it;
   a dropped name is recorded once in first-drop order. A line whose kept code is blank and that has no comment and had
   code is removed; a blank kept code with a comment becomes `<indent><comment>`; else `<kept><comment>`.
8. `guard_commands(text, missing, dropped)` (commands.ts:176-185): line endings as in 7. An earlier block, from the
   first line equal to GUARD_BEGIN to the first GUARD_END at or after it, is removed, with one empty line right after
   it. With no missing and no dropped names the rest comes back joined. Else the block is GUARD_BEGIN, one guard line per
   missing name in order (commands.ts:171, byte for byte), one `# plugin <name> left out: <name> is not on the image`
   per dropped name, then when any are missing `# Guarded above: a call to one of these that is not on this machine is
   silent instead of an error: <names joined by ", ">.`, then GUARD_END and an
   empty line, then the text.

## Edge cases
- `called_commands("", Sh)` is `[]`. A GUARD_BEGIN with no GUARD_END after it removes nothing.
- `guard_commands` on a CRLF file writes the block with `\r\n` too.

## Decisions
1. `drop` is `&dyn Fn(&str) -> bool`; `syntax` is an enum defaulting to `Sh`, as the TS parameter does.

## After the port
- `without_array_literals`'s comment test differs from shell_rc's `comment_at` (it accepts `(` before `#` and not
  `=(`), so the two readers can disagree on a line like `x=(a #b)`.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/commands.json` (`oracle/w2-commands.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_commands.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 7: words after the closing `)` on a list line are still offered to `drop` (`plugins=(git) eza` loses `eza`); the whitespace before a trailing comment goes with a dropped word (`plugins=(eza # c )` becomes `plugins=(# c )`).
- C2. Rule 2: own names come from every FUNCTION_HEAD match over the whole text (found by trying the pattern at each line start in turn on the `regex` crate, never in fancy-regex's backtracking VM, which stops at a million backtracks: a 330 KB rc file and 5,000 blank lines in a row are answered as node answers them), heredoc bodies included, at a line start only (an indented head counts, one after `;` or `then` does not).
- C3. Any word of a command that ends in `/<head>` makes it no call.

- W1. `called_commands` over 124 texts and every RC_BUILTIN member (`called_commands`, `called_commands_rc_builtin`); `drop_plugins` and the names offered to `drop` (`drop_plugins`, `drop_plugins_calls`); `guard_commands` (`guard_commands`).

## Tests
| Rule | Test |
|---|---|
| 1 | `a_fish_config_is_not_shell_and_yields_nothing_an_empty_file_yields_nothing` |
| 2, 3, 4, 5 | `names_each_command_the_file_calls_once_in_order`, `a_builtin_a_keyword_a_function_or_alias_the_file_defines_is_no_call`, `zshs_precommand_and_reserved_words_are_no_calls`, `an_alias_body_runs_to_the_next_name_token`, `a_name_defined_after_its_call_is_still_own` |
| 6 | `an_array_literal_over_lines_with_a_comment_and_nested_parentheses_is_a_value` |
| 7 | `takes_the_named_plugins_out_of_a_multi_line_list_and_a_plus_equals_line`, `a_one_line_list_keeps_its_shape_a_list_with_nothing_to_drop_is_returned_as_it_is`, `a_list_line_left_with_only_its_comment_keeps_the_comment_at_its_indent` |
| 8 | `prepends_one_block_that_defines_a_guard_for_each_name`, `a_re_run_replaces_the_block_it_wrote_before`, `names_each_plugin_left_out_on_its_own_note_line`, `keeps_the_files_line_endings` |
