# mcp
Source: `mcp.ts:292-464` (of 464 lines) at a3b870a6e. Tests: 14 of `test/mcp.test.ts`'s 16 cases. Rust:
`tests/detect_mcp.rs`. Reads the catalog's MCP agents and their format modules through `read_turn_servers` (the other
half of collect): the row tests stay red until the catalog and that function are implemented.

## What it does
One row per MCP server under the agent that runs it, read from the agent's own config file, saying how it fits
Linux, what it carries and how it signs in; then one row for mcp-remote's sign-in store when there is one.

## Behaviour
1. `detect_mcp(host, agents)` (mcp.ts:446-463): first the store (rule 6). Then for each agent in order (`None` is
   `mcp_agents()`): `read_turn_servers(host, agent, home, TurnRead { own: true, .. })`; its `user` turns first, then its
   `local` turns, each in their order (a `project` turn is left out); each server's scope becomes `home` for a local
   turn, else `user`. A stdio server whose `mcp_remote_hash` is a key of the store's tokens adds its name to `matched`.
   Each server gives `mcp_server_row`. Last, the store row when a store exists.
2. `mcp_server_row` (mcp.ts:436-439): what it carries (rule 3), then its home dependencies (rule 4) against the
   carried paths, then the row (rule 5). `linux_fit` is pure.
3. Carried (mcp.ts:315-361). `named` starts as the server's `env_refs`. http: each header in order adds the secret
   `header <name> (<utf8 bytes> B)` and key `<name>`; the agent's `mcp.http_auth`, when set, is a note; sign-in is
   `key` or `token` by rule 7 over the header names then `named` when either is non-empty, else `oauth` with an
   http_auth, else `none`. stdio: each env entry in order whose name is `secret_named` joins `named`; its value
   `~/`-expanded, stat'ed only when absolute: a file outside `<home>/` adds the note `the file <name> points at is
   outside your home and is not copied` and no secret; a file under home adds the secret
   `the file <name> points at (<fmt_bytes(bytes)>)`, the key, the path `~<rest>` and its bytes; anything else (no
   stat, a dir, a relative value) adds `env <name> (<utf8 bytes of the value> B)` and the key. Then every `read_args`
   entry with a secret adds it and its key. A definition running mcp-remote adds the note
   `no saved sign-in; the browser sign-in runs again on the machine` when its hash has no token, else
   `its saved sign-in (<fmt_bytes(bytes)>) travels on the mcp-remote sign-ins row`. Sign-in: `oauth` with mcp-remote,
   else rule 7 over `named` when non-empty, else `token` when any argument was secret, else `none`.
4. Home dependencies (mcp.ts:301-311): each `home_paths` entry not among the carried paths is stat'ed; missing sets
   `gone` and notes `<p> is not on this computer; unticked, tick it if the server creates it on first start`; present
   notes `depends on <p>, which comes along only if a row carries it`.
5. The row (mcp.ts:365-390): id `agents/mcp/<agent id>/<"home/" for scope home><name>`, label the name, group
   `<agent name> MCP servers`, paths and bytes as carried, default bring unless linux_fit is `No` (then skip with its
   reason) or a dependency is gone (skip, no reason). With any secret: `consent: true` and `keys` once each in first
   order. `sign_in` as carried. `detail` joins with `; `: `local to ~` for scope home; `http: <shown url>` or
   `stdio: <stdio_line>`; when the fit is `Ok`, its needs then the dependency notes; `reads <ref> from the environment,
   set it on the machine` per env ref; `carries a secret: <list>` (one) or `carries secrets: <list joined by ", ">`,
   or with no secret and no note `carries no secret`; then the notes.
6. The store (mcp.ts:402-417): none unless `stat(expand(~/.mcp-auth))` is a dir. Excludes first list each
   `~/.mcp-auth` name matching `^mcp-remote-\d` as `~/.mcp-auth/<name>`, in listing order. Then each name listed in
   `~/.mcp-auth/mcp-remote-v1`: a `^[0-9a-f]{32}_lock\.json$` name is excluded; every other is stat'ed (`None` is 0
   bytes) and adds to the store's bytes; a `^([0-9a-f]{32})_tokens\.json$` name records its bytes under its hash.
7. Key or token: `key` when any name matches `/key/i`, else `token` (mcp.ts:276).
8. The store row (mcp.ts:419-433): rung agents, id MCP_REMOTE_ID, label MCP_REMOTE_LABEL, group `MCP sign-ins`, paths
   `["~/.mcp-auth"]`, excludes when any, bytes the store's. No token: skip, reason
   `no saved sign-in the current bridge reads` plus `; older versions' folders are left here` when an exclude is
   outside mcp-remote-v1. Else bring, `consent: true`, detail `browser sign-ins saved by mcp-remote for remote
   servers: <n> token<s> (<fmt_bytes(sum)>)`, then `, ` and the parts joined by `; ` when any (`for <matched joined by
   ", ">` when matched is non-empty; `<k> for server<s> configured elsewhere (a repo's .mcp.json)` when
   `k = n - matched.len()` is above 0), then `; older bridge versions' folders stay here` when older.
