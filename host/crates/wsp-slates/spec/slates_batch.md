# slates_batch
Source: slates.ts:405-424 (`approvalDecl`, `approvalNames`), 760-884 (inputs, where a run runs, starts, holds), 888-1015
(`batch`, `compose`, `deliverAll`) at 65f6b89c7. Tests: slates.test.ts `runs the setup chain without the agent`, `a
run that starts again keeps its last result`, `the agent's own starts meet the start budget`, `takes five of the
person's events`, `a run starts in the thread's own folder`, `an Always covers the scripts`, `a confirm formula
reaches the sheet`, and the press-and-send half of `the slate v2 host, round 4`.

## What it does
One arrival through the batch: a window's write, the agent's write, a run's end, a timer's tick or a press. The
module's pure core runs over the values; the starts it asks for run here, after the values are stored; the
cancels go to the runs table; the sends are composed into a message for the thread, at most one a press every 2 s
and one a reaction a minute; the values are saved and pushed. It also decides where a run runs and what its
approval covers.

## Behaviour
1. `approval_decl(r, decl)` (:407-411): the declaration with the slate files it reads (`with_files`) and, when the
   pin hashed any, `scripts`. `approval_names(r, key)` (:415-424): the MCP table's names when it owns the key; else
   the first declared run whose approval key is this one: a `cmd` run `{run, cmd, [scripts]}`, another
   `{run, cmd: "<server>.<tool or uri>"}`; else nothing.
2. `inputs_of(r, decl)` (:764-778), read when the command starts: a bare binding to a declared secret goes as
   `{secret: name}`; anything else is resolved against the views kept for the thread (`views_now`) as
   `{value: v or null}`; env, args and stdin only when declared.
3. `mcp_args_of(r, decl)` (:781-793): a tool's args resolved with every bare declared secret read as its mark.
4. `on_machine` (:799): a `cmd` run not `on: host` whose thread has a machine. `folder_for` (:821-824): the thread's
   host folder (else its folder) for `on: host`, else its folder. `pin_for` (:801): the machine's pins there, else
   `pin_here`. `startable(r)` (:803-806): the views, with the machine's pins read again first when any run runs there.
   `wake_for` (:808-811): a napping machine is woken (and pins read) only for a start not by a timer whose pin is
   unread. `held_again(r, views)` (:813-819): each held ask whose key moved is started again `by: person` and its
   new record written; answers their names.
5. `start_now(r, run, by, views, prior)` (:828-859): an undeclared run reads `{state:"failed", why: "$<run> is not
   declared", runs}`; a `cmd` run with no folder `{state:"failed", why: NO_FOLDER, runs}` (`runs` from the prior
   record or 0). Otherwise the views are kept for the thread, the start is captured, the confirm formula resolved
   (empty text means `Run $<run>?`), and the MCP table or the runs table starts it with the prior record as `last`
   and, for a `cmd` run not on the host of a napping machine, `asleep`. Who started it is remembered; a running
   answer announces `run`. It answers the record and, when held, the ask.
