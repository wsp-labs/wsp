# reach_daemon

Source: `packages/runtime/src/machines/reach.ts:199-372, 381-474` (about 270 of 483 lines) at `65f6b89c7`. Tests:
`packages/runtime/test/box-nap.test.ts:227-420` (the wait on a woken daemon, 8 with the `it.each` rows),
`wake-keeps-machine.test.ts:29`, `runtime-reach.test.ts:149-244` (the hello), `tree.test.ts:319-380, 432` (the
lead's tree), `changes.test.ts` and `local-workspace.test.ts` (the checkout read). Rust: `src/machines/reach/daemon.rs`.

## What it does

The roads to a machine's own daemon: one dial preamble (`dial_daemon`), the check that a woken machine's daemon
answers (`daemon_answer`), with a start of the daemon a minute into the wait (`starting_daemon`) and a look every 10
seconds for a daemon that exits as it starts (`ping_daemon`); the version a daemon announces in its hello
(`hello_version`); and two facts kept on the live entry and pushed on its status: the copy's checkout
(`read_checkout`) and a lead's tree of children (`read_tree`).

## Behaviour

1. Constants: `ASK_DAEMON_MS = 5_000`, `ASK_AGAIN_MS = 500`, `START_DAEMON_AFTER_MS = 60_000`, `LOOK_EXITS_EVERY_MS =
   10_000`.
2. `dial_daemon(entry, deadline, {on_event, heartbeat_ms})` (:203-208): the preview route (`entry.ws.daemon_reach()`)
   under `until(deadline, "preview route")`, then this runtime's token (`daemon_token_of(machine)`) under
   `until(deadline, "daemon token")`; no token answers `None`; else `connect_daemon({previewUrl, token, onEvent (a
   no-op by default), heartbeatMs?})`. The caller owns the link and closes it.
3. `daemon_answer(entry, quit)` (:222-272). The budget is `lifecycle_of(entry).budgets.daemonAnswersMs`. The stop is the
   entry's `wakeStop` signal, `quit`, or either. Every wait below races the stop, which fails it with `WAKE_STOPPED`
   (at once when already stopped).
   - A machine with a daemon-answers road: `deadline = now + budget`; loop: ask `daemon_answers(min(budget, 5_000))`
     under `until(deadline, "daemon answer")`; true answers none (no fault); false past the deadline answers
     `nothing listens on the daemon's port inside <machine>`; else wait 500 ms on the clock and ask again. Any error
     (the deadline, the stop, the road's own) answers `the daemon on <machine> could not be asked (<message>)`.
   - Else no preview route answers none.
   - Else `deadline = now + budget`; dial with `heartbeat_ms = budget`; a dial that lands after the stop fired closes
     its link. No token: `exec("true")` under `until(deadline, "guest exec")` answers none. Else `link.ready` and
     `link.request("ping")` each under `until(deadline, ...)`, then none. Any error answers `daemon on <machine> did
     not answer within <budget> ms (<message>)`. The link is closed on every exit.
4. `starting_daemon(entry, quit)` (:280-315): a machine without a start road is `daemon_answer`. Otherwise a timer at
   60 s: it warns and starts the daemon without waiting on the answer: `late = "daemon on <machine> (workspace <id>)
   had not answered <js_round(elapsed / 1000)> s into the wait, so wsp started it"`; an answer warns `<late> (exit
   <n>)` or `<late> (exit <n>: <stderr.trim() last 200>)` and, for a non-zero exit, arms again; a failure warns
   `<late>, and the start failed (<message>)` and arms again. Re-arming stops once the wait is over. The wait is
   `daemon_answer(entry, quit)`; on any exit the pending timer is cancelled.
5. `ping_daemon(entry)` (:321-352): with a `daemon_exits` road, a look every 10 s: none looks again; words set `exited
   = "the daemon on <machine> exits as it starts (<words>)"` and abort the quit signal; a failed look looks again.
   Looks stop once the wait is over. The answer is none when the wait answered none, else `exited` when set, else the
   wait's fault. The pending look is cancelled on every exit.
6. `hello_version(entry)` (:357-372): no preview route answers `None`. Else, within `daemon_hello_timeout_ms` of now,
   dial with an event handler that resolves on the first `daemon.hello` frame's `version`; no token answers `None`;
   else the version under `until(deadline, "daemon hello")`. Any failure answers `None`. The link is closed on every
   exit.
7. `read_checkout(entry, force)` (:386-421): unforced, a fact younger than `CHECKOUT_TTL_MS` (10,000 ms, `<`) answers
   it. A record not `running` whose computer does not serve it (`served_by_its_computer` none) answers the held fact.
   A read in flight is joined. The read: one `git.status` frame through `with_daemon` with `cwd = checkout_of(record)`,
   parsed; the fact keys in order: `branch` (head), `ahead`, `behind`, `changed` (entries whose `xy` is not `!!`),
   `editsUnread: true?`, `countsUnknown: true?`, `stashes?`, `head?` (only for an oid matching `^[0-9a-f]{7,40}$`,
   lower case), `readAt: now`. It is set on the entry; when it differs from the record's kept checkout (both compared as
   JSON text without `readAt`), it is written on the record and persisted; then `status_now(entry)`. Any failure keeps
   the last fact (its `readAt` says how old). Answers the entry's fact. The in-flight slot is cleared when it settles.
8. `read_tree(lead)` (:428-450): a read in flight is joined. Children are live entries whose `parentWorkspaceId` is the
   lead's id and that are not `creating`. None: a held tree is removed and the status pushed; answers none. Else
   `lead_branch = read_checkout(lead, false).branch ?? record.base ?? project.base ?? project.defaultBranch`; every
   child's row read at once (concurrently, answered in child order); `lead.tree = {leadBranch, children, readAt:
   now}`; `status_now(lead)`; answers it.
9. `tree_child_of(child, lead_branch)` (:456-468): `branch = read_checkout(child, false).branch ?? worktree.branch ??
   record.base ?? ""`; `thread` is the first session row on the child with a thread id. The compare is skipped for an
   empty branch, `DETACHED_HEAD`, or a remote with no host; otherwise `on_this_computer(git.branchCompare {cwd: home,
   remote, base: tree.merged.head ?? lead_branch, head: branch})`, parsed; any failure is no compare. Counted: none
   with no compare; `{pushed: true, aheadOfLead?, behindLead?}` when pushed; `{pushed: false}` otherwise. The row:
   `{workspaceId, threadId?, branch, ...counted, ...record.tree}` (the record's tree keys last, overriding).
10. `read_lead_of(child)` (:471-474): starts `read_tree` of the child's lead where it is live, without waiting.

## Edge cases

- A daemon that answers two and a half minutes into a 15-minute budget passes on the same machine (box-nap :227).
- A start road that throws, or exits 124, is asked again a minute later (box-nap :308 rows).
- A daemon that answered before the minute is never started, even after the minute (:340).
- A stop pulled during the wait lets go at once with `WAKE_STOPPED` (:401).
- `read_checkout` with `force` reads even within the TTL; a stopped machine's computer that serves it still reads.

## Behaviour nobody wrote down

- Rule 3.15: every `until` here takes the context clock. The TS passes none in the daemon-answers branch and in
  `hello_version` (raw timers and `Date.now()`); the port uses the clock for both (a decision, not a change).
- Rule 3.9: `checkout_reading` and `tree_reading` are joins on the live entry.
- Rule 3.3: no borrow of the entry across a dial, an exec, `with_daemon`, `persist` or `status_now`.

## Decisions

1. The stop race is a `select!` against the signal; an already-aborted signal fails before the first ask.
2. The quit signal is a `CancellationToken`-like handle from wsp-core; `AbortSignal.any` is a token tripped by either.

## After the port

- `hello_version` measures its deadline with `Date.now()` (reach.ts:359) while the rest of the area reads the clock.

## Tests (tests/reach_daemon.rs)

| Rule | Test (TS case) |
|---|---|
| 3 | `a_daemon_answering_late_within_the_budget_passes` (box-nap :227, :244) |
| 4 | `a_daemon_never_started_is_started_a_minute_in` (:288) |
| 4 | `asks_for_the_start_again_a_minute_after_one_that_throws` (:308 row 1) |
| 4 | `asks_for_the_start_again_a_minute_after_one_that_runs_out_its_timeout` (:308 row 2) |
| 4 | `starts_nothing_on_a_machine_whose_daemon_answered` (:340) |
| 3 | `a_daemon_that_never_answers_fails_when_the_budget_is_out` (:359) |
| 5 | `a_daemon_that_exits_as_it_starts_fails_with_its_own_words` (:377) |
| 3 | `a_stop_pulled_during_the_wait_lets_go_at_once` (:401) |
| 3 | `the_edge_road_pings_through_the_link_and_closes_it` (wake-keeps-machine :29) |
| 6 | `hello_version_reads_the_hello_and_closes_the_link` (runtime-reach :149) |
| 6 | `hello_version_is_none_without_a_route_or_a_token` (new) |
| 7 | `read_checkout_answers_the_held_fact_within_the_ttl_and_reads_when_forced` (new, from changes.test.ts) |
| 7 | `read_checkout_writes_the_record_only_when_the_fact_moved` (new) |
| 7 | `read_checkout_joins_a_read_in_flight` (new) |
| 8, 9 | `reads_each_childs_branch_against_the_leads` (tree :320) |
| 9 | `a_child_merged_counts_against_the_commit_the_merge_took` (tree :432) |
| 8 | `a_lead_whose_children_are_gone_loses_its_tree` (new) |
