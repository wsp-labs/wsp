# channels_copy: a client's channel into a copy, shell tokens, children's branches (`account/channels.ts:580-934`)

The channel a client of this host drives a workspace's daemon over (`copyChannel`), the served form of it for a
workspace whose computer answers its frames (`servedChannel`), the token a shell on a brokered machine holds, where a
child starts (`leadStart`, `startChildOn`) and whether two projects are one repository. TS tests:
`test/channels.test.ts`, `test/box-panes.test.ts`, `test/box-repository.test.ts`, `test/terminal-config.test.ts`,
`test/agents-brokered.test.ts` (shell tokens), `test/fork.test.ts`, `test/fork-start-events.test.ts`,
`test/sub-spawn.test.ts` (lead start), `test/spawn-guard.test.ts` (same repository).

## Rules

**The served channel** (587-667)

- C1 No place on the record, or no door → plain `placeServesDaemonLine(name, computerOf(entry))`; no link →
  `absentComputer(nameOf(place)).sentence`. A folder kind opens the box panels (box_panels) over `served` with the
  checkout as folder. Test `c1_served_channel_refusals`.
- C2 Opening sends one event to the client: `{ type: "daemon.hello", root: checkout, version? }` (version only where
  the door's report has one), before any frame. Test `c2_served_channel_says_hello_with_its_root`.
- C3 Events off the link, in order: the panels' (`heard`) pass first; a guest or tunnel event passes only where its
  `machineId` is this workspace's machine; a pty event passes only for a pty this channel named (created, listed or
  attached); a `pty.exit` also takes it off the attached set; anything else is dropped. Test `c3_events_this_channel_hears`.
- C4 Frames: an op the panels answer goes to them (`{ id: null, ...reply }`, `held` recorded when ok); a computer
  watch → `{ id: null, ok: false, code: "unsupported", error: placeWatchesItselfLine(name) }`; a computer proc op →
  same with `forkProcsUnreadLine(workspace, name)`; an op outside `carries` → `forkOpRefusedLine(op, workspace,
  name)`; else `served(frame)`, `{ id: null, ...reply }`. Test `c4_served_channel_frame_table`.
- C5 Close detaches every pty still attached (`{ op: "pty.detach", ptyId }`, errors ignored), closes the panels and
  the link. The link dropping closes the panels too. Test `c5_close_detaches_what_it_attached`.

**Shell tokens on a brokered machine** (672-785)

- C6 `terminalToken(entry, named)` (672-679): the scoping thread is `threadOn` (channels_daemon D16); none, or a launch
  with no `scoped` device → nothing. Else `{ deviceId, env }` where env is, in order, the launch's env, then
  `brokeredEnv(token, branchOfRecord, publicMarketplaces)`, then the shell's sign-ins (C7), then `THREAD_ENV:
  threadId`. Test `c6_terminal_token_env_order`.
- C7 `shellSignIns` (685-692): the harness off the thread record, else the latest row; none → `{}`. `terminalSignIn`
  nothing → `{}`. Else the terminal programs are written (C8) and the answer is the sign-in's env plus
  `TERMINAL_ENV: harness`.
- C8 `writeTerminal` (699-713): runs the line on the machine with `INLINE_EXEC_MS`; a failed exec counts as exit -1
  with its message; the pair `<machineId> <agent>` is marked written whatever the exit, and a non-zero exit warns
  `"the terminal programs for <agent> on <machine> (workspace <id>) were not written, so its terminals keep the last ones
  there: exit <code>, <first stderr line>"`. The first shell for a pair waits for the write up to `INLINE_EXEC_MS`;
  once marked, a shell does not wait (the write still runs each time). Test `c8_first_shell_waits_later_ones_do_not`.
- C9 The `pty-tokens` document per workspace (717-785): pty id → device id, read once (non-string values dropped),
  written one at a time per workspace (`put`, or `delete` when empty; a failure warns `"which shells in <id> hold a token
  was not kept for the next host: <message>"`). `shellsEnded(ws, ids)` drops each id and revokes its device after the
  write (a revoke failure warns `"the token of a shell was not taken away: <message>"`); ids with no device write and
  revoke nothing. `shellsAnswered`: an ok `pty.kill` ends its pty; an ok `pty.list` ends every held pty not listed
  standing. `shellHeldDevices()` is every device of a held workspace's document (documents of workspaces not live are
  skipped). A `workspace.deleted` forgets the workspace's written terminals and ends all its shells. Tests
  `c9_shell_tokens_kept_and_revoked`, `c9_shell_held_devices_skip_dead_workspaces`, `c9_deleted_workspace_ends_its_shells`,
  and **restart**: `c9_shell_tokens_survive_a_restart`.

**copyChannel** (791-836)

