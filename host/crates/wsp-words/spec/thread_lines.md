# thread_lines
Source: `packages/protocol/src/words/thread.ts` lines 1-445 at a3b870a6e (the file splits in two units here; `thread_forks` is the rest). Tests: `packages/protocol/test/format.test.ts` and `redact.test.ts` (38 cases ported; `requestSecrets` and three incidental cases skipped), plus `pinned` in `tests/thread_lines.rs` (every value printed by the TS under node).

## What it does
The lines a turn the runtime cut, a harness that ended early and a step that timed out end on; what a person is told when the host's own validator refused their line; the machine, disk, resize and folder lines; the sentences every door quotes about turns, background work and coordinators; the two-halves refusal line, the one redaction rule and the schema issue line.

## Behaviour
1. `fmt_limit(ms)`: `<ms/3600000>h` when ms >= 1 h and a whole number of hours, else `<js_round(ms/60000)>m` (thread.ts:9-11). `turn_cut_line` per thread.ts:18-22 with `fmt_duration(_, Clock)`.
2. `timed_out_line(s, times)`: `timed out after <s>s`, `, twice` only for times 2 (thread.ts:25-27).
3. `harness_exit_line(bin, code, path, ended)`: 127 names PATH (or that the launch exported none) first, before any signal; then a signal; then a code; then `reached == Some(false)`; else killed (thread.ts:38-48).
4. `validator_refusal(error)`: None unless the text parses as a non-empty JSON array of objects each with a string `code` and an array `path`. When any issue is `invalid_union_discriminator` and every first path element that is a string is "op": the another-version line. Else the words of the first path elements in `REQUEST_WORDS` (threadId and sessionId both "the thread", argv, prompt, cwd, model, effort, permissionMode), duplicates dropped in first order, joined " and "; none named: "the host would not read this line" (thread.ts:62-97).
5. `disk_full_line(pct)` and `disk_mark_line`: the share is `floor(pct*10)/10` written by `number_string` (thread.ts:228-230, 248-256); the move clause uses `shell_line(["wsp","resize",thread,"--size",size_word])` (thread.ts:254).
6. `resize_word_line(w)`: `JSON.stringify(w)` then " names no size" (thread.ts:282).
7. `folder_level_line(dir, folders, hidden)`: `No folders in <dir>[, n hidden].` or `<n folder(s)> in <dir>[, n hidden].` (thread.ts:304-307); `folder_refusal_line`: whitespace collapsed and trimmed after "No folders read. " (thread.ts:312-314).
8. `idle_reason::of(ms)`: `idle <js_round(ms/60000)> min`; `window_in(r)`: the `\d+ min` of a reason that is exactly `idle <digits> min`, else None (thread.ts:288-293).
9. `background_work_words(road)`: the road, else "your harness's own background road" (thread.ts:342-344). `still_working_line(title)`: the trimmed title, "This thread" for None or blank (thread.ts:367-370).
10. `refusal_line(happened, fix)`: happened with JS trailing whitespace off; "." added unless it ends in `.`, `!`, `?` or `:`; then " " and the fix (thread.ts:393-396).
11. `redacted(text, hidden)`: for each hidden value whose `js_trim` is >= 4 units, both the value and its trimmed form; duplicates dropped; sorted longest first by UTF-16 length (a stable sort keeps first order among equals); each replaced everywhere by `<redacted>` in turn (thread.ts:402-407).
12. `issues_line`: `<path joined ".">: <message>` joined "; "; index parts written by `number_string` (thread.ts:411-413).
13. `say_once(prefix, message)`: name = prefix less a trailing `:\s*`; the message as is when name is non-empty and the message equals it or starts with `<name> `; else prefix + message (thread.ts:418-421).
14. `foreign_flag_line(flag, readers, here)`: one reader by name, more as `a, b and c`; no reader prints "undefined" as the TS does (thread.ts:432-435).

## Edge cases
- `fmt_limit(5400000)` is "90m"; `fmt_limit(0)` is "0m" (`limits_and_cut_turns`).
- `validator_refusal` of `[{"code":"x"}]` (no path) is None; a discriminator issue beside a field issue names the field (`the_validator_refusal_reads_only_an_issue_list`).
- `disk_full_line(99.999)` says 99.9 (`disk_lines_cut_their_share_to_a_tenth_never_up`).
- `window_in("idle 20 min ")` is None: the reason must be exact.
- `refusal_line("", "x")` is ". x"; `refusal_line("ends;", "x")` is "ends;. x"; an empty fix leaves a trailing space (`a_refusal_is_two_halves_joined_once`).
- `redacted("abc and abcdef", ["abc"])` changes nothing (3 units); a padded value blanks both the padded and the trimmed text; a value inside `<redacted>` is still replaced (`redaction_takes_the_longest_value_first_and_skips_short_ones`).
- `say_once("wsp new:", "x")` is "wsp new:x" (no space added).

## Behaviour nobody wrote down
- Rule 8.6: `trimEnd`, `trim` and `\s+` are JavaScript's.
- Rule 5.5: `refusal_line` is the join every refusal uses (`exit::refusal`).
- Rule 5.7: `issues_line` is the refused-op log's sentence for a schema refusal.

## Tapes
Every refusal's message on the wire (rule 5.5); the turn-cut and harness-exit lines in `session.end`; `places.*` folder listings.

## Decisions
1. `folder_level_line` takes the folder count, since the TS reads only the length.
2. Issue paths are `PathPart::Key` or `PathPart::Index`.

## After the port
- `foreign_flag_line` with no reader prints "belongs to undefined".
- `refusal_line` closes "ends;" with a second stop.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `turn_cut_line::*`, `timed_out_line_and_step_retry_line::*`, `limits_and_cut_turns` |
| 3 | `harness_exit_line::*`, `a_harness_that_ended_early_says_what_ended_it` |
| 4 | `a_refusal_the_host_s_own_validator_wrote::*`, `the_validator_refusal_reads_only_an_issue_list` |
| 5 | `disk_lines_cut_their_share_to_a_tenth_never_up` |
| 6, 7 | `folder_refusal_line::*`, `resize_and_folder_lines` |
| 8 | `idle_reason::*`, `the_idle_reason_reads_back_only_its_own_shape` |
| 9 | `still_working_line::*`, `provider_waits_and_the_words_doors_quote` |
| 10, 13 | `the_shape_a_refusal_takes_on_a_terminal::*`, `a_refusal_is_two_halves_joined_once` |
| 11 | `redacted::*`, `redaction_takes_the_longest_value_first_and_skips_short_ones` |
| 12, 14 | `issues_line::*`, `foreign_flag_line::*`, `issues_flags_and_agents_lines` |
