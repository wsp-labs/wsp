# base (golden-base.ts, all 192 lines)

The floor every machine gets before the picks: one guarded step per catalog floor row by its own road, the login
shell's PATH file, the ssh server, and the versions read back. Everything it leaves is on the machine; nothing
survives a host restart on the host (the next run's version read finds what stands).

## The plan (:14-83)

- `profile_path_line(path)`: `printf '%s\n' <shell_quote(path_line(path))> > /etc/profile.d/wsp-golden.sh`, the
  `path_line` with no prefix. `PROFILE_PATH_LINE` is it at `TOOLS_PATH`.
- Every step's `cmd` is `PRELUDE`, a newline, its script (`withEnv`). Ids are `base/<entry id>`.
- `base_installs(carried, path, prefix, left)`, in this order:
  1. `base/login-path`, label `login shell PATH`, manager `script`, cmd `withEnv(path_line(path, prefix) + "\n" +
     PROFILE_PATH_LINE)` (the file always gets the tools PATH, whatever `path` the job runs on), shown `the tools PATH
     in /etc/profile.d/wsp-golden.sh`. Always there.
  2. Each `base_floor()` row in catalog order, unless `carried` or `left` names it. It waits on `install_after(e)`
     unless `carried` names that row (what it needed is there). Before the first row that waits on `apt-index`, the
     index step: `apt_index_step("base/apt-index", withEnv(path_line(path, prefix) + "\n" + APT_UPDATE))`, once. The
     row's step: `via_road(e.install_road, e.bin, path, prefix)`; a note panics with `<id>: <note>` (the TS throws;
     no catalog row does it). Fields: label `e.name`, manager the road's name, `cmd` `withEnv(step.cmd)`, `shown`
     `step.shown`, `after` `base/<waits>` where it waits, `bin` `e.bin`. A row whose dependency is `left` but not
     carried still waits on it (the loop then skips it as not installed).
  3. Unless `carried` names `ssh-server`: `base/ssh-server`, label `OpenSSH server`, manager `apt`, check `test -x
     /usr/sbin/sshd`, shown `apt-get install openssh-server`, `after` `base/apt-index` only when the plan has the index
     step, cmd `withEnv` of the lines: `path_line(path, prefix)`; `if test -x /usr/sbin/sshd; then exit 0; fi`;
     `APT_UPDATE` only when the plan has no index step; the policy-rc.d guard (`if [ ! -e /usr/sbin/policy-rc.d ]; then
     printf '#!/bin/sh\nexit 101\n' >/usr/sbin/policy-rc.d; chmod 755 /usr/sbin/policy-rc.d; trap 'rm -f
     /usr/sbin/policy-rc.d' EXIT; fi`); `apt-get install -y -qq openssh-server`; `(systemctl disable --now ssh.socket
     ssh.service >/dev/null 2>&1 || true)`.

## The versions (:85-155)

- The checks: per floor row its bin with `smoke_of(e)` and its `brings` (bin, version line), then `sshd` with `sh -c
  'sshd -V 2>&1'`. `BASE_VERSION_LINES` (a constant in golden_seam.rs, checked against the TS) is one echo per check.
  `base_versions_cmd(path)` = `export PATH=<path>:$PATH`, newline, `BASE_VERSION_LINES`.
- `parse_versions(stdout)`: per `\n` line, JavaScript `trimEnd`, `^VERSION ([^:]+): (.*)$` (a name needs one char and
  stops at the first colon), then the first `\d+\.\d+(?:\.\d+)?` in the rest (ASCII digits): `{ name, version }`, in
  line order. No number, no row. A line with leading space is no row.
- `carried_by_image(versions)`: the read as a map (a later name wins). Each floor row in catalog order whose bin and
  every brought bin answered, and, where it pins a `major`, whose version's first `n` dot parts (n the major's part
  count) equal the major; then `ssh-server` when `sshd` answered.
- `versions_line(versions, results)`: each version `<name> <version>`, with ` (<fmt_bytes(bytes)>)` where the check has
  a floor id and that row's result has `bytes > 0`, joined `, `; then `already on the machine: <labels>` for the rows
  with that note; then `<label> <outcome> (<note>)` for every row not `base/apt-index` and not installed (a missing
  note prints `undefined`, as the TS does); the non-empty parts joined `; `.