- C10 A non-folder record on a blocked computer (`blockedLine`) opens a channel that carries only `git.status` and
  `ping`; every other frame answers `{ id: null, ok: false, code: "unsupported", error: <blocked line> }`, and only the
  `daemon.hello` event reaches the client. Test `c10_a_blocked_computer_carries_status_and_ping`.
- C11 The road: unserved → the machine's own daemon; served → C1-C5 with `carries`. Shell tokens apply (`mints`) only
  to an unserved, non-local, non-folder record, whether or not the broker is ready now: a `pty.exit` event ends that
  pty's token; an ok `pty.kill`/`pty.list` is answered after `shellsAnswered`. Test `c11_only_a_machines_own_daemon_mints`.
- C12 `pty.create` (810-825): the frame's `thread` key is removed; `cwd` is the frame's or `ctx.threadFolder(entry,
  {})`; where `brokered(entry)` a token is minted (C6) and `env` = `threadEnv(entry)`, the frame's object env, then the
  token's env; the frame's keys keep their order, `cwd` and `env` after. A send that throws revokes the minted device
  and rethrows. A reply not ok, without a string `ptyId`, or landing while the workspace is deleting or gone revokes it;
  else the pty is kept with its device (C9). Tests `c12_pty_create_mints_and_keeps`, `c12_a_late_reply_revokes`,
  `c12_a_failed_send_revokes`.

**Where a child starts** (845-896)

- C13 `leadStart(lead, child, road)` in order: `copyBlocked(lead)`; `started` = record base, project base, default
  branch; `git.status` at the checkout through `withDaemon`, a failure → plain `branchUnreadRefusal(name, message)`;
  commits on the remote's default branch (`defaultBranch` given, branch equal, ahead > 0 or counts unknown) →
  `defaultBranchRefused(road, name, branch)`; `stayed` = `[uncommittedStayed(n, name)]` for `n` entries not `!!`.
  Branch empty, detached or `started` → `{ base: started, onLeads: false, lines: stayed }`. Then `tracksItself`
  (upstream ends `/<branch>`), `tracksBase` (ends `/<started>`), `holdsNew` (ahead, unknown counts, or an upstream of
  another name): nothing new → `{ base: branch, onLeads: true }` when tracking itself, else the started branch.
  Else `git.push` with `base: started` (when defined): an on-default-branch refusal → `defaultBranchRefused`; any
  other → plain `forkNeedsPushLine(name, message)`. Pushed → the checkout read again (not awaited) and `{ base:
  pushed branch, onLeads: true, lines: [pushedForChildLine(branch, child), ...stayed] }`. Test `c13_lead_start_table`
  (11 rows).
- C14 `startChildOn(child, branch, lead)` (884-896): `git.startOn` at the child's checkout; a worktree record takes the
  branch put; the record is persisted; the checkout read again; the line `childStartedLine(name, branch put)`. Any
  failure deletes the child (a failed delete warns `"<name> was not put on <branch> and was not deleted: <message>"`)
  and throws plain `forkNeedsPushLine(lead, message)`. Tests `c14_start_child_on`, `c14_a_failed_start_deletes_the_child`.

**One repository** (899-926)

- C15 `repositoryOf(remote)`: `<remoteHost>/<owner/repo lowercased>`, nothing where either is unknown.
  `sameRepository(a, b, caller)`: b's repository is known and equals a's, or a's origin's. `originNow(p, caller)`: `""`
  unless the project is a folder source on a computer whose kind copies a folder; else `git -C <path> remote get-url
  origin` (2,000 ms bound), its trimmed stdout on exit 0, `""` otherwise. The TS memoises the read per caller object
  for that project (a `WeakMap`, so a listing asks git once per request); the port reads on every call, since a
  borrowed `Caller` has no identity that safely outlives its request. Only the count of git reads differs, never an
  answer; open point 1 in the report. Tests
  `c15_same_repository_table`, `c15_origin_read_only_for_a_folder_here`.

## What survives a restart

The `pty-tokens` documents. A restarted host's boot reads `shellHeldDevices()` to keep those devices standing.

## Tests

`tests/channels_copy.rs`, with `oracle/channels_copy.oracle.ts` and its fixture `tests/fixtures/oracle/channels_copy.json`.
The oracle runs the TS `channelsArea` on a fake context over scripted worlds (99 scenarios: a world, a script of
steps, and the trace the TS made of it). The Rust fake `Sim` builds the same context from the same world, `play` runs
the same script, and a test compares the trace as JSON strings (key order counts): each step's answer, and every
effect in order (`frame`, `client`, `put`, `store-delete`, `revoke`, `warn`, `exec`, `launch`, `thread-folder`,
`persist`, `read-checkout`, `delete-workspace`, `origin-read`). The TS pieces that live outside this file
(`brokeredEnv`, `terminalSignIn`, `terminalLine`, `publicMarketplaces`, `spawnSync`) are replaced in the oracle by
fixed functions that the Rust fake's `brokered_env`, `terminal_sign_in`, `terminal_line` and `git_origin` repeat.
Consecutive `link-closed`, `own-closed` and `released` effects are compared in name order (the TS's microtask order
between them is not a rule). The scenarios named `c1`, `c2`, `c3 listed...`, `c3 the panels...`, `c4 frame table` and
`c5` (all but the one that sends a pty.create) also run through `served_channel` alone and must leave the same trace.
`constants_match` is checked on every run: `INLINE_EXEC_MS`, `THREAD_ENV`, `WORKSPACE_FRAMES`.

