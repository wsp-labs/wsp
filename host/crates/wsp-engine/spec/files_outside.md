# files_outside (provision-files.ts:649-796)

What a setup on a computer somebody owns writes outside the agents' homes (`/usr/local`, `/opt`), recorded around each
install step and taken off by a leave. The record survives a host restart and is the only state: `/opt/wsp/landed`
(`$root$TOOL_PREFIX/landed`), lines `<state>\t<absolute path>`, appended only, each in one write, so two steps closing
at once lose none; every reader takes the last line a path has. `<state>` is a file's sha256, `link:<sha256 of the
link's target text and a newline>` (`readlink | sha256sum`: a link to `abc` is the digest of `abc\n`), or `dir`.

All four scripts are pure text functions, byte for byte the TS's (oracle `outside`), run by `provision_step` (before
and after) and by the leave on both roads (the daemon runs `outside_sweep_script` through sh; with `{root}` plus a
newline it is `daemon/fixtures/contract/landed-outside.sh`). `root` is empty on a real computer and a scratch folder
in tests.

## Pieces (:660-711)

- `outside_head(root)`: `set -u`, then `root=<q root>; prefix="$root/opt/wsp"; ledger="$prefix/landed"`, a literal
  newline in `nl`, a tab in `tab`.
- `outside_states()`: `states <scratch>`: reads paths on stdin; a link prints `link:<digest>\t<path>`, a folder
  `dir\t<path>`, a file is queued; then one `xargs -0 sha256sum -z --` over the queued files prints `<digest>\t<path>`.
  Anything else prints nothing.
- `outside_still()`: `still <scratch>`: the folders under `$root/usr/local` and `$root/opt` (never into the prefix,
  never a path with a newline; find never follows a link); the list's last line per path, kept only for a path under
  `/usr/local/` or `/opt/`, not the prefix or under it, without a `.` or `..` part or `//`, whose parent folder is in
  that walk; then those paths' states now, printing the lines whose state still matches.
- `outside_listing(walked, out, tests)`: a find over `"$root"<q dir>` per walked folder (prefix and newline paths
  pruned, `tests` before `-print`), then, only when find exited below 128, `LC_ALL=C sort`, the `wsp-listed` end line,
  and a move into `out`; otherwise remove every `$at.*` scratch file and exit 0.

## The three scripts

- `outside_before_script(step, walked, root)` (:718): `exit 0` for nothing walked. Else head, `at="$prefix/.landing-<step>"`,
  exit 0 unless the prefix is not a link and can be made, remove old `$at.*`, the two functions, touch `$at.mark`
  (exit 0 when it cannot), list into `$at.before`, and, where the list exists and is not a link, write `$at.ours`: the
  paths that still stand as wsp left them, sorted unique.
- `outside_after_script(step, walked, root)` (:742): `exit 0` for nothing walked. Else head, the same `at`, then
  nothing at all (remove `$at.*`, exit 0) unless `$at.mark` exists, the list is not a link and `$at.before` ends in
  `wsp-listed`. List into `$at.after`, list what changed since the mark (`-cnewer "$at.mark"`) into `$at.changed`, read
  every `$root/var/lib/dpkg/info/*.list` path as `$at.dpkg`; record `(after minus before) plus (changed and ours)`,
  sorted unique, minus dpkg's, through `states`, each line appended to the list; remove `$at.*`.
- `outside_sweep_script(root)` (:767): exit 0 unless the prefix is a folder, not a link, and the list a file, not a link.
  `still` sorted by path descending (children before their folder) into `$at.take`; files and links removed in one
  `xargs -0 rm -f --`, then folders in one `xargs -0 rmdir --` (a non-empty one stays); print `wsp-outside\t<path>` for
  each taken path that is gone; remove the list and `$at.*` last, so a leave cut short keeps the list for one that
  finishes.
- `outside_marks(stdout)` (:790): lines with the mark first, path fields 1.. joined by tab, kept only when it starts
  with `/`.

## Rules against tests

| Rule | Tests (files_outside.rs unless named) |
|---|---|
| records every path a step made, nothing the computer had | `writes_down_every_path_an_install_step_made_under_usr_local_and_opt_and_nothing_the_computer_had_before` |
| a dpkg path is never recorded | `leaves_off_a_path_a_package_put_there_which_is_dpkgs_to_take` |
| the leave takes exactly what still stands as wsp left it, folders once empty | `takes_off_exactly_what_the_list_names_as_wsp_left_it_and_every_folder_it_made_once_nothing_is_left_in_it` |
| restart: a later run's bytes are recorded again, the last line wins | `writes_a_file_down_again_at_the_bytes_a_later_run_left_in_it_and_leaves_the_old_lines_bytes_behind` |
| never outside the tops, under the prefix, or past a linked folder | `takes_nothing_a_line_names_outside_usr_local_and_opt_under_wsps_own_folder_or_past_a_folder_that_is_a_link`, `reads_nothing_off_a_list_that_sits_in_a_prefix_that_is_a_link` |
| a listing cut short records nothing | `records_nothing_when_the_listing_before_the_step_was_cut_short_so_the_boxs_own_files_are_never_claimed` |
| 4,000 files inside 60 s | `records_and_takes_four_thousand_files_well_inside_the_leaves_sixty_seconds` |
| a leave cut short keeps the list; the next finishes | `keeps_the_list_when_the_leave_is_cut_short_and_a_second_leave_finishes_it` |
| only the walked folders; the person's bytes never recorded | `walks_only_the_folders_the_steps_roads_write_so_a_file_somebody_else_puts_elsewhere_meanwhile_stays_theirs`, `never_writes_down_the_persons_bytes_when_a_later_step_only_touches_a_file_of_wsps_they_wrote_over` |
| marks; exact texts; nothing walked is `exit 0` | `answers_only_the_marks_own_lines_each_an_absolute_path`, `landed_outside_sh_equals_its_regeneration_so_the_twin_the_daemon_carries_is_this_script`; files_pins.rs: `the_record_and_the_leave_outside_the_home_are_the_ts_text_byte_for_byte`, `a_step_that_walks_nothing_records_nothing`, `the_outside_marks_are_absolute_paths_and_keep_a_tab_in_one` |
| the bracket around a step (provision_step) | `lists_the_folders_before_the_floor_and_writes_down_what_it_made_after_under_wsps_prefix_only`, `walks_the_folders_each_road_of_the_step_writes_under_usr_local_and_opt_wsps_own_folder_left_out`, `leaves_a_step_that_installs_nothing_outside_the_home_alone` |
