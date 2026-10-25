# local_backend: this computer as the one local machine

Source: local-backend.ts at `a4033dc34`. Module `local_backend`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/local_backend.json`.

## Readings

- `LOCAL_MACHINE_ID = "local"`.
- `local_shape(cores, total_bytes)` (:31): `{ cpu: cores, memMb: round(total_bytes / 2^30) * 1024 }` (`Math.round`,
  half up). Defaults: logical CPUs and total memory of this computer. 1.5 GiB is 2048; 0.5 GiB is 1024; 0 is 0.
- `MemoryHere { free_mb, total_mb }`.
- `memory_of_meminfo(text)` (:43): `^{key}:\s*(\d+)\s*kB` with the multiline flag for MemAvailable and MemTotal; each
  `floor(kB / 1024)`; `None` where either is missing (a leading space before the key does not match; `MemTotal:2048kB`
  does).
- `memory_of_pressure(level, total_bytes)` (:54): `pct = Number(trim(level))`; `None` for an empty trim or a pct that
  is not an integer in 0..=100 (`x`, `101`, `-1`, `5.5`); `Number` accepts ` 7 `, `1e1` (10) and `0x10` (16).
  `total_mb = floor(total_bytes / 2^20)`, `free_mb = floor(total_mb * pct / 100)`.
- `memory_here()` (:63): Linux reads `/proc/meminfo` (unreadable is `""`, so `None`); macOS runs `sysctl -n
  kern.memorystatus_level` with a 5,000 ms bound (a failure is `""`) over the total memory; any other platform
  `None`. `memory_here_on(platform)` takes the platform word (`linux`, `darwin`, `win32`, ...) for a test. Never held.

## `LocalMachine` (:91)

- Id `local`, kind sandbox, `takes_stdin` true, no stream URL. Offers `describe`, `facts`, `put_bytes`.
- `exec(cmd, opts)`: `run_child("bash", ["-c", cmd], { cwd: root, env, timeout_ms, stdin })`; the key is ignored.
- `run(script, opts)`: the same with `timeout_ms = deadline_ms` and the line listener; past the deadline the group is
  killed and the answer is 124. Signal and poll are ignored.
- `snapshot`, `pause`, `resume` fail (`this computer cannot be snapshotted|paused|resumed`), as errors and never a
  panic; `download_url` / `upload_url` fail with `this computer serves no signed {download|upload} URL; its files are
  {read|written} on it directly`. `kill` answers Ok and stops nothing; `state` is running.
- `describe()`: `local_shape()` with no disk or creation time.
- `facts()` (:135): `os` read once per machine through `read_os_name` over this machine's exec (5,000 ms), falling
  back to `"{os type} {os release}"` (Node's `type()` and `release()`: `Linux 6.8.0-...`, `Darwin 24.6.0`); held in
  memory for the machine's life, never across a restart. `uptime_ms = round(system uptime seconds * 1000)`, read
  every call; `folder = root`.
- `put_bytes(path, bytes)` (:150): parents made, the file written with mode 0600 for a new file (an existing file keeps
  its mode, as Node's `writeFile` does); answers `None`.

## `LocalBackend` (:158)

- `new(root, env)`: env defaults to the process env. Nothing on disk is touched at construction.
- Capabilities, in this key order: liveCloneForks false, replacesMachine false, previewUrls false, signedUrls false,
  callbackRelay false, diskSnapshots false, images false, snapshotsAnyLife false, snapshotListing false, templates
  false, sizes [], kept true, copies true, ownNetwork false; no pauseMode. No lifecycle, no offers.
- Pricing: rate 0 for every size; default size `local_shape()`; storage `{ freeGb: 0, usdPerGbMonth: 0, billedFrom:
  "" }`.
- `folder` is the root. `work_folder()` makes it (with parents) and answers it.
- `create` fails with `this computer already exists; it is not created, it is the computer wsp already runs on`.
- `get(_)`: makes the work folder, answers the one machine (the same one every call).
- `list(_)`: `[{ id: "local", state: running, labels: {} }]`.
- `delete_snapshot(_)` fails with `this computer holds no snapshots`.

## Pins

| Rule | Tests |
|---|---|
| capabilities, rate 0 | `every_capability_a_provider_fork_has_and_a_computer_does_not_is_false` |
| shape in whole GB | `this_computers_size_is_its_cores_and_its_memory_in_whole_gb`, `shapes_match_the_ts` |
| get and list, running | `get_and_list_answer_with_the_one_machine` |
| facts, os held, uptime read again | `facts_say_what_this_computer_is` |
| exec in the folder, stdin, files | `exec_runs_a_shell_command_on_this_computer_under_the_workspace_folder`, `exec_hands_the_bytes_it_is_given_to_the_commands_stdin`, `files_read_and_write_under_the_workspace_folder` |
| run streams, deadline 124 | `run_streams_each_line`, `a_command_past_its_deadline_is_killed_and_answers_124` |
| refusals as errors, kill a no-op | `the_moves_only_a_provider_fork_takes_are_refused` |
| bytes 0600, parents made | `puts_bytes_on_this_computer_by_its_own_road` |
| construction touches nothing; get makes the folder | `building_the_backend_makes_no_folder_and_get_makes_it` |
| memory readings | `is_memavailable_over_memtotal_on_linux`, `meminfo_and_pressure_match_the_ts` |
