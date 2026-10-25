# runtime_core: the assembly and the core every area shares

runtime.ts:425-992 at `65f6b89c7`. Rust: `src/runtime.rs` (`create_runtime`, the module list) and
`src/runtime/core.rs` (`Core`, `CoreCtx`, the place recording). Tests: `tests/runtime_core.rs`.

The TS builds one object: `runtimeCore` fills the shared state, each area adds its members in a fixed order, and
`runtimeOf` reads the public `Runtime` off it. In Rust the state is wsp-core's (`RuntimeState`, `LiveWorkspace`,
the maps); what `runtimeCore` itself decides is ported here behind `CoreCtx`, the calls the core makes on the areas
(`ready`, `agents_reach`, `brokered`, `wake`, `exec`, `delete`, the projects door, `end_sessions`, `drop`, ...).

## Assembly (runtime.ts:425-489)

- C1. `create_runtime(opts)` builds the core, then the areas in this order, each reading only what was built
  before it: activity, kinds, rules, transcripts, slates, channels, proc scopes, records, reach, pull requests,
  daemon, machines, boot, create, sub-spawn, sub-queue, sub-life, folders, start-from, workspaces, agents, threads,
  turns, waits, sessions, builders, golden, image, projects, usage, conversations, status, preferences, project
  icons; then the pipe area with the brokered rule (a record at a provider reads `{kind: "provider"}`), then
  `runtime_of`. A member read before its area is built panics `the runtime read <name> before it was built`
  (wsp-core `Late`). Tested by the whole-host tapes once every area is in, not here.

## Options and their defaults (runtime.ts:491-534, 977-978)

- C2. `provider_read_ms` `PROVIDER_READ_MS`, `gone_confirm_ms` `GONE_CONFIRM_MS`, `late_read_ms` (`wake`)
  `WAKE_LATE_READ_MS`, `clock` the real clock, the GitHub cache a memory one, `daemon_hello_timeout_ms`
  `DAEMON_HELLO_TIMEOUT_MS`, `vault_cap_bytes` (`wake`) `VAULT_CAP_BYTES`, `default_idle_window_ms` (`idle`)
  `NAP_AFTER_MS`, `host_id` this computer's hostname, `places` the wired place `default` over the backend.
- C3. `reads_state(machine)`: the machine's state bounded by `provider_read_ms` on the runtime's clock; a read that
  failed or ran out is `None`. `took_the_resume(state)`: `running` or `starting` only.
- C4. `sleeps(ms)` resolves after `ms` on the runtime's clock and holds no process open.
- C5. `vault_paths_of(machine)`: `opts.vault_paths` when given. Otherwise `date -u +%FT%TZ >> /root/.wsp-upgraded`,
  then `ls -A /root`; a non-zero exit fails `vault enumeration failed: <the last 200 characters of stderr>`; the
  names trimmed, empties and `VAULT_SKIP` dropped, each as `/root/<name>`, in the listing's order.
  `vault_export` hands those paths to the engine's export with `exclude` the vault caches where given.

## A thread's launch (runtime.ts:535-565)

- C6. No road to this host for the workspace (`agents_reach` none) mints nothing: `{env: {}}`.
- C7. A brokered record first re-reads its repo's visibility beside the launch (its failure warns
  `the visibility of <name>'s repo was not read again: <message>`), then awaits the broker read, the daemon's
  settling and the public marketplaces, together.
- C8. The token is a device minted named `thread <thread word>` with scope `{kind: "thread", threadId,
  workspaceId, rootThreadId}` at the wall clock, its road the kind's `turn_road`, `aside` where asked.
- C9. The env holds `HOST_TOKEN_ENV`, and only where the road has a url, `HOST_URL_ENV` and, only where a place
  door is wired, `HOST_KEY_ENV` the host key. `wsp` rides where the road names a server.

## Timers (runtime.ts:860-900)

