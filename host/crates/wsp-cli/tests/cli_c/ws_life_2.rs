// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from ws_life.json: one test per recorded case.
use super::common::holds;
use super::drivers_ws_life as d;

#[test]
fn nap_after_dead_launch__answers_the_same_where_the_host_cannot_be_reached_at_all() {
    holds("ws_life", "napAfterDeadLaunch", "answers the same where the host cannot be reached at all", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__the_thread_read_refused() {
    holds("ws_life", "napAfterDeadLaunch", "the thread read refused", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__the_nap_refused() {
    holds("ws_life", "napAfterDeadLaunch", "the nap refused", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__the_host_closes_during_the_thread_read() {
    holds("ws_life", "napAfterDeadLaunch", "the host closes during the thread read", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__the_workspace_is_named_in_the_thread_read() {
    holds("ws_life", "napAfterDeadLaunch", "the workspace is named in the thread read", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_wake_this_call_did_not_make_owes_nothing_cloud() {
    holds("ws_life", "napAfterDeadLaunch", "a wake this call did not make owes nothing (cloud)", d::nap_after_dead_launch);
}

#[test]
fn with_line__a_refusal_with_a_kind_keeps_the_kind() {
    holds("ws_life", "withLine", "a refusal with a kind keeps the kind", d::with_line);
}

#[test]
fn with_line__a_usage_refusal() {
    holds("ws_life", "withLine", "a usage refusal", d::with_line);
}

#[test]
fn with_line__a_plain_error_has_no_kind() {
    holds("ws_life", "withLine", "a plain error has no kind", d::with_line);
}

#[test]
fn with_line__no_line_returns_the_error_as_it_was() {
    holds("ws_life", "withLine", "no line returns the error as it was", d::with_line);
}

#[test]
fn with_line__an_empty_line_is_a_line() {
    holds("ws_life", "withLine", "an empty line is a line", d::with_line);
}

#[test]
fn with_line__the_fix_is_not_carried() {
    holds("ws_life", "withLine", "the fix is not carried", d::with_line);
}

#[test]
fn with_line__a_problem_list_is_not_carried() {
    holds("ws_life", "withLine", "a problem list is not carried", d::with_line);
}

#[test]
fn with_line__a_multi_line_line() {
    holds("ws_life", "withLine", "a multi-line line", d::with_line);
}

#[test]
fn with_line__a_message_that_is_already_two_lines() {
    holds("ws_life", "withLine", "a message that is already two lines", d::with_line);
}

#[test]
fn with_line__a_refusal_with_a_kind_keeps_the_kind_cloud() {
    holds("ws_life", "withLine", "a refusal with a kind keeps the kind (cloud)", d::with_line);
}

#[test]
fn stop__stopped_with_the_tree() {
    holds("ws_life", "stop", "stopped with the tree", d::stop);
}

#[test]
fn stop__the_latest_turn_s_session_id_is_the_one_stopped() {
    holds("ws_life", "stop", "the latest turn's session id is the one stopped", d::stop);
}

#[test]
fn stop__an_empty_under_is_dropped() {
    holds("ws_life", "stop", "an empty under is dropped", d::stop);
}

#[test]
fn stop__stopped_with_a_process_left_running() {
    holds("ws_life", "stop", "stopped with a process left running", d::stop);
}

#[test]
fn stop__not_found_by_the_host() {
    holds("ws_life", "stop", "not found by the host", d::stop);
}

#[test]
fn stop__refused_with_its_reason() {
    holds("ws_life", "stop", "refused with its reason", d::stop);
}

#[test]
fn stop__unsupported() {
    holds("ws_life", "stop", "unsupported", d::stop);
}

#[test]
fn stop__one_subagent_stopped() {
    holds("ws_life", "stop", "one subagent stopped", d::stop);
}

#[test]
fn stop__one_subagent_an_empty_name() {
    holds("ws_life", "stop", "one subagent, an empty name", d::stop);
}

#[test]
fn stop__one_subagent_already_over() {
    holds("ws_life", "stop", "one subagent already over", d::stop);
}

#[test]
fn stop__every_field_of_the_answer_in_the_order_kept() {
    holds("ws_life", "stop", "every field of the answer, in the order kept", d::stop);
}

#[test]
fn stop__an_unknown_key_of_the_answer_is_dropped() {
    holds("ws_life", "stop", "an unknown key of the answer is dropped", d::stop);
}

#[test]
fn stop__no_such_thread_asks_no_stop() {
    holds("ws_life", "stop", "no such thread asks no stop", d::stop);
}

#[test]
fn stop__a_prefix_naming_two_asks_no_stop() {
    holds("ws_life", "stop", "a prefix naming two asks no stop", d::stop);
}

#[test]
fn stop__the_stop_refused_by_the_host() {
    holds("ws_life", "stop", "the stop refused by the host", d::stop);
}

#[test]
fn stop__the_host_closes_during_the_stop() {
    holds("ws_life", "stop", "the host closes during the stop", d::stop);
}

#[test]
fn stop__stopped_with_the_tree_cloud() {
    holds("ws_life", "stop", "stopped with the tree (cloud)", d::stop);
}

#[test]
fn stop__the_stop_refused_by_the_host_cloud() {
    holds("ws_life", "stop", "the stop refused by the host (cloud)", d::stop);
}

#[test]
fn stop_line__accepted() {
    holds("ws_life", "stopLine", "accepted", d::stop_line);
}

#[test]
fn stop_line__not_running() {
    holds("ws_life", "stopLine", "not running", d::stop_line);
}

#[test]
fn stop_line__not_found() {
    holds("ws_life", "stopLine", "not found", d::stop_line);
}

#[test]
fn stop_line__refused() {
    holds("ws_life", "stopLine", "refused", d::stop_line);
}

#[test]
fn stop_line__unsupported() {
    holds("ws_life", "stopLine", "unsupported", d::stop_line);
}

#[test]
fn stop_line__a_subagent_named() {
    holds("ws_life", "stopLine", "a subagent named", d::stop_line);
}

#[test]
fn stop_line__an_error_says_it_in_place_of_the_outcome() {
    holds("ws_life", "stopLine", "an error says it in place of the outcome", d::stop_line);
}

#[test]
fn stop_line__an_error_on_a_subagent() {
    holds("ws_life", "stopLine", "an error on a subagent", d::stop_line);
}

#[test]
fn stop_line__an_error_beats_a_tree_and_a_left() {
    holds("ws_life", "stopLine", "an error beats a tree and a left", d::stop_line);
}

#[test]
fn stop_line__one_thread_under_it() {
    holds("ws_life", "stopLine", "one thread under it", d::stop_line);
}

#[test]
fn stop_line__two_threads_under_it() {
    holds("ws_life", "stopLine", "two threads under it", d::stop_line);
}

#[test]
fn stop_line__an_empty_under_says_nothing() {
    holds("ws_life", "stopLine", "an empty under says nothing", d::stop_line);
}

#[test]
fn stop_line__a_short_id_under_it() {
    holds("ws_life", "stopLine", "a short id under it", d::stop_line);
}

#[test]
fn stop_line__a_process_left_on_a_computer() {
    holds("ws_life", "stopLine", "a process left on a computer", d::stop_line);
}

#[test]
fn stop_line__a_tree_and_a_left() {
    holds("ws_life", "stopLine", "a tree and a left", d::stop_line);
}

#[test]
fn stop_line__not_running_with_a_left() {
    holds("ws_life", "stopLine", "not running with a left", d::stop_line);
}

#[test]
fn stop_line__a_subagent_with_a_tree_and_a_left() {
    holds("ws_life", "stopLine", "a subagent with a tree and a left", d::stop_line);
}

#[test]
fn stop_line__accepted_cloud() {
    holds("ws_life", "stopLine", "accepted (cloud)", d::stop_line);
}

#[test]
fn forget_thread__a_thread_that_never_ran() {
    holds("ws_life", "forgetThread", "a thread that never ran", d::forget_thread);
}

#[test]
fn forget_thread__a_check_asked() {
    holds("ws_life", "forgetThread", "a check asked", d::forget_thread);
}

#[test]
fn forget_thread__a_check_not_asked_is_not_sent() {
    holds("ws_life", "forgetThread", "a check not asked is not sent", d::forget_thread);
}

#[test]
fn forget_thread__the_thread_id_not_the_row_id_is_sent() {
    holds("ws_life", "forgetThread", "the thread id, not the row id, is sent", d::forget_thread);
}

#[test]
fn forget_thread__a_row_from_before_threads_is_refused_before_any_frame() {
    holds("ws_life", "forgetThread", "a row from before threads is refused before any frame", d::forget_thread);
}

#[test]
fn forget_thread__a_row_from_before_threads_a_check_asked() {
    holds("ws_life", "forgetThread", "a row from before threads, a check asked", d::forget_thread);
}

#[test]
fn forget_thread__refused_once_a_turn_ran() {
    holds("ws_life", "forgetThread", "refused once a turn ran", d::forget_thread);
}

#[test]
fn forget_thread__the_host_closes_during_the_forget() {
    holds("ws_life", "forgetThread", "the host closes during the forget", d::forget_thread);
}

#[test]
fn thread_forgot_line__a_thread() {
    holds("ws_life", "threadForgotLine", "a thread", d::thread_forgot_line);
}

#[test]
fn thread_forgot_line__a_row_id_with_spaces() {
    holds("ws_life", "threadForgotLine", "a row id with spaces", d::thread_forgot_line);
}

#[test]
fn forget_thread__a_thread_that_never_ran_cloud() {
    holds("ws_life", "forgetThread", "a thread that never ran (cloud)", d::forget_thread);
}

#[test]
fn settle_threads__one_thread() {
    holds("ws_life", "settleThreads", "one thread", d::settle_threads);
}

#[test]
fn settle_threads__two_threads_in_the_order_given() {
    holds("ws_life", "settleThreads", "two threads, in the order given", d::settle_threads);
}

#[test]
fn settle_threads__no_threads_named() {
    holds("ws_life", "settleThreads", "no threads named", d::settle_threads);
}

#[test]
fn settle_threads__finished_only() {
    holds("ws_life", "settleThreads", "finished only", d::settle_threads);
}

#[test]
fn settle_threads__the_row_id_is_sent_not_the_thread_id() {
    holds("ws_life", "settleThreads", "the row id is sent, not the thread id", d::settle_threads);
}

#[test]
fn settle_threads__processes_stop() {
    holds("ws_life", "settleThreads", "processes stop", d::settle_threads);
}

#[test]
fn settle_threads__processes_keep() {
    holds("ws_life", "settleThreads", "processes keep", d::settle_threads);
}

#[test]
fn settle_threads__processes_ask() {
    holds("ws_life", "settleThreads", "processes ask", d::settle_threads);
}

#[test]
fn settle_threads__one_settled_and_one_left() {
    holds("ws_life", "settleThreads", "one settled and one left", d::settle_threads);
}

#[test]
fn settle_threads__ended_processes_are_kept() {
    holds("ws_life", "settleThreads", "ended processes are kept", d::settle_threads);
}

#[test]
fn settle_threads__an_unknown_key_of_the_answer_is_dropped() {
    holds("ws_life", "settleThreads", "an unknown key of the answer is dropped", d::settle_threads);
}

#[test]
fn settle_threads__refused_while_processes_run() {
    holds("ws_life", "settleThreads", "refused while processes run", d::settle_threads);
}

#[test]
fn settle_threads__refused_for_a_thread_under_another_lead() {
    holds("ws_life", "settleThreads", "refused for a thread under another lead", d::settle_threads);
}

#[test]
fn restore_threads__one_thread() {
    holds("ws_life", "restoreThreads", "one thread", d::restore_threads);
}

#[test]
fn restore_threads__two_threads_in_the_order_given() {
    holds("ws_life", "restoreThreads", "two threads, in the order given", d::restore_threads);
}

#[test]
fn restore_threads__none_named() {
    holds("ws_life", "restoreThreads", "none named", d::restore_threads);
}

#[test]
fn restore_threads__the_row_id_is_sent() {
    holds("ws_life", "restoreThreads", "the row id is sent", d::restore_threads);
}

#[test]
fn restore_threads__refused_by_the_host() {
    holds("ws_life", "restoreThreads", "refused by the host", d::restore_threads);
}

#[test]
fn settle_threads__one_thread_cloud() {
    holds("ws_life", "settleThreads", "one thread (cloud)", d::settle_threads);
}

#[test]
fn restore_threads__refused_by_the_host_cloud() {
    holds("ws_life", "restoreThreads", "refused by the host (cloud)", d::restore_threads);
}

#[test]
fn settled_lines__nothing() {
    holds("ws_life", "settledLines", "nothing", d::settled_lines);
}

#[test]
fn settled_lines__one_settled() {
    holds("ws_life", "settledLines", "one settled", d::settled_lines);
}

#[test]
fn settled_lines__two_settled_in_order() {
    holds("ws_life", "settledLines", "two settled, in order", d::settled_lines);
}

#[test]
fn settled_lines__one_left() {
    holds("ws_life", "settledLines", "one left", d::settled_lines);
}

#[test]
fn settled_lines__settled_then_left() {
    holds("ws_life", "settledLines", "settled then left", d::settled_lines);
}

#[test]
fn settled_lines__a_title_with_a_quote_and_an_accent() {
    holds("ws_life", "settledLines", "a title with a quote and an accent", d::settled_lines);
}

#[test]
fn settled_lines__an_empty_title() {
    holds("ws_life", "settledLines", "an empty title", d::settled_lines);
}

#[test]
fn settled_lines__a_short_thread_id() {
    holds("ws_life", "settledLines", "a short thread id", d::settled_lines);
}

#[test]
fn settled_lines__ended_processes_are_not_said() {
    holds("ws_life", "settledLines", "ended processes are not said", d::settled_lines);
}

#[test]
fn settled_lines__nothing_cloud() {
    holds("ws_life", "settledLines", "nothing (cloud)", d::settled_lines);
}

#[test]
fn restored_lines__nothing() {
    holds("ws_life", "restoredLines", "nothing", d::restored_lines);
}

#[test]
fn restored_lines__one() {
    holds("ws_life", "restoredLines", "one", d::restored_lines);
}

#[test]
fn restored_lines__two_in_order() {
    holds("ws_life", "restoredLines", "two, in order", d::restored_lines);
}

#[test]
fn restored_lines__a_short_id_and_an_empty_title() {
    holds("ws_life", "restoredLines", "a short id and an empty title", d::restored_lines);
}

#[test]
fn restored_lines__nothing_cloud() {
    holds("ws_life", "restoredLines", "nothing (cloud)", d::restored_lines);
}

#[test]
fn rename__renamed() {
    holds("ws_life", "rename", "renamed", d::rename);
}

#[test]
fn rename__the_session_id_of_the_thread_s_latest_turn_is_sent() {
    holds("ws_life", "rename", "the session id of the thread's latest turn is sent", d::rename);
}

#[test]
fn rename__unsupported() {
    holds("ws_life", "rename", "unsupported", d::rename);
}

#[test]
fn rename__no_session() {
    holds("ws_life", "rename", "no session", d::rename);
}

#[test]
fn rename__failed_with_the_machine_s_line() {
    holds("ws_life", "rename", "failed with the machine's line", d::rename);
}

#[test]
fn rename__failed_with_no_line() {
    holds("ws_life", "rename", "failed with no line", d::rename);
}

#[test]
fn rename__not_found_by_the_host() {
    holds("ws_life", "rename", "not found by the host", d::rename);
}

#[test]
fn rename__an_error_beside_an_outcome_that_is_not_failed_is_still_kept() {
    holds("ws_life", "rename", "an error beside an outcome that is not failed is still kept", d::rename);
}

#[test]
fn rename__an_empty_title_is_sent() {
    holds("ws_life", "rename", "an empty title is sent", d::rename);
}

#[test]
fn rename__a_title_with_a_newline_and_an_emoji_is_sent_as_given() {
    holds("ws_life", "rename", "a title with a newline and an emoji is sent as given", d::rename);
}

#[test]
fn rename__a_row_from_before_threads_is_named_by_its_row_id() {
    holds("ws_life", "rename", "a row from before threads is named by its row id", d::rename);
}

#[test]
fn rename__an_unknown_key_of_the_answer_is_dropped() {
    holds("ws_life", "rename", "an unknown key of the answer is dropped", d::rename);
}

#[test]
fn rename__refused_by_the_host() {
    holds("ws_life", "rename", "refused by the host", d::rename);
}

#[test]
fn rename__the_host_closes_during_the_rename() {
    holds("ws_life", "rename", "the host closes during the rename", d::rename);
}

#[test]
fn rename__renamed_cloud() {
    holds("ws_life", "rename", "renamed (cloud)", d::rename);
}

#[test]
fn rename__refused_by_the_host_cloud() {
    holds("ws_life", "rename", "refused by the host (cloud)", d::rename);
}

#[test]
fn rename_line__claude_renamed() {
    holds("ws_life", "renameLine", "claude renamed", d::rename_line);
}

#[test]
fn rename_line__claude_unsupported() {
    holds("ws_life", "renameLine", "claude unsupported", d::rename_line);
}

#[test]
fn rename_line__claude_no_session() {
    holds("ws_life", "renameLine", "claude no-session", d::rename_line);
}

#[test]
fn rename_line__claude_failed() {
    holds("ws_life", "renameLine", "claude failed", d::rename_line);
}

#[test]
fn rename_line__claude_not_found() {
    holds("ws_life", "renameLine", "claude not-found", d::rename_line);
}

#[test]
fn rename_line__codex_renamed() {
    holds("ws_life", "renameLine", "codex renamed", d::rename_line);
}

#[test]
fn rename_line__codex_unsupported() {
    holds("ws_life", "renameLine", "codex unsupported", d::rename_line);
}

#[test]
fn rename_line__codex_no_session() {
    holds("ws_life", "renameLine", "codex no-session", d::rename_line);
}

#[test]
fn rename_line__codex_failed() {
    holds("ws_life", "renameLine", "codex failed", d::rename_line);
}

#[test]
fn rename_line__codex_not_found() {
    holds("ws_life", "renameLine", "codex not-found", d::rename_line);
}

#[test]
fn rename_line__someone_else_renamed() {
    holds("ws_life", "renameLine", "someone-else renamed", d::rename_line);
}

#[test]
fn rename_line__someone_else_unsupported() {
    holds("ws_life", "renameLine", "someone-else unsupported", d::rename_line);
}

#[test]
fn rename_line__someone_else_no_session() {
    holds("ws_life", "renameLine", "someone-else no-session", d::rename_line);
}

#[test]
fn rename_line__someone_else_failed() {
    holds("ws_life", "renameLine", "someone-else failed", d::rename_line);
}

#[test]
fn rename_line__someone_else_not_found() {
    holds("ws_life", "renameLine", "someone-else not-found", d::rename_line);
}

#[test]
fn rename_line__claude_failed_with_error_eacces_the_store_is_read_only() {
    holds("ws_life", "renameLine", "claude failed with error \"EACCES: the store is read-only\"", d::rename_line);
}

#[test]
fn rename_line__claude_failed_with_error() {
    holds("ws_life", "renameLine", "claude failed with error \"\"", d::rename_line);
}

#[test]
fn rename_line__claude_renamed_with_error_ignored() {
    holds("ws_life", "renameLine", "claude renamed with error \"ignored\"", d::rename_line);
}

#[test]
fn rename_line__claude_not_found_with_error_ignored() {
    holds("ws_life", "renameLine", "claude not-found with error \"ignored\"", d::rename_line);
}

#[test]
fn rename_line__codex_renamed_and_no_title() {
    holds("ws_life", "renameLine", "codex renamed and no title", d::rename_line);
}

#[test]
fn rename_line__claude_renamed_with_a_hard_title() {
    holds("ws_life", "renameLine", "claude renamed with a hard title", d::rename_line);
}

#[test]
fn rename_line__claude_renamed_cloud() {
    holds("ws_life", "renameLine", "claude renamed (cloud)", d::rename_line);
}
