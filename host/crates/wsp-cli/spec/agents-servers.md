# Unit `agents_servers`: `verbs/agents.ts` lines 219-653 (servers, plugins, an agent's own settings)

Rust: `src/verbs/agents.rs` (the last sixteen entries; order in `spec/agents-skills.md`). Lines cite
`packages/host/src/verbs/agents.ts` at `65f6b89c7`. Tests: `tests/cli_b/agents_servers.rs` (generated, 193) and
`tests/cli_b/agents_rules.rs` (hand). Expected bytes come from the TS command line over
`daemon/crates/wsp-mcp/tests/answers/{servers*,plugins*,agents_*}.json` and 25 hand cases, five of them at a
scripted terminal or with an environment holding a secret.

## Reads: `wsp servers|plugins [<thread>] [--on <c>]` (:225-230, :610-615)

As `wsp agents` (spec/agents-skills.md): one thread at most, `agentsReport`, emit `{...reportFacts, servers}` or
`{...reportFacts, plugins: report.plugins ?? []}`, lines `serverRowLines` / `pluginRowLines`.

## Writes that need `--agent`

`servers tools|add|remove|disable|enable` and `plugins disable|enable` each: no name or more than two positionals is
`wsp <verb> takes one server's name and one thread at most.` (`one plugin's id` for plugins); then no `--agent` is
`wsp <verb> needs --agent, the agent whose config names the server, as wsp servers shows it.` (`servers add`: `the
agent whose config takes the server.`; plugins: `the agent whose plugin it is, as wsp plugins shows it.`), both +
usage, the count first.

