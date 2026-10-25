// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_stream.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_stream as d;

#[test]
fn turn_failure__completed() {
    holds("turns_stream", "turnFailure", "completed", d::turn_failure);
}

#[test]
fn turn_refusal__completed() {
    holds("turns_stream", "turnRefusal", "completed", d::turn_refusal);
}

#[test]
fn turn_failure__completed_with_an_error_text() {
    holds("turns_stream", "turnFailure", "completed with an error text", d::turn_failure);
}

#[test]
fn turn_refusal__completed_with_an_error_text() {
    holds("turns_stream", "turnRefusal", "completed with an error text", d::turn_refusal);
}

#[test]
fn turn_failure__failed_with_the_agent_s_error() {
    holds("turns_stream", "turnFailure", "failed with the agent's error", d::turn_failure);
}

#[test]
fn turn_refusal__failed_with_the_agent_s_error() {
    holds("turns_stream", "turnRefusal", "failed with the agent's error", d::turn_refusal);
}

#[test]
fn turn_failure__failed_with_an_empty_error() {
    holds("turns_stream", "turnFailure", "failed with an empty error", d::turn_failure);
}

#[test]
fn turn_refusal__failed_with_an_empty_error() {
    holds("turns_stream", "turnRefusal", "failed with an empty error", d::turn_refusal);
}

#[test]
fn turn_failure__failed_with_no_error_the_runtime_s_reason() {
    holds("turns_stream", "turnFailure", "failed with no error, the runtime's reason", d::turn_failure);
}

#[test]
fn turn_refusal__failed_with_no_error_the_runtime_s_reason() {
    holds("turns_stream", "turnRefusal", "failed with no error, the runtime's reason", d::turn_refusal);
}

#[test]
fn turn_failure__failed_with_no_error_and_no_reason() {
    holds("turns_stream", "turnFailure", "failed with no error and no reason", d::turn_failure);
}

#[test]
fn turn_refusal__failed_with_no_error_and_no_reason() {
    holds("turns_stream", "turnRefusal", "failed with no error and no reason", d::turn_refusal);
}

#[test]
fn turn_failure__interrupted() {
    holds("turns_stream", "turnFailure", "interrupted", d::turn_failure);
}

#[test]
fn turn_refusal__interrupted() {
    holds("turns_stream", "turnRefusal", "interrupted", d::turn_refusal);
}

#[test]
fn turn_failure__interrupted_with_a_reason() {
    holds("turns_stream", "turnFailure", "interrupted with a reason", d::turn_failure);
}

#[test]
fn turn_refusal__interrupted_with_a_reason() {
    holds("turns_stream", "turnRefusal", "interrupted with a reason", d::turn_refusal);
}

#[test]
fn turn_failure__no_result_a_reason() {
    holds("turns_stream", "turnFailure", "no result, a reason", d::turn_failure);
}

#[test]
fn turn_refusal__no_result_a_reason() {
    holds("turns_stream", "turnRefusal", "no result, a reason", d::turn_refusal);
}

#[test]
fn turn_failure__no_result_no_reason() {
    holds("turns_stream", "turnFailure", "no result, no reason", d::turn_failure);
}

#[test]
fn turn_refusal__no_result_no_reason() {
    holds("turns_stream", "turnRefusal", "no result, no reason", d::turn_refusal);
}

#[test]
fn turn_failure__no_result_an_empty_reason() {
    holds("turns_stream", "turnFailure", "no result, an empty reason", d::turn_failure);
}

#[test]
fn turn_refusal__no_result_an_empty_reason() {
    holds("turns_stream", "turnRefusal", "no result, an empty reason", d::turn_refusal);
}

#[test]
fn turn_failure__refused_for_a_sign_in() {
    holds("turns_stream", "turnFailure", "refused for a sign-in", d::turn_failure);
}

#[test]
fn turn_refusal__refused_for_a_sign_in() {
    holds("turns_stream", "turnRefusal", "refused for a sign-in", d::turn_refusal);
}

#[test]
fn turn_failure__refused_for_a_sign_in_with_no_words() {
    holds("turns_stream", "turnFailure", "refused for a sign-in with no words", d::turn_failure);
}

#[test]
fn turn_refusal__refused_for_a_sign_in_with_no_words() {
    holds("turns_stream", "turnRefusal", "refused for a sign-in with no words", d::turn_refusal);
}

#[test]
fn turn_failure__refused_for_a_sign_in_interrupted() {
    holds("turns_stream", "turnFailure", "refused for a sign-in, interrupted", d::turn_failure);
}

#[test]
fn turn_refusal__refused_for_a_sign_in_interrupted() {
    holds("turns_stream", "turnRefusal", "refused for a sign-in, interrupted", d::turn_refusal);
}

#[test]
fn answer_keys_line__allow_deny_and_a_mode() {
    holds("turns_stream", "answerKeysLine", "allow, deny and a mode", d::answer_keys_line);
}

