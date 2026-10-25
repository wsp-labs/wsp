# Unit claude-launch: environment, sign-in order, launch line

TS at a3b870a6e: `packages/adapter-claude/src/landmines.ts` (467 lines). Rust: `src/claude/landmines.rs`. Tests:
`tests/claude_landmines.rs` (45 + 7 ported), `tests/claude_launch.rs` (hard cases). Pure string building, except
`new_session_id`. Nothing here survives a restart; the launch line is what a re-attached run already ran.

## Environment (landmines.ts:12-157)

- Env order is the TS object's: a key set again keeps its first place; `delete` then set moves it last.
- `is_landmine(key)`: `CLAUDE_CODE_*`, `CLAUDECODE`, `FORCE_CODE_TERMINAL`. `strip_landmine_env` drops unset values
  and landmines in base order, but keeps `PLUGIN_SEED_ENV` (`CLAUDE_CODE_PLUGIN_SEED_DIR`, wsp_wire::types::env).
- `build_env` (:137), the stripped base less `ANTHROPIC_API_KEY` first:
  - brokered: drop ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL, ANTHROPIC_CUSTOM_HEADERS, ANTHROPIC_FOUNDRY_API_KEY,
    AWS_BEARER_TOKEN_BEDROCK; then base, then the headless overrides (CLAUDE_CODE_AUTO_CONNECT_IDE=0,
    CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL=1, CLAUDE_CODE_ENABLE_TODO_TOOLS=1, CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0,
    CLAUDE_CODE_MCP_STARTUP_WAIT_MS=0 in that order), then `brokered_sign_in_env`, then PROJECT_DIR_ENV where named.
    No FIRST_BYTE_ENV, no vault key or token.
  - otherwise: FIRST_BYTE_ENV=30000 first (a base value for it keeps first place with the base's value), then base,
    the overrides, the inherited sign-in (only where neither key, token nor brokered was handed: the base's non-empty
    CLAUDE_CODE_OAUTH_TOKEN, else its non-empty ANTHROPIC_API_KEY; the token wins), then ANTHROPIC_API_KEY=apiKey,
    CLAUDE_CODE_OAUTH_TOKEN=oauthToken, PROJECT_DIR_ENV, each where given.
  - Never CLAUDE_CONFIG_DIR or IS_SANDBOX of its own: both come from base unchanged.
- `brokered_sign_in_env` (:104): ANTHROPIC_BASE_URL=`http://127.0.0.1:<BROKER_PORT>`, the placeholder (login and
  token: CLAUDE_CODE_OAUTH_TOKEN; key: ANTHROPIC_API_KEY; gateway: ANTHROPIC_AUTH_TOKEN then ANTHROPIC_API_KEY=""),
  CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1, CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST=1, DISABLE_UPDATES=1,
  API_TIMEOUT_MS=3600000, and ENABLE_TOOL_SEARCH=true except for gateway.
- `sign_in_guard` (:180): `""` where vaulted (key, token or brokered) or nothing inherited; else
  `wsp_auth=$(env -u ANTHROPIC_API_KEY -u CLAUDE_CODE_OAUTH_TOKEN <program> auth status 2>/dev/null); case "$wsp_auth" in *'"authMethod": "api_key_helper"'*) ;; *'"loggedIn": true'*) unset ANTHROPIC_API_KEY CLAUDE_CODE_OAUTH_TOKEN ;; esac; unset wsp_auth; `
  (trailing space), `<program>` being `program_word("claude", launch)`.
- `terminal_resume_command` (:53): `CLAUDE_CONFIG_DIR=<shell_line([dir])> ` where base names one (a `None` value reads
  as absent), then `<program_word> --resume`.

## Launch line (landmines.ts:197-436)

- `build_command` refuses, in this order (the TS evaluates the flags array left to right): zero or two of
  sessionId/resume ("buildCommand needs exactly one of sessionId or resume"); an id that is not a UUID ("session
  identifier must be a UUID, got \"<id>\""); a copyOf without a fresh sessionId or not a UUID ("a copy opens a new
  session of a session named by its UUID, got \"<copyOf>\""); an aside without resume ("a side question resumes the
  copy it is asked on"); the permission mode slug, read only off a turn (an aside runs at `default` and never reads
  its own, a bad one included); then contextWindow with no model ("contextWindow needs a model to
  ride on") or not 200k/1m ("contextWindow must be \"200k\" or \"1m\", got \"<w>\""); then the model slug, then
  the effort slug. A slug must match `^[A-Za-z0-9][A-Za-z0-9._:\[\]-]*$`, else `<name> must be a plain slug, got
  "<value>"` with name permissionMode, model or effort. The model is checked with `[1m]` already on where the window
  is 1m (`got "bad model[1m]"`). `""` is a value like any: an empty slug is refused, an empty name is `--name ''`, an
  empty cwd `cd '' && `.
- Words, joined by one space: `<agent_word("claude", launch)> -p`; the person's words (less a JSON --settings) each
  shell-quoted; `--input-format stream-json`; `--output-format stream-json`; `--verbose`; `--forward-subagent-text`
  where subagentText; `--include-partial-messages` for an aside; permission flags; `--model '<m>'` (with `[1m]` for
  1m); `--effort '<e>'`; `--name '<name>'`; `--mcp-config '<json>'`; `--mcp-config "$WSP_MCP_VALUES"` where
  serverValues; `--append-system-prompt '<SLATE_BRIEF>'` where launch_has_slate; one `--settings '<json>'` unless the
  person named a settings file; the id flag last.
- Permission flags: no mode or bypassPermissions → `--dangerously-skip-permissions --permission-prompt-tool stdio`;
  any other → `--permission-mode '<mode>' --permission-prompt-tool stdio`. An aside is always `default`.
- Settings object key order: fastMode (only `true`), autoMemoryDirectory, the person's keys, deniedMcpServers
  (`[{serverName}]` per off server, only when non-empty), disableAllHooks (aside). A person's key overrides an earlier
  one in place, and so does a later host key over a person's (an aside's disableAllHooks over theirs). Keys that read
  as array indexes go first, ascending, as a JS object holds them (wave 1's `js_key_order`). An aside drops every
  person `--settings` pair where any of them is a file, the JSON ones too; with JSON ones alone it takes them.
- Id flag: aside `--max-turns 2 --resume <id>`; resume `--resume <id>`; fresh `--session-id <id>` plus
  ` --resume <copyOf> --fork-session` for a copy.
- Prefix: off servers `None` → nothing; `Some([])` → `rm -f <OFF_SERVERS_FILE> 2>/dev/null; `; else
  `{ mkdir -p /etc/claude-code/managed-settings.d && printf '%s' '<json>' > <file>; } 2>/dev/null; `. Then
  `in_folder(cwd, claude)`.
- `mcp_config_flag`: `{"mcpServers":{name:{command,args[,alwaysLoad:true]}}}`, alwaysLoad only on the server named
  MCP_SERVER_NAME and only where launch_has_slate; noSlate never written. Empty or None map: no flag.
- `person_settings`: `--settings X` and `--settings=X` both read; a value that is not a JSON object (missing, a path,
  an array, `null`, a string, a number, a flag) makes `{words: args unchanged, settings: {}, file: true}` at once;
  the last JSON one wins.
- `forwards_subagent_text`: `^(\d+)\.(\d+)\.(\d+)` (anchored: `v2.1.300` is false, `2.1.270-beta` true) compared
  part by part to 2.1.270, each part a JS number (`2.1.0270` and a part past u64 read, so parse as f64, never u32),
  equal counting as taken; None or no match is false.
- `launch_command`: resume → `resume`, else `sessionId = local_id`; title → name; valued → serverValues;
  forwards_subagent_text(version) → subagentText.
- `saved_spend_command` (:441): `wsp_saved=$(ls -1td '<config>/projects'/*/'<id>.jsonl' 2>/dev/null | head -n 1); [ -z "$wsp_saved" ] || tail -c 8388608 "$wsp_saved" | grep -F '"type":"cost-state"' | tail -n 1; `.
- `user_message_line` (:451): `{"type":"user","message":{"role":"user","content":[images..., {"type":"text","text":..}]},"parent_tool_use_id":null,"session_id":..[,"uuid":..]}`;
  each image `{"type":"image","source":{"type":"base64","media_type":..,"data":..}}` in the order given.
- `server_values_file`: `None` for None or empty entries; else `ExecSecret{files: {WSP_MCP_VALUES: {"mcpServers":entries}}, input: false}`.
- `new_session_id`: a fresh lower-case v4 UUID each call.

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **L1.** An aside with a JSON and a file `--settings` drops both: `-p '-x' ... --settings '{"disableAllHooks":true}'`.
- **L2.** An aside's own permission mode is never read: `bad mode` still gives `--permission-mode 'default'`.
- **L3.** `""` is a given value: `got ""` for a model, `--name ''`, `cd '' && `; the window's `[1m]` is on before the
  model's check.
- **L4.** `--settings` followed by `null`, `"abc"`, `5` or a flag reads as a file; `--settings {} -a` folds `{}`.
- **L5.** `build_env` with `api_key: Some("")` sets `ANTHROPIC_API_KEY=""` last and adds no inherited token; a
  `project_dir_name` of `""` is set too. Empty strings are given (`??` in the TS, never `||`).
- **L6.** `forwards_subagent_text`: `2.1.99999999999999999999`, `02.1.270`, `2.1.0270`, `2.1.270.5` true; ` 2.1.270`,
  `2.1`, `2.1.9a`, `2.1.27` false.
- **L7.** Person settings `{"b":1,"2":"x","1":"y"}` with fast and a memory folder fold to
  `{"1":"y","2":"x","fastMode":true,"autoMemoryDirectory":"/m","b":1}`.

## Rules against tests

| Rule | Test |
|---|---|
| env key order and a base value of FIRST_BYTE_ENV in first place | `claude_launch::env_keeps_the_ts_key_order` |
| inherited token beats inherited key; vault drops both | `claude_launch::inherited_sign_in_is_a_fallback_the_vault_replaces` |
| guard text exact | `claude_launch::the_guard_asks_auth_status_with_the_inherited_values_out_of_the_way` |
| refusal order and words | `claude_launch::build_command_refuses_in_the_ts_order` |
| settings key order, person keys win in place | `claude_launch::settings_fold_into_one_flag_in_the_ts_key_order` |
| person settings file and `=` form | `claude_launch::person_settings_reads_both_forms_and_gives_way_to_a_file` |
| version cut at 2.1.270 | `claude_launch::subagent_text_starts_at_2_1_270` |
| user line bytes with images and uuid | `claude_launch::a_user_line_is_byte_for_byte_the_ts_one` |
| the 52 TS cases | `claude_landmines::*` (ported) |
| L1 | `claude_launch::a_side_question_drops_every_settings_pair_where_one_is_a_file` |
| L2 | `claude_launch::a_side_question_never_reads_its_permission_mode` |
| L3 | `claude_launch::empty_values_are_given_values` |
| L4 | `claude_launch::settings_json_that_is_no_object_reads_as_a_file` |
| L5 | `claude_launch::an_empty_vault_key_is_still_the_key` |
| L6 | `claude_launch::subagent_text_reads_each_part_as_a_js_number` |
| L7 | `claude_launch::settings_put_index_keys_first_as_a_js_object_does` |
