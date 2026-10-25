# harness_catalog
Source at 65f6b89c7: packages/runtime/src/harness-catalog.ts (308). Tests: harness-catalog.test.ts (38). Rust:
`src/harness_catalog.rs`; tests `tests/harness_catalog.rs` (46: the 38 cases and 8 fixture tests). Fixture:
`tests/fixtures/core_rest/harness_catalog.json`. The protocol helpers the cases call (`start_picks`, `efforts_for`,
`every_model`, `model_of`, `listed_pick`, `marked_default`, `catalog_source_line`, `no_models_line`) are wsp-wire's and
wsp-words'.

## The table
- **H1** (:71-223) Seven rows in this order: claude, codex, gemini, opencode, pi, hermes, cursor. The whole table is
  the fixture's `catalogs`: the implementer writes the rows from the TS and the test compares every value. Copy it with
  a script off the fixture rather than by hand.
- **H2** (:53-69 `fromTable`) Every row: `source: "table"`, `version` = `"<read> <version>, <date>"` from its pin or
  null without one (gemini, pi, hermes), `contextWindows: []` unless the row names its own, `steers`, `renames` and
  `images` false, and its permission mode equal to `bypassMode` marked `isDefault: true` (the others keep their own).
- **H3** (:37-41) An effort's label is its value capitalized, `xhigh` is `Extra high`; `levels(values, default)` marks
  the one default.
- **H4** (:225) `harness_catalog(id)` is the row of that harness; any other id is `None`.

## The readers
- **H5** `tool_asks` (:232-236): no mode, or the mode equal to the row's `bypassMode`, is `Never` (an unknown harness
  with no mode too). Else `UnlessReadOnly` where the row says `readOnlyToolsFree` (codex), else `Every` (no row
  included). Fixture `toolAsks`.
- **H6** `not_run_refused` (:240-242): `None` without lists; else the protocol's `not_run_refusal(lists,
  harness_catalog(lists.harness), model, the catalog agent's update_line)` (stand-in in `protocol_pending.rs`, spec
  agent_setups.md P4).
- **H7** `named_only` (:245): one option, `{value: "gpt-6-astra", label: "GPT-6-Astra"}`; none of the table's models.
- **H8** `model_label` (:249-259): the empty id is `Model not named`. `bare` = the text after the last `/`. A model of
  any row (`every_model`, current and legacy) or of `named_only` whose value equals `bare`, else whose value with a
  trailing `-\d{8}` dropped equals `bare` so dropped, answers its label. Else the words of `bare` undated split on
  `-`, empty ones dropped: a number after a number joins with `.`, every other word is capitalized and joined by a
  space; the result trimmed at the start, or `bare` where that is empty. Fixture `labels`: `x/` is the empty string,
  `-` is `-`, `claude-opus-4-1` is `Claude Opus 4.1`, `gpt-4o-mini` is `Gpt 4o Mini`.
- **H9** `smallest_model` (:264-267): the row's `smallModel` where `every_model` still lists it, else `None`.
- **H10** `catalog_from_probe` (:274-307):
  1. Each probed model, in the probe's order: `value` = slug; `label` = the table's label for that value (current or
     legacy) else the probe's; `description` where the probe has one; `isDefault: true` only where true; `efforts`
     copied where the probe names them (absent, not empty, where it names none); `defaultEffort` where named;
     `contextWindows` copied; `fast: true` where the probe says fast, else where the table's model of that value is
     fast, else absent.
  2. With `legacyListed` (codex), a probed model the table lists as legacy and not the default is folded: it leaves
     `models` and makes `legacyModels`. Without it, `legacyModels` is the table's legacy models less the probed ones;
     no table legacy list means none.
  3. `efforts` follow the probe's list, each the table's option for that value else one built by H3. Where the
     probe's default model names a `defaultEffort`, the table's mark is dropped and that effort alone is marked (none,
     where the list lacks it); else the table's marks stand.
  4. `permissionModes` follow the probe's list, each the table's option else `{value, label: value}`. The table's
     bypass mark rides only where the probe lists that mode.
  5. Every other field is the table's (`isDefault`, `steers` the runtime set on it included), with `source:
     "harness"` and `version` the probe's (null stays null).

## Decisions
1. The table is built per call (`harness_catalogs()` answers owned rows); a `LazyLock` is the implementer's choice.
2. `ToolAsks` lives here until wsp-slates takes slate-mcp.ts; `as_str` gives the TS words.

## Key order
A TS table row writes `source, version, contextWindows, steers, renames, images` first, then the row's own keys in the
order written, then `permissionModes` (harness-catalog.ts `fromTable`); `catalog_from_probe` writes the table's order
with `source` and `version` in place. The one order every row shares is wsp-wire's `HarnessCatalog` field order
(changed in the join to the TS's, ruling 6402177374 rule 1: `source, version, contextWindows, steers, renames, images,
harness, label, mcpServers, ...`, the adapter's declarations after the table's keys). It is not a parity line.

## Tests (`tests/harness_catalog.rs`)
| Rule | Test |
|---|---|
| H1, H2 | `names_every_harness_the_recipe_collects_each_parsing_as_the_wire_type_with_at_most_one_default_per_picker`, `every_agent_wsp_can_run_stands_in_with_its_own_models_its_own_pin_and_its_own_binary_in_the_footer_never_another_agents`, `the_pin_is_what_was_run_on_the_rows_own_binary_and_a_row_written_from_a_clis_docs_claims_none`, the `fixture_catalogs_*` test, and the claude, codex, access and permission cases |
| H1 against the pinned binary | `the_claude_row_is_the_pinned_binarys_own_handshake_*` and the three `the_claude_rows_against_the_pinned_binarys_own_model_catalog` cases (read packages/adapter-claude/test/fixtures) |
| H4 | `a_cli_without_a_flag_or_with_open_values_leaves_that_list_empty` and fixture `unknown` |
| H5 | `never_at_its_bypass_or_with_no_mode_named_*` and fixture `toolAsks` |
| H6 | fixture `notRun` |
| H7, H8 | `are_none_an_agents_table_lists_*` and fixtures `namedOnly`, `labels` |
| H9 | fixture `smallest` and the codex case |
| H10 | the eight `catalog_from_probe` cases, `marks_the_models_that_offer_faster_output_*`, and fixture `probes` (16 probes) |
| start_picks against the table | the 9 `startPicks` cases and the 3 default-effort cases |

The `start_picks` cases test wsp-wire's helper against this table; they go green with the table and wsp-wire's own
implementation.
