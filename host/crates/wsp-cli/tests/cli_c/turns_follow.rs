// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_follow.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_follow as d;

#[test]
fn start_detached__started() {
    holds("turns_follow", "startDetached", "started", d::start_detached);
}

#[test]
fn start_detached__the_fields_go_out_in_order_then_startedby_requestid_and_answ() {
    holds("turns_follow", "startDetached", "the fields go out in order, then startedBy, requestId and answerHeld", d::start_detached);
}

#[test]
fn start_detached__started_by_another_origin() {
    holds("turns_follow", "startDetached", "started by another origin", d::start_detached);
}

#[test]
fn start_detached__a_notice_rides_the_turn() {
    holds("turns_follow", "startDetached", "a notice rides the turn", d::start_detached);
}

#[test]
fn start_detached__queued_hearing_one_wait() {
    holds("turns_follow", "startDetached", "queued, hearing one wait", d::start_detached);
}

#[test]
fn start_detached__queued_hearing_the_wait_for_a_computer() {
    holds("turns_follow", "startDetached", "queued, hearing the wait for a computer", d::start_detached);
}

#[test]
fn start_detached__queued_hearing_two_waits_in_order() {
    holds("turns_follow", "startDetached", "queued, hearing two waits in order", d::start_detached);
}

#[test]
fn start_detached__a_wait_with_another_request_id_is_not_this_start_s() {
    holds("turns_follow", "startDetached", "a wait with another request id is not this start's", d::start_detached);
}

#[test]
fn start_detached__a_wait_with_no_request_id_is_not_this_start_s() {
    holds("turns_follow", "startDetached", "a wait with no request id is not this start's", d::start_detached);
}

#[test]
fn start_detached__no_one_listening_for_waits() {
    holds("turns_follow", "startDetached", "no one listening for waits", d::start_detached);
}

#[test]
fn start_detached__a_steer_and_a_starting_are_not_waits() {
    holds("turns_follow", "startDetached", "a steer and a starting are not waits", d::start_detached);
}

#[test]
fn start_detached__held_with_its_computer_s_cap() {
    holds("turns_follow", "startDetached", "held with its computer's cap", d::start_detached);
}

#[test]
fn start_detached__steered() {
    holds("turns_follow", "startDetached", "steered", d::start_detached);
}

#[test]
fn start_detached__the_session_names_no_thread() {
    holds("turns_follow", "startDetached", "the session names no thread", d::start_detached);
}

#[test]
fn start_detached__a_reply_that_is_no_start_answer() {
    holds("turns_follow", "startDetached", "a reply that is no start answer", d::start_detached);
}

#[test]
fn start_detached__an_outcome_the_runtime_does_not_know() {
    holds("turns_follow", "startDetached", "an outcome the runtime does not know", d::start_detached);
}

#[test]
fn start_detached__the_host_refuses() {
    holds("turns_follow", "startDetached", "the host refuses", d::start_detached);
}

#[test]
fn start_detached__the_host_refuses_with_no_kind() {
    holds("turns_follow", "startDetached", "the host refuses with no kind", d::start_detached);
}

