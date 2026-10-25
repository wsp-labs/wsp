# thread_forks
Source: `packages/protocol/src/words/thread.ts` lines 446-917 at a3b870a6e (the second unit of the file; `thread_lines` is the first). Tests: `packages/protocol/test/format.test.ts` and `rewind-words.test.ts` (25 cases ported; 4 ignored as constants alone), plus `pinned` in `tests/thread_forks.rs` (every value printed by the TS under node).

## What it does
The words of a side question, a rewind and a fork; the codex sign-in and model-catalog lines; a machine that ran out of memory and the size to move to; the helper's update lines; a nap's vault lines; a create's own lines; a machine gone at its provider; name refusals; the providers and places an image is built on; and how a word that is not a thread is refused.

## Behaviour
1. `rewind_note(o)`: with `kept` (even ""), its first unit upper-cased plus the rest, then the conversation-stays sentence; an agent that cuts no history says it keeps its own; no files: `<turns after> the conversation; the files stay as they are.`; else the full sentence. turnsAfter is "The turn after this reply leaves" for 1, else "The N turns after this reply leave" (thread.ts:530-536).
2. `fork_where_line(branch)`: "in this folder" for None, else `on <branch>`, even "" (thread.ts:553). `fork_branch_name(source, taken, id)`: base = `fork-<first 8 units of id>` for None or "", else `<source>-fork`; the base when free, else `<base>-<n>` for the first n from 2 not taken (thread.ts:557).
3. `fork_turns_line(n)`, `fork_turns_fix(n)`: 0, 1 and many each their own sentence (thread.ts:588,589).
4. `catalog_source_line(c, where)`: a harness source says `<label>[ <version>] on <where>`; a table says the refusal, else `<harness> table`, with `, <version>` when set (thread.ts:637). `no_models_line` per source.
5. `memory_near_full(used, total)`: total > 0 and used/total >= 0.9 (`MEMORY_NEAR_FULL`). `out_of_memory_line`: bytes by `fmt_bytes`, load by `to_fixed(load1, 1)`. `out_of_memory_row_line`: `out of memory, <fmt_bytes_of(used, total)>`.
6. `bigger_size_line(current, offers)`: of the offers with more memory, the one with the least memory (a stable sort: the first listed among equals); none: the no-size sentence (thread.ts:684).
7. `vault_kept_line(at)` and `vault_stale_line(at, refused)`: stale only where refused is Some (even ""); `no backup` without a vault time, else `no backup since <first 10 units of at>` (thread.ts:780).
8. `copy_first_line(where, name, rate)`: the billed clause only where `charges_nothing(rate)` is false, with `fmt_rate` (thread.ts:761).
9. `iso_seconds(ms)`: `toISOString()` with its fraction removed, so `YYYY-MM-DDTHH:MM:SSZ`, the milliseconds truncated (rule 8.15). `gone_words(id, seen)`: `the <by> found it gone at <iso_seconds(at)>` and ` (<answer>)` only for a non-empty answer.
10. `not_a_thread_line(word, is, line, or_computer)`: `<line> takes a thread[, or a computer with --on]`; a project adds `as wsp threads <word> lists them.`, a machine `as wsp threads lists them.`
11. `shared_folder_line(others)`: one `thread a works in this folder too`; more `threads a, b and c work in this folder too`.
12. `image_home_kept_line(on, home)`: None for no `on`, or when `names_place(home, on)`; else `your image lives on <place_said(home)>, so it is built there and not on <on>`.

## Edge cases
- `fork_branch_name(Some(""), {fork-01234567}, "0123456789")` is "fork-01234567-2"; a taken `-2` with a free base gives the base (`a_forks_branch_is_numbered_past_every_name_taken`).
- `memory_near_full(1, 0)` is false (no total) (`memory_lines_read_the_share_and_the_next_size_up`).
- `vault_stale_line(Some("short"), Some(""))` is "no backup since short": a short stamp is cut to what it has.
- `iso_seconds(1788744070999)` is ".10Z"-free "01:21:10Z"; `iso_seconds(-1)` is 1969-12-31T23:59:59Z (`a_moment_is_utc_to_the_second`).
- `gone_words` with an empty answer adds no brackets.

## Behaviour nobody wrote down
- Rule 8.15: `iso_seconds` is UTC, never the host's zone.
- Rule 8.13: the load figure is `to_fixed`, not rounding by `{:.1}`.

## Tapes
`sessions.rewind`, `sessions.start` with a fork, `workspaces.*` gone and name refusals, `init.start` placement lines.

## Decisions
1. `vault_kept_line` and `vault_stale_line` take the view's two fields.
2. `names_place` and `place_said` are declared in `pending_wire::types::place_word` until the `place_word` unit of this crate is written.

## After the port
- `fork_where_line(Some(""))` says "on " with nothing after it.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_words_a_rewind_says::*`, `asides_and_rewinds_say_what_moves_and_what_stays` |
| 2, 3 | `a_forks_branch_is_numbered_past_every_name_taken` |
| 4 | `where_the_composer_s_model_lists_came_from_in_one_line::*`, `codex_and_catalog_lines` |
| 5, 6 | `a_machine_that_stopped_answering_with_its_memory_near_full::*`, `memory_lines_read_the_share_and_the_next_size_up` |
| 7, 8 | `the_nap_s_words_when_its_vault_was_not_stored::*`, `vault_and_create_lines` |
| 9 | `gone_words::*`, `a_moment_is_utc_to_the_second` |
| 10, 11 | `places_and_threads_named_by_a_word` |
| 12 | `wsp_init_on_once_an_image_stands::*` |
