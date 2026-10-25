# small: key check, fingerprint, presets, template drop, provider slot, project installs, vault cipher

Source at `a4033dc34`: `packages/engine/src/key-check.ts` (42), `key-fingerprint.ts` (6, re-exporting
`packages/keys/src/index.ts:23`), `dotfiles-presets.ts` (9), `drop-template.ts` (13), `provider-slot.ts` (49),
`project-install.ts` (60), `vault-cipher.ts` (111). Rust: the same names under `src/`. Tests: `tests/small.rs`,
`tests/key_check.rs`, hard tests in `tests/small_hard.rs`. Oracle: `small.json`, `vault.json`.

## key-check.ts

- B1. No `check_key` offered: `Taken { answer: None }`, nothing called. A `Null` answer is `Taken { None }`, any other
  value `Taken { Some(v) }`. A failure with a kind and a status whose kind is auth or plan: `Refused { said:
  provider_said_line(status, message) }` (`401 Unauthorized`). Anything else, a transient 503 and a road failure among
  them: `Unchecked { said: the message }` (`Service Unavailable`, `fetch failed`).
- B2. `key_check_line`: taken `None`; unchecked `key_unchecked_line(said, provider)` saved or not; refused
  `saved_key_refused_line` when saved, else `key_refused_line`.

## key_fingerprint

- B3. `SHA256:` + base64 (standard, padding stripped) of sha256 of the bytes node's `Buffer.from(s, "base64")` decodes:
  `-` and `_` read as `+` and `/`, characters outside the alphabet skipped, decoding stops at the first `=`. Oracle
  `fingerprints`: whitespace and `!!` change nothing, `=AAAA` is the hash of nothing, `AA=AA` of one byte; `QUJ`, `QUJD` and `not*base64!` as the roads slice pins them.

## presets, template drop

- B4. `PRELUDE` the TS text; `apt(pkg, cmd)` = `command -v <cmd> >/dev/null 2>&1 || { apt-get update -qq; apt-get install
  -y -qq <pkg>; }`, cmd defaulting to pkg.
- B5. `drop_template`: `None` id does nothing. A backend whose capabilities have `templates` and whose offers have
  `templates` deletes it; a missing failure is fine, any other is the failure. Otherwise nothing happens.

## provider-slot.ts

- B6. `ProviderSlot::new(initial)`; `backend()` is the slot as a `MachineBackend`; every method, getter and `offers()`
  read goes to the module current at the moment of the call, so a handle taken before a swap reaches the new module;
  `swap(next)` changes it for every later call; `current()` answers the very `Arc` last set (pointer equality).

## project-install.ts

- B7. `project_installs(root_names, dir)`: `seed_installs_for(root_names)` in catalog order; command is `run`, or with a
  store `<run> <flag> <q <dir without trailing slashes>/<store dir>>`; check copied where present. Oracle `installs`.
- B8. `INSTALL_STORES`: store dirs of every install across SEED_ROWS in order (`[".pnpm-store"]` today).
- B9. `install_script`: oracle `installScript`; the log's folder is the log less `/[^/]+$`, so a log with no slash
  keeps the whole name (`mkdir -p 'plain.log'`, oracle `installScriptNoSlash`). `check_scripts`: oracle `checkScripts`.
  `INSTALL_MS` 1800000, `CHECK_MS` 300000.

## vault-cipher.ts

- B10. Seal: X25519 with a fresh ephemeral key; shared secret through HKDF-SHA256 (salt 16 random bytes, info
  `wsp-vault-1` + the ephemeral SPKI DER, 32 bytes out); AES-256-GCM with a 12-byte random nonce, the header line
  (JSON + `\n`) as additional data; output = header line, ciphertext, 16-byte tag. Header keys in order format
  `wsp-vault-1`, cipher `aes-256-gcm`, to `key`, salt, nonce, ephemeral (standard base64), image.
