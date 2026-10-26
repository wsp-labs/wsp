# Unit `workspaces_git`: `verbs/workspaces.ts` lines 1-320 (rename, fork, commit, discard, fix, merge in, merge)

Rust: `src/verbs/workspaces.rs` (the first seven entries of `workspace_verbs()`; the rest in
`spec/workspaces-life.md`). Lines cite `packages/host/src/verbs/workspaces.ts` at `65f6b89c7`. Tests:
`tests/cli_b/workspaces_git.rs` (generated, 152) and `tests/cli_b/workspaces_rules.rs` (hand). Expected bytes come
from the TS command line over `daemon/crates/wsp-mcp/tests/answers/{rename,fork,commit,discard,fix,merge_in,merge}.json`
and 8 hand cases.

`workspace_verbs()` order: rename, fork, commit, discard, fix, merge in, merge, start, review, review post, update,
pause, resize, wake, rebuild, image, image build, forget, delete (:51-711). `rename`, `fork`, `pause`, `wake`,
`rebuild` and `forget` are `cloud: true`: with the cloud off they are not in the table at all, as line or tool, and
`delete` loses its `workspace` input (`cloudFlags: ["workspace"]`).

## Shared steps (workspaces-help, turns-help)

- `threadAt(client, ref, "wsp <verb>")`: `sessions.list` then `workspaces.get {workspaceId, threadId}`. A ref naming a
  project refuses `<ref> is a project; wsp <verb> takes a thread, as wsp threads <ref> lists them. Name a thread by
  its id.` (after `projects.list`); a machine's name refuses likewise (after `workspaces.list`); none: `no thread <ref>`.
  A record in a shape this build cannot read: `otherVersion("workspaces.get")`, exit 1. A gone machine refuses before
  any git op, exit 1.
- `awake(client, workspace, verb, say)`: `workspaces.wake` (always sent for the git verbs, even when running), the
  line `waking <name>` on stderr when the machine was not running.
- `sharingWith(client, at)`: a second `sessions.list` after the act, naming the other threads in the folder;
  `withShared(line, shared)` adds `thread <t> works in this folder too` or `threads <a> and <b> work ...`.
- `threadIdOf(thread)` is `threadId ?? id`; `threadLabel(thread)` is how a line names it.

## `wsp rename <workspace> "<name>"` (:59-65)

Exactly two positionals: `wsp rename takes a workspace and one name.` + usage. `renameWorkspace` (`workspaces.resolve`,
`workspaces.rename`); emit the answer `{was, workspace}`, line `renamedWorkspaceLine`.

## `wsp fork <workspace> [--name] [--size] [--send "<task>" [--agent] [--model] [--effort] [--access] [--cwd] [--notify]...] [--max-machines] [--max-depth]` (:84-107)

1. Exactly one positional: `wsp fork takes one workspace.` + usage.
2. Without `--send`, each of `agent, model, effort, access, cwd, notify` that is given refuses, in that order:
   `--<flag> says how a thread opens, and this line opens none.`, fix `Add --send "<task>", or drop --<flag>.` (a relative
   `--cwd` is refused earlier by the parse, rule 10.3).
3. Reads (`reads`): `workspaceOf(ref)` (`workspaces.resolve`); a gone source refuses `goneRefusal(name, "fork", gone)`;
   `notifyOf(--notify)` (`sessions.list` when any); with a task, `checkedStart(task, agent, picks, source.id)`
   (`harnesses.list`; an empty task refuses `the message is empty; say what the thread is to do. Put it in quotes after
   the flags.`). With a task the reads run inside `beforeSending`.
4. `agentsAsked(--max-machines, --max-depth)`; `createFor(client, out, project, name ?? "<source>-fork", {parent,
   size?, agents?})`: `projects.resolve`, `workspaces.landing`, `events.subscribe`, `workspaces.create`; its frames
   stream as it goes (the `workspace` and `notice` frames under `--json`).
