# shell_rc
Source: `shell-rc.ts:1-205` and `:525-563` (of 563 lines) at a3b870a6e. Tests: the cases of `test/shell-rc.test.ts`
that call no `stripExports` (6 of 37). Rust: `tests/detect_shell_rc.rs`. Pure: no host. `strip_exports` is the
unit shell_rc_strip, which calls this unit's crate-private helpers.

## What it does
Which files are rc files; which variable names mean a secret; a line's simple commands and the variables each assigns,
read the way bash, zsh and fish read them; and the files a line sources by a literal path.

## Behaviour
1. RC_PATHS is shell::RC_FILES each with a leading `.`, then `.config/fish/config.fish`; RC_NAMES their basenames
   (shell-rc.ts:13-16). `is_rc_path(rel)`: one of RC_PATHS, or a `.fish` file whose folder (text before the last `/`)
   is exactly FISH_CONF_D (shell-rc.ts:19-22). `rc_files(conf_d)`: RC_PATHS then each `.fish` name of `conf_d` as
   `<FISH_CONF_D>/<name>`, in the listing's order (shell-rc.ts:25-27).
2. `is_secret_name(name)` (shell-rc.ts:45-53), on `upper` = the name uppercased: a SETTING match is plain; a part of
   `upper` split on `_` (empties dropped) in SECRET_WORDS is a secret, in any case; otherwise only a name already all
   capitals can be one: any part matching SECRET_END, or a last part ending in `PASS` that is not PLAIN_PASS, or more
   than one part with a last in SECRET_LAST. (`name !== upper` compares UTF-16 text: lowercase `ß` uppercases to `SS`.)
3. `simple_commands(line)` (shell-rc.ts:74-105): walk the line; a `\` outside single quotes copies itself and the next
   character; inside a quote everything copies and the same quote closes; `'` or `"` opens one; outside quotes a
   comment (rule 4) ends the walk; one of `; | & { } ( )` ends the current command (dropped from both); each command
   then loses leading keywords (KEYWORDS, repeatedly); blank ones (after `js_trim`) are dropped. Commands keep their own
   spacing.
4. `comment_at(line, i)` (shell-rc.ts:238-240): the character is `#` and it is the first, or follows a JS-whitespace
   character or one of `; & |`, or follows `=(`. Counted over characters; every compared character is ASCII but `\s`.
5. `shell_words(command)` (shell-rc.ts:114-147): split on JS whitespace outside quotes; single quotes and `$'...'` keep
   text literally (inside `$'`, a `\` is handled by the backslash rule); `\` outside single quotes takes the next
   character, but inside double quotes keeps itself before a character other than `"`, `\`, `$`, backtick; quotes are
   removed; an empty quoted word (`''`) is a word.
6. `command_words(command)`: `shell_words`, split at the first word not matching ASSIGNED (`^NAME\+?=`): leading
   assignments, then the rest (shell-rc.ts:171-175).
7. `assigned_in(command)` (shell-rc.ts:179-199): the leading assignments' names, then: after `eval`, the names
   `assigned_names` finds in its arguments joined by a space; after `alias` or `abbr`, those of each alias body
   (rule 8). Otherwise the words of the command with every `\` newline and bare newline as a space, re-joined and
   keyword-skipped, give `bare`: a FISH_SET match assigns its group 2 unless its flags (group 1) match FISH_READS; else
   an EXPORTED match assigns each `\s+`-split token of its group 1 that matches ASSIGNED; else the run of ASSIGNED
   words starting at the first word index from which the remaining words joined by a space equal `bare`.
8. `alias_bodies(words)` (shell-rc.ts:153-168): after word 0, `--` makes every later word an argument; a word matching
   `^[-+].` is an option, and for `abbr` one matching ABBR_VALUE also swallows the next word; other words are
   arguments. Each argument holding `=` gives its text after the first `=`; when there is more than one argument and
   the first holds no `=`, the arguments after the first joined by a space are one more body (fish `abbr name body`).
9. `assigned_names(text)`: every `\n` line's simple commands' `assigned_in`, in order (shell-rc.ts:201-203).
10. `source_command(command)` matches SOURCE: indent, word (`source`, `.` or `\.`), token, rest (shell-rc.ts:537-540).
11. `sourced_path(token, home)` (shell-rc.ts:544-549): a token wrapped in matching `"` or `'` loses them; a leading `~`,
    `$HOME` or `${HOME}` followed by `/` becomes home; the result must start with `/`, hold none of `$`, backtick,
    `*`, `?`, `[`, `]`, `\`, and have no empty, `.` or `..` segment after the first `/`; else `None`.
12. `sourced_paths(text, home)` (shell-rc.ts:552-563): each `\r?\n` line with every `${HOME}` written `$HOME` first,
    then each simple command's source token's path, once each in first order.

## Edge cases
- `is_rc_path("config.fish")`, `"x/.zshrc"` and `".config/fish/conf.d/sub/deep.fish"` are false.
- `is_secret_name("PWD")` and `"_PWD"` are plain (one part); `"MYSQL_PWD"` is a secret.
- A `#` after a no-break space (U+00A0) starts a comment, since JS `\s` holds it; after U+0085 it does not.
- `simple_commands("")` is `[]`; `"a;;b"` is `["a", "b"]`.

## Decisions
1. Helpers shell_rc_strip needs are `pub(crate)`; `comment_at` takes the line as `&[char]` with a char index.

## After the port
- `sourced_paths` folds `${HOME}` before splitting, but `$HOME` inside double quotes in a `[[ ]]` test is followed too.

## Tests
| Rule | Test |
|---|---|
| 1 | `names_the_rc_files_the_shell_rung_carries_plus_fishs_config_and_its_conf_d` |
| 2 | `names_that_mean_a_secret_a_secret_word_alone_between_underscores_and_in_capitals`, `pass_or_passphrase_as_a_word_in_any_case_and_in_capitals_glued_at_the_end_or_a`, `a_shells_or_oh_my_zshs_own_setting_that_holds_a_secret_word_alone_is_plain` (its strip half is in shell_rc_strip) |
| 3, 4 | `simple_commands_split_outside_quotes_and_drop_keywords_and_comments`, `a_hash_after_a_no_break_space_starts_a_comment` |
| 10, 11, 12 | `the_paths_an_rc_file_sources_by_a_literal_path_home_and_absolute_forms_quoted_or`, `the_alias_fallback_and_the_source_guard_read_the_same_files_nvms_escaped_dot`, `sourced_path_folds_home_and_refuses_anything_a_shell_would_expand` |
| 5-9 | through `strip_exports` in shell_rc_strip's tests (every assignment case there runs `assigned_in`) |
