# Unit `projects`: `verbs/projects.ts` (274)

Rust: `src/verbs/projects.rs`. Lines cite `packages/host/src/verbs/projects.ts` at `65f6b89c7`. Tests:
`tests/cli_b/projects.rs` (generated, 96) and `tests/cli_b/projects_rules.rs` (hand). Expected bytes come from the TS
command line over `daemon/crates/wsp-mcp/tests/answers/{projects,projects_remove,projects_set,projects_seed,threads,
threads_wait}.json` and 14 hand cases (`fixtures/cli_b/projects.json`).

## Entries

`project_verbs()` is `[projects, projects add (tool only), projects remove, projects set, projects seed, threads,
threads wait]` (:49-274). `projects add` has no line: the command line records a project with `wsp add`
(`toolOnly`, :82-83), so its tool is the only door.

## `wsp projects` (:56-68)

1. Any positional: `wsp projects takes no positional arguments.` + usage.
2. `projects.list` (`projectsOf`). With none: emit `{projects: [], defaults: {}}` and the line `NO_PROJECT_YET`;
   nothing else is asked.
3. With some, in this order, one at a time: `workspaces.list`, `sessions.list` (`threads`), `places.list`
   (`placeNames`; a refusal is swallowed and every computer reads by its id), `projects.defaults`.
4. Emit `{projects, defaults}`; the line is `table([[PROJECT, ID, COMPUTER, SOURCE, PATH, BASE, THREADS, NEW THREADS],
   ...projectLine(p, held, running, named) + defaultsCell(defaults[p.id])])`.

## `wsp projects remove <project> [--yes] [--force]` (:118-133)

1. Not exactly one positional: `wsp projects remove takes one project.` + usage.
2. `projects.resolve` (`projectOf`), then `projects.remove` with `{projectId, check: true, force?: true}`: the
   host's own refusals come before any question.
3. The question (`confirmed`, io.ts:217): `Remove <name>?` then the second line, with
   `\nWith it goes work no remote has: <unsaved>.` added when the check answered `unsaved`. Off a terminal without
   `--yes`: refused `<first line> There is no terminal to answer on. Pass --yes to say yes.` (usage). Answered no at a
   terminal: `<name> kept`, exit 1.
4. `projects.remove` with `{projectId, force?: true}`; emit `{project, said}` (project as resolved, `said` off the
   wire), the line `said`.

## `wsp projects set <project> [--agent] [--model] [--effort] [--access] [--after-worktree] [--tracker]... [--reset]...` (:155-168)

Not exactly one positional: `wsp projects set takes one project.` + usage. Then `projectDefaultsSet` (agents-help)
with the given fields in the order agent, model, effort, access, afterWorktree, then `trackers` and `reset` always
(empty lists when absent). Emit the answer `{project, defaults, afterWorktree?, trackers?}`; the line
`project_set_lines`: `newThreadsHeadLine(project.name)`, `threadDefaultsLines(defaults)`, then
`afterWorktreeLine(cmd)` and `trackersLine(list)` when the answer carries them.

## `wsp projects seed <project> [--keep]... [--cut]... [--none] [--yes]` (:197-202)

Not exactly one positional: `wsp projects seed takes one project.` + usage. `projects.seed` with `{project, keep?,
cut?, none?: true, save?: true}` (lists only when not empty, `--yes` is `save`), the reply parsed as `ProjectSeed`.
Emit the seed; the line `project_seed_text`: `head`, a blank line, then when there are rows `table(rows)` each line
indented two spaces and a blank line, then `foot`, joined by newlines. A save the host refuses (a thread's token) is
the host's sentence.

## `wsp threads [<project>] [--tree] [--watch]` (:224-229)

More than one positional: `wsp threads takes at most one project; wsp threads wait is its one subcommand, and wsp
thread read <thread> prints what one said.` + usage. Then `drawRows(ctx, "wsp threads", ..)` (agents-help): once,
or every second with `--watch`. Each draw: `threadRows(client, project)` (`workspaces.list`, `sessions.list`, and
`projects.list` when a project is named); with `--tree` the rows as `threadTree`, each indented two spaces per depth;
the lines `table([THREAD_HEAD, ...])` then `diskGoneLines(rows)`. The emit value is `{threads: rows}`.

## `wsp threads wait <thread>... [--timeout <s>] [--tail]` (:246-258)

1. No positional: `wsp threads wait takes one thread or more.` + usage.
2. `timeoutFlag(--timeout)` before dialling: refused `--timeout takes seconds, a number above zero, and got "<w>".
   Write it as --timeout <seconds>.`; seconds become milliseconds.
3. `threadsOf(client, refs)` (`sessions.list`; a ref naming none: `no thread <ref>`, usage). `waitThrough`
   subscribes to events (`events.subscribe`, once) and reads `sessions.list` again; a thread already over is
   answered at once with its history (`sessions.history` with `{workspaceId, threadId, whole: true}`), a Boat
   sub-thread also with `sessions.head` `{threadId, afterRead: true}`. A host restarting prints
   `HOST_RESTARTING_LINE` on stderr and the wait goes on.
4. `waitAnswer(named, waited, tail ? "tail" : "whole")`. Timed out: emit `{timedOut: true}` with no line and print the
   line on stderr (`thread <t> still running after 50ms`, `2 threads still running after 250ms`), exit 0. Else emit
   the value with the line (the reply whole under the finished line, or its last line with `--tail`).

## Tool door

`projects` answers `{projects, defaults}` read at once (`projectsOf`, `projectDefaultsOf`); `projects_add` sends
`projects.add` with `{source, on?, name?, base?, into?}` in that order and answers the added line (`addedProjectLine`
over `placeNames`, swallowed when refused) with the notice on a second line when there is one; `projects_remove`
asks no question (no `check` op); `threads_wait` takes `threads` (at least one) and `timeout` in seconds. The recorded
tool cases pin each.

## Tests (rule: test)

| Rule | Test |
|---|---|
| refusals, empty project list, full table with swallowed places refusal, tree | `projects.rs::line_hand_*` |
| check before confirm, the question, `--yes` | `projects.rs::line_projects_remove_*` and `projects_rules.rs::remove_asks_only_after_the_check` |
| frames and lines of set, seed, threads, wait | `projects.rs::line_*` |
| tool door | `projects.rs::tool_*` |
| pages | `projects.rs::page_*` |
| `project_seed_text` layout, `project_set_lines` | `projects_rules.rs::seed_text_*`, `set_lines_*` |

## What the TS leaves unclear

- `threads wait` timeouts are wall clock: the recorded `--timeout=0.05` and `0.25` cases wait that long.
- `--watch` redraws every second until a signal; not recorded (no terminal in the harness).

## After-port tickets

- `wsp projects` prints a project's name raw in its table: a newline in a name breaks the row (case `projects with
  rows`), where `wsp threads` cuts its titles.
- `threads wait --timeout` past 2^31-1 ms: node's timer clamps it to 1 ms, so `--timeout=3000000` times out at once
  and says `after 50000m`; the Rust waits the whole time (34.7 days). A TS fault; port it as is or fix both.
