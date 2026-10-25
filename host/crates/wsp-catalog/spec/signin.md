# signin (with signin_rows)

Source: packages/catalog/src/signin.ts (594) at a3b870a6e, split: signin.rs for the shapes, the readers and the rules
(signin.ts:1-342), signin_rows.rs for the twenty rows and every list derived from them (signin.ts:343-594). Tests:
signin-state.test.ts (14) and the sign-in cases of catalog.test.ts. Rust: tests/signin_state.rs (14),
tests/oracle_signin.rs (9: every row, every reader on 28 outputs, every derived list), tests/catalog.rs.

## What it does

How each catalog entry is signed in on a machine or minted here: the command, the questions it asks and what settles
each, how it finishes with no browser, its status command and how that command's output is read, where the login
lands, and the vault's variables. Every other module reads a row only through these rules.

## Behaviour

1. `SignIn` is a token row (claude), a login row (oauth, device or key) or a none row (op, kubectl); each serializes
   with its own `kind`.
2. Status readers (signin.ts:256-536): `ok(re)` is exit 0 and the regex matching; `has(re)` the regex alone; gh is
   "Logged in to" and not "Failed to log in" whatever the exit; vercel exit 0 and not "No existing credentials";
   netlify exit 0 and not /Not logged in/i; kubectl's detail is `context <trimmed output>`.
3. Claude's status (signin.ts:290-336): the JSON is the text from the first `{` to the last `}`; signed in only where
   `loggedIn === true`; detail: `ANTHROPIC_API_KEY` as `apiKeySource` names the key's source first, else signed in
   with `authMethod` `oauth_token` is TOKEN_SOURCE and `claude.ai` "OAuth credentials"; kind: api-key, subscription
   (oauth_token, or claude.ai with a plan), oauth (claude.ai with none); plan: a non-empty string `subscriptionType`
   where no `apiKeySource`.
4. Codex's kind: "Logged in using ChatGPT" subscription, "Logged in using an API key" api-key. Gemini's: the last
   whitespace word of the trimmed output: `oauth_creds.json` ("OAuth credentials", oauth) or a key name (its source,
   api-key).
5. `secret_named`: the first held name, in the map's order, that is a whole word (`[^A-Za-z0-9_]+` splits) of the
   output. A key's source is "API key from <file>, set on the machine as a secret" where the secrets step set it,
   else "API key from <name> on the machine".
6. `login_there`: a login row itself; a token row's `there` as an oauth login with `sources: []`,
   `stateOnMachine: []` and the row's status; else nothing (signin.ts:146).
7. `sign_in_road_of` (signin.ts:199): terminal where a question is the person's; token, key, none or device by kind;
   else the row's headless, else code.
8. `login_words`: the login up to its first ` --`. `token_in`: the trimmed paste where it is wholly the token shape.
   `login_id_of` maps kubectl to kube; `keys_id_of` adds `-keys`; `keys_row_of` is a none row with sources
   `["file"]`, the keys' note and the login's status.
9. Derived lists (signin.ts:539-594): SHARED_LOGINS (codex's alone); `shares_in(logins)`; NEVER_IN_IMAGE (Claude's
   credentials file and key file under the guest config dir, then each shared target); LOGIN_PATHS (that, each row's
   state paths, then the two secrets files, deduplicated in first-seen order); NEVER_ON_BROKERED (that plus
   `/root/.mcp-auth` and `/root/.git-credentials`); VAULT_VARIABLES (each row's token variable then key variable, in
   row order); `server_values_of` drops vault variables and adds bearer names ahead; `vault_variable_row` is the
   first row declaring the name.

## Edge cases

- Claude's status with a `}` after the object (a warning line) fails the parse: not signed in, no detail.
- doppler's code regex uses a lookbehind (`(?<=Your auth code is:\s*)`), rule 8.7.

## Behaviour nobody wrote down

- Rule 4.26: SIGN_IN_ROWS keeps the TS object's order; every derived list follows it.
- `StatusCheck::why` is always `None`: no row has one at a3b870a6e, and the host reads it only to word a refusal.

## Tapes

agents.read, agents.signIn, agents.signIns and the cloud sign-in ops (#2443).

## Decisions

1. Each status closure is a `StatusReader` named by its row; the oracle writes a status as `{command, typed?,
   reader}`.
2. A question's settle serializes as the TS key beside `asks`: `flag`, `answer` or `person`.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1 | oracle_signin: every_row_is_the_ts_row_in_the_ts_order; catalog: catalog::proves_every_browser_or_device_sign_in_with_a_status_check |
| 2, 3, 4 | oracle_signin: every_status_reader_reads_every_output_as_the_ts_reader_does, claudes_status_json_is_read_from_the_first_brace_to_the_last; catalog: catalog::answers_each_agent_s_sign_in_kind_off_its_own_status_where_the_status_names_one, ::names_claude_code_s_plan_off_its_own_status_where_the_account_names_one_and_none |
| 5 | oracle_signin: the_secret_a_status_names_and_geminis_source_are_the_ts_words |
| 6, 7, 8 | oracle_signin: every_rows_readers_answer_as_the_ts_readers_do, login_and_keys_ids_and_the_keys_row_are_the_ts_ones, a_paste_is_the_token_only_where_the_whole_trimmed_paste_is_its_shape; signin_state: the_token_sign_in_and_the_vault::claude_s_row_mints_a_token_on_this_computer_and_nothing_of_it_travels, ::the_token_in_a_paste_is_the_whole_of_it_and_nothing_around_it_trimmed, ::the_key_variable_a_row_reads_is_written_once_and_a_row_that_reads_none_says_so; mcp_login: the_sign_in_road_a_report_names::is_read_off_the_row_s_declared_headless_flow_never_off_the_words_of_its_command |
| 9 | oracle_signin: the_shared_logins_and_every_path_list_are_the_ts_lists_in_order, the_vault_keeps_row_variables_out_of_server_values_and_names_their_rows; signin_state: sign_in_state_on_the_machine::* (3), a_login_signed_in_once_on_the_computer_that_runs_the_workspaces::* (2), the_token_sign_in_and_the_vault::the_vault_holds_the_agents_token_and_key_variables_and_nothing_else, ::the_paths_a_builder_may_never_hold_are_absolute_guest_paths_the_claude_credentia, ::the_login_paths_a_provider_s_nap_and_snapshot_read_are_every_row_s_state_every_p, ::what_a_provider_s_machine_is_never_sealed_holding_is_every_login_path_claude_s_o |
