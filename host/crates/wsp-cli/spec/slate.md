# Unit `slate`: `verbs/slate.ts` (224)

Rust: `src/verbs/slate.rs`. Lines cite `packages/host/src/verbs/slate.ts` at `65f6b89c7`. Tests:
`tests/cli_b/slate.rs` (generated, 72) and `tests/cli_b/slate_rules.rs` (hand). Expected bytes come from the TS
command line over `daemon/crates/wsp-mcp/tests/answers/slate_*.json` and 33 hand cases (`fixtures/cli_b/slate.json`).

## Entries

`slate_verbs()` is `[slate catalog, slate write, slate state, slate read]` (:84-221). cli-a's `thread_verbs()`
splices them in after the answer verbs (threads.ts:449). `slate_tool_names()` is `SLATE_TOOLS`, the four tools in
that order (:224), which a client loads up front and a server with no slate leaves off.

## The shared steps

- **Target** (`slate_target`, :19-26). A thread named: `threadOf(client, ref)` (`sessions.list`), then
  `{threadId: thread.threadId ?? thread.id}`. None named: `{turnToken: env.WSP_TURN}` when `WSP_TURN` is set and not
  empty (`turnTokenOf`), else `{}`, which the host reads off the caller's own token. Catalog never names a thread.
- **Ask** (`slate_ask`, :29-32): the reply with its own `id` and `ok` removed, every other key kept in the host's order.
- **Params** are built by spreads, so key order is: the target, then the verb's fields in the order below, `undefined`
  values left out (`pick`).
- **Emit** (`emit_slate`, :35-39): two emits, `{text}` printed as the text, then the rest of the answer with no line.
  So `--json` prints two lines (`{"text":..}` then the rest, even `{}`), and text mode prints the sketch alone.
  Catalog emits once: the whole answer, printed as its `text`.

## `wsp slate catalog [<name>]` (:91-97)

More than one positional: `wsp slate catalog takes one name at most.` + usage. `slates.catalog` with
`{turnToken?, name?}`.

## `wsp slate write [<thread>] [<file>] [--check] [--set p=json]... [--press id] [--row n] [--action n] [--if-version n]` (:107-126)

In this order, each refusal usage class, exit 3:
1. Each `--set` word through `state_value`: none refuses `--set <word> is not <path>=<json>.`, fix
   `Write it like --set '$i=2'.` The first bad word refuses; values keep the order given, a repeated path takes the
   last value at its first place (a JS object).
2. `--row` then `--action`: each read with `Number` when given; refused `--<flag> goes with --press, a whole number from 0:`
   + usage when `--press` is absent, the number is not an integer, or below 0. `0x2` is 2.
3. Rehearsal = some value set or `--press`. No positional and no rehearsal, or more than two positionals: refused
   `wsp slate write takes a file, after a thread where it is not yours.` + usage.
4. Two positionals are `<thread> <file>`; one ending `.slate` or `.json` is the file; any other one is the thread.
   No file and no rehearsal: `wsp slate write takes a .slate or .json file, after a thread where it is not yours.` + usage.
5. Dial. Then `--if-version` (`if_version_of`, :64-70): a string flag read with `Number` (so `--if-version=` is 0),
   refused `--if-version takes a version number, and <raw> is not one.` + usage unless a whole number from 0.
   This check comes after the dial (see tickets).
