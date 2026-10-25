# views_activity
Source: the functions of `packages/protocol/src/views/activity.ts` (578 lines) at a3b870a6e that wsp-wire left here: `cloudActivityLine` (:456), `notJoinedActivityLine` (:458), `agentEndLine`, `agentUnderLine`, `agentEndFix` (:471-473), `endPlan` (:487-521), `processesLeftLine` with its private cap `LEFT_NAMED` (:549-560), `processesLeftRefusal` (:567-568) and `processesLeftOf` (:571-575). The rest of the file is wsp-wire's (its spec/activity.md): the shapes (`ActivityRead`, `ActivityScope`, `ProcEntry`, `EndPlan`, `LeftThread`), the placement (`placeProcs` and its `kidsOf`, `whose`, `agent`, `foldsInto`, `rootOf`, `endable`, `reachOf`), `activityOf`, `processesLeft`, `endHeads`, `standingAfterStop`, `processesLeftFix`, the other refusal lines (`endGoneLine`, `endReusedLine`, `wspOwnLine`, `notThreadsLine`, `notStartedLine`, `boxBehindActivityLine`, `notReadLine`) and the constants (`SAME_START_MS`, `END_AGAIN_FIX`, `WSP_OWN_FIX`, `NOT_STARTED_FIX`, `NOT_THREADS_FIX`, ...). Tests: `packages/protocol/test/activity.test.ts` (12 of its 22 cases ported in `tests/views_activity.rs`: :46, the nine of :79-137, :197 and :250; the other 10 test wsp-wire's placement and view and are ported there). No pinned module.

## What it does
The End of one process a thread or the person asked for: the refusals, in a fixed order, and the processes the End takes or the kept agent it frees through its thread. The sentence and refusal a settle or a delete gets over processes it would leave running, and the threads that refusal carries back to a client. The lines naming a thread whose processes this host cannot read.

