// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from ws_life.json: one test per recorded case.
use super::common::holds;
use super::drivers_ws_life as d;

#[test]
fn rename_workspace__renamed() {
    holds("ws_life", "renameWorkspace", "renamed", d::rename_workspace);
}

#[test]
fn rename_workspace__the_rename_is_sent_by_id_with_the_name_as_typed() {
    holds("ws_life", "renameWorkspace", "the rename is sent by id with the name as typed", d::rename_workspace);
}

#[test]
fn rename_workspace__the_old_name_is_the_resolved_one_not_the_word_typed() {
    holds("ws_life", "renameWorkspace", "the old name is the resolved one, not the word typed", d::rename_workspace);
}

#[test]
fn rename_workspace__a_blank_name_is_the_runtime_s_refusal() {
    holds("ws_life", "renameWorkspace", "a blank name is the runtime's refusal", d::rename_workspace);
}

#[test]
fn rename_workspace__a_name_another_workspace_carries() {
    holds("ws_life", "renameWorkspace", "a name another workspace carries", d::rename_workspace);
}

#[test]
fn rename_workspace__no_such_workspace_asks_no_rename() {
    holds("ws_life", "renameWorkspace", "no such workspace asks no rename", d::rename_workspace);
}

#[test]
fn rename_workspace__a_record_this_build_cannot_read_asks_no_rename() {
    holds("ws_life", "renameWorkspace", "a record this build cannot read asks no rename", d::rename_workspace);
}

#[test]
fn rename_workspace__the_rename_refused_by_the_host() {
    holds("ws_life", "renameWorkspace", "the rename refused by the host", d::rename_workspace);
}

#[test]
fn renamed_workspace_line__a_rename() {
    holds("ws_life", "renamedWorkspaceLine", "a rename", d::renamed_workspace_line);
}

#[test]
fn renamed_workspace_line__names_with_spaces_and_quotes() {
    holds("ws_life", "renamedWorkspaceLine", "names with spaces and quotes", d::renamed_workspace_line);
}

#[test]
fn renamed_workspace_line__an_empty_old_name() {
    holds("ws_life", "renamedWorkspaceLine", "an empty old name", d::renamed_workspace_line);
}

#[test]
fn rename_workspace__renamed_cloud() {
    holds("ws_life", "renameWorkspace", "renamed (cloud)", d::rename_workspace);
}

#[test]
fn rename_workspace__the_rename_refused_by_the_host_cloud() {
    holds("ws_life", "renameWorkspace", "the rename refused by the host (cloud)", d::rename_workspace);
}

#[test]
fn awake__running_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "running for send, the wake answers running", d::awake);
}

#[test]
fn awake__running_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "running for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__napping_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "napping for send, the wake answers running", d::awake);
}

#[test]
fn awake__napping_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "napping for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__pausing_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "pausing for send, the wake answers running", d::awake);
}

#[test]
fn awake__pausing_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "pausing for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__waking_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "waking for send, the wake answers running", d::awake);
}

#[test]
fn awake__waking_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "waking for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__queued_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "queued for send, the wake answers running", d::awake);
}

#[test]
fn awake__queued_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "queued for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__released_for_send_the_wake_answers_running() {
    holds("ws_life", "awake", "released for send, the wake answers running", d::awake);
}

#[test]
fn awake__released_for_merge_in_the_wake_answers_running() {
    holds("ws_life", "awake", "released for merge in, the wake answers running", d::awake);
}

#[test]
fn awake__napping_the_wake_leaves_it_napping() {
    holds("ws_life", "awake", "napping, the wake leaves it napping", d::awake);
}

#[test]
fn awake__napping_the_wake_leaves_it_waking() {
    holds("ws_life", "awake", "napping, the wake leaves it waking", d::awake);
}

#[test]
fn awake__napping_with_its_disk_gone_the_wake_answers_running() {
    holds("ws_life", "awake", "napping with its disk gone, the wake answers running", d::awake);
}

#[test]
fn awake__pausing_the_wake_leaves_it_napping() {
    holds("ws_life", "awake", "pausing, the wake leaves it napping", d::awake);
}

#[test]
fn awake__pausing_the_wake_leaves_it_waking() {
    holds("ws_life", "awake", "pausing, the wake leaves it waking", d::awake);
}

#[test]
fn awake__pausing_with_its_disk_gone_the_wake_answers_running() {
    holds("ws_life", "awake", "pausing with its disk gone, the wake answers running", d::awake);
}

#[test]
fn awake__waking_the_wake_leaves_it_napping() {
    holds("ws_life", "awake", "waking, the wake leaves it napping", d::awake);
}

#[test]
fn awake__waking_the_wake_leaves_it_waking() {
    holds("ws_life", "awake", "waking, the wake leaves it waking", d::awake);
}

