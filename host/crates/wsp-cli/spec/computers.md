# Unit `computers`: `verbs/computers.ts` (314)

Rust: `src/verbs/computers.rs`. Lines cite `packages/host/src/verbs/computers.ts` at `65f6b89c7`. Tests:
`tests/cli_b/computers.rs` (generated, 113) and `tests/cli_b/computers_rules.rs` (hand). Expected bytes come from the
TS command line over `daemon/crates/wsp-mcp/tests/answers/{computers,computers_set,recipes*,usage}.json` and 21 hand
cases (`fixtures/cli_b/computers.json`).

## Entries

`computer_verbs()` is `[computers, computers set, add (the command's tool), recipes, recipes show, recipes save,
recipes remove, usage, usage reset (line only)]` (:44-314). `add` is a `CommandToolVerb`: its line is `wsp add`
(cli/commands.ts), and this table carries only its tool, whose call is `addComputer` (setup-follow.ts).
`usage reset` is `cliOnly`: it spends a reset the person owns and a thread's socket is refused every usage op.

## `wsp computers` (:51-56)

Any positional: `wsp computers takes no positional arguments.` + usage. `readComputers` sends `places.list` and
`cost.spend` (both out before either answers); emit `{computers, spend, pending}`, the line
`computerLines(computers, hostPlatform(), spend, pending)`. A refused token is auth, exit 2.

## `wsp computers set <computer> [flags]` (:71-100)

1. Not exactly one positional: `wsp computers set takes one computer.` + usage.
2. Before dialling, in this order, each word checked by its helper with the computer's word as `on`:
   `--max-machines`/`--max-depth` (`agentsAsked`), `--threads` and `--machines` (`countAsked(flag, word, 1, ref)`),
   `--spend` (`dollarsAsked`), `--delete-after` (`deleteAfterAsked`, minutes to ms ×60,000), `--nap` (`napAsked`, then
   `napMsOf`), `--turn-limit` (`turnLimitAsked`, then `turnLimitMsOf`), then each `--reset` word through
   `one_of("reset", SETTING_RESETS, word, ref)`. Each refusal is its helper's sentence (recorded: `--threads for a
   takes a whole number of one or more, and got "0". Write it as --threads <n>.`).
   Note the object is built in the order threads, machines, spend, delete-after, nap, turn-limit, spawn, but the
   checks run as the spread evaluates, which is the same order; spawn is computed first.
3. `placeNamed(client, ref)` (`places.list`; `no place named <ref>; you have <names>. Run wsp computers.`).
4. `size_asked(place, --size, reset)` (:36-42): no word, nothing; a word `sizeFromWord` reads, `{size}`; a word it
   cannot read on a row that takes a size (`placeTakes(place, "size")`), `sizeChosen(place.sizes, word)` (refuses
   naming the sizes); on any other row, no size and `size` added to the resets once.
5. `setComputer(client, place, {...set, ...asked.set}, asked.reset, {name, ssh, recipe})` sends one `places.set`;
   the key order recorded is `placeId`, then the set fields in object order with `size` last, then `reset`, then
   `name`, `ssh`, `recipe`. A call that sets nothing still goes and the host refuses it.
6. Emit `{computer}`; the line `computerSetLines(computer, was, also, settled)` where settled is any set field or
   any reset given.

## `wsp recipes`, `recipes show`, `recipes save`, `recipes remove` (:179-260)

- `recipes`: no positional (`wsp recipes takes no positional arguments.`); `recipes.list`; emit `{recipes}`, line
  `recipeLines`.
- `recipes show <name>`: exactly one (`wsp recipes show takes one recipe.`); `recipes.get {name}`; emit
  `{recipe, hash}`, line `recipeShownLines`.
- `recipes save <name> --from <c>`: exactly one and `--from` (`wsp recipes save takes one name and the computer to save
  it from.`); `recipes.save {name, from}`; emit `{recipe}`, line `recipeSavedLine`.
- `recipes remove <name> [--yes]`: exactly one (`wsp recipes remove takes one recipe.`); `recipes.remove {name,
  check: true}` first, then the question `Remove the recipe <name>?` (second line: the computers keep what it put
  there), then `recipes.remove {name}`; emit `{recipe}`, line `recipeRemovedLine`.

## `wsp usage [--range day|week|month] [--by ...]` (:277-284)

No positional (`wsp usage takes no positional arguments.`). `--range` through `one_of` over `USAGE_RANGES`, default
`day`; `--by` over `USAGE_SPLITS`, default `agent`; both checked before dialling. `readUsage` sends
`usage.accounts {fresh: true}` and `usage.used {range, split, outside: true}` together. Emit `{accounts, used}` as the
host sent them (raw values); the line `usageTableLines({accounts, used} parsed, now)`, now the clock at print.

## `wsp usage reset <account> [--credit <id>] [--on <computer>] [--yes]` (:300-312)

Exactly one positional (`wsp usage reset takes one account.`). `usage.accounts` with no params (no `fresh`), then
`accountNamed(rows, word)`: the key exactly, else one row whose label or address equals the word ignoring case
(`no account <word>; wsp usage lists every account`, not-found; two rows: `<word> names <n> accounts: ...`, fix
`Name one by its key.`). The question `resetQuestion(label, credits.count)`, kept word `Every reset banked on
<label>`; then `usage.reset {account: key, creditId?, on?}`; emit the parsed answer, line its `said`.

## Tool door

`computers` answers the read whole; `computers_set` takes numbers where the line takes words (`threads`,
`machines`, `spend`, `delete_after` in minutes, `nap` in minutes through `napMsOf`, `turn_limit` in hours through
`turnLimitMsOf`), `size` as a word through `size_asked`, `reset` as a list, and answers `{computer}`; `add` is
`addComputer` with `{address, recipe?, later?, resume?, name?, sshPort?, keyPath?, hostKey?}`; the recipes and
`usage` tools answer their reads. The recorded tool cases pin each.

## Tests (rule: test)

| Rule | Test |
|---|---|
| refusals and the order of the word checks, nothing dialled before them | `computers.rs::line_hand_*` |
| places.set key order and every recorded set | `computers.rs::line_computers_set_*` |
| check before the question, off-terminal refusal | `computers.rs::line_hand_recipes_remove_off_a_terminal_after_the_check`, `line_hand_usage_reset_*` |
| usage's two reads and defaults | `computers.rs::line_usage_*` |
| tool door, pages | `computers.rs::tool_*`, `page_*` |
| `size_asked` on each kind of row | `computers_rules.rs::size_asked_*` |

## What the TS leaves unclear

- `usage` prints relative reset times against the clock at print; the recorded readings are all in the past, so no
  relative time shows. A case with a reset in the future would print a different line each minute.
- `usage reset` reads the accounts without `fresh`, where `usage` asks fresh ones; the reset then acts on a reading
  that may be old. Recorded as is.

## After-port tickets

- `computers set` sends a set with nothing in it and lets the host refuse (`nothing to set on attic`); the line could
  refuse before dialling, as `projects set` does.
