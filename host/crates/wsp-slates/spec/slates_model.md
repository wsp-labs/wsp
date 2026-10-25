# slates_model
Source: slates.ts:81-292 (types, constants, small helpers, the `Slates` interface), 686-758 (`snapOf`,
`refreshSecrets`, `armTimers`, `become`, `valuesFor`, `dropDeclared`, `trimSnapshots`) at 65f6b89c7. Tests:
slates.test.ts `a check rehearses ... a changed command marks its last result stale`, `changing how often a command
runs ... asks again`, `a rewind restores the turn's snapshot`; the snapshot caps are new tests.

## What it does
The stored record of one thread's slate (schema 2), its snapshot shape, the deps the area takes, and the functions
that move a record between documents: which live values a new document keeps, what a dropped declaration leaves
behind, how a snapshot is put back, how timers are armed off a document, and how old snapshots are trimmed.

## Behaviour
1. `SlateRecord` (:100-122), keys in the order `fresh_record` writes them (:516): `threadId, workspaceId,
   rootThreadId, schema (2), version, revision, document, values, empty, turns, kept, comments, approvals, problems,
   shownOnce, updatedAt`, then `previous` and `sendsAllowed` when set. `version` moves only with the document;
   `revision` with every batch that moved a value and every version.
2. `SlateSnap` is `{document, values, [empty]}` (:82-86, 686). `turns` maps a turn id to `{at, version, revision}`
   in the order turns ended; `kept` maps `String(revision)` to a snapshot.
3. `Approval` is `{state, at, [run], [cmd], [scripts]}` (:89-96).
4. `problem(code, name, message, extra)` is `{code, name, message, ...extra}` (:226). `refused(p, kind)` and
   `usage(p)` are errors whose message is `<code> <name>: <message>`, with that kind and the code (:229-231).
5. `sources_named(text)`: of the host sources in their order (`thread, usage, cost, time, git, pr`), those the text
   names as `\b<name>\.` (:234). `own_path(p)` puts a `$` before a path that lacks one (:237).
6. `command_of(decl)`: the declaration less `every`, `always`, `timeout`, `stream` (:240-243). `size_of(v)` is the
   UTF-8 bytes of `JSON.stringify(v)` (:254).
7. `SAID` (:246-252): what each delivery outcome reads under a pressed piece, by the send's kind, word for word.
8. `values_for(r, next)` (:719-735): first each value `next` declares, in its order: a secret its handle; a name the
   old document also declared as a value and the record holds keeps its live value; else a copy of its `start`.
   Then each run `next` declares: a run the old document did not declare, or whose value is not a run record, reads
   `{state:"idle", runs:0}`; a done, failed or cancelled record whose `command_of` changed gets `stale: true` (added
   last); any other keeps its record.
9. `drop_declared(r, next)` (:738-745): each secret the old document declared that `next` does not declare as a
   secret is forgotten; each run it declared that `next` does not is cancelled in the runs table and the MCP runs.
10. `refresh_secrets(r)` writes each declared secret's handle as read now (:689-691).
11. `arm_timers(r, stopped)` (:693-698): each run with `every` not in `stopped`, in document order, as
    `{run, every, key, [always: true]}` where `key` is the run key of the approval declaration for a `cmd` run and
    `JSON.stringify(decl)` for the others; then the MCP servers: every non-`cmd` run's server, and those whose run has
    `every` and `always` as kept.
12. `become(r, snap)` (:702-715): every run of the thread stopped quietly in both tables; document and a copy of the
    values from the snapshot; each declared run whose value reads running becomes `cancelled`, `the thread was
    rewound`; secrets refreshed; `empty` set to the snapshot's (or `none`) when it has no document, dropped
    otherwise; timers armed. Nothing fires.
13. `trim_snapshots(r)` (:748-758): turns past the newest 100 dropped oldest first; every kept snapshot no turn points
    at dropped; then while more than one turn is left and the kept snapshots' JSON passes 8 MiB in UTF-8 bytes, the
    oldest turn and every snapshot no turn points at go.
14. Constants (:210-221): `SNAPSHOTS_KEPT` 100, `SNAPSHOT_BYTES` 8,388,608, `MESSAGE_CHARS` 20,000,
    `REQUEST_KEPT_MS` 600,000, `REQUESTS_KEPT` 500, `EVENTS_PER_SECOND` 5, `PRESS_SEND_MS` 2,000,
    `REACTION_SEND_MS` 60,000, `PROBLEMS_KEPT` 20, `PIECES_NAMED` 20; `SLATE_SEND_KEY` is `send`.

## Edge cases
- A value whose declaration moved from value to run (or back) starts fresh: idle, or its `start`.
- A command written again with only `timeout` changed keeps its result fresh; a changed `cmd` marks a done result
  stale and keeps `out` (`changed command marks its last result stale`).
- The 101st turn drops the first turn's pointer and the snapshot only it used; a turn that ends twice moves to the
  end of `turns` (:1502-1503).
- An 8 MiB cap with one turn left keeps that turn whatever its size.

## Behaviour nobody wrote down
- Rule 3.11: `turns` order is the order turns ended, and the trim drops in that order.
- Rule R7: a rewind's or a restart's secret is read off the runs table; neither fills nor empties a secret.

## Decisions
1. `SlateRecord`, `SlateSnap`, `Approval` are serde structs in the order of 1-3; a record read back from the store
   whose `empty` was deleted and set again had it last in the TS (an accepted difference, state.db only).