#[test]
fn awake__waking_with_its_disk_gone_the_wake_answers_running() {
    holds("ws_life", "awake", "waking with its disk gone, the wake answers running", d::awake);
}

#[test]
fn awake__running_the_wake_leaves_it_napping() {
    holds("ws_life", "awake", "running, the wake leaves it napping", d::awake);
}

#[test]
fn awake__running_the_wake_leaves_it_waking() {
    holds("ws_life", "awake", "running, the wake leaves it waking", d::awake);
}

#[test]
fn awake__running_with_its_disk_gone_the_wake_answers_running() {
    holds("ws_life", "awake", "running with its disk gone, the wake answers running", d::awake);
}

#[test]
fn awake__napping_with_its_disk_gone_the_wake_leaves_it_napping() {
    holds("ws_life", "awake", "napping with its disk gone, the wake leaves it napping", d::awake);
}

#[test]
fn awake__napping_with_an_empty_disk_line() {
    holds("ws_life", "awake", "napping with an empty disk line", d::awake);
}

#[test]
fn awake__the_wake_answers_the_record_the_host_wrote() {
    holds("ws_life", "awake", "the wake answers the record the host wrote", d::awake);
}

#[test]
fn awake__the_wake_is_asked_by_the_workspace_id() {
    holds("ws_life", "awake", "the wake is asked by the workspace id", d::awake);
}

#[test]
fn awake__gone_with_the_provider_s_words() {
    holds("ws_life", "awake", "gone, with the provider's words", d::awake);
}

#[test]
fn awake__gone_with_no_words() {
    holds("ws_life", "awake", "gone, with no words", d::awake);
}

#[test]
fn awake__gone_with_empty_words() {
    holds("ws_life", "awake", "gone, with empty words", d::awake);
}

#[test]
fn awake__gone_a_name_with_spaces_and_an_empty_action() {
    holds("ws_life", "awake", "gone, a name with spaces and an empty action", d::awake);
}

#[test]
fn awake__napping_the_wake_refused_by_the_host() {
    holds("ws_life", "awake", "napping, the wake refused by the host", d::awake);
}

#[test]
fn awake__running_the_wake_refused_by_the_host() {
    holds("ws_life", "awake", "running, the wake refused by the host", d::awake);
}

#[test]
fn awake__napping_the_host_closes_during_the_wake() {
    holds("ws_life", "awake", "napping, the host closes during the wake", d::awake);
}

#[test]
fn awake__napping_the_host_stops_during_the_wake() {
    holds("ws_life", "awake", "napping, the host stops during the wake", d::awake);
}

#[test]
fn awake__a_memory_pause_workspace_says_the_same_words() {
    holds("ws_life", "awake", "a memory-pause workspace says the same words", d::awake);
}

#[test]
fn awake__running_for_send_the_wake_answers_running_cloud() {
    holds("ws_life", "awake", "running for send, the wake answers running (cloud)", d::awake);
}

#[test]
fn awake__gone_with_the_provider_s_words_cloud() {
    holds("ws_life", "awake", "gone, with the provider's words (cloud)", d::awake);
}

#[test]
fn awake__napping_the_wake_refused_by_the_host_cloud() {
    holds("ws_life", "awake", "napping, the wake refused by the host (cloud)", d::awake);
}

#[test]
fn awake__running_the_wake_refused_by_the_host_cloud() {
    holds("ws_life", "awake", "running, the wake refused by the host (cloud)", d::awake);
}

#[test]
fn no_draft_line__a_note() {
    holds("ws_life", "noDraftLine", "a note", d::no_draft_line);
}

#[test]
fn no_draft_line__no_note() {
    holds("ws_life", "noDraftLine", "no note", d::no_draft_line);
}

#[test]
fn no_draft_line__an_empty_note() {
    holds("ws_life", "noDraftLine", "an empty note", d::no_draft_line);
}

#[test]
fn const_no_draft_fix__the_sentence() {
    holds("ws_life", "const_no_draft_fix", "the sentence", d::const_no_draft_fix);
}

#[test]
fn no_draft_line__a_note_cloud() {
    holds("ws_life", "noDraftLine", "a note (cloud)", d::no_draft_line);
}

#[test]
fn committed__a_message_given_every_change() {
    holds("ws_life", "committed", "a message given, every change", d::committed);
}

#[test]
fn committed__a_message_given_with_the_files_named() {
    holds("ws_life", "committed", "a message given with the files named", d::committed);
}

#[test]
fn committed__a_message_given_a_thread_named() {
    holds("ws_life", "committed", "a message given, a thread named", d::committed);
}

#[test]
fn committed__a_message_the_files_and_a_thread_in_the_order_sent() {
    holds("ws_life", "committed", "a message, the files and a thread, in the order sent", d::committed);
}

#[test]
fn committed__an_empty_message_is_a_message_no_draft_is_asked() {
    holds("ws_life", "committed", "an empty message is a message, no draft is asked", d::committed);
}