## install_base (:165-192)

1. Exec `base_versions_cmd(path)`, timeout 20,000; its exit is not read. `carried = carried_by_image(parse(stdout))`.
2. `install_tools_at(base_installs(carried, path, prefix, left), stage, deploying-daemon, { keep_caches, path, prefix
   })`: no `present`, no `floor`, no `on_tool`.
3. The rows: the loop's steps that are not floor rows (login PATH, apt index, ssh server) in loop order, then each
   floor row in catalog order: `left` and not carried is no row; carried is `{ id, label: e.name, installed, note:
   already on the machine }` with no `ms` or `bytes`; else the loop's row where it has one.
4. Exec the version read again; `versions` its parse; `line = versions_line(versions, rows)`.
`install_base(machine, stage, path, prefix, left)` is this with `keep_caches: true`. A rejected call is the `Err`.

## Rules against tests

| Rule | Tests (`golden_base_hard.rs`, `golden_base.rs`) |
|---|---|
| the profile line, the version lines, the read | `the_version_lines_and_the_profile_line_are_the_ts_byte_for_byte`, `asks_each_floor_command_for_its_version_on_the_tools_path_unzip_with_zip_and_nothing_of_the_node`, `writes_the_login_shells_path_before_the_floor_on_every_golden` |
| every step byte for byte, five carried/left sets | `base_installs_is_every_floor_step_in_order_with_its_script_byte_for_byte`, `is_every_floor_entry_by_its_catalog_road_in_order_each_waiting_on_what_it_needs`, `every_step_runs_under_set_e_with_a_home_and_the_tools_path_and_each_road_is_the_catalogs_pinned_one` |
| a carried row and what waits on it; left and carried | `a_carried_row_leaves_the_plan_and_what_waited_on_it_waits_on_nothing`, `a_row_left_off_and_carried_is_off_the_plan_either_way`, `leaves_the_c_toolchain_off_the_floor_unless_a_picked_row_needs_it_and_keeps_every_other_floor_row` |
| the ssh server, with and without the index | `installs_an_ssh_server_on_every_golden_whatever_its_recipe_with_port_22_closed_while_it_installs` |
| one curl function, curl's own error | `every_base_step_that_downloads_runs_under_the_one_curl_function_and_types_no_flags_of_its_own`, `leaves_curls_own_error_as_the_steps_reason_the_uv_script_under_bash_with_a_curl_that_fails_like_a_404`, `every_step_the_apt_index_included_names_one_line_a_person_reads_for_what_it_runs` |
| parsing versions: empty, no number, colons, CR, ASCII digits | `parse_versions_keeps_the_number_alone_and_drops_a_line_with_none`, `keeps_the_version_number_out_of_each_tools_own_wording_and_leaves_out_a_command_that_printed_nothing` |
| carried: every bin, a pinned major, sshd | `carried_by_image_names_the_rows_the_image_satisfies_in_catalog_order_then_the_ssh_server`, `keeps_the_first_line_of_a_read_that_carries_a_version_so_zip_reads_carried_and_no_apt_index_runs`, `a_floor_row_that_pins_a_major_runs_when_the_image_carries_another_and_the_apt_index_is_left_out` |
| the line: sizes, already, missed, `undefined`, empty | `versions_line_names_each_version_with_its_cost_then_what_was_there_then_what_missed`, `the_stage_line_names_each_version_with_what_its_install_cost_and_a_floor_entry_that_did_not_land_by_its_reason` |
| install_base: calls, rows in order, stages, line; failure; carried; left; PATH check | `install_base_runs_the_read_the_loop_and_the_read_after_as_the_ts_did_in_every_scenario` (six scenarios), `install_base_as_provision_runs_it_keeps_the_caches`, `runs_the_floor_under_the_base_stage_reads_the_versions_once_after_and_its_line_carries_the_sizes_df_saw`, `a_step_that_fails_is_named_on_the_stage_and_in_the_line_and_what_waited_on_it_is_skipped_by_its_name`, `a_floor_row_the_providers_image_already_satisfies_reads_as_on_the_machine_and_installs_nothing`, `an_install_that_exits_0_without_its_command_on_path_is_a_failure_not_a_version` |
| the import result's fields | `a_saved_import_result_reads_its_fields_and_ignores_the_rest` (green: types only) |
