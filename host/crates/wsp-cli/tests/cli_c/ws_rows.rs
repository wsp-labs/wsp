// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from ws_rows.json: one test per recorded case.
use super::common::holds;
use super::drivers_ws_rows as d;

#[test]
fn const_thread_head__the_head_of_the_threads_table() {
    holds("ws_rows", "constThreadHead", "the head of the threads table", d::const_thread_head);
}

#[test]
fn const_no_project_yet__the_line_with_no_project() {
    holds("ws_rows", "constNoProjectYet", "the line with no project", d::const_no_project_yet);
}

#[test]
fn made_worktree__no_worktree() {
    holds("ws_rows", "madeWorktree", "no worktree", d::made_worktree);
}

#[test]
fn made_worktree__a_worktree_wsp_made() {
    holds("ws_rows", "madeWorktree", "a worktree wsp made", d::made_worktree);
}

#[test]
fn made_worktree__a_worktree_wsp_did_not_make() {
    holds("ws_rows", "madeWorktree", "a worktree wsp did not make", d::made_worktree);
}

#[test]
fn made_worktree__a_worktree_wsp_made_and_removed() {
    holds("ws_rows", "madeWorktree", "a worktree wsp made and removed", d::made_worktree);
}

#[test]
fn made_worktree__a_made_worktree_with_gone_absent_keeps_its_path_as_given() {
    holds("ws_rows", "madeWorktree", "a made worktree with gone absent keeps its path as given", d::made_worktree);
}

#[test]
fn dropping__a_cloud_workspace_with_no_thread() {
    holds("ws_rows", "dropping", "a cloud workspace with no thread", d::dropping);
}

#[test]
fn dropping__one_thread_of_two_sessions_counts_once() {
    holds("ws_rows", "dropping", "one thread of two sessions counts once", d::dropping);
}

#[test]
fn dropping__two_threads_and_a_session_with_no_thread_id() {
    holds("ws_rows", "dropping", "two threads and a session with no thread id", d::dropping);
}

#[test]
fn dropping__a_box_folder_is_refused() {
    holds("ws_rows", "dropping", "a box folder is refused", d::dropping);
}

#[test]
fn dropping__a_local_folder_is_refused() {
    holds("ws_rows", "dropping", "a local folder is refused", d::dropping);
}

#[test]
fn dropping__a_local_worktree_is_refused_with_the_worktree_words() {
    holds("ws_rows", "dropping", "a local worktree is refused with the worktree words", d::dropping);
}

#[test]
fn dropping__a_local_worktree_already_gone_is_still_refused() {
    holds("ws_rows", "dropping", "a local worktree already gone is still refused", d::dropping);
}

#[test]
fn dropping__a_worktree_on_a_box_folder_is_refused_as_a_folder() {
    holds("ws_rows", "dropping", "a worktree on a box folder is refused as a folder", d::dropping);
}

#[test]
fn dropping__a_cloud_workspace_with_a_worktree_record_goes_on() {
    holds("ws_rows", "dropping", "a cloud workspace with a worktree record goes on", d::dropping);
}

#[test]
fn dropping__a_record_with_no_kind_is_a_cloud_workspace() {
    holds("ws_rows", "dropping", "a record with no kind is a cloud workspace", d::dropping);
}

#[test]
fn dropping__a_ref_that_is_a_name() {
    holds("ws_rows", "dropping", "a ref that is a name", d::dropping);
}

#[test]
fn dropping__an_empty_ref() {
    holds("ws_rows", "dropping", "an empty ref", d::dropping);
}

#[test]
fn dropping__the_host_refuses_the_ref() {
    holds("ws_rows", "dropping", "the host refuses the ref", d::dropping);
}

#[test]
fn dropping__the_sessions_listing_is_refused() {
    holds("ws_rows", "dropping", "the sessions listing is refused", d::dropping);
}

#[test]
fn dropping__a_workspace_that_does_not_read_is_another_version() {
    holds("ws_rows", "dropping", "a workspace that does not read is another version", d::dropping);
}

#[test]
fn dropping__no_workspace_in_the_reply_is_another_version() {
    holds("ws_rows", "dropping", "no workspace in the reply is another version", d::dropping);
}

#[test]
fn deleting__a_workspace_on_a_joined_computer_is_named_with_its_computer() {
    holds("ws_rows", "deleting", "a workspace on a joined computer is named with its computer", d::deleting);
}

#[test]
fn deleting__the_place_is_a_provider_so_the_delete_takes_the_cloud_words() {
    holds("ws_rows", "deleting", "the place is a provider, so the delete takes the cloud words", d::deleting);
}

#[test]
fn deleting__the_place_is_a_provider_with_a_key_name() {
    holds("ws_rows", "deleting", "the place is a provider with a key name", d::deleting);
}

#[test]
fn deleting__the_place_is_not_in_the_list() {
    holds("ws_rows", "deleting", "the place is not in the list", d::deleting);
}

#[test]
fn deleting__the_places_list_is_empty() {
    holds("ws_rows", "deleting", "the places list is empty", d::deleting);
}

#[test]
fn deleting__places_list_is_refused_so_the_computer_has_no_name() {
    holds("ws_rows", "deleting", "places.list is refused so the computer has no name", d::deleting);
}

#[test]
fn deleting__places_list_is_refused_with_an_auth_kind() {
    holds("ws_rows", "deleting", "places.list is refused with an auth kind", d::deleting);
}

#[test]
fn deleting__no_place_on_the_record_reads_no_places() {
    holds("ws_rows", "deleting", "no place on the record reads no places", d::deleting);
}

#[test]
fn deleting__two_places_the_second_is_the_one_named() {
    holds("ws_rows", "deleting", "two places, the second is the one named", d::deleting);
}

#[test]
fn deleting__two_rows_with_the_one_id_take_the_first() {
    holds("ws_rows", "deleting", "two rows with the one id take the first", d::deleting);
}

#[test]
fn deleting__a_name_with_an_apostrophe_and_a_space() {
    holds("ws_rows", "deleting", "a name with an apostrophe and a space", d::deleting);
}

#[test]
fn deleting__a_copy_on_a_computer_somebody_joined_is_named_by_the_compute() {
    holds("ws_rows", "deleting", "a copy on a computer somebody joined is named by the computer's name", d::deleting);
}

#[test]
fn deleting__a_caller_that_may_not_read_the_computers_names_still_never_r() {
    holds("ws_rows", "deleting", "a caller that may not read the computers' names still never reads the machine's id", d::deleting);
}

#[test]
fn deleting__a_fork_at_another_provider_s_account_takes_the_cloud_s_words() {
    holds("ws_rows", "deleting", "a fork at another provider's account takes the cloud's words", d::deleting);
}

#[test]
fn deleting__a_made_worktree_on_a_cloud_record() {
    holds("ws_rows", "deleting", "a made worktree on a cloud record", d::deleting);
}

#[test]
fn deleting__a_local_folder_is_refused_before_any_place_is_read() {
    holds("ws_rows", "deleting", "a local folder is refused before any place is read", d::deleting);
}

#[test]
fn deleting__a_local_worktree_is_refused_before_any_place_is_read() {
    holds("ws_rows", "deleting", "a local worktree is refused before any place is read", d::deleting);
}

#[test]
fn deleting__the_sessions_listing_is_refused_before_places_are_read() {
    holds("ws_rows", "deleting", "the sessions listing is refused before places are read", d::deleting);
}

#[test]
fn deleting__the_ref_names_nothing() {
    holds("ws_rows", "deleting", "the ref names nothing", d::deleting);
}

#[test]
fn deleting__the_socket_goes_while_the_places_are_read() {
    holds("ws_rows", "deleting", "the socket goes while the places are read", d::deleting);
}

#[test]
fn delete_question__lifecycle_test_this_computer_s_own() {
    holds("ws_rows", "deleteQuestion", "lifecycle test: this computer's own", d::delete_question);
}

#[test]
fn deleted_line__lifecycle_test_this_computer_s_own() {
    holds("ws_rows", "deletedLine", "lifecycle test: this computer's own", d::deleted_line);
}

#[test]
fn delete_question__lifecycle_test_a_worktree_wsp_made() {
    holds("ws_rows", "deleteQuestion", "lifecycle test: a worktree wsp made", d::delete_question);
}

#[test]
fn deleted_line__lifecycle_test_a_worktree_wsp_made() {
    holds("ws_rows", "deletedLine", "lifecycle test: a worktree wsp made", d::deleted_line);
}

#[test]
fn delete_question__lifecycle_test_a_fork() {
    holds("ws_rows", "deleteQuestion", "lifecycle test: a fork", d::delete_question);
}