6. `provisional(r, run)` (:863-869): an MCP run's own provisional record; a `cmd` run approved always gives the
   running record (with the prior's result and runs + 1); else `{state:"held", why: "needs your approval", runs}`.
   `approved_always` (:872-875): a `cmd` run with no `confirm` whose approval key is allowed.
7. `run_moved(thread, run, record)` (:879-884): an undeclared run does nothing; a running record announces `run` by
   whoever started it (`person` by default); then one batch `by: run` writing the record.
8. `batch(r, input, by, o)` (:897-963): no document answers nothing. In order: views (`startable`); the context with
   `by` (`timer` reads as `run`), the event, whether reaction sends are allowed, `react: false` when asked, and a
   `start` that records a deferred start with its provisional record; `wake_for` the timer and agent starts; those
   starts run now and their records join the input; the module's batch; values over 262,144 UTF-8 bytes keep `S500
   values-too-big: the values would be <n> bytes; a slate holds at most 262144`, save, and answer nothing; else the
   values and problems are kept, cancels go to the runs table, `wake_for` the deferred starts, and each deferred
   start runs against the values from before the batch: an ask is collected; a record in the provisional state is
   written, one in another state goes through its own `run_moved` on the chain. Then the sends: a press's send
   within 2,000 ms of the thread's last refuses conflict `V753 send-rate: too fast; try again in a moment`; a
   reaction's within 60,000 ms keeps `R912 reaction-failed: reaction <name or ?>: V753 send-rate, one message a
   minute from reactions` and is skipped; the rest stamp the time and compose. Finally the names whose value
   changed (both old and new names, `slate_equal`); a change or a problem moves the revision, saves and pushes them.
   It answers the asks, the deferred starts' names and the sends `{prompt, kind, requestId}` (the request id given,
   or `<thread>:<version>:<reaction or piece or "slate">:<now>`).
9. `compose(r, send, views, event, now)` (:967-1004): each `with` path's value (the batch's own, else evaluated),
   scrubbed and defanged; the piece (the send's, or the pressed one for a press); the label of the pressed row action
   or the piece, its first 80 units; a row's `key` prop resolved when the event has an index. The message is the
   text, a blank line, then `slate: ` and the JSON `{v:2, kind ("action"|"reaction"), thread (word), version,
   [piece], [label], [event (press only)], [reaction], [index (press only)], [key], [with], by, at (ISO)}`. While it
   passes 20,000 units, at most 50 times, the longest carried string (20 units or more) is cut to
   `max(0, length - over - 10)` units plus ` (cut)` and the message rebuilt.
10. `deliver_all(r, sends)` (:1008-1015): each send delivered in order, awaited; answers the first outcome with its
    turn and the send's kind.

## Edge cases
- The setup chain: a window's write starts a held check; Always runs it with `PROJECT` in its env; its done sets
  the step; no turn starts.
- A run started again keeps its last result `refreshing` until the new one ends; the first run had nothing to keep;
  `done` fires once per result (`a run that starts again keeps its last result`).
- The press's message carries the handle, never the token, and `$env.out` scrubbed.
- An approved run starts in the thread's own folder, `SLATE_DIR` outside it, across a restart.
- The agent's thirteenth start in a minute waits on the budget whatever call carries it.

## Behaviour nobody wrote down
- Rule R1: `start_now` sets `capturing` around the runs table's start, so the record that start writes is the
  batch's, not a batch of its own; the capture is cleared even when the start fails.
- Rule 3.4: deferred starts run after the values are stored in the record, so a run's env reads them.

## Decisions
1. `capturing` is a `Cell<Option<(String, String)>>` on the area.

## After the port
- None.

## Tests

13 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| slates_batch Behaviour 5-10: slates.test.ts:104-187 (the restart half is slates_access') | `slates_batch::runs_the_setup_chain_without_the_agent_and_keeps_the_secret_from_it` |
| slates_batch Behaviour 5, 6, 8: slates.test.ts:749 (a run that starts again keeps its last result) | `slates_batch::a_run_that_starts_again_keeps_its_last_result_marked_refreshing_across_a_restart` |
| slates_batch Behaviour 5, rule R1: slates.test.ts:1194 (the agent's own starts meet the start budget) | `slates_batch::the_agent_s_own_starts_meet_the_start_budget_however_many_calls_carry_them` |
| slates_batch Behaviour 4, 5: slates.test.ts:851 (a run starts in the thread's own folder, across a restart) | `slates_batch::a_run_starts_in_the_thread_s_own_folder_and_slate_dir_is_outside_it` |
| slates_batch Behaviour 5: a cmd run of a thread with no folder fails at once with NO_FOLDER (slates.ts:833) | `slates_batch::a_command_of_a_thread_with_no_folder_fails_without_starting` |
| slates_batch Behaviour 1, 8: slates.test.ts:426 (an Always covers the scripts a command names) | `slates_batch::an_always_covers_the_scripts_a_command_names_an_edit_asks_again_and_names_it` |
| slates_batch Behaviour 5: slates.test.ts:574 (a confirm formula reaches the sheet as its text) | `slates_batch::a_confirm_formula_reaches_the_sheet_as_its_text_read_when_the_run_is_held` |
| slates_batch Behaviour 9: a message past 20,000 units cuts its longest value to fit, marked (slates.ts:997-1002) | `slates_batch::the_message::a_message_past_its_cap_cuts_the_longest_value_to_fit` |
| slates_batch Behaviour 9, rule 12 (injection): a carried line that reads as a second slate: line is defanged | `slates_batch::the_message::a_carried_value_that_reads_as_a_slate_line_is_made_harmless` |
| slates_batch Behaviour 8: a press's second send within 2 s is refused, and not remembered (slates.ts:946-947) | `slates_batch::the_message::a_second_press_send_within_two_seconds_is_refused` |
| slates_batch Behaviour 8: a reaction's second send within a minute is skipped and kept as R912 (slates.ts:948-949) | `slates_batch::the_message::a_reaction_send_within_a_minute_of_the_last_is_skipped_and_kept_as_a_problem` |
| slates_batch Behaviour 9: a label over 80 units is cut, and a send with no with carries none (slates.ts:977, 992) | `slates_batch::the_message::the_label_is_cut_to_80_units_and_a_send_with_nothing_carries_no_with` |
| slates_batch Behaviour 8: values past 256 KiB keep S500 and leave the values as they were (slates.ts:923-927) | `slates_batch::values_past_the_cap_keep_a_problem_and_change_nothing` |
