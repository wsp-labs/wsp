# codex_session_title
Source: packages/adapter-codex/src/session-title.ts (160 lines), lost-session.ts:1-58 (the session-held reading) at a3b870a6e.
Tests: packages/adapter-codex/test/session-title.test.ts (16 cases), session-held.test.ts (7 cases), command.test.ts "the probe, a title
and a draft run the program and none of the launch words" (1). Cases that call the adapter's `title_for`, `draft_for`, `rename_session`,
`session_held` or `probe_catalog` run in `tests/codex_adapter.rs`. Rust: `src/codex/session_title.rs`, `src/codex/session_held.rs`;
tests `tests/codex_session_title.rs`. The tests build real sqlite dbs with the `sqlite3` binary, as the TS tests do.

## What it does
What Codex itself calls a thread, read from its thread index (`state_<n>.sqlite`, table `threads`) with `sqlite3 -readonly`; a rename
through the app server's own `thread/name/set`; a title or a commit message asked of `codex exec` in one read-only turn; and the one-line
reading of what a machine holds for a thread whose resume Codex refused.

## Behaviour
1. `NEWEST_STATE_DB` is exactly `d=$(ls -1 state_*.sqlite 2>/dev/null | sort -t_ -k2,2n | tail -n 1)` (session-title.ts:18): the highest
   schema number wins, so `state_10` beats `state_5`.
2. `session_title_command` (:28): `command -v sqlite3 > /dev/null 2>&1 || exit 0; cd '<home>' 2>/dev/null || exit 0; <NEWEST_STATE_DB>;
   [ -n "$d" ] || exit 0; sqlite3 -readonly "$d" '<query>' 2>/dev/null; true`, the query
   `select json_object('name', name, 'title', title) from threads where id = '<slug>' limit 1;`. A non-slug id is refused.
3. `parse_session_title` (:39): the first line that trims to start with `{` and parses to an object decides: its `name`, else its
   `title`, each trimmed, the first non-empty wins; a row with neither is `None`. A line that does not parse is skipped; no line is `None`.
4. `title_for_command` (:66) refuses a prompt holding a line that is exactly `WSP_PROMPT_END`. The line is
   `<ENV_FROM_INPUT>; cd ~ && <question line> <<'WSP_PROMPT_END'\n<prompt>\nWSP_PROMPT_END`.
5. The question line (:74): `<launch word> exec <launch flags> --json --skip-git-repo-check -c sandbox_mode='"read-only"'
   -c approval_policy='"never"'`, then ` -m <slug model>` where a model is named, then ` -`. The launch is `codex_launch(launch, broker, now)`:
   the broker's overrides go after `exec`, where codex keeps them. The launch words (`args`) never appear; the program does.
6. `draft_for_command` (:84): `<ENV_FROM_INPUT>; cd ~ && <question line> < '<prompt file>'`.
7. `parse_draft_for` (:89): the `text` of the last `{"type":"item.completed","item":{"type":"agent_message","text":..}}` line, whole;
   `None` where none, or where it trims empty. The last message with a string `text` wins even when that text is `""` (ST2). `parse_title_for` (:113): the same last text through `wsp_words::generated_title`, `None`
   where none.
8. `rename_command` (:147): `cd ~ && <ENV_FROM_INPUT>\n<app_server_script(codex_word, [{lines: [initialize(1), initialized,
   thread/name/set(2) {threadId: slug, name: title}], answers: 2}])>`. The title rides JSON, its quotes and newlines as its own bytes.
9. `parse_rename` (:155): no answer to id 2 is `{kind: failed, error: "codex's app server did not answer the rename"}`; a result is
   `written`; an error starting `no rollout found` is `no-session`; any other is `failed` with the error's message.
10. `session_held_command` (lost-session.ts:17): the lines, joined by `\n`, of lost-session.ts:17-32 verbatim: the boot id, `cd` into the
    home or `nohome`, every file under `sessions` and `archived_sessions` whose name holds the id as `file <size> <abs path>`, `listed`,
    `nosqlite`, the newest db or `nodb`, the row read with `path` and `mode` and, failing that, with `path` alone, or
    `unread <db> <first 160 chars of the error>`, `norow <db>`, `row <db> <json>`, then `named there` or `named missing` (the file or
    `<file>.zst`). The id is a slug.
11. `parse_session_held` (lost-session.ts:35): lines are trimmed; boot is the text after `boot ` on the first such line, trimmed, else
    `unread`; each `file <size> <path...>` reads `<path> (<size> bytes)`, the path rejoined on spaces; the index is `no Codex home`,
    `not read, no sqlite3`, `no state db`, `not read` (no row line), `not read from <rest>`, `no row in <rest>`, or
    `<rest>, the file it names is there|is missing`. The listing reads cut short unless `listed` or `nohome` was said. The answer:
    `boot <boot>; rollout files: <none | files joined by ", ">[, the listing cut short]; index row: <index>`.

## Edge cases
- A name holding a quote and a newline comes back whole: the row is JSON, never printed columns.
- A home path with a quote is shell-quoted: `cd '/root/it'\''s here'`.
- A db sqlite3 cannot open reads `not read from state_5.sqlite <its words>`, never `no row`; the db's bytes never reach the line.
- `parse_session_held("boot 1f0c\nfile 12 /root/.codex/sessions/a.jsonl\n")` is
  `boot 1f0c; rollout files: /root/.codex/sessions/a.jsonl (12 bytes), the listing cut short; index row: not read`.

## Behaviour nobody wrote down
- Rule 8.6 for every trim. `now` reaches `codex_launch` as an argument (codex_command decision 1).

## Decisions
1. None beyond codex_command's.

## After the port
- Nothing.

## Rules added for the port (each value printed by the TS at 65f6b89c7)

- **ST1.** `parse_session_title`: the first parsed object decides even with neither field (`{} \n{"name":"X"}` is `None`); a non-string
  name falls to the title (`{"name":5,"title":"T"}` is `T`); a line with trailing text does not parse and is skipped.
- **ST2.** `one` then `""` gives `None` for both title and draft; `Fix the bug.` titles as `Fix the bug`; two lines give no title; a
  draft keeps `  keep  \n  spaces \n` whole; a later non-string text does not displace `one`.
- **ST3.** `parse_rename`: `result: []` is no answer; only a message starting lowercase `no rollout found` is `no-session`
  (`No rollout found` fails in its words); a message `""` fails with `""`.
- **ST4.** `parse_session_held` splits a `file` line on single spaces (`file  12  /a` reads `12  /a ( bytes)`); `nodb` outranks a `row`
  line; empty input is `boot unread; rollout files: none, the listing cut short; index row: not read`; `nohome` reads
  `index row: no Codex home`.

## Tests (tests/codex_session_title.rs, 22; adapter-level 2 in tests/codex_adapter.rs; TS 24)
- Rules 1-3: `the_title_codex_keeps_for_a_thread::*` (6: title, name, highest schema, missing row and db, quote and newline, slug).
- Rules 4-7: `the_title_codex_makes_for_a_thread::*` (2), `the_commit_message_codex_drafts::*` (2),
  `a_persons_setup_for_codex_on_a_computer::the_probe_a_title_and_a_draft_run_the_program_and_none_of_the_launch_words`.
- Rules 8, 9: `naming_a_codex_thread_from_wsp::*` (5).
- Rules 10, 11: `what_the_machine_holds_for_a_session_codex_would_not_resume::*` (6).
- ST1 to ST4: `rules_added::*` in `codex_session_title.rs` (4 tests).
