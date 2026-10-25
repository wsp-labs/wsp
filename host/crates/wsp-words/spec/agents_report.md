# agents_report
Source: the functions of `packages/protocol/src/agents-report.ts` (558 lines) at a3b870a6e; its zod shapes (`AgentRow`, `SkillRow`, `McpRow`, `PluginRow`, `AgentsReport`, `AgentsTarget`, `AgentsSignInEvent`, ...) and flat constants (`SKILL_PREVIEW_BYTES`, `CONFIG_READ_MAX_BYTES`, `CONFIG_NOT_PLAIN`, `CONFIG_PAST_CAP`, `SIGN_IN_ENDED_KEPT_MS`, the refusals that take no value such as `skillsSearchEmptyRefusal`, `unclosedQuoteRefusal`, `serverNameRefusal`, `addToolsHereRefusal`, `controlSignInRefusal`, `noSignInRefusal`, `SIGN_IN_LINE_REFUSAL`, `AGENTS_KEY_REFUSAL`) are `wsp_wire::types::agents_report`. Tests: `packages/protocol/test/agents-report.test.ts` (2 of 15 cases ported whole, :74 and :128, and the one `serverToolsLateRefusal` assert of :44; the other 12 parse wire shapes and stay with wsp-wire), in `tests/agents_report.rs`.

## What it does
The words of the report of what stands on one computer or workspace for each agent: the refusals of the skill, server, plugin and sign-in acts, the lines a report or a leave says about a config it did not read, and two readers: the words of a command line a person typed for a tool server, and the test for a control character in a name.

## Behaviour
1. `napped_word(pause_mode)`: "napping" for memory, "stopped" for disk and for none (agents-report.ts:16). The six napping refusals put it after "<name> is ": skills (:146), servers (:220), plugins (:295), the report (:424), a server's tools (:427), a sign-in (:536).
2. `pty_bare_on(target)`: true for a computer target whose `placeId` is not "here" (HERE_PLACE_ID); false for "here" and for a workspace target; the target's `project` is not read (:37).
3. `command_words(line)` (:151-173), one UTF-16 unit at a time: a single quote takes everything up to the next single quote as written, with no escapes; a double quote takes everything up to the next double quote, a backslash in it taking the next unit whatever it is and dropping itself; a backslash outside quotes takes the next unit; JavaScript whitespace ends a word, and a run of it makes no empty word; anything else joins the word. A quote of either kind that never closes answers None. A pair of quotes with nothing between still makes a word. A backslash that is the last unit of the line stays a backslash. Nothing is expanded.
4. `plugin_missing_line(row, agent, computer)` (:272-277), by `row.missing`: folder is "<agent> has <id> on|installed, but its folder is not on <computer>, so no turn loads it." (on where `row.on`, else installed); uninstalled is the config.toml sentence naming the marketplace; marketplace is the unlisted sentence and then "Its marketplace, <marketplace>, is a folder that is not on <computer>."; unlisted, and a row with no `missing`, is the unlisted sentence alone.
5. `server_tools_late_refusal(ms)`: "Did not answer in <js_round(ms / 1000)> s." (:392).
6. `login_path_refusal(chars)`: `chars` and EXEC_BODY_MAX (16384) in en-US grouping, "16,384" (:434-435).
7. `server_sign_in_copy_refusal(agent, line, why)`: "inside" is "<agent> signs a server in inside its own session: <line>"; any other value is the callback sentence that tells the person to run the line in a terminal on the computer their browser is on (:503-504).
8. `tool_not_installed_line(name, said)`: one "." at the very end of `said` comes off, then the sentence ends with its own "." (:550). `tool_not_installed_fix` (:551).
9. `has_control_char(s)`: true where `s` holds any of U+0000 to U+001F or U+007F to U+009F, which is C0, DEL and C1 (:521). `without_control_chars(s)`: the same units taken out, everything else kept (:524).
10. The rest are fixed sentences with the names put in as given, nothing quoted or trimmed: the project refusals (:45, :48), skills (:134-143), servers and config files (:179-217), plugins (:280-292), `cloud_server_off_line` (:377), the report refusals (:420-439), sign-in words (:488-511), `add_tools_no_config_refusal` (:517), `control_name_refusal` (:527), and the terminal sign-in lines (:539-546).

## Edge cases
- `command_words("  uvx  'a b'  \"c \\\" d\" e\\ f $HOME ")` is ["uvx", "a b", "c \" d", "e f", "$HOME"]; `"node '' x"` is ["node", "", "x"]; `""` is []; `x "unclosed` and `x 'unclosed` are None (test).
- `command_words("a\\")` is ["a\\"]; a no-break space (U+00A0) splits a word, being JavaScript whitespace (node, over the TS function's body).
- `has_control_char` is true for U+0085 and U+009B and false for "ü"; `without_control_chars("no\u{1b}[2Jti\u{9b}31mon\u{85}\u{7f} ü")` is "no[2Jti31mon ü" (test).
- `server_tools_late_refusal(20000.0)` is "Did not answer in 20 s." (test); 2500 is "3 s", since `Math.round(2.5)` is 3.

## Behaviour nobody wrote down
- Rule 8.6: `command_words` splits on JavaScript's whitespace (`/\s/`), which takes U+00A0, U+FEFF and the Unicode spaces, not ASCII alone.
- Rule 8.7: the control-character class reaches past ASCII (U+0080 to U+009F); every unit of it is a single char, so a walk by char matches the walk by UTF-16 unit.
- Rules 8.11 and 8.12: `server_tools_late_refusal` rounds with `js_round` and writes the number with `number_string`.
- Rule 8.14: `login_path_refusal` is `toLocaleString("en-US")`, written by hand as comma groups of three.

## Tapes
The `agents.read` answer (its `refused` lines and a stale report's word); `wsp agents`, `wsp skills`, `wsp servers` and `wsp plugins` with their acts and refusals; `wsp agents signin` and `wsp agents key`; the `said` of an `agents.signIn` event; the Settings page of each agent and its Add a tool server sheet, which splits the typed command with `command_words`.

## Decisions
1. A `pauseMode?: PauseMode` is `Option<&PauseMode>`.
2. `plugin_missing_line` takes the whole `PluginRow` for the TS `Pick<PluginRow, "id" | "marketplace" | "missing" | "on">`.
3. `server_sign_in_copy_refusal` takes `why` as `&str` for the TS union `"inside" | "callback"`; anything but "inside" reads as callback, as the TS ternary does.
4. The numbers `ms` and `chars` are `f64`.

## After the port
- The doc comment of `hasControlChar` says "A C0 control character or DEL", but its class also takes C1 (U+0080 to U+009F), and the test pins that. The behaviour is the one to keep; the comment is short.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | none: no test calls them |
| 3 | `the_agents_report_on_the_wire::a_command_line_a_person_types_splits_into_its_words_quotes_kept_together_and_nothing_exp` |
| 4 | none: no test calls `plugin_missing_line` |
| 5 | `the_agents_report_on_the_wire::a_tools_answer_carries_the_tools_or_the_harness_that_holds_the_sign_in_or_why_nothing_ca` |
| 6, 7, 8 | none: no test calls them |
| 9 | `the_agents_report_on_the_wire::a_name_holding_a_control_character_is_caught_by_one_predicate_whichever_one_it_holds_and` |
| 10 | `the_agents_report_on_the_wire::a_name_holding_a_control_character_is_caught_by_one_predicate_whichever_one_it_holds_and` (`control_name_refusal` alone); the rest none |
