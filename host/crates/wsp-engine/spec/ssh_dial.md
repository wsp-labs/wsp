# ssh_dial: how wsp dials a machine over ssh

Source: ssh-backend.ts:1-375 at `a4033dc34`. Module `ssh_backend::dial`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/ssh_dial.json`, `ssh_carried.json` (the TS's own answers).

## The dial and its written forms

- `SshReach { user, host, port, key_path: Option }`, JSON keys in that order, `keyPath` absent when `None` (:24).
- `SSH_DEFAULT_PORT = 22` (:38).
- `ssh_machine_id(reach)` (:43): WHATWG `new URL("ssh://" + encodeURIComponent(user) + "@" + host + ":" + port)`,
  then `searchParams.set("key", keyPath)` when there is one; answers `url.toString()`. `ssh` is not a special
  scheme, so the host is opaque: never lowercased (`Box.Example.COM` stays), non-ASCII percent-encoded (`héllo` is
  `h%C3%A9llo`), `[::1]` kept; port 22 is kept (`ssh://dev@box:22`). The key is form-encoded (`/` is `%2F`, a
  space `+`, `&` `%26`, `=` `%3D`). The `url` crate's `Url::parse` plus `query_pairs_mut().append_pair` gives the
  same bytes; the oracle's five reaches pin them. A host the URL parser refuses is an error (the TS throws).