6. The target (a thread lookup goes out before the file is read), then the file (`slate_file`, :42-62):
   - on a line carried from a machine (`elsewhere`): `<path> is a file on your machine, which this host cannot read.`,
     fix `Pass the slate to slate_write as text instead.`
   - the path is resolved against the line's cwd (else the process's);
   - neither `.slate` nor `.json` (judged on the path as typed): `<path> is neither a .slate nor a .json file.`
   - unreadable: `there is no file at <absolute path>.`, fix `Name a .slate or .json file that is there.`
   - `.slate`: `{text}` as read (UTF-8).
   - `.json`: parsed as V8's `JSON.parse` does, and refused `<path> does not parse as JSON (<V8's message>).`, fix
     `Fix the JSON or write the JSX-like form to a .slate file.` The message is V8's own
     (`Expected property name or '}' in JSON at position 1 (line 1 column 2)`, `Unexpected end of JSON input`,
     `Expected double-quoted property name in JSON at position 7 (line 1 column 8)`). Not an object (an array,
     `null`, a number): `<path> holds no JSON object.`, fix `Write the stored document, an object with schema, root and pieces.`
   - the document is sent as JS reads it: a duplicate key keeps its first place with the last value, a number past
     2^53 is the double (`12345678901234567890` is sent `12345678901234567000`), `0.1e1` is `1`.
7. `slates.write` with `{target, text | document, check: true (only when --check), values?, press?: {piece, index?,
   action?}, ifVersion?}`, the row sent as `index`.

## `wsp slate state [<thread>] [<path>=<json>...] [--start <run>]... [--if-version <n>]` (:149-163)

Every positional through `state_value`: pairs become values, the rest are loose words. More than one loose word, or no
value and no `--start`: `wsp slate state takes $path=<json> words or --start <run>, after a thread where it is not
yours.` + usage. Dial, then `--if-version` as above, then the target (the one loose word is the thread), then
`slates.state` with `{target, values?, start?, ifVersion?}`.

## `wsp slate read [<thread>] [--values <path>]... [--no-text] [--no-sketch] [--document]` (:180-187)

More than one positional: `wsp slate read takes one thread at most.` + usage. `slates.read` with `{target, values?,
text: false (--no-text), sketch: false (--no-sketch), document: true (--document)}`.

## `state_value(word)` (:73-82)

The first `=` at index 1 or later splits the word; at index 0 or absent the word is not a pair. The value is
`JSON.parse(raw)` as V8 reads it, else the raw text: `$big=12345678901234567890` is the double, `$huge=1e400` is
Infinity and is sent `null`, `-0` is sent `0`, `01` stays the text `"01"`, ` 1 ` is 1, `i=` is the text `""`.

## Tool door

Each tool (`call`) takes the same steps with its inputs: `thread`, `text`, `document`, `check`, `values`, `press`,
`row` (sent as `press.index`), `action`, `if_version` (sent as `ifVersion`), each sent only when given, and answers
`asText(answer.text, answer)`: the text and the whole answer as structured content. The recorded tool cases pin it.

## Tests (rule: test)

| Rule | Test |
|---|---|
| refusal order and words, file reading, V8 JSON, `Number` words, token target | `slate.rs::line_hand_*` |
| frames, two-emit JSON output, recorded answers | `slate.rs::line_slate_*`, `tool_slate_*` |
| pages | `slate.rs::page_*` |
| `state_value` splits and V8 values | `slate_rules.rs::state_value_*` |
| `slate_ask` drops only `id` and `ok` | `slate_rules.rs::slate_ask_keeps_every_other_key_in_order` |
| `slate_tool_names` | `shared_rules.rs::slate_tool_names_are_the_four_slate_tools` |

## What the TS leaves unclear

- `--row`'s refusal ends in a colon before `usage:` (`a whole number from 0: usage: ...`); kept as is.
- A `.json` file read as UTF-8 with a byte order mark: V8 rejects the BOM; not recorded.

## After-port tickets

- `--if-version` is checked after the dial (:115, :154), so a typo in it can start a host before it is refused.
- The V8 `JSON.parse` reader with V8's messages lives privately in `wsp-words/src/start.rs:222-240` (`parse_json`);
  this unit needs it public. One line in `wsp-words`, outside this crate: the coordinator's call (rule 1.7 does not
  apply, no crate moves).
- A slate value or `.json` document nested past 500 levels is refused here (`json_whole.rs`: the catalog's reader stops
  at 500, and the read fails where it would have answered a tree cut at 128 levels); node's `JSON.parse` sends it whole.