#[test]
fn committed__a_multi_line_message_is_sent_whole() {
    holds("ws_life", "committed", "a multi-line message is sent whole", d::committed);
}

#[test]
fn committed__no_message_the_draft_is_said_and_used() {
    holds("ws_life", "committed", "no message, the draft is said and used", d::committed);
}

#[test]
fn committed__no_message_the_files_and_a_thread_go_to_the_draft_too() {
    holds("ws_life", "committed", "no message, the files and a thread go to the draft too", d::committed);
}

#[test]
fn committed__no_message_an_empty_draft_is_still_a_draft() {
    holds("ws_life", "committed", "no message, an empty draft is still a draft", d::committed);
}

#[test]
fn committed__no_message_a_draft_with_a_note_the_note_is_not_said() {
    holds("ws_life", "committed", "no message, a draft with a note, the note is not said", d::committed);
}

#[test]
fn committed__no_message_no_draft_with_the_note() {
    holds("ws_life", "committed", "no message, no draft, with the note", d::committed);
}

#[test]
fn committed__no_message_no_draft_no_note() {
    holds("ws_life", "committed", "no message, no draft, no note", d::committed);
}

#[test]
fn committed__no_message_no_draft_an_empty_note() {
    holds("ws_life", "committed", "no message, no draft, an empty note", d::committed);
}

#[test]
fn committed__the_draft_refused_by_the_host() {
    holds("ws_life", "committed", "the draft refused by the host", d::committed);
}

#[test]
fn committed__the_commit_refused_by_a_hook() {
    holds("ws_life", "committed", "the commit refused by a hook", d::committed);
}

#[test]
fn committed__the_commit_refused_after_a_draft_was_said() {
    holds("ws_life", "committed", "the commit refused after a draft was said", d::committed);
}

#[test]
fn committed__the_reply_s_unknown_keys_are_dropped() {
    holds("ws_life", "committed", "the reply's unknown keys are dropped", d::committed);
}

#[test]
fn committed__the_host_closes_during_the_commit() {
    holds("ws_life", "committed", "the host closes during the commit", d::committed);
}

#[test]
fn committed__a_message_given_every_change_cloud() {
    holds("ws_life", "committed", "a message given, every change (cloud)", d::committed);
}

#[test]
fn committed__the_draft_refused_by_the_host_cloud() {
    holds("ws_life", "committed", "the draft refused by the host (cloud)", d::committed);
}

#[test]
fn asked_to_fix__a_failed_check() {
    holds("ws_life", "askedToFix", "a failed check", d::asked_to_fix);
}

#[test]
fn asked_to_fix__the_conflicts_no_check() {
    holds("ws_life", "askedToFix", "the conflicts, no check", d::asked_to_fix);
}

#[test]
fn asked_to_fix__a_child_to_merge() {
    holds("ws_life", "askedToFix", "a child to merge", d::asked_to_fix);
}

#[test]
fn asked_to_fix__a_check_and_a_child_in_the_order_sent() {
    holds("ws_life", "askedToFix", "a check and a child, in the order sent", d::asked_to_fix);
}

#[test]
fn asked_to_fix__an_empty_check_is_sent() {
    holds("ws_life", "askedToFix", "an empty check is sent", d::asked_to_fix);
}

#[test]
fn asked_to_fix__held_by_its_computer_s_threads() {
    holds("ws_life", "askedToFix", "held by its computer's threads", d::asked_to_fix);
}

#[test]
fn asked_to_fix__updated_clean() {
    holds("ws_life", "askedToFix", "updated clean", d::asked_to_fix);
}

#[test]
fn asked_to_fix__an_unknown_key_is_dropped() {
    holds("ws_life", "askedToFix", "an unknown key is dropped", d::asked_to_fix);
}

#[test]
fn asked_to_fix__a_sent_answer_naming_no_agent_is_another_version() {
    holds("ws_life", "askedToFix", "a sent answer naming no agent is another version", d::asked_to_fix);
}

#[test]
fn asked_to_fix__an_outcome_this_build_does_not_know_is_another_version() {
    holds("ws_life", "askedToFix", "an outcome this build does not know is another version", d::asked_to_fix);
}

#[test]
fn asked_to_fix__an_answer_with_no_outcome_is_another_version() {
    holds("ws_life", "askedToFix", "an answer with no outcome is another version", d::asked_to_fix);
}

#[test]
fn asked_to_fix__refused_by_the_host() {
    holds("ws_life", "askedToFix", "refused by the host", d::asked_to_fix);
}

#[test]
fn asked_to_fix__a_failed_check_cloud() {
    holds("ws_life", "askedToFix", "a failed check (cloud)", d::asked_to_fix);
}

