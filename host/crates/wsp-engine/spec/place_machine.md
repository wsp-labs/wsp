# place_machine: the computer itself as a machine

Source: place-machine.ts at `a4033dc34`. Module `place_machine`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/place_machine.json` (frames byte for byte, random names pinned to `ab`).

## Constants

- `NOT_A_WORKSPACE = "this is the computer itself, not a workspace on it"`.
- `PLACE_PART_BYTES = 1_048_576`; `LINK_BYTES_PER_MS = 213` (private).
- `place_part_bound_ms(bytes, floor = 20_000)` (:33): `min(max(2 * ceil(base64_length(bytes) / 213), floor),
  600_000)`. 1 MiB is 20,000; 4 MiB is 52,512; 64 MiB is 600,000; 5,000,000 is 62,598; with floor 1, 0 bytes is 1
  and 1 byte is 2.

## `PlaceMachine` (:58)

- `new(link, id, home)`; the link is only `request`. Kind sandbox, `takes_stdin` true; offers `put_bytes` only.
- `exec(cmd, opts)` (:75): `timeout = min(opts.timeout_ms or 20,000, 600,000)`; frame `exec { cmd, timeoutMs, stdin? }`
  (stdin base64, absent without bytes), ask `{ timeoutMs: timeout + 5,000, idempotencyKey? }`. The reply is parsed as
  `DaemonExecReply` (a missing field is an error); answers exit code, stdout, stderr (`truncated` dropped).
- `run(script, opts)`: `exec_detached(self, script, opts, place_provision_paths(home).run_dir)`
  (`/root/.wsp/provision/run` for home `/root`).
- `put_bytes(path, bytes, timeout)` (:98):
  - parts = max(1, ceil(len / 1 MiB)); `names = '{path}'.part{0..N-1}`; `put` = 12 hex.
  - Part i: `exec("umask 077\nmkdir -p '{dirname}'\ncat > '{path}'.part{i}", { timeout: place_part_bound_ms(part
    len, timeout), key: "{put}/{i}", stdin: part })`. Exit not 0: `part {i+1} of {N} did not land at {path} on {id}:
    {machine_answer}`.
  - Join: `exec("umask 077; if [ -e '{path}'.part0 ]; then cat {names} > '{path}' && rm -f {names}; fi", { key:
    "{put}/join", timeout where given })` (the default 20,000 otherwise). Exit not 0: `{path} did not land on {id}:
    {machine_answer}`.
  - Any failure (an answer or a rejected frame): `rm -f {names}` with timeout 20,000, its own failure ignored, then
    the first failure.
  - Answers `pieces = N`. Sent twice, every frame lands the same bytes once; every part and the file are 0600 under a
    022 mask.
- `state()` answers running and sends nothing. `snapshot`, `pause`, `resume`, `kill`, `download_url`, `upload_url`
  fail with `NOT_A_WORKSPACE` and send nothing.

## `PlaceFolderMachine` (:160)

- `new(link, id, home, path)`. `given = { home, path }`.
- `login_of()` (:174): `target_login(base exec, given)`, then `login_shell_path(base exec, login)` replacing `path`
  where it answers one. Shared by every caller while in flight and held once read; a failure is not held (the next
  call reads again). Held in memory only. Its two reads go through the base `PlaceMachine::exec`, unwrapped.
- `kill()` answers Ok and sends nothing.
- `framed()`: `wrap(cmd) = as_login(login, cmd)`; `no_room = login_path_refusal(login.path's UTF-16 length)` where
  the login has a path. Offers `framed`, `put_bytes`, `daemon_frame`.
- `exec(cmd, opts)`: the base exec of `framed().wrap(cmd)`; a login read that fails fails the exec.
- `in_cgroup(cgroup, path_first)` (:208): a machine sharing this one's login and link whose `framed` wraps `cmd` as
  `{cgroup_join_line(cgroup)}\n{as_login(login, "{path}{cmd}")}` and whose `exec` is the base exec of that wrap. With
  `path_first`, `{path}` is the shell start line then `export PATH='{path_first}':"$PATH"; `; without, empty. The
  shell start line: `{ mkdir -p '{dir}' && {each thread_shell_files(dir, empty env) file written as printf %s '{text}' >
  '{dir}/{name}'.$$ && mv -f '{dir}/{name}'.$$ '{dir}/{name}' joined by " && "}; } 2>/dev/null && export {each
  thread_shell_vars(dir) as K='v' joined by space}; `. Every other call (run, put_bytes, state, kill, daemon_frame,
  id) is this machine's own, so `run` and `put_bytes` on the grouped machine go through the grouped exec (the TS
  `Object.create(this)` with `exec` and `framed` replaced).
