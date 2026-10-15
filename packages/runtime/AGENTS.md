# Runtime

The host's core: workspaces, threads, turns, transcripts and joined computers. Paths are under packages/runtime/ unless rooted at the repo.

## How it works

A turn is a process on the workspace's computer, not the host's: it logs there, and a new host re-opens it.
`src/runtime.ts` is the hub, its parts in folders under `src/`. No code compares the kind string: ask `moduleOf()` or `isLocalWorkspace()`.
An agent writes everything in its copy, the gh on its PATH included, and the daemon there may run as root.

## Invariants

1. A restarted host re-opens each running turn by its handle and reads its log from byte 0, keeping a reply that landed meanwhile (`test/local-exec.test.ts`).
2. A re-opened turn writes no row twice: `turnWritten()` counts what it wrote and the replay skips that much (`test/subagents.test.ts`).
3. A request's origin is stamped off its token, never the client's word (`test/spawn-guard.test.ts`).
4. Nothing posts as the person through a copy's gh (`postAsPerson()`), and a merge sends the head the person was shown (`test/pull-request.test.ts`).
5. A transcript that does not parse is set aside and every later turn is still written (`test/transcript-index.test.ts`).
6. A computer's record changes only in its turn, through `change()`, so no writer drops another's fields (`test/places-setup.test.ts`).

## Traps

- `withDaemon()` never reads a computer's block: call `copyBlocked()` before any act inside a copy; `aside` and `rewind` still skip it (#1383).
- `readPullRequest()` without force answers a fact minutes old: a verb acting on a check passes force; merge keeps the held one (#1466, #1718).
- `mergeIn()` refuses while the lead runs a turn, except the asking thread's own, whose tool call runs in it (#1476).
- Every road making a worktree record goes through `worktreeRecordAt()`, which keeps `madeFor`, or the thread's own tree refuses it (#1607).
- `ls-remote --exit-code` exits 2 for a branch never pushed or deleted: ask `branchGoneAtRemote()` (#1607).
- A daemon op outside the home needs its folder in the roots file: `writeDaemonRoots()` writes it whole from every live checkout, the import via `kind.roots` with its two paths alone (#1607, #1645, #1774).
- A folder record no thread names is bare: count with `bareFolder()` (`packages/protocol/src/projects.ts`); `removeFolder` counts every record (#1634).
- A prompt held back at launch goes in the seed file on disk, not memory alone (#1620, #1665).
- Rows carry the agent's session id (`claudeSessionId`), `sessions` the launch's, which a Codex thread's first process lacks: look up both, as `answer()` does (#1776).
- `end()` hands `settleCut()` a fresh literal: change the row through its entry in `sessions` (#1646).
- `sharedFolder()` compares checkout tops, not cwds: threads in two subfolders of one repo share it (#1664).
- Trim a transcript with `dropOldest()`; set aside a blob that does not parse or the table refuses (#1520, #1666, #1722).
- An undo is wsp's only when its owner row reads installed, never when any id of its group does (#1481).
- A vault value reaches an owned computer on stdin with `withEnvFromInput()` (`packages/engine/src/run-env.ts`); a clone there runs no setup-git (#1482).
- `buildDatabase()` makes `state.db` WAL before the link: two processes switching it at once fail one, busy wait or not (#1785).
- A daemon or CLI one version behind still answers: gate a new op or flag on its version; read a missing op as refused (#1484, #1613).
- A box is listed before its create returns: claim it via `claiming()` or `ctx.inflight` from its first state to its stop (#2255).

## One home for

| Rule | File | Function |
|---|---|---|
| what a caller may see and drive | `src/account/rules.ts` | `refusalFor()` |
| the folder a turn or a command starts in | `src/machines/kinds.ts` | `threadFolder()` |
| a git host read for a workspace | `src/account/pull-requests.ts` | `readHost()` |
| a git host named from a remote | `packages/catalog/src/git-hosts.ts` | `gitHostOf()` |
| a computer busy with a setup | `src/places/setup.ts` | `settingNow()` |