#[test]
fn asked_to_fix__refused_by_the_host_cloud() {
    holds("ws_life", "askedToFix", "refused by the host (cloud)", d::asked_to_fix);
}

#[test]
fn fix_line__updated() {
    holds("ws_life", "fixLine", "updated", d::fix_line);
}

#[test]
fn fix_line__updated_with_a_child_named_which_is_ignored() {
    holds("ws_life", "fixLine", "updated with a child named, which is ignored", d::fix_line);
}

#[test]
fn fix_line__steered_on_a_check() {
    holds("ws_life", "fixLine", "steered on a check", d::fix_line);
}

#[test]
fn fix_line__started_on_the_conflicts() {
    holds("ws_life", "fixLine", "started on the conflicts", d::fix_line);
}

#[test]
fn fix_line__queued_on_the_conflicts() {
    holds("ws_life", "fixLine", "queued on the conflicts", d::fix_line);
}

#[test]
fn fix_line__a_child_named_by_its_label() {
    holds("ws_life", "fixLine", "a child, named by its label", d::fix_line);
}

#[test]
fn fix_line__a_child_with_no_label_named_by_its_workspace_id() {
    holds("ws_life", "fixLine", "a child with no label, named by its workspace id", d::fix_line);
}

#[test]
fn fix_line__a_child_and_a_check_the_child_wins() {
    holds("ws_life", "fixLine", "a child and a check, the child wins", d::fix_line);
}

#[test]
fn fix_line__an_agent_the_catalog_does_not_name() {
    holds("ws_life", "fixLine", "an agent the catalog does not name", d::fix_line);
}

#[test]
fn fix_line__held_on_a_check_the_wait_under_it() {
    holds("ws_life", "fixLine", "held on a check, the wait under it", d::fix_line);
}

#[test]
fn fix_line__held_on_the_conflicts_a_slot_wait() {
    holds("ws_life", "fixLine", "held on the conflicts, a slot wait", d::fix_line);
}

#[test]
fn fix_line__held_on_a_child_a_minute_wait() {
    holds("ws_life", "fixLine", "held on a child, a minute wait", d::fix_line);
}

#[test]
fn fix_line__held_with_no_wait_stated() {
    holds("ws_life", "fixLine", "held with no wait stated", d::fix_line);
}

#[test]
fn fix_line__started_with_a_wait_stated_which_is_not_said() {
    holds("ws_life", "fixLine", "started with a wait stated, which is not said", d::fix_line);
}

#[test]
fn fix_line__names_with_spaces_and_an_emoji() {
    holds("ws_life", "fixLine", "names with spaces and an emoji", d::fix_line);
}

#[test]
fn fix_line__updated_cloud() {
    holds("ws_life", "fixLine", "updated (cloud)", d::fix_line);
}

#[test]
fn merged_in__a_running_lead() {
    holds("ws_life", "mergedIn", "a running lead", d::merged_in);
}

#[test]
fn merged_in__a_napping_lead_is_woken_first_and_the_merge_goes_to_the_woke() {
    holds("ws_life", "mergedIn", "a napping lead is woken first, and the merge goes to the woken record's id", d::merged_in);
}

#[test]
fn merged_in__a_pausing_lead_is_woken() {
    holds("ws_life", "mergedIn", "a pausing lead is woken", d::merged_in);
}

#[test]
fn merged_in__a_lead_with_its_disk_gone_says_the_line_and_merges() {
    holds("ws_life", "mergedIn", "a lead with its disk gone says the line and merges", d::merged_in);
}

#[test]
fn merged_in__a_released_lead_is_woken_as_any_action_but_send_is() {
    holds("ws_life", "mergedIn", "a released lead is woken, as any action but send is", d::merged_in);
}

#[test]
fn merged_in__a_gone_lead_is_refused_before_any_frame() {
    holds("ws_life", "mergedIn", "a gone lead is refused before any frame", d::merged_in);
}

#[test]
fn merged_in__conflicts() {
    holds("ws_life", "mergedIn", "conflicts", d::merged_in);
}

#[test]
fn merged_in__a_sub_thread_s_commits_not_pushed() {
    holds("ws_life", "mergedIn", "a sub-thread's commits not pushed", d::merged_in);
}

#[test]
fn merged_in__nothing_to_take() {
    holds("ws_life", "mergedIn", "nothing to take", d::merged_in);
}

#[test]
fn merged_in__the_merge_refused_by_the_host() {
    holds("ws_life", "mergedIn", "the merge refused by the host", d::merged_in);
}

#[test]
fn merged_in__the_wake_refused_by_the_host_asks_no_merge() {
    holds("ws_life", "mergedIn", "the wake refused by the host asks no merge", d::merged_in);
}

#[test]
fn merged_in__a_running_lead_cloud() {
    holds("ws_life", "mergedIn", "a running lead (cloud)", d::merged_in);
}

