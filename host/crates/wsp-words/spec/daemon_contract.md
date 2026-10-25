# daemon_contract
Source: the functions of `packages/protocol/src/daemon-contract.ts` (403 lines) at a3b870a6e; its flat constants (the paths `GUEST_WSP_HOME`, `DAEMON_TOKEN_PATH`, `TOOLS_PATH`, `PLACE_WORKSPACE_PATH`, `RUNTIME_FOLDERS`, ..., the caps `PRE_AUTH_MAX_BYTES`, `FS_READ_CAP_BYTES`, `GUEST_FRAME_CAP_BYTES`, ..., the fixed sentences `DAEMON_TOKEN_REFUSED`, `PIPE_POOL_FULL`, `GUEST_UNWATCHED`, `PLACE_LOGIN_LINKED`, `NO_REMOTE`, ..., and the set `CACHE_DIRS`) are `wsp_wire::types::daemon_contract`. Tests: `packages/protocol/test/daemon-contract.test.ts` (9 of the 11 cases of `describe("the words and numbers are what this package exports")` ported in `tests/daemon_contract.rs`; its `words()` and `numbers()` tables, :315-:487, ported as the helpers that build words.json and numbers.json). Skipped: :267-:313 (each op's and event's accept and reject frames: zod and fixture files, wsp-wire's), :1027-:1046 (the replies: schemas and samples, wsp-wire's), :1116 (constants alone), :1121 (reads the daemon's Rust source: a law test).

## What it does
The words and numbers the daemon and every client of it must read alike: the sentences a socket is closed or refused with, the lines the link of a joined computer logs, the lines a leave and a bring back say, the wsp shim a machine carries, and the memory one workspace may take. The fixture set under daemon/fixtures/contract is these values rendered with `{name}` holes, and the daemon is held to it.

## Behaviour
1. `guest_wsp_shim(binary)`: "#!/bin/sh\nexec <binary> wsp \"$@\"\n", the binary put in as given, unquoted (daemon-contract.ts:86).
2. `daemon_refused_token(code, reason)`: true where `code` is exactly 4401 and `reason` is anything but DAEMON_AUTH_DEADLINE_PASSED, an unknown reason included; any other code is false (:213).
3. `unknown_op_line(op)` is "unknown op: <op>" (:218); `port_scope_refusal(port)` (:230); `daemon_listening_line(host, port)` is "wsp-daemon listening on <host>:<port>" (:266).
4. `box_full_line(need_mb, free_mb, quietest)` (:222-225): with no quietest, the "no workspace of yours is awake to stop" sentence; with one, "...; stop <name>, quiet for <quietMin> min, to make room". Free is said before need, though need is the first argument.
5. `workspace_mem_mb(total_mb)`: `max(1024, min(floor(total_mb / 3), total_mb - 1024))` (:228).
6. `folders_outside_line(dir, roots, on)`: "<dir> is outside the folders wsp browses on <on>: <roots>", `on` "that computer" where none is given (:264).
7. The link lines of a joined computer (:288-301): `linked_line`, `host_quiet_line`, `dial_unanswered_line`, `dial_timed_out_line`, `dial_failed_line`, `not_a_frame_line`, `auth_unreadable_line`, `host_refused_line` ("<url>: <refusal>"), `link_host_unsealed_line`, `link_out_of_order_line`; each puts its values in as given.
8. The leave lines (:320-377): `place_kept_for_link_line`, `place_line_out_line`, `place_line_stays_line`, `place_login_unread_line` (ends in PLACE_LOGIN_LINKED), `place_outside_left_line` (names `<prefix>/landed`), `place_kept_mounted_line`, `place_kept_mounts_unread_line`, `place_runtime_stands_line`, `place_cgroup_stands_line`, `place_unread_line`, `no_change_line`, `place_stood_before_line`. `place_leave_unsaved_line(lines)` joins its lines with "; " (:362); `place_owners_unknown_line(paths)` joins its paths with " or " (:376-377).
9. `guest_no_daemon_line(port)` names 127.0.0.1:<port> (:382).
10. The bring back lines (:386-402): `on_base_refusal(base)`, `nothing_ahead_line(branch, base)`, `no_host_cli_line(host)`, and `no_git_credential_line(host, fix)`, which ends "so nothing was pushed" with no fix and adds "; <fix>, then bring back again" with one.

## Edge cases
- `workspace_mem_mb`: 1000, 1500, 2048 and 3072 are 1024; 3500 is 1166; 6000 is 2000; 16384 is 5461 (node, over the TS body).
- `place_leave_unsaved_line(&[])` is "nothing was removed: ; that work is on this computer alone": an empty join is "" (read off :362).
- words.json holds `folders_outside_line("{dir}", "{roots}", None)` as "{dir} is outside the folders wsp browses on that computer: {roots}", and the four 4401 reasons as four different sentences (test).

## Behaviour nobody wrote down
- Rule 8.11: every TS `number | string` value (ports, megabytes, seconds, minutes) is `&str` here, so a caller with a number writes it through `number_string` first.
- `Math.max` and `Math.min` answer NaN where any argument is NaN, where Rust's `f64::max` and `f64::min` drop the NaN: `workspace_mem_mb(NaN)` is NaN in the TS (node) and 1024 under a naive port.

## Tapes
The fixture files daemon/fixtures/contract/words.json and guest-wsp-shim.sh, which the Rust daemon is tested against; the close reasons of a daemon socket (4401); the log lines of a joined computer's link to its host; the lines `wsp remove` prints for a leave; the refusal of a create on a computer with no room (packages/runtime/src/runtime.ts); the bring back refusals.

## Decisions
1. Each TS `number | string` parameter (`port`, `seconds`, `need_mb`, `free_mb`, `quiet_min`) is `&str`, so a `{port}` hole and a number take one road.
2. The TS `quietest?: { name; quietMin }` is `Option<&Quietest>`, a struct with `name` and `quiet_min` as Strings.
3. The TS default parameter `on = "that computer"` is `Option<&str>`, None reading as the default.
4. `daemon_refused_token` takes `code: f64` and `workspace_mem_mb` takes and answers `f64`, the TS number.
5. `readonly string[]` parameters are `&[impl AsRef<str>]`.

## After the port
- `workspace_mem_mb`'s doc says "a third, never the last gigabyte, and never under a gigabyte", but on a computer under 2048 MB the 1024 floor wins over "never the last gigabyte": 1500 MB answers 1024, leaving 476 MB. The `total_mb - 1024` term never changes the answer at any size, since wherever it is the smaller of the two, both are under 1024.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_words_and_numbers_are_what_this_package_exports::guest_wsp_shim_sh_equals_its_regeneration_so_the_word_inside_a_fork_and_inside_a` |
| 2 | none: no test calls `daemon_refused_token` |
| 3 | `the_words_and_numbers_are_what_this_package_exports::words_json_equals_its_regeneration`, `the_words_and_numbers_are_what_this_package_exports::holds_every_4401_reason_once_and_the_listening_line_names_a_host_and_a_port` |
| 4 | `the_words_and_numbers_are_what_this_package_exports::words_json_equals_its_regeneration` (both forms) |
| 5 | none: no test calls `workspace_mem_mb` |
| 6, 7, 8, 9, 10 | `the_words_and_numbers_are_what_this_package_exports::words_json_equals_its_regeneration` (each with one-item lists for the two joins) |
