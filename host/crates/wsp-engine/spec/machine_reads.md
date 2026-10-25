# machine_reads: who a line runs as, bytes onto a machine, disk sync, previews, daemon targets

Source at `a4033dc34`: `packages/engine/src/target-line.ts` (129), `daemon-targets.ts` (33), `land-bytes.ts` (39),
`disk-sync.ts` (59), `preview.ts` (36). Rust: `src/target_line.rs`, `src/daemon_targets.rs`, `src/land_bytes.rs`,
`src/disk_sync.rs`, `src/preview.rs`. Tests: `tests/machine_reads.rs`, hard tests in `tests/machine_reads_hard.rs`.
Oracle: `login.json`, `small.json` (`diskUse`).

## target-line.ts

- V1. `target_login` sends one exec, `timeout_ms` 20000, of the probe joined by `; `: `uname -s`, `id -u`, `id -un`,
  `stat -L -c %U <home> 2>/dev/null || stat -L -f %Su <home> 2>/dev/null || echo`, `command -v runuser >/dev/null 2>&1
  && echo 1 || echo 0`, `printf '%s\n' "$HOME" "$PATH"`, where `<home>` is `shell_quote(given.home)` or `"$HOME"`.
  Oracle `probes` (three, byte for byte).
- V2. Non-zero exit: `Other("the computer did not say who its lines run as: <first line of (stderr or stdout).trim()>")`
  (empty where there is no line). The seven stdout lines (split on `\n`, each trimmed, missing ones empty): system, uid,
  self, owner, runuser, shell home, shell path. platform `platform_of_system(system)` else Linux; user = owner, else self;
  home = given else shell home; path = given else shell path, absent when empty.
- V3. Not Linux, or uid not `0`: answered as read. Root: owner empty fails `no_home_refusal(home)`; user `root` is
  answered as read; runuser not `1` fails `no_runuser_refusal(user)`; else `run_as = user`.
- V4. `stores_reached`: all stores where `run_as` is `None`; else only those `under_project(store, home)`, map order.
- V5. `as_login`: `export HOME=<q home>[ PATH=<q path>]; cd "$HOME" 2>/dev/null; <line>`; with `run_as`, `runuser -u
  <q user> -- bash -c <q inner>`. Oracle `asLogin`.
- V6. `login_shell_path`: Linux only. The line `s=$(getent passwd <q user> | cut -d: -f7); timeout 15 "${s:-/bin/sh}"
  -ilc 'printf %s "$PATH"' < /dev/null 2> /dev/null` through `as_login`, `timeout_ms` 20000 (oracle `loginShellCmd`).
  A failed exec, a non-zero exit, or an empty last line is `None`; else the last stdout line trimmed (a greeting before
  it is dropped).
- V7. `stage_as_login`: dir `/tmp/wsp-land-<12 lowercase hex>`; `mkdir -m 0700 <q dir>`; failing:
  `no private folder to stage <what> in: <first>` and nothing else runs (no rm). Then `land_bytes(land, <dir>/bytes,
  bytes, {timeout_ms})`; then `chmod 0600 <q file>` plus ` && chown -R -- <q run_as> <q dir>` where `run_as` is set;
  failing: `the staging folder for <what> could not be handed to <user>: <first>`. Then `use_(file, dir)`. After the
  folder was made, `rm -rf -- <q dir>` runs whatever happened (its failure ignored) and the answer or the first failure
  stands. `first` is the first line of `(stderr or stdout).trim()`, else `exit <code>`. Every exec carries `timeout_ms`
  only where given.
- V8. `land_as_login`: as V7 with `what = dest`, then as the login `mkdir -p <q dest> && tar -xzf <q file> -C <q dest>`
  (unpack) or `mkdir -p <q dirname(dest)> && cat <q file> > <q dest>`; non-zero: `<dest> was not written as <user>:
  <first>`.

## daemon-targets.ts, land-bytes.ts, preview.ts