#[test]
fn merged_in__the_merge_refused_by_the_host_cloud() {
    holds("ws_life", "mergedIn", "the merge refused by the host (cloud)", d::merged_in);
}

#[test]
fn merged_in__the_wake_refused_by_the_host_asks_no_merge_cloud() {
    holds("ws_life", "mergedIn", "the wake refused by the host asks no merge (cloud)", d::merged_in);
}

#[test]
fn started_from__a_link_alone() {
    holds("ws_life", "startedFrom", "a link alone", d::started_from);
}

#[test]
fn started_from__every_pick_in_the_order_of_the_line() {
    holds("ws_life", "startedFrom", "every pick, in the order of the line", d::started_from);
}

#[test]
fn started_from__an_absent_pick_is_left_out_of_the_frame() {
    holds("ws_life", "startedFrom", "an absent pick is left out of the frame", d::started_from);
}

#[test]
fn started_from__access_ask() {
    holds("ws_life", "startedFrom", "access ask", d::started_from);
}

#[test]
fn started_from__access_auto_edit() {
    holds("ws_life", "startedFrom", "access auto-edit", d::started_from);
}

#[test]
fn started_from__access_plan() {
    holds("ws_life", "startedFrom", "access plan", d::started_from);
}

#[test]
fn started_from__an_access_that_is_no_word_is_refused_before_any_frame() {
    holds("ws_life", "startedFrom", "an access that is no word is refused before any frame", d::started_from);
}

#[test]
fn started_from__an_empty_access_is_no_word() {
    holds("ws_life", "startedFrom", "an empty access is no word", d::started_from);
}

#[test]
fn started_from__an_access_in_capitals_is_no_word() {
    holds("ws_life", "startedFrom", "an access in capitals is no word", d::started_from);
}

#[test]
fn started_from__empty_picks_are_sent() {
    holds("ws_life", "startedFrom", "empty picks are sent", d::started_from);
}

#[test]
fn started_from__an_answer_with_no_origin() {
    holds("ws_life", "startedFrom", "an answer with no origin", d::started_from);
}

#[test]
fn started_from__only_the_three_fields_of_the_answer_are_kept() {
    holds("ws_life", "startedFrom", "only the three fields of the answer are kept", d::started_from);
}

#[test]
fn started_from__refused_by_the_host() {
    holds("ws_life", "startedFrom", "refused by the host", d::started_from);
}

#[test]
fn review_started__a_link() {
    holds("ws_life", "reviewStarted", "a link", d::review_started);
}

#[test]
fn review_started__a_workspace_s_own_pull_request() {
    holds("ws_life", "reviewStarted", "a workspace's own pull request", d::review_started);
}

#[test]
fn review_started__every_pick_in_the_order_of_the_line() {
    holds("ws_life", "reviewStarted", "every pick, in the order of the line", d::review_started);
}

#[test]
fn review_started__a_workspace_and_the_picks() {
    holds("ws_life", "reviewStarted", "a workspace and the picks", d::review_started);
}

#[test]
fn review_started__nothing_named() {
    holds("ws_life", "reviewStarted", "nothing named", d::review_started);
}

#[test]
fn review_started__an_agent_with_no_read_only_access() {
    holds("ws_life", "reviewStarted", "an agent with no read-only access", d::review_started);
}

#[test]
fn review_posted__no_edits_posted_as_drafted() {
    holds("ws_life", "reviewPosted", "no edits, posted as drafted", d::review_posted);
}

#[test]
fn review_posted__a_verdict_edited_first() {
    holds("ws_life", "reviewPosted", "a verdict edited first", d::review_posted);
}

#[test]
fn review_posted__a_summary_edited_first() {
    holds("ws_life", "reviewPosted", "a summary edited first", d::review_posted);
}

#[test]
fn review_posted__an_empty_summary_is_an_edit() {
    holds("ws_life", "reviewPosted", "an empty summary is an edit", d::review_posted);
}

#[test]
fn review_posted__the_verdict_and_the_summary_in_the_order_given() {
    holds("ws_life", "reviewPosted", "the verdict and the summary, in the order given", d::review_posted);
}

#[test]
fn review_posted__no_review_yet() {
    holds("ws_life", "reviewPosted", "no review yet", d::review_posted);
}

#[test]
fn review_posted__the_draft_edit_refused_asks_no_post() {
    holds("ws_life", "reviewPosted", "the draft edit refused asks no post", d::review_posted);
}

#[test]
fn review_posted__an_unknown_key_of_the_answer_is_dropped() {
    holds("ws_life", "reviewPosted", "an unknown key of the answer is dropped", d::review_posted);
}

#[test]
fn started_from__a_link_alone_cloud() {
    holds("ws_life", "startedFrom", "a link alone (cloud)", d::started_from);
}