#[test]
fn start_detached__the_subscription_is_refused() {
    holds("turns_follow", "startDetached", "the subscription is refused", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_no_way_back() {
    holds("turns_follow", "startDetached", "the host stops under the start, no way back", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_dialled_again() {
    holds("turns_follow", "startDetached", "the host stops under the start, dialled again", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_dialled_again_and_the_wait_is() {
    holds("turns_follow", "startDetached", "the host stops under the start, dialled again, and the wait is heard", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_the_dial_is_refused() {
    holds("turns_follow", "startDetached", "the host stops under the start, the dial is refused", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_twice_dialled_twice() {
    holds("turns_follow", "startDetached", "the host stops under the start twice, dialled twice", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_start_then_no_second_dial_is_left() {
    holds("turns_follow", "startDetached", "the host stops under the start, then no second dial is left", d::start_detached);
}

#[test]
fn start_detached__the_host_closes_with_another_code_under_the_start() {
    holds("turns_follow", "startDetached", "the host closes with another code under the start", d::start_detached);
}

#[test]
fn start_detached__the_host_closes_with_no_way_back_and_another_code() {
    holds("turns_follow", "startDetached", "the host closes with no way back and another code", d::start_detached);
}

#[test]
fn start_detached__the_host_stops_under_the_subscription() {
    holds("turns_follow", "startDetached", "the host stops under the subscription", d::start_detached);
}

#[test]
fn start_detached__the_host_refuses_then_is_not_asked_again() {
    holds("turns_follow", "startDetached", "the host refuses then is not asked again", d::start_detached);
}

#[test]
fn follow__a_turn_that_completes() {
    holds("turns_follow", "follow", "a turn that completes", d::follow);
}

#[test]
fn follow__the_follow_ends_at_the_done_the_end_after_it_is_not_handed_o() {
    holds("turns_follow", "follow", "the follow ends at the done: the end after it is not handed on", d::follow);
}

#[test]
fn follow__the_done_carries_a_whole_result() {
    holds("turns_follow", "follow", "the done carries a whole result", d::follow);
}

#[test]
fn follow__the_start_s_followed_mark_goes_last_among_the_start_s_own_fi() {
    holds("turns_follow", "follow", "the start's followed mark goes last among the start's own fields", d::follow);
}

#[test]
fn follow__a_start_after_a_cut_turn() {
    holds("turns_follow", "follow", "a start after a cut turn", d::follow);
}

#[test]
fn follow__a_start_that_is_not_after_a_cut() {
    holds("turns_follow", "follow", "a start that is not after a cut", d::follow);
}

#[test]
fn follow__a_start_of_another_turn_does_not_mark_this_one() {
    holds("turns_follow", "follow", "a start of another turn does not mark this one", d::follow);
}

#[test]
fn follow__an_end_with_the_runtime_s_reason_and_no_done() {
    holds("turns_follow", "follow", "an end with the runtime's reason and no done", d::follow);
}

#[test]
fn follow__an_end_with_no_reason_and_no_done() {
    holds("turns_follow", "follow", "an end with no reason and no done", d::follow);
}

#[test]
fn follow__a_done_after_an_end_of_the_same_turn_is_not_read() {
    holds("turns_follow", "follow", "a done after an end of the same turn is not read", d::follow);
}

#[test]
fn follow__another_turn_s_done_does_not_end_this_one() {
    holds("turns_follow", "follow", "another turn's done does not end this one", d::follow);
}

#[test]
fn follow__another_turn_s_end_does_not_end_this_one() {
    holds("turns_follow", "follow", "another turn's end does not end this one", d::follow);
}

#[test]
fn follow__a_frame_that_is_no_session_event_is_not_handed_on() {
    holds("turns_follow", "follow", "a frame that is no session event is not handed on", d::follow);
}

#[test]
fn follow__a_session_frame_with_no_type_string_is_not_handed_on() {
    holds("turns_follow", "follow", "a session frame with no type string is not handed on", d::follow);
}

#[test]
fn follow__events_live_after_the_start_s_answer_in_order() {
    holds("turns_follow", "follow", "events live after the start's answer, in order", d::follow);
}

#[test]
fn follow__queued_the_wait_and_the_turn() {
    holds("turns_follow", "follow", "queued, the wait and the turn", d::follow);
}

#[test]
fn follow__queued_for_a_computer() {
    holds("turns_follow", "follow", "queued for a computer", d::follow);
}

#[test]
fn follow__held() {
    holds("turns_follow", "follow", "held", d::follow);
}

#[test]
fn follow__the_message_joined_a_running_turn() {
    holds("turns_follow", "follow", "the message joined a running turn", d::follow);
}

#[test]
fn follow__the_steer_says_the_turn_waits_on_a_permission() {
    holds("turns_follow", "follow", "the steer says the turn waits on a permission", d::follow);
}

#[test]
fn follow__a_steer_with_another_request_id_is_not_this_start_s() {
    holds("turns_follow", "follow", "a steer with another request id is not this start's", d::follow);
}

#[test]
fn follow__the_agent_is_starting() {
    holds("turns_follow", "follow", "the agent is starting", d::follow);
}

#[test]
fn follow__the_agent_installs_on_its_first_run() {
    holds("turns_follow", "follow", "the agent installs on its first run", d::follow);
}

#[test]
fn follow__a_starting_after_the_session_started_is_not_heard() {
    holds("turns_follow", "follow", "a starting after the session started is not heard", d::follow);
}

#[test]
fn follow__a_starting_after_the_message_steered_is_not_heard() {
    holds("turns_follow", "follow", "a starting after the message steered is not heard", d::follow);
}

#[test]
fn follow__a_starting_with_another_request_id_is_not_heard() {
    holds("turns_follow", "follow", "a starting with another request id is not heard", d::follow);
}

#[test]
fn follow__a_starting_after_the_turn_s_own_end_is_not_heard() {
    holds("turns_follow", "follow", "a starting after the turn's own end is not heard", d::follow);
}

#[test]
fn follow__a_notice_rides_the_start() {
    holds("turns_follow", "follow", "a notice rides the start", d::follow);
}

#[test]
fn follow__the_session_names_no_thread() {
    holds("turns_follow", "follow", "the session names no thread", d::follow);
}

#[test]
fn follow__a_reply_that_is_no_start_answer() {
    holds("turns_follow", "follow", "a reply that is no start answer", d::follow);
}

#[test]
fn follow__the_host_refuses_the_start() {
    holds("turns_follow", "follow", "the host refuses the start", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_start_no_way_back() {
    holds("turns_follow", "follow", "the host stops under the start, no way back", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_start_dialled_again_the_transcript_() {
    holds("turns_follow", "follow", "the host stops under the start, dialled again: the transcript is read there although the start just landed", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_start_dialled_again_a_done_the_tran() {
    holds("turns_follow", "follow", "the host stops under the start, dialled again: a done the transcript holds is taken", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_start_dialled_again_and_the_transcr() {
    holds("turns_follow", "follow", "the host stops under the start, dialled again, and the transcript read is refused", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_start_the_dial_is_refused() {
    holds("turns_follow", "follow", "the host stops under the start, the dial is refused", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_host_s_transcript_holds_it() {
    holds("turns_follow", "follow", "the host stops under the turn, the host's transcript holds its done", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_transcript_holds_its_end() {
    holds("turns_follow", "follow", "the host stops under the turn, the transcript holds its end", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_transcript_holds_only_anot() {
    holds("turns_follow", "follow", "the host stops under the turn, the transcript holds only another turn's done", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_new_host_pushes_the_done_w() {
    holds("turns_follow", "follow", "the host stops under the turn, the new host pushes the done while the transcript is read", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_new_host_s_transcript_has_() {
    holds("turns_follow", "follow", "the host stops under the turn, the new host's transcript has nothing and the turn goes on until its done", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_no_way_back() {
    holds("turns_follow", "follow", "the host stops under the turn, no way back", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_the_dial_is_refused() {
    holds("turns_follow", "follow", "the host stops under the turn, the dial is refused", d::follow);
}

#[test]
fn follow__the_host_closes_under_the_turn_with_another_code() {
    holds("turns_follow", "follow", "the host closes under the turn with another code", d::follow);
}

#[test]
fn follow__the_host_closes_under_the_turn_with_no_code() {
    holds("turns_follow", "follow", "the host closes under the turn with no code", d::follow);
}

#[test]
fn follow__the_host_stops_under_the_turn_twice() {
    holds("turns_follow", "follow", "the host stops under the turn twice", d::follow);
}

#[test]
fn follow__the_history_read_fails_on_the_new_host() {
    holds("turns_follow", "follow", "the history read fails on the new host", d::follow);
}

#[test]
fn follow__the_end_carries_the_runtime_s_reason_and_the_host_stopped_un() {
    holds("turns_follow", "follow", "the end carries the runtime's reason and the host stopped under the turn", d::follow);
}

#[test]
fn follow__a_turn_that_never_ends_hangs() {
    holds("turns_follow", "follow", "a turn that never ends hangs", d::follow);
}

#[test]
fn first_ended__a_named_thread_the_listing_no_longer_holds_nothing_in_its_tr() {
    holds("turns_follow", "firstEnded", "a named thread the listing no longer holds, nothing in its transcript", d::first_ended);
}

#[test]
fn first_ended__a_named_thread_the_listing_no_longer_holds_a_done_in_its_tra() {
    holds("turns_follow", "firstEnded", "a named thread the listing no longer holds, a done in its transcript", d::first_ended);
}

#[test]
fn first_ended__a_named_thread_already_over_reads_its_transcript() {
    holds("turns_follow", "firstEnded", "a named thread already over reads its transcript", d::first_ended);
}

#[test]
fn first_ended__a_named_thread_already_over_nothing_in_its_transcript_takes_() {
    holds("turns_follow", "firstEnded", "a named thread already over, nothing in its transcript, takes the listing's status", d::first_ended);
}

#[test]
fn first_ended__a_named_thread_already_over_failed() {
    holds("turns_follow", "firstEnded", "a named thread already over, failed", d::first_ended);
}

#[test]
fn first_ended__two_named_the_first_over_in_the_order_named_wins() {
    holds("turns_follow", "firstEnded", "two named, the first over in the order named wins", d::first_ended);
}

#[test]
fn first_ended__two_named_one_gone_and_one_over_the_gone_one_is_answered_fir() {
    holds("turns_follow", "firstEnded", "two named, one gone and one over: the gone one is answered first", d::first_ended);
}

#[test]
fn first_ended__two_named_one_running_and_one_over() {
    holds("turns_follow", "firstEnded", "two named, one running and one over", d::first_ended);
}

#[test]
fn first_ended__a_done_pushed_for_a_named_thread() {
    holds("turns_follow", "firstEnded", "a done pushed for a named thread", d::first_ended);
}

#[test]
fn first_ended__a_done_pushed_after_the_listing_is_read() {
    holds("turns_follow", "firstEnded", "a done pushed after the listing is read", d::first_ended);
}

#[test]
fn first_ended__an_end_pushed_the_transcript_holds_the_reply() {
    holds("turns_follow", "firstEnded", "an end pushed, the transcript holds the reply", d::first_ended);
}

#[test]
fn first_ended__an_end_pushed_with_a_reason_the_transcript_holds_nothing() {
    holds("turns_follow", "firstEnded", "an end pushed with a reason, the transcript holds nothing", d::first_ended);
}

#[test]
fn first_ended__an_end_pushed_with_no_reason_the_transcript_holds_nothing() {
    holds("turns_follow", "firstEnded", "an end pushed with no reason, the transcript holds nothing", d::first_ended);
}

#[test]
fn first_ended__an_end_pushed_reads_the_workspace_the_end_names() {
    holds("turns_follow", "firstEnded", "an end pushed reads the workspace the end names", d::first_ended);
}

#[test]
fn first_ended__a_done_for_a_thread_not_named_is_not_an_answer() {
    holds("turns_follow", "firstEnded", "a done for a thread not named is not an answer", d::first_ended);
}

#[test]
fn first_ended__a_done_with_no_thread_is_not_an_answer() {
    holds("turns_follow", "firstEnded", "a done with no thread is not an answer", d::first_ended);
}

#[test]
fn first_ended__a_frame_that_is_not_a_session_frame_is_not_an_answer() {
    holds("turns_follow", "firstEnded", "a frame that is not a session frame is not an answer", d::first_ended);
}

#[test]
fn first_ended__the_deadline_passes_first() {
    holds("turns_follow", "firstEnded", "the deadline passes first", d::first_ended);
}

#[test]
fn first_ended__a_deadline_of_zero() {
    holds("turns_follow", "firstEnded", "a deadline of zero", d::first_ended);
}

#[test]
fn first_ended__an_answer_before_the_deadline() {
    holds("turns_follow", "firstEnded", "an answer before the deadline", d::first_ended);
}

#[test]
fn first_ended__no_deadline_and_nothing_to_hear() {
    holds("turns_follow", "firstEnded", "no deadline and nothing to hear", d::first_ended);
}

#[test]
fn first_ended__the_socket_stops_while_waiting() {
    holds("turns_follow", "firstEnded", "the socket stops while waiting", d::first_ended);
}

#[test]
fn first_ended__the_socket_closes_while_waiting() {
    holds("turns_follow", "firstEnded", "the socket closes while waiting", d::first_ended);
}

#[test]
fn first_ended__the_socket_stops_during_the_listing() {
    holds("turns_follow", "firstEnded", "the socket stops during the listing", d::first_ended);
}

#[test]
fn first_ended__the_listing_is_refused() {
    holds("turns_follow", "firstEnded", "the listing is refused", d::first_ended);
}

#[test]
fn first_ended__the_subscription_is_refused() {
    holds("turns_follow", "firstEnded", "the subscription is refused", d::first_ended);
}

#[test]
fn first_ended__the_transcript_read_is_refused_for_a_gone_thread() {
    holds("turns_follow", "firstEnded", "the transcript read is refused for a gone thread", d::first_ended);
}

#[test]
fn first_ended__a_named_thread_with_only_an_id() {
    holds("turns_follow", "firstEnded", "a named thread with only an id", d::first_ended);
}

#[test]
fn first_ended__no_thread_named_the_listing_empty() {
    holds("turns_follow", "firstEnded", "no thread named, the listing empty", d::first_ended);
}

#[test]
fn wait_through__one_already_over() {
    holds("turns_follow", "waitThrough", "one already over", d::wait_through);
}

#[test]
fn wait_through__a_done_pushed() {
    holds("turns_follow", "waitThrough", "a done pushed", d::wait_through);
}

#[test]
fn wait_through__the_deadline_passes() {
    holds("turns_follow", "waitThrough", "the deadline passes", d::wait_through);
}

#[test]
fn wait_through__a_sub_thread_s_end_carries_what_its_machine_was_read_to_hold() {
    holds("turns_follow", "waitThrough", "a sub-thread's end carries what its machine was read to hold", d::wait_through);
}

#[test]
fn wait_through__a_sub_thread_s_end_its_machine_has_no_result_yet() {
    holds("turns_follow", "waitThrough", "a sub-thread's end, its machine has no result yet", d::wait_through);
}

#[test]
fn wait_through__a_sub_thread_that_is_not_named_a_sub_thread_asks_nothing_mor() {
    holds("turns_follow", "waitThrough", "a sub-thread that is not named a sub-thread asks nothing more", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_under_the_wait_and_the_one_that_comes_back_an() {
    holds("turns_follow", "waitThrough", "the host stops under the wait and the one that comes back answers", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_under_the_wait_twice() {
    holds("turns_follow", "waitThrough", "the host stops under the wait twice", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_after_listing_the_wait_goes_on_at_the_next_ho() {
    holds("turns_follow", "waitThrough", "the host stops after listing, the wait goes on at the next host", d::wait_through);
}

#[test]
fn wait_through__the_host_closes_under_the_wait_with_another_code() {
    holds("turns_follow", "waitThrough", "the host closes under the wait with another code", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_and_the_dial_fails_once_then_answers() {
    holds("turns_follow", "waitThrough", "the host stops and the dial fails once, then answers", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_and_never_comes_back_no_deadline() {
    holds("turns_follow", "waitThrough", "the host stops and never comes back, no deadline", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_and_never_comes_back_before_the_deadline() {
    holds("turns_follow", "waitThrough", "the host stops and never comes back before the deadline", d::wait_through);
}

#[test]
fn wait_through__the_host_stops_and_the_next_host_runs_out_the_deadline() {
    holds("turns_follow", "waitThrough", "the host stops and the next host runs out the deadline", d::wait_through);
}

#[test]
fn wait_through__the_list_is_refused() {
    holds("turns_follow", "waitThrough", "the list is refused", d::wait_through);
}

#[test]
fn wait_through__no_thread_named() {
    holds("turns_follow", "waitThrough", "no thread named", d::wait_through);
}

#[test]
fn restart_host__the_host_restarts_and_nothing_runs() {
    holds("turns_follow", "restartHost", "the host restarts and nothing runs", d::restart_host);
}

#[test]
fn restart_host__the_host_restarts_and_one_thread_runs() {
    holds("turns_follow", "restartHost", "the host restarts and one thread runs", d::restart_host);
}

#[test]
fn restart_host__the_host_restarts_and_two_threads_run_in_the_listing_s_order() {
    holds("turns_follow", "restartHost", "the host restarts and two threads run, in the listing's order", d::restart_host);
}

#[test]
fn restart_host__the_host_closes_with_another_code_on_its_way_down() {
    holds("turns_follow", "restartHost", "the host closes with another code on its way down", d::restart_host);
}

#[test]
fn restart_host__the_host_closes_before_it_answers() {
    holds("turns_follow", "restartHost", "the host closes before it answers", d::restart_host);
}

#[test]
fn restart_host__the_restart_is_refused_in_the_road_s_own_words() {
    holds("turns_follow", "restartHost", "the restart is refused in the road's own words", d::restart_host);
}

#[test]
fn restart_host__the_host_comes_back_after_a_refused_dial() {
    holds("turns_follow", "restartHost", "the host comes back after a refused dial", d::restart_host);
}

#[test]
fn restart_host__the_host_never_comes_back() {
    holds("turns_follow", "restartHost", "the host never comes back", d::restart_host);
}

#[test]
fn restart_host__the_new_host_s_listing_is_refused() {
    holds("turns_follow", "restartHost", "the new host's listing is refused", d::restart_host);
}

#[test]
fn restart_host__the_host_took_the_restart_and_kept_serving() {
    holds("turns_follow", "restartHost", "the host took the restart and kept serving", d::restart_host);
}

#[test]
fn read_thread__the_messages() {
    holds("turns_follow", "readThread", "the messages", d::read_thread);
}

#[test]
fn read_thread__the_final_reply() {
    holds("turns_follow", "readThread", "the final reply", d::read_thread);
}

#[test]
fn read_thread__the_final_reply_of_a_thread_with_a_newer_turn() {
    holds("turns_follow", "readThread", "the final reply of a thread with a newer turn", d::read_thread);
}

#[test]
fn read_thread__the_final_reply_the_runtime_cut() {
    holds("turns_follow", "readThread", "the final reply the runtime cut", d::read_thread);
}

#[test]
fn read_thread__no_words_in_the_reply() {
    holds("turns_follow", "readThread", "no words in the reply", d::read_thread);
}

#[test]
fn read_thread__an_empty_transcript() {
    holds("turns_follow", "readThread", "an empty transcript", d::read_thread);
}

#[test]
fn read_thread__an_empty_transcript_the_last_reply() {
    holds("turns_follow", "readThread", "an empty transcript, the last reply", d::read_thread);
}

#[test]
fn read_thread__the_transcript_is_read_before_the_stamp_moves() {
    holds("turns_follow", "readThread", "the transcript is read before the stamp moves", d::read_thread);
}

#[test]
fn read_thread__the_transcript_read_is_refused() {
    holds("turns_follow", "readThread", "the transcript read is refused", d::read_thread);
}

#[test]
fn read_thread__a_thread_with_only_an_id() {
    holds("turns_follow", "readThread", "a thread with only an id", d::read_thread);
}

#[test]
fn read_line__messages() {
    holds("turns_follow", "readLine", "messages", d::read_line);
}

#[test]
fn read_line__the_final_reply() {
    holds("turns_follow", "readLine", "the final reply", d::read_line);
}

#[test]
fn read_line__no_messages() {
    holds("turns_follow", "readLine", "no messages", d::read_line);
}

#[test]
fn read_line__no_reply() {
    holds("turns_follow", "readLine", "no reply", d::read_line);
}

#[test]
fn read_line__messages_but_no_reply() {
    holds("turns_follow", "readLine", "messages but no reply", d::read_line);
}

#[test]
fn thread_head__a_head() {
    holds("turns_follow", "threadHead", "a head", d::thread_head);
}

#[test]
fn thread_head__a_head_keeps_only_its_four_parts() {
    holds("turns_follow", "threadHead", "a head keeps only its four parts", d::thread_head);
}

#[test]
fn thread_head__a_head_with_no_events() {
    holds("turns_follow", "threadHead", "a head with no events", d::thread_head);
}

#[test]
fn thread_head__the_host_refuses() {
    holds("turns_follow", "threadHead", "the host refuses", d::thread_head);
}

#[test]
fn head_line__head() {
    holds("turns_follow", "headLine", "head", d::head_line);
}

#[test]
fn head_line__a_sub_thread_on_boat() {
    holds("turns_follow", "headLine", "a sub-thread on Boat", d::head_line);
}

#[test]
fn head_line__no_model_access_or_folder_and_no_events() {
    holds("turns_follow", "headLine", "no model, access or folder, and no events", d::head_line);
}

#[test]
fn host_did_not_stop_line__0_ms() {
    holds("turns_follow", "hostDidNotStopLine", "0 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__1_ms() {
    holds("turns_follow", "hostDidNotStopLine", "1 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__999_ms() {
    holds("turns_follow", "hostDidNotStopLine", "999 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__1000_ms() {
    holds("turns_follow", "hostDidNotStopLine", "1000 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__20000_ms() {
    holds("turns_follow", "hostDidNotStopLine", "20000 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__61000_ms() {
    holds("turns_follow", "hostDidNotStopLine", "61000 ms", d::host_did_not_stop_line);
}

#[test]
fn host_did_not_stop_line__3600000_ms() {
    holds("turns_follow", "hostDidNotStopLine", "3600000 ms", d::host_did_not_stop_line);
}

#[test]
fn host_restarted_line__0_running() {
    holds("turns_follow", "hostRestartedLine", "0 running ", d::host_restarted_line);
}

#[test]
fn host_restarted_line__1_running_aaaaaaaa_1111() {
    holds("turns_follow", "hostRestartedLine", "1 running aaaaaaaa-1111", d::host_restarted_line);
}

#[test]
fn host_restarted_line__1_running_short() {
    holds("turns_follow", "hostRestartedLine", "1 running short", d::host_restarted_line);
}

#[test]
fn host_restarted_line__2_running_abcdefgh12345678_12345678abcdefgh() {
    holds("turns_follow", "hostRestartedLine", "2 running abcdefgh12345678,12345678abcdefgh", d::host_restarted_line);
}

#[test]
fn host_restarted_line__3_running_a_b_c() {
    holds("turns_follow", "hostRestartedLine", "3 running a,b,c", d::host_restarted_line);
}