9. `mcp_remote_token_file(host, hash, env)` (mcp.ts:116-117): `<MCP_REMOTE_CONFIG_DIR>/mcp-remote-v1/<hash>_tokens.json`
   when that variable is set and non-empty, else `expand(~/.mcp-auth/mcp-remote-v1)/<hash>_tokens.json`.
10. `mcp_remote_tokens` is the store's tokens, empty with no store (mcp.ts:442).

## Edge cases
- No agent config: no server rows; a store alone still gives its row.
- Two servers with the same mcp-remote url both join `matched`, so `k` can be 0 or below and says nothing.
- Token files are only listed and stat'ed: no `read` of anything under `~/.mcp-auth`.

## Decisions
1. `agents` takes catalog entries (`&AgentEntry` with `mcp` set), TS `McpAgent`; an entry with `mcp: None` gives no
   servers.

## After the port
- A secret-named env value pointing at a file outside home is noted but counted as no secret, so the row can travel
  on a bare tick while naming a credential file.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/mcp.json` (`oracle/w2-mcp.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_mcp.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 3: an env value's byte count is of the value as written, not the expanded path (`~/missing` is `(9 B)`).
- C2. Rule 3 (sign-in): `named` holds only env refs and secret-named env keys, never a secret flag's key, so a stdio server whose only secret is `--api-key v` is `token`, not `key`.
- C3. Edge: two servers with the same mcp-remote url print the name twice in the detail (`for a, a`), across agents too.
- C4. Rule 5: `reads <ref> from the environment` is per `server.env_refs` only; the catalog's `reads` field is never read here.
- C5. Rows are never merged: one name under two scopes or two agents is two rows, in agent order, user scope before local; the `disabled` flag is ignored by this rung.

- W1. 31 computers through `detect_mcp` with every read (`detect_mcp`); token files, token sizes and single rows (`mcp_remote_token_file`, `mcp_remote_tokens`, `mcp_server_row`).

## Tests
| Rule | Test |
|---|---|
| 1, 5, 6, 8 | `claude_code_one_row_per_user_scope_server_and_per_server_local_to_the_home`, `a_server_local_to_another_project_stays_behind_its_repo_is_not_on_the_machine`, `an_mcp_remote_server_without_a_saved_sign_in_says_the_browser_sign_in_runs_again` |
| 1 | `codex_mcp_servers_tables_in_config_toml_env_sub_tables_included_with_the_bearer`, `gemini_cli_and_opencode_mcp_servers_and_mcp_comments_and_trailing_commas_allowed`, `an_agent_the_catalog_gains_with_a_format_of_its_own_is_read_through_its_module`, `a_config_that_does_not_parse_or_has_no_servers_adds_no_row` |
| 3, 7 | `a_value_after_a_secret_named_flag_or_inside_a_secret_named_assignment_is_a`, `an_argument_the_definition_does_not_name_is_a_value_hidden_and_counted_so_the`, `a_positional_address_a_path_and_a_name_value_whose_name_is_no_secrets_are_the`, `mcp_remotes_static_oauth_client_info_blob_is_a_secret_whatever_its_name_says_a`, `says_how_each_server_signs_in_nothing_a_key_or_a_token_it_carries_or_a_browser`, `an_http_servers_sign_in_note_is_what_the_agents_mcp_module_says_an_entry_that` |
| 3, 4 | `a_secret_named_env_value_pointing_outside_home_is_named_without_its_value_a_home` |
| 9 | `mcp_remote_token_file_follows_mcp_remote_config_dir_unless_it_is_empty` |
| 10 | `mcp_remote_tokens_is_empty_without_a_store_and_never_reads_a_token` |
