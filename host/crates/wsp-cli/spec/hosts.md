# hosts

Source: `packages/host/src/hosts.ts` (371 lines) at 65f6b89c7. TS tests: `test/hosts.test.ts` (21, of which 4 are
`hostAddress` from verbs.ts and stay with cli-a), `test/launch-pair.test.ts` (1). Rust tests: `tests/hosts.rs` (33).
Oracle: `tests/fixtures/hosts_oracle.json` (`aliasFrom`, `aliasOk`, `lines`, `empty`, `badAlias`, `noSuch`, `noKey`,
`named`, `window`, `wsUrl`), printed by the TS. Chapter 10 rules 10.3 step 3, 10.18, 10.22, 10.26.

## What it does

The hosts records this computer keeps (one 0600 file per alias under `<home>/hosts`), and the one rule that
decides which host a line runs against. The command line and the tool server both come through `aimed_host`.

## Rules

1. `wsp_home(env)` is `WSP_HOME` when non-empty, `" "` included, else `<HOME>/.wsp` (:48-50, serving-home.ts:19).
2. An alias matches `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$` and holds no `..` (:65-70). `alias_from` folds: every
   character outside the class to `-`, each run of two or more dots to one, leading characters outside
   `[A-Za-z0-9]` dropped, cut to 64 UTF-16 units; a result the checker refuses is `host` (:83-90). Every fold is
   an alias.
3. A record reads (`isRecord`, :92-99) when `url`, `deviceId`, `deviceToken` are strings and `via.kind` is exactly
   `account` with a string `via.hostId`. Empty strings are fine; `pairedAt` and `hostKey` are not checked (a
   non-string `hostKey` is a decision below). A name the checker refuses, a missing file and bad JSON all read
   `None`, never an error.
4. `write_host` refuses a bad alias before touching the disk with the usage refusal `"<JSON-quoted alias>" is not a
   host alias.` + ` ` + `A name is letters, digits, dots, dashes and underscores, opens with a letter or a digit,
   and is at most 64 characters.` (fix is the second sentence). Otherwise `write_own(home, "hosts/<alias>.json",
   JSON.stringify(record, null, 2) + "\n")`: the file 0600 through a temp file and rename, each folder from the
   home down 0700, a wider one repaired. Keys in the record's order, `hostKey` and `pairedAt` left out when absent.
5. `list_hosts` reads every `*.json` under the hosts folder (none when the folder is missing), keeps the ones that
   read (rule 3), sorts by `localeCompare` (ICU root collation: `attic, box, Box, cellar`), and answers
   `{alias, url, deviceId}`, never the token (:122-131).
6. `remove_host` removes only a file that reads as a record and answers whether it did, and fails where `rmSync` throws (:134-138).
7. `account_records` is every listed record; `account_hosts` drops the one whose `via.hostId` equals this state's
   relay record's `hostId` (`account::read_relay_record`); `account_aim` is One, Several (aliases in list order) or
   None (:177-203). `aimed_alias` is One's alias. All of it reads files only.
8. `named_host` (:296-303): a `--host` word that trims to empty refuses with `empty_host_refusal` (usage:
   `--host was given an empty word, which names no host.` / fix `Name one as wsp hosts lists it, or leave the flag
   off.`). The name is the first of the trimmed `--host` word and the trimmed `WSP_HOST` that is non-empty; a blank
   `WSP_HOST` is ignored. None when neither names one.
9. `aimAt(named)` (:328-339): an address by `wsp_words::app_ports::is_url` is `Url`, carrying the launch pair's
   token and key only when the pair's URL equals the named address byte for byte. Anything else is an alias: its
   record, or the usage refusal `no_such_host_line(named, home)` + ` Run wsp hosts for the list.` (fix
   `Run wsp hosts for the list.`).
10. `aimed_host` (:308-326), in order: `named_host`; the launch pair (`hostFromEnv`: `WSP_HOST_URL` and
    `WSP_HOST_TOKEN` both set, `WSP_HOST_KEY` optional) as `Url{url, token, host_key}`, ahead of any lock or record;
    `Here` when `serving_host(state)` holds; the account's One host; Several refuses with usage
    `several_account_hosts_line(aliases)` + ` ` + `name_one_host()` (fix `name_one_host()`); else `Here`.
11. `keyed(aim, where)` (:347-353) runs on every aim but `Here`: an aim holding a token and no key refuses with
    `authRefusal(hostNoKeyLine(where))` (kind auth, no fix) unless its address's `served_hostname` is loopback.
    `where` is the alias, the named address, or `LAUNCHED_WITH` for the pair. `served_hostname` reads only
    http and https addresses, so a `ws://127.0.0.1` pair with a token and no key is refused.
12. `dial_window_ms` (:260-266): `Here` 5 000; an aim whose address `served_hostname` reads as loopback
    (`127.0.0.0/8`, `localhost` in any case, `[::1]`, `127.1`) 5 000; anything else, unreadable words included,
    15 000. Never panics on a hand-edited word.
