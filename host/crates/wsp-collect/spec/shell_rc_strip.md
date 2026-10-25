# shell_rc_strip
Source: `shell-rc.ts:205-523` (of 563 lines) at a3b870a6e. Tests: the 32 cases of `test/shell-rc.test.ts` that call
`stripExports`. Rust: `tests/detect_shell_rc_strip.rs`. Pure: no host. Uses shell_rc's `simple_commands`,
`assigned_in`, `command_words`, `comment_at`, `is_secret_name` and its regex sources (QUOTED, HEREDOC, ASSIGNS, GUARD,
KEYWORDS); its tests go red with that unit's helpers too.

## What it does
`strip_exports` cuts every line that assigns a secret-named variable out of a carried rc file, with its continuation
and whatever quote, substitution, array or heredoc it leaves open, and names each variable cut. With `script`, each
cut command stands as `true` (or as a false condition) so every block the file opens still parses. Values never leave.

## Behaviour
1. Line endings: the result joins with `\r\n` when the text holds one, else `\n`; input splits on `\r?\n`
   (shell-rc.ts:421,475,522). A text with nothing to cut comes back byte for byte.
2. A line "holds" a secret when one of its simple commands assigns (`assigned_in`) an `is_secret_name` name, or when
   `secret_in` names something in it (shell-rc.ts:422).
3. Per line, in this order of states (shell-rc.ts:475-519): an open cut takes the line (rule 7); an open join takes it
   (rule 6); an open heredoc takes it (rule 8); else a line that opens shell (rule 6) starts a join; else `take`.
4. `take(line)` (shell-rc.ts:458-474): the hits are each simple command's assigned names that are secret names, or,
   when `secret_in` names something in the whole line, every assigned name of a command `secret_in` also names.
   No hit and nothing named: the line is kept and the heredoc and arithmetic it leaves open carry on (rule 9). Else each
   hit joins `names` once, in first-cut order; with no hit, what `secret_in` named joins `held` once. The line is
   scanned (rule 10) from the arithmetic depth carried so far: still open, a cut starts holding it (`whole` while no
   heredoc is open); closed, it is left (rule 11) as whole, and only its arithmetic depth carries on.
5. Without `script` a cut command, with every line it took, is dropped. Nothing stands in its place.
6. A join (shell-rc.ts:211-214,487-496,517): a line opens shell when the quote left open at its end is `'`, `"` or
   `$'`, no QUOTED token on it is a single-quoted string ending in `\'`, and the first non-assignment word of its last
   simple command is `eval`, `alias` or `abbr`. Following lines join it while a quote, a group or a backslash
   continuation stays open; the joined text (`\n` between lines) then goes to `take` as one line. A join still open at
   the end goes to `take` as it is.
7. A cut (shell-rc.ts:476-486): each following line is scanned from the open state and joins the cut; when the scan
   closes, the cut is left (rule 11) with `whole` false if a heredoc was ever open in it, and the arithmetic depth
   carries on. A cut still open at the end is left with `whole` false.
8. A heredoc body (shell-rc.ts:497-516): a line whose `js_trim` equals the word ends it and is kept. While a cut body
   line left a value open, lines are dropped until a scan from that state closes. Else a body line that assigns a
   secret name is dropped and its names join `names`; one that assigns none but that `secret_in` names is dropped and
   joins `held`; others are kept. A dropped line whose kept predecessor ends in an odd run of backslashes keeps an empty
   line in its place. A dropped assigning line that leaves something open other than a heredoc opens that value.
9. Only a heredoc and the arithmetic depth carry from a kept line to the next (shell-rc.ts:336-339); a stray quote or
   backtick, or a comment, never carries. The arithmetic depth is one stream across cut and kept lines.
10. `scan_line(line, start)` (shell-rc.ts:243-333): inside a heredoc only the line equal (after `js_trim`) to its word
    matters. Otherwise per character: `\` outside single quotes skips the next and, as the last character, marks a
    continuation; `$'` closes at `'`; `'` and backtick close at themselves; an unquoted comment (shell_rc rule 4) ends
    the scan; `"` toggles; unquoted `$((` and `((` deepen arithmetic and `))` with depth above 0 closes one; `$(`
    pushes a `)` group remembering a surrounding double quote and clears it; `${` pushes a `}` group; an unquoted `(`
    on top of a `)` group or right after `=` pushes a `)` group; an unquoted `)` pops a `)` group and restores its
    quote; `}` pops a `}` group in any quote state; `<<<` is skipped; an unquoted `<` with arithmetic at 0 that starts a
    HEREDOC match opens that heredoc and ends the scan. Open means a quote, a group, a heredoc or a continuation.
11. Leaving a cut (shell-rc.ts:423-453), only with `script`. The cut lines join with `\n`. When `whole`, its command
    spans (rule 12) that hold a secret each stand as: a comment as nothing; an ASSIGNS command as `true`; a command
    under an `if`, `elif`, `while` or `until` (GUARD over the text before its whole and-or list) as `false` (`true`
    for `until`) from the end of the keyword to the end of the list, or as `[[ -n '' ]]` (`-z` for until) when a `{`
    follows the list; a command that leads into `&` or `|` within its list as one `true` for the list; else `true`.
    If any holding span starts with `for`, `select` or `case` and a blank, nothing stands. Stands are applied sorted
    by start, longer first, skipping one that starts inside the last; a "nothing" also drops the blanks before it.
    The result is kept when it differs from the cut, is not blank, and none of its lines holds a secret still;
    otherwise the cut stands as one `<indent of its first line>true`, unless its first line is a comment or holds
    `;;`, `;&` or `;|`, when it stands as nothing.
