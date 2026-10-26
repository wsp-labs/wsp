# Unit `activity`: `verbs/activity.ts` (129), `verbs/activity-table.ts` (134), `verbs/conversations.ts` (60)

Rust: `src/verbs/activity.rs`, `src/verbs/activity_table.rs`, `src/verbs/conversations.rs`. Lines cite the TS at
`65f6b89c7`, files under `packages/host/src/verbs/`. Tests: `tests/cli_b/activity.rs` (generated, 55) and
`tests/cli_b/activity_rules.rs` (hand, named below). Expected bytes come from running the TS: the command line
over `daemon/crates/wsp-mcp/tests/answers/{activity,activity_end,conversations}.json` and hand cases
(`tests/fixtures/cli_b/activity.json`), the pure functions over the views of `packages/host/test/activity-table.test.ts`
(`pure.json`).

## Entries

`activity_verbs()` is `[activity, activity end]` (activity.ts:69-128), `conversation_verbs()` is `[conversations]`
(conversations.ts:32-59). Usage, about, flags and tool text are data generated from the TS table; the page tests
(`page_local_*`, `page_cloud_*`) hold them through the help page byte for byte.

## `wsp activity [<thread>] [--computer <c>] [--wide]` (activity.ts:77-82)

1. More than one positional: `usage_refusal("wsp activity takes one thread at most.", usage_is)`. Nothing dialled.
2. A thread named: `threadOf(client, ref)` first (one `sessions.list`; a ref naming none fails `no thread <ref>`, usage,
   exit 3), then `activity.read` with `{computer?, thread}`, keys in that order, `thread` the thread's `id`.
   No thread: `activity.read` with `{computer?}` alone. `--computer ""` is a string and is sent as `""`.
3. The reply's `activity` field parses as `ActivityView`; the emit value is the view, re-serialized in schema order
   (the json run prints `{"computer","at","threads","wsp","unowned"}` whatever order the host sent).
4. The line is `activity_table(view, {width, wide})`, width `io.redraw().columns()` when the terminal redraws, else
   none (TS `Infinity`): off a terminal nothing is cut. `--wide` is `flags["wide"] === true`.

## `wsp activity end <pid> --started <ms> [--computer <c>]` (activity.ts:103-114)

1. `pid = Number(word)` (`wsp_words::js::to_number`): refused `wsp activity end takes one pid.` + usage when the word is
   absent, there is not exactly one positional, or the number is not an integer above 0. So `0x10` is 16, `1e3` is
   1000, ` 7 ` is 7, `1.5`, `0`, `-4` (after `--`) and `x` are refused. Checked before `--started`.
2. `--started`: refused `wsp activity end takes --started, <STARTED_WORDS>.` + usage when absent, `""`, or
   `Number(started)` is not finite (`Infinity`, `abc`). A blank `" "` is 0 and is sent; `0x1F` is 31; `1.5` is sent
   as 1.5. Nothing is dialled before both checks pass.
3. One `activity.end` with `{pid, startedAt, computer?}` in that order. The reply parses whole as `ActivityEnded`
   (`id` and `ok` dropped by the schema); emit value is the parsed answer, line `ended_lines`.

## Pure words

- `activity_lines(view)` (activity.ts:42-48): per thread `thread <threadWord(id)> <title>  <fmtBytes(memory)>  <cpu>`
  then its processes in the order given, each `"  ".repeat(depth+1)` + `pid  kind  command[  :p1 :p2]  bytes  cpu`;
  then `wsp` and its processes if any, then `no thread` and the unowned if any. None at all: `no thread runs a process on <computer>`.
  `cpu` is `toFixed(1)` + `%`.
- `ended_lines(e)` (activity.ts:50-58): `freed` set wins: `freed the agent thread <threadWord(freed)> kept`, whatever
  `ended` holds. No `ended`: `endGoneSaid(pid)`. Else `ended <pid> <name>` + ` and <n> under it` when more than one,
  then `KILL after TERM's 2 s: <forced joined by space>` when any, then `still running after KILL: <left>` when any.
- `processes_asked(value, on)` is `one_of("processes", ["stop","keep"], value, on)`: absent passes as None; any other
  word refuses `--processes takes one of stop, keep, and got "<w>". Name one of those.`
- `processes_in()` describes the input through `cloud_text(.., cloud_on)`, so the cloud span drops with the cloud off.

## `activity_table(view, {width, wide})` (activity-table.ts)

Groups, in order: `wsp` (when `view.wsp` is not empty), each thread sorted by memory descending then `threadId`
ascending (`localeCompare`), then `No thread` (when `unowned` is not empty). Nothing at all: the same
`no thread runs a process on <computer>` line.

- wsp group: the daemon is the first `wsp`-kind row whose `procBins` include `wsp-daemon`; the host is the first other
  `wsp`-kind row. Head `wsp  <wspNote>`, or `wsp  <wspNoteBox>` (`What runs wsp here`) when there is no host (a box).
  Its figures are the sum of its rows. It is never folded.