- C10. `wake_at(thread, until)` cancels the thread's armed snooze wake, and arms nothing for a moment already past.
  The wake fires after `min(wait, MAX_TIMER_MS)`; finding the record's `snoozedUntil` moved does nothing; early
  (a long wait cut to the timer's cap) re-arms; at or past `until` emits
  `{"type":"thread.marked","workspaceId","threadIds":[thread]}`.
- C11. `resume_on_reset(thread, at)` cancels the armed one; fires after `min(max(at - now, 0), MAX_TIMER_MS)`, so a
  reset already past fires at once; `limitResume.at` moved does nothing; early re-arms; otherwise asks
  `resume_after_limit(thread)`, whose failure warns `thread <word> did not resume at its reset: <message>`.

## What stands on a computer (runtime.ts:905-939)

- C12. `standing_on(place)` waits on `ready`. Forks are the live records there that do not run in a folder;
  folders are the ones that do; projects are the held projects on that computer. A workspace's threads are the
  distinct thread ids of its thread records and of its sessions' views. A fork's row is `{name: fork_name, threads}`;
  a project's counts the threads of every workspace of it that is not a fork there.
- C13. `fork_name(entry)`: the title of its one thread when it folds to exactly one, else its record's name.
- C14. `forget_project(id)`: off the held map, off the store, `{"type":"project.removed","projectId"}` emitted,
  its icon forgotten, in that order.

## The place door's recording (runtime.ts:592-765)

- C15. Built only when `opts.place_links` is given; a door's every event rides the bus; `place.removed` drops that
  place's copy row.
- C16. `holds_on(place, read = true)`: the rows of `standing_on`; with `read`, every fork's unsaved line (woken
  first where it is not running, read through the workspace's exec) and every project's, in that order, as
  `unsaved`; with `read` false, `unsaved` is empty.
- C17. `drop_on(place)`: forks by their names, then folders by their records' names, then projects (removed with
  `force`), each by its own road; the first that refuses stops the rest and fails
  `place_drop_stopped_line(<its message>, <names gone so far>)`, keeping that error's fields. Answers the rows.
- C18. `forget_on(place)`: every fork and folder's sessions ended with `DELETED_REASON` and dropped, then every
  project forgotten (C14). Answers the rows.
- C19. `running_on(place, rows)` folds the sessions whose turn the cap does not hold. `machines_on(place)`: the
  machine ids of the live records there with one. `forget_records_on(place)`: every record there that is no sub
  ended and dropped, then the subs there released as forgotten. `release_records_on(place)` releases the subs.
  `sub_holds_on(place)`: a line per sub there holding unpushed work and having a thread.
- C20. `remove_folder(place, project)`: a project not on that computer does nothing; one with standing records
  fails `project_in_use_refusal(name, their names)`; else it is forgotten (C14).
- C21. `folder_look(project, was, now)`: the icon and the hue that moved from `was` to `now` win over the person's;
  a look with neither is written `null`; an image that moved is worn (or cleared to `null` when gone).
- C22. `add_folder(place, key, folder, move, stage)`: `~` at the front is this computer's home; a folder that does
  not exist fails `recipe_folder_gone_line(source)`; a seed plan with no remote fails `no_remote_line(source)`.
  A move whose `from` and `name` match keeps the project, seeds only the newly kept paths, and answers `installed`
  with the old project's name, id and age. A move that changed fails `folder_move_stands_line` while the old folder
  stands; the old project is removed keeping its look, its refusal's first line becoming `folder_move_held_line`.
  The add keeps the move's id and the oldest age; a fresh add writes the folder's icon and hue and wears its image.
  The answer is `{id: "folders/<key>", label, outcome: "installed", project: {id}, pick, createdAt, note?}`.

## Bus wiring (runtime.ts:766-846)

- C23. A `place.setup` with an `end` asks the image to keep that place's copy current.
- C24. `place.absent` re-emits every stage of a build on that place with the dial-back line as its detail;
  `place.present` re-emits it with the frame's own detail. `place` is named only where the stage named it.
- C25. `place.present`, after `ready`: every stored workspace on that place held by a stand-in is hydrated again
  (a fork live through the gap is left as it is); a hydrated one syncs its daemon, one that is not is re-read; then
  every held row there is polled. A failure warns `the records on <name> were not read again: <message>`.
- C26. A `golden.stage` naming a place writes that place's copy row (`copy_build_of`), or drops it when the frame
  says the copy stands.
