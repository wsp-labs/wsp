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
  for that project (a `WeakMap`); the port keys it per caller handle (README A9; open point 1 in the report). Tests
  `c15_same_repository_table`, `c15_origin_read_only_for_a_folder_here`.

## What survives a restart

The `pty-tokens` documents. A restarted host's boot reads `shellHeldDevices()` to keep those devices standing.