#[test]
fn started_from__refused_by_the_host_cloud() {
    holds("ws_life", "startedFrom", "refused by the host (cloud)", d::started_from);
}

#[test]
fn started_lines__an_issue() {
    holds("ws_life", "startedLines", "an issue", d::started_lines);
}

#[test]
fn started_lines__an_issue_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "an issue, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__a_pull_request() {
    holds("ws_life", "startedLines", "a pull request", d::started_lines);
}

#[test]
fn started_lines__a_pull_request_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "a pull request, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__a_review() {
    holds("ws_life", "startedLines", "a review", d::started_lines);
}

#[test]
fn started_lines__a_review_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "a review, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__no_origin() {
    holds("ws_life", "startedLines", "no origin", d::started_lines);
}

#[test]
fn started_lines__no_origin_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "no origin, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__a_short_thread_id() {
    holds("ws_life", "startedLines", "a short thread id", d::started_lines);
}

#[test]
fn started_lines__a_short_thread_id_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "a short thread id, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__a_name_with_spaces_and_an_emoji() {
    holds("ws_life", "startedLines", "a name with spaces and an emoji", d::started_lines);
}

#[test]
fn started_lines__a_name_with_spaces_and_an_emoji_the_caller_s_own_thread_line() {
    holds("ws_life", "startedLines", "a name with spaces and an emoji, the caller's own thread line", d::started_lines);
}

#[test]
fn started_lines__an_issue_cloud() {
    holds("ws_life", "startedLines", "an issue (cloud)", d::started_lines);
}

#[test]
fn review_lines__an_issue() {
    holds("ws_life", "reviewLines", "an issue", d::review_lines);
}

#[test]
fn review_lines__an_issue_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "an issue, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__a_pull_request() {
    holds("ws_life", "reviewLines", "a pull request", d::review_lines);
}

#[test]
fn review_lines__a_pull_request_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "a pull request, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__a_review() {
    holds("ws_life", "reviewLines", "a review", d::review_lines);
}

#[test]
fn review_lines__a_review_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "a review, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__no_origin() {
    holds("ws_life", "reviewLines", "no origin", d::review_lines);
}

#[test]
fn review_lines__no_origin_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "no origin, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__a_short_thread_id() {
    holds("ws_life", "reviewLines", "a short thread id", d::review_lines);
}

#[test]
fn review_lines__a_short_thread_id_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "a short thread id, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__a_name_with_spaces_and_an_emoji() {
    holds("ws_life", "reviewLines", "a name with spaces and an emoji", d::review_lines);
}

#[test]
fn review_lines__a_name_with_spaces_and_an_emoji_the_caller_s_own_thread_line() {
    holds("ws_life", "reviewLines", "a name with spaces and an emoji, the caller's own thread line", d::review_lines);
}

#[test]
fn review_lines__an_issue_cloud() {
    holds("ws_life", "reviewLines", "an issue (cloud)", d::review_lines);
}

#[test]
fn thread_label__a_thread_id() {
    holds("ws_life", "threadLabel", "a thread id", d::thread_label);
}

#[test]
fn thread_label__a_row_with_no_thread_id_is_named_by_its_row_id() {
    holds("ws_life", "threadLabel", "a row with no thread id is named by its row id", d::thread_label);
}

#[test]
fn thread_label__a_short_id() {
    holds("ws_life", "threadLabel", "a short id", d::thread_label);
}

#[test]
fn thread_label__an_id_of_exactly_8() {
    holds("ws_life", "threadLabel", "an id of exactly 8", d::thread_label);
}

#[test]
fn thread_label__an_id_of_9() {
    holds("ws_life", "threadLabel", "an id of 9", d::thread_label);
}

#[test]
fn thread_label__the_thread_id_wins_over_the_row_id() {
    holds("ws_life", "threadLabel", "the thread id wins over the row id", d::thread_label);
}

#[test]
fn thread_label__a_thread_id_cloud() {
    holds("ws_life", "threadLabel", "a thread id (cloud)", d::thread_label);
}

#[test]
fn thread_cwd__no_workspace_at_all() {
    holds("ws_life", "threadCwd", "no workspace at all", d::thread_cwd);
}

#[test]
fn thread_cwd__no_worktree() {
    holds("ws_life", "threadCwd", "no worktree", d::thread_cwd);
}

#[test]
fn thread_cwd__no_turn_folder() {
    holds("ws_life", "threadCwd", "no turn folder", d::thread_cwd);
}

#[test]
fn thread_cwd__no_turn_folder_under_a_gone_worktree() {
    holds("ws_life", "threadCwd", "no turn folder under a gone worktree", d::thread_cwd);
}