- `parse_ssh_address(address, port, key_path)` (:51): user is everything before the LAST `@`, the rest is the host
  part; the host part splits at its LAST `:`; the port after it is read with JavaScript `Number`.
  - user or host empty: `"{address} is not an ssh address; name the machine as user@host"` (`box`, `@box`, `dev@`).
  - port = the given port, else the named one, else 22. A given port wins even where the named one is junk
    (`dev@box:abc` with port 2200 is box:2200).
  - `Number` rules that matter: `""` is 0 (refused), `" 22"` is 22, `0x16` is 22, `22.0` is 22, `1e3` is 1000,
    `abc` is NaN. Not an integer in 1..=65535: `"{address} names no port ssh can dial"` (0, -1, 65536, NaN).
  - `a@b@box` is user `a@b`; `dev@[::1]:22` is host `[::1]`; `dev@::1` is host `:` port 1 (the TS's own answer).
  - Answer key order user, host, port, keyPath.
- `ssh_login_word(reach)` (:67): `user@host`, plus `:port` only where the port is not 22.
- `ssh_machine_name(reach)` (:73): the whole host where it has no `.` or matches `^[\d.]+$`; else the part before
  the first `.` (`box.example.com` is `box`, `1.2.3.4.example` is `1`, `.hidden` is the empty string).

## The transport and the scripts it carries

- `SshTransport`: an async function of (reach, script, options) answering `ExecResult`; tests hand their own.
  `SshTransportOptions { timeout_ms, on_line, stdin, ride }` where `ride` is `AsRoot { sudo_password: Option }`
  (default, no password) or `AsLogin`. A read as the login has no room for a password (:85).
- `SSH_CONNECT_TIMEOUT_S = 10`, `SSH_LINE_CAP = 300`, `SSH_CONTROL_PERSIST_S = 60`, `SSH_CONTROL_DIR_MODE = 0o700`.
- `box_word(said, cap = 48)` (:96): removes every char in U+0000..=U+001F and U+007F..=U+009F, then keeps the first
  `cap` UTF-16 units.
- `ssh_control_dir(home)` (:106): `{home}/.wsp/ssh`; `home` defaults to Node's `homedir()`, which is `$HOME` when
  set and non-empty, else the passwd entry's home.
- `make_ssh_control_dir(home)` (:115): creates it and every missing parent with mode 0700 (under the umask), then
  chmods the leaf to 0700 whatever it was (a 0755 folder left by something else is tightened). Answers the path.
  The folder and its sockets are the only thing this unit leaves that survives a restart.
- `ssh_control_path(reach, dir)` (:125): `{dir}/wsp-ssh-{first 16 hex of sha256("user@host:port")}.sock`. The key
  path is not in the hash; the port is. Always under 100 chars for a 200-char host with dir `/tmp`.
- The root road (:133), exact: `if [ "$(id -u)" -eq 0 ]; then exec bash -c "$1"; fi` newline
  `exec sudo {sudo} bash -c "{DROP_SUDO_MARKS}` newline `$1"`, where `{sudo}` is `-n` for root and `-S -k -p ''`
  for root by password.
- `ssh_args(reach, script, control_dir, stdin, as)` (:141), argv in this order: `-n` unless `stdin` is true; `-T`;
  `-o ControlMaster=auto`; `-o ControlPath={ssh_control_path(reach, control_dir)}`; `-o ControlPersist=60`; then
  `ssh_dial_args(reach)`; `user@host`; `bash`; `-c`; then for `login` the one word `shell_quote(script)`, else
  `shell_quote(root road)`, `wsp`, `shell_quote(script)`. `as` defaults to root; `control_dir` to
  `ssh_control_dir()`. No `-lc`, no `LogLevel`.
- `ssh_dial_args(reach)` (:166): `-o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -p
  {port}`, then `-i {key} -o IdentitiesOnly=yes` only with a key.
- `ssh_client` (:366): `as` is login for `AsLogin`, root-by-password where a password is given, else root. For
  root-by-password stdin is `password + "\n"` followed by the caller's stdin bytes (or none); otherwise the caller's
  stdin as given. Runs `child_exec::run_child("ssh", ssh_args(reach, script, make_ssh_control_dir(None), as,
  stdin.is_some()), { env: the process env, timeout_ms, on_line, stdin })`. The password never appears in argv.
  The control folder is made on every call, before the child starts.

## The person's config carried onto a forward child

- `CARRIED_SSH_OPTIONS` (:182), in this order, `ssh -G` name, option, shape: identityfile IdentityFile one;
  identitiesonly IdentitiesOnly one; certificatefile CertificateFile one; proxycommand ProxyCommand rest;
  userknownhostsfile UserKnownHostsFile rest; globalknownhostsfile GlobalKnownHostsFile rest; hostkeyalias
  HostKeyAlias one; checkhostip CheckHostIP one; hostkeyalgorithms HostKeyAlgorithms one;
  pubkeyacceptedalgorithms PubkeyAcceptedAlgorithms one; identityagent IdentityAgent one; addkeystoagent
  AddKeysToAgent one; usekeychain UseKeychain one.
- A `one` value with white space, `"`, `'` or `\` goes in double quotes with `"` and `\` backslashed; a `rest` value
  and a plain one go as printed (:200).
- `missing_known_hosts_line(target, file)` (:207): exact sentence with `target` cut to 48 UTF-16 units and `file` to
  72; never over 300. The error it is thrown in is `MissingKnownHosts` (its own variant, so a caller says it whole).
- `carried_ssh_values(reach, run)` (:232), `run` an `SshLocalRun` (ssh_keys):
  1. `ssh -G {dial args} user@host` with the config, then `ssh -G -F /dev/null {dial args} user@host` bare, each
     bounded 5,000 ms. Either exit not 0: the error is `ssh_refusal_line(said, reach)` (ssh's own line).
  2. Each table key in order: skip when its values joined by `\n` equal the bare read's. For a known hosts key,
     every value split on single spaces: the first word that is not empty, not `none`, and not an absolute path to
     an existing file refuses with `MissingKnownHosts` (a relative path counts as missing).
  3. IdentityAgent values have each `%` doubled. Each non-empty value pushes `{Option}={value}`.
  4. Then, when `proxycommand` is empty and `proxyjump` is neither empty nor `none`: `ProxyCommand=` the jump
     command: hops split on `,`, the last popped; `ssh -o BatchMode=yes [-J '{other hops joined by ,}'] -W
     '[%h]:%p' 'ssh://{last}'` (each word shell-quoted as shown).
  5. Reach: user = config `user` or the typed one; host = config `hostname` or the typed one; port = `Number(port)`
     or, where that is 0 or NaN, the typed port; key path kept. Key order user, host, port, keyPath.
  `readLists` keeps every value of a repeated key in order; a line with no space or a leading space is skipped.
- `SSH_BACK_UP = "WSP_BACK_UP"`; the forward's command is `echo WSP_BACK_UP; exec cat` (:263).
- `ssh_back_args(carried, box_port, door_port)` (:268): `-F /dev/null`, dial args of the carried reach, `-o {each
  option}`, `-T`, `-o ExitOnForwardFailure=yes`, `-o ServerAliveInterval=15`, `-o ServerAliveCountMax=3`, `-o
  ControlPath=none`, `-R 127.0.0.1:{box}:127.0.0.1:{door}`, `user@host`, the command. No `-N`, no
  ClearAllForwardings, one `-R`.

## The held forward

- `hold_back_forward(carried, box_port, door_port, spawn)` (:323) spawns `ssh` with those args once.
  - stdout and stderr are read as lines through `LineFeed` with `keep = 4096`; each line loses one trailing `\r`
    and is cut to 300 units. A partial line at the end of a stream is never read (nothing flushes it).
  - `up` resolves on the first stdout line whose JS-trim is `WSP_BACK_UP`. No stderr line stands for it.
  - The last 50 stderr lines are kept.
  - When the child exits (or fails to start), once, after its stderr has ended: the words are the last line of
    `client_words(kept lines joined by \n)`, or where that is empty the fallback (`ssh ended on {SIGNAL}` for an
    exit by signal with no code, `ssh exited with {code}` otherwise, the spawn error's message on a start
    failure), passed through `box_word(_, 300)`. `up` rejects with them (no effect once resolved) and `ended`
    resolves with them.
  - The host never writes to the child's stdin and keeps it open until `release()`.
  - `release()`: the first call closes stdin and sends SIGTERM; later calls do nothing.
  - A rejected `up` nobody awaits raises nothing.

## Pins

| Rule | Tests |
|---|---|
| parse: user@host, port, key, refusals | `user_at_host_names_the_dial_with_the_port_and_key_the_person_gave`, `parse_answers_what_the_ts_answered_for_every_address` |
| machine id written form | `the_machine_id_is_the_url_the_ts_writes` |
| name from address | `a_machine_with_no_name_given_is_called_what_its_address_calls_it`, `names_and_login_words_match_the_ts` |
| argv order, root road, no login shell | `the_client_runs_the_script_under_bash_c_never_a_login_shell`, `ssh_args_match_the_ts_byte_for_byte` |
| control path and folder | `every_command_to_one_machine_rides_one_master_connection`, `the_master_sockets_live_in_a_folder_only_the_person_can_read` |
| box_word cap and control chars | `box_word_strips_control_chars_and_cuts_by_utf16_units` |
| -n only off the stdin road | `keeps_the_connections_stdin_for_the_file` |
| sudo roads through the client | ssh_sudo.rs (7 cases) |
| carried options, quoting, %%, jump, known hosts | ssh_back_forward.rs (8 config cases), `carried_values_match_the_ts_for_every_config` |
| held forward up, ended, release, caps | ssh_back_forward.rs (8 holder cases) |

## Pins (wave 2)

Scenes in `oracle/roads_scenes.oracle.ts`, replayed by `tests/roads_scenes.rs::<name>` against
`tests/fixtures/scenes/roads.json`: each step is one write to the fake child's pipe (or its exit), with a pause
after it, and `up` and `ended` must say what the TS said.

- D-H1 A line is cut to 300 units from its head after the one trailing `\r` goes: a 1,000-unit stderr line written in
  one piece keeps its first 300 (`the_cut_keeps_a_lines_head`). The held text is cut to its last 4,096 units only
  when, after a read's complete lines are taken, more than 4,096 units are still waiting; so a 5,000-unit write with no
  newline keeps its last 4,096, and the line the next newline ends starts there
  (`a_flood_without_a_newline_keeps_only_its_last_4096_units`).
- D-H2 Only the last 50 stderr lines are kept, and `client_words` runs over those 50 alone: ten real lines followed by
  fifty of ssh's known-hosts note leave nothing, so the words are the fallback `ssh exited with 255`
  (`only_the_last_50_stderr_lines_are_kept`).
- D-H3 The words are the last line `client_words` keeps: debug lines and the known-hosts note after the real line are
  dropped (`the_last_line_client_words_keeps_is_the_words`).
- D-H4 `up` assembles a line across writes and trims it with JS trim after the one `\r` goes: `  WSP_BA` then
  `CK_UP \r\n` is up (`up_on_a_line_written_in_two_pieces_and_padded`).
- D-H5 More words on the line is not up, and an unended `WSP_BACK_UP` at the end of stdout is never read
  (`the_up_word_with_more_on_its_line_is_not_up`).
- D-H6 An exit with code 0 is `ssh exited with 0` (`an_exit_with_code_zero_says_so`); a start failure is the error's
  message, `up` rejected with it (`a_child_that_fails_to_start_says_the_error`).
- D-H7 Only one trailing `\r` goes with the line; `box_word` then strips every control character left, the tab and the
  other `\r` included: `bad\tword\r\r\n` says `badword`
  (`one_trailing_cr_goes_with_the_line_and_box_word_strips_the_rest`).
- D-H8 A stderr tail with no newline when the child ends is never a line: the words are the line before it
  (`a_stderr_tail_with_no_newline_is_never_read`).
- D-H9 A cut inside a surrogate pair writes U+FFFD for the half left, so `box_word` keeps exactly `cap` units
  (`a_cut_inside_an_emoji_writes_the_replacement_character_for_the_half`; the ruling in roads-README.md).
- D-H10 The TS settles on the child's `exit` event, which can come before its pipes have been read to the end; this
  spec has the holder read both pipes to their end before it settles (the fake closes them first), so a last line
  written just before the exit is always counted. A difference only in a race the TS loses.