#[test]
fn deleted_line__lifecycle_test_a_fork() {
    holds("ws_rows", "deletedLine", "lifecycle test: a fork", d::deleted_line);
}

#[test]
fn delete_question__lifecycle_test_a_copy_on_spoo() {
    holds("ws_rows", "deleteQuestion", "lifecycle test: a copy on spoo", d::delete_question);
}

#[test]
fn deleted_line__lifecycle_test_a_copy_on_spoo() {
    holds("ws_rows", "deletedLine", "lifecycle test: a copy on spoo", d::deleted_line);
}

#[test]
fn delete_question__lifecycle_test_a_copy_on_that_computer() {
    holds("ws_rows", "deleteQuestion", "lifecycle test: a copy on that computer", d::delete_question);
}

#[test]
fn deleted_line__lifecycle_test_a_copy_on_that_computer() {
    holds("ws_rows", "deletedLine", "lifecycle test: a copy on that computer", d::deleted_line);
}

#[test]
fn forget_question__no_thread() {
    holds("ws_rows", "forgetQuestion", "no thread", d::forget_question);
}

#[test]
fn forgot_line__no_thread() {
    holds("ws_rows", "forgotLine", "no thread", d::forgot_line);
}

#[test]
fn delete_question__no_thread() {
    holds("ws_rows", "deleteQuestion", "no thread", d::delete_question);
}

#[test]
fn deleted_line__no_thread() {
    holds("ws_rows", "deletedLine", "no thread", d::deleted_line);
}

#[test]
fn forget_question__one_thread() {
    holds("ws_rows", "forgetQuestion", "one thread", d::forget_question);
}

#[test]
fn forgot_line__one_thread() {
    holds("ws_rows", "forgotLine", "one thread", d::forgot_line);
}

#[test]
fn delete_question__one_thread() {
    holds("ws_rows", "deleteQuestion", "one thread", d::delete_question);
}

#[test]
fn deleted_line__one_thread() {
    holds("ws_rows", "deletedLine", "one thread", d::deleted_line);
}

#[test]
fn forget_question__two_threads() {
    holds("ws_rows", "forgetQuestion", "two threads", d::forget_question);
}

#[test]
fn forgot_line__two_threads() {
    holds("ws_rows", "forgotLine", "two threads", d::forgot_line);
}

#[test]
fn delete_question__two_threads() {
    holds("ws_rows", "deleteQuestion", "two threads", d::delete_question);
}

#[test]
fn deleted_line__two_threads() {
    holds("ws_rows", "deletedLine", "two threads", d::deleted_line);
}

#[test]
fn forget_question__a_thousand_threads() {
    holds("ws_rows", "forgetQuestion", "a thousand threads", d::forget_question);
}

#[test]
fn forgot_line__a_thousand_threads() {
    holds("ws_rows", "forgotLine", "a thousand threads", d::forgot_line);
}

#[test]
fn delete_question__a_thousand_threads() {
    holds("ws_rows", "deleteQuestion", "a thousand threads", d::delete_question);
}

#[test]
fn deleted_line__a_thousand_threads() {
    holds("ws_rows", "deletedLine", "a thousand threads", d::deleted_line);
}

#[test]
fn forget_question__a_name_with_quotes_and_an_emoji() {
    holds("ws_rows", "forgetQuestion", "a name with quotes and an emoji", d::forget_question);
}

#[test]
fn forgot_line__a_name_with_quotes_and_an_emoji() {
    holds("ws_rows", "forgotLine", "a name with quotes and an emoji", d::forgot_line);
}

#[test]
fn delete_question__a_name_with_quotes_and_an_emoji() {
    holds("ws_rows", "deleteQuestion", "a name with quotes and an emoji", d::delete_question);
}

#[test]
fn deleted_line__a_name_with_quotes_and_an_emoji() {
    holds("ws_rows", "deletedLine", "a name with quotes and an emoji", d::deleted_line);
}

#[test]
fn forget_question__a_gone_machine() {
    holds("ws_rows", "forgetQuestion", "a gone machine", d::forget_question);
}

#[test]
fn forgot_line__a_gone_machine() {
    holds("ws_rows", "forgotLine", "a gone machine", d::forgot_line);
}

#[test]
fn delete_question__a_gone_machine() {
    holds("ws_rows", "deleteQuestion", "a gone machine", d::delete_question);
}

#[test]
fn deleted_line__a_gone_machine() {
    holds("ws_rows", "deletedLine", "a gone machine", d::deleted_line);
}

#[test]
fn forget_question__a_made_worktree() {
    holds("ws_rows", "forgetQuestion", "a made worktree", d::forget_question);
}

#[test]
fn forgot_line__a_made_worktree() {
    holds("ws_rows", "forgotLine", "a made worktree", d::forgot_line);
}

#[test]
fn delete_question__a_made_worktree() {
    holds("ws_rows", "deleteQuestion", "a made worktree", d::delete_question);
}

#[test]
fn deleted_line__a_made_worktree() {
    holds("ws_rows", "deletedLine", "a made worktree", d::deleted_line);
}

#[test]
fn forget_question__a_worktree_wsp_did_not_make() {
    holds("ws_rows", "forgetQuestion", "a worktree wsp did not make", d::forget_question);
}

#[test]
fn forgot_line__a_worktree_wsp_did_not_make() {
    holds("ws_rows", "forgotLine", "a worktree wsp did not make", d::forgot_line);
}

#[test]
fn delete_question__a_worktree_wsp_did_not_make() {
    holds("ws_rows", "deleteQuestion", "a worktree wsp did not make", d::delete_question);
}

#[test]
fn deleted_line__a_worktree_wsp_did_not_make() {
    holds("ws_rows", "deletedLine", "a worktree wsp did not make", d::deleted_line);
}

#[test]
fn forget_question__a_worktree_made_and_gone() {
    holds("ws_rows", "forgetQuestion", "a worktree made and gone", d::forget_question);
}

#[test]
fn forgot_line__a_worktree_made_and_gone() {
    holds("ws_rows", "forgotLine", "a worktree made and gone", d::forgot_line);
}

#[test]
fn delete_question__a_worktree_made_and_gone() {
    holds("ws_rows", "deleteQuestion", "a worktree made and gone", d::delete_question);
}

#[test]
fn deleted_line__a_worktree_made_and_gone() {
    holds("ws_rows", "deletedLine", "a worktree made and gone", d::deleted_line);
}

#[test]
fn forget_question__a_create_that_made_no_machine() {
    holds("ws_rows", "forgetQuestion", "a create that made no machine", d::forget_question);
}

#[test]
fn forgot_line__a_create_that_made_no_machine() {
    holds("ws_rows", "forgotLine", "a create that made no machine", d::forgot_line);
}

#[test]
fn delete_question__a_create_that_made_no_machine() {
    holds("ws_rows", "deleteQuestion", "a create that made no machine", d::delete_question);
}

#[test]
fn deleted_line__a_create_that_made_no_machine() {
    holds("ws_rows", "deletedLine", "a create that made no machine", d::deleted_line);
}

#[test]
fn forget_question__a_computer_somebody_joined() {
    holds("ws_rows", "forgetQuestion", "a computer somebody joined", d::forget_question);
}

#[test]
fn forgot_line__a_computer_somebody_joined() {
    holds("ws_rows", "forgotLine", "a computer somebody joined", d::forgot_line);
}

#[test]
fn delete_question__a_computer_somebody_joined() {
    holds("ws_rows", "deleteQuestion", "a computer somebody joined", d::delete_question);
}

#[test]
fn deleted_line__a_computer_somebody_joined() {
    holds("ws_rows", "deletedLine", "a computer somebody joined", d::deleted_line);
}

#[test]
fn forget_question__a_computer_somebody_joined_that_has_no_name() {
    holds("ws_rows", "forgetQuestion", "a computer somebody joined that has no name", d::forget_question);
}

#[test]
fn forgot_line__a_computer_somebody_joined_that_has_no_name() {
    holds("ws_rows", "forgotLine", "a computer somebody joined that has no name", d::forgot_line);
}

#[test]
fn delete_question__a_computer_somebody_joined_that_has_no_name() {
    holds("ws_rows", "deleteQuestion", "a computer somebody joined that has no name", d::delete_question);
}

#[test]
fn deleted_line__a_computer_somebody_joined_that_has_no_name() {
    holds("ws_rows", "deletedLine", "a computer somebody joined that has no name", d::deleted_line);
}

#[test]
fn forget_question__a_joined_computer_with_a_worktree_record() {
    holds("ws_rows", "forgetQuestion", "a joined computer with a worktree record", d::forget_question);
}