#[test]
fn answer_keys_line__the_roads_order_not_the_options() {
    holds("turns_stream", "answerKeysLine", "the roads' order, not the options'", d::answer_keys_line);
}

#[test]
fn answer_keys_line__allow_only() {
    holds("turns_stream", "answerKeysLine", "allow only", d::answer_keys_line);
}

#[test]
fn answer_keys_line__deny_only() {
    holds("turns_stream", "answerKeysLine", "deny only", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_only() {
    holds("turns_stream", "answerKeysLine", "a mode only", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_with_a_label_that_starts_in_a_capital_outside_ascii() {
    holds("turns_stream", "answerKeysLine", "a mode with a label that starts in a capital outside ASCII", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_with_an_empty_label() {
    holds("turns_stream", "answerKeysLine", "a mode with an empty label", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_with_a_label_already_lower() {
    holds("turns_stream", "answerKeysLine", "a mode with a label already lower", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_whose_label_starts_with_a_capital_outside_the_first_p() {
    holds("turns_stream", "answerKeysLine", "a mode whose label starts with a capital outside the first plane: JS lowers one UTF-16 unit", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_whose_label_starts_with_a_capital_that_lowers_to_two_() {
    holds("turns_stream", "answerKeysLine", "a mode whose label starts with a capital that lowers to two characters", d::answer_keys_line);
}

#[test]
fn answer_keys_line__a_mode_whose_label_starts_with_a_digit() {
    holds("turns_stream", "answerKeysLine", "a mode whose label starts with a digit", d::answer_keys_line);
}

#[test]
fn answer_keys_line__two_allows_the_first_stands() {
    holds("turns_stream", "answerKeysLine", "two allows, the first stands", d::answer_keys_line);
}

#[test]
fn answer_keys_line__no_road_at_all_the_prompt_s_own_answers() {
    holds("turns_stream", "answerKeysLine", "no road at all: the prompt's own answers", d::answer_keys_line);
}

#[test]
fn answer_keys_line__no_option() {
    holds("turns_stream", "answerKeysLine", "no option", d::answer_keys_line);
}

#[test]
fn answer_verbs_line__thread_thread_9d8c7b6a5f4e() {
    holds("turns_stream", "answerVerbsLine", "thread thread-9d8c7b6a5f4e", d::answer_verbs_line);
}

#[test]
fn answer_verbs_line__thread_abc() {
    holds("turns_stream", "answerVerbsLine", "thread abc", d::answer_verbs_line);
}

#[test]
fn answer_verbs_line__thread_empty() {
    holds("turns_stream", "answerVerbsLine", "thread (empty)", d::answer_verbs_line);
}

#[test]
fn answer_verbs_line__thread_t_1111aaaa() {
    holds("turns_stream", "answerVerbsLine", "thread t-1111aaaa", d::answer_verbs_line);
}

#[test]
fn answer_verbs_line__thread_thread_b0b0b0b0c1c1() {
    holds("turns_stream", "answerVerbsLine", "thread thread-b0b0b0b0c1c1", d::answer_verbs_line);
}

#[test]
fn answered_line__a_command() {
    holds("turns_stream", "answeredLine", "a command", d::answered_line);
}

#[test]
fn answered_line__a_command_with_no_detail() {
    holds("turns_stream", "answeredLine", "a command with no detail", d::answered_line);
}

#[test]
fn answered_line__an_edit() {
    holds("turns_stream", "answeredLine", "an edit", d::answered_line);
}

#[test]
fn answered_line__a_tool_the_words_do_not_know() {
    holds("turns_stream", "answeredLine", "a tool the words do not know", d::answered_line);
}

#[test]
fn answered_line__an_input_that_is_no_json() {
    holds("turns_stream", "answeredLine", "an input that is no JSON", d::answered_line);
}

#[test]
fn no_open_ask_line__thread_9d8c7b6a5f4e() {
    holds("turns_stream", "noOpenAskLine", "thread-9d8c7b6a5f4e", d::no_open_ask_line);
}

#[test]
fn no_such_answer_line__thread_9d8c7b6a5f4e_allow() {
    holds("turns_stream", "noSuchAnswerLine", "thread-9d8c7b6a5f4e allow", d::no_such_answer_line);
}

#[test]
fn no_such_answer_line__thread_9d8c7b6a5f4e_deny() {
    holds("turns_stream", "noSuchAnswerLine", "thread-9d8c7b6a5f4e deny", d::no_such_answer_line);
}

#[test]
fn no_open_ask_line__short() {
    holds("turns_stream", "noOpenAskLine", "short", d::no_open_ask_line);
}

#[test]
fn no_such_answer_line__short_allow() {
    holds("turns_stream", "noSuchAnswerLine", "short allow", d::no_such_answer_line);
}

#[test]
fn no_such_answer_line__short_deny() {
    holds("turns_stream", "noSuchAnswerLine", "short deny", d::no_such_answer_line);
}

#[test]
fn answer_roads__the_table() {
    holds("turns_stream", "answerRoads", "the table", d::answer_roads);
}

#[test]
fn answer_in_the_app__the_words() {
    holds("turns_stream", "answerInTheApp", "the words", d::answer_in_the_app);
}

#[test]
fn answer_open_ask__allow() {
    holds("turns_stream", "answerOpenAsk", "allow", d::answer_open_ask);
}

#[test]
fn answer_open_ask__deny_with_the_person_s_words() {
    holds("turns_stream", "answerOpenAsk", "deny with the person's words", d::answer_open_ask);
}

#[test]
fn answer_open_ask__deny_with_no_words() {
    holds("turns_stream", "answerOpenAsk", "deny with no words", d::answer_open_ask);
}

#[test]
fn answer_open_ask__deny_with_an_empty_reason() {
    holds("turns_stream", "answerOpenAsk", "deny with an empty reason", d::answer_open_ask);
}

#[test]
fn answer_open_ask__allow_by_a_prefix_of_the_thread() {
    holds("turns_stream", "answerOpenAsk", "allow by a prefix of the thread", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_first_allow_option_is_the_one_picked() {
    holds("turns_stream", "answerOpenAsk", "the first allow option is the one picked", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_thread_waits_on_no_prompt() {
    holds("turns_stream", "answerOpenAsk", "the thread waits on no prompt", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_prompt_was_closed() {
    holds("turns_stream", "answerOpenAsk", "the prompt was closed", d::answer_open_ask);
}

#[test]
fn answer_open_ask__a_prompt_of_another_thread_is_not_this_one_s() {
    holds("turns_stream", "answerOpenAsk", "a prompt of another thread is not this one's", d::answer_open_ask);
}

#[test]
fn answer_open_ask__two_prompts_open_one_closed() {
    holds("turns_stream", "answerOpenAsk", "two prompts open, one closed", d::answer_open_ask);
}

#[test]
fn answer_open_ask__two_prompts_open() {
    holds("turns_stream", "answerOpenAsk", "two prompts open", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_prompt_takes_no_deny() {
    holds("turns_stream", "answerOpenAsk", "the prompt takes no deny", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_prompt_carries_only_its_own_answers() {
    holds("turns_stream", "answerOpenAsk", "the prompt carries only its own answers", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_answer_lands_too_late() {
    holds("turns_stream", "answerOpenAsk", "the answer lands too late", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_agent_raises_no_prompt_the_host_can_answer() {
    holds("turns_stream", "answerOpenAsk", "the agent raises no prompt the host can answer", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_host_holds_no_turn_of_that_thread() {
    holds("turns_stream", "answerOpenAsk", "the host holds no turn of that thread", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_prompt_has_no_option_by_that_id() {
    holds("turns_stream", "answerOpenAsk", "the prompt has no option by that id", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_host_refuses_the_answer() {
    holds("turns_stream", "answerOpenAsk", "the host refuses the answer", d::answer_open_ask);
}

#[test]
fn answer_open_ask__no_such_thread() {
    holds("turns_stream", "answerOpenAsk", "no such thread", d::answer_open_ask);
}

#[test]
fn answer_open_ask__the_transcript_is_refused() {
    holds("turns_stream", "answerOpenAsk", "the transcript is refused", d::answer_open_ask);
}

#[test]
fn follow_verb__prose_then_the_done() {
    holds("turns_stream", "followVerb", "prose, then the done", d::follow_verb);
}

#[test]
fn follow_verb__prose_then_the_done_json() {
    holds("turns_stream", "followVerb", "prose, then the done (--json)", d::follow_verb);
}

#[test]
fn follow_verb__prose_with_no_trailing_newline() {
    holds("turns_stream", "followVerb", "prose with no trailing newline", d::follow_verb);
}

#[test]
fn follow_verb__prose_with_no_trailing_newline_json() {
    holds("turns_stream", "followVerb", "prose with no trailing newline (--json)", d::follow_verb);
}

#[test]
fn follow_verb__prose_on_the_person_s_own_screen() {
    holds("turns_stream", "followVerb", "prose on the person's own screen", d::follow_verb);
}

#[test]
fn follow_verb__prose_on_the_person_s_own_screen_json() {
    holds("turns_stream", "followVerb", "prose on the person's own screen (--json)", d::follow_verb);
}

#[test]
fn follow_verb__no_prose_streamed_on_the_person_s_own_screen() {
    holds("turns_stream", "followVerb", "no prose streamed, on the person's own screen", d::follow_verb);
}

#[test]
fn follow_verb__no_prose_streamed_on_the_person_s_own_screen_json() {
    holds("turns_stream", "followVerb", "no prose streamed, on the person's own screen (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_done_with_no_text() {
    holds("turns_stream", "followVerb", "a done with no text", d::follow_verb);
}

#[test]
fn follow_verb__a_done_with_no_text_json() {
    holds("turns_stream", "followVerb", "a done with no text (--json)", d::follow_verb);
}

#[test]
fn follow_verb__muted_asides() {
    holds("turns_stream", "followVerb", "muted asides", d::follow_verb);
}

#[test]
fn follow_verb__muted_asides_json() {
    holds("turns_stream", "followVerb", "muted asides (--json)", d::follow_verb);
}

#[test]
fn follow_verb__asides_with_no_ink() {
    holds("turns_stream", "followVerb", "asides with no ink", d::follow_verb);
}

#[test]
fn follow_verb__asides_with_no_ink_json() {
    holds("turns_stream", "followVerb", "asides with no ink (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_second_message_opens_its_own_paragraph() {
    holds("turns_stream", "followVerb", "a second message opens its own paragraph", d::follow_verb);
}

#[test]
fn follow_verb__a_second_message_opens_its_own_paragraph_json() {
    holds("turns_stream", "followVerb", "a second message opens its own paragraph (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_second_message_after_a_line_stands_apart_already() {
    holds("turns_stream", "followVerb", "a second message after a line stands apart already", d::follow_verb);
}

#[test]
fn follow_verb__a_second_message_after_a_line_stands_apart_already_json() {
    holds("turns_stream", "followVerb", "a second message after a line stands apart already (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_line_of_work_between_two_messages() {
    holds("turns_stream", "followVerb", "a line of work between two messages", d::follow_verb);
}

#[test]
fn follow_verb__a_line_of_work_between_two_messages_json() {
    holds("turns_stream", "followVerb", "a line of work between two messages (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_piece_with_no_message_id_after_one_with() {
    holds("turns_stream", "followVerb", "a piece with no message id after one with", d::follow_verb);
}

#[test]
fn follow_verb__a_piece_with_no_message_id_after_one_with_json() {
    holds("turns_stream", "followVerb", "a piece with no message id after one with (--json)", d::follow_verb);
}

#[test]
fn follow_verb__an_empty_piece_is_nothing() {
    holds("turns_stream", "followVerb", "an empty piece is nothing", d::follow_verb);
}

#[test]
fn follow_verb__an_empty_piece_is_nothing_json() {
    holds("turns_stream", "followVerb", "an empty piece is nothing (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_tool_call_and_its_result() {
    holds("turns_stream", "followVerb", "a tool call and its result", d::follow_verb);
}

#[test]
fn follow_verb__a_tool_call_and_its_result_json() {
    holds("turns_stream", "followVerb", "a tool call and its result (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_failed_tool_result() {
    holds("turns_stream", "followVerb", "a failed tool result", d::follow_verb);
}

#[test]
fn follow_verb__a_failed_tool_result_json() {
    holds("turns_stream", "followVerb", "a failed tool result (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_result_with_no_call_open() {
    holds("turns_stream", "followVerb", "a result with no call open", d::follow_verb);
}

#[test]
fn follow_verb__a_result_with_no_call_open_json() {
    holds("turns_stream", "followVerb", "a result with no call open (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_call_with_no_id() {
    holds("turns_stream", "followVerb", "a call with no id", d::follow_verb);
}

#[test]
fn follow_verb__a_call_with_no_id_json() {
    holds("turns_stream", "followVerb", "a call with no id (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_result_forgets_its_call() {
    holds("turns_stream", "followVerb", "a result forgets its call", d::follow_verb);
}

#[test]
fn follow_verb__a_result_forgets_its_call_json() {
    holds("turns_stream", "followVerb", "a result forgets its call (--json)", d::follow_verb);
}

#[test]
fn follow_verb__attachments_of_the_person_s_turn() {
    holds("turns_stream", "followVerb", "attachments of the person's turn", d::follow_verb);
}

#[test]
fn follow_verb__attachments_of_the_person_s_turn_json() {
    holds("turns_stream", "followVerb", "attachments of the person's turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__after_a_cut_turn() {
    holds("turns_stream", "followVerb", "after a cut turn", d::follow_verb);
}

#[test]
fn follow_verb__after_a_cut_turn_json() {
    holds("turns_stream", "followVerb", "after a cut turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_cap_wait_and_its_end() {
    holds("turns_stream", "followVerb", "a cap wait and its end", d::follow_verb);
}

#[test]
fn follow_verb__a_cap_wait_and_its_end_json() {
    holds("turns_stream", "followVerb", "a cap wait and its end (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_notify_for_me() {
    holds("turns_stream", "followVerb", "a notify for me", d::follow_verb);
}

#[test]
fn follow_verb__a_notify_for_me_json() {
    holds("turns_stream", "followVerb", "a notify for me (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_notify_for_another_thread() {
    holds("turns_stream", "followVerb", "a notify for another thread", d::follow_verb);
}

#[test]
fn follow_verb__a_notify_for_another_thread_json() {
    holds("turns_stream", "followVerb", "a notify for another thread (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_turn_s_own_spend_word() {
    holds("turns_stream", "followVerb", "the turn's own spend word", d::follow_verb);
}

#[test]
fn follow_verb__the_turn_s_own_spend_word_json() {
    holds("turns_stream", "followVerb", "the turn's own spend word (--json)", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_the_thread_s_id() {
    holds("turns_stream", "followVerb", "announce with the thread's id", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_the_thread_s_id_json() {
    holds("turns_stream", "followVerb", "announce with the thread's id (--json)", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_a_line_of_its_own_and_the_folder() {
    holds("turns_stream", "followVerb", "announce with a line of its own and the folder", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_a_line_of_its_own_and_the_folder_json() {
    holds("turns_stream", "followVerb", "announce with a line of its own and the folder (--json)", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_a_line_of_its_own_and_no_folder() {
    holds("turns_stream", "followVerb", "announce with a line of its own and no folder", d::follow_verb);
}

#[test]
fn follow_verb__announce_with_a_line_of_its_own_and_no_folder_json() {
    holds("turns_stream", "followVerb", "announce with a line of its own and no folder (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_notice_on_the_start() {
    holds("turns_stream", "followVerb", "a notice on the start", d::follow_verb);
}

#[test]
fn follow_verb__a_notice_on_the_start_json() {
    holds("turns_stream", "followVerb", "a notice on the start (--json)", d::follow_verb);
}

#[test]
fn follow_verb__queued_behind_the_running_turn() {
    holds("turns_stream", "followVerb", "queued behind the running turn", d::follow_verb);
}

#[test]
fn follow_verb__queued_behind_the_running_turn_json() {
    holds("turns_stream", "followVerb", "queued behind the running turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__queued_for_a_computer_then_behind_the_turn() {
    holds("turns_stream", "followVerb", "queued for a computer, then behind the turn", d::follow_verb);
}

#[test]
fn follow_verb__queued_for_a_computer_then_behind_the_turn_json() {
    holds("turns_stream", "followVerb", "queued for a computer, then behind the turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__queued_for_a_computer_alone() {
    holds("turns_stream", "followVerb", "queued for a computer alone", d::follow_verb);
}

#[test]
fn follow_verb__queued_for_a_computer_alone_json() {
    holds("turns_stream", "followVerb", "queued for a computer alone (--json)", d::follow_verb);
}

#[test]
fn follow_verb__held() {
    holds("turns_stream", "followVerb", "held", d::follow_verb);
}

#[test]
fn follow_verb__held_json() {
    holds("turns_stream", "followVerb", "held (--json)", d::follow_verb);
}

#[test]
fn follow_verb__joined_the_running_turn() {
    holds("turns_stream", "followVerb", "joined the running turn", d::follow_verb);
}

#[test]
fn follow_verb__joined_the_running_turn_json() {
    holds("turns_stream", "followVerb", "joined the running turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_picks_dropped() {
    holds("turns_stream", "followVerb", "joined with picks dropped", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_picks_dropped_json() {
    holds("turns_stream", "followVerb", "joined with picks dropped (--json)", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_one_pick_dropped() {
    holds("turns_stream", "followVerb", "joined with one pick dropped", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_one_pick_dropped_json() {
    holds("turns_stream", "followVerb", "joined with one pick dropped (--json)", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_fast_false() {
    holds("turns_stream", "followVerb", "joined with fast false", d::follow_verb);
}

#[test]
fn follow_verb__joined_with_fast_false_json() {
    holds("turns_stream", "followVerb", "joined with fast false (--json)", d::follow_verb);
}

#[test]
fn follow_verb__joined_a_turn_waiting_on_a_permission() {
    holds("turns_stream", "followVerb", "joined a turn waiting on a permission", d::follow_verb);
}

#[test]
fn follow_verb__joined_a_turn_waiting_on_a_permission_json() {
    holds("turns_stream", "followVerb", "joined a turn waiting on a permission (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_agent_is_starting() {
    holds("turns_stream", "followVerb", "the agent is starting", d::follow_verb);
}

#[test]
fn follow_verb__the_agent_is_starting_json() {
    holds("turns_stream", "followVerb", "the agent is starting (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_agent_installs_on_first_run() {
    holds("turns_stream", "followVerb", "the agent installs on first run", d::follow_verb);
}

#[test]
fn follow_verb__the_agent_installs_on_first_run_json() {
    holds("turns_stream", "followVerb", "the agent installs on first run (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_failed_turn() {
    holds("turns_stream", "followVerb", "a failed turn", d::follow_verb);
}

#[test]
fn follow_verb__a_failed_turn_json() {
    holds("turns_stream", "followVerb", "a failed turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_turn_the_runtime_ended() {
    holds("turns_stream", "followVerb", "a turn the runtime ended", d::follow_verb);
}

#[test]
fn follow_verb__a_turn_the_runtime_ended_json() {
    holds("turns_stream", "followVerb", "a turn the runtime ended (--json)", d::follow_verb);
}

#[test]
fn follow_verb__refused_for_a_sign_in() {
    holds("turns_stream", "followVerb", "refused for a sign-in", d::follow_verb);
}

#[test]
fn follow_verb__refused_for_a_sign_in_json() {
    holds("turns_stream", "followVerb", "refused for a sign-in (--json)", d::follow_verb);
}

#[test]
fn follow_verb__an_interrupted_turn() {
    holds("turns_stream", "followVerb", "an interrupted turn", d::follow_verb);
}

#[test]
fn follow_verb__an_interrupted_turn_json() {
    holds("turns_stream", "followVerb", "an interrupted turn (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_turn_the_reply_is_printed_whole() {
    holds("turns_stream", "followVerb", "the host stops under the turn: the reply is printed whole", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_turn_the_reply_is_printed_whole_jso() {
    holds("turns_stream", "followVerb", "the host stops under the turn: the reply is printed whole (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_start() {
    holds("turns_stream", "followVerb", "the host stops under the start", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_start_json() {
    holds("turns_stream", "followVerb", "the host stops under the start (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_start_no_way_back() {
    holds("turns_stream", "followVerb", "the host stops under the start, no way back", d::follow_verb);
}

#[test]
fn follow_verb__the_host_stops_under_the_start_no_way_back_json() {
    holds("turns_stream", "followVerb", "the host stops under the start, no way back (--json)", d::follow_verb);
}

#[test]
fn follow_verb__the_host_refuses_the_start() {
    holds("turns_stream", "followVerb", "the host refuses the start", d::follow_verb);
}

#[test]
fn follow_verb__the_host_refuses_the_start_json() {
    holds("turns_stream", "followVerb", "the host refuses the start (--json)", d::follow_verb);
}

#[test]
fn follow_verb__nothing_ends_the_turn() {
    holds("turns_stream", "followVerb", "nothing ends the turn", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_no_keys_the_verbs_answer_it() {
    holds("turns_stream", "followVerb", "a prompt, no keys: the verbs answer it", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_no_keys_the_verbs_answer_it_json() {
    holds("turns_stream", "followVerb", "a prompt, no keys: the verbs answer it (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_the_keys_answer_it_y() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard: the keys answer it, y", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_the_keys_answer_it_y_json() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard: the keys answer it, y (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_n() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, n", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_n_json() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, n (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a_json() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a_key_no_road_has() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a key no road has", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a_key_no_road_has_json() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a key no road has (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a_key_whose_option_the_prompt_lacks() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a key whose option the prompt lacks", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_a_key_whose_option_the_prompt_lacks_j() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard, a key whose option the prompt lacks (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_that_carries_only_its_own_answers() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard that carries only its own answers", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_at_a_keyboard_that_carries_only_its_own_answers_jso() {
    holds("turns_stream", "followVerb", "a prompt at a keyboard that carries only its own answers (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_with_no_options_at_a_keyboard() {
    holds("turns_stream", "followVerb", "a prompt with no options at a keyboard", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_with_no_options_at_a_keyboard_json() {
    holds("turns_stream", "followVerb", "a prompt with no options at a keyboard (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_key_that_the_host_refuses_goes_to_stderr() {
    holds("turns_stream", "followVerb", "a key that the host refuses goes to stderr", d::follow_verb);
}

#[test]
fn follow_verb__a_key_that_the_host_refuses_goes_to_stderr_json() {
    holds("turns_stream", "followVerb", "a key that the host refuses goes to stderr (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_key_that_the_host_fails_on_goes_to_stderr() {
    holds("turns_stream", "followVerb", "a key that the host fails on goes to stderr", d::follow_verb);
}

#[test]
fn follow_verb__a_key_that_the_host_fails_on_goes_to_stderr_json() {
    holds("turns_stream", "followVerb", "a key that the host fails on goes to stderr (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_closed_by_anyone_releases_the_read() {
    holds("turns_stream", "followVerb", "a prompt closed by anyone releases the read", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_closed_by_anyone_releases_the_read_json() {
    holds("turns_stream", "followVerb", "a prompt closed by anyone releases the read (--json)", d::follow_verb);
}

#[test]
fn follow_verb__two_prompts_in_a_row() {
    holds("turns_stream", "followVerb", "two prompts in a row", d::follow_verb);
}

#[test]
fn follow_verb__two_prompts_in_a_row_json() {
    holds("turns_stream", "followVerb", "two prompts in a row (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_for_a_tool_with_no_detail() {
    holds("turns_stream", "followVerb", "a prompt for a tool with no detail", d::follow_verb);
}

#[test]
fn follow_verb__a_prompt_for_a_tool_with_no_detail_json() {
    holds("turns_stream", "followVerb", "a prompt for a tool with no detail (--json)", d::follow_verb);
}

#[test]
fn detach_verb__started() {
    holds("turns_stream", "detachVerb", "started", d::detach_verb);
}

#[test]
fn detach_verb__started_json() {
    holds("turns_stream", "detachVerb", "started (--json)", d::detach_verb);
}

#[test]
fn detach_verb__started_with_an_opened_line_and_a_folder() {
    holds("turns_stream", "detachVerb", "started with an opened line and a folder", d::detach_verb);
}

#[test]
fn detach_verb__started_with_an_opened_line_and_a_folder_json() {
    holds("turns_stream", "detachVerb", "started with an opened line and a folder (--json)", d::detach_verb);
}

#[test]
fn detach_verb__started_with_an_opened_line_and_no_folder() {
    holds("turns_stream", "detachVerb", "started with an opened line and no folder", d::detach_verb);
}

#[test]
fn detach_verb__started_with_an_opened_line_and_no_folder_json() {
    holds("turns_stream", "detachVerb", "started with an opened line and no folder (--json)", d::detach_verb);
}

#[test]
fn detach_verb__a_notice() {
    holds("turns_stream", "detachVerb", "a notice", d::detach_verb);
}

#[test]
fn detach_verb__a_notice_json() {
    holds("turns_stream", "detachVerb", "a notice (--json)", d::detach_verb);
}

#[test]
fn detach_verb__said_beside_the_thread() {
    holds("turns_stream", "detachVerb", "said beside the thread", d::detach_verb);
}

#[test]
fn detach_verb__said_beside_the_thread_json() {
    holds("turns_stream", "detachVerb", "said beside the thread (--json)", d::detach_verb);
}

#[test]
fn detach_verb__a_sub_thread_s_answer_rides_it() {
    holds("turns_stream", "detachVerb", "a sub-thread's answer rides it", d::detach_verb);
}

#[test]
fn detach_verb__a_sub_thread_s_answer_rides_it_json() {
    holds("turns_stream", "detachVerb", "a sub-thread's answer rides it (--json)", d::detach_verb);
}

#[test]
fn detach_verb__queued_behind_the_running_turn() {
    holds("turns_stream", "detachVerb", "queued behind the running turn", d::detach_verb);
}

#[test]
fn detach_verb__queued_behind_the_running_turn_json() {
    holds("turns_stream", "detachVerb", "queued behind the running turn (--json)", d::detach_verb);
}

#[test]
fn detach_verb__queued_for_a_computer() {
    holds("turns_stream", "detachVerb", "queued for a computer", d::detach_verb);
}

#[test]
fn detach_verb__queued_for_a_computer_json() {
    holds("turns_stream", "detachVerb", "queued for a computer (--json)", d::detach_verb);
}

#[test]
fn detach_verb__queued_for_a_computer_then_behind_the_turn() {
    holds("turns_stream", "detachVerb", "queued for a computer, then behind the turn", d::detach_verb);
}

#[test]
fn detach_verb__queued_for_a_computer_then_behind_the_turn_json() {
    holds("turns_stream", "detachVerb", "queued for a computer, then behind the turn (--json)", d::detach_verb);
}

#[test]
fn detach_verb__held_with_its_cap() {
    holds("turns_stream", "detachVerb", "held with its cap", d::detach_verb);
}

#[test]
fn detach_verb__held_with_its_cap_json() {
    holds("turns_stream", "detachVerb", "held with its cap (--json)", d::detach_verb);
}

#[test]
fn detach_verb__held_with_its_cap_and_a_wait_for_a_slot() {
    holds("turns_stream", "detachVerb", "held with its cap and a wait for a slot", d::detach_verb);
}

#[test]
fn detach_verb__held_with_its_cap_and_a_wait_for_a_slot_json() {
    holds("turns_stream", "detachVerb", "held with its cap and a wait for a slot (--json)", d::detach_verb);
}

#[test]
fn detach_verb__steered() {
    holds("turns_stream", "detachVerb", "steered", d::detach_verb);
}

#[test]
fn detach_verb__steered_json() {
    holds("turns_stream", "detachVerb", "steered (--json)", d::detach_verb);
}

#[test]
fn detach_verb__steered_with_picks_dropped() {
    holds("turns_stream", "detachVerb", "steered with picks dropped", d::detach_verb);
}

#[test]
fn detach_verb__steered_with_picks_dropped_json() {
    holds("turns_stream", "detachVerb", "steered with picks dropped (--json)", d::detach_verb);
}

#[test]
fn detach_verb__the_host_stops_under_the_start_dialled_again() {
    holds("turns_stream", "detachVerb", "the host stops under the start, dialled again", d::detach_verb);
}

#[test]
fn detach_verb__the_host_stops_under_the_start_dialled_again_json() {
    holds("turns_stream", "detachVerb", "the host stops under the start, dialled again (--json)", d::detach_verb);
}

#[test]
fn detach_verb__the_host_stops_under_the_start_no_way_back() {
    holds("turns_stream", "detachVerb", "the host stops under the start, no way back", d::detach_verb);
}

#[test]
fn detach_verb__the_host_stops_under_the_start_no_way_back_json() {
    holds("turns_stream", "detachVerb", "the host stops under the start, no way back (--json)", d::detach_verb);
}

#[test]
fn detach_verb__the_host_refuses_the_start() {
    holds("turns_stream", "detachVerb", "the host refuses the start", d::detach_verb);
}

#[test]
fn detach_verb__the_host_refuses_the_start_json() {
    holds("turns_stream", "detachVerb", "the host refuses the start (--json)", d::detach_verb);
}

#[test]
fn detach_verb__send_waits_a_wait_behind_the_turn_turns_into_a_wait_for_a_co() {
    holds("turns_stream", "detachVerb", "send-waits: a wait behind the turn turns into a wait for a computer", d::detach_verb);
}

#[test]
fn detach_verb__send_waits_a_wait_behind_the_turn_turns_into_a_wait_for_a_co_2() {
    holds("turns_stream", "detachVerb", "send-waits: a wait behind the turn turns into a wait for a computer (--json)", d::detach_verb);
}

#[test]
fn detach_verb__send_waits_a_wait_for_a_computer_gives_way_to_a_wait_behind_() {
    holds("turns_stream", "detachVerb", "send-waits: a wait for a computer gives way to a wait behind the turn", d::detach_verb);
}

#[test]
fn detach_verb__send_waits_a_wait_for_a_computer_gives_way_to_a_wait_behind__2() {
    holds("turns_stream", "detachVerb", "send-waits: a wait for a computer gives way to a wait behind the turn (--json)", d::detach_verb);
}

#[test]
fn follow_verb__send_waits_a_followed_send_names_the_computer_at_the_wait_s_() {
    holds("turns_stream", "followVerb", "send-waits: a followed send names the computer at the wait's end too", d::follow_verb);
}

#[test]
fn follow_verb__send_waits_a_followed_send_names_the_computer_at_the_wait_s__2() {
    holds("turns_stream", "followVerb", "send-waits: a followed send names the computer at the wait's end too (--json)", d::follow_verb);
}

#[test]
fn follow_verb__send_waits_a_followed_send_that_waited_for_a_computer_alone() {
    holds("turns_stream", "followVerb", "send-waits: a followed send that waited for a computer alone", d::follow_verb);
}

#[test]
fn follow_verb__send_waits_a_followed_send_that_waited_for_a_computer_alone_() {
    holds("turns_stream", "followVerb", "send-waits: a followed send that waited for a computer alone (--json)", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_y() {
    holds("turns_stream", "followVerb", "keys pick by effect, y", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_y_json() {
    holds("turns_stream", "followVerb", "keys pick by effect, y (--json)", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_n() {
    holds("turns_stream", "followVerb", "keys pick by effect, n", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_n_json() {
    holds("turns_stream", "followVerb", "keys pick by effect, n (--json)", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_a() {
    holds("turns_stream", "followVerb", "keys pick by effect, a", d::follow_verb);
}

#[test]
fn follow_verb__keys_pick_by_effect_a_json() {
    holds("turns_stream", "followVerb", "keys pick by effect, a (--json)", d::follow_verb);
}

#[test]
fn follow_verb__a_second_prompt_replaces_the_first_read_of_keys() {
    holds("turns_stream", "followVerb", "a second prompt replaces the first read of keys", d::follow_verb);
}

#[test]
fn follow_verb__a_second_prompt_replaces_the_first_read_of_keys_json() {
    holds("turns_stream", "followVerb", "a second prompt replaces the first read of keys (--json)", d::follow_verb);
}

#[test]
fn before_sending__the_read_answers() {
    holds("turns_stream", "beforeSending", "the read answers", d::before_sending);
}

#[test]
fn before_sending__the_read_fails_and_the_host_did_not_stop() {
    holds("turns_stream", "beforeSending", "the read fails and the host did not stop", d::before_sending);
}

#[test]
fn before_sending__the_host_stopped_under_the_read() {
    holds("turns_stream", "beforeSending", "the host stopped under the read", d::before_sending);
}

#[test]
fn before_sending__the_host_closed_under_the_read_with_another_code() {
    holds("turns_stream", "beforeSending", "the host closed under the read with another code", d::before_sending);
}

#[test]
fn turn_view__started_no_result() {
    holds("turns_stream", "turnView", "started, no result", d::turn_view);
}

#[test]
fn turn_view__completed_with_text() {
    holds("turns_stream", "turnView", "completed with text", d::turn_view);
}

#[test]
fn turn_view__completed_with_no_text() {
    holds("turns_stream", "turnView", "completed with no text", d::turn_view);
}

#[test]
fn turn_view__failed_with_text() {
    holds("turns_stream", "turnView", "failed with text", d::turn_view);
}

#[test]
fn turn_view__held_with_its_cap() {
    holds("turns_stream", "turnView", "held with its cap", d::turn_view);
}

#[test]
fn turn_view__held_with_no_cap_on_the_session() {
    holds("turns_stream", "turnView", "held with no cap on the session", d::turn_view);
}

#[test]
fn turn_view__started_with_a_cap_on_the_session_is_not_held() {
    holds("turns_stream", "turnView", "started with a cap on the session is not held", d::turn_view);
}

#[test]
fn turn_view__after_a_cut_turn() {
    holds("turns_stream", "turnView", "after a cut turn", d::turn_view);
}

#[test]
fn turn_view__queued() {
    holds("turns_stream", "turnView", "queued", d::turn_view);
}

#[test]
fn turn_view__steered_with_the_harness_named() {
    holds("turns_stream", "turnView", "steered with the harness named", d::turn_view);
}
