# slate_box
Source: packages/runtime/src/slate-box.ts (174 lines) and slate-mcp-on.ts (145 lines) at 65f6b89c7. TS tests: `test/slate-box.test.ts:90-317` (launcher, road, ledger; ported whole), `:635-657` (`pipeOn` refused; ported whole), `:319-657` and `test/slate-box-server.test.ts`, `slate-box-http.test.ts`, `slate-here-http.test.ts`, `slate-box-timer.test.ts` (through `createSlates` or the runtime: what they assert of these two files is ported against fakes, the rest belongs to the slates join and the runtime join). Rust: `src/slate_box.rs`, `src/slate_mcp_on.rs`; tests `tests/slate_box.rs`, `tests/slate_mcp_on.rs`.

## What it does
`slate_box`: a slate's command on the thread's own machine (a box, a cloud machine, or a folder on a computer the person joined), through the engine's `Machine`. Before each command the slate's files and the command's values go up; a fixed launcher decodes the values, deletes them, and runs the agent's text as `bash -c`'s one argument with the values as its environment, positional parameters and stdin, never in its text. A ledger keeps what this host still owes each box (values of runs in flight, removals never answered). `slate_mcp_on`: a slate's MCP server on that computer, started through its daemon's `stdio.open`, or an http server asked through its `http.post`, or from here with the sign-in the agent keeps here.

## Behaviour: slate_mcp_on
1. `BOX_SLATES = "/tmp/wsp-slates"`; `box_slate_dir(thread) = "/tmp/wsp-slates/<thread>"`.
2. `server_line(login, t, cwd)` (`:297-298`) = `as_login(login, ENV_THEN_SERVER + "cd -- " + q(t.cwd or cwd) + " 2>/dev/null; exec " + [command, ...args] each q'd joined by " ")`, `q` = `shell_quote` (always single quotes), ENV_THEN_SERVER = `while IFS= read -r -d '' k && [ -n "$k" ] && IFS= read -r -d '' v; do export "$k=$v"; done; `. Pinned: login `{linux, /root, root}`, `npx -y it's`, cwd `/root/my proj` gives `export HOME='/root'; cd "$HOME" 2>/dev/null; while IFS= read -r -d '' k && [ -n "$k" ] && IFS= read -r -d '' v; do export "$k=$v"; done; cd -- '/root/my proj' 2>/dev/null; exec 'npx' '-y' 'it'\''s'`. With `run_as` the whole line is wrapped by `as_login`'s `runuser` (pinned in the test).
3. `env_pairs(vars)` = each `k\0v\0` in order, then one more `\0` (the empty name that ends the list). An empty map gives `\0`.
4. `pipe_on(computer, login, road, mark = {})` (`:308-351`) answers an `McpPipeOpen`. Per open:
   - `stdio_id` = 8 random bytes as 16 hex (through `Ids`);
   - the first held write is `env_pairs(spec vars overlaid by the transport's vars overlaid by mark)`; writes before the open answers are held in order;
   - `road()` is dialled once. When it resolves after a kill, the road is closed and nothing is sent. Else it listens: events whose `stdioId` is ours: `stdio.out` (base64 data) to `out`, `stdio.err` (base64, utf8) to `err`, `stdio.exit` to `exit("the server exited with <code>")` when `code` is a number, else `exit("the server was stopped")`. When the road's `closed` settles: `exit("the link to <computer> closed")`.
   - sends `{op: "stdio.open", stdioId, cmd: server_line(login, t, spec.cwd)}` plus the key `env` holding a copy of the mark, only when the mark is not empty. Killed meanwhile: end (below). Else the road is open and the held writes go, in order, each `{op: "stdio.write", stdioId, data: base64(text)}`, failures ignored.
   - any failure of the dial or the open: `exit("it could not be started on <computer>: <message>")` first, then stop listening and close the dialled road. So the refusal is said before the close's own `link ... closed` (TS :635-657: first exit is the refusal, listeners 0, one close).
   - `write(text)`: held until open, else sent. `kill()` once: when open, end: stop listening, send `{op: "stdio.close", stdioId}` (failure ignored) and close the road after it settles. A kill before the open answers sets killed only; the open's answer then ends it.
   - The pipe has no `pid`.
