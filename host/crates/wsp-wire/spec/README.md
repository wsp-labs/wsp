# wsp-wire: the wire units

packages/protocol/src/wire at a3b870a6e, 5,356 TS lines, cut into ten units. Each unit has a spec here and a test file under `tests/`; `tests/common/mod.rs` reads a frame through the crate's parse and checks it writes back. The views and top-level types the wire names are the views writer's, under `src/views` and `src/types`, merged in.

| Unit | TS source | TS lines | TS cases ported | Rust tests |
|---|---|---|---|---|
| helpers_limits | wire/helpers.ts, wire/limits.ts | 307 | 8 (19 `it.each` rows) | 20 |
| capabilities_devices | wire/capabilities.ts, wire/devices.ts | 423 | 17 | 25 |
| place_link | wire/place-link.ts | 481 | 21 | 27 |
| machine_link | wire/machine-link.ts:1-509 | 509 | 27 | 33 |
| proc_frames | wire/machine-link.ts:510-702, daemon.ts process rows | 193 + 140 | 3 | 12 |
| daemon | wire/daemon.ts | 972 | 16 | 20 |
| daemon_version | wire/daemon-version.ts | 838 | 6 | 10 |
| events | wire/events-union.ts, wire/daemon-events.ts | 202 | 30 | 33 |
| runtime_ops | wire/runtime.ts:1-1029 | 1029 | 42 | 45 |
| runtime | wire/runtime.ts:1030-1431 | 402 | 12 | 17 |
| (contract) | daemon/fixtures/contract, read by daemon-contract.test.ts | | 3 | 4 |
| **Total** | | **5,356** | **185** | **246** |

The TS module's own tests: 188 `it` blocks across 23 protocol test files touch a wire export (some of them only a constant list or another module's export, which are skipped and named in each porter's report); the TS has no test file of its own for the wire folder.

## Rules pinned, and the test that pins each

Null, absent and empty:
- helpers_limits 13 (`.catch(undefined)` reads null and a bad value as absent): `or_none_reads_a_value_that_does_not_parse_as_absent`.
- helpers_limits 4 (a URL with no host is `""`, not None): `host_of_answers_the_parser_s_host`.
- helpers_limits 11 (`listed` with an absent value passes, `""` with no options refuses): `listed_refuses_a_value_no_option_holds_naming_the_options_and_the_legacy_ones`.
- capabilities_devices 11 (an empty key is none, an empty code stays empty): `read_join_token_splits_at_the_first_mark_and_keeps_the_key_as_typed`.
- capabilities_devices 4 (zero over zero is NaN, a missing or empty image reads memory alone): `fork_room_at_its_edges`.
- place_link 8 (joinedAt any value, absent stays absent): `parse_place_file_checks_six_fields_and_keeps_the_rest`.
- place_link 10 (no marker, or nothing after it, is None): `the_held_read_finds_its_own_line_and_names_what_a_join_left`.
- machine_link 6 (`parent` absent, null and a string kept apart): `a_snapshot_row_keeps_an_absent_a_null_and_a_named_parent_apart`; machine_link 10 (an empty reason keeps the colon): `the_cannot_boot_line_turns_this_computer_into_it_everywhere_and_keeps_an_empty_reason`.
- machine_link 11 (absent door is no line): `the_no_tools_line_speaks_only_of_a_door_that_said_it_is_shut`.
- proc_frames 1-3 (null and unknown kind is other, absent kind stays absent, unknown owner and placedBy absent): `a_null_or_wrong_typed_kind_reads_as_other_and_an_absent_one_stays_absent`, `reads_a_kind_it_does_not_know_as_other_and_an_owner_rung_it_does_not_know_as_no_owner`, `a_container_placed_by_a_word_it_does_not_know_reads_as_unplaced`.
- proc_frames 5 (a snapshot with no seq takes no changes; absent cost and cgroups stay absent): `changes_that_do_not_follow_the_snapshot_held_leave_nothing`, `rows_by_pid_and_the_containers_held_where_the_changes_carry_none`.
- daemon 4 (`moved` absent reads as `[]`): `a_diff_reply_with_no_moved_reads_as_none_moved`.
- daemon_version 2-4 (None is no line, 0 is behind, absent wsp version reads 0): `says_nothing_of_a_computer_that_is_level_one_that_is_ahead_or_one_that_never_reported`, `names_both_versions_when_this_wsp_deploys_a_newer_daemon_than_the_computer_runs`, `the_leave_a_computer_runs_asks_from_141_and_takes_from_145`.
- events 2 (no seq writes none): `seq_is_a_positive_integer_written_after_the_event`.
- runtime 1-3 (non-object frames have no secrets, `""` is a secret, null event has no workspace): `request_secrets_reads_one_level_in_the_fields_order`, `workspace_id_of_reads_the_first_string_in_its_order`, `asker_of_needs_both_ids`.

