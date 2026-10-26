# account (the files-and-readings unit)

Source at 65f6b89c7: `packages/host/src/account.ts` (134), `serving-home.ts` (51), `daemon-fix.ts` (40),
`repo-root.ts` (28), `cloud.ts` (17), `version.ts` (16): 286 lines. TS tests: the two serving-home cases of
`test/serving-home.test.ts` that use only this file (the other four test `cli/state.ts`); the rest have none. Rust
tests: `tests/account.rs` (63), written from the source where the TS has no test.

## Rules

1. `write_json_file(path, value)` (account.ts:76-78): `JSON.stringify(value, null, 2)` and `\n` through
   `write_own(dirname, basename)`: 0600 file through a temp file and rename, folders 0700, a wider one repaired.
2. Relay record `<dirname(state)>/relay.json` (:80-94): reads when `relayUrl`, `hostId`, `token` are strings; the
   other keys are kept as the file holds them. Remove is `rm -f`.
3. Client record `<home>/relay-client.json` (:98-112): reads when `relayUrl` and `token` are strings and the object
   has no `hostId` key at all (`hostId: null` is no client record).
4. Device key `<home>/device-key.json` (:114-131): reads when `publicKey` and `privateKeyPem` are strings.
   `device_key_here` answers the held pair untouched, else mints `wsp_store::keys::new_place_key_pair()`, writes
   `{"publicKey", "privateKeyPem"}` by rule 1 and answers it; a file that does not read is replaced. One key per
   home; a sign-out never removes it. `device_key_of` is `{fingerprint: key_fingerprint(publicKey), publicKey}`.
5. `home_named(v)`: unset and `""` are none; `" "` is a home (serving-home.ts:19). `serving_home(env, user)` is that
   or `<user>/.wsp`.
6. `real_state(path)` (:28-32): the real path of the file when it exists, else the real path of its folder joined
   with the base name when the folder exists, else the path as written.
7. `serving_elsewhere(state, env, user)` (:38-41): the served state is `serving_home/state.json`; answer it when its
   real path differs from the line's state's and a host serves it (`host_lock::serving_host`); else none. A folder
   reached through a symlink is one state.
8. `SERVING_HOME_SH` is byte for byte the TS: two lines, one newline, none at the end. Run under `/bin/sh` with
   `HOME` and `WSP_HOME` set, it settles on the same home as rule 5 and its `wsp_serving` agrees with
   `serving_host` for a live pid, a dead pid and no lock (a pid under another login aside).
9. `git_root_of(folder)` (repo-root.ts:9-14): the folder or the nearest one up holding a `.git` entry of any kind;
   none at `/`. `main_worktree_of` (:18-28): the root when `.git` is a folder; else the `gitdir:` line's value
   (any line, trimmed), resolved against the root when relative; its parent must end `/.git/worktrees`, and the
   answer is two folders up from that; anything else is none.
10. `VERSION` is `packages/host/package.json`'s `version` (0.3.8 at 65f6b89c7). `installed_version(path)` reads the
    string `version` now; unreadable, bad JSON or no string version is none. `state_writer_here(argv, exec, cwd)` is
    `{wsp: VERSION, daemon: DAEMON_VERSION, bin}`, `bin` = `resolve(cwd, argv[1])` or `exec` without an `argv[1]`.
11. `cloud_on_in(env)` is `CLOUD_BUILT` and `WSP_CLOUD` exactly `1` (`""`, `true`, ` 1`, `0` are off).
    `cloud_on()` reads the process environment once and keeps the answer.
12. `road_of(argv, shim)` (daemon-fix.ts:18-23): the shim, else `argv[1]`, else `""`, split on `/`: a part equal to
    `node_modules` is npm (checked first), a part ending `.app` is the app, else checkout. An empty shim is still
    the shim. Lines: npm `npm i -g @wsp-labs/wsp` / `npm i -g @wsp-labs/wsp@<v>`; app `updating the wsp app` for
    both; checkout `a cargo build of the daemon and node packages/wspx/scripts/daemon-binary.mjs --from its binary`
    / `a pull of the checkout and a build`.

## Decisions

