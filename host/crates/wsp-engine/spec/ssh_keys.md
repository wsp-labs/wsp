# ssh_keys: the key a machine is known by, and the words of a refusal

Source: ssh-backend.ts:377-615 at `a4033dc34`. Module `ssh_backend::keys`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/ssh_keys.json`.

## Seams

- `SshHostKeyReader`: async (reach) answering `Option<String>`.
- `SshLocalRun`: async (file, args, timeout_ms) answering `ExecResult`; the default runs
  `child_exec::run_child(file, args, { env: process env, timeout_ms })` (:386).
- `SSH_LOCAL_READ_MS = 5_000` for every `ssh -G` and `ssh-keygen -F`; `SSH_KEYSCAN_S = 5` told to the tool and
  `SSH_KEYSCAN_MS = 10_000` bounding the child (:390-396).
- Every `ssh -G` here other than `ssh_word_reach`'s carries `ssh_dial_args(reach)` then `user@host` (the typed
  reach, never the resolved one).

## Reading `ssh -G`

`read_values` (machine_facts) keeps the last value of a key: `values[k]` below. Absent and empty are different only
where a rule says so.

- `known_host_target(values)` (:401): `hostkeyalias` when non-empty; else `None` when `hostname` is absent or empty;
  else `[{hostname}]:{port}` when `Number(port)` is an integer other than 22, else the bare hostname (port absent,
  `abc` or `2222.5` give the bare name).
- `known_host_files(values, home)` (:415): `userknownhostsfile` then `globalknownhostsfile`, each split on runs of
  white space, dropping empty words and `none`. A word starting `~/` becomes `{home}/{rest}`, where `home` defaults
  to the passwd entry's home (`getpwuid`), never `$HOME`. A bare `~` or `~x/y` is kept as written.
- `host_key_found(found, algorithms)` (:433): the preference list is `algorithms` split on `,`, each name matching
  `^rsa-sha2-\d+$` read as `ssh-rsa` (`rsa-sha2-512-cert-v01@openssh.com` does not match). Each line of `found` is
  JS-trimmed and split on white space; a line with fewer than three words, or whose second word is not in the list,
  is skipped (this drops comment lines and `@cert-authority` / `@revoked` lines). The best is the lowest rank; on a
  tie the first line wins. Answer `"{type} {key_fingerprint(third word)}"`, or `None`. `key_fingerprint` decodes
  base64 the way Node does (characters outside the alphabet ignored, no padding needed): the oracle pins `QUJ`,
  `QUJD` and `not*base64!`.
- `trusted_by_authority(found)` (:452): of the JS-trimmed lines that are neither empty nor start with `#`, there is
  at least one and every one starts with `@cert-authority `.
- `offered_host_keys(values, run)` (:466): `""` with no scan when `hostname` is empty or absent, or `proxyjump` or
  `proxycommand` is non-empty. Else `ssh-keyscan -T 5 -p {Number(port) or 22} {hostname}` bounded 10,000 ms; its
  stdout when it exits 0, else `""`.
- `proxied_by(values)` (:493): `ProxyJump` when `proxyjump` is non-empty, else `ProxyCommand` when `proxycommand` is,
  else none.

## The reads

- `offered_host_key(reach, run)` (:482): `ssh -G` exit not 0 answers `{}`. Proxied answers `{ stoppedBy }` and scans
  nothing. Else `{ key }` from `host_key_found(offered_host_keys, hostkeyalgorithms or "")`, or `{}`.
- `known_host_key(reach, run)` (:536): `ssh -G` exit not 0, or no target, answers `None`. Then `ssh-keygen -F
  {target} -f {file}` for each file in order, one after another; the stdout of each that exits 0 is appended. A
  key found there, or a found text not trusted by an authority, is the answer (possibly `None`; nothing is scanned
  when the client holds no line at all). Only an authority-only text asks the machine: `host_key_found` of the scan.
  Never the authority's own key.
- `known_hosts_written(reach, run)` (:560): exit not 0 answers `{}`; else `{ file: first known_host_files, target }`,
  each absent where `None`, key order file then target.
- `SSH_WORD_REFUSAL(word)` (:498): `"{word} is neither a login like root@host nor an alias your ssh config gives a
  HostName"`, one sentence.
- `ssh_word_reach(word, port, key_path, run)` (:505):
  1. A word with `@` is `parse_ssh_address(word, port, key_path)` and its own errors.
  2. A word not matching `^\w[\w.-]*$`, or matching `^(0x[0-9a-f]*|\d+)(\.(0x[0-9a-f]*|\d+)){0,3}$` (case-insensitive)
     is refused with `SSH_WORD_REFUSAL` before the client is asked (`-oProxyCommand=touch`, `123`, `1.2.3`,
     `0x7f000001`, `0177.1`).
  3. `ssh -G {word}` (no dial args), 5,000 ms; values empty where it exits not 0. Refused when hostname or user is
     empty or absent, or the hostname lowercased equals the word lowercased (ssh only lowercased it).
  4. `parse_ssh_address("{user}@{word}", port or Number(values.port) or 22, key_path)`: the alias stays the host.
