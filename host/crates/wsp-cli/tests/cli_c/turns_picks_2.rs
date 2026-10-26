// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_picks.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_picks as d;

#[test]
fn thread_fork__a_bad_access_word() {
    holds("turns_picks", "threadFork", "a bad access word", d::thread_fork);
}

#[test]
fn thread_fork__an_access_the_agent_maps_to_no_mode() {
    holds("turns_picks", "threadFork", "an access the agent maps to no mode", d::thread_fork);
}

#[test]
fn thread_fork__an_empty_task_is_refused_after_the_thread_is_read() {
    holds("turns_picks", "threadFork", "an empty task is refused after the thread is read", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_zero_is_refused_before_the_host_is_aske() {
    holds("turns_picks", "threadFork", "a turn count that is zero is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_negative_is_refused_before_the_host_is_() {
    holds("turns_picks", "threadFork", "a turn count that is negative is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_a_word_is_refused_before_the_host_is_as() {
    holds("turns_picks", "threadFork", "a turn count that is a word is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_a_fraction_is_refused_before_the_host_i() {
    holds("turns_picks", "threadFork", "a turn count that is a fraction is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_an_empty_word_is_refused_before_the_hos() {
    holds("turns_picks", "threadFork", "a turn count that is an empty word is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_a_lone_minus_is_refused_before_the_host() {
    holds("turns_picks", "threadFork", "a turn count that is a lone minus is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_infinity_is_refused_before_the_host_is_() {
    holds("turns_picks", "threadFork", "a turn count that is infinity is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_not_a_number_is_refused_before_the_host() {
    holds("turns_picks", "threadFork", "a turn count that is not a number is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_an_underscore_is_refused_before_the_hos() {
    holds("turns_picks", "threadFork", "a turn count that is an underscore is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_that_is_a_unit_is_refused_before_the_host_is_as() {
    holds("turns_picks", "threadFork", "a turn count that is a unit is refused before the host is asked", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_spaces() {
    holds("turns_picks", "threadFork", "a turn count with spaces", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_a_plus_sign() {
    holds("turns_picks", "threadFork", "a turn count with a plus sign", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_an_exponent() {
    holds("turns_picks", "threadFork", "a turn count with an exponent", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_hex() {
    holds("turns_picks", "threadFork", "a turn count with hex", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_binary() {
    holds("turns_picks", "threadFork", "a turn count with binary", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_octal() {
    holds("turns_picks", "threadFork", "a turn count with octal", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_a_trailing_point() {
    holds("turns_picks", "threadFork", "a turn count with a trailing point", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_a_zero_fraction() {
    holds("turns_picks", "threadFork", "a turn count with a zero fraction", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_with_leading_zeros() {
    holds("turns_picks", "threadFork", "a turn count with leading zeros", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_the_number_one() {
    holds("turns_picks", "threadFork", "a turn count as the number one", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_the_number_zero_is_refused() {
    holds("turns_picks", "threadFork", "a turn count as the number zero is refused", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_a_negative_number_is_refused() {
    holds("turns_picks", "threadFork", "a turn count as a negative number is refused", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_a_fraction_is_refused() {
    holds("turns_picks", "threadFork", "a turn count as a fraction is refused", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_at_the_largest_safe_integer() {
    holds("turns_picks", "threadFork", "a turn count at the largest safe integer", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_one_past_the_largest_safe_integer_is_refused() {
    holds("turns_picks", "threadFork", "a turn count one past the largest safe integer is refused", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_the_word_of_the_largest_safe_integer() {
    holds("turns_picks", "threadFork", "a turn count as the word of the largest safe integer", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_count_as_the_word_one_past_it_is_refused() {
    holds("turns_picks", "threadFork", "a turn count as the word one past it is refused", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_naming_no_thread() {
    holds("turns_picks", "threadFork", "a ref naming no thread", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_naming_two_threads() {
    holds("turns_picks", "threadFork", "a ref naming two threads", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_naming_the_empty_prefix_names_every_thread() {
    holds("turns_picks", "threadFork", "a ref naming the empty prefix names every thread", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_that_is_a_project_reads_as_no_thread_and_says_so() {
    holds("turns_picks", "threadFork", "a ref that is a project reads as no thread and says so", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_that_is_a_project_with_the_cloud() {
    holds("turns_picks", "threadFork", "a ref that is a project, with the cloud", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_that_is_a_machine_with_the_cloud() {
    holds("turns_picks", "threadFork", "a ref that is a machine, with the cloud", d::thread_fork);
}

#[test]
fn thread_fork__a_ref_that_is_a_machine_with_no_cloud_is_only_no_thread() {
    holds("turns_picks", "threadFork", "a ref that is a machine, with no cloud, is only no thread", d::thread_fork);
}

#[test]
fn thread_fork__a_workspace_the_host_answers_in_another_version() {
    holds("turns_picks", "threadFork", "a workspace the host answers in another version", d::thread_fork);
}

#[test]
fn thread_fork__a_workspace_the_host_refuses() {
    holds("turns_picks", "threadFork", "a workspace the host refuses", d::thread_fork);
}

#[test]
fn thread_fork__the_host_refusing_the_lists() {
    holds("turns_picks", "threadFork", "the host refusing the lists", d::thread_fork);
}

#[test]
fn resume_asked__no_conversation_no_copy() {
    holds("turns_picks", "resumeAsked", "no conversation, no copy", d::resume_asked);
}

#[test]
fn resume_asked__no_conversation_no_copy_where_named() {
    holds("turns_picks", "resumeAsked", "no conversation, no copy, where named", d::resume_asked);
}

#[test]
fn resume_asked__a_copy_with_no_conversation() {
    holds("turns_picks", "resumeAsked", "a copy with no conversation", d::resume_asked);
}

#[test]
fn resume_asked__a_copy_with_no_conversation_beside_a_branch() {
    holds("turns_picks", "resumeAsked", "a copy with no conversation beside a branch", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation() {
    holds("turns_picks", "resumeAsked", "a conversation", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_to_copy() {
    holds("turns_picks", "resumeAsked", "a conversation to copy", d::resume_asked);
}

#[test]
fn resume_asked__an_empty_conversation_id_is_a_conversation() {
    holds("turns_picks", "resumeAsked", "an empty conversation id is a conversation", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_and_an_empty_where() {
    holds("turns_picks", "resumeAsked", "a conversation and an empty where", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_beside_a_branch() {
    holds("turns_picks", "resumeAsked", "a conversation beside a branch", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_beside_a_folder() {
    holds("turns_picks", "resumeAsked", "a conversation beside a folder", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_beside_a_thread() {
    holds("turns_picks", "resumeAsked", "a conversation beside a thread", d::resume_asked);
}

#[test]
fn resume_asked__a_conversation_beside_all_three() {
    holds("turns_picks", "resumeAsked", "a conversation beside all three", d::resume_asked);
}

#[test]
fn resume_asked__a_copy_of_a_conversation_beside_a_branch() {
    holds("turns_picks", "resumeAsked", "a copy of a conversation beside a branch", d::resume_asked);
}

#[test]
fn resume_asked__an_empty_branch_counts_as_named() {
    holds("turns_picks", "resumeAsked", "an empty branch counts as named", d::resume_asked);
}

#[test]
fn resume_asked__an_empty_folder_counts_as_named() {
    holds("turns_picks", "resumeAsked", "an empty folder counts as named", d::resume_asked);
}

#[test]
fn resume_asked__an_empty_thread_counts_as_named() {
    holds("turns_picks", "resumeAsked", "an empty thread counts as named", d::resume_asked);
}

#[test]
fn replaced_of__none_named_asks_nothing() {
    holds("turns_picks", "replacedOf", "none named asks nothing", d::replaced_of);
}

#[test]
fn replaced_of__by_id() {
    holds("turns_picks", "replacedOf", "by id", d::replaced_of);
}

#[test]
fn replaced_of__by_a_prefix_answered_by_the_full_id() {
    holds("turns_picks", "replacedOf", "by a prefix, answered by the full id", d::replaced_of);
}

#[test]
fn replaced_of__a_thread_with_no_thread_id_is_named_by_its_row() {
    holds("turns_picks", "replacedOf", "a thread with no thread id is named by its row", d::replaced_of);
}

#[test]
fn replaced_of__the_empty_word_names_every_thread() {
    holds("turns_picks", "replacedOf", "the empty word names every thread", d::replaced_of);
}

#[test]
fn replaced_of__a_word_naming_no_thread() {
    holds("turns_picks", "replacedOf", "a word naming no thread", d::replaced_of);
}

#[test]
fn replaced_of__a_word_naming_two() {
    holds("turns_picks", "replacedOf", "a word naming two", d::replaced_of);
}

#[test]
fn replaced_of__a_thread_the_host_says_may_not_be_replaced() {
    holds("turns_picks", "replacedOf", "a thread the host says may not be replaced", d::replaced_of);
}

#[test]
fn replaced_of__the_host_asked_of_nothing_it_knows() {
    holds("turns_picks", "replacedOf", "the host asked of nothing it knows", d::replaced_of);
}

#[test]
fn notify_of__none_named() {
    holds("turns_picks", "notifyOf", "none named", d::notify_of);
}

#[test]
fn notify_of__me() {
    holds("turns_picks", "notifyOf", "me", d::notify_of);
}

#[test]
fn notify_of__me_twice() {
    holds("turns_picks", "notifyOf", "me twice", d::notify_of);
}

#[test]
fn notify_of__a_thread_by_id() {
    holds("turns_picks", "notifyOf", "a thread by id", d::notify_of);
}

#[test]
fn notify_of__a_thread_by_a_prefix_is_named_by_its_full_id() {
    holds("turns_picks", "notifyOf", "a thread by a prefix is named by its full id", d::notify_of);
}

#[test]
fn notify_of__me_and_a_thread_one_listing_read_for_the_thread() {
    holds("turns_picks", "notifyOf", "me and a thread, one listing read for the thread", d::notify_of);
}

#[test]
fn notify_of__a_thread_and_me_in_the_order_named() {
    holds("turns_picks", "notifyOf", "a thread and me, in the order named", d::notify_of);
}

#[test]
fn notify_of__two_threads_read_the_listing_twice() {
    holds("turns_picks", "notifyOf", "two threads read the listing twice", d::notify_of);
}

#[test]
fn notify_of__the_same_thread_twice() {
    holds("turns_picks", "notifyOf", "the same thread twice", d::notify_of);
}

#[test]
fn notify_of__me_between_two_threads() {
    holds("turns_picks", "notifyOf", "me between two threads", d::notify_of);
}

#[test]
fn notify_of__a_row_with_no_thread_id_is_named_by_its_row() {
    holds("turns_picks", "notifyOf", "a row with no thread id is named by its row", d::notify_of);
}

#[test]
fn notify_of__the_word_me_is_exact_capitals_name_no_thread() {
    holds("turns_picks", "notifyOf", "the word me is exact: capitals name no thread", d::notify_of);
}

#[test]
fn notify_of__a_word_naming_no_thread() {
    holds("turns_picks", "notifyOf", "a word naming no thread", d::notify_of);
}

#[test]
fn notify_of__a_word_naming_two() {
    holds("turns_picks", "notifyOf", "a word naming two", d::notify_of);
}

#[test]
fn notify_of__the_first_good_ref_is_read_before_the_bad_one_is_refused() {
    holds("turns_picks", "notifyOf", "the first good ref is read before the bad one is refused", d::notify_of);
}

#[test]
fn notify_of__a_bad_ref_stops_the_rest() {
    holds("turns_picks", "notifyOf", "a bad ref stops the rest", d::notify_of);
}

#[test]
fn notify_of__the_host_refusing_the_listing() {
    holds("turns_picks", "notifyOf", "the host refusing the listing", d::notify_of);
}

#[test]
fn message_to__names_the_thread_by_its_runtime_id() {
    holds("turns_picks", "messageTo", "names the thread by its runtime id", d::message_to);
}

#[test]
fn message_to__a_row_with_a_thread_id_and_no_session() {
    holds("turns_picks", "messageTo", "a row with a thread id and no session", d::message_to);
}

#[test]
fn message_to__a_row_with_no_thread_id_is_named_by_its_own_id() {
    holds("turns_picks", "messageTo", "a row with no thread id is named by its own id", d::message_to);
}

#[test]
fn message_to__the_thread_s_own_agent() {
    holds("turns_picks", "messageTo", "the thread's own agent", d::message_to);
}

#[test]
fn message_to__an_empty_prompt_is_sent_as_it_is() {
    holds("turns_picks", "messageTo", "an empty prompt is sent as it is", d::message_to);
}

#[test]
fn message_to__the_picks_follow() {
    holds("turns_picks", "messageTo", "the picks follow", d::message_to);
}

#[test]
fn message_to__a_pick_access_word_of_the_harness_s_own_is_refused() {
    holds("turns_picks", "messageTo", "a pick access word of the harness's own is refused", d::message_to);
}

#[test]
fn message_to__fast_false_is_left_out() {
    holds("turns_picks", "messageTo", "fast false is left out", d::message_to);
}

#[test]
fn message_to__a_file_travels_before_the_picks() {
    holds("turns_picks", "messageTo", "a file travels before the picks", d::message_to);
}

#[test]
fn message_to__two_files() {
    holds("turns_picks", "messageTo", "two files", d::message_to);
}

#[test]
fn message_to__a_file_that_is_not_there() {
    holds("turns_picks", "messageTo", "a file that is not there", d::message_to);
}

#[test]
fn message_to__elsewhere_with_a_file_is_refused() {
    holds("turns_picks", "messageTo", "elsewhere with a file is refused", d::message_to);
}

#[test]
fn message_to__elsewhere_with_no_file_passes() {
    holds("turns_picks", "messageTo", "elsewhere with no file passes", d::message_to);
}

#[test]
fn message_to__the_files_are_read_before_the_picks_are_checked() {
    holds("turns_picks", "messageTo", "the files are read before the picks are checked", d::message_to);
}

#[test]
fn message_to__six_files() {
    holds("turns_picks", "messageTo", "six files", d::message_to);
}

#[test]
fn message_to__takes_no_folder_no_target_and_no_token() {
    holds("turns_picks", "messageTo", "takes no folder, no target and no token", d::message_to);
}