5. `sign_in_gone_line(computer, line)` = `its address wants a sign-in that <computer> does not hold now; run <line> there, then start the run again`.
6. `renew_on_refusal(computer, sign_in, send)` (`:360-371`): sends once; with no sign-in, or a status other than 401/403, that answer stands. Else one renewal at a time per wrapped sender (every refused message in that window waits on the same renewal); false fails with `sign_in_gone_line(computer, sign_in.line)`; true sends again, and a second 401/403 fails the same way.
7. `post_on(computer, road, sign_in)` (`:374-394`) answers a maker of one `McpPost` per connection: the road is dialled at the first message and shared; a closed post fails `the server's connection is closed` (checked before and after the dial). Each message sends `{op: "http.post", url, headers, body, timeoutMs: min(ms, EXEC_TIMEOUT_MAX_MS = 600000)}` plus `bearer` when there is a sign-in, and parses the reply as `HttpPostReply`. Wrapped in rule 6. `close` marks closed and closes the road once it is dialled (a failed dial is ignored).
8. `post_here(computer, sign_in)` (`:398-406`) wraps this computer's own post: at each message `sign_in.read()` gives the token, and `Authorization: Bearer <token>` is added only when there is one with no `\r`, `\n` or `\0`; wrapped in rule 6; `close` closes the inner post.

## Behaviour: slate_box
9. `box_ledger(retry_ms = 60_000)` (`:30-73`): `live`, the set of payload paths of runs in flight; `owed`, per machine id, paths whose removal the box never confirmed.
   - `remove(machine, path)`: exec `rm -rf <q(path)>`; exit 0 is gone: paid, answers true. A non-zero exit or a failed exec: the path is owed; when no retry is armed and the ledger is open, one timer of `retry_ms` (through the clock) clears itself and removes each owed path again. Answers false.
   - `paid(machine, paths)`: removes them from what is owed; when none is left the timer is cancelled and the machine forgotten.
   - `owed(machine)`: the owed paths, oldest first. `close()`: no new timers, every armed one cancelled.
10. `box_launcher({payload, cwd, cmd})` (`:79-91`), these nine lines joined by `\n`:
    `set -u`; `p=<q(payload)>`; `base64 -d < "$p/env" > "$p/env0" && base64 -d < "$p/args" > "$p/args0" && base64 -d < "$p/in" > "$p/in0" || { rm -rf "$p"; echo "the run's values did not arrive" >&2; exit 1; }`; `while IFS= read -r -d '' k && IFS= read -r -d '' v; do export "$k=$v"; done < "$p/env0"`; `a=(); while IFS= read -r -d '' x; do a+=("$x"); done < "$p/args0"`; `exec 3< "$p/in0"`; `rm -rf "$p"`; `cd <q(cwd)> 2>/dev/null || { echo <q("the folder <cwd> does not exist")> >&2; exit 1; }`; `exec bash -c <q(cmd)> bash ${a[@]+"${a[@]}"} <&3`.
