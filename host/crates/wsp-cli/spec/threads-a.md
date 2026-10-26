# threads-a

Source: `packages/host/src/verbs/threads.ts` lines 1-448 (448 lines) at 65f6b89c7: `ANSWER_VERBS`, `besideOn`, `subStart`, and the entries run, worktree, worktree remove, thread read, thread head, thread rename, thread forget, thread settle, thread restore, thread allow, thread deny. Tests: `verbs-threads.test.ts` (13), `verbs-turns.test.ts` (10), `run-beside.test.ts` (3), `sub-run.test.ts` (6), `verbs-send-notify.test.ts` (5), all against a running TS host and not ported as such; the tool server's record (`daemon/crates/wsp-mcp/tests/answers/*.json`) replays the same calls. Rust: `src/verbs/threads.rs`; tests `tests/threads_answers.rs`, `tests/threads_answers_cloud.rs`, `tests/threads_lines.rs`, the run rows of `tests/fixtures/run_verb.json`.

## What it does
Each entry is a command line and a tool over the same reads: a run opens a thread (in a project folder, beside a thread, in a worktree, forked from a thread's turn, or as a sub-thread on Boat) and follows or detaches its first turn; the others make or remove a worktree, read a thread or its head, rename, forget, settle or restore threads, and answer a prompt a thread is stopped on. The bodies are thin: the reads live in `turns-help.ts` and `workspaces-help.ts`.

## Behaviour
1. The table's words (usage, about, page, options, tool description, inputs, output, stream) are `tests/fixtures/thread_verbs.json`, copied by script; the run and call bodies are the implementer's.
2. Order in `thread_verbs()` (104, 448-449): run ... thread restore, the answer verbs (one per `ANSWER_ROADS` row with an `answer`: allow, then deny with `--reason`), the slate verbs (`slate::slate_verbs()`), then threads-b's.
3. Arity refusals before any dial, each `usage_refusal(<sentence>, usage_is(ctx))` and printed bare because the sentence opens with the verb: `wsp thread allow takes one thread.`, `wsp worktree takes a project and a branch.`, `wsp worktree remove takes a project and a branch.`, `wsp thread read takes one thread.`, `wsp thread head takes one thread.`, `wsp thread rename takes a thread and one name.`, `wsp thread forget takes one thread.`, `wsp thread settle takes one thread or more.`, `wsp thread restore takes one thread or more.` (`run_verb.json`).
4. run (112-166): no words is `EMPTY_MESSAGE_LINE` with `Put the message in quotes: wsp run <project> "say hi".`; three words is `wsp run takes a project and a message; <third> reads as a third word.`; both before the dial. Then the dial (115), then `--size`/`--from` without `--on` refused (121-122), so those two refusals come after a socket opened. With `--on`: the first of `--beside --fork --at --cwd --resume --copy --replaces` given (not false) is refused by name (`beside_on`), then `subStart` inside `beforeSending`. Else `--beside` with a project or `--branch`, `--at` without `--fork`, `--fork` with a project/beside/cwd/replaces, `--fork` with resume/copy, each refused in that order; `resume_asked`; then inside `beforeSending` the fork, beside or run target, `checked_start`, the resume-here refusal, the opening, `fork_for` for a project elsewhere, `awake` for a workspace. A failure after a wake is `with_line(e, nap_after_dead_launch(...))`.
5. run's tool (171-215): the same checks on the tool's arguments; the size/from refusals come before the dial here; detach answers `detached_out`; a followed turn answers `as_text(opened? + turn_text, out)`, `opened` printed only where the target named one.
6. worktree remove (249-260): `worktree.remove {project, branch, force?, check: true}` first, then `confirmed(ctx, "Remove the worktree for <b> in <p>?\nThe branch stays[, and the files no commit holds there go].", branch)`, then the removal without `check`; a no is exit 1 with `<branch> kept`. The tool removes at once (267).
7. thread forget (366-376): `forget_thread(check)` first, then `Forget thread <id>?\nIts row leaves this computer and the sidebar.`, then the forget. The tool forgets at once.
8. thread settle (396-405): `processes_asked(--processes)` before the dial; answer lines are each ended process's `ended_lines` then `settled_lines`, joined by newlines; the tool takes `processes` with "ask" as default.
9. thread rename (336-345): reads the thread, wakes its workspace (`awake(.., "rename", say)`) before the rename.
10. The answer verbs (56-81): `answer_open_asks(thread, road, reason)`; value `{threadId, askId, optionId}` and the answered line; the tool's description capitalizes the about's first letter.

## Edge cases
- `run --size x hi` and `run --from x hi` open a socket and ask nothing before refusing (`threads_lines::the_run_line::a_size_or_from_without_on_*`).
- The record's 61 run cases (32 cloud off, 29 on, 3 as a guest) carry the rest: beside, fork, at, resume, copy, replaces, notify, files (a missing file, a guest's file), a host stopping under the start.

## Behaviour nobody wrote down
- Rule 10.8: worktree remove and thread forget ask only after the host's check passed.
- Rule 10.20: the run's follow redials a host that stopped under it (`host_back`).

## Decisions
1. The table's words are data copied from the TS run; `thread_verbs()` carries no cloud switch (Decision 2 of verbs-table).
2. The answer verbs are written as two literal entries rather than mapped from `ANSWER_ROADS`, which lives in `turns_help` (no writer yet).

## After the port
- `wsp run --size`/`--from` without `--on` dial (and may start a host) before refusing, where the tool refuses first (threads.ts:115 against :172).

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `threads_answers::thread_verbs_are_the_table_the_oracle_wrote`, `verbs_oracle::the_table_has_the_fixtures_entries_in_order` |
| 3 | `verbs_oracle::run_verb_answers_the_exit_code_and_the_lines_before_it_dials` |
| 4 | the same, `threads_lines::the_run_line::*` (2), `threads_answers::tools::run`, `threads_answers_cloud::tools::run` |
| 5 | `threads_answers::tools::run`, `threads_answers_cloud::tools::run` |
| 6 | `threads_lines::taking_away::*` (4), `threads_answers::tools::worktree_remove` |
| 7 | `threads_answers::tools::thread_forget` (tool); the line's order is read by the reviewer (no recorded host replies for its ops) |
| 8 | `threads_answers::tools::thread_settle`, `run_verb.json` (`--processes maybe`) |
| 9 | `threads_answers::tools::thread_rename` |
| 10 | `threads_answers::tools::{thread_allow,thread_deny}`, `threads_lines::the_answer_verbs::*` |
| worktree, read, head, restore | `threads_answers::tools::{worktree,thread_read,thread_head,thread_restore}` |
