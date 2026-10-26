# Unit `workspaces_life`: `verbs/workspaces.ts` lines 321-711 (start, review, review post, update, pause, resize, wake, rebuild, image, image build, forget, delete)

Rust: `src/verbs/workspaces.rs` (the last twelve entries). Lines cite `packages/host/src/verbs/workspaces.ts` at
`65f6b89c7`; the shared steps are in `spec/workspaces-git.md`. Tests: `tests/cli_b/workspaces_life.rs` (generated,
188) and `tests/cli_b/workspaces_rules.rs` (hand). Expected bytes come from the TS command line over
`daemon/crates/wsp-mcp/tests/answers/{start,review,review_post,update,pause,resize,wake,rebuild,image,image_build,
forget,delete}.json` and 21 hand cases, two at a scripted terminal.

## GitHub starts

- `wsp start <link> [--project] [--agent] [--model] [--effort] [--access]` (:327-333): exactly one positional (`wsp
  start takes one link.`). `startedFrom(client, {url, project, agent, model, effort, access})` → `workspaces.start`
  with the given fields; emit `{...started}`, line `startedLines(started)`.
- `wsp review <link|thread> [--agent] [--model] [--effort]` (:357-364): exactly one (`wsp review takes one pull
  request link or thread.`). A target matching `^https?://` is `{url}`; anything else is a thread (`threadAt`) and
  sends `{workspaceId}`. `reviewStarted` → `workspaces.review`; line `reviewLines`.
- `wsp review post <thread> [--verdict comment|approve|request-changes] [--summary "<text>"]` (:390-401): exactly
  one (`wsp review post takes one thread.`); the verdict through `VERDICTS` (`comment`, `approve`,
  `request-changes` and `request_changes`) before dialling: `--verdict takes comment, approve or request-changes, and
  got <w>.` + usage. `threadAt`, `reviewPosted(client, id, {verdict?, summary?}, threadId)` (`workspaces.reviewDraft`
  first when either is given, then `workspaces.reviewPost`); line `START_WORDS.posted(label, number, comments, folded)`.

## `wsp update <thread>` (:425-435)

Exactly one. `threadAt`, `awake`, `workspaces.update {workspaceId, threadId}` parsed as `GitUpdateReply`,
`sharingWith`; emit `{...done, ...sharedOf}`, line `withShared(updateLine(label, done), shared)`.

## Machine verbs

- `wsp pause <workspace>` (cloud, :458-465): exactly one. `nap` → `workspaces.resolve`, `workspaces.nap`; emit
  `{workspace}`, line `stateLine(workspace)`.
- `wsp resize <thread> --size <cpu>x<memGb>` (:479-488): exactly one positional and `--size` (`wsp resize takes one
  thread and --size.`); `sizeFromWord(word)` before dialling, refused `resizeWordLine(word)` with `RESIZE_WORD_FIX`;
  `resize(client, ref, size)` (`sessions.list`, `workspaces.resize`); emit `{workspace}`, line
  `resizedLine(name, size)`. Page `app`.
- `wsp wake <workspace>` (cloud, :508-515): exactly one. `workspaceOf`, `awake` (stderr `waking <name>`), then
  `wokeLine` reads `status.list`; emit `{workspace}`.
- `wsp rebuild <workspace>` (cloud, :533-539): exactly one. `rebuild` (`workspaces.resolve`, `status.list`,
  `workspaces.rebuild`, refused on a machine that still answers); emit `{workspace}`, line `rebuiltLine`. Page `app`.

## Image

- `wsp image` (:557-562): no positional (`wsp image takes no positional arguments.`). `imageView` → `image.get`; emit
  the view, line `imageLines`.
- `wsp image build <place> [--force]` (:581-586): exactly one (`wsp image build takes one place.`).
  `buildImageAt(client, out, place, force)`: `places.list`, `events.subscribe`, `image.build`, `image.get`, its progress
  streamed; emit `built` (the copy), line `sealedBuiltLine(image, built)`.

## `wsp forget <workspace> [--yes]` (cloud, :605-614)

Exactly one. `dropping(client, ref)` (`workspaces.resolve`, refusing a project's folder or a worktree here, then
`sessions.list`), the question `forgetQuestion(f)`, then `forget` (`workspaces.forget`); emit `{workspaceId, name,
threads}`, line `forgotLine(f)`.

## `wsp delete <thread>[|<workspace>] [--processes stop|keep] [--yes]` (:636-669)

1. Exactly one positional: `wsp delete takes one thread.`, with the cloud on `wsp delete takes one thread, or one
   workspace.`, + usage. `--processes` through `processes_asked` before dialling.
2. `threadOf(client, ref)` (`sessions.list`), a failure taken as no thread.
3. A Boat sub-thread (`subDeleting`): `sessions.delete {threadId, check..}` read for the question, the question, then
   `threadDeleted(client, id)`; emit `{threadId, ...gone without on}`, line `subDeletedLine(id, on ?? "")`.
4. A thread here (`threadHere`, `workspaces.list`): `sessions.delete {threadId, processes: choice ?? "ask", check:
   true}` first (a thread still running processes is refused here, naming them, until `--processes`), the question
   `threadDeleteQuestion(id, worktree)`, then `sessions.delete {threadId, processes}`; emit `{threadId, ...gone}`;
   the line is each ended tree's `ended_lines` then `threadDeletedLine(id, gone)`.
5. Neither, cloud off: `usage_refusal("no thread <ref>", NO_THREAD_FIX)`.
6. Neither, cloud on: `deleting(client, ref)` (`workspaces.resolve`, `sessions.list`, `places.list`); a failure first
   asks `refuseMachineThread(client, ref)` (a thread on a machine says its machine goes with it) and else throws the
   first error; the question `deleteQuestion(d)`; `deleteWorkspace` (`workspaces.delete`); emit `{workspaceId, name,
   machineId, threads}`, line `deletedLine(d)`.
7. The question at a terminal answered no prints `<id or name> kept` on stderr and exits 1 with nothing on stdout,
   under `--json` too; off a terminal without `--yes` it is refused as usage (10.8).

## Tool door

`delete` never asks: without `confirm` it answers `<thread or name> kept. <the question's second line> Ask the person,
then call delete again with confirm true.` as an error result carrying `{threadId?, workspaceId, threads: 1}` or the
workspace's fields, and deletes nothing (:681-705); a sub-thread likewise. `resize` takes `size` matching `^\d+x\d+$`.
The recorded tool cases pin each.

## Tests (rule: test)

| Rule | Test |
|---|---|
| count refusals, `--verdict`, `--size`, `--processes`, cloud on and off sentence | `workspaces_life.rs::line_hand_*` |
| delete: check before the question; no terminal; answered no keeps it, exit 1; answered yes deletes | `workspaces_life.rs::line_delete_*`, `line_hand_delete_at_a_terminal_*`, `workspaces_rules.rs::delete_answered_no_sends_no_delete` |
| the tool's delete never deletes without confirm | `workspaces_life.rs::tool_delete_*unconfirmed*`, `workspaces_rules.rs::tool_delete_without_confirm_sends_no_delete` |
| frames and lines of every recorded case | `workspaces_life.rs::line_*` |
| tool door, pages | `workspaces_life.rs::tool_*`, `page_*` |

## What the TS leaves unclear

- `wsp delete` swallows every `threadOf` failure (`.catch(() => undefined)`), a refused token included, and goes on
  to the workspace road; the error a person then reads is the workspace road's.

## After-port tickets

- `wsp delete <ref>` with the cloud off where `threadOf` failed for a reason other than "no thread" (the host
  refused the read) says `no thread <ref>`, hiding the real failure.