Caps and limits, one past each:
- `HTTP_URL_MAX` 8192: `is_http_url_also_caps_the_length_and_refuses_whitespace_and_control_characters` (8192 takes, 8193 refuses).
- cgroup path 512 and watch name 64: `a_cgroup_path_is_absolute_plain_names_under_512`, `a_watch_name_is_one_to_64_letters_digits_dashes_and_underscores`.
- base64 byte count: `base64_of_n_bytes_counts_the_bytes_off_the_text_and_its_padding`; place link lengths: `refuses_a_nonce_that_is_not_the_length_a_challenge_is_a_short_one_is_a_nonce_som` and its two neighbours.
- `AGENTS_KEPT` 6 and the 400 MB share: `agents_kept_on_a_joined_computer_follow_its_memory`; backoff ceiling 10 s plus 249 ms: `link_backoff_doubles_from_half_a_second_to_ten_with_a_quarter_second_of_jitter`.
- `EXEC_BODY_MAX` on machine.exec: `refuses_an_op_it_does_not_carry_a_part_out_of_range_and_a_command_over_the_body_ca`; http.post 64 headers, 8192 per value, stdio.open's empty cmd, container id: `the_caps_on_http_post_and_stdio_open`.
- `PAIR_CODE_LENGTH` 8: `is_pair_code_takes_eight_characters_of_the_alphabet_alone`, `shown_pair_code_groups_what_it_can_and_cuts_at_eight`.
- agents reported 32: `takes_the_agents_a_computer_found_on_itself_up_to_the_cap_the_sentence_they_land`; PlaceDoorView needs one address: `a_door_view_needs_an_address`.
- seq positive, subscribe seq non-negative: `seq_is_a_positive_integer_written_after_the_event`, `the_subscribe_reply_takes_seq_zero_and_gap_true_alone`.

Ordering, key order included:
- Rule 4.26 index keys: `js_key_order_moves_index_keys_first_as_node_writes_them`.
- A page's frame keeps its keys in order: `a_daemon_frame_keeps_its_keys_in_order_and_never_names_auth`.
- place file text in the file's order: `parse_place_file_checks_six_fields_and_keeps_the_rest`.
- apply_proc_changes rows by pid and field order: `rows_by_pid_and_the_containers_held_where_the_changes_carry_none` (compares the written text byte for byte).
- by_proc_column heaviest first, ties by pid: `by_proc_column_orders_heaviest_first_and_ties_by_pid`.
- request_secrets in the fields' order: `request_secrets_reads_one_level_in_the_fields_order`.
- Event written type first, seq last: `seq_is_a_positive_integer_written_after_the_event`.
- Every discriminator issue names every op or type in the union's order: `an_unknown_op_is_one_discriminator_issue_naming_all_211_ops_in_order`, `an_unknown_op_is_one_discriminator_issue_naming_every_op_in_order`, `an_unknown_type_is_one_discriminator_issue_naming_all_77_types_in_order`.
- workspaces_blocked_by asks in one order and loads the overlay module before the dry run: `asks_in_one_order_so_the_reason_a_person_reads_is_the_first_thing_missing_rather_` and `loads_an_overlay_module_the_kernel_does_not_list_yet_and_reads_the_list_again_befo`.
- A held read takes the first marker: `the_held_read_finds_its_own_line_and_names_what_a_join_left`.

What survives a host restart:
- Nothing in these units holds state. The one restart-facing rule is events 2 and the stream id: `the_subscribe_reply_takes_seq_zero_and_gap_true_alone` pins the reply a client reads after a restart (`gap: true`); the cursor itself is the runtime's bus.
- `StateShape` is what a save records about its writer: `a_state_shape_names_its_writer_by_binary_build_and_daemon`.

## The merge with the views half

- Every stand-in now names the real type: `src/wire/pending.rs` and the views writer's `src/types/pending_wire.rs` are gone; `crate::wire` re-exports `parse`, `Check`, `SchemaError` from `crate::types::check`.
- The slate shapes the op and event unions name (`SlateWireValues`, `SlateCause`, `SlateBy`, `SessionSlateEvent`, `SlateValuesEvent`, `SlateRunEvent`) sit in `crate::types::slate_wire`, since wsp-slate depends on wsp-wire.
- Fifteen test assertions that compared a stand-in field to `json!(...)` compare the field's written value (`serde_json::to_value`) instead; what each asserts is unchanged.
