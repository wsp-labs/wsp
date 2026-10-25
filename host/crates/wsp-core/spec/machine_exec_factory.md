# machine_exec_factory
Source: packages/runtime/src/machine-exec.ts:42-93, 300-345, 738-861 (about 230 lines) at 65f6b89c7. Rust:
`src/machine_exec/factory.rs`. Tests: the launch, attach and sweep cases of packages/runtime/test/machine-exec.test.ts
(37 cases, listed below). Rust: `tests/machine_exec_factory.rs`.

## What it does
`MachineExecFactory::new(machine, opts, waiting)` is the `ExecStreamFactory` of a turn on a machine reached by exec
(a cloud machine, a box, a joined computer). `launch` puts a run's script up and starts it detached under `setsid`,
`attach` re-opens a run an earlier host launched, by its handle, and `sweep` ends every run the machine holds that the
connecting host did not name. Every guest step is shell text; the shell builders here are the stream unit's too.

## Behaviour
1. Defaults (machine-exec.ts:301-308): `poll_ms` 1500, `idle_ms` `TURN_IDLE_MS`, `deadline_ms` `TURN_WALL_MS`,
   `exec_timeout_ms` `INLINE_EXEC_MS` (20 s, every exec this file sends), `run_dir` `RUN_DIR` (`/tmp/wsp-run`), `now`
   the system clock, `sleep` a real timer. `q` is `shell_quote`; `claim(b)` is `b + ".d"`.
2. `minted(run)`: `run` starts with `run_dir + "/"` and the rest matches `^[0-9a-f]{12}$` (machine-exec.ts:340).
3. Shell builders, byte for byte (machine-exec.ts:319-338): `release_held(b)`, `release_late(b)` and
   `reap_script(b, read, signal)`. `reap_script` with `signal` is `P=$(cat 'b'.pid 2>/dev/null); if [ -n "$P" ]; then
   kill -TERM -- -$P 2>/dev/null; for i in 1 2 3 4 5 6 7 8 9 10; do kill -0 -- -$P 2>/dev/null || break; sleep 0.2;
   done; kill -KILL -- -$P 2>/dev/null; fi; ` then `read`, then `rm -rf 'b'.*` (`GRACE_CHECKS` is
   `RUN_STOP_MS / 200` counted from 1, joined by spaces).
4. `launch` (machine-exec.ts:738-810), in order: `base = run_dir/<12 lowercase hex from 6 random bytes>`;
   `holds_seed = input.is_some() && input_after.is_some()`; seed path `base.held` when it holds, else `base.in`; a
   `secret` on a machine whose `takes_stdin()` is false refuses with `<id> takes no input on its launch, so text holding
   a value would ride its command; nothing was started` and no exec goes out; `held` is each `secret.files` entry `i`
   in order as `{WSP_LAND_<i>, base.f<i>, text, variable}`, then, when `secret.input` and an input exist,
   `{WSP_LAND_IN, seed path, seed, no variable}`; `named` is the env less keys not matching `^[A-Za-z_][A-Za-z0-9_]*$`.
5. The script `base.sh` is `work_score_line(None) + "\n" + run`, where `run` is `<command>\necho $? > 'base'.exit\n`
   with no input, and with one `( tail -n +1 -f 'base'.in > 'base'.fifo & echo $! > 'base'.tail )\n{ <command>\n} <
   'base'.fifo\necho $? > 'base'.exit\nkill $(cat 'base'.tail) 2>/dev/null\n`. The seed file (each input line plus
   `\n`) is uploaded as a file only when an input exists and no held entry lacks a variable.
6. The upload is `put_files(launch_on or machine, files, {before, after, timeout, stdin})` with `before = ["umask
   077", <claim mkdir>]`, the claim mkdir being `mkdir 'claim' 2>/dev/null || { [ -d 'claim' ] && { echo
   WSP_LAUNCHED; exit 0; }; echo 'no run folder on this machine: claim' >&2; exit 1; }`; `after` = `export
   WSP_AGENT_PID_FILE='base.agent'`, `: > 'base'.in` when it holds the seed, `mkfifo 'base'.fifo` with an input, the
   env lines (`ENV_FROM_INPUT` alone on a stdin machine, else `export K='v'` per named entry in order), per held entry
   `printf %s "$NAME" > 'path' || exit 1`, `unset NAME`, `export VAR='path'` when it has a variable, then `setsid bash
   'base'.sh > 'base'.log 2>&1 & echo $! > 'base'.pid; echo WSP_LAUNCHED`; `stdin` only on a stdin machine:
   `env_input(named then each held name to its text)`.
7. The upload goes at once, at `launch`, whether or not anything reads the stream, under `until_reached` on the
   factory's `now` and `sleep` (network failures retried at the link backoff inside `LINK_RETRY_WINDOW_MS`). Each try
   counts a post; a try that fails neither `never_sent` nor `far_end_answered` marks that the run may have started.
8. The launch's outcome (machine-exec.ts:799-808): an answer with a non-zero exit or no `WSP_LAUNCHED` fails with
   `remote launch failed on <id>: nothing came back saying WSP_LAUNCHED, the word the guest prints once the run is up;
   <machine_answer(res)>`; an `Unreached`, a `MachineUnreachable` (an exec-failed one too) or a `PlaceAbsent` failure
   keeps its own sentence and its kind; any other failure is `remote launch failed on <id>: <its message>`, kind
   `Other`. The stream unit reads this outcome.
