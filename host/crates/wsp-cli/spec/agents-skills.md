# Unit `agents_skills`: `verbs/agents.ts` lines 1-218 (agents, skills and its subcommands)

Rust: `src/verbs/agents.rs` (the first eight entries). Lines cite `packages/host/src/verbs/agents.ts` at `65f6b89c7`.
Tests: `tests/cli_b/agents_skills.rs` (generated, 124) and `tests/cli_b/agents_rules.rs` (hand, shared with
`agents_servers`). Expected bytes come from the TS command line over
`daemon/crates/wsp-mcp/tests/answers/{agents,skills,skills_*}.json` and 14 hand cases.

agents.ts is 653 lines and splits at its seam: this unit is the reads of a computer (agents, skills) and the skill
changes; `agents_servers` is the MCP servers, plugins and an agent's own settings. Both write into the one file and
one `agent_verbs()`, in the TS order: agents, skills, skills search, skills show, skills add, skills remove,
skills disable, skills enable, servers, agents signin, agents key, agents addtools, agents default, agents set,
agents setup, servers signin, servers tools, servers add, servers remove, servers disable, servers enable, plugins,
plugins disable, plugins enable. The disable/enable pairs are built from one template each (:194-218, :573-603,
:626-652): the words differ only by `disable`/`enable`, `off`/`on again`, and `on: word === "enable"`.

## The read verbs: `wsp agents|skills [<thread>] [--on <computer>]` (:52-57, :74-79)

1. More than one positional: `wsp <verb> takes one thread at most.` + usage.
2. `agentsReport(client, thread, on, usage_is, "wsp <verb>")` (agents-help): the target is `{placeId: "here"}` with
   neither; `--on` reads `places.list` and targets `{placeId}`; a thread reads `sessions.list` then `workspaces.get`
   and targets `{workspaceId}`; both refused (`A thread already names its computer; give the thread or --on
   <computer>, not both.` + usage); a project's name refused naming `wsp threads <project>`. Then `agents.read
   {target}`. An answer this build cannot read is the host-schema refusal (`the host would not read this line.` + usage).
3. Emit `{...reportFacts(report), agents}` (or `skills`), the line `agentRowLines` / `skillRowLines`, a stopped or
   napping thread's kept report noted on the line under the table.

## `wsp skills search <query> [--limit <n>]` (:96-104)

The positionals joined by one space are the query (none is `""`, sent and refused by the host). `--limit` read with
`Number`: refused `--limit takes a whole number from 1 to 50.` + usage unless an integer in 1..=50 (`0x10` is 16).
`skills.search {q, limit?}`; emit `{skills: hits}`, line `skillHitLines`.

## `wsp skills show <skill> [<thread>] [--on <c>] [--project [<name>]]` (:122-127)

No skill or more than two positionals: `wsp skills show takes one skill and one thread at most.` + usage.
`--project` is a string flag that may stand alone (`valueWith: "on"`: `--project` with no value is `""` only with
`--on`, handled by `optional_values`). `projectAsked(project, thread, on, usage)` resolves `{project, name}`:
`--project` bare with a thread is the thread's own; with `--on` it must name one; a name without `--on` is refused.
`skillShown(..)`: an `<owner>/<repo>/<skill>` goes to `skills.get {skill}` and refuses a computer or project; a name
goes to `skills.preview {target, name, project?}`. Emit the preview, line `shownText` (the text, then the cut note
`(the first 64 KB of N KB)` when cut). The text is printed as the file holds it, escape bytes included.

## `wsp skills add <skill> [<thread>] [--on <c>] [--agent <id>]... [--project [<name>]]` (:145-151)

Same count rule (`wsp skills add takes one skill and one thread at most.`). `--agent` repeated is a list, sent only
when not empty. `skills.add {target, skill, agents?, project?: true}`; emit the answer, line `addedLine(skill, added)`.

## `wsp skills remove <name> [<thread>] [--on <c>] [--project [<name>]] [--yes]` (:176-183)

Count rule (`takes one skill's name and one thread at most.`). `projectAsked` before the question. Question
`Remove the skill <name>?` (second line: every folder and link go), then `skills.remove {target, name,
project?}`; emit `{removed}`, line `removedLine`. No check op: the question comes before any host refusal.

## `wsp skills disable|enable <name> [<thread>] [--on <c>]` (:201-207)

Count rule. `skills.toggle {target, name, on}`, never a project (`{project: false}` to the helper); emit
`{paths}`, line `turnedLine(name, on)`.

## Tool door

Each tool takes `thread`, `on` and, where its verb has it, `project` (a boolean or a name), and refuses with the
tool's own usage word (`aimedUsage("<tool>")`) where the line says `usage: ...`. Recorded tool cases pin each.

## Tests (rule: test)

| Rule | Test |
|---|---|
| count refusals, `--limit`, the query words | `agents_skills.rs::line_hand_*` |
| aim: here, `--on`, thread, both, project's name, unknown computer | `agents_skills.rs::line_agents_*`, `line_skills_*` |
| `--project` alone and with a value | `agents_skills.rs::line_skills_show_*` |
| remove asks before the host's answer | `agents_skills.rs::line_skills_remove_*` |
| every entry's place in `agent_verbs()` | `shared_rules.rs::every_entry_in_ts_order_and_of_its_kind` |
| the two templates make the same words as the TS | `agents_rules.rs::toggle_templates_*` and the pages |
| tool door, pages | `agents_skills.rs::tool_*`, `page_*` |

## After-port tickets

- `wsp skills show` prints a fetched SKILL.md raw, escape sequences included (`skills.sh` case prints `ESC[31m`): a
  file from skills.sh can drive the person's terminal. `printable()` exists for this.
- `wsp skills remove` asks before the host's own refusals (no check op), unlike every other remove (10.8).