13. `ws_url_of` is wsp_words' rule re-exported: `http`→`ws`, `https`→`wss`, one trailing slash dropped, `/ws`
    appended (`wss://a/ws` becomes `wss://a/ws/ws`).
14. Words, byte for byte from the oracle: `no_such_host_among` (`this computer holds none` or `holds a, b`),
    `several_account_hosts_line`, `name_one_host` (`Name one on the line with --host <name>.` blank line `Run wsp
    hosts for the list.`), `read_the_hosts`, `address_not_paired_line`, `device_refused_line`,
    `host_side_only_line`, `host_side_only_fix` (default `HOST_SIDE_ACCESS`), `state_ignored_line`.
15. `no_answer_refusal(where, why)` is `the host at <where> did not answer: <why>` with kind `unreachable` and no
    fix; `no_answer_within` says `nothing came back within <ms> ms` (:240-248).
16. `aim_name`: alias, url, or `this computer`. `aim_address`: the record's url or the url; `None` for `Here`.
    `aim_holds`: the record's device token and key, or the url's token and key; nothing for `Here`.
17. Every reading is redone at each call; nothing is cached across lines (rule 10.26).

## Decisions

1. `HostRecord.paired_at` is `Option<Value>` and `host_key` `Option<String>`: a record with a non-string `hostKey`
   fails to deserialize in Rust and reads `None`, where the TS reads it and later compares the number. Recorded
   here, not tested; no writer makes one.
2. A record's unknown keys are dropped on a Rust rewrite (the TS admission spreads them back). cli-a's admission
   rewrite is the only rewriter.
3. `Env` is `IndexMap<String, String>`; `HostPick.env` `None` is the process environment.

## After the port

1. `ws_url_of("wss://a/ws")` doubles the path; a launch pair carrying a ws URL gets the far window and is refused
   without a key even on loopback (rule 11).
2. `alias_from` folds a non-ASCII letter to `-` then strips it when leading (`Ünïcode name` → `n-code-name`).

## Rules to tests

| Rule | Test |
|---|---|
| 1 | wsp_home_is_the_named_home_and_an_empty_one_names_none, the_default_home_is_dot_wsp_under_the_users_home_directory |
| 2 | a_name_nobody_typed_folds_through_the_rule_a_typed_one_is_held_to, an_alias_is_one_name_of_at_most_64_characters_opening_with_a_letter_or_digit |
| 3 | a_file_with_no_account_behind_it_reads_as_no_record, a_record_with_empty_device_fields_and_no_paired_at_still_reads, a_hosts_folder_somebody_hand_edited_lists_what_reads_and_skips_the_rest |
| 4 | a_record_is_written_only_this_user_can_read_and_read_back_whole, a_record_lands_as_two_space_json_with_a_newline_in_its_key_order, a_record_and_a_folder_an_older_build_left_wider_are_repaired_by_the_write, a_word_that_could_never_be_a_file_under_the_hosts_folder_is_refused_in_one_sentence |
| 5 | every_alias_is_listed_with_its_url_sorted_and_a_home_with_no_hosts_folder_lists_nothing, the_listing_is_in_locale_order_not_byte_order |
| 6 | remove_takes_the_file_away_and_answers_false_when_there_was_none, remove_leaves_a_file_that_does_not_read_as_a_record |
| 7 | a_line_goes_to_the_one_account_host_that_is_not_this_computer, a_host_starts_here_when_the_accounts_one_host_is_this_computer, the_account_is_read_off_the_hosts_folder_and_this_computers_record_alone |
| 8 | an_empty_host_word_refuses_a_blank_wsp_host_is_ignored_and_a_padded_name_is_trimmed |
| 9 | a_url_typed_where_an_alias_goes_is_a_host_nothing_is_stored_for, a_host_address_takes_the_token_the_environment_carries_for_that_same_address_only, an_alias_nothing_is_stored_for_is_refused_in_one_sentence_naming_the_ones_that_are |
| 10 | the_flag_beats_the_environment_beats_the_lock_here_beats_the_accounts_one_host, the_launch_pair_beats_any_state_file_or_alias_here_and_loses_to_the_flag_and_wsp_host, several_account_hosts_refuse_in_one_sentence_naming_them_and_a_name_settles_it, a_host_starts_here_with_no_account_record_and_a_serving_host_beats_the_account, a_line_with_no_pair_and_no_home_records_aims_here |
| 11 | an_aim_at_a_host_with_no_key_held_refuses_whether_a_record_or_a_launch_carried_it, a_token_with_no_key_over_a_ws_loopback_address_is_refused_since_the_rule_reads_no_ws_address |
| 12, 13 | a_dial_waits_a_loopbacks_window_near_and_the_roads_window_far |
| 14 | the_hosts_words_are_the_ts_words |
| 15 | a_host_that_did_not_answer_is_stamped_unreachable |
| 16 | an_aim_names_its_host_and_its_address_and_what_it_holds |
