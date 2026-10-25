# plugins_codex (plugins-codex, codex-app-server)

Source: packages/catalog/src/plugins-codex.ts (144) and codex-app-server.ts (97) at a3b870a6e. Tests: the Codex half of
plugins.test.ts (5) and the app-server cases of catalog.test.ts. Rust: tests/plugins.rs (codexs_plugins_through_its_app_server,
5, and the whole read), tests/oracle_text.rs (codex_app_server, 2).

## What it does

Codex's plugins asked of `codex app-server` in one bash script per round: `plugin/list` for the rows, `config/read` for
what config.toml names and the version a write checks, `plugin/read` for what each brings; the switch is
`config/value/write` on `plugins.<id>.enabled`.

## Behaviour

1. `request` is `{"jsonrpc":"2.0","id":<id>,"method":<m>,"params":<p>}` written by `JSON.stringify`; `notification`
   has no id and `{}` params; `initialize_request` names the client `wsp` version `0` (codex-app-server.ts:450-454).
2. `app_server_script` (codex-app-server.ts:463): two fifos, the server in the background, stage by stage: each
   stage's lines quoted and written, then lines read with a 10 s wait until the answers so far reach the running
   total; a later stage only where every earlier answer came; then the pipes closed, the recorded pid killed, the
   folder removed. Byte for byte as the TS.
3. `answers_of`: lines starting `{` (after trimming) that parse with a numeric `id`; a result object, or an error
   object with its numeric code, its string message (else "codex refused the request without saying why") and its
   data object; the last answer of an id wins; anything else skipped.
4. The read (plugins-codex.ts:344): no codex on PATH (the script prints `__WSP_NO_CODEX__`) is no plugins and no
   refusal; `plugin/list` answered as an unknown variant is the same; no list or no config refuses "plugins: Codex's
   app server did not answer, so Codex's plugins were not read". Installed plugins are read once more with
   `plugin/read` (ids from 2 up). Each row: id, name, marketplace, local version, scope user, on where `enabled`,
   the cache path under `codexHome` where the version is known, the description (plugin/read's, else the
   interface's short one), the source (url, path or package), what it brings (skills, hooks by event, servers sorted,
   apps). Then each id config.toml names that the list lacks (sorted): `missing` marketplace where its local source
   folder is not there, else unlisted; then each listed, uninstalled one config names: `missing` uninstalled. Load
   errors become "plugins: Codex could not load a marketplace: <message>". The user layer's file and version ride
   the result.
5. The switch (plugins-codex.ts:421): a status `ok` result is done; a `configVersionConflict` is
   `config_changed_refusal(file or "config.toml")`; else "<message less a final dot>, so nothing was switched." with
   "Codex's app server did not answer" where none.

## Edge cases

- `answers_of` skips a string id (`"4"`), a result that is an array, and a broken line.

## Behaviour nobody wrote down

- Rule 4.26: rows keep the list's marketplace and plugin order; the absent ones follow in sorted order.

## Tapes

agents.read (plugins) and plugins.toggle for Codex (#2443).

## Decisions

1. `ServerAnswer` is an enum for the TS union; ids are i64.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2, 3 | oracle_text: codex_app_server::requests_and_the_script_are_the_ts_text, ::answers_are_read_by_numeric_id_the_last_of_an_id_winning_and_noise_skipped |
| 4 | plugins: codexs_plugins_through_its_app_server::lists_brag_and_no_claude_coauthor_with_what_each_brings_from_plugin_list_and_plu, ::reads_the_six_codex_app_plugins_config_toml_names_and_plugin_list_does_not_retur, ::reads_a_plugin_config_toml_names_and_plugin_list_returns_as_not_installed_as_mis, ::says_codexs_plugins_were_not_read_where_its_app_server_did_not_answer_and_nothin; the_whole_read_is_the_ts_read::codexs_read_is_every_ts_row_with_its_config_version_and_file |
| 5 | plugins: codexs_plugins_through_its_app_server::switches_with_config_value_write_on_plugins_id_enabled_with_the_version_it_read |