- `servers tools <name> --agent <id> [<thread>] [--on] [--project <name>] [--refresh]` (:471-479): `toolsProject`
  (a bare `--project` refused: a thread finds its own project's servers), then `serverToolsOf` → `servers.tools
  {target, agent, name, refresh?: true}`; emit the answer, line `toolLines`.
- `servers add <name> [<thread>] --agent (--command "<line>" | --url <address> [--header n=VAR]...) [--env NAME]...
  [--project [<name>]]` (:505-517): in this order, `serverValues(ctx.env, --env, --header)` reads each named
  variable's value off this line's environment (unset: `<NAME> is not set in this environment, so there is no value to
  write.`; a header word not `<name>=<VARIABLE>`: `<word> is not <header>=<variable>.`), `projectAsked`,
  `serverCommand(--command)` splits the line as a shell would with nothing expanded (`${NAME}` kept as written; an
  unclosed quote refused). Body key order: `agent, name, command, args, url?, env?, headers?, project?: true`, then
  `servers.add {target, ...body}`. Emit the answer `{file}`, line `isInLine(name, file)`. The values cross in the frame
  and are never printed.
- `servers remove <name> ... --agent [--scope user|home|local|project] [--project [<name>]] [--yes]` (:550-559):
  `projectAsked`, `serverScope` (a project with `user` or `home` is refused), then the question `Remove <name> from
  the <agent> config?` before any host answer, then `servers.remove {target, agent, name, scope?}`; emit `{file}`,
  line `goneFromLine`.
- `servers disable|enable` (:580-589): `servers.toggle {target, agent, name, scope?, on}`; line `turnedInLine`.
- `plugins disable|enable <id> --agent <id> [<thread>] [--on]` (:633-640): `plugins.toggle {target, agent, plugin,
  on}`; emit `{plugin: row}`, line `pluginTurnedLine`.

## An agent's own settings

- `agents signin <agent> [<thread>]` (cli only, :248-257): one agent and one thread at most. No thread: the target is
  this computer (`agentsTarget`); a thread: `signInWorkspace` (:36-43) reads `threadAt` then `places.list` and refuses
  a machine at a cloud provider with `brokeredSignInRefusal(agentName)` and fix `brokeredSignInFix(hereName(places) ||
  "this computer")`, before anything runs there. Then `signInHere` runs the agent's own sign-in at this terminal; the
  line `signedInLine`; exit 0 when signed in, else 1.
- `agents key <agent> [--api-key]` (cli only, host side, :267-283): one agent. Off a terminal: `nobody is at this
  terminal to paste a token.`, fix `Run it in a terminal on the computer the host runs on.`, before dialling. `codex`
  without `--api-key` signs in here as `agents signin` does. Else the question is `Run <mint> in another terminal,
  then paste the token it prints` where the catalog names a mint command (Claude Code: `claude setup-token`) and not
  `--api-key`, else `Paste <name>'s API key`; the answer goes in `agents.key {agent, key, way?: "key"}` and the line is
  `<name>'s key is in this host's vault; every turn reads it from there.` on stdout through `io.log`, under `--json`
  too (nothing is emitted). `hostSide` makes the dispatcher refuse it aimed at another host.
- `agents addtools <agent>` (:292-296): one agent; `addTools` → `agents.addTools {target: {placeId: "here"}, agent}`;
  emit the answer, line `toolsAddedLine(agentName, file)`.
- `agents default <agent>` (:315-319): one agent; `preferences.set {patch: {defaultAgent}}`; line `defaultAgentLine`.
- `agents set <agent> [--model] [--effort] [--access] [--hide]... [--show]... [--order a,b] [--add-model]...
  [--drop-model]... [--reset]...` (:335-350): one agent; `--order` split on commas, each trimmed, empties dropped;
  `agentDefaultsSet` (reads `preferences.get` when a picker list changes, merges, then `preferences.set`); emit
  `{agent, defaults}`, line `agentDefaultsLine`.
- `agents setup <agent> [--on] [--enable | --disable] [--program] [--config] [--arg]... [--env NAME]...
  [--unset-env NAME]... [--reset]...` (:390-418), in this order: one agent; both `--enable` and `--disable`:
  `--enable and --disable say two things.`, fix `Name one.`; each `--env` name through `envNameOf(name, "--env")` then
  `agentEnvRefusal(name, agentName)` (a variable that decides how the process starts, like `PATH`: `<refusal>.`, fix
  `ENV_REFUSED_FIX`); any `--env` off a terminal: `nobody is at this terminal to type a variable's value.`; then each
  value asked `<NAME> for <agent name>` where nothing echoes it, in order; then `agentSetupSet` → `agents.setup
  {placeId, agent, on?, program?, configDir?, args?, env?, ...}`; emit `{agent: row}`, line `agentSetupLines(row,
  config given)`. A typed value is never printed.

## Tool door

The tools take what the lines take, with three differences the recorded tool cases pin: `servers_add` reads values
off the tool server's own environment (`deps.env`), `agents_setup` takes no variable values at all (`{}`) and names
its computer as `agents_setup takes on, a computer by name`, and `agents_set` takes `order` as a list.

## Tests (rule: test)

| Rule | Test |
|---|---|
| count, then `--agent`, refusals; `--enable --disable`; `PATH` refused; off-terminal refusals | `agents_servers.rs::line_hand_*` |
| a typed or environment secret reaches the host and is printed nowhere | `agents_rules.rs::secrets_reach_the_host_and_are_never_printed` |
| no value taken off a terminal | `agents_rules.rs::a_variable_value_is_never_taken_off_a_terminal` |
| frames and lines of every recorded case | `agents_servers.rs::line_*` |
| tool door, pages | `agents_servers.rs::tool_*`, `page_*` |
| disable/enable templates | `agents_rules.rs::toggle_templates_make_the_same_entry_but_for_their_word` |

## What the TS leaves unclear

- `agents signin` and `servers signin` run a sign-in in this terminal (`signInHere`, agents-help); no case records
  one, since it needs a pty and a host that runs a program. Their refusals are recorded.

## After-port tickets

- `wsp servers remove` asks before the host's own refusals (no check op), as `wsp skills remove` does (10.8).
- `wsp agents key` prints its sentence on stdout under `--json` with no JSON object, so `--json` output is not JSON
  there (the contract says stdout carries JSON alone).
