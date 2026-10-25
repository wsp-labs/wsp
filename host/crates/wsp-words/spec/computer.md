# computer
Source: `packages/protocol/src/words/computer.ts` (743 lines) at a3b870a6e. Tests: `packages/protocol/test/format.test.ts`, `copy.test.ts`, `projects.test.ts`, `place-state.test.ts`, `workspace-state.test.ts` (18 cases ported, 3 in part; 2 ignored as constants alone; 4 `workspace-state` cases skipped, their subject is that unit), plus `pinned` in `tests/computer.rs` (every value printed by the TS under node).

## What it does
What this computer, a joined computer and a fork are called; the one sentence for every refusal a thread's own token, a guest line, a paired device and a malformed request meet; the host-key, root-shell and preflight checks of a computer being added, with the boolean marks a machine's check stamps on its error; the lines a pause, a wake and a stop on a joined computer end with.

## Behaviour
1. `this_computer(Darwin)` is "this Mac", Linux "this computer" (computer.ts:27). `is_mac_machine(os)`: `^(?:macOS|Darwin)\b` on the text, "" for None (computer.ts:33-35). `computer_word(os)`: this_computer of that (computer.ts:39).
2. `ports_word(c, computer)`: "own network" when ownNetwork; "" when the computer copies nothing; else `shares <computer>'s ports` (computer.ts:54-58).
3. `thread_word(id)`: the first 8 UTF-16 units (computer.ts:96); every sentence naming a thread reads it, `thread_at(id, on)` adds ` on <on>` for Some, even an empty one (computer.ts:260-262).
4. `SpawnAct` is the 20 keys of `SPAWN_ACTS` in its order, serialized as the key; `words()` is the table's phrase (computer.ts:134-156). `SPAWN_ACTS_ALLOWED` is the 9 acts of computer.ts:159 in that order. `spawn_act_refusal` lists the allowed acts' words joined by ", ", then `never <act words>` (computer.ts:265-267).
5. Lines naming a shell command build it with `shell_line` (wsp_wire::types::shell_quote): a word with a space or a quote is single-quoted, an embedded quote written `'\''` (computer.ts:170, 294, 329, 372, 383, 389, 407).
6. `is_object_frame(v)`: true only for a JSON object (computer.ts:204-206). `REQUEST_BODY_TOO_LARGE` is "a request body is at most 64 KiB" (computer.ts:214-215).
7. `spawn_depth_refusal(id, depth, cap, held)`: deeper = depth + 1 written by `number_string`; a computer's cap names `wsp computers set <computer> --max-depth <deeper>` and "this computer"; a workspace's cap names the workspace and "ask the person" (computer.ts:325-332).
8. `elsewhere_workspace_line(name, on, from, act, lead)`: `ACROSS_ACTS[act]`; with a lead, `ask thread <8 units>, your lead, with wsp send <8 units> "<message>"`; else "ask the person" (computer.ts:388-391).
9. `agents_word(a)`: "" unless spawn is exactly true; `<n> machine` for 1 else `machines`. `agents_line`: "agents may not spawn", or `agents may spawn: up to <n> machine(s)`; maxDepth is never read (computer.ts:454-461).
10. `host_key_kept_note(key, file)`: the key alone when file is None or ends with `/.ssh/known_hosts`, else `<key> in <file>` (computer.ts:473-475).
11. `place_root_shell_refusal(address, shell)`: zsh reads `~/.zshenv`, fish `config.fish`; any other shell takes the not-read clause (computer.ts:495-506). `PLACE_ROOT_SHELLS` is bash, sh.
12. `host_key_matches(pinned, read)`: each side `js_trim`, split on JS whitespace, the first word starting `SHA256:`, else the trimmed whole; true only when pinned's mark is non-empty and the marks are equal (computer.ts:511-514).
13. `machine_lacking(said)` and `machine_unanswered(said)` are `HostError::new(said)` with the mark `wspMachineLacks` or `wspMachineUnanswered` and nothing else (computer.ts:594-603). `machine_lacks_line(e)`: the message only for a lacks mark; `machine_never_answered(e)`: the unanswered mark (computer.ts:606-613). `machine_lacks_short`: the text before the first ",", trimmed (computer.ts:620-622).
14. `wake_asks_in(for, every)`: `ceil(for / every)` (computer.ts:665). `wake_asking_again_line`: short `asking a/o`, long `waking, asking again (a of o)` (computer.ts:678-680). `thread_left_line(name, pids)`: none, one or many pids, joined ", " (computer.ts:717-721).

## Edge cases
- `thread_word("abcdefg😀")` is "abcdefg": node keeps the lone high half (`"abcdefg\ud83d"`), rule 8.5 drops it (`a_thread_is_named_by_its_first_eight_units`). An id of four emoji is the four emoji.
- `is_mac_machine("darwin")` and `"macOSX"` are false (case and word boundary).
- `host_key_matches("", "")` and two blank sides are false; `"ssh-ed25519 AAAA"` against `"AAAA"` is false (no SHA256 mark, whole texts differ).
- `machine_lacks_short(",x")` is ""; `wake_asks_in(0, 1000)` is 0.
- A computer name with a quote is quoted `'o'\''brien'` in the command a refusal names.

## Behaviour nobody wrote down
- Rules 8.4, 8.5: 8 units, never half a pair (the one difference from the TS, class `lone-surrogate`).
- Rule 5.1: the marks are not kinds; a marked error classes as provider.

## Tapes
Every refusal of a thread-scoped token (`THREAD_OPS`) and a paired device (spec 04 rule 4.17 quotes `deviceHeldRefusal`); `places.add` host-key refusals; the non-object frame refusal (rule 4.8).

## Decisions
1. `HostError::marks` carries the two marks (exit unit); `marked` reads them.
2. `move_timed_out_line` and the MachineState arguments take the state's wire word.
3. camelCase string constants of the TS (`guestNoSessionLine` and five more) are SCREAMING_SNAKE here.

## After the port
- `agents_line` never says the depth cap it is handed.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `the_computer_being_read::*`, `what_a_row_says_about_a_copys_network::*`, `this_computer_is_named_off_what_it_reported` |
| 3 | `a_thread_is_named_by_its_first_eight_units` |
| 4, 5 | `every_spawn_act_reads_its_own_words`, `spawn_acts_keep_the_tables_order_keys_and_words` |
| 6 | `only_a_json_object_is_a_frame` |
| 7, 8, 9 | `the_depth_refusal::*`, `forks_depth_and_reach_refusals` |
| 10, 11, 12 | `host_keys_and_root_shells` |
| 13 | `a_machine_check_marks_its_error_and_nothing_else_reads_as_marked` |
| 14 | `a_resume_the_provider_does_not_take::*`, `a_machines_lines_hold_their_edges` |