#[test]
fn forgot_line__a_joined_computer_with_a_worktree_record() {
    holds("ws_rows", "forgotLine", "a joined computer with a worktree record", d::forgot_line);
}

#[test]
fn delete_question__a_joined_computer_with_a_worktree_record() {
    holds("ws_rows", "deleteQuestion", "a joined computer with a worktree record", d::delete_question);
}

#[test]
fn deleted_line__a_joined_computer_with_a_worktree_record() {
    holds("ws_rows", "deletedLine", "a joined computer with a worktree record", d::deleted_line);
}

#[test]
fn forget_question__a_joined_computer_and_no_machine_made() {
    holds("ws_rows", "forgetQuestion", "a joined computer and no machine made", d::forget_question);
}

#[test]
fn forgot_line__a_joined_computer_and_no_machine_made() {
    holds("ws_rows", "forgotLine", "a joined computer and no machine made", d::forgot_line);
}

#[test]
fn delete_question__a_joined_computer_and_no_machine_made() {
    holds("ws_rows", "deleteQuestion", "a joined computer and no machine made", d::delete_question);
}

#[test]
fn deleted_line__a_joined_computer_and_no_machine_made() {
    holds("ws_rows", "deletedLine", "a joined computer and no machine made", d::deleted_line);
}

#[test]
fn forget_question__a_box_folder_kind() {
    holds("ws_rows", "forgetQuestion", "a box folder kind", d::forget_question);
}

#[test]
fn forgot_line__a_box_folder_kind() {
    holds("ws_rows", "forgotLine", "a box folder kind", d::forgot_line);
}

#[test]
fn delete_question__a_box_folder_kind() {
    holds("ws_rows", "deleteQuestion", "a box folder kind", d::delete_question);
}

#[test]
fn deleted_line__a_box_folder_kind() {
    holds("ws_rows", "deletedLine", "a box folder kind", d::deleted_line);
}

#[test]
fn forget_question__a_local_kind() {
    holds("ws_rows", "forgetQuestion", "a local kind", d::forget_question);
}

#[test]
fn forgot_line__a_local_kind() {
    holds("ws_rows", "forgotLine", "a local kind", d::forgot_line);
}

#[test]
fn delete_question__a_local_kind() {
    holds("ws_rows", "deleteQuestion", "a local kind", d::delete_question);
}

#[test]
fn deleted_line__a_local_kind() {
    holds("ws_rows", "deletedLine", "a local kind", d::deleted_line);
}

#[test]
fn delete_question__a_cloud_with_the_cloud_on() {
    holds("ws_rows", "deleteQuestion", "a cloud with the cloud on", d::delete_question);
}

#[test]
fn deleted_line__a_cloud_with_the_cloud_on() {
    holds("ws_rows", "deletedLine", "a cloud with the cloud on", d::deleted_line);
}

#[test]
fn forget__sends_the_workspace_id() {
    holds("ws_rows", "forget", "sends the workspace id", d::forget);
}

#[test]
fn forget__the_host_refuses_a_machine_that_still_stands() {
    holds("ws_rows", "forget", "the host refuses a machine that still stands", d::forget);
}

#[test]
fn delete_workspace__sends_the_workspace_id() {
    holds("ws_rows", "deleteWorkspace", "sends the workspace id", d::delete_workspace);
}

#[test]
fn delete_workspace__the_host_refuses() {
    holds("ws_rows", "deleteWorkspace", "the host refuses", d::delete_workspace);
}

#[test]
fn delete_workspace__the_socket_goes_first() {
    holds("ws_rows", "deleteWorkspace", "the socket goes first", d::delete_workspace);
}

#[test]
fn shortened_front__39_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedFront", "39 cells in a width of 40", d::shortened_front);
}

#[test]
fn shortened_end__39_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedEnd", "39 cells in a width of 40", d::shortened_end);
}

#[test]
fn shortened_front__40_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedFront", "40 cells in a width of 40", d::shortened_front);
}

#[test]
fn shortened_end__40_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedEnd", "40 cells in a width of 40", d::shortened_end);
}

#[test]
fn shortened_front__41_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedFront", "41 cells in a width of 40", d::shortened_front);
}

#[test]
fn shortened_end__41_cells_in_a_width_of_40() {
    holds("ws_rows", "shortenedEnd", "41 cells in a width of 40", d::shortened_end);
}

#[test]
fn shortened_front__59_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedFront", "59 cells in a width of 60", d::shortened_front);
}

#[test]
fn shortened_end__59_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedEnd", "59 cells in a width of 60", d::shortened_end);
}

#[test]
fn shortened_front__60_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedFront", "60 cells in a width of 60", d::shortened_front);
}

#[test]
fn shortened_end__60_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedEnd", "60 cells in a width of 60", d::shortened_end);
}

#[test]
fn shortened_front__61_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedFront", "61 cells in a width of 60", d::shortened_front);
}

#[test]
fn shortened_end__61_cells_in_a_width_of_60() {
    holds("ws_rows", "shortenedEnd", "61 cells in a width of 60", d::shortened_end);
}

#[test]
fn shortened_front__0_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedFront", "0 cells in a width of 1", d::shortened_front);
}

#[test]
fn shortened_end__0_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedEnd", "0 cells in a width of 1", d::shortened_end);
}

#[test]
fn shortened_front__1_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedFront", "1 cells in a width of 1", d::shortened_front);
}

#[test]
fn shortened_end__1_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedEnd", "1 cells in a width of 1", d::shortened_end);
}

#[test]
fn shortened_front__2_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedFront", "2 cells in a width of 1", d::shortened_front);
}

#[test]
fn shortened_end__2_cells_in_a_width_of_1() {
    holds("ws_rows", "shortenedEnd", "2 cells in a width of 1", d::shortened_end);
}

#[test]
fn shortened_front__3_cells_in_a_width_of_2() {
    holds("ws_rows", "shortenedFront", "3 cells in a width of 2", d::shortened_front);
}

#[test]
fn shortened_end__3_cells_in_a_width_of_2() {
    holds("ws_rows", "shortenedEnd", "3 cells in a width of 2", d::shortened_end);
}

#[test]
fn shortened_front__4_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedFront", "4 cells in a width of 5", d::shortened_front);
}

#[test]
fn shortened_end__4_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedEnd", "4 cells in a width of 5", d::shortened_end);
}

#[test]
fn shortened_front__5_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedFront", "5 cells in a width of 5", d::shortened_front);
}

#[test]
fn shortened_end__5_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedEnd", "5 cells in a width of 5", d::shortened_end);
}

#[test]
fn shortened_front__6_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedFront", "6 cells in a width of 5", d::shortened_front);
}

#[test]
fn shortened_end__6_cells_in_a_width_of_5() {
    holds("ws_rows", "shortenedEnd", "6 cells in a width of 5", d::shortened_end);
}

#[test]
fn shortened_front__an_empty_path() {
    holds("ws_rows", "shortenedFront", "an empty path", d::shortened_front);
}

#[test]
fn shortened_end__an_empty_text() {
    holds("ws_rows", "shortenedEnd", "an empty text", d::shortened_end);
}

#[test]
fn shortened_front__accents_count_one_cell_each() {
    holds("ws_rows", "shortenedFront", "accents count one cell each", d::shortened_front);
}

#[test]
fn shortened_end__accents_count_one_cell_each() {
    holds("ws_rows", "shortenedEnd", "accents count one cell each", d::shortened_end);
}

#[test]
fn shortened_front__an_emoji_counts_two_cells_so_40_characters_are_41_cells() {
    holds("ws_rows", "shortenedFront", "an emoji counts two cells, so 40 characters are 41 cells", d::shortened_front);
}

#[test]
fn shortened_front__an_emoji_that_is_cut_whole() {
    holds("ws_rows", "shortenedFront", "an emoji that is cut whole", d::shortened_front);
}

#[test]
fn shortened_end__an_emoji_counts_two_cells_so_30_emoji_are_60_cells() {
    holds("ws_rows", "shortenedEnd", "an emoji counts two cells, so 30 emoji are 60 cells", d::shortened_end);
}

#[test]
fn shortened_end__an_emoji_early_is_kept_whole() {
    holds("ws_rows", "shortenedEnd", "an emoji early is kept whole", d::shortened_end);
}

#[test]
fn shortened_front__a_width_wider_than_the_path() {
    holds("ws_rows", "shortenedFront", "a width wider than the path", d::shortened_front);
}

#[test]
fn shortened_end__a_width_wider_than_the_text() {
    holds("ws_rows", "shortenedEnd", "a width wider than the text", d::shortened_end);
}