- V9. `DAEMON_TARGETS` the four rows in order; `platform_of_system` exact match of `system` (Linux, Darwin), else `None`.
- V10. `has_byte_road` = `offers().put_bytes`; `lands_bytes` = `capabilities.signed_urls || has_byte_road`.
- V11. `land_bytes`: a byte road is used first, `put_bytes(path, bytes, timeout_ms)`, its answer passed back as is; else
  `upload_url(path)`, one `PUT` of the bytes with no headers through `opts.fetch` (the platform's when `None`), a
  status outside 200..300 is `<path> did not land on <id>: HTTP <status>`; answers `None`.
- V12. `preview_is_fresh`: `expires_at - now > 600000`, strictly. `refresh_preview_token`: the current reach when fresh
  (nothing called); a machine without `preview_url` fails `machine <id> is on a backend without preview URLs`; else
  `preview_url(port)`. `DAEMON_PORT` 7070, `DAEMON_LISTENING_CHECK` `(exec 3<>/dev/tcp/127.0.0.1/7070) 2>/dev/null`.

## disk-sync.ts

- V13. `sync_disk`: one exec of `DISK_SYNC_CMD`, `timeout_ms` exec-detached's `INLINE_EXEC_MS` 20000. A failed exec is
  `Exec(e)` as it came. Non-zero: `Sync { answer: machine_answer(res), line: disk_sync_failed_line(answer) }`, where
  `machine_answer` is `it printed nothing and exited <code>` or `it exited <code> and said: <stdout and stderr trimmed,
  non-empty ones joined by a space>`. Exit 0: each counter is the digits of the first line (multiline) matching
  `^Dirty: (\d+)$` / `^Writeback: (\d+)$`; either missing is `None`.
- V14. `disk_use_of(stdout)`: the first line of the trimmed text matching `^(\d+) (\d+)$`; `used * 1024`, `free *
  1024`, `pct = used * 100 / (used + free)`; both zero is `None`. Oracle `diskUse` (8 cases: `1  2` and `7 8 9` none).
  `disk_unsettled`: some and either counter > 0.

## Restart

Nothing is kept; every read asks the machine again.

## Pinned since (K-rules, values from running the TS)

- K1. A failure's words take `(stderr || stdout)` before the trim: a blank stderr wins and names nothing; the first
  `\n` line keeps a `\r` and inner trailing spaces. `first_line` falls to `exit <code>` only when that line is empty.
- K2. `dirname(dest)` is node's `posix.dirname`: trailing slashes cut, then cut at the last `/` with nothing collapsed
  (`a//b` is `a/`, `/a/./b` is `/a/.`), `.` where no slash is left (`plain.txt`, `x/`, `""`), `/` or `//` for a root
  cut. `Path::parent` differs on four of these.
- K3. `stores_reached` with a login handed to keeps a store equal to the home or starting `<home>/`, by plain string,
  in map order (`/home/ada/../x` is kept; a home with a trailing slash reaches only stores under `<home>//`); with
  none handed to, every store. `under_project("", "")` holds, so an empty home reaches an empty store.
- K4. `login_shell_path` takes the last `\n` line, then trims: `/usr/bin\r` is `/usr/bin`, `/usr/bin\r\n` is `None`.
- K5. disk-sync's multiline `^`/`$` break at `\n`, `\r`, U+2028 and U+2029 (the `regex` crate's `(?m)` knows only
  `\n`); `\d` is ASCII (`(?-u:\d)` or `[0-9]`); the first match wins; a counter is `Number(...)`, so one past f64's
  range (400 nines) is no reading. `disk_use_of` takes the first line that is exactly two ASCII numbers.
- K6. `preview_is_fresh` is `expires_at - now > 600000` strictly; `refresh_preview_token` answers a fresh `current` as
  it is without asking (a machine with no preview road included) and refuses a stale or absent one there with
  `machine <id> is on a backend without preview URLs`.
- Also read (not pinned here): `land_bytes` lands on 200 to 299 only (300 to 399 fail `<path> did not land on <id>:
  HTTP <status>`); `timeout_ms: Some(0)` is forwarded as 0 to every exec and to `put_bytes`.

## Rules and their tests