2. `SlatesDeps` is one trait (13.6): optional TS members are methods answering `Option` or a default.

## After the port
- None.

## Tests

29 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| slates_access Behaviour 11, slates_model Behaviour 2: a turn's snapshot is a copy, and a turn ending again moves last | `slates_access::targets::restart::turns::a_turn_s_snapshot_is_a_copy_and_a_turn_ending_again_moves_to_the_end` |
| slates_model Behaviour 13 / Edge case: the 101st turn drops the first turn's pointer and the snapshot only it used | `slates_model::the_101st_turn_drops_the_first_turn_and_the_snapshot_only_it_used` |
| slates_model Behaviour 13: a turn where nothing changed shares its snapshot | `slates_model::turns_that_share_a_revision_share_one_snapshot_and_the_trim_keeps_it` |
| slates_model Behaviour 13: every kept snapshot no turn points at goes | `slates_model::a_kept_snapshot_no_turn_points_at_is_dropped` |
| slates_model Behaviour 13: past 8 MiB the oldest turn and its snapshot go | `slates_model::past_8_mib_of_snapshots_the_oldest_turn_and_the_snapshot_only_it_used_go` |
| slates_model Behaviour 13: the byte cap is a cap, not a count | `slates_model::snapshots_under_8_mib_are_all_kept` |
| slates_model Behaviour 13: the loop drops turn after turn until the snapshots fit | `slates_model::the_byte_cap_drops_as_many_old_turns_as_it_takes` |
| slates_model Edge case: an 8 MiB cap with one turn left keeps that turn whatever its size | `slates_model::one_turn_is_kept_whatever_its_size` |
| slates_model Behaviour 6: the declaration less every, always, timeout and stream | `slates_model::command_of_leaves_out_every_always_timeout_and_stream` |
| slates_model Edge case: a command written again with only timeout changed keeps its result fresh | `slates_model::command_of_is_equal_when_only_how_often_or_how_long_changed` |
| slates_model Behaviour 5 | `slates_model::own_path_puts_a_dollar_before_a_path_that_lacks_one` |
| slates_model Behaviour 5 | `slates_model::sources_named_answers_the_host_sources_the_text_names_in_their_own_order` |
| slates_model Behaviour 5: the source name must start a word and be followed by a dot | `slates_model::sources_named_needs_a_word_start_and_a_dot` |
| slates_model Behaviour 4 | `slates_model::problem_is_code_name_message_and_a_fix_when_given` |
| slates_model Behaviour 4 | `slates_model::refused_and_usage_say_code_name_message_and_carry_the_kind_and_the_code` |
| slates_model Behaviour 7 | `slates_model::said_reads_each_outcome_under_a_pressed_piece_by_the_send_s_kind` |
| slates_model Behaviour 6 | `slates_model::size_of_is_the_utf8_bytes_of_the_json_text` |
| slates_model Behaviour 1: the stored record's keys in fresh_record's order, previous and sendsAllowed last | `slates_model::a_stored_record_keeps_the_keys_of_a_fresh_one_in_order_and_adds_previous_and_sends_allowed_last` |
| slates_model Behaviour 2: turns by turn id in the order they ended, a snapshot by revision | `slates_model::a_turn_s_end_writes_a_pointer_and_one_snapshot_and_a_turn_that_ends_twice_moves_to_the_end` |
| slates_model Behaviour 8 / Edge case: a value whose declaration moved from value to run (or back) starts fresh | `slates_model::a_value_that_moved_to_a_run_starts_idle_and_a_run_that_moved_to_a_value_starts_at_its_start` |
| slates_model Behaviour 8: names both documents declare keep their live value, new names start, old names go | `slates_model::a_new_document_keeps_live_values_starts_new_names_and_drops_old_ones` |
| slates_model Behaviour 8: each value in the new document's order, then each run | `slates_model::the_live_values_of_a_new_document_list_its_values_first_then_its_runs` |
| slates_model Behaviour 8, 9: a secret keeps its handle through a new document and a dropped one is forgotten | `slates_model::a_secret_keeps_its_handle_through_a_new_document_and_a_dropped_one_is_forgotten` |
| slates_model Behaviour 8 / Edge case: a changed command marks a done result stale and keeps out | `slates_model::a_changed_command_marks_a_done_result_stale_last_in_the_record_and_keeps_its_out` |
| slates_model Behaviour 9: a run the new document does not declare is cancelled | `slates_model::a_run_a_new_document_drops_is_cancelled_and_leaves_nothing_of_its_record` |
| slates_model Behaviour 11: a timed run is armed off the document, on its period | `slates_model::a_document_with_an_always_timer_arms_it_on_its_period_and_a_new_every_arms_again` |
| slates_model Behaviour 12 / Edge case: slates.test.ts:219 | `slates_model::a_rewind_restores_the_turn_s_snapshot_cancels_a_run_in_flight_with_no_reaction` |
| slates_model Behaviour 12 / Behaviour nobody wrote down: a rewind neither fills nor empties a secret | `slates_model::a_rewind_reads_a_secret_off_the_runs_table_and_never_fills_or_empties_it` |
| slates_model Behaviour 12: a rewind to a turn with no pointer empties the slate and a read says so | `slates_model::a_rewind_to_a_turn_with_no_pointer_reads_rewound_before_in_a_read` |