## Behaviour
1. `cloud_activity_line(thread_id)`, `not_joined_activity_line(thread_id, name)` (:456, :458), `agent_end_line(pid, scope)`, `agent_under_line(pid, scope)` and `agent_end_fix(scope)` (:471-473) are fixed sentences that name the thread by `thread_word` and quote the scope's title in double quotes.
2. `end_plan(read, pid, started_at, may)` places the read once (wsp-wire's placement, zombies already dropped) and refuses in this order (:488-497): no live row with the pid is `end_gone_line`, END_AGAIN_FIX, kind not-found; `may(reach_of(pid))` false is `not_threads_line`, NOT_THREADS_FIX, usage, checked before anything that reads the process, so the refusal names nothing of it; the read's daemon or host pid is `wsp_own_line`, WSP_OWN_FIX, usage; a pid no thread owns whose `owner.by` is none of root, fork, group, env, cgroup is `not_started_line`, NOT_STARTED_FIX, usage; a start more than SAME_START_MS (2000 ms) from `started_at` is `end_reused_line`, END_AGAIN_FIX, not-found.
3. Then the agent (:498-508). A pid that is a thread's agent, or folds into one, where a scope of the read names that thread: the scope's `kept` equal to the pid's root answers `Kept(scope)`, else `agent_end_line` with `agent_end_fix`, usage. A row the daemon stamped kind agent: its owner's scope gives the same refusal; with no scope, "<pid> is an agent, which only its thread's stop ends:" with "stop its thread's turn with wsp stop.", usage.
4. A pid the placement does not call endable is `not_started_line`, usage (:509). Else the targets (:510-520): the process, then each child in `kids_of` order, depth first. A target that is an agent or folds into one refuses with `agent_under_line(pid, scope)`, naming the pid asked and not the child; the daemon or host among them refuses with `wsp_own_line(child)`. The answer is `Targets`, the rows as read.
5. `processes_left_line(verb, left, theirs)` (:553-560): one row per thread and process, in that order; the first 8 each read "<pid> <name> (<kind>[, :<port> :<port>][, the person's to end]) of <thread word>[ on <on>]", joined ", "; past 8 it adds " and <n> more". It opens "a thread to settle" or "the thread to delete", then " still runs a process" for one row, else " still runs <n> processes", then ": ", the names and a closing ":". `theirs` absent is the empty set.
6. `processes_left_refusal(verb, left, theirs)` (:567-568) is `refusal(processes_left_line(..), processes_left_fix(verb, theirs), processes-left)` with `left` carried beside it. `processes_left_of(e)` (:571-575): the threads where the kind is processes-left and `left` reads as a list of LeftThread; anything else, one bad entry included, is None.

## Edge cases
- A start 1000 ms off the listed one is the same process (test :100); exactly 2000 ms off still is, since the check is `>` (:497).
- A thread asking with a wrong start hears only `not_threads_line`, never the process's name (test :113).
- A process placed by `owner.by` folder or coalition alone is refused as not started; one marked env with a thread no longer among the scopes is ended for the person (test :122).
- `processes_left_line` with 9 rows names 8 and adds " and 1 more"; with no rows it reads "... still runs 0 processes: :" (read off :554-559).
- `processes_left_of` on a processes-left error with no `left` is None; with `left` empty it is an empty list.

## Behaviour nobody wrote down
- Rules 8.1 and 8.5: `thread_word` is the id's first 8 UTF-16 units, never half a surrogate pair.
- Rule 8.11: pids, ports and counts in these sentences are `String(n)`; the f64 pid goes through `js::number_string`.
- `theirs` keeps insertion order, which the fix's list of pids reads (`[...theirs]`, :564).
- A refusal met during the walk of rule 4 ends the plan: no partial targets come back.

## Tapes
`wsp activity end` and the `activity_end` tool (the plan and every refusal); `wsp thread settle` and `wsp delete` with processes ask, and their tools, refused with `left` beside the error, which the app's client reads (apps/web/src/protocol/client.ts:503); `wsp activity` naming a thread on a cloud machine or on a computer this host has not joined (runtime/src/activity.ts:296-298).

## Decisions
1. `end_plan` answers `Result<EndPlan, crate::exit::HostError>`: each TS throw is the Err, built by `refusal` with kind not-found or usage. `may` is `&dyn Fn(Option<&str>) -> bool`.
2. Where the TS takes a number for a pid or a start (`end_plan`'s `pid` and `started_at`, the pid of `agent_end_line` and `agent_under_line`), the Rust takes f64; wsp-wire's lines take i64.
3. The verb union is `LeftVerb { Settle, Delete }`; `theirs?: ReadonlySet<number>` is `Option<&IndexSet<u32>>`, None the empty set.
4. `processes_left_refusal` answers a `HostError` whose `left` holds the threads as JSON values; `processes_left_of` takes a `&HostError` rather than any value and answers `Option<Vec<LeftThread>>` for the TS `{ left? }`.

## After the port
- The walk of rule 4 (:511-518) keeps no seen set: a read where two live rows name each other as parent recurses until the stack runs out, a RangeError in the TS.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `an_end_s_plan::refuses_each_process_of_the_stretch_to_the_agent_and_one_with_the_agent_under_it`, `an_end_s_plan::refuses_the_native_codex_that_codex_s_npm_launcher_runs_as_its_one_child_which_i`; `cloud_activity_line`, `not_joined_activity_line` and `agent_end_fix` are pinned by no test |
| 2 | `an_end_s_plan::refuses_a_pid_that_is_gone_and_one_that_names_another_process_now`, `an_end_s_plan::refuses_wsp_s_own_and_for_a_thread_anything_outside_its_tree`, `an_end_s_plan::tells_a_thread_nothing_of_a_process_outside_its_tree_whatever_start_it_names`, `an_end_s_plan::refuses_for_the_person_a_process_wsp_did_not_start_one_its_folder_or_coalition_a`, `an_end_s_plan::lets_no_thread_end_a_process_by_the_wsp_thread_mark_alone_which_any_process_can_`, `what_a_thread_s_pane_and_its_slate_started::*` |
| 3 | `an_end_s_plan::refuses_each_process_of_the_stretch_to_the_agent_and_one_with_the_agent_under_it`, `an_end_s_plan::refuses_the_native_codex_that_codex_s_npm_launcher_runs_as_its_one_child_which_i`, `an_end_s_plan::frees_a_kept_agent_through_its_thread_instead_of_a_signal`; the daemon-stamped agent with no scope is pinned by no test |
| 4 | `an_end_s_plan::takes_the_process_and_everything_under_it_each_by_the_start_it_was_read_with`, `an_end_s_plan::refuses_each_process_of_the_stretch_to_the_agent_and_one_with_the_agent_under_it`, `what_a_thread_s_pane_and_its_slate_started::*`, `an_agent_outside_this_host_s_threads::*` |
| 5 | `a_snapshot_read_by_thread::*` (one row, no ports, no `theirs`, no `on`); the cap of 8, ports, `theirs` and `on` are pinned by no test |
| 6 | none |
