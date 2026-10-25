# codex_command
Source: packages/adapter-codex/src/command.ts (337 lines) at a3b870a6e.
Tests: packages/adapter-codex/test/command.test.ts (18 cases), placeholder-login.test.ts (29 cases; 3 drive the adapter and move to
codex_adapter, 1 reads every unit's lines and moves to codex_conversations). Rust: `src/codex/command.rs`; tests `tests/codex_command.rs`.

## What it does
Builds the one bash line that runs `codex app-server` for a turn, a side question or a side run, and the environment it runs under.
On a machine whose credentials the host keeps (a broker), the line first writes a placeholder login into `$CODEX_HOME/auth.json`
carrying the turn token read off the environment where the line runs, points codex's base URLs at the door on the machine's loopback,
and rewrites the person's own `model_providers` entries to the door. Also the launch words every other codex command (probe, title,
draft, rename, conversation read, a terminal's `codex`) is started with.

## Behaviour
1. `slug(name, v)` passes `^[A-Za-z0-9][A-Za-z0-9._:-]*$` or refuses with `<name> must be a plain slug, got "<v>"` (command.ts:257).
2. `build_env` (command.ts:66): the home is `js_trim`med and must start with `/`, else `home must be an absolute path, got "<home as given>"`.
   The base drops entries whose value is absent, keeps order, then sets `CODEX_HOME`, then the key variables. A base that already
   holds `CODEX_HOME` keeps its position with the new value (JS spread; `IndexMap::insert` does the same).
3. Keys with no broker (command.ts:62): an API key goes under `CODEX_API_KEY` then `OPENAI_API_KEY`, none where no key.
4. Keys under a broker (command.ts:52): ChatGPT mode with a key refuses `a turn on the host's ChatGPT login carries no key; its login is
   a placeholder`. API-key mode with no key sets nothing; a key not matching `^[A-Za-z0-9._~+/=-]+$` refuses `a turn on the host's API key
   needs the placeholder key the host shaped for it, one plain word`; else `WSP_CODEX_KEY=<key>`. `brokered_codex_env` adds
   `CODEX_REFRESH_TOKEN_URL_OVERRIDE=http://127.0.0.1:7071/oauth/token` and `CODEX_REVOKE_TOKEN_URL_OVERRIDE=http://127.0.0.1:7071/oauth/revoke`.
5. `config(k, v)` is `-c k=` + shell-quoted JSON string of v; `config_raw(k, toml)` is `-c k=` + shell-quoted toml (command.ts:263-266).
6. `build_command` (command.ts:326): `inFolder(cwd, [...steps, server].join(" && "))`, server being `agentWord("codex", launch) app-server`,
   the launch words, `-c tools.update_plan.enabled='true'`, the broker's flags, the MCP servers' flags, the off servers' flags, joined
   by one space. Steps and flags: no broker gives none; `side_run: true` (the TS `turn: false`) gives the no-login line; else the turn's brokered line at `now`.
   With no cwd the line starts `cd ~ && `.
7. Placeholder login (command.ts:103), exact text: the body is `umask 077 && mkdir -p "$CODEX_HOME"` then the write steps, joined by
   ` && `. A turn wraps it as `( : "${WSP_HOST_TOKEN:?a turn whose credentials the host keeps was launched without its token}"<guard> && <body> )`,
   the guard being ` && : "${WSP_CODEX_KEY:?a turn on the key the host keeps was launched without its placeholder key}"` in API-key mode.
   Any other launch writes `{ [ -z "${<needs>:-}" ] || ( <body> ); }`, needs being `WSP_CODEX_KEY` (API key) or `WSP_HOST_TOKEN`.
8. API-key login file: `{"auth_mode":"apikey","OPENAI_API_KEY":<mark>}`. ChatGPT: payload `{"exp":floor(now/1000)+31536000,
   "https://api.openai.com/auth":{"chatgpt_account_id":..,["chatgpt_plan_type"],["chatgpt_user_id"],"chatgpt_account_is_fedramp":fedramp===true},
   "https://wsp.dev/turn":<turn mark>}`; jwt `<base64url({"alg":"none","typ":"JWT"})>.<$jwt>.wsp`; file `{"auth_mode":"chatgpt",
   "OPENAI_API_KEY":null,"tokens":{"id_token":jwt,"access_token":jwt,"refresh_token":"rt_wsp_"+turn mark,"account_id":..},
   "last_refresh":<ISO 8601 of now, milliseconds, Z>}`. Marks `@@wsp-turn@@`, `@@wsp-jwt@@`, `@@wsp-key@@` become `"$WSP_HOST_TOKEN"`,
   `"$jwt"`, `"$WSP_CODEX_KEY"`; every other run between marks is shell-quoted, an empty run stays empty (command.ts:88).
9. Brokered line (command.ts:176): steps are the login, `unset WSP_CODEX_KEY` in API-key mode, and
   `export WSP_CODEX_PROVIDER_KEY="sk-wsp-$WSP_HOST_TOKEN"` where any provider is keyed; flags are the door flags with store `file`.
10. Door flags (command.ts:184), in order: `openai_base_url` (`<models>/backend-api/codex` for ChatGPT, `<models>/v1` for a key),
    `chatgpt_base_url="http://localhost:7071/backend-api/"` always (the second origin), `cli_auth_credentials_store`, `analytics.enabled='false'`,
    then each provider's flags. No-login line (command.ts:223): models `http://127.0.0.1:0`, store `ephemeral`, plus `features.plugins='false'`
    last; its only step is `unset WSP_CODEX_KEY` in API-key mode.
11. `provider_flags(id, entry, mode, models)` (command.ts:140): id must match `^[A-Za-z0-9_-]+$` else `a model provider's id must be one plain
    word of a config key, got "<id>"`. A present `aws` key gives no flags and keyed false. Keyed is: not `requires_openai_auth === true`,
    no `auth` key, no `experimental_bearer_token` key. Flags: `base_url` (`<models>/backend-api/codex` only for an OpenAI-auth entry on
    ChatGPT, else `<models>/v1`); `name="azure"` where the lowercased name is not `azure` and the lowercased `base_url` holds one of
    `openai.azure.`, `cognitiveservices.azure.`, `aoai.azure.`, `azure-api.`, `azurefd.`, `windows.net/openai`; then the credential: an
    auth command becomes `auth.command="sh"` and `auth.args=["-c","printf %s \"sk-wsp-$WSP_HOST_TOKEN\""]`; a bearer token becomes
    `-c model_providers.<id>.experimental_bearer_token='"sk-wsp-'"$WSP_HOST_TOKEN"'"'`; a keyed entry gets `env_key="WSP_CODEX_PROVIDER_KEY"`;
    then every `http_headers` key blanked as `{"K"=""}` joined by commas.
12. MCP servers (command.ts:272), in the record's key order: a name failing `^[A-Za-z0-9_-]+$` refuses `an MCP server name must be one
    plain word of a config key, got "<name>"`. Each entry is `command` then `args` (JSON); the servers named `wsp` and `wsp_slate` also
    get `env_vars` = `["WSP_HOST_URL","WSP_HOST_TOKEN","WSP_HOST_KEY","WSP_TURN","WSP_THREAD"]` and `tool_timeout_sec='3600'`. The `wsp`
    server of a launch with a slate (`wsp` present and not `noSlate`) is followed by `wsp.disabled_tools` = the four slate tools, the
    whole `wsp_slate` entry on the same spec, `wsp_slate.enabled_tools`, `wsp_slate.omit_tools_from='["deferred"]'`, and one
    `wsp_slate.tools.<tool>.approval_mode="approve"` per slate tool (`slate_catalog`, `slate_write`, `slate_state`, `slate_read`).
13. Off servers (command.ts:306): a name failing the server-name pattern is dropped silently; each other is
    `$(grep -Eqs '^[[:space:]]*(\[mcp_servers\.<n>[].]|mcp_servers\.<n>\.)' "$CODEX_HOME/config.toml" && printf %s '-c mcp_servers.<n>.enabled=false')`.
14. `access_params(None | "danger-full-access")` is `danger-full-access`/`never`; `read-only` and `workspace-write` keep their sandbox with
    `on-request`; anything else refuses `permissionMode must be one of read-only, workspace-write, danger-full-access, got "<mode>"`.
15. `codex_launch` (command.ts:198) is the program word alone with no broker; else `sh -c '<steps> && exec "$0" "$@"' <program>` with the
    door flags kept apart, for a caller to place after its subcommand. `codex_word` (command.ts:210) is the program alone with no broker,
    else the no-login word (sh -c only when it has steps) followed by its flags, space-joined.
16. `codex_for_shell` (command.ts:231) is the script text: `#!/bin/sh`, the PATH filter dropping `folder`, the CODEX_HOME default
    `${CODEX_HOME:-$HOME/.codex}`, the exec/e `--skip-git-repo-check` insertion (never twice), then `exec <launch word and flags> "$@"`,
    lines joined by `\n` with a trailing newline.
17. `image_path` refuses a path not starting with `/` or holding `\n`: `an image path must be one absolute path on the machine, got "<p>"`.

## Edge cases
- A record's integer-like keys (`"123"`) come first in JS `Object.entries`, ascending (rule 4.26); providers and servers keep that order.
- `aws: null` and `auth: null` count as present (TS checks `!== undefined`). `http_headers` that is an array blanks its indices.
- `build_env({ home: " /x " })` sets `CODEX_HOME=/x`.

## Behaviour nobody wrote down
- Rule 8.6: the home trim is `js_trim`. Rule 4.26: env and record order. `Date.now()` here is the `now_ms` argument (rule 1.4).

## Decisions
1. `now` is passed in (`now_ms: i64`) wherever the TS reads `Date.now()`, so a test pins it as the TS tests do with `vi.spyOn(Date, "now")`.
2. The `last_refresh` stamp is `new Date(now).toISOString()`: `YYYY-MM-DDTHH:MM:SS.mmmZ`, UTC, always three fraction digits.

## After the port
- An `http_headers` array is blanked by index; a person's config never holds one, but the TS would write `{"0"=""}`.

## Tests (tests/codex_command.rs, 42; TS 18 + 29 less 5 moved)
- Rules 1, 17: through the store and title files (`refuse_an_id_that_is_not_a_plain_slug_before_anything_runs`); images in
  `codex_turn.rs` `images_ride_the_turn_as_local_paths_and_one_with_no_path_on_the_machine_is_refus`.
- Rules 2-4: `build_env_cases::*` (3), `a_brokered_codex_launch_on_an_api_key::refuses_a_key_beside_the_host_s_chatgpt_login_and_a_key_turn_handed_no_placehold`.
- Rules 5, 6, 12, 13: `build_command_cases::*` (9; whole-entry servers, slate server and its four approval modes, no-slate thread, bad
  server name, off servers run in bash against a config, quoted folder), `a_person_s_setup_for_codex_on_a_computer::runs_the_program_in_place_of_codex_on_the_server_line_the_launch_words_after_app`.
- Rules 7-10 (placeholder login, run in a real bash): `a_brokered_codex_launch_on_a_chatgpt_login::*` (5: auth.json fields, exp,
  last_refresh, mode 0600, door flags, no file without a token), `a_brokered_codex_launch_on_an_api_key::*` (2),
  `every_other_codex_launch_on_a_brokered_machine::*` (6, no-login line: store `ephemeral`, plugins off, models at port 0).
- Rule 11: `the_person_s_own_providers_on_a_brokered_launch::*` (6: credential fields, blanked headers, Azure name, OpenAI auth, AWS, bad id).
- Rule 14: `access_params_cases::*` (4).
- Rules 15, 16: `every_other_codex_launch_on_a_brokered_machine::a_title_hands_codex_the_broker_s_overrides_after_exec_beside_its_own`,
  `codex_typed_in_a_terminal_on_a_brokered_machine::*` (5: PATH filter, `--skip-git-repo-check` once, key file, CODEX_HOME default, no token).