11. `nul_separated(items)` = base64 of each item followed by `\0`, UTF-8.
12. `box_road(machine, thread, files, ready, ledger, launch_on)` (`:98-174`). `slate_dir = box_slate_dir(thread)`. `launch_on` defaults to `machine.in_cgroup(thread_cgroup(thread))` for a `PlaceFolderMachine`, else the machine itself (the caller passes it in Rust, see Decisions). `start(o, on)`:
    - payload = `<dir>/.run-<12 hex from 6 random bytes>`, added to `ledger.live` at once;
    - the work: `await ready()`; `owed = ledger.owed(machine)`; `spared` = `! -path <q(p)>` for every live payload joined by spaces; `put_files(machine, files, {before})` where files are the slate's files at `<dir>/<name>` (in `files()` order), then `<payload>/env` = `nul_separated(the values' entries flattened k, v, ...)`, `<payload>/args` = `nul_separated(args)`, `<payload>/in` = base64 of `stdin` or empty; `before` = `umask 077`, `rm -rf <q(p)>` for each owed path, `mkdir -p <q(dir)>`, `find <q(dir)> -mindepth 1 -maxdepth 1 ! -name '.run-*' -exec rm -rf {} +`, `find '/tmp/wsp-slates' -mindepth 2 -maxdepth 2 -name '.run-*' <spared> -exec rm -rf {} +`, `mkdir -p <q(payload)> && chmod 700 <q(payload)>`;
    - a non-zero upload fails `the slate's files did not reach the thread's machine: <stderr trimmed, else "exit <code>">`; else `ledger.paid(machine, owed)`;
    - `machine.run(box_launcher({payload, cwd, cmd}), {deadline_ms: timeout_s * 1000, launch_on (only when it differs from the machine), on_line (only when stream; each line to on.line("out", line)), signal: the start's cancel, unlogged: true})`;
    - end = `{code: exit, out: stdout cut to EXEC_OUTPUT_MAX, err: stderr cut to max(0, EXEC_OUTPUT_MAX - stdout length), cut: stdout + stderr longer than EXEC_OUTPUT_MAX, timed_out: exit == DEADLINE_EXIT (124)}`; any failure: `{code: null, error, out: "", err: "", cut: false, timed_out: false}`;
    - always then `await ledger.remove(machine, payload)`, remove it from `live`, then `on.end(end)` once.
    - answers `{kill}` at once; kill cancels the run (the machine kills the group, exit 130).
13. `reshape(o)` (`:165-171`) starts `{cmd, args: [], cwd, values, stdin: input, timeout_s, stream: false}` on the same road and answers `reshape_answer(end, timeout_s, scrub)` (slates-a's); `kill` kills that start.

## Invariants (named tests)
- No value of a run reaches a command line or survives the launch on disk: `the_launcher_hands_values_whole_runs_none_and_leaves_none_on_disk`.
- The payload folder is 0700 before the values are decoded into it: `the_payload_folder_is_the_accounts_alone_before_decoding`.
- The launch alone goes through the cgroup machine; polls, kill and cleanup go through the machine: `hands_the_launch_alone_to_the_cgroup_machine`.
- A removal a box never confirmed is retried and paid at the next contact, and a run's values in flight are never swept: `values_a_box_left_go_once_it_answers_and_live_values_are_spared`.
- A secret written as a server's variable never rides `stdio.open`'s command: `pipe_on_sends_variables_on_stdin_never_in_the_command`.
- A bearer token never reaches a frame or an answer from this host on a box road; here a token with `\r`, `\n` or `\0` is never sent: `post_on_names_where_the_bearer_is_never_what`, `post_here_sends_no_bearer_that_would_break_the_request_line`.

## Edge cases
- A kill before the road resolves: the road is closed when it resolves, nothing sent.
- Two messages refused while one renewal runs: one `renew()` call.
- The thread's folder missing on the machine: stderr `the folder <cwd> does not exist\n`, exit 1.

## Decisions
1. `box_road` takes `launch_on: Arc<dyn Machine>` from its caller; `launch_on_for(machine, place, thread)` does the TS default, since Rust has no `instanceof` on a trait object. The area (`account/slates`) holds the concrete handle where it has one.
2. Random names go through `wsp_core::ids::Ids` (rule 3.16); the retry timer through `Clock` (rule 3.15).
3. `StdioRoad` is a trait: `send(frame) -> future of Value`, `listen(fn) -> unlisten`, `closed() -> shared future`, `close()`.

## After the port
- `box_road`'s sweep of other threads' `.run-*` folders runs at every contact with whatever `live` holds; a second host on the same box would sweep this host's in-flight values. Kept.
- `pipe_on` answers no `pid`, so a box server is never named the thread's by `own`; the daemon names it by the mark. Kept.

## Tests
| Rule | Test |
|---|---|
| 2, 3 | `server_line_quotes_as_the_ts` (both logins), `env_pairs_ends_with_an_empty_name` (`tests/slate_mcp_on.rs`) |
| 4 | `pipe_on_holds_writes_until_open_and_sends_variables_first`, `pipe_on_sends_variables_on_stdin_never_in_the_command`, `pipe_on_routes_only_its_own_stdio_events`, `pipe_on_closes_the_road_when_the_daemon_refuses_and_says_the_refusal_first` (TS :635-657), `pipe_on_a_kill_before_the_road_resolves_closes_it_unsent`, `pipe_on_kill_sends_stdio_close_once` |
| 5, 6 | `renew_on_refusal_renews_once_and_sends_again`, `renew_on_refusal_says_sign_in_gone_where_it_cannot_renew`, `renew_on_refusal_shares_one_renewal` |
| 7 | `post_on_names_where_the_bearer_is_never_what`, `post_on_caps_timeout_at_600000_and_dials_once`, `post_on_refuses_after_close` |
| 8 | `post_here_reads_the_token_at_each_message`, `post_here_sends_no_bearer_that_would_break_the_request_line` |
| 9 | `values_a_box_left_go_once_it_answers_and_live_values_are_spared` (TS :248-291), `ledger_close_cancels_retries` (`tests/slate_box.rs`) |
| 10, 11 | `the_launcher_hands_values_whole_runs_none_and_leaves_none_on_disk` (TS :91-104), `the_launcher_says_plainly_when_the_folder_is_missing` (TS :106-118), `the_launcher_text_is_the_ts_text` |
| 12 | `writes_the_slates_files_sweeps_and_runs_with_values_folder_and_lines` (TS :124-144), `the_payload_folder_is_the_accounts_alone_before_decoding` (TS :146-162), `hands_the_launch_alone_to_the_cgroup_machine` (TS :164-176), `a_failed_upload_ends_with_the_machines_words`, `caps_output_at_2_mb_and_marks_it_cut` |
| 12, 13 | `a_kill_cancels_the_command_and_a_then_reshapes_there` (TS :293-316) |

The joined-computer cases (TS :178-246) need a real `PlaceFolderMachine` over a fake daemon link; they are left to the slates join, which holds the area that builds `launch_on`, and listed in the report.
