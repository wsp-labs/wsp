# rules: what a caller may see and drive (`account/rules.ts`, 334 lines)

The one home of "what a caller may see and drive" (`refusalFor()`, packages/runtime/AGENTS.md's table). Every
expected value in the tests is the TS's own: `oracle/rules.oracle.ts` builds one world (projects on this computer,
two joined computers and a cloud; fifteen live records; eleven thread records; rows for two threads from before thread
records), runs `rulesArea` on a fake context over it, and asks every door across every record and seventeen callers.
The world is written into `tests/fixtures/oracle/rules.json` beside the answers, and the Rust fake builds its context
from that same world, so the two cannot drift.

## The context the area reads (`RulesCtx`)

`opts.noMachinesLine`, `store.put`, `placeDoor?.nameOf`, `landing` (shared, mutable), `live`, `projectsHeld` (shared,
mutable), `sessions.size`, `threadRecords`, `preferences.get/set`, `backendFor(record).capabilities`,
`moduleOf(kind).relayed/turnReach/wspMcp`, `sameRepository`, `projectHeld` (throws `noSuchProjectLine` for an id not
held), `kindOf(computer)`, `placeName(id)`, `rowsOn(threadId)`, `agentsHeld(record)`, `placeIdOf(record)`.

`landing` and `projectsHeld` are maps other areas read too, so the trait hands them out as shared cells
(`Rc<RefCell<IndexMap<..>>>`); the area changes them in place.

## Rules

**The memory writes**

- R1 `rememberProject` (15-18) sets the project in `projectsHeld` first (an existing id keeps its map position), then
  `store.put("projects", id, project)`. A failing put leaves memory already moved. Test `r1_remember_project_moves_the_map_then_the_store`.
- R2 `rememberTarget` (20-25) reads the preferences; if `target.workspace` already names the record, it writes
  nothing. Otherwise it sets the patch `{ target: { workspace: id } }` alone, no `project` key.
  Tests `r2_remember_target_writes_nothing_when_it_already_names_it`, `r2_remember_target_sets_workspace_alone`.

**The capability gates**

- R3 `cannotLine` (30-35), in this order: a kind wsp does not drive (`kindWords(kind).driven` false: local and
  place) → `undrivenRefusal(name, machineWord(kind), action)`; a backend whose `sizes` is empty →
  `opts.noMachinesLine`, else `NO_PROVIDER_LINE`; else `providerCannotRefusal(name, machineWord(kind), action)`.
  Every gate throws it as a plain error (no kind).
- R4 `refuseCannot(entry, can, action)` (43-45) passes only where `capabilities[can] === true`: a capability absent
  from the table refuses. `refusePauseless` (49-51) passes where `pauseMode` is present at all, whatever its value.
  `pauses` (48) is that test. `holdsSlot` (56) is `pauses && phaseHoldsSlot(record)`.
  Tests `r3_r4_refuse_cannot_matrix`, `r3_no_machines_line_outranks_the_provider_line`,
  `r4_refuse_pauseless_matrix`, `r4_pauses_and_holds_slot_matrix`.

**The kind rule, the tree rule, the project rule and the computer rule**

- R5 `drives(record, caller)` (61): any caller whose road is not `relayed` drives; a relayed one drives only a kind
  whose module's `relayed(machineId)` says so. A `Caller::Scoped` whose origin is relayed is relayed too. Test
  `r5_drives_matrix`.
- R6 `inTree` (65-73): no scope, or a record with no id, is in; else the record's id is the scope's workspace, or its
  `rootThreadId` is the scope's root, or its worktree was `madeFor` the scope's root.