- C27. `row_says_failure(place, e)` is false for a fork-nowhere or a provisioning refusal, an absent place, or a
  place away (no backend and no link).

## Rules against tests (`tests/runtime_core.rs`)

Generated from the rule each test names in its first comment; 35 tests.

| Rule | Tests | TS case |
|---|---|---|
| C2 | `the_defaults_are_the_wiring_constants` | runtime.ts:498-515 |
| C3 | `took_the_resume_only_running_or_starting`, `reads_state_answers_the_providers_state`, `reads_state_is_none_past_the_providers_read` | runtime.ts:504-505; runtime.ts:505 (until, bounded on the runtime's clock); runtime.ts:509 |
| C5 | `vault_paths_drop_the_skipped_and_the_empty`, `vault_paths_given_read_nothing_off_the_machine`, `vault_enumeration_failing_names_stderrs_tail` | runtime.ts:517-528; runtime.ts:518; runtime.ts:522 |
| C6 | `a_launch_with_no_road_mints_nothing` | runtime.ts:544-545 |
| C7 | `a_brokered_launch_reads_the_broker_first` | runtime.ts:549-553 |
| C8 | `the_mint_is_scoped_to_the_thread_and_its_road` | runtime.ts:554, spawn-guard.test.ts |
| C9 | `a_launch_names_the_url_and_key_only_where_they_stand` | runtime.ts:556-563 |
| C10 | `a_snooze_wakes_every_window_at_its_time`, `a_moved_snooze_says_nothing_and_a_new_one_replaces_the_old`, `a_long_snooze_rearms_past_the_timer_cap`, `a_past_snooze_arms_nothing` | runtime.ts:865-873; runtime.ts:867-868; runtime.ts:874, 877 (MAX_TIMER_MS); thread-marks.test.ts:186 |
| C11 | `resume_at_reset_fires_at_its_time_and_at_once_when_past`, `a_moved_resume_does_nothing`, `a_failed_resume_warns` | runtime.ts:892; runtime.ts:894; usage-limit-resume.test.ts; runtime.ts:886-899 |
| C12 | `standing_on_splits_forks_folders_and_projects` | places-remove.test.ts; runtime.ts:908-927 |
| C13 | `fork_name_is_its_one_threads_title_else_its_name` | runtime.ts:936-939 |
| C14 | `forget_project_in_order` | runtime.ts:929-934 |
| C15 | `place_removed_drops_the_copy_row` | runtime.ts:769-771 |
| C16 | `holds_on_reads_unsaved_forks_then_projects` | runtime.ts:629-648 |
| C17 | `drop_on_stops_at_the_first_refusal_naming_those_gone`, `drop_on_takes_forks_then_folders_then_projects_with_force` | runtime.ts:653-670; runtime.ts:665-669 |
| C18 | `forget_on_ends_and_drops_then_forgets` | runtime.ts:671-679 |
| C19 | `machines_on_names_the_machines_there` | runtime.ts:686-689 |
| C20 | `remove_folder_refuses_while_records_stand` | runtime.ts:756-762 |
| C21 | `folder_look_keeps_what_the_person_set`, `folder_look_with_neither_writes_null_and_a_moved_image_is_worn_or_cleared` | runtime.ts:583-589; runtime.ts:587-588 |
| C23 | `place_setup_with_an_end_keeps_the_copy_current` | runtime.ts:774-777 |
| C24 | `place_absent_and_present_re_emit_the_stage` | runtime.ts:781-791, places-image.test.ts |
| C25 | `place_present_rehydrates_stand_ins_only` | runtime.ts:796-812, places.test.ts |
| C26 | `golden_stage_writes_and_drops_the_copy_row` | runtime.ts:841-846, places-image.test.ts |
| C27 | `row_says_failure_is_false_for_the_place_reasons` | runtime.ts:850-853 |

Not pinned here: C1 (the assembly order) and C22 (`add_folder`, the recipe's folder road) need every area built, and are left to the whole-host tapes and to places-setup.test.ts's cases once wsp-machines is joined; C4 (`sleeps`) is one line over the clock.
