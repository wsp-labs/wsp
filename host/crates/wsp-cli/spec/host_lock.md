# host_lock

Source: `packages/host/src/host-lock.ts` (383 lines) at 65f6b89c7. TS tests: `test/host-lock.test.ts` (12, of which
6 drive a whole `serve()` and stay with the host's own port), `test/host-lock-take.test.ts` (11 with the `it.each`
rows). Rust tests: `tests/host_lock.rs` (32). Oracle: `tests/fixtures/hosts_oracle.json` (`paths`, `addressLines`,
`dial`, `startedBy`), printed by the TS.

## What it does

One state file, one host. `host.lock` beside the state names the pid serving it, the port it bound, the address,
the road that started it and the start time. Every reader finds the host through it; every start takes it so that
of starts at once exactly one wins. It also names every file a host keeps beside its state.

## Rules

1. Paths are `join(dirname(state), name)` as node joins strings: `state.json` gives `host.lock`, `/a/b/` gives
   `/a/host.lock` (host-lock.ts:132-186). Names: `host.lock`, `host-token`, `runs`, `roots`, `door-port`,
   `readings`, `inbox`, `host.log`.
2. `host_token_for` is the token file trimmed by JavaScript's `trim()`, `Some("")` for an empty file, `None` when
   unreadable (:143-149).
3. A lock reads (`lockOf`, :116-123) when the text parses as a JSON object whose `pid` and `port` are numbers and
   `startedAt` a string; anything else (bad JSON, `null`, an array, a string pid, a missing key) is no lock. Other
   keys are ignored; `startedBy` keeps whatever value it holds. Numbers are `JsNumber`: `4400.5` reads.
4. `signalled(pid)` is node's `process.kill(pid, 0)` (:64-71): a pid that is not an integer or lies outside
   `-2^31..2^31-1` is refused by node before any signal, which reads `Gone`; `EPERM` is `Another`; success `Own`;
   any other errno `Gone`. 0 and -1 signal a process group and read `Own`, so a lock naming pid 0 or -1 holds
   (after-port ticket 1). `pid_alive` is not `Gone`; `own_pid` is `Own`.
5. `program_gone(path)` strips one trailing ` (deleted)` and answers whether nothing stands there (:84).
   `runsVanished(pid)` reads `/proc/<pid>/exe`; an unreadable link (no /proc, another login's process) is never
   vanished (:89-97). A lock holds when its pid is alive and not vanished (:100). On Linux an upgrade in place
   (old file unlinked, new one at the same path) still holds (invariant 8 of `packages/host/AGENTS.md`).
6. `serving_host` answers the held lock; `vanished_host` the lock whose pid is alive and vanished (:225-235).
7. `held_or_started` answers the serving lock without calling the starter, else the starter's answer, else nothing
   (:240-242). Its errors pass through.
8. The refusal (`heldBy`, :125-130), no kind (exit class provider):
   `another wsp host (pid <pid>) is already serving <statePath> on port <port>. ` then `wsp down stops it, or point
   --state at a different file.` when the lock has any `startedBy` key, else `Stop it first, or point --state at a
   different file.` Numbers print as JavaScript prints them.
9. `take_lock` (:259-280), as pid P through `LockFs`:
   1. `refuse_if_served`: a held lock refuses (rule 8).
   2. lock = `{pid: P, ...ports, startedAt: now ISO}`; the JSON text is `pid`, `port`, `address`?, `startedBy`?,
      `startedAt` with absent options left out, no spaces. `startedAt` is `YYYY-MM-DDTHH:MM:SS.mmmZ`.
   3. `own_folder(dirname(state))` then `own_folder(dirname(lock))` (wsp_store::own_file): made 0700, repaired to
      0700.
   4. write the text to `<lock>.<P>`; `linkInto(mine, lock)`; on false, `takeOverStale`; `<lock>.<P>` is removed in
      every case (finally).
   5. sweep the lock's folder: remove every `<base>.taking.*`, and every `<base>.<N>.taking` whose N is not alive.
   6. answer the lock.
10. `linkInto(from, to)` (:368-383): `link`; `EEXIST` answers false; `EPERM`, `ENOTSUP`/`EOPNOTSUPP`, `EXDEV` fall
    to an exclusive create of `to` with `from`'s bytes (`EEXIST` false); any other error escapes.
11. `takeOverStale` (:291-306): read the lock text. Missing: link again or `tookFirst`. Present: a held lock refuses
    (rule 8); else hold a marker; read again, and text that differs from the first read is `tookFirst`; remove the
    lock; link, or `tookFirst`; remove the marker in every case.
12. `holdMarker` (:311-324): write `{"pid":P}` to `<lock>.<P>.taking`; for n = 0, 1, ...: link it to
    `<lock>.taking.<n>` and answer that on success; else read the holder of that marker and refuse when one lives:
    `another wsp host (pid <holder>) is starting on <statePath> right now; it serves in a moment.`, or without
    ` (pid ...)` when the holder is unknown. `<lock>.<P>.taking` is removed in every case. An abandoned marker is
    skipped, never removed here.
13. `markerHolder` (:328-341): the marker's JSON `pid` when it is a number: that pid if alive, else none. No pid
    (empty file, bad JSON, no key): unknown while `now - mtime <= MARKER_MS` (10 000 ms), none after. A marker that
    vanished (`ENOENT` on read) is none. A live pid holds however old the marker is.
14. `tookFirst` (:343-346): the winner's refusal (rule 8) when a lock reads, else `another wsp host just took
    <lockPath>`.
15. An error from a file call that rules 10-13 do not catch escapes as a `HostError` with no kind whose message
    opens with node's code name (`EACCES: ...`). Nothing is left at the lock path.
16. `release_lock` removes the lock only when it names this process (:350-352). `rewrite_lock` writes
    `<lock>.<pid>` and renames it over the lock (:356-360).
17. `address_lines` (:194-202): `app         http://<authority>`, `runtime ws  ws://<authority>/ws (token:
    <token path>)`, `state       <state>`, and `public      https://<hostname>` when a hostname is given. The bound
    address is `address` or `127.0.0.1`, a wildcard printed as it is (`http://0.0.0.0:4400`, `http://[::]:4400`).