- R7 `ofThreadsRepository` (80-84): the caller's own workspace's project (`live[scope.workspaceId].project`) is
  the project, or `sameRepository(projectHeld(mine), projectHeld(project), caller)`. No scope, or a scope whose
  workspace is not held: false. A project id not held throws `noSuchProjectLine` out of `projectHeld` (only
  reached when it differs from the thread's own). Test `r7_of_threads_repository_matrix`.
- R8 `projectReached` (91-99): false without a scope or off the repository; true for the thread's own project;
  false for a project on a kind that copies a folder (this computer: the person's); else true where the asking
  workspace copies a folder, or where neither the asking kind nor the project's kind runs in a folder. Test
  `r8_project_reached_matrix`.
- R9 `opensIn` (102-103): a scope, a record that runs in a folder, has no worktree, and whose project is reached.
  Test `r9_opens_in_matrix`.
- R10 `boxOf` (106-111) is defined only for a thread whose own workspace runs in a folder and does not copy one (a
  thread on a joined computer). `leadOf` (114-119) is undefined for a thread that is its own root, or whose root's
  record is not held (read through `recordOfThread`, R16); else the lead as a `here` caller scoped to the lead's
  own thread, workspace and root.
- R11 `elsewhereRefusal(caller, project, word)` (127-137): nothing off a joined computer or for a project on the
  asking computer. Then, in order: off the repository → `refusal(spawnRepositoryRefusal(threadId,
  projectHeld(asking.project).name, word), SPAWN_REPOSITORY_FIX, "usage")`; the lead reaches the project →
  usage `childToLeadsComputerLine(from, to, lead)`; the lead runs on the project's computer →
  `refusal(spawnFolderRefusal(threadId, word), SPAWN_FOLDER_FIX, "usage")`; else usage
  `childOnAnotherComputerLine(from, to)`. `from` and `to` are `placeName` of the two computers. Test
  `r11_elsewhere_refusal_matrix`.
- R12 `awayOn` (140-144) reads the record's project with `projectsHeld.get` (never throws); `awayFor` (157-160)
  answers usage `elsewhereWorkspaceLine(word, placeName(on), placeName(box), act, leadActsOn(record, caller))`;
  `actsOn` (147) is `drives && !away`; `leadActsOn` (150-154) is the lead's thread id where `refusalFor(record,
  leadCaller)` is nothing. Tests `r12_acts_on_matrix`, `r12_away_for_matrix`, `r12_lead_acts_on_matrix`.
- R13 `talksToItsTree` (164-168): a thread on a joined computer, or a thread whose own record has a `lead` (a
  sub-thread). Test `r13_talks_to_its_tree_matrix`.
- R14 `refusalFor(record, caller)` (174-186), the first that answers: no record → nothing; the kind rule →
  `relayedRefusal(name)`; no scope → nothing; the project rule (only when the thread's own project is held and the
  record names a project) → `spawnRepositoryRefusal(threadId, projectHeld(mine).name, projectHeld(record.project).name)`;
  the computer rule → `awayFor(record, caller, "work", record.name).message`; the tree rule →
  `spawnReachRefusal(threadId, name)`. Tests `r14_refusal_for_matrix` (17 callers x 17 records),
  `r14_project_rule_before_computer_rule_before_tree_rule`, `r14_no_record_is_nobodys_to_refuse`.
- R15 `refuseRelayed(record, caller, act = "work", thread?)` (194-201): nothing when `refusalFor` is nothing. A
  caller with no scope: a plain error with the line. With `thread` named and a record: `awayFor(record, caller,
  act, "thread " + threadWord(thread))`, else usage `treeThreadOutOfReachLine(thread, scope.threadId)`. Otherwise:
  where the lead acts on the record, `awayFor(record, caller, act, record.name)`; else
  `notFoundRefusal(noWorkspaceRefusal())` (kind `not-found`, "no such workspace"): a thread learns only absence.
  Test `r15_refuse_relayed_matrix`.
- R16 `refuseNamed(id, caller)` (206-210): a thread naming an id this host does not hold reads the same not-found
  error as one outside its tree; a caller with no scope reads nothing for an unknown id. Then R15. Test
  `r16_refuse_named_matrix`, `r16_a_thread_reads_absence_for_an_unknown_id`.
- R17 `held()` (212) is every live entry not `creating`, in the live map's order; `listedFor` (216) is those with no
  refusal, in that order. Test `r17_listed_for_matrix`, `r17_a_creating_record_is_never_listed`.
- R18 `refuseRecording(named, caller)` (220-222): a relayed road (string or scoped) throws plain
  `relayedRecordRefusal(named)`. Test `r18_refuse_recording_matrix`.

**The tree and its caps**

- R19 `recordOfThread(threadId)` (231-234): the thread record's workspace, else the first row's workspace
  (`rowsOn(threadId)[0]`), then `live`. `parentOf` (249) is the record's `parentThreadId`, else the first row
  that carries one. `rootOf` (262) is the record's `rootThreadId`, else the first row that carries one, else the
  thread itself. Tests `r19_parent_of_table`, `r19_root_of_table`.
- R20 `agentsOf(record)` (235-238): the caps of the record its root runs on (the record itself where it names no
  root), through `agentsHeld`; nothing when that root's record is not held. Test `r20_agents_of_table`.
- R21 `agentsHeldAt` (240-245) names where the caps sit: `{ workspace: name }` where that record has its own
  `agents`, else `{ computer: placeDoor.nameOf(placeIdOf(record) ?? "here") }`. Seen through the depth refusal's
  words (R24).
- R22 `depthUnderRoot` (250-260) walks parents; the walk is bounded by `sessions.size + threadRecords.size + 1`
  steps, so a loop of parents answers that bound. With the fixture's loop (4 sessions, 13 records) the depth reads
  17. Test `r22_a_parent_loop_is_bounded`.
- R23 `spawnGuard(act, caller)` (283-324), in order, the first that answers:
  1. no scope → a no-op release;
  2. the thread's own workspace not held → plain `folderGoneRefusal(threadId, act)`;
  3. `rooted` = own record when the thread is its own root, else `recordOfThread(root)`; not held → plain
     `rootGoneRefusal(threadId, act)`; `over` = `[rooted]`, plus own record where it is another record that runs in
     a folder;
  4. an act outside `SPAWN_ACTS_ALLOWED` → `actRefusal` (usage, `spawnActRefusal(threadId, act, placeOfThread)`);
  5. a sub-thread (own record has `lead`) asking `thread_new` or `fork` → `subStartsNothingRefusal(threadId,
     lead.threadId)` (usage, with its fix);
  6. `send`, `bring_back`, `delete_sub` → a no-op release, whatever the depth;
  7. depth ≥ the lowest `maxDepth` over `over` (ties keep the first, the root's) → plain `spawnDepthRefusal(threadId,
     depth, maxDepth, agentsHeldAt(deepest))`;
  8. any act but `fork` → a no-op release;
  9. `standing` = live entries (creating ones included) whose `rootThreadId` is the root and whose
     `workspaceState({ phase })` is neither `gone` nor `released`; with `held = landing[root] ?? 0`, `standing + held
     >= min(maxMachines)` → plain `spawnCapRefusal(root, standing + held, maxMachines)`;
  10. else `landing[root] = held + 1` in the same step, and the release decrements once (a second call does
      nothing), deleting the key at zero.
  Tests `r23_spawn_guard_matrix` (17 callers x 11 acts, the landing map after each),
  `r23_landing_slot_taken_in_the_same_step_and_freed_once`, `r23_a_sub_thread_starts_nothing`,
  `r23_send_bring_back_delete_sub_pass_at_the_depth_cap`, `r23_a_creating_record_counts_against_the_cap`.
- R24 A root whose own record names a root that is gone has no caps (`agentsOf` is nothing) and the TS reads
  `policy!.maxDepth` off undefined: every act past step 6 throws the TypeError's message, `Cannot read properties
  of undefined (reading 'maxDepth')`. The port answers `HostError::new` with that message (pinned as the TS does;
  after-port ticket 1). Test `r24_a_root_with_no_caps_fails_as_the_ts_does`.
- R25 `agentsReach(entry)` (265-270): the kind's `turnReach`, nothing where it has none; then `wsp` added after the
  reach's own keys where `wspMcp` answers. Test `r25_agents_reach_table`.
- R26 `treeOf` (327-328), `placeOfThread` (273-276: the computer's name where the kind runs in a folder, else the
  kind's machine word; nothing for a thread whose workspace is not held), `actRefusal` (279). Tests
  `r26_tree_of`, `r26_place_of_thread_table`, `r26_act_refusal_table`.

## Null and empty inputs

`refusalFor(None, _)` is `None` for every caller. A record with no id (a create's shape) is in every tree, and a
record with no project skips the project rule (fixture record `noProject`). `listedFor` of a thread whose workspace
is not held is empty (caller `tLost`), and its `spawnGuard` refuses every act with `folderGoneRefusal`.

## What the TS leaves unclear

- `ofThreadsRepository` throws for a project id not held; `refusalFor` reaches it for a record whose project is not
  held. The fixture's records all name held projects; the throw is pinned by `r7_of_threads_repository_matrix` only
  through `projectHeld`'s fake. Kept as the TS does.
