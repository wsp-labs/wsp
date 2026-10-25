# ssh_machine: root over a login, the read that adopts a machine, and the machine itself

Source: ssh-backend.ts:617-969 at `a4033dc34`. Module `ssh_backend::machine`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/ssh_machine.json`, `ssh_read.json`.

## Root over a login

- `SUDO_ASKS_LINE = "a password is required"`. `SshSudo`: root, free, taken, asks, wrong, tty, none;
  `SshSudoRoad` the five without taken and wrong. Words on the wire are the lowercase names.
- `SSH_SUDO_READ` and `SSH_SUDO_TRY` (:639, :648): exact text, pinned by the oracle. The read runs nothing as root
  (`sudo -n -v` then `sudo -n -l bash -c true`); the try reads the password off stdin with `-S -k -p ''`.
- `read_ssh_sudo(reach, transport)` (:652): the read as the login, bounded `SSH_DIAL_MS`. The answer is the first line
  of stdout that is exactly `WSP_SUDO {root|free|asks|tty|none}` (a mark answers whatever the exit code). No mark:
  exit 255 fails with `ssh_refusal_line`; else `"{user}@{host} did not answer over ssh: {tail}"` where tail is
  `client_words(stderr)`, or JS-trimmed stdout where that is empty, cut to its last 300 UTF-16 units.
- `try_ssh_sudo(reach, password, transport)` (:660): the try as the login, stdin `password + "\n"`, bounded
  `SSH_DIAL_MS`. In this order: stdout holds `WSP_SUDO taken` is taken; stderr holds `must have a tty` is tty;
  stderr holds `incorrect password attempt` is wrong; else none. The password is never in the script.

## The read that adopts a machine

- `SSH_STORE_VARS` (:670): each catalog agent's `state_home_env` in catalog order, kept where it matches
  `^[A-Z_][A-Z0-9_]*$` (today `CLAUDE_CONFIG_DIR`, `CODEX_HOME`).
- `LOGIN_READ`, `SSH_READ_SCRIPT` and `SSH_FACTS_SCRIPT` (:677, :687, :705): exact text, pinned by the oracle. The
  read opens no login file (`env -u BASH_ENV bash --noprofile --norc -c`); `SYSTEM_READ` comes before `ARCH_READ`;
  `SHELL_READ` reads passwd, then `$SHELL`, then `bash`.
- `read_ssh_machine(reach, transport)` (:709): the read as root (no ride of its own), bounded 30,000 ms.
  - Exit not 0: `"{user}@{host} did not answer over ssh: {tail}"` (tail as above).
  - `values = read_values(stdout)` (a repeated key keeps its last value).
  - `home` absent or not `is_plain_path`: `"{user}@{host} answered over ssh with {said} for {user}"`, where said
    is `no home folder` for absent or empty, else `{JSON of home}, which is not a plain path`. A space is plain
    (`/Users/John Smith`); `$(..)`, a backtick, `;`, a relative path are not.
  - Stores: each store var whose `store:{NAME}` value is present, non-empty and a plain path.
  - `cpu = Number(values.cpu or "0")`: a missing or empty value is 0, junk is NaN (JSON `null`, as the TS writes).
  - `mem_mb = mem_mb_of(values)` or 0. `system`, `arch` as `system_of`, `arch_of`; `shell` the raw value, absent
    when empty.
  - `login` key order: the kept stores in `SSH_STORE_VARS` order, then `HOME`, `USER` (`values.user`, the typed user
    only when absent; an empty answer stays empty), `PATH` (`plain_path(values.path)`).
  - Answer key order: login, shape `{ cpu, memMb }`, then system, arch, shell where present.
- `DEFAULT_REMOTE_PATH = "/usr/local/bin:/usr/bin:/bin"`. `plain_path(path)` (:764): split on `:`; keep a folder
  that is non-empty, `is_plain_path` and has no white space (`\s`); duplicates kept; nothing kept answers the
  default.
- `ssh_dials_this_computer(reach, names)` (:748): host lowercased with one trailing `.` removed; true for
  `localhost`, `127.0.0.1`, `::1`, `[::1]`, `0.0.0.0` or any `127.` prefix; else true where some non-empty name
  (lowercased, one trailing dot off) equals the host, or the host is `{name}.local`, or `{host}.local` is the name.

## Bytes over the connection

- `SSH_BYTES_OK = "WSP_BYTES_OK"`. `put_bytes_script(path, size, tmp)` (:777), lines: `set -e`, `umask 077`, `mkdir
  -p {quoted posix dirname}`, `cat > {tmp}`, the count check (`wc -c` with spaces removed equals size, else remove tmp,
  echo `WSP_BYTES_SHORT`, exit 1), `mv -f {tmp} {path}`, `echo WSP_BYTES_OK`. posix dirname: `file` is `.`, `/f` is
  `/`, trailing slashes ignored.

## `SshMachine`

- `id = ssh_machine_id(reach)`, kind sandbox, no stream URL, no labels; offers `put_bytes`, `describe`, `facts`
  and nothing else (no preview, no daemon frame, no framing). `takes_stdin` false.
- `exec(cmd, opts)`: the transport with `timeout_ms` only; the idempotency key and stdin are ignored. Rides as the
  backend rides.
- `run(script, opts)`: the transport with `timeout_ms = deadline_ms` and `on_line`; poll, signal, launch_on,
  unlogged ignored (the 124 at the deadline is `run_child`'s).
- `snapshot`, `pause`, `resume` fail with `a machine reached over ssh cannot be {snapshotted|paused|resumed}`;
  `download_url`/`upload_url` with `a machine reached over ssh serves no signed {download|upload} URL; its files are
  {read|written} over the connection`. `kill` answers Ok and sends nothing. `state` is running and sends nothing.
- `describe()`: `read_ssh_machine(...).shape`.
- `facts()` (:844): `SSH_FACTS_SCRIPT` bounded 10,000 ms; values empty unless exit 0. `os = os_name_of`, `uptime_ms =
  uptime_ms_of(values, now)`, `folder = values.home`. Any missing (folder empty counts) fails with `"{user}@{host}
  did not say what it is over ssh (exit {code}): {tail}"`; the same words go to the warning sink only when they
  differ from the last words warned for this machine, and are then held. A success clears the held words, so the
  next failure warns again. Nothing else is held: every call reads the machine. The held words do not survive a
  restart. The warning sink defaults to stderr; `SshMachine::with_warn` lets a test hand its own.