| Rule | Tests |
|---|---|
| C1 | `c1_served_channel_refusals` |
| C2 | `c2_served_channel_says_hello_with_its_root` |
| C3 | `c3_events_this_channel_hears` (also the `String(...)` naming of a pty id, a TS quirk: a reply with no ptyId names `"undefined"`) |
| C4 | `c4_served_channel_frame_table`, `a_served_reply_is_answered_under_a_null_id_and_keeps_its_own` |
| C5 | `c5_close_detaches_what_it_attached` |
| C6 | `c6_terminal_token_env_order`, `c6_terminal_token_and_sign_ins_called_directly` |
| C7 | `c6_terminal_token_env_order` (the harness off the record, else the latest row, none, no sign-in), `c6_terminal_token_and_sign_ins_called_directly` |
| C8 | `c8_first_shell_waits_later_ones_do_not`, `a_refused_write_warns_why_and_the_shell_opens_all_the_same` |
| C9 | `c9_shell_tokens_kept_and_revoked`, `c9_shell_held_devices_skip_dead_workspaces`, `c9_deleted_workspace_ends_its_shells`, `c9_shell_tokens_survive_a_restart` |
| C10 | `c10_a_blocked_computer_carries_status_and_ping` |
| C11 | `c11_only_a_machines_own_daemon_mints` |
| C12 | `c12_pty_create_mints_and_keeps`, `c12_a_late_reply_revokes`, `c12_a_failed_send_revokes` |
| C13 | `c13_lead_start_table` (24 rows: the 11 the spec names and the rows around them), `c13_a_blocked_lead_reads_nothing_and_a_folder_lead_is_never_blocked`, `c13_a_lead_on_its_own_daemon_reads_over_a_channel_it_closes` |
| C14 | `c14_start_child_on`, `c14_a_failed_start_deletes_the_child` |
| C15 | `c15_repository_of_names_a_remote_by_host_and_lowercased_owner_and_repo`, `c15_same_repository_table`, `c15_origin_read_only_for_a_folder_here`, `c15_origin_now_answers_the_trimmed_origin_or_nothing` |

### Ported TS cases

