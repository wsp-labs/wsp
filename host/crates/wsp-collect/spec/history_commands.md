# history_commands

Source: `packages/collect/src/history/commands.ts` (240 lines) at `a3b870a6e`. Tests: `packages/collect/test/history.test.ts` `describe("command names")` (6 cases). Rust: `src/history/commands.rs`; tests `tests/history_commands.rs` (6), `tests/history_hard.rs` `parser::*` (2).

## What it does

Turns one shell line an agent ran into names: the program each command in it starts, and the packages its install commands asked for. Heredoc bodies are dropped before anything is read; quoted text is never split; a version check or a lookup is not a use. Nothing but names leaves: the line itself is dropped by the caller. `package_json.rs` and `workflows.rs` call `command_names` too.

## Behaviour

1. `without_heredocs` (commands.ts:27-40): split on `\n`; a line holding `<<TAG`, `<<-TAG`, `<< 'TAG'` or `<<"TAG"` (regex `/<<-?\s*(?:'([^']+)'|"([^"]+)"|(\w+))/`, first match on the line) is kept and opens a body (the regex also matches a here-string `<<< foo`, on its last two `<`, and a shift `1<<2`: both open a body too); every following line is dropped up to and including the first whose JS-trimmed text equals the tag. An unclosed body drops the rest. Joined back with `\n`.
2. `split_commands` (:54-123) walks the line one UTF-16 unit at a time (every character the rules name is ASCII, so a char walk is the same) with a quote state and a stack `opened` of the quote each `(` or `$(` was met in:
   - inside a quote: in `"`, `\` keeps itself and the next char; `$(` inside `"` flushes, pushes `"` and leaves the quote; the closing quote ends it; anything else is kept. Quotes themselves are never kept.
   - `'` or `"` opens a quote. Outside quotes `\` drops itself and keeps the next char (nothing at the end); that char may be `\n`, which then sits inside the word.
   - `#` at index 0 or after one of `` \s;&|(){} `` flushes and skips to the next `\n` (the `\n` itself is then read, so it flushes).
   - the two-char tokens `|| && $(` and the one-char separators `| ; & \n` flush; `$(` pushes `None`. Two-char tokens are checked before one-char ones (`||` is one separator). At the last unit of the line the two-unit slice is one unit long, so a trailing `&` is a separator and never reaches the redirection test: `echo >&` splits to `["echo >"]`, `echo 2>&` to `["echo 2>"]`, while `echo >& x` keeps the `&`.
   - `${` copies through its matching `}` (nested braces counted, to the end when unclosed) into the current word.
   - `(` flushes and pushes `None`; `)` flushes and, when the stack is not empty, pops into the quote state (so `"$(a) b"` resumes the string).
   - `&` right after `>` or `<`, or right before `>`, is part of a redirection and is kept; otherwise `` ` `` `{` `}` and the separators flush.
   - flush pushes the JS-trimmed word when it is not blank. Output keeps order.
3. `command_words` (:126-149): split on `\s+`, drop empties; skip leading assignments (`/^[A-Za-z_][A-Za-z0-9_]*=/`) and redirections (`/^(?:\d*|&)[<>]/`; a lone one, `/^(?:\d*|&)[<>]{1,2}&?$/`, also skips its target); stop at `command` followed by `-v` or `-V`; skip a prefix word (`sudo doas env time nohup exec command builtin nice xargs caffeinate then do else if elif while until !`) and the options after it: any word starting `-`, and for `env` also assignments; for `sudo -u -g -h -p`, `doas -u`, `nice -n`, `xargs -I -n -P -d -L -s` the option also skips its value. The NOT_COMMANDS test is on the uncut head, before the path cut, so `/bin/echo hi` is not skipped and counts as `echo`. `env`'s options never skip a value (`env -u FOO ls` gives head `FOO`). The rest's head in `for case function fi done esac export local declare typeset readonly unset return exit break continue shift set cd echo printf true false [ [[ test wait trap source . alias eval read :` gives `[]`; otherwise the head is cut after its last `/`, followed by the remaining words.
4. `is_version_check` (:152-158): head `which`; or head `command` with second word `-v`/`-V`; or exactly two words whose second is `--version`, `-version`, `-v` or `-V`.
5. `command_names` (:162-167): over `split_commands(without_heredocs(line))`, the first word of each command whose words are not a version check; repeats kept.
6. `installs_in` (:201-235) by head: `brew install|reinstall` from word 2; `npm install|i|add`, `pnpm add|install|i`, `bun add|install|i` only when a later word is `-g` or `--global`, from word 2; `uv tool install` from word 3; `pipx install` from 2; `pipx run` the first positional as `pipx-run`; `cargo install`, `go install` from 2; `apt`/`apt-get install` from 2 as `apt`; `npx`, `bunx`, `uvx` the first positional after the head under its own name. Positionals skip words starting `-`, redirections, and a lone redirection's target. For every installer, all positionals are turned into names, empty names are dropped, and only then do `pipx run`, `npx`, `bunx` and `uvx` keep the first (commands.ts:206, 220, 231): `uvx =x ruff` installs `ruff`.
7. `bare_name` (:178-187): `go` cuts at the first `@`, takes the text after the last `/`, and a last segment `v<digits>` becomes empty; any empty result (a trailing slash too: `github.com/a/b/`) falls back to the whole cut path (`|| s`, commands.ts:182); `uv pipx uvx pipx-run` cut at the first of `=<>!~[`; any other cuts at the first `@` after index 0 (a scope's leading `@` stays).
8. `install_names` (:238-240): `installs_in` over every command of the heredoc-free line, in order.

## Edge cases

- `""`, blanks and a lone comment give `[]` for both (`parser::an_empty_or_blank_line_runs_nothing_and_installs_nothing`).
- An unclosed quote keeps everything after it in the current word: `echo 'unclosed | ls` is one command (oracle).
- `ls#x` is one word: `#` only comments at a word start (oracle).
- `go install example.com/mod/v2@latest` installs `example.com/mod/v2`: the `v2` segment empties, so the cut path stands (oracle).

## Behaviour nobody wrote down

- Rule 8.3 (UTF-16 units): no index here reaches output, so a char walk is exact; `trim()` is `wsp_words::js_trim` (rule 8.6).
- `\s` in the TS regexes is JavaScript's whitespace set (rule 8.6), which includes U+00A0 and U+FEFF; use `wsp_words::js::is_js_whitespace`.

## Decisions

1. The oracle (`tests/fixtures/oracle/commands.json`, 70 lines) is the arbiter where this page and the TS disagree.

## After the port

- `npm install -g` with no package names an empty list but `npm i -g x y` reads both; `command ls` (no `-v`) is skipped as a prefix, so it counts `ls`. Kept.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/history_commands.json` (252 lines, each through all five functions, written by
`oracle/w2-history_commands.oracle.ts`), replayed by `tests/history_hard.rs`
`w2_oracle::every_line_splits_names_and_installs_as_the_ts_did`.

- W1. Heredocs: a here-string `<<< foo` and a shift `1<<2` open a body; a `<<TAG` inside quotes opens one too (the
  heredoc pass runs before quotes are known); the closer is any line whose `js_trim`med text equals the tag, even for
  a plain `<<EOF` and even indented; only the first marker on a line counts.
- W2. Splitting: backslash-newline leaves `\n` inside the word; `{` and `}` are separators (brace expansion and
  `find -exec ... {} \;` give fake names such as `;`); `${...}` copies through its matching brace; `$((`, `((` and
  `<(` split like subshells; quotes are dropped before words split, so `FOO="a b" ls` gives head `b`; a trailing `&`
  is a separator (`echo >&` is `["echo >"]`).
- W3. Words: `!` is a prefix only as its own word (`!ls` is a name); the NOT_COMMANDS and version tests read the
  uncut head; a head ending in `/` gives the name `""`, which is kept; a version flag with any third word is a use
  (`node --version 2>&1` counts); `env`'s options skip no value; the assignment regex needs a letter or `_` first
  (`=b ls` and `1a=b ls` are heads); a leading redirection and a lone one's target are skipped.
- W4. Whitespace is JavaScript's: U+0085 stays inside a word (`ls\u0085 x` is the name `ls\u0085`), U+FEFF and
  U+00A0 split and trim.
- W5. Installs: every non-dash positional is a package, option values included (`npm i -g typescript --prefix
  /usr/local` installs `typescript` and `/usr/local`); `-g`/`--global` anywhere after the head; one-shot runners keep
  the first name after empties are dropped; `go install` names follow rule 7's fallback; apt keeps version pins and
  needs `install` as word 1; substitutions stay in the word as written.

## Rules pinned

| Rule | Test |
|---|---|
| 1 | `never_splits_inside_quotes_and_never_reads_a_heredoc_body`, `parser::every_line_of_the_oracle_*` (heredocless column) |
| 2 | `takes_the_first_word_of_each_command_across_*`, `keeps_an_unquoted_variable_inside_the_word_it_sits_in`, oracle (split column) |
| 3, 4, 5 | `a_version_check_is_not_a_use_*`, oracle (names column) |
| 6, 7, 8 | `reads_the_packages_an_installer_was_asked_for_*`, `never_reads_a_redirection_target_as_a_package`, oracle (installs column) |
| empty input | `parser::an_empty_or_blank_line_runs_nothing_and_installs_nothing` |