- `put_bytes(path, bytes, timeout)` (:872): `tmp = "{path}.wsp-in-{12 hex}"`; the transport runs
  `put_bytes_script(path, len, tmp)` with the bytes as stdin. Exit not 0, or stdout without `WSP_BYTES_OK`: `"{len}
  bytes did not land at {path} over ssh (exit {code}): {tail}"`. Answers `None` (no pieces).

## `SshBackend`

- Options: transport (default `ssh_client`), host key (default `known_host_key`), known hosts (default
  `known_hosts_written`), offered key (default `offered_host_key`), host name (default `ssh_host_name`).
- `sudo_for` and `sudo_try` are `read_ssh_sudo` and `try_ssh_sudo` on the backend's transport.
- `riding(r)` (:927): a new backend with every other seam kept and a transport that passes a read as the login
  through untouched; otherwise rides as the login for `AsLogin`, or as root with `r`'s password. A backend made by
  riding a riding backend keeps the first password (the inner wrapper runs last). The backend ridden from is
  unchanged.
- `adopt(reach)` (:937): `read_ssh_machine` first; its failure ends the call with no key read. Then `key_for(reach)`.
  Answer: the machine (on this backend's transport, so it keeps riding), login, shape, then system, arch, shell,
  host key where present.
- `key_for`, `known_hosts_entry`, `offered_key_for`, `host_name_for` delegate to their seams.

## Pins

| Rule | Tests |
|---|---|
| adopt dials once, login and size | `adopt_dials_once_reads_the_machines_own_login_and_size` |
| PATH rule | `holds_each_folder_of_the_machines_own_path_to_the_rule`, `plain_path_matches_the_ts` |
| identity from the client's entry, none invented | `a_dial_that_exchanged_no_key_still_hands_back_the_machines_identity`, `a_machine_the_client_holds_no_key_for_is_recorded_without_an_identity` |
| home refusal | `the_machines_own_words_for_its_home_are_held_to_a_plain_path` |
| stores and login order | `the_store_folder_each_harness_reads_is_asked_of_the_login_shell`, `read_answers_match_the_ts_with_login_key_order` |
| no login file opened, shell read | `opens_no_login_file_of_the_machines_own`, `reads_which_login_shell_the_machines_root_runs` |
| exec, run, refusals | `exec_and_run_carry_the_script_to_the_machine`, `the_moves_only_a_machine_wsp_forks_takes_are_refused` |
| arch and system | `carries_the_arch_word_back_whatever_it_is`, `answers_no_arch_where_the_machine_printed_none`, `asks_uname_s_ahead_of_the_chip` |
| facts read every time, warn once per change | `the_three_the_pane_waits_on_are_read_off_the_machine_itself`, `a_mac_over_ssh_says_when_it_booted`, `says_why_the_rows_sit_at_pending_once`, `a_machine_that_did_not_answer_leaves_the_rows_waiting`, `a_success_clears_the_held_warning` |
| scripts byte for byte | `scripts_match_the_ts_byte_for_byte` |
| sudo read and try, riding | ssh_sudo.rs: `reads_the_road_as_the_login`, `says_sshs_own_line_where_the_login_does_not_stand`, `a_backend_riding_a_password_hands_it_to_every_script`, `reads_root_free_asks_tty_none` |
| bytes over stdin | ssh_bytes.rs (4) |
| this computer | `dials_this_computer_matches_the_ts` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `ssh_machine`), held by
`tests/ssh_machine_pins.rs`.

- M-1 The landing's folder is `posix.dirname`: `a//b` makes `mkdir -p 'a/'`, `a/b//c` makes `'a/b/'`, `//a` makes
  `'//'`, `///a/b` makes `'///a'`, `/a//` makes `'/'`, `a/`, `./a` and `..` make `'.'`, `a/b/` makes `'a'`
  (`the_landing_folder_is_posix_dirname`).
- M-2 Exactly one trailing dot is stripped before the loopback test: `localhost..` is not this computer, `[::1].`,
  `LOCALHOST.`, `127.foo.example`, `::1` and `0.0.0.0` are (`one_trailing_dot_is_stripped`).
- M-3 The sudo mark is `^WSP_SUDO (root|free|asks|tty|none)$` under JS `m`, where a line ends at `\n`, `\r`, U+2028 or
  U+2029: `a\r\nWSP_SUDO root\r\nWSP_SUDO none` is root (the first), `x` U+2028 `WSP_SUDO free` is free, a mark inside a
  line or with a trailing space is no mark and fails `{user}@{host} did not answer over ssh: {stdout trimmed}`
  (`the_sudo_mark_is_read_with_javascript_line_rules`).
- M-4 A ride as the login beats a password in either nesting order: `riding(password).riding(AsLogin)` and
  `riding(AsLogin).riding(password)` both carry every script as the login, since the outer wrapper runs first and a
  script already marked as the login passes every inner one untouched (`as_the_login_beats_a_password_in_either_nesting`).