| TS case | Test |
|---|---|
| `box-panes.test.ts:31` | `a_folder_pane_hears_only_its_own_ptys_and_detaches_only_what_stands` |
| `box-panes.test.ts:94` | `a_pane_does_not_hear_the_readings_the_computers_own_page_watches` |
| `box-panes.test.ts:107` | `a_command_on_a_panes_channel_is_refused_before_the_link` |
| `box-panes.test.ts:115` (weak) | `a_served_reply_is_answered_under_a_null_id_and_keeps_its_own` |
| `channels.test.ts:226` | `a_shell_on_a_folder_of_a_joined_computer_carries_no_token_while_the_broker_is_ready` |
| `channels.test.ts:263` | `a_shells_tabs_open_in_the_threads_folder_each_with_a_token_of_its_own` |
| `channels.test.ts:280` | `a_shell_carries_the_git_and_gh_variables_a_turns_launch_does_on_its_own_token` |
| `channels.test.ts:293` | `a_shell_carries_the_sign_in_a_claude_turn_is_handed_on_its_own_token` |
| `channels.test.ts:319` | `a_codex_threads_shell_is_marked_for_the_profile_that_finds_codexs_program_first` |
| `channels.test.ts:337` | `a_machines_first_codex_shell_opens_once_the_program_is_written_and_a_later_one_does_not_wait` |
| `channels.test.ts:356` | `a_machines_first_shell_opens_after_an_inline_execs_span_where_the_write_never_answers` |
| `channels.test.ts:375` | `a_refused_write_warns_why_and_the_shell_opens_all_the_same` |
| `channels.test.ts:430` | `a_shell_signs_in_only_its_threads_own_agent` |
| `channels.test.ts:449` | `each_threads_agent_is_signed_in_in_its_own_terminal_where_both_share_a_machine` |
| `channels.test.ts:465` | `a_shells_token_dies_at_its_exit_its_kill_and_a_list_that_no_longer_holds_it` |
| `channels.test.ts:484` | `a_shells_token_outlives_a_host_restart_while_its_shell_stands` |
| `channels.test.ts:506` | `a_shells_token_is_held_off_the_documents_with_no_turn_running` |
| `channels.test.ts:535` | `a_shell_is_not_minted_a_token_while_the_broker_is_not_ready` |
| `channels.test.ts:543` | `a_shells_token_is_taken_away_when_its_open_answers_after_its_workspace_was_deleted` |
| `channels.test.ts:559` (weak) | `a_thread_of_another_workspace_scopes_the_token_to_this_workspaces_newest_thread` |
| `channels.test.ts:575` | `a_shells_token_goes_with_its_workspace` |
| `spawn-guard.test.ts:186` | `a_child_starts_on_the_branch_its_lead_is_on` |
| `spawn-guard.test.ts:274` | `a_lead_with_commits_on_the_default_branch_is_refused_before_anything_is_pushed` |
| `spawn-guard.test.ts:288` | `the_default_branch_is_the_one_the_leads_remote_starts_on` |
| `spawn-guard.test.ts:307` | `a_lead_whose_own_base_is_the_default_branch_and_holds_commits_on_it_is_refused` |
| `spawn-guard.test.ts:319` | `the_default_branch_refusal_speaks_in_the_words_of_the_road_that_met_it` |
| `spawn-guard.test.ts:334` | `a_lead_ahead_of_its_upstream_pushes_its_branch_first` |
| `spawn-guard.test.ts:415` | `one_repository_is_one_whatever_its_remotes_spelling` |
| `box-repository.test.ts:31` | `a_folders_origin_is_read_now_and_matches_either_spelling` |
| `fork-start-events.test.ts:146` | `a_leads_default_branch_refusal_is_read_before_any_stage` |
| `terminal-folder.test.ts:32` | `a_tab_that_names_no_folder_opens_in_the_threads_folder_and_one_that_names_a_folder_keeps_it` |
| `terminal-folder.test.ts:45` | `a_cloud_forks_first_tab_opens_in_the_folder_the_thread_works_in` |
| `serve-daemon.test.ts:294` | `a_thread_terminal_with_no_folder_starts_where_the_thread_works` |
| `tree.test.ts:208` | `a_childs_base_is_the_leads_pushed_branch_and_the_fork_says_what_stayed` |
| `tree.test.ts:225` | `a_lead_on_the_branch_its_work_started_from_pushes_nothing` |
| `tree.test.ts:234` | `a_lead_whose_branch_the_remote_holds_whole_pushes_nothing_and_children_start_on_it` |
| `tree.test.ts:242` | `a_lead_on_an_upstream_of_another_name_is_pushed` |
| `tree.test.ts:250` | `a_lead_cut_from_the_bases_remote_branch_with_no_commits_pushes_nothing` |
| `tree.test.ts:262` | `a_lead_with_no_commits_over_the_base_pushes_nothing` |
| `tree.test.ts:270` | `a_lead_with_no_credential_refuses_the_fork_in_the_pushs_own_sentence` |
| `tree.test.ts:279` | `a_lead_that_did_not_say_its_branch_refuses_the_fork` |
| `tree.test.ts:286` | `a_child_that_would_not_go_onto_the_leads_branch_is_deleted_and_the_fork_refused` |
| `tree.test.ts:301` | `a_childs_project_may_be_another_computers_project_of_the_same_repository_and_never_another` |
| `bring-back.test.ts:115` | `a_branch_only_the_parent_holds_is_pushed_first` |
| `bring-back.test.ts:133` | `a_parent_that_did_not_say_which_branch_it_is_on_stops_the_fork` |
| `bring-back.test.ts:173` | `a_child_is_put_on_the_branch_its_lead_was_on_at_the_fork` |
| `guest-spawn.test.ts:363` | `a_child_that_leads_reads_its_own_branch_as_the_new_base` |
| `guest-spawn.test.ts:520` | `a_leads_folder_whose_origin_moved_matches_the_projects_recorded_under_the_old_name_and_the_new` |
| `guest-spawn.test.ts:534` | `a_leads_folder_whose_origin_points_at_a_fork_matches_no_other_projects_name` |
| `guest-spawn.test.ts:546` | `c15_same_repository_table` (the origin is read on demand on every call, nothing cached) |

Not ported: `spawn-guard.test.ts:225` (which parent's branch a create reads is the create's choice of lead, outside
this unit; `lead_start` reads whichever entry it is given). Weak cases left out: `box-panels.test.ts:225` and `:638`
(the panels' own tests), `channels.test.ts:392` and `:441` (the text of `terminalLine`, machines/brokered),
`channels.test.ts:410` (`terminalSignIn`'s key placeholder, machines/brokered), `spawn-guard.test.ts:388` (what a
listing reads), `bring-back.test.ts:143`.