Files: `machine_reads.rs` (ported), `machine_reads_hard.rs` (hard).

| Rule | Tests |
|---|---|
| K1 | machine_reads_more.rs: `a_failure_names_the_first_line_of_stderr_chosen_before_the_trim` |
| K2 | machine_reads_more.rs: `the_landing_line_makes_nodes_dirname_of_the_dest` |
| K3 | machine_reads_more.rs: `the_stores_a_login_reaches_are_a_plain_prefix_test` |
| K4 | machine_reads_more.rs: `the_login_shells_path_is_the_last_line_so_crlf_reads_none` |
| K5 | machine_reads_more.rs: `disk_sync_breaks_lines_as_js_does` |
| K6 | machine_reads_more.rs: `the_preview_boundary_is_strict_and_a_fresh_reach_needs_no_road` |
| V1, V2 | hard `probes_write_the_ts_lines_byte_for_byte`; ported `reads_the_owner_of_the_folder_a_linked_home_points_at_not_of_the_link` |
| V3 | ported `a_root_daemon_hands_every_line_to_the_owner_of_the_home_never_to_root_with_the_home_and_path_reported`, `a_mac_a_login_that_is_not_root_and_a_fork_whose_home_is_roots_own_run_the_line_as_it_is`, `refuses_to_read_at_all_where_the_road_is_root_the_home_is_somebody_elses_and_there_is_no_runuser`, `refuses_where_a_root_road_names_a_home_that_is_not_there_rather_than_running_as_root` |
| V4, V5 | ported `the_wrapped_line_runs_in_bash_as_the_logins_own_quoting_and_all`; hard `as_login_writes_the_ts_lines` |
| V6 | hard `login_shell_path_takes_the_last_line_and_nothing_on_failure` |
| V7, V8 | ported `stages_the_bytes_in_a_folder_only_the_login_can_open_writes_them_as_the_login_and_takes_the_folder_away`, `takes_away_no_folder_it_did_not_make_a_name_already_taken_refuses_the_landing_and_leaves_that_folder_be`, `unpacks_a_folder_as_the_login_and_a_write_the_login_is_refused_still_takes_the_staging_folder_away`, `writes_a_real_file_through_the_line_it_builds_keeping_the_mode_a_file_there_already_had`; hard `staging_removes_its_folder_whatever_happened` |
| V9 | ported `reads_nodes_platform_off_the_target_tables_own_rows_and_nothing_for_a_system_no_row_has` |
| V10, V11 | ported `takes_the_backends_own_road_when_it_has_one_with_the_callers_own_bound_on_it`, `puts_the_bytes_at_the_signed_url_when_that_is_the_only_road`, `names_the_file_and_the_machine_when_the_signed_put_is_refused`, `says_whether_bytes_can_land_at_all_so_nothing_offers_a_daemon_where_they_could_not` |
| V12 | ported `keeps_a_fresh_reach_without_minting`, `remints_when_under_10_minutes_remain_older_than_about_50_min`, `refuses_on_a_backend_without_preview_support` |
| V13 | ported `runs_the_one_sync_command_under_the_inline_bound_and_answers_both_counters_at_zero_as_settled`, `answers_what_a_writer_still_running_left_once_and_never_waits_for_it_to_stop`, `a_sync_that_exits_non_zero_throws_the_typed_failure_with_the_machines_answer_in_the_sentence`, `a_guest_that_synced_and_printed_no_counters_has_synced_no_reading_nothing_unsettled`, `an_exec_the_provider_refused_passes_as_that_refusal_kind_and_status_intact`, `the_command_runs_on_this_linux_machines_bash_and_prints_both_counters_in_the_shape_the_reader_parses`; hard `sync_disk_reads_the_counters_and_says_what_a_failed_sync_said` |
| V14 | ported `is_the_share_used_unrounded_off_dfs_used_and_free_counts_and_nothing_for_a_reading_that_is_not_two_counts`, `reads_this_computers_own_root_the_way_the_guests_is_read`; hard `disk_use_matches_the_ts_oracle` |