- `ssh_host_name(reach, run)` (:522): values empty where `ssh -G` exits not 0; proxied answers `None`; an empty or
  absent hostname answers the typed host; else the hostname.
- `ssh_identity(host_key, user)` (:572): `"{host_key} as {user}"`.

## A refusal's words

- `client_words(text)` (:587): split on `\n`; drop lines matching `^debug\d+:`, `^Warning: Permanently added .* to
  the list of known hosts\.?$` and `^(bind \[.*\]:\d+: .*|channel_setup_fwd_listener_tcpip: cannot listen to port:
  \d+|Could not request local forwarding\.)$` (no multiline flag: each test is on one line; a line ending `\r` is
  NOT dropped by the known hosts pattern); join with `\n`; JS-trim. An indented `  debug12:` line is kept.
- `ssh_refusal_line(said, reach)` (:599): `client_words(stderr)`; empty gives `"{user}@{host} refused the login
  over ssh (exit {code})"`, else its last 300 UTF-16 units.
- `SSH_DIAL_MS = 15_000`. `ssh_dial(reach, transport)` (:607): the transport runs `exit 0` as the login with that
  bound; exit 0 answers `Ok`, anything else fails with `ssh_refusal_line`. It installs and leaves nothing.

Nothing here keeps state; nothing survives a restart except what ssh itself writes.

## Pins

| Rule | Tests |
|---|---|
| key off the client's own entry, asks in order | `the_key_is_read_off_the_clients_own_entry_with_nothing_dialled` |
| target and files | `the_entry_is_looked_up_under_the_name_and_port_the_client_writes`, `targets_and_files_match_the_ts` |
| tilde from passwd, not $HOME | `a_tilde_is_the_passwd_home_not_home_env` |
| preferred type, rsa-sha2, markers skipped | `the_key_picked_is_the_type_a_dial_would_negotiate`, `host_key_found_matches_the_ts` |
| authority-only asks the machine | `a_machine_trusted_through_an_authority_is_known_by_the_key_its_certificate_signs`, `only_a_machine_the_client_knows_by_nothing_else_is_asked` |
| file and target from one reading | `the_file_and_the_name_the_entry_is_written_under_come_off_one_reading` |
| offered key, stopped by proxy | `the_key_a_machine_itself_answers_with_is_one_read_of_its_ssh_port`, `a_machine_reached_through_a_jump_is_asked_nothing` |
| alias words, refusals, numeric words | `a_bare_word_ssh_renames_is_the_alias_itself`, `a_port_or_a_key_the_person_typed_wins`, `a_word_ssh_does_not_rename_is_refused`, `a_numeric_word_is_refused_without_asking`, `a_typed_login_is_read_as_it_always_was` |
| host name | `is_the_hostname_the_config_gives_an_alias`, `is_no_address_where_a_jump_carries_the_dial` |
| client words and refusal line | `a_machine_that_does_not_answer_the_dial_is_refused_with_the_clients_own_words`, `client_words_match_the_ts`, `refusal_lines_match_the_ts` |
| dial runs as the login | `sshDial` in ssh_sudo.rs `as_the_login_itself_for_the_reads_that_ask_about_the_login` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `ssh_keys`), held by
`tests/ssh_keys_pins.rs`.

- K-1 `known_host_target` reads `port` with JS `Number` and checks only that it is an integer other than 22: `""` and
  `0` are `[h]:0`, `-1` and `99999` are kept, `1e3` is 1000, `0x16` is 22 and `22.0` is 22 (bare host), `0x8ae` is
  2222, `Infinity` is not an integer (bare host), ` 2222 ` is 2222
  (`the_known_host_target_reads_the_port_with_js_number`).
- K-2 A `~/` file is `path.join(home, rest)`, which normalizes: `/home/x/` and `~/.ssh/k` is `/home/x/.ssh/k`,
  `~/a//b` is `/home/x/a/b`, `~/../etc/k` is `/home/etc/k`, `~/./k` is `/home/x/k`, `~/` is `/home/x`; the list is
  split on single spaces first (`a_tilde_is_joined_and_normalized_as_path_join_does`). The spec's `{home}/{rest}`
  above was short of this.
- K-3 `ssh_refusal_line`: the last 300 units of `client_words(stderr)` with no trim after the cut, so a leading `\n`
  can stay; with nothing kept, `{user}@{host} refused the login over ssh (exit {code})` with the code as given, 124
  and -1 included (`the_refusal_line_keeps_its_last_300_units_untrimmed`).
- K-4 `client_words` matches with JavaScript's rules: `.` does not match `\r` (nor U+2028, U+2029), `$` is the end
  of the line's text, `\d` is ASCII. A bind line ending in `\r` is kept, the known-hosts note with a `\r` inside it is
  kept, and `debug١:` is kept (`client_words_matches_with_javascript_regex_rules`).
