# exit
Source: `packages/protocol/src/exit.ts` (116 lines) at a3b870a6e, and `HostError` as spec chapter 05 Proposal 4 shapes it. Tests: `packages/protocol/test/exit.test.ts` (8 cases; 7 ported, the `RuntimeResponse` case at :48 belongs to `wsp-wire`), plus the pinned cases in `tests/exit.rs`.

## What it does
The exit contract: four classes with codes 0 to 3, each error classed by the kind stamped where it was born, never by its words. A refusal is two halves, joined for the terminal by `refusal_line` (words/thread) with the fix kept beside it. A refusal that outlives its answer is cut to 11 lines of 400 units. `json_line` is every JSON a verb prints.

## Behaviour
1. `ExitClass` order and codes: ok 0, provider 1, auth 2, usage 3; `words()` are `EXIT_WORDS` word for word (exit.ts:12-24).
2. `Kind::class`: usage, invalid, not-found, processes-left are Usage; auth is Auth; every other kind, an `Other` included, is Provider; no kind is Provider (exit.ts:34-40).
3. `verb_failure(e)`: `{error: e.message, class, exit, errors?, warnings?}` in that key order; errors and warnings only where the error holds them (exit.ts:56-58,63-67).
4. `refusal_fields(e)`: errors and warnings as above, then `left` only when the kind is ProcessesLeft and `left` is set (exit.ts:44-47).
5. `refusal(happened, fix, kind)`: message `refusal_line(happened, fix)`, `fix` set, kind only when given (exit.ts:72-73). `usage_refusal` stamps Usage; `cloud_off_refusal(line)` is `refusal("<line> needs the cloud, which is off on this computer:", "start the host with WSP_CLOUD=1 to turn it on.", Usage)` (exit.ts:106-109). `auth_refusal` and `not_found_refusal` keep the message as given, no fix (exit.ts:112-116).
6. `refusal_said(message, fix)`: when the fix is given and the message ends with `" " + fix`, the message less that tail; else the message (exit.ts:76).
7. `refusal_parts(e)`: `{said: refusal_said(message, fix), fix, kind: kind string}` (exit.ts:84-88).
8. `marked_cut(line, max)`: unchanged up to `max` UTF-16 units; else the first `max` units and ` (cut N characters)`, N = units taken off (exit.ts:96, rule 8.4).
9. `kept_said(said)`: split on "\n"; the first 11 lines each `marked_cut(_, 400)`; `(cut N lines)` appended when more than 11 (exit.ts:99-103).
10. `escape_c1`: each char U+007F..U+009F becomes `\u00xx` in lower-case hex (exit.ts:51). `json_line(v, space)`: JSON.stringify layout (no spaces; or `space`-indented with `": "`, `[]` and `{}` for empties), numbers by `js::number_string`, keys in held order, then `escape_c1` (exit.ts:54).

## Edge cases
- `kept_said("")` is ""; exactly 11 lines say no cut; a blank line is a line (`kept_said_keeps_exactly_eleven_lines_and_says_none_cut_at_eleven`).
- `marked_cut` at exactly 400 units is unchanged; a cut inside a pair drops the half and N still counts units taken off the original (`marked_cut_counts_utf16_units_and_says_how_many_it_took`: 201 emoji cut at 400 units say 2).
- `refusal_said` with an empty fix takes one trailing space off a message ending in one (`refusal_said_takes_only_the_fix_it_was_joined_with_off_the_end`).
- A VerbFailure with class "ok" does not parse (`the_failure_object_is_...`).
- A float that is integral prints with no ".0" in `json_line` (rule 4.25).

## Behaviour nobody wrote down
- Rule 5.1: the class table is one table; a kind outside it is the provider's.
- Rule 4.26: `json_line` keeps the key order of the value; serde_json's `preserve_order` is on.

## Tapes
Every refusal on every op: its `kind` and `fix` come from here. None is exit's alone.

## Decisions
1. `Kind` is closed over chapter 05's table plus `Other(String)`: the TS stamps `unavailable`, `quota` and `provider` too (`packages/` grep at a3b870a6e), and a client reads kinds off the wire it does not know. `Kind::parse("Usage")` is `Other`: kinds are case-sensitive.
2. `HostError::marks` holds the boolean marks words/computer stamps (`wspMachineLacks`, `wspMachineUnanswered`); the TS keeps them as Error properties.
3. A thrown non-Error (`verbFailure("plain")`, `refusalParts("plain")`) has no Rust counterpart: every error is a `HostError`; `HostError::new("plain")` gives the same answers.
4. `refusalParts` reads `kind` as a string; here `kind` is `Kind::as_str`.

## After the port
- `exitClassOf` classes a schema failure (no kind) as provider, exit 1 (chapter 05 lists it).

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `is_one_table_ok_0_provider_1_auth_2_usage_3_each_class_with_its_words` |
| 2 | `classes_an_error_by_the_kind_stamped_on_it_...`, `every_kind_classes_by_kind_class_and_none_is_the_providers`, `kind_reads_back_as_the_string_it_was_stamped_with` |
| 3 | `the_failure_object_is_the_message_...`, `refusal_fields_carry_left_only_under_processes_left` |
| 4 | `refusal_fields_carry_left_only_under_processes_left` |
| 5 | `a_refusal_keeps_both_halves_apart_...`, `cloud_off_refusal_names_the_line_and_the_variable_that_turns_the_cloud_on` |
| 6, 7 | `reads_the_halves_a_refusal_was_stamped_with_...`, `refusal_said_takes_only_the_fix_it_was_joined_with_off_the_end` |
| 8 | `marked_cut_counts_utf16_units_and_says_how_many_it_took` |
| 9 | `cuts_each_line_at_400_characters_...`, `kept_said_keeps_exactly_eleven_lines_and_says_none_cut_at_eleven` |
| 10 | `escapes_del_and_every_c1_control_character_...`, `json_line_lays_out_as_json_stringify_with_keys_in_the_order_held` |