18. `dial_address` (:219-222): the address, `127.0.0.1` when absent or a wildcard by `wsp_words::app_ports::
    is_wildcard` (`0.0.0.0`, `::`); `[::]`, `0:0:0:0:0:0:0:0`, `localhost` and `""` pass through unchanged.
19. `started_by_env`: `WSP_STARTED_BY` exactly `verb` or `service`; `up`, `Verb`, `""` and unset are none (:35).

## What survives a restart

The lock file alone. A host that died leaves a lock that rule 9 takes over; a crash mid take-over leaves markers
and `<lock>.<pid>.taking` files that the next winner's sweep removes once their pid is gone.

## Decisions

1. The TS test swaps `process.pid` and mocks `node:fs`; the port takes the pid and a `LockFs` seam
   (`take_lock_with`). `RealFs` is the only production `LockFs`; `own_folder` is not behind it.
2. The six-process race (host-lock.test.ts:271) is cut to six threads, each its own live pid, through `RealFs`.
3. The six `serve()` cases of host-lock.test.ts (token before sweep, close removes lock, fails to start, no write
   under `~/.wsp`) belong to the host process's port, not this crate.

## After the port

1. A lock naming pid 0 or -1 reads held forever (node signals a group); found while probing node 22.
2. `markerHolder` reads a marker's pid with `pidAlive`, so a reused pid of another login holds a marker forever.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | every_path_beside_the_state_is_joined_as_node_joins_it |
| 2 | the_token_beside_the_state_is_read_trimmed_and_nothing_where_none_was_written |
| 3 | a_lock_serves_only_while_it_parses_and_its_pid_lives, a_lock_that_does_not_parse_gives_way |
| 4 | a_pid_of_this_login_is_its_own_and_a_pid_that_is_gone_is_neither_own_nor_alive, process_one_is_another_logins_unless_this_login_is_root_and_held_either_way, a_pid_node_refuses_reads_as_gone_and_a_group_pid_reads_as_own |
| 5, 6 | a_program_is_gone_only_where_nothing_stands_at_its_path, a_lock_whose_program_file_is_gone_holds_nothing_and_the_next_host_takes_it, a_lock_whose_program_was_upgraded_in_place_still_holds_and_refuses_a_second_host |
| 7 | held_or_started_takes_the_serving_host_and_starts_one_only_where_none_serves |
| 8 | refuse_if_served_refuses_a_held_lock_and_passes_a_stale_or_missing_one, a_second_host_is_refused_naming_the_pid_and_port_and_the_lock_stays_the_first_ones, a_lock_with_any_started_by_names_wsp_down |
| 9 | a_taken_lock_holds_pid_the_ports_in_order_and_the_start_time, the_state_folder_is_the_owners_once_the_lock_is_taken, a_lock_whose_pid_is_gone_gives_way_and_the_new_lock_names_this_process, exactly_one_of_six_takers_at_once_wins_every_round_and_the_lock_names_it |
| 10 | the_lock_is_taken_by_exclusive_create_on_a_file_system_with_no_hard_links |
| 11 | a_start_that_takes_over_after_any_read_of_the_stale_lock_keeps_it_and_no_third_start_slips_in |
| 12, 13 | a_start_stalled_past_marker_ms_while_it_holds_the_marker_keeps_it, a_marker_whose_pid_is_gone_is_a_crashs_and_the_next_start_takes_over_and_clears_every_marker, a_marker_held_by_a_live_pid_refuses_this_start_however_old_it_is, a_marker_that_names_no_pid_holds_until_it_is_older_than_marker_ms, a_marker_held_by_a_live_start_a_moment_old_refuses_and_leaves_the_stale_lock_to_it |
| 9.5 | the_words_a_crash_left_beside_a_marker_go_and_a_live_starts_stay |
| 15 | any_other_link_error_escapes_with_its_code_and_leaves_no_lock |
| 16 | release_takes_away_only_this_processes_lock, rewrite_swaps_the_whole_lock_in_and_leaves_nothing_beside_it |
| 17 | the_address_lines_name_the_bound_address_the_token_file_the_state_and_the_public_name |
| 18 | a_tool_dials_the_bound_address_and_loopback_only_for_the_wildcard |
| 19 | the_environment_marks_a_host_started_by_a_verb_or_the_service_and_nothing_else |
