# codex_catalog
Source: packages/adapter-codex/src/catalog.ts (176 lines) at a3b870a6e.
Tests: packages/adapter-codex/test/catalog.test.ts (13 cases; the 4 under "a probe on a machine whose ChatGPT sign-in the host holds"
call the adapter's `probe_catalog` and `account_models` and run in codex_adapter's file). Rust: `src/codex/catalog.rs`; tests
`tests/codex_catalog.rs`. Fixtures: `test/fixtures/catalog-probe.txt`, `catalog-probe-route.jsonl`, copied byte for byte.

## What it does
The probe that asks the installed codex what it takes, without a turn: `codex --version`, `codex --help`, and four app-server requests
(`initialize`, `model/list`, `config/read`, `account/read`) through wsp-catalog's `app_server_script`, read back by request id. Where the
host holds the machine's ChatGPT sign-in, the account's own model list (ChatGPT's `/codex/models`, read by the host) replaces the
binary's.

## Behaviour
1. Request ids 1 initialize, 2 `model/list`, 3 `config/read`, 4 `account/read` (catalog.ts:360-364). A brokered probe sends the first
   three only: the door holds the sign-in, so `account/read` would always say none (catalog.ts:366).
2. `catalog_probe_command` (catalog.ts:373): `<version probe>; echo __WSP_CATALOG_SEP__; <codex word> --help; echo __WSP_CATALOG_SEP__\n<server script>`,
   the server script being `app_server_script(word, [{lines, answers: lines.len()}])`, the word `codex_word(launch, broker)`.
3. `version_probe_command` is `cd ~ && <ENV_FROM_INPUT>; <codex word> --version` (catalog.ts:382): the env comes off the input, so no
   `PATH=` or `CODEX_HOME=` is a word of the line.
4. `parse_version` is the first `[0-9]+\.[0-9]+\.[0-9]+` in the text, else `None` (catalog.ts:387; JS `\d` is ASCII, so not Rust's `\d`).
5. `parse_catalog_probe(stdout, login, account_list)` (catalog.ts:476): split on the separator; fewer than three parts is `None`; parts past
   the third are ignored. No result for id 1 is `None` (wsp-catalog's `answers_of` keeps a result only when it is an object).
6. Models (catalog.ts:408) come from `account_rows(account_list)` when that gives rows, else the `model/list` result. Each row: `id` a
   non-empty string, not `hidden: true`. Fields in this order: `slug`, `label` (non-empty `displayName`, else the slug), `description` where
   a non-empty string, `efforts` (each `supportedReasoningEfforts[].reasoningEffort` that is a non-empty string; a non-array gives `[]`),
   `defaultEffort` where non-empty, `contextWindows: []`, `isDefault`, then `fast: true` only where `additionalSpeedTiers` is an array
   holding `"fast"`. `isDefault` is `entry.isDefault === true` when config names no model, else `slug == configured`.
7. The configured model (`config/read` → `config.model`, non-empty) that no row carries leads the list as
   `{slug, label: slug, contextWindows: [], isDefault: true}`, with no `efforts` key at all: absent means every effort stays open.
8. No models: `{refused: codexNotSignedInLine(login)}` only where `account/read` answered `requiresOpenaiAuth: true` and `account: null`
   (an absent `account` key is not null); else `None`.
9. The probe is `{version, models, efforts, permissionModes}`: efforts the union of every model's efforts in first-seen order
   (catalog.ts:465); permission modes the comma list in the first `[possible values: ...]` within 600 UTF-16 units of the first
   `--sandbox <SANDBOX_MODE>` in the help (catalog.ts:390), each trimmed, empties dropped; none found is `[]`.
10. `account_rows` (catalog.ts:445): `models` must be an array; rows that are objects with a non-empty `slug`, stable-sorted by `priority`
    ascending (a non-finite or absent priority reads 0); the first row whose `visibility` is `"list"` is the default; none such is `None`.
    Each row maps to a model/list row: `id`, `displayName`, `description`, `supportedReasoningEfforts` from `supported_reasoning_levels[].effort`,
    `defaultReasoningEffort`, `additionalSpeedTiers`, `hidden: visibility != "list"`, `isDefault`.
11. `read_catalog_probe(stdout, login, models)` (catalog.ts:500) asks `models(version)` only where a read is handed and the version part
    names a version; a failed read is no list. `account_models_of(models, version)` (catalog.ts:509) asks for the version given, else
    wsp's npm pin for codex (wsp-catalog's codex install road); no version, a failed read or no shown row is `None`, else the rows as models
    with no configured model.

## Edge cases
- `parse_catalog_probe("", ..)` and `"x<SEP>y<SEP>not json\n"` are `None`. A head of `<SEP><SEP>` gives version `None` and modes `[]`.
- A hidden model is left out; answers read by id whatever order they land in.
- `requiresOpenaiAuth: false`, or a signed-in account, with no models is `None`, not a refusal.

## Behaviour nobody wrote down
- Rule 8.3: the 600-unit window counts UTF-16 units. Rule 8.6: the `trim` of each mode is `js_trim`. The `str` here refuses `""`.

## Decisions
1. `AccountModels` (in codex_command) answers `Result<Option<Value>, String>`: an error and a `None` both read as no list here.

## After the port
- Nothing.

## Tests (tests/codex_catalog.rs, 9; the brokered 4 in tests/codex_adapter.rs; TS 13)
- Rules 1-3: `catalog_probe_command::reads_the_base_env_a_turn_gets_off_its_input_path_and_codex_home_included_so_a_p`,
  `catalog_probe_command::asks_for_the_version_the_help_and_the_four_app_server_requests_and_never_a_turn`,
  `catalog_probe_command::holds_the_app_server_s_stdin_open_until_its_answers_are_in_and_lets_go_on_the_la`; brokered requests:
  `codex_command.rs` `a_brokered_probe_asks_nothing_of_the_sign_in_which_the_host_holds`.
- Rules 4-6, 9: `parse_catalog_probe::reads_the_version_the_models_the_app_server_offers_with_their_reasoning_efforts`,
  `parse_catalog_probe::marks_a_model_fast_where_model_list_names_a_fast_speed_tier_for_it`,
  `parse_catalog_probe::leaves_out_a_model_the_app_server_hides_and_reads_each_answer_by_its_own_id_what`,
  `parse_catalog_probe::is_null_when_the_app_server_said_nothing_and_tolerates_a_missing_version_or_help`.
- Rule 7: `parse_catalog_probe::a_route_to_another_provider_leads_with_the_model_config_names_since_that_is_what` (asserts no `efforts` key).
- Rule 8: `parse_catalog_probe::names_the_sign_in_when_the_app_server_answered_and_offered_no_model_for_want_of`.
- Rules 10, 11: `codex_adapter.rs` `a_probe_on_a_machine_whose_chatgpt_sign_in_the_host_holds::*` (4).