9. `attach(run, {input, started_at, from})` (machine-exec.ts:812-830): a handle `minted` refuses with `<run> is not a
   run this host could have launched` before any exec. One exec under `until_reached`: `if [ -d 'run.d' ]; then echo
   WSP_RUN;` + (with input) ` <release_late(run)>;` + (with input) the channel read ` n=$({ wc -c < 'run'.in; }
   2>/dev/null || echo 0); if [ "$n" -gt 262144 ]; then tail -c 262145 'run'.in | sed 1d; else cat 'run'.in
   2>/dev/null; fi;` + ` else echo WSP_GONE; fi`. The first line holding `WSP_RUN` opens a stream (already launched,
   no posting) whose `taken` is every non-empty line after it (with input) or absent; else `WSP_GONE` anywhere answers
   `Gone`; else it fails with `the machine did not answer whether it still holds <run>; <machine_answer(res)>`. A
   failure of the exec itself is the attach's failure as `until_reached` answers it: nothing is killed or removed.
10. `sweep(keep)` (machine-exec.ts:832-858): one exec `for d in 'run_dir'/*.d; do [ -d "$d" ] && printf '%s\n' "$d";
   done`; the stale runs are its lines trimmed, ending `.d`, less the suffix, `minted`, and not in `keep`, in listed
   order. None stale: answer `[]` with no second exec. Else pages of `{ <reap_script(b)>; } &` joined by a space, then
   ` wait; true`, packed in order while `exec_fits(page)` holds (a page always takes its first run), each page one exec,
   in order. Answers the stale runs. An exec failure fails the sweep.

## Edge cases
- A claim folder the machine refuses to make (another login's) fails the launch with the stderr in the sentence:
  `...; it exited 1 and said: no run folder on this machine: /tmp/wsp-run/<id>.d`.
- A launch posted twice under one base starts the script once (the second mkdir finds the claim and prints the word).
- An attach answer with neither word, or an exit with garbage, is the not-answered failure, never `Gone`.
- 200 stale runs reap in several pages, every exec body under `EXEC_BODY_MAX` as the Solari body measures it, each run
  in exactly one page.

## Behaviour nobody wrote down
- Only the machine's own `WSP_GONE` ends a run on attach; silence, a refusal or garbage never does (rule 6.20).
- The handle is checked by shape before it reaches shell text, on attach and on sweep (a guest-written name in the
  run folder is never trusted).

## Decisions
1. The TS factory throws synchronously on a refused secret; the Rust trait's `launch` cannot fail. `try_launch`
   answers `Err(ExecFailure)` with the sentence and sends nothing; `launch` calls it and on `Err` answers a stream
   whose `lines` yields that one `Err`, whose `exited` is `None`, with no exec.
2. A run's name, the sentinels, the reap mark and a late line's key are drawn through the `Ids` seam (rule 3.16,
   `ids`, `OsIds` by default): `hex(ids.bytes(6))`.
3. `now` and `sleep` mirror the TS options one for one (`Arc<dyn Fn() -> i64>`, `Arc<dyn Fn(u64) -> BoxFuture>`);
   the factory builds the engine's `RetryClock` from them for `until_reached`, with the real random draw.

## After the port
- A launch whose `secret.input` is set but which has no `input` lands nothing for the seed and still runs.

## Rules against tests
Ported from machine-exec.test.ts (lines in brackets): 1-3,8 `launches_detached_streams_log_lines_across_polls_and_resolves_the_exit_code` [31],
`a_run_folder_the_machine_will_not_make_fails_the_launch_rather_than_reading_as` [51],
`a_launch_the_machine_answered_nothing_to_names_the_handshake_that_never_came` [71],
`a_launch_the_machine_refused_carries_what_the_machine_said_beside_the_code` [89]; 5,6
`a_command_that_fits_one_exec_body_launches_in_one_exec_under_the_cap` [135],
`a_40_kb_command_goes_up_in_pieces_every_exec_body_under_the_cap_and_the_script` [147],
`makes_every_file_of_a_run_under_the_login_s_own_mask_from_the_claim_folder_dow` [174],
`the_run_script_puts_the_turn_s_processes_at_the_work_score_before_anything_els` [187],
`without_an_input_channel_the_command_runs_as_before_no_input_file_no_tail_and` [268],
`with_an_input_channel_the_launch_seeds_the_file_and_the_script_feeds_it_to_the` [279],
`a_seeded_prompt_over_the_cap_goes_up_in_pieces_ahead_of_the_launch_every_exec` [296]; 7,8 the eleven launch-retry
cases [821-1082]; 9 the twelve attach cases [1146-1306] and [1328]; 10 the three sweep cases [1530-1590]; 4 the two
secret cases [1968, 1986] and the two key cases [1356 x2, 1398]; 7 `a_launch_posted_twice_under_one_base_as_a_retried_exec_does_starts_the_script` [1310].
Pinned beyond the TS (tests/machine_exec_shell.rs): `the_shell_text_is_the_ts_text_byte_for_byte` and `the_shell_builders_are_the_ts_text` (rules 3, 5, 6, 9, 10 against the text node printed, tests/fixtures/machine_exec_shell.json), `a_handle_is_minted_only_in_the_run_folder_with_12_lowercase_hex` (rule 2).
