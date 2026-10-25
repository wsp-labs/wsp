# changes
Source: the functions of `packages/protocol/src/changes.ts` (124 lines) at a3b870a6e; its zod shapes (`Checkout`, `CheckoutReply`, `CommitDraft`, `ViewedMarks`) and flat constants (`DRAFT_DIFF_MAX_BYTES`, `DRAFT_NOTES`) are `wsp_wire::types::changes`. Tests: `packages/protocol/test/changes.test.ts` (9 cases, ported whole in `tests/changes.rs`; none skipped). No `pinned` module.

## What it does
The words a checkout's counts read as on the composer's row, the diff and the question a commit message is drafted from, the harness's answer read back as a commit message, and the lines `wsp discard` and `wsp commit` print.

## Behaviour
1. `checkout_words::changed(n)` is "<n> changed"; `checkout_words::UNREAD` is "changes not read" (changes.ts:27-30).
2. `checkout_counts(c)`, in order: `editsUnread` exactly true is `[UNREAD]`, whatever `changed` says; else `changed > 0` is `["<n> changed"]`; else `[]`. Ahead, behind, `countsUnknown`, stashes and head are never read (:35-38).
3. `cut_diff(text, max)`: `max` defaults to `DRAFT_DIFF_MAX_BYTES` (48 * 1024 = 49 152). It counts UTF-8 bytes. A text of `max` bytes or fewer comes back whole. Else the first `max` bytes are kept; where the last newline byte inside them sits past index 0, the cut ends just after it; else (no newline, or the only one at index 0) all `max` bytes stay. The bytes are decoded with `TextDecoder` and every trailing U+FFFD is taken off, so a letter cut in half leaves nothing behind (:56-62).
4. `draft_prompt(diff, task)`: five fixed lines (:72-76), then, where the task trimmed is neither absent nor "", a blank line, "The task the agent was given:" and the trimmed task cut to 2000 UTF-16 units (`DRAFT_TASK_MAX`), then a blank line, "The diff:" and the diff as given. Lines join with "\n"; the diff is last and untouched (:69-82).
5. `commit_message(answer)`, in order (:90-102): trim; a whole text of the form "```<info>\n<body>\n```" (one optional newline before the closing fence) becomes its body, trimmed again; a text that opens with `"`, `'`, U+201C or U+2018 and closes with `"`, `'`, U+201D or U+2019 loses both ends and is trimmed (the pair need not match); every em dash (U+2014) with the whitespace around it becomes ", "; the text splits on "\n" or "\r\n"; no line with anything but whitespace is None.
6. The subject is the first non-blank line trimmed. Past 72 UTF-16 units (`SUBJECT_MAX`) it is `words_within(first, 72)`, else its first 72 units. The body is every line after it joined with "\n" and trimmed; an empty body answers the subject alone, else "<subject>\n\n<body>" (:98-101).
7. `discarded_line` is "<workspace>: <path> discarded"; `clean_checkout_line` is "<workspace>: nothing to commit, the checkout has no changes" (:105-112).
8. `committed_line`: "<workspace>: committed <first 7 units of oid> <subject> (<n> file)" for exactly 1 file, "files" otherwise, 0 included (:115-117).

## Edge cases
- `cut_diff("é\né\né\n", Some(5.0))` is "é\n"; `cut_diff("one\ntwo\nthree\n", Some(9.0))` is "one\ntwo\n" (test asserts).
- `cut_diff("\nabcdef", Some(3.0))` is "\nab": a newline at index 0 is no cut point.
- `cut_diff("abc", Some(0.0))` is "".
- `draft_prompt("D", Some("  "))` carries no task lines.
- `commit_message("```\n```")` is None; `commit_message("\"")` is `"` (one character is no quoted pair).
- `commit_message("one\n\u{2014} two")` is "one, two": the whitespace eaten around a dash includes the newline.
- `commit_message("a\r\n\r\nb\r\nc")` is "a\n\nb\nc".
- `commit_message` of 80 "a" with no space is the first 72 "a".
- `committed_line("w", oid "abc", subject "s", 0 files)` is "w: committed abc s (0 files)".

## Behaviour nobody wrote down
- Rules 8.1 and 8.4: `DRAFT_TASK_MAX`, `SUBJECT_MAX`, `words_within` and the 7-unit oid count UTF-16 units; `cut_diff` alone counts UTF-8 bytes.
- Rule 8.5: the task cut and the subject's fallback cut never leave half a surrogate pair.
- Rule 8.6: every trim, and the `\s*` around an em dash, is JavaScript's whitespace.
- Rule 8.7: the fence and quote regexes are ported by hand; the quote class holds two non-ASCII units, each one UTF-16 unit.
- Rule 8.11: `checkout_words::changed(n)` and the file count write `String(n)`, so `number_string`.

## Tapes
The workspace status's `checkout` (the composer's checkout row); `workspaces.checkout`; the commit box's draft and its `DRAFT_NOTES` line; `wsp commit` and `wsp discard` and their tools.

## Decisions
1. CHECKOUT_WORDS is the module `checkout_words`: the fixed word a constant, the sentence a function taking `n: f64`.
2. `checkout_counts` takes the whole `Checkout` where the TS takes `Pick<Checkout, "changed" | "editsUnread">`.
3. `cut_diff`'s default parameter is `Option<f64>`, None reading `DRAFT_DIFF_MAX_BYTES`.
4. `committed_line` takes `MadeCommit { oid, subject, files_changed: u64 }` for the TS inline object; `files_changed` is a whole count where the TS takes any number.

## After the port
- `cut_diff` decodes with a default `TextDecoder`, which drops a leading byte order mark: a diff over the cap that opens with U+FEFF loses it (`cut_diff("\u{feff}ab\nccc", Some(5.0))` is "ab"). The trailing U+FFFD strip also eats a real U+FFFD the diff held just before the cut (`"abc\u{fffd}😀"` at 9 bytes is "abc").
- `commit_message` strips mismatched ends: `"Fix" the 'cart'` becomes `Fix" the 'cart`.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `the_counts_after_a_branch::*` |
| 3 | `the_diff_a_draft_is_asked_from::*` |
| 4 | `the_draft_question::*` |
| 5, 6 | `the_answer_read_as_a_commit_message::*` |
| 7 | not pinned: no TS test reads `discardedLine` or `cleanCheckoutLine` |
| 8 | not pinned: no TS test reads `committedLine` |