- B11. Open, refusals in order: a `Public` key `opening a vault sealed to a place needs that place's private key`; no
  newline `this file is not a sealed wsp image: it has no header line`; a line that is not JSON or not a header `this
  file is not a sealed wsp image: its header is not one wsp wrote`; `to` not key `this vault was sealed to a passphrase,
  not to a place's key`; no ephemeral `this vault names no ephemeral key and cannot be opened`; body shorter than 16
  `the sealed vault is shorter than its own tag`; a tag that does not verify `the sealed vault would not open: the
  passphrase or the key is not the one it was sealed to, or the file was changed`. Seal with a `Private` key: `sealing to
  a place needs its public key`.
- B12. A vault node sealed opens here (oracle `vault.json`, raw keys from the JWK); `sealed_image_of` reads the image
  with no key; a vault sealed here opens here, and an edited header byte refuses.

## Restart

The slot holds its module in memory only (a host restart builds it again); nothing else here keeps state.

## Pinned since (K-rules, values from running the TS)

- K1. `open` and `sealed_image_of` authenticate the header line's raw bytes up to and including its `\n` as the
  additional data, never a re-serialised header: a header with keys out of schema order, an unknown image key (dropped
  from the answer), spaces, a tab or a `\r` before the `\n` still opens. Header validity is the zod schema's (hash 64
  hex, version and usedBytes non-negative integers, `to` `key`); a key derivation that fails comes before the length
  check; a body of 15 bytes is `the sealed vault is shorter than its own tag`, of 16 a tag failure.
- K2. `install_script`'s log folder is the log less `/[^/]+$`: `/c.log` gives `''`, `logs/` stays `logs/`, `/a//c.log`
  gives `/a/`, no slash keeps the whole log.
- K3. `drop_template(Some(""))` deletes `""`; only `None` asks nothing.
- `ProviderSlot` forwards every `MachineBackend` method, defaults included (26 in all), to the module current at the
  call; a call in flight keeps its module after a swap, and `swap` never waits on one.

## Rules and their tests

Files: `small.rs` (ported), `key_check.rs` (key), `small_hard.rs` (hard).

| Rule | Tests |
|---|---|
| K1 | small_hard.rs: `open_authenticates_the_header_bytes_as_they_came` |
| K2 | small_hard.rs: `the_install_logs_folder_is_the_log_less_its_last_name` |
| K3 | small_hard.rs: `an_empty_template_id_is_still_dropped` |
| B1, B2 | key `asks_the_template_table_the_cheapest_read_the_key_opens_and_nothing_else`, `reads_a_refusal_as_the_providers_own_status_and_word_under_our_sentence`, `a_plan_that_may_not_read_the_account_is_the_same_refusal_the_key_alone_will_not_do`, `a_road_that_never_left_this_computer_says_so_instead_of_blaming_the_key`, `a_gateway_status_answered_about_the_gateway_not_the_key`, `a_backend_that_holds_no_key_has_none_to_check` |
| B3 | ported `is_the_word_every_ssh_tool_prints_for_the_same_key`, `wears_that_one_form_for_a_place_link_key_too_which_travels_as_spki_der_rather_than_an_ssh_blob`; hard `fingerprints_decode_base64_as_node_does` |
| B4 | hard `apt_and_prelude_write_the_ts_text` |
| B5 | hard `drop_template_ignores_a_missing_template_only` |
| B6 | ported `delegates_every_read_and_call_to_the_module_it_holds_and_a_swap_moves_them_all_at_once`, `a_backend_taken_off_the_slot_before_a_swap_still_runs_against_the_module_current_when_it_is_called`; hard `a_handle_taken_before_a_swap_reaches_the_new_module` |
| B7 to B9 | ported `is_the_lockfiles_command_in_catalog_order_one_per_ecosystem`, `keeps_a_store_that_hard_links_into_the_checkout_inside_the_checkout`, `checks_a_manager_only_where_its_row_names_a_command_that_exits_0_on_one_that_works`, `is_nothing_for_a_repo_no_row_names_an_install_for`, `runs_in_the_project_folder_and_writes_its_output_to_a_log_outside_it`; hard `install_lines_match_the_ts_oracle` |
| B10 to B12 | ported `a_places_key_round_trips_and_another_places_key_does_not_open_it`; hard `a_vault_node_sealed_opens_here`, `each_refusal_says_its_words` |