12. `command_spans(text, lists)` (shell-rc.ts:352-406) gives `[start, end)` spans of top-level simple commands and
    comments, leading blanks, backslash-newlines and keywords skipped and trailing blanks trimmed. Splits on `;`, `\n`,
    `(` or `)` at top level, `|` not after `>`, `&` not after `<` or `>` nor before `>`, and a bare `{` or `}`;
    quotes, `$(`, `${`, backticks, `[[ ]]` (at a command's start) and a `(` inside a command hold together. With
    `lists`, `&&`, `||` and `|` do not split.

## Edge cases
- `strip_exports("", ..)` is `{names: [], carried: ""}`.
- `"export A_TOKEN=x\n"` carries `""` without `script` (the empty last line is all that is kept) and `"true\n"` with it.
- CRLF input keeps CRLF in what is carried.

## Decisions
1. `secret_in` is `Option<&dyn Fn(&str) -> Option<String>>` and `held` a `&mut Vec<String>`, for TS's optional function
   and default-empty array that the caller can read after.
2. `Open.quote` is `&'static str` holding exactly the TS literal, so the scan reads like the TS.

## After the port
- A `for`, `select` or `case` head holding a secret makes the whole cut one `true`, which can break a loop body that
  spanned the same lines.

## Tests
| Rule | Test |
|---|---|
| 1 | `a_script_with_nothing_to_cut_comes_back_byte_for_byte`, `crlf_files_are_stripped_too_and_keep_their_line_endings`, `an_empty_file_and_a_lone_cut_line_carry_nothing_but_their_newline_rule` |
| 2, 4, 5 | `lists_the_names_and_cuts_the_lines_exported_or_plain_single_or_multiple_per_line`, `typeset_declare_and_fish_set_are_assignments_as_well`, `readonly_local_a_quoted_assignment_and_a_flagless_fish_set_are_assignments_too`, `an_export_that_is_not_the_first_word_of_the_line_is_found_and_the_whole_line_cut`, `an_assignment_behind_a_precommand_word_a_declaration_builtin_or_fishs_and_or_and`, `an_assignment_sudo_doas_nohup_or_nice_passes_on_is_cut_and_a_line_naming_the`, `a_quoted_builtin_set_or_variable_name_is_read_as_the_bare_one_a_case_patterns`, `secret_in_names_go_to_held_and_never_to_names` |
| 6 | `an_alias_or_abbr_body_is_shell_again_so_a_secret_it_assigns_is_cut_like_a_bare`, `a_heredoc_body_line_or_an_eval_or_alias_body_over_lines_that_assigns_a_secret_is` |
| 7, 10 | `a_quoted_value_spanning_lines_and_a_backslash_continuation_are_cut_whole`, `an_array_spanning_lines_under_a_secret_name_is_cut_whole_quotes_comments_and`, `an_array_under_a_secret_name_is_cut_whole_past_a_comment_right_after_its_a`, `heredocs_command_substitutions_and_backticks_spanning_lines_are_cut_whole`, `a_parameter_expansion_spanning_lines_and_a_backslash_quoted_heredoc_word_are_cut`, `ansi_c_quoting_on_a_cut_line_closes_at_its_own_quote`, `a_comment_right_after_or_on_a_cut_line_ends_the_scan_there_and_var_is_a_length`, `a_cut_lines_trailing_comment_does_not_swallow_the_lines_after_it` |
| 8 | `a_heredoc_body_line_or_an_eval_or_alias_body_over_lines_that_assigns_a_secret_is` |
| 9 | `a_comments_stray_quote_an_arithmetic_shift_or_a_stray_backtick_on_a_kept_line`, `a_here_string_or_a_shift_inside_arithmetic_on_a_kept_line_opens_nothing_so_the`, `an_arithmetic_block_spanning_kept_lines_opens_no_heredoc_on_its_later_line`, `a_zsh_glob_flag_or_a_brace_expansion_holding_is_not_a_comment_so_an_export_after`, `inside_double_quotes_counts_for_nothing_an_unbalanced_suppresses_heredoc`, `an_arithmetic_block_spanning_kept_lines_keeps_a_shift_on_every_one_of_them`, `the_arithmetic_depth_is_one_stream_across_cut_and_kept_lines` |
| 11, 12 | `in_a_script_a_cut_command_stands_as_true_at_its_own_indent_so_a_block_it_was_the`, `in_a_script_a_cut_command_that_shares_its_line_stands_as_true_and_the_rest_of`, `a_cut_command_holds_together_through_its_substitutions_quotes_arrays_and`, `in_a_script_what_a_cut_command_decided_never_runs_a_condition_holding_one_stands`, `zshs_repeat_and_coproc_a_backslash_before_the_word_and_execs_bundled_flags_hide`, `a_shells_or_oh_my_zshs_own_setting_that_holds_a_secret_word_alone_is_plain` |