- A thread's head is its title, figures the thread's own `memory` and `cpu`, not the sum of its rows.
- No thread's head is `<loose>  <looseNote>`, figures summed, never folded.
- Folding (`treeOf`, :45-71), only a thread's group and only without `--wide`: an *agent* row is kind `agent`, not turn
  plumbing (`isTurnPlumbing(command)`, a run script or input pump), and with no `agent` child. Every other `agent`-kind
  row that is not the group's first agent folds into the nearest agent below it, else into the first agent (the first
  real agent, else the first `agent`-kind row). Folded rows give their memory and cpu to the row they join and their
  children become its children. Children sort by memory descending, then pid ascending.
- A row's name is `activityName(entry, {daemon?, host?, agent?})`, `agent` given only to an `agent`-kind row and,
  under `--wide`, only one that is not plumbing (so `--wide` names a run script `bash` by its own name). The agent's
  name is the catalog agent its bin names (`agentByBin`), else `Agent`. Ports follow the name as ` :p1 :p2`.
  The kind column is `ACTIVITY_KIND_WORDS[kind]`, empty for `UNTAGGED_KINDS`.
- Totals line: `<computer>   wsp <fmtBytes(sum of group memory)>   CPU <round(sum cpu)>%   <n> process[es]   <t> thread[s]`,
  three spaces between, process count the non-head rows, thread count the threads; cut to the width with `ellipsize`.
- Then a blank line, a title row (`PID` and `COMMAND` only under `--wide`, `MEMORY`, `CPU`), then the rows.
- Widths are cells (`cell_width`: a wide character is two, an emoji is two, never cut through a character). `left` =
  `max(1, min(max(widest proc label + tag part, widest head label), 72, width - 2 - figures))`, figures being
  `[pidW + 2] + memW + 2 + cpuW`. The kind column drops (tagW = 0) when `left - tagPart < min(16, widest proc label)`.
  A head spans name and kind; a process row pads its label to `nameW` then the kind. Each line is the cells joined by
  `GUTTER` (two spaces), `trimEnd`, then ellipsized to the width. `--wide` never cuts (`room` infinite), the command last.

## `wsp conversations [<project>] [--agent <id>]` (conversations.ts:39-45)

1. Two positionals: `wsp conversations takes one project; <second> reads as a second.` + usage.
2. The project: the one named; else, when the line has a cwd, the project whose folder holds it
   (`projectOfFolder` over `projects.list`, a failed list read as none); else none. Then `conversations.list` with
   `{project?, agent?}` in that order. The reply parses as `ConversationsAnswer`; the emit value is the parsed answer.
3. Lines (`conversations_lines`, :27-30): a table `AGENT ID CHANGED SIZE BRANCH "" TITLE` when there are rows, else
   `no Claude Code or Codex conversations in this folder` only when nothing is held; then every held row as
   `<said>. <fix>`. CHANGED is `changed(lastAt)`: local time `YYYY-MM-DD HH:MM` (`sv-SE`), so tests run under
   `TZ=UTC`; a negative time is before 1970. SIZE `fmtBytes(bytes)` else `-`; BRANCH else `-`; the mark is
   `thread <first 8 of thread>`, else `open elsewhere` when live, else `wsp` for origin wsp, else empty. The title is
   printed raw (see tickets).

## Tests (rule: test)

| Rule | Test |
|---|---|
| refusals and their order, pid and start as `Number` reads them | `activity.rs::line_hand_*` (20 hand cases) |
| frames, key order, emit values, lines, exit codes for every recorded case | `activity.rs::line_activity_*`, `line_activity_end_*`, `line_conversations_*` |
| tool door answers and asked ops | `activity.rs::tool_*` |
| help pages, cloud on and off | `activity.rs::page_*` |
| table at 80/160/40 columns, `--wide`, early agent, wide characters, a box, empty, outside agent | `activity_rules.rs::table_*` (ported from activity-table.test.ts) and `table_matches_ts_byte_for_byte` |
| sort ties by pid and by thread id | `activity_rules.rs::table_ties_sort_by_pid_then_thread_id` |
| a thread's head carries the thread's own figures, not its rows' sum | `activity_rules.rs::thread_head_uses_thread_figures` |
| ended_lines and activity_lines words | `activity_rules.rs::ended_lines_*`, `activity_lines_match_ts` |
| conversations lines | `activity_rules.rs::conversations_*` (ported from conversations-verb.test.ts plus marks) |
| processes_asked | `activity_rules.rs::processes_asked_*` |
| host invariant 7: with the cloud off no page or tool text says cloud | `activity_rules.rs::invariant_7_cloud_off_names_no_cloud` (shared, runs over all of cli-b's entries) |

## What the TS leaves unclear

- `activity.read`'s `computer` and `thread` are sent in the spread order `{computer, thread}`; no test pins it but the
  frames recorded here do.
- `ctx.io.redraw?.columns()` is read once per line; a resize mid-print is not followed.

## After-port tickets

- conversations prints a title raw: a tab or newline in a title breaks the table (`marks` in pure.json). Other
  tables run `cell()` on each cell.
- activity end reads its pid with `Number`, so `0x10` ends pid 16; a person typing hex is unlikely, but the refusal
  sentence says "one pid" for `1.5` too.