- `daemon_frame(frame)` (:223): `op` out; the rest sent as that op with no `machineId` and no ask options; `{ ok: true,
  ...reply }` or `{ ok: false, error, code? }`, never an error.

## `place_folder_backend(machine, shape)` (:236)

Capabilities: every flag false except `kept` true; no pauseMode; sizes empty; key order as `local_backend`'s with
copies false. Pricing: rate 0, default size `shape`, storage `{ 0, 0, "" }`. `create` and `delete_snapshot` fail
with `NOT_A_WORKSPACE`; `get(_)` answers `machine()`; `list` answers empty. No lifecycle, no offers.

## Pins

| Rule | Tests |
|---|---|
| exec frame, margin, cap, key, stdin | `rides_the_daemons_own_exec_frame`, `holds_the_wait_under_the_cap_the_daemon_holds_its_own_timer_to`, `carries_the_bytes_for_the_commands_own_stdin_on_the_frame` |
| run under wsp's folder | `is_launched_under_wsps_own_folder_on_that_computer` |
| parts on stdin, order, base64 once | `cross_in_parts_each_one_frame`, `cross_as_base64_once_not_as_base64_of_base64` |
| part bound | `bound_each_frame_by_the_bytes_it_carries`, `part_bounds_match_the_ts` |
| failure cleanup names every part | `leave_nothing_of_the_file_on_the_computer_where_a_frame_failed`, `a_rejected_frame_also_cleans_up` |
| frames sent twice land once; modes 0600 | `land_the_same_bytes_once_where_the_link_lost_an_answer`, `are_its_logins_alone_in_every_part_and_in_the_file` |
| workspace calls refused, nothing sent | `are_refused_in_one_sentence_rather_than_sent_as_a_frame` |
| login PATH leaves no room, three roads at -1/0/+1 | `refuses_<road>_at_<offset>` (9), `names_the_path_where_it_is_merely_too_long` |
| frames byte for byte, cgroup and shell start line | `folder_and_cgroup_frames_match_the_ts` |
| login read once, a failure read again | `the_login_is_read_once_and_a_failed_read_is_read_again` |
| folder backend | `the_folder_backend_is_the_computer_as_one_machine` |
| daemon frame names no machine | `a_daemon_frame_goes_up_naming_no_machine` |

## Pins (wave 2)

Scenes in `oracle/roads_scenes.oracle.ts` (`places`), replayed by `tests/place_machine_scenes.rs::<name>`: every
frame byte for byte, the upload id as the TS's pinned one.

- PL-1 No bytes are one part (`max(1, ceil(0 / 1 MiB))`): its frame carries `"stdin":""`, then the join, and the
  answer is one piece (`no_bytes_are_one_part_with_an_empty_stdin`). The "absent without bytes" above means absent
  where the caller gave no stdin, not where it gave an empty one.
- PL-2 A folder machine does not override `put_bytes`, so every frame of an upload goes through its `exec`: each part,
  the join and the cleanup after a refused part is wrapped as the login (`export HOME=... PATH=...; cd "$HOME"
  2>/dev/null; ...`), the stdin never; the login is read first, once
  (`a_folder_machines_parts_join_and_cleanup_ride_as_the_login`).
- PL-3 `path_first` is tested against `None` only: `Some("")` writes the four zsh startup files and `.bash_env` under
  `/` (`'/.zshenv'` and the rest) and puts `''` in front of PATH, as the TS does
  (`an_empty_path_first_is_a_path_first`). After-port ticket: an empty folder should be no folder.
- PL-4 `posix.dirname` names the parts' folder: the same table as exec_put.md's U-2 (`a.txt` is `.`, `a//b/c` is
  `a//b`, `a/b/` is `a`).
