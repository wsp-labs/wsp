# logins
Source: `logins.ts` (183 lines) at a3b870a6e. Tests: `test/logins.test.ts` (14 cases, two `it.each` of 12 and 2 rows).
Rust: `tests/detect_logins.rs`. Reads the catalog's `login_rows`, `login_row`, `SIGN_IN_ROWS`, `signs_in_by_default`,
`mints_token`, `lives_on_computer`, `CLAUDE_SETTINGS_FILE`: the row tests stay red until wsp-catalog is implemented.

## What it does
The logins rung by presence: one row per CLI or agent login whose files are here, a row per catalog key file found,
the 1Password CLI as a locked row, and Claude Code's row when it bills here. No login file is read except Claude
Code's `.credentials.json` (for whether it holds a login), the rc files and Claude's settings.json (for names).

## Behaviour
1. `login_default(id)`: `skip` when `login_row(id)` exists and its sign-in `signs_in_by_default` or `mints_token`;
   else `bring`, an unknown id included (logins.ts:28-31).
2. `detect_logins` row order (logins.ts:155-183): gh; then the LOGINS table in its order (gcloud, wrangler,
   cloudflared, vercel, aws, kube, codex, gemini, opencode, pi, hermes); then key-file rows in `login_rows()` order;
   then `logins/op`; then Claude. Absent rows dropped.
3. gh (logins.ts:70-75): `found(["~/.config/gh/hosts.yml"])`; none, no row. On darwin, `security
   ["find-generic-password","-s","gh:github.com"]` answering anything adds the path `Keychain: gh:github.com` after
   the file (an answer of `""` counts: test `Option::is_some`, never emptiness; the Keychain is asked only when the
   file was found); never on linux (no run). Label GITHUB_LOGIN_NAME, group `CLI logins`, default `login_default("gh")`.
4. A LOGINS row's candidates are `paths[platform]` then `paths.all`, except codex with `codex_home` set, whose one
   candidate is `<codex_home>/auth.json` (logins.ts:159). Found paths keep candidate order. A login whose catalog
   sign-in `lives_on_computer` gets `paths: []`, `bytes: 0`, `skip` and detail LIVES_ON_COMPUTER_DETAIL; else the found
   paths and bytes, default `login_default(id)`, and no detail (no table row sets `default` or `detail`).
5. Key-file rows (logins.ts:52-61): each login row with `keys`, its `keys.paths` found; label `<entry.name> API keys`,
   group `Agent logins` for an agent entry else `CLI logins`, default `login_default(row.id)`, detail `keys.note`.
6. `which("op")` true adds an item `logins/op`, label `1Password CLI`, group `CLI logins`, skip, reason
   `needs the 1Password desktop app; the machine uses a service account token` (logins.ts:178-180).
7. `claude_bills` (logins.ts:114-119): `api_key_helper_of(read_text(expand(home, CLAUDE_SETTINGS_FILE)))`; a helper or
   CLAUDE_KEY_ENV exported in an rc file is `key`. Else on darwin, the Keychain answers for
   `claude_keychain_service(claude_config_dir)`; on linux, `holds_claude_login(read_text(expand(home,
   "<claude_config_dir or ~/.claude>/.credentials.json")))`. Either true is `login`, else `None`. The settings file is always `~/.claude/settings.json`, whatever
   `claude_config_dir` says; `claude_config_dir` of `Some("")` is not unset (linux reads `/.credentials.json`, darwin
   hashes `""`), and `~/` in it is expanded for the read but hashed raw for the service name. A Keychain answer of
   `""` is a login. Reads and runs happen in this order and stop at the first answer: settings, then the rc files of
   rule 8, then the Keychain (darwin) or the credentials file (linux).
8. Exported means: for each RC_PATHS entry in order, `read_text("<home>/<rel>")` and `strip_exports(text, None, &mut
   vec![], false).names` holding the name; the first such file answers (logins.ts:97-103). Reads stop at that file.
9. `claude_keychain_service(None)` is `Claude Code-credentials`; `Some(dir)` adds `-` and the first 8 hex digits of
   sha256 of `dir` in Unicode NFC (logins.ts:81-82).
10. `api_key_helper_of`: JSON object whose `apiKeyHelper` is a string not blank after `js_trim`; the string is returned
    untrimmed. `None`, bad JSON, `null`, an array or a non-string helper are `None` (logins.ts:85-94).
11. `holds_claude_login`: JSON whose `claudeAiOauth.accessToken` is a non-empty string (logins.ts:123-131); `null`
    parses to no login.
