# threads_tree: rows, trees, tokens and who is told

TS: `T/threads.ts:64-97, 303-362, 523-585, 642-661, 1144-1179` at `65f6b89c7`. Rust: `src/threads/threads/tree.rs`,
methods on `ThreadsArea`. Every function here is synchronous except `stop_under` and `limit_details_due`. All read
`ctx.sessions()` (the runtime's rows, an `IndexMap<row id, SessionEntry>`) and `ctx.thread_records()` in map order.

## Rules

TR1. `computer_silent(e)` (`:67-68`) is true for an error born as `MachineUnreached`, `MachineUnreachableError`,
`DeadlineError`, `LaunchUnanswered`, an engine `PlaceAbsent`, or a daemon channel that did not answer, and false for
any other, a plain refusal included. It reads the class stamped where the error was born, never the words: in Rust a
`HostError` mark from `SILENT_MARKS` (`"MachineUnreached"`, `"MachineUnreachableError"`, `"DeadlineError"`,
`"LaunchUnanswered"`, `"PlaceAbsent"`, `"DaemonUnanswered"`). `None` (no error) is false.

TR2. `thread_runs(t)` (`:75`): some row has `view.threadId == t` and `view.status == running`, with or without a
handle. A row with no thread never counts.

TR3. `launching_on(t)` (`:77-82`): the first row in map order with that thread and `launch` set, as
`(turn_id, launch)`. `running_on(t)` (`:84-89`): the first row with that thread, `running` and a handle.

TR4. `latest_on(t)` (`:91-97`): the row of that thread with the largest `startedAt`, an absent `startedAt` read as 0;
on a tie the later row in map order wins (`>=`). Rows of every workspace count. `None` when the thread has no row.

TR5. `thread_of_token(token)` (`:307-314`), **invariant 3**: the first row in map order whose `turnToken == token`,
`status == running` and `threadId` is set answers its thread. Else the first kept process (`keptAgents` order) whose
`turnToken == token`. Else `Err(HostError::new(NO_SUCH_TURN))`, no kind. A row that ended, or a running row with no
thread, never answers, even when it carries the token.

TR6. `tree_under(t)` (`:318-330`): builds `parents` first from rows (`threadId -> parentThreadId`, each row in map
order, a later row overwriting a value but not moving its key), then from thread records (`id -> parentThreadId`,
overwriting). Then level by level, at most `parents.len() + 1` levels: a level is every entry of `parents`, in its
insertion order, whose parent is in the previous level, not found yet, and not `t` itself. Answers the found ids,
level by level. A loop (`a` parent of `b`, `b` parent of `a`) ends; `t` is never in its own answer. A thread with no
children answers `[]`.

TR7. `restarts()` (`:333-338`): `replaces -> threadId`, rows first (rows with both set), then records, a record
overwriting a row's value for the same `replaces`.

TR8. `drives_thread(t, caller)` (`:346-350`): no scope (`scopeOf(caller)` absent, the person or a road string) is
true for every thread, `None` included. With a scope: `t` is set and `ctx.root_of(t) == scope.rootThreadId`.

TR9. `settles_thread(t, caller)` (`:353-356`): no scope is true; else `t == scope.threadId` or `t` is in
`tree_under(scope.threadId)`. A thread's lead and a sibling are false.

TR10. `thread_title(t)` (`:359-362`): the thread's rows sorted by `startedAt` ascending (absent 0, stable), folded by
`foldThreads`; the first thread's `title`, else `threadWord(t)` (the first 8 characters).

TR11. `stopped_behind(s)` (`:527-543`): `None` when `s.view.threadId` or `s.calls` is absent. For each call in
order, `threadsFollowed(call)`; `None` skips it. `opened` reads `tree_under(caller)`; `named` reads every row's
`threadId` in map order (repeats kept) that starts with one of the named refs. For each candidate in that order: skip
it when `leadAsks` holds no prompt for it or it is the caller; skip it when no row of it runs; else answer
`ThreadWaitingOn { threadId, workspaceId, sessionId: <that running row's view.id>, title: thread_title, prompt: a copy
with options copied }`. The first match answers; none answers `None`.

TR12. `stop_under(t, caller)` (`:546-558`): over `tree_under(t)` reversed (deepest first), one at a time: the
child's `latest_on` row, skipped unless it is `running`; `ctx.interrupt(row.id, caller).await`; an error warns
`thread <threadWord(child)> was not stopped with its root: <message>` and counts as not stopped; only an outcome
`accepted` adds the child. Answers the stopped children in the order stopped.

TR13. `notify_on(t)` (`:561-568`): the first row of `t` with `notify` set answers `{notify, by?: notifyBy, road?:
notifyRoad}`; `None` when none has.

TR14. `tellable(row)` (`:572`): the row is set and carries `claudeSessionId`.

TR15. `notify_reach(from)` (`:576-585`): breadth first from `from` less `NOTIFY_ME` ("me"), each thread visited once,
following each thread's `notify_on` targets less `me`. Answers an `IndexSet` in visit order. A loop ends; `from`
threads are in the answer.

TR16. `tell_as(s)` (`:615-618`): `{by: notifyBy?, road: notifyRoad?}`, each only where set.

TR17. `failure_of(id, turn_id)` (`:658-661`): the row `id`'s `turnLive.failure` only while that row's `turnId` is
`turn_id`; `None` otherwise or when the row is gone.

TR18. `usage_computer_of(r)` (`:1149`): `r.place`, else `here` for a local workspace, else `r.provider`, else `r.kind`.

TR19. `vaulted_for(agent, login_stands, vault)` (`:1152-1155`): `vault` defaults to `ctx.vault()` (empty when the
host wires none). An agent not in `CATALOG_AGENTS` holds nothing. `"token"` where `secretsOf` finds an OAuth token,
else `"key"` where it finds an API key, else `None`.

TR20. `usage_account_of` (`:1158-1165`) and `limit_details_due` (`:1169-1179`) compose usage.ts's `accountOf`,
`accountOnComputer` and `resetDetailsDue` with `harnessCatalog(harness).label ?? harness` as the agent's name and
`{id: usage_computer_of, name: ctx.computer_of(entry)}` as the computer. `limit_details_due` is false for an agent with
no `PLAN_RESETS` entry, and false when the ledger's `limits()` fails (no warning).

## Tests (`tests/threads_tree.rs`)

| Rule | Test |
|---|---|
| TR1 | `computer_silent_reads_the_class_not_the_words` |
| TR2 | `thread_runs_counts_a_running_row_with_no_handle_and_no_row_without_a_thread` |
| TR3 | `launching_on_and_running_on_answer_the_first_row_in_map_order` |
| TR4 | `latest_on_takes_the_newest_start_and_gives_a_tie_to_the_later_row`, `latest_on_reads_an_absent_start_as_zero` |
| TR5 | `invariant_3_a_token_no_running_row_or_kept_process_holds_is_refused`, `thread_of_token_reads_a_kept_process_after_the_rows` |
| TR6 | `tree_under_walks_level_by_level_in_parents_order`, `tree_under_ends_on_a_loop_and_never_holds_itself`, `tree_under_reads_a_record_past_a_row_that_fell_off` |
| TR7 | `restarts_reads_rows_then_records` |
| TR8 | `drives_thread_with_no_scope_is_every_thread_and_with_one_its_root` |
| TR9 | `settles_thread_is_the_scope_and_its_tree_never_its_lead` |
| TR10 | `thread_title_folds_the_rows_else_the_thread_word` |
| TR11 | `stopped_behind_names_the_question_at_the_bottom_of_the_tree`, `stopped_behind_skips_the_caller_and_a_thread_with_no_running_row` |
| TR12 | `stop_under_stops_deepest_first_and_counts_only_accepted`, `stop_under_warns_and_goes_on_past_a_failed_stop` |
| TR13, TR16 | `notify_on_answers_the_first_registering_row_with_its_scope_and_road` |
| TR14 | `tellable_needs_an_agent_session` |
| TR15 | `notify_reach_walks_targets_once_and_leaves_me_out` |
| TR17 | `failure_of_reads_only_the_turn_still_on_the_row` |
| TR18 | `usage_computer_of_is_place_then_here_then_provider_then_kind` |
| TR19 | `vaulted_for_is_token_before_key_and_nothing_for_an_unknown_agent` |
| TR20 | `limit_details_due_is_false_without_plan_resets_or_a_ledger` |