#[test]
fn shortened_end__newlines_and_tabs_are_cells_like_any_other() {
    holds("ws_rows", "shortenedEnd", "newlines and tabs are cells like any other", d::shortened_end);
}

#[test]
fn thread_lines__a_plain_thread() {
    holds("ws_rows", "threadLines", "a plain thread", d::thread_lines);
}

#[test]
fn thread_lines__a_thread_started_by_the_command_line() {
    holds("ws_rows", "threadLines", "a thread started by the command line", d::thread_lines);
}

#[test]
fn thread_lines__a_thread_started_by_an_agent() {
    holds("ws_rows", "threadLines", "a thread started by an agent", d::thread_lines);
}

#[test]
fn thread_lines__every_column_empty_but_the_id_and_the_agent() {
    holds("ws_rows", "threadLines", "every column empty but the id and the agent", d::thread_lines);
}

#[test]
fn thread_lines__indented_one_step() {
    holds("ws_rows", "threadLines", "indented one step", d::thread_lines);
}

#[test]
fn thread_lines__indented_two_steps_with_subagents() {
    holds("ws_rows", "threadLines", "indented two steps with subagents", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_under_the_home() {
    holds("ws_rows", "threadLines", "a folder under the home", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_that_is_the_home() {
    holds("ws_rows", "threadLines", "a folder that is the home", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_that_only_starts_with_the_home_s_name() {
    holds("ws_rows", "threadLines", "a folder that only starts with the home's name", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_outside_the_home() {
    holds("ws_rows", "threadLines", "a folder outside the home", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_with_the_home_and_a_trailing_slash() {
    holds("ws_rows", "threadLines", "a folder with the home and a trailing slash", d::thread_lines);
}

#[test]
fn thread_lines__the_home_is_the_root() {
    holds("ws_rows", "threadLines", "the home is the root", d::thread_lines);
}

#[test]
fn thread_lines__the_home_is_a_deep_folder() {
    holds("ws_rows", "threadLines", "the home is a deep folder", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_39_cells() {
    holds("ws_rows", "threadLines", "a folder of 39 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_40_cells() {
    holds("ws_rows", "threadLines", "a folder of 40 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_41_cells() {
    holds("ws_rows", "threadLines", "a folder of 41 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_42_cells() {
    holds("ws_rows", "threadLines", "a folder of 42 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_39_cells_once_the_home_is_a_tilde() {
    holds("ws_rows", "threadLines", "a folder of 39 cells once the home is a tilde", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_40_cells_once_the_home_is_a_tilde() {
    holds("ws_rows", "threadLines", "a folder of 40 cells once the home is a tilde", d::thread_lines);
}

#[test]
fn thread_lines__a_folder_of_41_cells_once_the_home_is_a_tilde() {
    holds("ws_rows", "threadLines", "a folder of 41 cells once the home is a tilde", d::thread_lines);
}

#[test]
fn thread_lines__a_long_folder_with_an_emoji_whose_cells_cross_the_cap() {
    holds("ws_rows", "threadLines", "a long folder with an emoji whose cells cross the cap", d::thread_lines);
}

#[test]
fn thread_lines__a_title_of_59_cells() {
    holds("ws_rows", "threadLines", "a title of 59 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_title_of_60_cells() {
    holds("ws_rows", "threadLines", "a title of 60 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_title_of_61_cells() {
    holds("ws_rows", "threadLines", "a title of 61 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_title_of_62_cells() {
    holds("ws_rows", "threadLines", "a title of 62 cells", d::thread_lines);
}

#[test]
fn thread_lines__a_title_with_accents_past_the_cap() {
    holds("ws_rows", "threadLines", "a title with accents past the cap", d::thread_lines);
}

#[test]
fn thread_lines__a_title_with_an_emoji_at_the_cap() {
    holds("ws_rows", "threadLines", "a title with an emoji at the cap", d::thread_lines);
}

#[test]
fn thread_lines__a_title_with_a_newline_stays_as_it_is() {
    holds("ws_rows", "threadLines", "a title with a newline stays as it is", d::thread_lines);
}

#[test]
fn thread_lines__a_thread_with_an_empty_subagent_list() {
    holds("ws_rows", "threadLines", "a thread with an empty subagent list", d::thread_lines);
}

#[test]
fn thread_lines__two_subagents_one_of_each_state() {
    holds("ws_rows", "threadLines", "two subagents, one of each state", d::thread_lines);
}

#[test]
fn thread_lines__a_subagent_title_of_59_60_and_61_cells() {
    holds("ws_rows", "threadLines", "a subagent title of 59, 60 and 61 cells", d::thread_lines);
}

#[test]
fn thread_lines__subagents_repeat_the_shortened_folder_and_the_branch() {
    holds("ws_rows", "threadLines", "subagents repeat the shortened folder and the branch", d::thread_lines);
}

#[test]
fn thread_lines__subagents_under_an_indent() {
    holds("ws_rows", "threadLines", "subagents under an indent", d::thread_lines);
}

#[test]
fn thread_lines__subagents_of_an_agent_that_is_not_claude() {
    holds("ws_rows", "threadLines", "subagents of an agent that is not claude", d::thread_lines);
}

#[test]
fn thread_lines__running() {
    holds("ws_rows", "threadLines", "running", d::thread_lines);
}

#[test]
fn thread_lines__completed_and_seen() {
    holds("ws_rows", "threadLines", "completed and seen", d::thread_lines);
}

#[test]
fn thread_lines__completed_and_unseen_reads_done() {
    holds("ws_rows", "threadLines", "completed and unseen reads done", d::thread_lines);
}

#[test]
fn thread_lines__interrupted_reads_idle() {
    holds("ws_rows", "threadLines", "interrupted reads idle", d::thread_lines);
}

#[test]
fn thread_lines__failed() {
    holds("ws_rows", "threadLines", "failed", d::thread_lines);
}

#[test]
fn thread_lines__failed_and_unseen_still_reads_failed() {
    holds("ws_rows", "threadLines", "failed and unseen still reads failed", d::thread_lines);
}

#[test]
fn thread_lines__asking_reads_needs_you() {
    holds("ws_rows", "threadLines", "asking reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__waiting_on_another_thread_reads_needs_you() {
    holds("ws_rows", "threadLines", "waiting on another thread reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__a_disk_kept_until_long_ago_reads_expired() {
    holds("ws_rows", "threadLines", "a disk kept until long ago reads expired", d::thread_lines);
}

#[test]
fn thread_lines__a_disk_kept_until_the_far_future_reads_idle() {
    holds("ws_rows", "threadLines", "a disk kept until the far future reads idle", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_of_a_running_turn_reads_working() {
    holds("ws_rows", "threadLines", "an expired disk of a running turn reads working", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_of_a_failed_turn_reads_failed() {
    holds("ws_rows", "threadLines", "an expired disk of a failed turn reads failed", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_with_a_question_reads_needs_you() {
    holds("ws_rows", "threadLines", "an expired disk with a question reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_behind_another_thread_s_question_reads_needs() {
    holds("ws_rows", "threadLines", "an expired disk behind another thread's question reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_of_an_interrupted_turn() {
    holds("ws_rows", "threadLines", "an expired disk of an interrupted turn", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_of_an_unseen_finish() {
    holds("ws_rows", "threadLines", "an expired disk of an unseen finish", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_of_a_sub_thread_wins_over_its_own_word() {
    holds("ws_rows", "threadLines", "an expired disk of a sub-thread wins over its own word", d::thread_lines);
}

#[test]
fn thread_lines__an_expired_disk_keeps_the_subagent_rows() {
    holds("ws_rows", "threadLines", "an expired disk keeps the subagent rows", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_waiting_for_a_slot() {
    holds("ws_rows", "threadLines", "a sub-thread waiting for a slot", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_starting() {
    holds("ws_rows", "threadLines", "a sub-thread starting", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_released_after_a_completed_turn() {
    holds("ws_rows", "threadLines", "a sub-thread released after a completed turn", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_released_after_an_interrupted_turn() {
    holds("ws_rows", "threadLines", "a sub-thread released after an interrupted turn", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_released_after_a_failed_turn() {
    holds("ws_rows", "threadLines", "a sub-thread released after a failed turn", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_released_and_unseen_reads_done() {
    holds("ws_rows", "threadLines", "a sub-thread released and unseen reads done", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_released_and_still_running() {
    holds("ws_rows", "threadLines", "a sub-thread released and still running", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_napping_with_work_not_pushed() {
    holds("ws_rows", "threadLines", "a sub-thread napping with work not pushed", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_running_with_work_not_pushed_and_the_turn_done() {
    holds("ws_rows", "threadLines", "a sub-thread running with work not pushed and the turn done", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_napping_with_work_not_pushed_while_its_turn_run() {
    holds("ws_rows", "threadLines", "a sub-thread napping with work not pushed while its turn runs", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_napping_with_nothing_held() {
    holds("ws_rows", "threadLines", "a sub-thread napping with nothing held", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_running_with_nothing_held() {
    holds("ws_rows", "threadLines", "a sub-thread running with nothing held", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_waiting_that_asks_reads_needs_you() {
    holds("ws_rows", "threadLines", "a sub-thread waiting that asks reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_starting_behind_another_thread_s_question_reads() {
    holds("ws_rows", "threadLines", "a sub-thread starting behind another thread's question reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_with_work_not_pushed_that_asks_reads_needs_you() {
    holds("ws_rows", "threadLines", "a sub-thread with work not pushed that asks reads needs you", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_with_work_not_pushed_and_an_interrupted_turn() {
    holds("ws_rows", "threadLines", "a sub-thread with work not pushed and an interrupted turn", d::thread_lines);
}

#[test]
fn thread_lines__a_sub_thread_with_subagents_keeps_the_sub_thread_word_on_its() {
    holds("ws_rows", "threadLines", "a sub-thread with subagents keeps the sub-thread word on its own row only", d::thread_lines);
}

#[test]
fn thread_lines__a_thread_with_no_home_to_shorten_against() {
    holds("ws_rows", "threadLines", "a thread with no home to shorten against", d::thread_lines);
}

#[test]
fn disk_gone_lines__no_rows() {
    holds("ws_rows", "diskGoneLines", "no rows", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__no_row_holds_a_gone_disk() {
    holds("ws_rows", "diskGoneLines", "no row holds a gone disk", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__one_row() {
    holds("ws_rows", "diskGoneLines", "one row", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__two_rows_in_order_with_one_between() {
    holds("ws_rows", "diskGoneLines", "two rows in order with one between", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__an_empty_reason_is_still_a_line() {
    holds("ws_rows", "diskGoneLines", "an empty reason is still a line", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__a_reason_with_a_newline_and_an_emoji() {
    holds("ws_rows", "diskGoneLines", "a reason with a newline and an emoji", d::disk_gone_lines);
}

#[test]
fn disk_gone_lines__the_same_id_twice_is_two_lines() {
    holds("ws_rows", "diskGoneLines", "the same id twice is two lines", d::disk_gone_lines);
}

#[test]
fn thread_tree__no_rows() {
    holds("ws_rows", "threadTree", "no rows", d::thread_tree);
}

#[test]
fn thread_tree__one_row() {
    holds("ws_rows", "threadTree", "one row", d::thread_tree);
}

#[test]
fn thread_tree__two_roots_in_the_order_given() {
    holds("ws_rows", "threadTree", "two roots in the order given", d::thread_tree);
}

#[test]
fn thread_tree__a_chain() {
    holds("ws_rows", "threadTree", "a chain", d::thread_tree);
}

#[test]
fn thread_tree__a_chain_given_child_first() {
    holds("ws_rows", "threadTree", "a chain given child first", d::thread_tree);
}

#[test]
fn thread_tree__a_parent_after_its_child() {
    holds("ws_rows", "threadTree", "a parent after its child", d::thread_tree);
}

#[test]
fn thread_tree__siblings_keep_their_order_under_one_parent() {
    holds("ws_rows", "threadTree", "siblings keep their order under one parent", d::thread_tree);
}

#[test]
fn thread_tree__two_trees_do_not_interleave() {
    holds("ws_rows", "threadTree", "two trees do not interleave", d::thread_tree);
}

#[test]
fn thread_tree__a_parent_missing_from_the_listing_stands_at_the_top() {
    holds("ws_rows", "threadTree", "a parent missing from the listing stands at the top", d::thread_tree);
}

#[test]
fn thread_tree__a_missing_parent_s_children_hang_under_it() {
    holds("ws_rows", "threadTree", "a missing parent's children hang under it", d::thread_tree);
}

#[test]
fn thread_tree__a_root_and_an_orphan_keep_their_order() {
    holds("ws_rows", "threadTree", "a root and an orphan keep their order", d::thread_tree);
}

#[test]
fn thread_tree__a_parent_of_the_empty_string_stands_at_the_top() {
    holds("ws_rows", "threadTree", "a parent of the empty string stands at the top", d::thread_tree);
}

#[test]
fn thread_tree__a_row_whose_id_is_the_empty_string_holds_a_parent_of_that_na() {
    holds("ws_rows", "threadTree", "a row whose id is the empty string holds a parent of that name", d::thread_tree);
}

#[test]
fn thread_tree__two_rows_that_are_each_other_s_parent_stand_flat_at_the_top() {
    holds("ws_rows", "threadTree", "two rows that are each other's parent stand flat at the top", d::thread_tree);
}

#[test]
fn thread_tree__a_cycle_beside_a_healthy_tree() {
    holds("ws_rows", "threadTree", "a cycle beside a healthy tree", d::thread_tree);
}

#[test]
fn thread_tree__a_thread_that_is_its_own_parent_stands_at_the_top() {
    holds("ws_rows", "threadTree", "a thread that is its own parent stands at the top", d::thread_tree);
}

#[test]
fn thread_tree__a_cycle_with_a_tail_stands_flat_in_the_order_given() {
    holds("ws_rows", "threadTree", "a cycle with a tail stands flat in the order given", d::thread_tree);
}

#[test]
fn thread_tree__a_three_cycle() {
    holds("ws_rows", "threadTree", "a three-cycle", d::thread_tree);
}

#[test]
fn thread_tree__a_row_with_the_same_id_twice_draws_the_first_and_drops_the_s() {
    holds("ws_rows", "threadTree", "a row with the same id twice draws the first and drops the second", d::thread_tree);
}

#[test]
fn thread_tree__a_duplicate_that_is_a_child() {
    holds("ws_rows", "threadTree", "a duplicate that is a child", d::thread_tree);
}

#[test]
fn thread_tree__a_deep_chain() {
    holds("ws_rows", "threadTree", "a deep chain", d::thread_tree);
}

#[test]
fn thread_tree__a_wide_tree() {
    holds("ws_rows", "threadTree", "a wide tree", d::thread_tree);
}

#[test]
fn thread_tree__nested_siblings_interleave_depth_first() {
    holds("ws_rows", "threadTree", "nested siblings interleave depth first", d::thread_tree);
}

#[test]
fn thread_tree__a_tree_listed_backwards() {
    holds("ws_rows", "threadTree", "a tree listed backwards", d::thread_tree);
}

#[test]
fn thread_tree__rows_with_a_root_thread_id_are_not_read() {
    holds("ws_rows", "threadTree", "rows with a root thread id are not read", d::thread_tree);
}

#[test]
fn place_names__no_places() {
    holds("ws_rows", "placeNames", "no places", d::place_names);
}

#[test]
fn place_names__one_computer() {
    holds("ws_rows", "placeNames", "one computer", d::place_names);
}

#[test]
fn place_names__a_computer_a_provider_and_a_joined_box() {
    holds("ws_rows", "placeNames", "a computer, a provider and a joined box", d::place_names);
}

#[test]
fn place_names__the_box_provider_by_its_key_name() {
    holds("ws_rows", "placeNames", "the box provider by its key name", d::place_names);
}

#[test]
fn place_names__a_label_is_not_the_table_name() {
    holds("ws_rows", "placeNames", "a label is not the table name", d::place_names);
}

#[test]
fn place_names__two_rows_with_one_id_keep_the_last_name_in_the_first_place() {
    holds("ws_rows", "placeNames", "two rows with one id keep the last name in the first place", d::place_names);
}

#[test]
fn place_names__a_name_with_an_emoji_and_a_newline() {
    holds("ws_rows", "placeNames", "a name with an emoji and a newline", d::place_names);
}

#[test]
fn place_names__places_list_is_refused() {
    holds("ws_rows", "placeNames", "places.list is refused", d::place_names);
}

#[test]
fn place_names__the_socket_is_gone() {
    holds("ws_rows", "placeNames", "the socket is gone", d::place_names);
}

#[test]
fn projects_of__no_projects() {
    holds("ws_rows", "projectsOf", "no projects", d::projects_of);
}

#[test]
fn projects_of__two_projects_in_the_host_s_order() {
    holds("ws_rows", "projectsOf", "two projects in the host's order", d::projects_of);
}

#[test]
fn projects_of__a_project_with_a_base() {
    holds("ws_rows", "projectsOf", "a project with a base", d::projects_of);
}

#[test]
fn projects_of__the_host_refuses() {
    holds("ws_rows", "projectsOf", "the host refuses", d::projects_of);
}

#[test]
fn project_of__by_name() {
    holds("ws_rows", "projectOf", "by name", d::project_of);
}

#[test]
fn project_of__by_id_with_a_git_source() {
    holds("ws_rows", "projectOf", "by id with a git source", d::project_of);
}

#[test]
fn project_of__an_empty_ref() {
    holds("ws_rows", "projectOf", "an empty ref", d::project_of);
}

#[test]
fn project_of__a_ref_naming_nothing() {
    holds("ws_rows", "projectOf", "a ref naming nothing", d::project_of);
}

#[test]
fn project_line__a_folder_project_on_this_computer() {
    holds("ws_rows", "projectLine", "a folder project on this computer", d::project_line);
}

#[test]
fn project_line__a_project_on_a_computer_by_its_name() {
    holds("ws_rows", "projectLine", "a project on a computer by its name", d::project_line);
}

#[test]
fn project_line__a_project_on_a_computer_nobody_named_is_shown_by_its_id() {
    holds("ws_rows", "projectLine", "a project on a computer nobody named is shown by its id", d::project_line);
}

#[test]
fn project_line__a_git_source() {
    holds("ws_rows", "projectLine", "a git source", d::project_line);
}

#[test]
fn project_line__a_github_source() {
    holds("ws_rows", "projectLine", "a github source", d::project_line);
}

#[test]
fn project_line__a_gitlab_source() {
    holds("ws_rows", "projectLine", "a gitlab source", d::project_line);
}

#[test]
fn project_line__a_base_branch() {
    holds("ws_rows", "projectLine", "a base branch", d::project_line);
}

#[test]
fn project_line__an_empty_base_is_kept_empty() {
    holds("ws_rows", "projectLine", "an empty base is kept empty", d::project_line);
}

#[test]
fn project_line__a_path_of_39_cells() {
    holds("ws_rows", "projectLine", "a path of 39 cells", d::project_line);
}

#[test]
fn project_line__a_path_of_40_cells() {
    holds("ws_rows", "projectLine", "a path of 40 cells", d::project_line);
}

#[test]
fn project_line__a_path_of_41_cells() {
    holds("ws_rows", "projectLine", "a path of 41 cells", d::project_line);
}

#[test]
fn project_line__a_path_with_an_emoji_at_the_cap() {
    holds("ws_rows", "projectLine", "a path with an emoji at the cap", d::project_line);
}

#[test]
fn project_line__no_workspace_and_no_thread_counts_zero() {
    holds("ws_rows", "projectLine", "no workspace and no thread counts zero", d::project_line);
}

#[test]
fn project_line__a_thread_of_a_workspace_of_this_project_counts() {
    holds("ws_rows", "projectLine", "a thread of a workspace of this project counts", d::project_line);
}

#[test]
fn project_line__threads_of_another_project_s_workspace_do_not_count() {
    holds("ws_rows", "projectLine", "threads of another project's workspace do not count", d::project_line);
}

#[test]
fn project_line__a_thread_of_a_workspace_not_listed_does_not_count() {
    holds("ws_rows", "projectLine", "a thread of a workspace not listed does not count", d::project_line);
}

#[test]
fn project_line__three_threads_over_two_workspaces_and_one_of_another_project() {
    holds("ws_rows", "projectLine", "three threads over two workspaces and one of another project", d::project_line);
}

#[test]
fn project_line__a_project_name_with_a_newline_and_an_emoji() {
    holds("ws_rows", "projectLine", "a project name with a newline and an emoji", d::project_line);
}

#[test]
fn project_line__two_workspaces_with_one_id_count_a_thread_once() {
    holds("ws_rows", "projectLine", "two workspaces with one id count a thread once", d::project_line);
}

#[test]
fn create_for__a_cloud_project_with_nothing_asked() {
    holds("ws_rows", "createFor", "a cloud project with nothing asked", d::create_for);
}

#[test]
fn create_for__the_folder_the_workspace_holds_the_project_in() {
    holds("ws_rows", "createFor", "the folder the workspace holds the project in", d::create_for);
}

#[test]
fn create_for__a_notice_rides_under_the_line() {
    holds("ws_rows", "createFor", "a notice rides under the line", d::create_for);
}

#[test]
fn create_for__an_empty_notice_is_still_a_notice() {
    holds("ws_rows", "createFor", "an empty notice is still a notice", d::create_for);
}

#[test]
fn create_for__under_json_the_value_is_one_line_and_the_lines_are_frames() {
    holds("ws_rows", "createFor", "under --json the value is one line and the lines are frames", d::create_for);
}

#[test]
fn create_for__stages_stream_to_stderr_as_they_arrive() {
    holds("ws_rows", "createFor", "stages stream to stderr as they arrive", d::create_for);
}

#[test]
fn create_for__a_stage_of_another_name_is_not_shown() {
    holds("ws_rows", "createFor", "a stage of another name is not shown", d::create_for);
}

#[test]
fn create_for__a_frame_of_another_type_is_not_shown() {
    holds("ws_rows", "createFor", "a frame of another type is not shown", d::create_for);
}

#[test]
fn create_for__stages_heard_while_the_subscription_is_asked_are_held_and_sh() {
    holds("ws_rows", "createFor", "stages heard while the subscription is asked are held and shown first", d::create_for);
}

#[test]
fn create_for__a_stage_heard_after_the_reply_is_dropped() {
    holds("ws_rows", "createFor", "a stage heard after the reply is dropped", d::create_for);
}

#[test]
fn create_for__stages_under_json_print_as_frames_before_the_result() {
    holds("ws_rows", "createFor", "stages under --json print as frames before the result", d::create_for);
}

#[test]
fn create_for__a_stage_with_a_message_holding_a_newline_and_an_emoji() {
    holds("ws_rows", "createFor", "a stage with a message holding a newline and an emoji", d::create_for);
}

#[test]
fn create_for__a_size_on_a_cloud_project() {
    holds("ws_rows", "createFor", "a size on a cloud project", d::create_for);
}

#[test]
fn create_for__a_size_of_half_a_gigabyte_steps() {
    holds("ws_rows", "createFor", "a size of half a gigabyte steps", d::create_for);
}

#[test]
fn create_for__a_size_word_with_spaces_round_it() {
    holds("ws_rows", "createFor", "a size word with spaces round it", d::create_for);
}

#[test]
fn create_for__a_size_the_provider_does_not_offer() {
    holds("ws_rows", "createFor", "a size the provider does not offer", d::create_for);
}

#[test]
fn create_for__a_size_that_is_no_size() {
    holds("ws_rows", "createFor", "a size that is no size", d::create_for);
}

#[test]
fn create_for__an_empty_size_word() {
    holds("ws_rows", "createFor", "an empty size word", d::create_for);
}

#[test]
fn create_for__a_size_when_the_provider_offers_none() {
    holds("ws_rows", "createFor", "a size when the provider offers none", d::create_for);
}

#[test]
fn create_for__a_size_with_a_named_offer() {
    holds("ws_rows", "createFor", "a size with a named offer", d::create_for);
}

#[test]
fn create_for__a_size_on_a_copy_of_a_folder_is_refused_before_anything_is_s() {
    holds("ws_rows", "createFor", "a size on a copy of a folder is refused before anything is sent", d::create_for);
}

#[test]
fn create_for__an_empty_size_on_a_copy_of_a_folder_is_refused_too() {
    holds("ws_rows", "createFor", "an empty size on a copy of a folder is refused too", d::create_for);
}

#[test]
fn create_for__no_size_on_a_copy_of_a_folder_goes_on() {
    holds("ws_rows", "createFor", "no size on a copy of a folder goes on", d::create_for);
}

#[test]
fn create_for__agents_with_both_caps() {
    holds("ws_rows", "createFor", "agents with both caps", d::create_for);
}

#[test]
fn create_for__agents_with_one_cap() {
    holds("ws_rows", "createFor", "agents with one cap", d::create_for);
}

#[test]
fn create_for__agents_with_no_cap_named_is_still_sent() {
    holds("ws_rows", "createFor", "agents with no cap named is still sent", d::create_for);
}

#[test]
fn create_for__a_parent() {
    holds("ws_rows", "createFor", "a parent", d::create_for);
}

#[test]
fn create_for__a_size_agents_and_a_parent_in_the_order_sent() {
    holds("ws_rows", "createFor", "a size, agents and a parent in the order sent", d::create_for);
}

#[test]
fn create_for__an_empty_name_is_sent_as_it_is() {
    holds("ws_rows", "createFor", "an empty name is sent as it is", d::create_for);
}

#[test]
fn create_for__the_landing_is_refused_so_nothing_else_is_sent() {
    holds("ws_rows", "createFor", "the landing is refused so nothing else is sent", d::create_for);
}

#[test]
fn create_for__the_create_is_refused() {
    holds("ws_rows", "createFor", "the create is refused", d::create_for);
}

#[test]
fn create_for__the_subscription_is_refused_so_no_create_is_sent() {
    holds("ws_rows", "createFor", "the subscription is refused so no create is sent", d::create_for);
}

#[test]
fn create_for__the_socket_goes_during_the_create() {
    holds("ws_rows", "createFor", "the socket goes during the create", d::create_for);
}

#[test]
fn create_for__the_socket_goes_during_the_landing() {
    holds("ws_rows", "createFor", "the socket goes during the landing", d::create_for);
}

#[test]
fn create_for__a_project_with_a_name_that_needs_escaping() {
    holds("ws_rows", "createFor", "a project with a name that needs escaping", d::create_for);
}

#[test]
fn projects_here__the_host_lists_two_projects() {
    holds("ws_rows", "projectsHere", "the host lists two projects", d::projects_here);
}

#[test]
fn projects_here__the_host_lists_none_which_is_an_answer() {
    holds("ws_rows", "projectsHere", "the host lists none, which is an answer", d::projects_here);
}

#[test]
fn projects_here__the_list_is_refused_so_the_workspaces_name_them() {
    holds("ws_rows", "projectsHere", "the list is refused so the workspaces name them", d::projects_here);
}

#[test]
fn projects_here__the_list_is_refused_with_an_auth_kind() {
    holds("ws_rows", "projectsHere", "the list is refused with an auth kind", d::projects_here);
}

#[test]
fn projects_here__a_reply_with_no_projects_falls_back() {
    holds("ws_rows", "projectsHere", "a reply with no projects falls back", d::projects_here);
}

#[test]
fn projects_here__two_workspaces_of_one_project_name_it_once_the_last_name_in_() {
    holds("ws_rows", "projectsHere", "two workspaces of one project name it once, the last name in the first place", d::projects_here);
}

#[test]
fn projects_here__no_workspace_either() {
    holds("ws_rows", "projectsHere", "no workspace either", d::projects_here);
}

#[test]
fn projects_here__both_reads_are_refused() {
    holds("ws_rows", "projectsHere", "both reads are refused", d::projects_here);
}

#[test]
fn projects_here__the_socket_goes_on_the_second_read() {
    holds("ws_rows", "projectsHere", "the socket goes on the second read", d::projects_here);
}

#[test]
fn agents_asked__nothing_named() {
    holds("ws_rows", "agentsAsked", "nothing named", d::agents_asked);
}

#[test]
fn agents_asked__machines_only() {
    holds("ws_rows", "agentsAsked", "machines only", d::agents_asked);
}

#[test]
fn agents_asked__depth_only() {
    holds("ws_rows", "agentsAsked", "depth only", d::agents_asked);
}

#[test]
fn agents_asked__both_machines_first() {
    holds("ws_rows", "agentsAsked", "both, machines first", d::agents_asked);
}

#[test]
fn agents_asked__both_as_numbers() {
    holds("ws_rows", "agentsAsked", "both as numbers", d::agents_asked);
}

#[test]
fn agents_asked__machines_zero_is_a_cap() {
    holds("ws_rows", "agentsAsked", "machines zero is a cap", d::agents_asked);
}

#[test]
fn agents_asked__depth_zero_is_refused() {
    holds("ws_rows", "agentsAsked", "depth zero is refused", d::agents_asked);
}

#[test]
fn agents_asked__machines_below_zero() {
    holds("ws_rows", "agentsAsked", "machines below zero", d::agents_asked);
}

#[test]
fn agents_asked__machines_that_is_no_number() {
    holds("ws_rows", "agentsAsked", "machines that is no number", d::agents_asked);
}

#[test]
fn agents_asked__the_machines_are_read_before_the_depth() {
    holds("ws_rows", "agentsAsked", "the machines are read before the depth", d::agents_asked);
}

#[test]
fn agents_asked__a_good_machines_and_a_bad_depth() {
    holds("ws_rows", "agentsAsked", "a good machines and a bad depth", d::agents_asked);
}

#[test]
fn agents_asked__a_computer_names_the_flag() {
    holds("ws_rows", "agentsAsked", "a computer names the flag", d::agents_asked);
}

#[test]
fn agents_asked__a_computer_names_the_depth_flag() {
    holds("ws_rows", "agentsAsked", "a computer names the depth flag", d::agents_asked);
}

#[test]
fn agents_asked__a_computer_and_no_flag_is_not_read() {
    holds("ws_rows", "agentsAsked", "a computer and no flag is not read", d::agents_asked);
}

#[test]
fn count_asked__a_plain_number() {
    holds("ws_rows", "countAsked", "a plain number", d::count_asked);
}

#[test]
fn count_asked__zero_where_zero_is_allowed() {
    holds("ws_rows", "countAsked", "zero where zero is allowed", d::count_asked);
}

#[test]
fn count_asked__zero_where_one_is_the_least() {
    holds("ws_rows", "countAsked", "zero where one is the least", d::count_asked);
}

#[test]
fn count_asked__one_where_one_is_the_least() {
    holds("ws_rows", "countAsked", "one where one is the least", d::count_asked);
}

#[test]
fn count_asked__below_zero() {
    holds("ws_rows", "countAsked", "below zero", d::count_asked);
}

#[test]
fn count_asked__a_fraction() {
    holds("ws_rows", "countAsked", "a fraction", d::count_asked);
}

#[test]
fn count_asked__a_fraction_that_is_whole() {
    holds("ws_rows", "countAsked", "a fraction that is whole", d::count_asked);
}

#[test]
fn count_asked__an_exponent_that_is_whole() {
    holds("ws_rows", "countAsked", "an exponent that is whole", d::count_asked);
}

#[test]
fn count_asked__a_hex_word() {
    holds("ws_rows", "countAsked", "a hex word", d::count_asked);
}

#[test]
fn count_asked__white_space_around_a_number() {
    holds("ws_rows", "countAsked", "white space around a number", d::count_asked);
}

#[test]
fn count_asked__an_empty_word_reads_as_zero() {
    holds("ws_rows", "countAsked", "an empty word reads as zero", d::count_asked);
}

#[test]
fn count_asked__an_empty_word_where_one_is_the_least() {
    holds("ws_rows", "countAsked", "an empty word where one is the least", d::count_asked);
}

#[test]
fn count_asked__a_blank_word() {
    holds("ws_rows", "countAsked", "a blank word", d::count_asked);
}

#[test]
fn count_asked__not_a_number() {
    holds("ws_rows", "countAsked", "not a number", d::count_asked);
}

#[test]
fn count_asked__a_number_with_a_unit() {
    holds("ws_rows", "countAsked", "a number with a unit", d::count_asked);
}

#[test]
fn count_asked__a_plus_sign() {
    holds("ws_rows", "countAsked", "a plus sign", d::count_asked);
}

#[test]
fn count_asked__infinity() {
    holds("ws_rows", "countAsked", "infinity", d::count_asked);
}

#[test]
fn count_asked__a_number_given_as_a_number() {
    holds("ws_rows", "countAsked", "a number given as a number", d::count_asked);
}

#[test]
fn count_asked__a_fraction_given_as_a_number() {
    holds("ws_rows", "countAsked", "a fraction given as a number", d::count_asked);
}

#[test]
fn count_asked__zero_given_as_a_number_where_one_is_the_least() {
    holds("ws_rows", "countAsked", "zero given as a number where one is the least", d::count_asked);
}

#[test]
fn count_asked__a_word_with_quotes_and_an_emoji() {
    holds("ws_rows", "countAsked", "a word with quotes and an emoji", d::count_asked);
}

#[test]
fn count_asked__a_computer_names_the_flag() {
    holds("ws_rows", "countAsked", "a computer names the flag", d::count_asked);
}

#[test]
fn count_asked__a_computer_names_the_depth_flag() {
    holds("ws_rows", "countAsked", "a computer names the depth flag", d::count_asked);
}

#[test]
fn count_asked__a_big_whole_number() {
    holds("ws_rows", "countAsked", "a big whole number", d::count_asked);
}

#[test]
fn count_asked__a_leading_zero() {
    holds("ws_rows", "countAsked", "a leading zero", d::count_asked);
}

#[test]
fn count_asked__an_underscore_is_not_a_digit_separator() {
    holds("ws_rows", "countAsked", "an underscore is not a digit separator", d::count_asked);
}

#[test]
fn count_asked__arabic_digits_are_not_digits() {
    holds("ws_rows", "countAsked", "arabic digits are not digits", d::count_asked);
}

#[test]
fn count_asked__an_exponent_past_the_double_range_is_infinity() {
    holds("ws_rows", "countAsked", "an exponent past the double range is infinity", d::count_asked);
}

#[test]
fn count_asked__minus_zero_is_zero() {
    holds("ws_rows", "countAsked", "minus zero is zero", d::count_asked);
}

#[test]
fn count_asked__a_leading_dot_with_an_exponent() {
    holds("ws_rows", "countAsked", "a leading dot with an exponent", d::count_asked);
}

#[test]
fn count_asked__a_trailing_dot() {
    holds("ws_rows", "countAsked", "a trailing dot", d::count_asked);
}

#[test]
fn count_asked__a_binary_word() {
    holds("ws_rows", "countAsked", "a binary word", d::count_asked);
}

#[test]
fn count_asked__an_octal_word() {
    holds("ws_rows", "countAsked", "an octal word", d::count_asked);
}

#[test]
fn count_asked__tabs_and_newlines_round_a_number() {
    holds("ws_rows", "countAsked", "tabs and newlines round a number", d::count_asked);
}

#[test]
fn count_asked__a_comma() {
    holds("ws_rows", "countAsked", "a comma", d::count_asked);
}

#[test]
fn count_asked__a_bare_hex_prefix() {
    holds("ws_rows", "countAsked", "a bare hex prefix", d::count_asked);
}

#[test]
fn count_asked__a_signed_hex_word() {
    holds("ws_rows", "countAsked", "a signed hex word", d::count_asked);
}

#[test]
fn count_asked__a_minus_where_zero_is_the_least() {
    holds("ws_rows", "countAsked", "a minus where zero is the least", d::count_asked);
}

#[test]
fn count_asked__a_number_as_a_number_with_a_fraction_that_is_whole() {
    holds("ws_rows", "countAsked", "a number as a number with a fraction that is whole", d::count_asked);
}

#[test]
fn size_chosen__an_offered_size() {
    holds("ws_rows", "sizeChosen", "an offered size", d::size_chosen);
}

#[test]
fn size_chosen__the_second_offered_size() {
    holds("ws_rows", "sizeChosen", "the second offered size", d::size_chosen);
}

#[test]
fn size_chosen__a_size_in_gigabytes_with_a_fraction() {
    holds("ws_rows", "sizeChosen", "a size in gigabytes with a fraction", d::size_chosen);
}

#[test]
fn size_chosen__a_size_of_half_a_gigabyte() {
    holds("ws_rows", "sizeChosen", "a size of half a gigabyte", d::size_chosen);
}

#[test]
fn size_chosen__white_space_round_the_word() {
    holds("ws_rows", "sizeChosen", "white space round the word", d::size_chosen);
}

#[test]
fn size_chosen__a_size_not_offered() {
    holds("ws_rows", "sizeChosen", "a size not offered", d::size_chosen);
}

#[test]
fn size_chosen__a_word_that_is_no_size() {
    holds("ws_rows", "sizeChosen", "a word that is no size", d::size_chosen);
}

#[test]
fn size_chosen__a_size_of_zero_cpus() {
    holds("ws_rows", "sizeChosen", "a size of zero cpus", d::size_chosen);
}

#[test]
fn size_chosen__a_size_of_zero_memory() {
    holds("ws_rows", "sizeChosen", "a size of zero memory", d::size_chosen);
}

#[test]
fn size_chosen__a_size_with_a_capital_x() {
    holds("ws_rows", "sizeChosen", "a size with a capital x", d::size_chosen);
}

#[test]
fn size_chosen__an_empty_word() {
    holds("ws_rows", "sizeChosen", "an empty word", d::size_chosen);
}

#[test]
fn size_chosen__no_sizes_offered() {
    holds("ws_rows", "sizeChosen", "no sizes offered", d::size_chosen);
}

#[test]
fn size_chosen__a_named_offer() {
    holds("ws_rows", "sizeChosen", "a named offer", d::size_chosen);
}

#[test]
fn size_chosen__a_rate_in_cents_and_a_rate_in_thousandths() {
    holds("ws_rows", "sizeChosen", "a rate in cents and a rate in thousandths", d::size_chosen);
}

#[test]
fn size_chosen__a_memory_that_rounds() {
    holds("ws_rows", "sizeChosen", "a memory that rounds", d::size_chosen);
}

#[test]
fn ssh_asked__nothing() {
    holds("ws_rows", "sshAsked", "nothing", d::ssh_asked);
}

#[test]
fn ssh_asked__a_name() {
    holds("ws_rows", "sshAsked", "a name", d::ssh_asked);
}

#[test]
fn ssh_asked__an_empty_name_is_kept() {
    holds("ws_rows", "sshAsked", "an empty name is kept", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port() {
    holds("ws_rows", "sshAsked", "a port", d::ssh_asked);
}

#[test]
fn ssh_asked__the_lowest_port() {
    holds("ws_rows", "sshAsked", "the lowest port", d::ssh_asked);
}

#[test]
fn ssh_asked__the_highest_port() {
    holds("ws_rows", "sshAsked", "the highest port", d::ssh_asked);
}

#[test]
fn ssh_asked__one_past_the_highest_port() {
    holds("ws_rows", "sshAsked", "one past the highest port", d::ssh_asked);
}

#[test]
fn ssh_asked__port_zero() {
    holds("ws_rows", "sshAsked", "port zero", d::ssh_asked);
}

#[test]
fn ssh_asked__a_negative_port() {
    holds("ws_rows", "sshAsked", "a negative port", d::ssh_asked);
}

#[test]
fn ssh_asked__a_fraction() {
    holds("ws_rows", "sshAsked", "a fraction", d::ssh_asked);
}

#[test]
fn ssh_asked__not_a_number() {
    holds("ws_rows", "sshAsked", "not a number", d::ssh_asked);
}

#[test]
fn ssh_asked__an_empty_port_reads_as_zero() {
    holds("ws_rows", "sshAsked", "an empty port reads as zero", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_with_white_space_round_it() {
    holds("ws_rows", "sshAsked", "a port with white space round it", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_in_exponent_form() {
    holds("ws_rows", "sshAsked", "a port in exponent form", d::ssh_asked);
}

#[test]
fn ssh_asked__a_binary_port() {
    holds("ws_rows", "sshAsked", "a binary port", d::ssh_asked);
}

#[test]
fn ssh_asked__an_exponent_port_of_eighty() {
    holds("ws_rows", "sshAsked", "an exponent port of eighty", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_with_a_leading_plus() {
    holds("ws_rows", "sshAsked", "a port with a leading plus", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_with_tabs_round_it() {
    holds("ws_rows", "sshAsked", "a port with tabs round it", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_past_the_exponent_range() {
    holds("ws_rows", "sshAsked", "a port past the exponent range", d::ssh_asked);
}

#[test]
fn ssh_asked__a_hex_port() {
    holds("ws_rows", "sshAsked", "a hex port", d::ssh_asked);
}

#[test]
fn ssh_asked__a_port_with_quotes_and_an_emoji() {
    holds("ws_rows", "sshAsked", "a port with quotes and an emoji", d::ssh_asked);
}

#[test]
fn ssh_asked__an_absolute_key() {
    holds("ws_rows", "sshAsked", "an absolute key", d::ssh_asked);
}

#[test]
fn ssh_asked__a_relative_key_is_refused() {
    holds("ws_rows", "sshAsked", "a relative key is refused", d::ssh_asked);
}

#[test]
fn ssh_asked__a_key_that_starts_with_a_tilde_is_refused() {
    holds("ws_rows", "sshAsked", "a key that starts with a tilde is refused", d::ssh_asked);
}

#[test]
fn ssh_asked__an_empty_key_is_refused() {
    holds("ws_rows", "sshAsked", "an empty key is refused", d::ssh_asked);
}

#[test]
fn ssh_asked__all_three_in_the_order_name_port_key() {
    holds("ws_rows", "sshAsked", "all three in the order name, port, key", d::ssh_asked);
}

#[test]
fn ssh_asked__a_bad_port_is_read_before_the_key() {
    holds("ws_rows", "sshAsked", "a bad port is read before the key", d::ssh_asked);
}

#[test]
fn ssh_asked__a_good_port_and_a_bad_key() {
    holds("ws_rows", "sshAsked", "a good port and a bad key", d::ssh_asked);
}

#[test]
fn const_folder_width__the_folder_width() {
    holds("ws_rows", "constFolderWidth", "the folder width", d::const_folder_width);
}

#[test]
fn const_title_width__the_title_width() {
    holds("ws_rows", "constTitleWidth", "the title width", d::const_title_width);
}
