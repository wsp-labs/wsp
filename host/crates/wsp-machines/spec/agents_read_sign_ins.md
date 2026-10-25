# agents_read_sign_ins: one watched sign-in per agent or server on a target

TS: `agents-read.ts:312-350, 426-439, 486-605`. Rust: `src/agents_read/sign_ins.rs`, methods of `AgentsReads`. TS
tests: agents-read.test.ts's sign-ins group (14 cases), places-setup's gh sign-in cases. Tests:
`tests/agents_read_sign_ins.rs`.

## Rules

N1. The join key (agents-read.ts:339) is the JSON of `[target, agent, server or null, terminal == true, login ==
true]`. A start while one with that key is starting or running joins it: it waits for that start and follows it (N3).
A start that failed is removed from the key so the next start runs fresh.
N2. A start, in order (agents-read.ts:495-569): the acts port (A2), the target (`napping_sign_in_refusal` kind `usage`
for a napping workspace), the acts' plan. A refused plan logs `sign-in refused: <what>: <message>` and fails; no
channel opens and nothing is listed. Then the id `si_<12 hex>` (6 random bytes from `ctx.ids()`), the log `sign-in
<id> started: <what>`, then the channel (`o.channel(target, <fan out to the link's readers>, origin)`). `what` is
`<agent>[, server <server>], on workspace <id>` or `... on computer <placeId>`, agent and server with control
characters taken out.
N3. `follow(id, run, emit)` (agents-read.ts:340-350): an emit not following yet is added and, where the run has a last
step, handed it at once; an emit already following is handed nothing. `leave` removes that emit and nothing else; the
run goes on.
N4. The run is handed (agents-read.ts:521-540): a link whose `op(op, extra)` sends `{...extra, op}` (op last) on the
channel and answers its reply, whose `on_event` adds a reader and answers its removal, whose `closed` is the
channel's, `bare` `pty_bare_on(target)`; `emit`, which makes `{type: "agents.signIn", signInId, ...step}`, keeps it
as the last and hands it to every follower; `typing`, which sets or clears the code writer; `stop`; and `forward`, for
a server's sign-in only, the first registered relay whose `open(target)` answers one.
N5. gh first (agents-read.ts:429-439): only for a computer target, no server, not `toolThere`, agent `GITHUB_CLI`. The
door's `gh_there(placeId, on_step)`; the first step with `at` and no `row` steps `{state: running, said:
installing_first_line(<gh's catalog name>)}`, once. A row whose outcome is `failed` ends the sign-in with `{state:
failed, said: last_line(note) or "<label> did not install"}` and the plan is never run.
N6. A run that throws steps `{state: failed, said: <message>}` (agents-read.ts:543).
N7. The end (agents-read.ts:544-566), in order: the forward closed; the run unlisted; ended runs older than
`SIGN_IN_ENDED_KEPT_MS` pruned (strictly older); this run kept as ended `{...listed, ended: true}` at `now()`; its key
removed where it is still this start's; the channel closed; the log `sign-in <id> ended: <what>: <stopped | last state
| failed>`. Then, for a computer target with no server that was not stopped and whose last state is `signed-in`: on
this computer an agent whose catalog sign-in mints a token lands as `key_landed(agent)`, any other as
`login_landed(placeId, agent)`; a failure there logs `sign-in <id>: the landed login was not noted: <message>`. Then
the reader forgets the target's JSON key, then the target changed (A1).
N8. `stop` (agents-read.ts:514-518) removes the key where it is this start's, marks the run stopped and settles
`stop` for the plan; the run ends as the plan returns. The next start with that key runs fresh.
N9. `sign_ins()` (agents-read.ts:578-581): prunes, then every running sign-in in start order `{signInId, target,
agent, server?, scope?, project?, last?}`, then every ended one kept.
N10. By id (agents-read.ts:582-596): `sign_in_follow`, `sign_in_code` (only while the run holds a code writer) and
`sign_in_stop` refuse an id not running with `NO_SIGN_IN_REFUSAL`, kind `usage`.
N11. `sign_in_line(target, ask)` (agents-read.ts:597-605): the acts port, the target (napping refused as N2), gh first
with no step told; a failed gh install refuses `tool_not_installed_line(gh, said)` with fix
`tool_not_installed_fix(gh)`, no kind; else the acts' line.

## Restart

Every sign-in, ended list and key is memory: a restarted host lists none (`a_new_reads_lists_no_sign_in`).

## After the port

- N7: a sign-in stopped by the setup's skip lands nothing even when its tool signed in.
- N3: a follower that left is handed nothing more, but the run keeps its channel until the plan ends.

## Tests

| Rule | Test |
|---|---|
| N1 | `a_second_start_joins_the_running_sign_in`, `a_failed_start_is_not_joined_by_the_next` |
| N2 | `a_refused_plan_opens_no_channel_and_logs_the_refusal`, `a_napping_workspace_is_refused_before_any_plan`, `a_start_logs_its_id_and_names_without_control_characters` |
| N3 | `a_follower_joining_late_is_handed_the_last_step`, `leaving_drops_one_follower_and_the_run_goes_on` |
| N4 | `the_plan_runs_over_the_targets_channel_with_op_last`, `steps_go_to_every_follower_under_the_sign_in_id`, `a_server_sign_in_gets_a_forward_of_its_own_closed_at_the_end` |
| N5 | `gh_goes_on_first_and_says_installing_once`, `a_failed_gh_install_ends_the_sign_in_failed` |
| N6 | `a_plan_that_throws_ends_failed_with_its_message` |
| N7 | `the_end_notes_a_landed_login_before_saying_changed`, `a_minting_agent_here_lands_as_a_key`, `a_server_or_workspace_sign_in_notes_no_login`, `the_end_forgets_the_readers_copy_then_says_changed`, `a_failed_note_is_logged` |
| N8 | `stop_ends_the_run_for_everyone_and_the_next_start_runs_fresh` |
| N9 | `sign_ins_list_running_then_ended_and_drop_ended_past_ten_minutes` |
| N10 | `follow_code_and_stop_refuse_an_id_not_running`, `a_code_goes_to_the_writer_the_run_handed` |
| N11 | `sign_in_line_refuses_a_failed_gh_install_with_its_fix`, `sign_in_line_answers_the_acts_line` |
| Restart | `a_new_reads_lists_no_sign_in` |