12. `billed_here` is `{claude: bills}` or empty (logins.ts:134-137).
13. Claude's row when `claude_bills` is not `None`: entry `logins/claude`, label `Claude Code login`, group
    `Agent logins`, `paths: []`, `bytes: 0`, skip, detail `claude_token_detail()` (logins.ts:141-153), which is
    `Claude Code signs in with the token <SIGN_IN_ROWS.claude.mint> prints on this computer; nothing of its login here
    travels`.
14. No run carries `-w`, and no run has a timeout or env.

## Edge cases
- An empty host gives no rows and runs nothing (op is asked by `which`, not `run`).
- On linux the Claude branch runs nothing; on darwin with a helper set it asks no Keychain (the helper wins first).

## Decisions
1. `CLAUDE_TOKEN_DETAIL` is a TS constant built from the catalog at load; Rust has `claude_token_detail()`.

## After the port
- `exported_in` reads every rc file in full through `strip_exports` to learn one name, which costs the slowest
  reader on the rung for a yes or no.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/logins.json` (`oracle/w2-logins.oracle.ts`), replayed by `tests/detect_logins.rs`
`w2_oracle::*`.

- W1. A row with a platform path and an `all` path lists the platform's first, then `all`; the other platform's path
  is never stat'ed (`detect_logins`: vercel cases).
- W2. gh: the Keychain is asked only when hosts.yml was found; an answer of `""` counts; a hosts.yml that is a folder
  counts (`detect_logins`: gh cases).
- W3. Claude presence is by platform only: darwin asks the Keychain and never reads the credentials file; linux reads
  the file and never runs `security`; a Keychain answer of `""` is a login (`detect_logins`, `claude_bills`).
- W4. `claude_bills` reads and runs in order and stops at the first answer: settings (always `~/.claude/settings.json`),
  then each RC_PATHS file until one exports the key, then the Keychain or the credentials file. With a helper, nothing
  after the settings read happens (`claude_bills` calls).
- W5. `claude_config_dir`: `~/` expanded for the read, hashed raw for the service; `Some("")` is not unset; settings
  under the dir are never read (`claude_bills`, `claude_keychain_service`). `codex_home`: `~/` expanded, `Some("")` reads
  `/auth.json`, the default path is not read when it is set (`detect_logins`: codex cases).
- W6. `holds_claude_login` and `api_key_helper_of` over every JSON shape: a blank token counts, a blank helper (after
  `js_trim`, U+FEFF and U+00A0 included) does not, the helper is returned untrimmed, a duplicate key keeps the last,
  text with a BOM or trailing junk is no JSON, `None` input is `false`/`None` (`holds_claude_login`, `api_key_helper_of`).
- W7. What counts as exported is `strip_exports`'s `names` (shell_rc_strip): `export K=`, `export A=1 K=x B=2` and
  `K=x claude` count; a comment, a bare `export K`, `K_2` do not; fish `set -gx` counts only in `config.fish`
  (`detect_logins`: export cases).
- W8. The full table on each platform with every candidate present, `op` on PATH: rows, labels, groups, defaults and
  details in order (`detect_logins`: "every candidate file on ..."). `op` is listed on linux too and runs nothing.

## Tests
| Rule | Test |
|---|---|
| 1 | `every_login_rows_default_follows_its_catalog_entrys_sign_in_kind` |
| 2, 4 | `presence_by_path_a_browser_or_device_sign_in_starts_as_a_sign_in_on_the_machine` (12 rows), `codex_is_found_by_its_file_and_offers_nothing_of_it`, `finds_claude_codes_and_codexs_logins_where_claude_config_dir_and_codex_home_put_them`, `rows_come_in_table_order_gh_first_claude_last` |
| 3 | `gh_with_its_token_in_the_macos_keychain_carries_that_item_as_a_second_path`, `is_labelled_by_name_any_sentence_about_it_is_a_detail` (2 rows), `gh_on_linux_asks_no_keychain` |
| 5 | `hermes_agent_is_two_rows_its_device_login_and_the_keys_file_beside_it` |
| 6 | `onepassword_cli_is_listed_locked_off` |
| 7, 8, 13 | `claude_codes_login_is_found_here_and_nothing_of_it_is_offered_to_copy`, `on_linux_claude_code_is_signed_in_only_where_its_credentials_file_holds_its_own_login`, `claude_code_with_every_key_source_here_still_travels_nothing`, `an_api_key_helper_or_an_exported_key_here_is_a_claude_row_with_nothing_to_copy` |
| 9 | `claude_keychain_service_names_the_config_folder_by_its_nfc_sha256` |
| 10, 11 | `api_key_helper_of_reads_a_non_blank_string_helper_only`, `holds_claude_login_wants_a_non_empty_access_token` |
| 12 | `billed_here_names_claude_only_when_it_bills_here` |
| 14 | `never_reads_a_login_file_and_never_asks_the_keychain_for_a_secret`, `no_logins_on_an_empty_laptop` |
