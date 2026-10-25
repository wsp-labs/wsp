# activity

Source: `packages/protocol/src/views/activity.ts` (578 lines; this unit takes all but `endPlan`, `processesLeftLine`,
`processesLeftRefusal`, `processesLeftOf` and the four `threadWord` lines, which are wsp-words'), `views/activity-fold.ts`
(48), `views/activity-names.ts` (113), and `cgroupMemory`/`inCgroup` of `thread-cgroup.ts` (lines 20-27), at a3b870a6e.
Tests: `packages/protocol/test/activity.test.ts` (22 cases; 11 are this unit's, the End's plan is wsp-words'),
ported in `tests/views_activity.rs`; hard cases in `tests/views_activity_rules.rs`, whose expectations
(`fixtures/rules/oracle.json`) were written by running the TS. Rust: `src/views/activity.rs`, `activity_fold.rs`,
`activity_names.rs`, `src/types/thread_cgroup.rs`.

## What it does

One reading of a computer's process list: which thread each process is (or wsp's own, or wsp's with no thread),
what kind it is, which processes are a thread's agent and which rows fold into the agent's, and what an End may
reach. Activity, `wsp activity`, settle and delete all read it through `place_procs`. `activity_of` turns one read into
the view the page draws; `processes_left` is what a settle or delete would leave running.

## Behaviour

1. `place_procs` drops every row whose `state` starts with `Z` before anything else (activity.ts:239); `procs` keeps
   the read's order. `by_pid` keeps the last row of a pid listed twice (`new Map`, :240).
2. Children: a row is its parent's child only if `ppid != pid` and the parent is live (:242), in the read's order.
3. Tabs: for each thread in order, each of its `tabs` that the app map calls `Tab` and no earlier thread took is that
   thread's (:248).
4. For each thread in order (:254-283): `named` is its roots followed by every row the daemon stamped `kind: agent`
   with `owner.thread` = this thread, de-duplicated keeping first position, limited to live pids (:258-259). Each of
   those goes to `stand_ins` in that order. With no roots and a cgroup, the heads are the live rows `in_cgroup` of it
   whose parent is not also in it (:263-266). For each root of `named` then heads (de-duplicated, first kept): the path
   is `agent_path` with `is_agent` = "pid recorded in `agents`"; if none, `agent_path` with "a name in `proc_bins`
   equals `bin`, or any of `agent_bins` when the thread names no bin". When the recorded path was found, it is
   extended down through only children whose names match (:271). With a path: its last pid is the agent
   (`agent[at] = thread`, `folds_into` loses `at`, `root_of[at] = root`), and every earlier pid not already an agent
   folds into `at` with `root_of = root`.
5. Each stand-in root that is neither an agent nor folded becomes an agent of its thread with itself as root (:284-288).
6. Groups (:290-305): over agent pids then folded pids (map order), each pid's `pgid` and its root map to its agent,
   first writer wins. Then every live row with a `pgid`, not already agent or folded, whose command line
   `is_turn_plumbing`, folds into its group's agent unless its `owner.thread` names a known thread other than that
   agent's; its `root_of` is the agent's root.
7. Whose (:307-338), worked out once per pid in the read's order, recursing to parents with a seen set: a known
   `owner.thread` → that thread; else the agent stretch it is in → its thread; else a tab a thread took → that thread;
   else wsp's part (daemon pid, host pid, any app pid, `kind` app or wsp) → `Wsp`; else `owner.by` in root, fork,
   group, env, cgroup → `Loose`; else the first thread whose cgroup it stands in; else a command line matching
   `SLATE_FOLDER` → the first thread whose id appears as `slates/<id>/` in it, or `Loose` when none; else a row with a
   pty → `Loose`; else its parent's answer (parent not yet seen), then `Loose` for `owner.by` coalition. No answer is
   kept as "nobody" and the pid is absent from `whose`.
8. `kind_of` (:341-368) is cached per pid and sets `other` before recursing, so a parent loop reads `other`. Order:
   app part; agent or folded → agent; daemon or host with no daemon kind → wsp; a daemon kind other than `terminal`,
   or `terminal` with a pty → that kind; a pty → terminal; not a thread's row and a name in `agent_bins` → agent;
   `SLATE_FOLDER` → slate; `TEST_LINE` → test; `BUILD_LINE` → build; listening or `DEV_LINE` → dev; parent dev, test or
   build → the same; parent agent, parent not folded, comm not a shell (sh bash zsh fish dash) → tool; else other. The
   regexes are at :170-174; `\b` is an ASCII word boundary and `(?! serve)` a lookahead (no `regex` crate support:
   hand-match).
9. `needed` = a live wsp part that the app map does not call `Tab` (:371). `endable` = live, has a `whose`, not a
   thread's agent (agent, folded, or daemon kind agent), not needed, not an app pid, and either a thread's or started
   by wsp (:377-381). `reach_of` (:382-393): only a thread's row; its thread unless `owner.by` is env; for env: the
   agent stretch, then the first thread whose cgroup holds it, then its parent's reach.
10. `activity_of` (:426-445): threads in the read's scope order, only those with a member; members in the placement's
    procs order; memory is the members' rss sum unless `cgroup_memory(read.cgroups, scope.cgroup)` has one; cpu the
    sum. Rows (:411-423): heads are members whose parent is not a member or who are their own parent; each sibling list
    is sorted by cpu descending, then pid ascending (`by_proc_column("cpu")`), depth-first; `command` is the comm when
    the command line is empty; `ports` is a copy of the read's list for the pid, `[]` when none. `wsp` and `unowned`
    are the `Wsp` and `Loose` rows the same way. The view's keys: computer, at, threads, wsp, unowned.
11. `processes_left` (:526-540): for each asked thread in view order, the rows with no step up the parent chain (at
    most as many steps as rows) that is kind agent or turn plumbing; a thread left with none is dropped.
12. `fold_tree`: a row folds into `folds_into(pid)` only when that is another pid in the list; a folded row is held by
    its target in list order; others hang under their first standing ancestor, a folded ancestor standing as its
    target unless that target is the row itself; at most `list.len() + 1` steps up (activity-fold.ts:16-48).

## Edge cases

- `agent_path` returns None for a root no row has; breadth first, kids in `kids_of` order, never re-visiting a pid or
  the root (:140-159). `is_turn_plumbing` matches the whole line: `^bash \S*/[0-9a-f]{12}\.sh$` or
  `^tail -n \+1 -f \S*/[0-9a-f]{12}\.in$`, lower-case hex only (rules test `is_turn_plumbing`).
- `proc_bins` leaves out an empty comm; `proc_name` falls back to argv[0]'s base name for an empty comm, and keeps a
  package's `@scope` while cutting a trailing `@version` (`(?<=.)@[^@/]*$`).
- `cgroup_memory` takes the first entry for the cgroup; `in_cgroup` needs `at == cgroup` or a `cgroup/` prefix.
- A pid asked of a placement that is not in the read: kind other, not needed, not endable, no reach, no kids.

## Behaviour nobody wrote down

- Map order is state (rule 4.26): `whose`, `agent`, `folds_into`, `root_of` are `IndexMap`s; a TS `delete` is
  `shift_remove`, and a key set again after a delete goes last.
- `end_reused_line` writes epoch ms as `Date.toISOString` (`2026-10-25T10:00:00.000Z`, milliseconds always shown).
- `box_behind_activity_line` calls `crate::wire::daemon_version::place_behind_line(name, "its daemon reads no
  processes in one read")`, the wire writer's.

## Decisions

1. Pids are `u32` as wsp-frames reads them; `ActivityProc.pid` stays `i64` as zod's `.int()` says.
2. `Placement` borrows its input; closures in the TS are methods. The words crate's `end_plan` reads the placement
   through `with_read_placement`.

## After the port

- A row listed twice keeps the later row in `by_pid` but both in `procs` (rule 1): two rows can share a tree slot.

## Rules pinned → tests

| Rule | Test (`tests/views_activity_rules.rs` unless named) |
|---|---|
| 1-9 maps, order, zombies, whose, kinds, need, End, reach | `a_placement::every_map_of_a_placement_holds_the_entries_and_the_order_the_ts_maps_hold`, `a_placement::each_pid_s_kind_need_end_and_reach_read_in_the_procs_order_are_the_ts_answers` (7 snapshots) |
| unknown pid | `a_placement::a_pid_outside_the_read_is_other_needed_by_nobody_ended_by_nobody_and_reached_by_none` |
| 10 view | `the_view::the_view_is_the_ts_view_key_for_key_thread_order_tree_order_depth_totals_ports_and_fallbacks`; `views_activity.rs` 34, 52, 185, 234, 238 |
| 11 left | `the_view::what_a_thread_leaves_is_what_stands_under_no_agent_and_no_turn_plumbing_and_a_thread_left_none_is_dropped`; `views_activity.rs` 46, 156, 208 |
| 12 fold | `views_activity.rs` 217, 224 |
| agent path | `the_stretch_to_the_agent::the_path_runs_breadth_first_to_the_shallowest_agent_then_on_through_an_only_child_that_is_one_too` |
| names, bins, plumbing | `names::*` (5 tests, 34 command lines) |
| cgroups | `cgroups::*` |
| lines, ISO time, fix | `the_lines::*`; `standing_after_stop`, `end_heads` in `the_view::*` |