5. No task: exit 0. With a task, `followVerb(..)` opens the first thread (`sessions.start`, a fresh `requestId`) and
   follows it to its end, streaming the reply; emit `{turn}`. A failed first turn exits 1 with the turn's failure.

## `wsp commit <thread> [-m|--message "<m>"] [--file <path>]...` (:145-154)

Exactly one positional (`wsp commit takes one thread.`). `threadAt`, `awake`, `committed(client, id, message, files,
say, threadId)`: without a message `workspaces.commitDraft` first (no draft: `no message was drafted: <note or "the
agent gave none">. Pass one with --message "<message>".`, usage, and the drafted message is printed on stderr before
the commit); `workspaces.commit`; then `sharingWith`. Emit `{...made, ...sharedOf(shared)}`; line
`withShared(committedLine(label, made), shared)`. `-m` is `--message`'s short form.

## `wsp discard <thread> <path> [--yes]` (:181-193)

Exactly two positionals (`wsp discard takes one thread and one file.`). `threadAt`, `awake`, then `workspaces.discard
{workspaceId, path, check: true, threadId}` (the host's refusal first), the question `Discard the change to <path> in
<folder>?`, then `workspaces.discard {workspaceId, path, threadId}` parsed as `GitDiscardReply`, `sharingWith`. Emit
`{...put, ...sharedOf}`; line `withShared(discardedLine(label, put.path), shared)`.

## `wsp fix <thread> [--check "<name>" | --child <thread>]` (:216-228)

Exactly one positional (`wsp fix takes one thread.`); both flags: `FIX_CHECK_OR_CHILD` + usage, before dialling.
`threadAt`; with `--child`, `childOf(client, at, child, "wsp fix")` (a second `sessions.list` and `workspaces.get`; a
child in its lead's folder refused). Only without `--check` is the machine woken. `askedToFix(client, id, check,
child?, threadId)` → `workspaces.fix`; emit `{...asked}`, line `fixLine(label, asked, childLabel?)` (a turn held by its
computer's cap says so on a second line).

## `wsp merge in <lead> <child>` (:260-267)

Exactly two positionals (`wsp merge in takes a lead thread and one of its children.`). `threadAt(lead)`, `childOf`,
`mergedIn(..)` (wakes the lead, `workspaces.mergeIn {workspaceId, threadId, child: {workspaceId, threadId}}`); emit
`{...done}`, line `mergeInLine({...done, lead, child}, childThreadId)`.

## `wsp merge <thread> [--method merge|squash|rebase] [--when-checks-pass]` (:293-302)

Exactly one positional (`wsp merge takes one thread.`). `--method` through the `MergeMethod` enum before dialling:
`--method takes merge, squash or rebase, and got <w>.` + usage. `threadAt`, `mergedPr(client, id, method?,
whenChecksPass, threadId)` → `workspaces.merge`; emit `{...merged}`, line `mergedLine`.

## Tool door

Each tool mirrors its line: `fork`'s task is `task` and its first-turn failure answers `created <name> <id>; first turn
failed: <why>` as an error result that still carries the workspace (:125-135); `commit` takes `files`; `fix` refuses
`check` with `child` as a plain error. Recorded tool cases pin each.

## Tests (rule: test)

| Rule | Test |
|---|---|
| count refusals, fork's picks without `--send`, `--method`, check with child | `workspaces_git.rs::line_hand_*` |
| frames of every recorded case: wake always sent, check before question, drafted message, shared folders | `workspaces_git.rs::line_*` |
| fork streams its create frames and follows its first turn | `workspaces_git.rs::line_fork_*` |
| tool door, pages | `workspaces_git.rs::tool_*`, `page_*` |
| the cloud verbs leave the table with the cloud off | `shared_rules.rs::the_cloud_verbs_are_marked_and_delete_carries_its_cloud_input` |

## What the TS leaves unclear

- `fork --send` builds `openingOf(env, workspace, task, {..., elsewhere})`; a fork from a machine line (`elsewhere`)
  is not recorded.

## After-port tickets

- `wsp commit` without a message prints the drafted message on stderr and commits with it, with no question; a
  person cannot read it before it lands.