#[test]
fn thread_cwd__a_live_worktree_and_a_folder_inside_it() {
    holds("ws_life", "threadCwd", "a live worktree and a folder inside it", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_and_a_folder_inside_it() {
    holds("ws_life", "threadCwd", "a gone worktree and a folder inside it", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_and_its_own_folder() {
    holds("ws_life", "threadCwd", "a gone worktree and its own folder", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_and_a_sibling_that_shares_the_prefix() {
    holds("ws_life", "threadCwd", "a gone worktree and a sibling that shares the prefix", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_and_a_folder_elsewhere() {
    holds("ws_life", "threadCwd", "a gone worktree and a folder elsewhere", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_whose_path_ends_in_a_slash() {
    holds("ws_life", "threadCwd", "a gone worktree whose path ends in a slash", d::thread_cwd);
}

#[test]
fn thread_cwd__a_gone_worktree_whose_path_ends_in_a_slash_the_sibling() {
    holds("ws_life", "threadCwd", "a gone worktree whose path ends in a slash, the sibling", d::thread_cwd);
}

#[test]
fn thread_cwd__an_empty_turn_folder_under_a_gone_worktree() {
    holds("ws_life", "threadCwd", "an empty turn folder under a gone worktree", d::thread_cwd);
}

#[test]
fn thread_cwd__no_workspace_at_all_cloud() {
    holds("ws_life", "threadCwd", "no workspace at all (cloud)", d::thread_cwd);
}

#[test]
fn folder_of__the_workspace_folder_under_the_home() {
    holds("ws_life", "folderOf", "the workspace folder, under the home", d::folder_of);
}

#[test]
fn folder_of__the_home_itself() {
    holds("ws_life", "folderOf", "the home itself", d::folder_of);
}

#[test]
fn folder_of__a_sibling_of_the_home_sharing_its_prefix() {
    holds("ws_life", "folderOf", "a sibling of the home sharing its prefix", d::folder_of);
}

#[test]
fn folder_of__no_folder_the_project_path() {
    holds("ws_life", "folderOf", "no folder, the project path", d::folder_of);
}

#[test]
fn folder_of__outside_the_home() {
    holds("ws_life", "folderOf", "outside the home", d::folder_of);
}

#[test]
fn folder_of__a_home_with_a_trailing_slash() {
    holds("ws_life", "folderOf", "a home with a trailing slash", d::folder_of);
}

#[test]
fn folder_of__an_empty_folder_is_a_folder() {
    holds("ws_life", "folderOf", "an empty folder is a folder", d::folder_of);
}

#[test]
fn with_shared__none_share_it() {
    holds("ws_life", "withShared", "none share it", d::with_shared);
}

#[test]
fn with_shared__one_shares_it() {
    holds("ws_life", "withShared", "one shares it", d::with_shared);
}

#[test]
fn with_shared__two_share_it() {
    holds("ws_life", "withShared", "two share it", d::with_shared);
}

#[test]
fn with_shared__three_share_it_a_short_id_and_a_long_one() {
    holds("ws_life", "withShared", "three share it, a short id and a long one", d::with_shared);
}

#[test]
fn with_shared__a_multi_line_line() {
    holds("ws_life", "withShared", "a multi-line line", d::with_shared);
}

#[test]
fn shared_of__none() {
    holds("ws_life", "sharedOf", "none", d::shared_of);
}

#[test]
fn shared_of__one() {
    holds("ws_life", "sharedOf", "one", d::shared_of);
}

#[test]
fn shared_of__two_whole_and_in_order() {
    holds("ws_life", "sharedOf", "two, whole and in order", d::shared_of);
}

#[test]
fn const_verdicts__the_table() {
    holds("ws_life", "const_verdicts", "the table", d::const_verdicts);
}

#[test]
fn folder_of__the_workspace_folder_under_the_home_cloud() {
    holds("ws_life", "folderOf", "the workspace folder, under the home (cloud)", d::folder_of);
}

#[test]
fn merged_pr__no_method_not_on_checks() {
    holds("ws_life", "mergedPr", "no method, not on checks", d::merged_pr);
}

#[test]
fn merged_pr__a_method() {
    holds("ws_life", "mergedPr", "a method", d::merged_pr);
}

#[test]
fn merged_pr__squash() {
    holds("ws_life", "mergedPr", "squash", d::merged_pr);
}

#[test]
fn merged_pr__merge() {
    holds("ws_life", "mergedPr", "merge", d::merged_pr);
}

#[test]
fn merged_pr__when_checks_pass_armed() {
    holds("ws_life", "mergedPr", "when checks pass, armed", d::merged_pr);
}

#[test]
fn merged_pr__a_method_and_when_checks_pass_in_the_order_sent() {
    holds("ws_life", "mergedPr", "a method and when checks pass, in the order sent", d::merged_pr);
}

#[test]
fn merged_pr__an_unknown_key_of_the_answer_is_dropped() {
    holds("ws_life", "mergedPr", "an unknown key of the answer is dropped", d::merged_pr);
}

#[test]
fn merged_pr__refused_by_the_repository() {
    holds("ws_life", "mergedPr", "refused by the repository", d::merged_pr);
}

#[test]
fn update_line__merged_several_commits() {
    holds("ws_life", "updateLine", "merged, several commits", d::update_line);
}

#[test]
fn update_line__merged_one_commit() {
    holds("ws_life", "updateLine", "merged, one commit", d::update_line);
}

#[test]
fn update_line__already_current() {
    holds("ws_life", "updateLine", "already current", d::update_line);
}

#[test]
fn update_line__conflicts_two_files() {
    holds("ws_life", "updateLine", "conflicts, two files", d::update_line);
}

#[test]
fn update_line__conflicts_one_file() {
    holds("ws_life", "updateLine", "conflicts, one file", d::update_line);
}

#[test]
fn update_line__not_merged_and_no_files_named() {
    holds("ws_life", "updateLine", "not merged and no files named", d::update_line);
}

#[test]
fn update_line__merged_with_files_listed_which_are_not_said() {
    holds("ws_life", "updateLine", "merged, with files listed which are not said", d::update_line);
}

#[test]
fn update_line__names_with_spaces_and_an_emoji() {
    holds("ws_life", "updateLine", "names with spaces and an emoji", d::update_line);
}

#[test]
fn merged_pr__no_method_not_on_checks_cloud() {
    holds("ws_life", "mergedPr", "no method, not on checks (cloud)", d::merged_pr);
}

#[test]
fn nap_after_dead_launch__a_wake_this_call_did_not_make_owes_nothing() {
    holds("ws_life", "napAfterDeadLaunch", "a wake this call did not make owes nothing", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_turn_the_agent_refused_ran_so_nothing_is_owed() {
    holds("ws_life", "napAfterDeadLaunch", "a turn the agent refused ran, so nothing is owed", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__naps_it_where_nothing_else_is_running_there() {
    holds("ws_life", "napAfterDeadLaunch", "naps it where nothing else is running there", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__naps_it_for_a_turn_whose_thread_is_the_only_one() {
    holds("ws_life", "napAfterDeadLaunch", "naps it for a turn whose thread is the only one", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__naps_it_where_no_thread_is_listed_at_all() {
    holds("ws_life", "napAfterDeadLaunch", "naps it where no thread is listed at all", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__naps_it_where_no_turn_is_given_and_no_thread_is_listed() {
    holds("ws_life", "napAfterDeadLaunch", "naps it where no turn is given and no thread is listed", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__naps_it_where_the_turn_s_thread_is_not_listed_and_others_hav() {
    holds("ws_life", "napAfterDeadLaunch", "naps it where the turn's thread is not listed and others have ended", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_thread_whose_turn_did_work_is_not_a_dead_launch() {
    holds("ws_life", "napAfterDeadLaunch", "a thread whose turn did work is not a dead launch", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_thread_of_the_turn_that_is_still_running_did_work() {
    holds("ws_life", "napAfterDeadLaunch", "a thread of the turn that is still running did work", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_thread_that_never_reached_the_agent_is_a_dead_launch_even_() {
    holds(
        "ws_life",
        "napAfterDeadLaunch",
        "a thread that never reached the agent is a dead launch even beside a refused one",
        d::nap_after_dead_launch,
    );
}

#[test]
fn nap_after_dead_launch__leaves_it_up_where_another_thread_is_working_with_no_nap_tim() {
    holds(
        "ws_life",
        "napAfterDeadLaunch",
        "leaves it up where another thread is working, with no nap time to say",
        d::nap_after_dead_launch,
    );
}

#[test]
fn nap_after_dead_launch__leaves_it_up_with_a_nap_time_already_past_which_reads_as_non() {
    holds("ws_life", "napAfterDeadLaunch", "leaves it up with a nap time already past, which reads as none left", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__leaves_it_up_where_the_workspace_has_no_status_row() {
    holds("ws_life", "napAfterDeadLaunch", "leaves it up where the workspace has no status row", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__leaves_it_up_where_the_status_list_is_empty() {
    holds("ws_life", "napAfterDeadLaunch", "leaves it up where the status list is empty", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__leaves_it_up_where_the_status_list_is_refused() {
    holds("ws_life", "napAfterDeadLaunch", "leaves it up where the status list is refused", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__another_thread_working_counts_with_no_turn_given() {
    holds("ws_life", "napAfterDeadLaunch", "another thread working counts with no turn given", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__two_others_working() {
    holds("ws_life", "napAfterDeadLaunch", "two others working", d::nap_after_dead_launch);
}

#[test]
fn nap_after_dead_launch__a_turn_s_thread_listed_twice_is_one_thread_and_its_other_tur() {
    holds("ws_life", "napAfterDeadLaunch", "a turn's thread listed twice is one thread, and its other turn runs", d::nap_after_dead_launch);
}
