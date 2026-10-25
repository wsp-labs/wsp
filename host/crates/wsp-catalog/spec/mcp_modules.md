# mcp_modules (mcp_check, mcp_oauth, mcp_login, mcp_config)

Source: packages/catalog/src/mcp-check.ts (62), mcp-oauth.ts (33), mcp-login.ts (85), mcp-config.ts (32) at
a3b870a6e. Tests: mcp-check.test.ts (4), mcp-login.test.ts (4). Rust: tests/mcp_check.rs (4), tests/mcp_login.rs (4),
tests/oracle_text.rs (the checks and the OAuth store), tests/oracle_catalog.rs (sign-in roads per agent and reach).

## What it does

Each harness's own modules for one MCP server: the line that asks for its sign-in and how its words read, where it
keeps an http server's token, how it signs a server in, and whether a server needs a credential a cloud machine never
holds. An agent entry registers them in its `McpConfig`.

## Behaviour

1. Claude's check: `claude mcp get '<name>'` (shell-quoted); `CLAUDE_NOT_SET_UP` (multi-line `^No MCP server (named
   "|found with name: )`) gives NotSetUp; else the first `^\s*Status:\s*(.*)$` line: "Needs authentication" →
   needs-sign-in, "Failed to connect" → failed, "Connected" → signed-in, else nothing (mcp-check.ts:143-154).
2. Codex's check: `codex mcp list --json` whatever the name; the JSON array from the first line starting `[`; not
   parseable or not an array → nothing; no entry of that name → NotSetUp; `auth_status` `o_auth` → signed-in,
   `not_logged_in` → needs-sign-in, else nothing (mcp-check.ts:159-177).
3. Claude's OAuth store: home `~/.claude`; the bearer is in `<home>/.credentials.json` at `["mcpOAuth",
   "<name>|<sha256 hex first 16>"]`, field `accessToken`, hashing `JSON.stringify({type: "http", url, headers})` with
   the headers in their own order (mcp-oauth.ts:205).
4. Logins: Claude `claude mcp login '<n>'` and pasted `... --no-browser`, not-set-up words as rule 1; Codex `codex
   mcp login '<n>'`, words `^Error: No MCP server named '` (m); OpenCode `opencode mcp auth '<n>'`; Gemini only
   inside, `/mcp auth <n>` unquoted (mcp-login.ts:229-243).
5. `login_road` (mcp-login.ts:290): brokered refuses with `usage_refusal(brokered_server_sign_in_line(name),
   BROKERED_SERVER_SIGN_IN_FIX)`; inside → copy/inside; reach here or relay → pty callback; reach none → pty code
   with the pasted command where one exists, else copy/callback with the command.
6. `needs_own_credential` (mcp-login.ts:277): any envRef not a login name (HOME PATH USER LOGNAME SHELL PWD TMPDIR
   LANG) → true; stdio: a program word (folder and `@ver`, `==ver`, `:tag` cut) of mcp-proxy-for-aws(-cli) without
   `--skip-auth` among the words, or an env value reading `${X}` or `{env:X}` of a non-login name → true; http with no
   headers → `signs_in(url) == Some(true)`; with headers → any header whose value is empty after a leading
   Bearer/Basic/Token word and trimming.
7. `addresses_to_ask`: http servers with no header and no non-login envRef, each url once, first-seen order.
8. `McpConfig::trusts`: `None` where the agent has no trust read; Codex: false with no user file, else
   `codex_trusts`.

## Edge cases

- The Codex check reads the array after warnings printed before it (`warn\n[...]`).
- `{}` alone is not an array: nothing.

## Behaviour nobody wrote down

- Rule 4.26: the OAuth hash depends on the headers' key order exactly as the config wrote them.

## Tapes

servers.signIn and agents.read (#2443).

## Decisions

1. The TS modules are objects with optional members; the port's enums answer `None` where a member is absent.

## After the port

- A header whose whole value is the word `token` (any case, spaces around it) reads as an empty credential:
  the scheme-word strip at mcp-login.ts:281 takes the value itself (probed: `{"X-Key": "  token  "}` needs one).

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2 | mcp_check: the_harness_s_own_word_on_one_server_s_sign_in::* (4); oracle_text: the_harness_checks_read_each_output_as_the_ts_modules_do |
| 3 | oracle_text: the_oauth_store_hashes_type_url_and_headers_in_that_order_and_keys_the_first_16_hex |
| 4, 5 | mcp_login: how_one_mcp_server_of_an_agent_is_signed_in::* (3); oracle_catalog: a_servers_sign_in_road_is_the_ts_one_for_each_agent_and_reach, a_brokered_machine_refuses_every_servers_sign_in_as_a_usage_refusal |
| 6, 7 | oracle_text: a_server_needs_its_own_credential_exactly_where_the_ts_says_so (18 servers and the addresses to ask) |
| 8 | mcp_launch: a_codex_thread_starts_config::reads_the_projects_file_only_where_codexs_own_marks_the_folder_trusted |
