# threads-b

Source: `packages/host/src/verbs/threads.ts` lines 449-723 (275 lines) at 65f6b89c7: send, restart, stop, ssh, exec, folders, setup, terminal config, export. Tests: `verbs-exec-host.test.ts` (14), `send-waits.test.ts` (6), `host-release.test.ts` (restart past the release, 2), `verbs-folders-settings.test.ts` (14), against a running TS host; the tool server's record replays the tool calls. Rust: `src/verbs/threads.rs`; tests `tests/threads_answers.rs`, `tests/threads_answers_cloud.rs` (export), `tests/threads_lines.rs`, `tests/fixtures/run_verb.json`.

## What it does
send follows or detaches a message's turn; restart brings the host back on its road and lists what runs; stop interrupts a turn or one subagent; ssh pipes one ssh connection (command line only, cloud); exec runs a command where a thread works and streams it; folders lists a computer's folders; setup reads the host's setup; terminal config reads this computer's Ghostty config; export brings a folder home (cloud).

## Behaviour
1. Arity refusals before any dial: `wsp send takes a thread and one message.`, `wsp restart takes no arguments.`, `wsp stop takes one thread.`, `wsp ssh takes one workspace.`, `wsp exec takes a thread, then -- and the command.` (no thread or no words after it), `wsp folders takes one folder on this computer at most.`, `wsp setup takes no positional arguments.`, `wsp terminal config takes no positional arguments.`, `wsp export takes a workspace and a folder on this computer.`; `--scheme` refused by `scheme_flag` before any read.
2. send (456-469): `beforeSending` over thread, `checked_start` (on the thread's harness and workspace) and `awake(.., "send", say)`; the start is `message_to(thread, message, picks, files, elsewhere)`; detach prints the id, else follows and emits `noticed(turn_view, said)`.
3. restart (495-499, 506-508): the entry has `any_release`, so its dial is not refused by the release check; answers `{running}` and `host_restarted_line(running)`.
4. stop (518-523): `stop(client, thread, --subagent)`; the value is the stopped answer, line `stop_line`.
5. ssh (544-554): cli only, `starts_no_host`; refused `SSH_PIPES_HERE_LINE` with `Run it from an ssh client on that computer, as wsp's ssh config does.` when the terminal has no byte streams or the line came from elsewhere; then `ssh.port {workspaceId}` and `pipe_bytes`.
6. exec (562-576): `absolute_folder(--cwd)` (also refused at the parse), dial, `thread_at(ref, "wsp exec")`, `awake(.., "exec", say)`, folder = the given cwd or `thread_cwd(thread, workspace)`; every `exec.output` frame is emitted as the frame (`--json`: the frame as pushed, one line each) with its text as the line; an exit with `error` fails with that error (exit 1, after the lines already printed); then emits `{exitCode, cwd?}` (nothing printed without --json); a non-zero, non-null code prints `exec_folder_line(ran_in)` on stderr; the line exits with the code, or 1 when it is null.
7. exec's tool (583-595): the same, the output collected; the text is the lines joined by newlines; `{exitCode, output, cwd?}`; `stream: ["output"]`.
8. folders (604-611): one positional at most; `host_folders` with the line's folder through node `resolve` (the tool's through `absolute_path("folder is a path on this computer", _)`); line `folder_lines(listing)` joined.
9. setup (634-646): `{setup}` and `init_setup_lines(setup)`; the tool answers `as_json`.
10. terminal config (656-668): `reads_here`; reads `read_ghostty_config(node_host(), scheme)` on this computer, never the host; the tool is not served to a guest (the record's guest line is the SDK's `Tool terminal_config not found`).
11. export (680-711): cloud; dest is the folder resolved here; `agents_flag(--agents)`; `workspace_of`; stages other than done and failed stream to stderr as `<message>\n`; the done stage's message is the line; a failure goes through `with_replace_hint`.

## Edge cases
- exec with no `--cwd` asks with the thread's folder; with `--cwd /tmp/x` asks with that (`threads_lines::exec::exec_runs_in_the_cwd_given...`).
- exec's frames carry C1 characters, escaped on stdout (`\u0085`).

## Behaviour nobody wrote down
- Rule 10.7: `wsp exec` alone exits with the command's own code, or 1 when it has none.
- Rule 10.5: exec's --json prints frames first and the result last.

## Decisions
None beyond threads-a's.

## After the port
- exec emits `{exitCode: null}` and exits 1 with nothing on stderr when the machine gave no code and no error, so the person reads no reason.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `verbs_oracle::run_verb_answers_the_exit_code_and_the_lines_before_it_dials` |
| 2 | `threads_answers::tools::send` |
| 3 | `threads_answers::tools::restart`, `run_verb::a_line_that_starts_no_host_dials_without_the_starter_and_any_release...`, `verbs_oracle::every_entry_carries_its_marks_and_reasons` |
| 4 | `threads_answers::tools::stop` |
| 5 | `verbs_oracle::every_entry_carries_its_marks_and_reasons` (the marks); the byte pipe is the reviewer's to read (no host here) |
| 6 | `threads_lines::exec::*` (5) |
| 7 | `threads_answers::tools::exec` |
| 8 | `threads_answers::tools::folders`, `io_roads::folders::*` |
| 9 | `threads_answers::tools::setup` |
| 10 | `threads_answers::tools::terminal_config` |
| 11 | `threads_answers_cloud::tools::export` |