1. `installed_version` takes the path (the TS reads `../package.json` beside its own module); the Rust binary has no
   package.json beside it, so the caller names one or uses `VERSION`.
2. `CLOUD_BUILT` is a const true; a public build sets it false at build time.

## After the port

1. `main_worktree_of` throws in the TS when the `.git` file cannot be read; the port answers none.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | write_json_file_writes_two_space_json_and_a_newline_and_leaves_no_temp_file, write_json_file_lands_at_0600_in_a_0700_folder_it_makes, write_json_file_repairs_a_wider_folder_and_replaces_a_wider_file_at_0600 |
| 2 | relay_record_path_is_relay_json_beside_the_state_file, relay_record_round_trips_and_lands_as_the_struct_writes_it, relay_record_lands_at_0600_in_a_0700_folder, relay_record_reads_with_only_url_host_id_and_token_and_keeps_the_other_fields_as_held, relay_record_missing_token_host_id_or_url_or_holding_a_non_string_reads_as_none, relay_record_that_is_not_an_object_of_the_shape_reads_as_none, remove_relay_record_takes_the_file_away_and_a_missing_one_is_no_error |
| 3 | client_record_path_is_relay_client_json_in_the_home, relay_client_round_trips_and_lands_as_the_struct_writes_it, relay_client_lands_at_0600_in_a_0700_home, relay_client_with_a_host_id_key_even_null_reads_as_none, relay_client_missing_token_or_url_or_holding_a_non_string_reads_as_none, remove_relay_client_takes_only_the_client_file_and_a_missing_one_is_no_error |
| 4 | device_key_path_is_device_key_json_in_the_home, read_device_key_pair_reads_a_file_holding_both_strings_and_nothing_else, device_key_here_mints_once_and_answers_the_same_pair_from_then_on, device_key_here_writes_the_pair_as_public_key_then_private_key_pem_at_0600_in_a_0700_home, device_key_here_answers_a_held_pair_untouched_and_replaces_one_that_does_not_read, device_key_here_mints_a_different_pair_for_each_home, device_key_of_names_the_public_half_by_its_fingerprint |
| 5 | default_home_in_is_dot_wsp_under_the_given_user_folder, home_named_treats_unset_and_empty_as_one_answer_and_a_space_as_a_home, serving_home_is_wsp_home_when_it_names_one_and_the_users_dot_wsp_otherwise |
| 6 | real_state_resolves_a_file_or_its_folder_through_links_and_leaves_what_nothing_made_as_written |
| 7 | serving_elsewhere_names_the_state_a_host_here_serves_when_a_line_works_on_another_and_nothing_when_they_are_the_same, serving_elsewhere_is_none_when_the_host_under_this_computers_home_is_gone |
| 8 | the_serving_home_is_wsp_home_when_it_names_one_and_this_computers_own_otherwise_in_sh_as_here, the_serving_home_reads_an_empty_wsp_home_as_unset_in_sh_as_here, serving_home_sh_is_two_lines_joined_by_one_newline_with_none_at_the_end, serving_home_sh_reads_no_host_from_a_lock_with_no_pid_a_dead_pid_or_no_lock |
| 9 | the eleven git_root_of_* and main_worktree_of_* tests |
| 10 | version_is_the_version_packages_host_package_json_carries_and_installed_version_reads_it, installed_version_reads_the_version_string_off_a_package_json_now, installed_version_of_a_missing_unparsable_or_versionless_file_is_none, state_writer_here_names_the_version_the_daemon_and_argv_1_as_it_was_given_when_absolute, state_writer_here_resolves_a_relative_argv_1_against_the_working_folder, state_writer_here_without_an_argv_1_names_the_executable |
| 11 | cloud_on_in_is_true_only_for_wsp_cloud_exactly_1, cloud_on_reads_the_process_environment_and_answers_the_same_each_time |
| 12 | the eight road_of_* tests, daemon_fix_line_is_the_npm_install_for_npm_the_app_update_for_the_app_and_a_cargo_build_for_a_checkout, release_update_line_names_the_release_for_npm_and_ignores_it_for_the_app_and_a_checkout, release_line_of_a_road_is_that_roads_release_line |
