// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from ws_reads.json: one test per recorded case.
use super::common::holds;
use super::drivers_ws_reads as d;

#[test]
fn workspaces__none() {
    holds("ws_reads", "workspaces", "none", d::workspaces);
}

#[test]
fn workspaces__one() {
    holds("ws_reads", "workspaces", "one", d::workspaces);
}

#[test]
fn workspaces__three_in_the_order_the_host_gave() {
    holds("ws_reads", "workspaces", "three in the order the host gave", d::workspaces);
}

#[test]
fn workspaces__every_optional_field() {
    holds("ws_reads", "workspaces", "every optional field", d::workspaces);
}

#[test]
fn workspaces__refused_by_the_host() {
    holds("ws_reads", "workspaces", "refused by the host", d::workspaces);
}

#[test]
fn workspaces__refused_with_no_kind() {
    holds("ws_reads", "workspaces", "refused with no kind", d::workspaces);
}

#[test]
fn workspaces__the_host_closes_during_the_call() {
    holds("ws_reads", "workspaces", "the host closes during the call", d::workspaces);
}

#[test]
fn workspaces__the_host_is_stopping() {
    holds("ws_reads", "workspaces", "the host is stopping", d::workspaces);
}

#[test]
fn workspace_statuses__none() {
    holds("ws_reads", "workspaceStatuses", "none", d::workspace_statuses);
}

#[test]
fn workspace_statuses__one_running() {
    holds("ws_reads", "workspaceStatuses", "one running", d::workspace_statuses);
}

#[test]
fn workspace_statuses__paused_and_unreachable_in_the_host_s_order() {
    holds("ws_reads", "workspaceStatuses", "paused and unreachable in the host's order", d::workspace_statuses);
}

#[test]
fn workspace_statuses__an_idle_time_and_a_reason() {
    holds("ws_reads", "workspaceStatuses", "an idle time and a reason", d::workspace_statuses);
}

#[test]
fn workspace_statuses__refused_by_the_host() {
    holds("ws_reads", "workspaceStatuses", "refused by the host", d::workspace_statuses);
}

#[test]
fn workspaces__none_cloud() {
    holds("ws_reads", "workspaces", "none (cloud)", d::workspaces);
}

#[test]
fn workspaces__refused_by_the_host_cloud() {
    holds("ws_reads", "workspaces", "refused by the host (cloud)", d::workspaces);
}

#[test]
fn workspace_statuses__refused_by_the_host_cloud() {
    holds("ws_reads", "workspaceStatuses", "refused by the host (cloud)", d::workspace_statuses);
}

#[test]
fn threads__no_sessions_no_workspace_named() {
    holds("ws_reads", "threads", "no sessions, no workspace named", d::threads);
}

#[test]
fn threads__a_workspace_named() {
    holds("ws_reads", "threads", "a workspace named", d::threads);
}

#[test]
fn threads__an_empty_workspace_id_is_still_sent() {
    holds("ws_reads", "threads", "an empty workspace id is still sent", d::threads);
}

#[test]
fn threads__a_row_with_no_thread_id_is_its_own_thread() {
    holds("ws_reads", "threads", "a row with no thread id is its own thread", d::threads);
}

#[test]
fn threads__two_turns_of_one_thread_fold_into_one_row() {
    holds("ws_reads", "threads", "two turns of one thread fold into one row", d::threads);
}

#[test]
fn threads__threads_in_the_order_of_their_first_turn() {
    holds("ws_reads", "threads", "threads in the order of their first turn", d::threads);
}

#[test]
fn threads__the_title_is_the_harness_name_over_the_opening() {
    holds("ws_reads", "threads", "the title is the harness name over the opening", d::threads);
}

#[test]
fn threads__the_title_is_the_opening_s_first_sentence() {
    holds("ws_reads", "threads", "the title is the opening's first sentence", d::threads);
}

#[test]
fn threads__the_title_is_cut_at_47_characters_and_an_ellipsis() {
    holds("ws_reads", "threads", "the title is cut at 47 characters and an ellipsis", d::threads);
}

#[test]
fn threads__the_title_is_the_opening_at_exactly_48() {
    holds("ws_reads", "threads", "the title is the opening at exactly 48", d::threads);
}

#[test]
fn threads__the_title_is_the_opening_49_characters_on_one_word() {
    holds("ws_reads", "threads", "the title is the opening, 49 characters on one word", d::threads);
}

#[test]
fn threads__the_title_falls_to_the_agent_s_session_id() {
    holds("ws_reads", "threads", "the title falls to the agent's session id", d::threads);
}

#[test]
fn threads__the_title_falls_to_the_row_id() {
    holds("ws_reads", "threads", "the title falls to the row id", d::threads);
}

#[test]
fn threads__ran_is_true_for_a_session_the_agent_started() {
    holds("ws_reads", "threads", "ran is true for a session the agent started", d::threads);
}

#[test]
fn threads__ran_is_false_for_a_refused_turn() {
    holds("ws_reads", "threads", "ran is false for a refused turn", d::threads);
}

#[test]
fn threads__ran_is_true_while_a_turn_runs() {
    holds("ws_reads", "threads", "ran is true while a turn runs", d::threads);
}

#[test]
fn threads__started_by_an_agent_with_its_parents() {
    holds("ws_reads", "threads", "started by an agent, with its parents", d::threads);
}

#[test]
fn threads__the_first_turn_s_parents_win_over_a_later_one() {
    holds("ws_reads", "threads", "the first turn's parents win over a later one", d::threads);
}

#[test]
fn threads__cost_adds_up_across_turns() {
    holds("ws_reads", "threads", "cost adds up across turns", d::threads);
}

#[test]
fn threads__refused_by_the_host() {
    holds("ws_reads", "threads", "refused by the host", d::threads);
}

#[test]
fn threads__no_sessions_no_workspace_named_cloud() {
    holds("ws_reads", "threads", "no sessions, no workspace named (cloud)", d::threads);
}

#[test]
fn threads__refused_by_the_host_cloud() {
    holds("ws_reads", "threads", "refused by the host (cloud)", d::threads);
}

#[test]
fn threads__the_key_order_of_a_folded_row_with_asking_and_a_cost() {
    holds("ws_reads", "threads", "the key order of a folded row with asking and a cost", d::threads);
}

#[test]
fn thread_of__an_exact_id() {
    holds("ws_reads", "threadOf", "an exact id", d::thread_of);
}

#[test]
fn thread_of__a_prefix_naming_one() {
    holds("ws_reads", "threadOf", "a prefix naming one", d::thread_of);
}

#[test]
fn thread_of__a_prefix_naming_two() {
    holds("ws_reads", "threadOf", "a prefix naming two", d::thread_of);
}

#[test]
fn thread_of__a_prefix_naming_all_three() {
    holds("ws_reads", "threadOf", "a prefix naming all three", d::thread_of);
}

#[test]
fn thread_of__the_empty_word_names_every_thread() {
    holds("ws_reads", "threadOf", "the empty word names every thread", d::thread_of);
}

#[test]
fn thread_of__the_empty_word_with_one_thread_names_it() {
    holds("ws_reads", "threadOf", "the empty word with one thread names it", d::thread_of);
}

#[test]
fn thread_of__an_id_that_is_also_the_prefix_of_another_wins_whole() {
    holds("ws_reads", "threadOf", "an id that is also the prefix of another wins whole", d::thread_of);
}

#[test]
fn thread_of__a_prefix_of_a_longer_id_and_a_shorter_one() {
    holds("ws_reads", "threadOf", "a prefix of a longer id and a shorter one", d::thread_of);
}

#[test]
fn thread_of__no_thread() {
    holds("ws_reads", "threadOf", "no thread", d::thread_of);
}

#[test]
fn thread_of__no_thread_among_none() {
    holds("ws_reads", "threadOf", "no thread among none", d::thread_of);
}

#[test]
fn thread_of__a_ref_longer_than_the_id() {
    holds("ws_reads", "threadOf", "a ref longer than the id", d::thread_of);
}

#[test]
fn thread_of__the_id_differs_in_case() {
    holds("ws_reads", "threadOf", "the id differs in case", d::thread_of);
}

#[test]
fn thread_of__a_ref_with_a_space() {
    holds("ws_reads", "threadOf", "a ref with a space", d::thread_of);
}

#[test]
fn thread_of__a_row_with_no_thread_id_is_named_by_its_row_id() {
    holds("ws_reads", "threadOf", "a row with no thread id is named by its row id", d::thread_of);
}

#[test]
fn thread_of__a_prefix_of_a_row_id() {
    holds("ws_reads", "threadOf", "a prefix of a row id", d::thread_of);
}

#[test]
fn thread_of__two_turns_of_one_thread_are_one_hit() {
    holds("ws_reads", "threadOf", "two turns of one thread are one hit", d::thread_of);
}

#[test]
fn thread_of__refused_by_the_host() {
    holds("ws_reads", "threadOf", "refused by the host", d::thread_of);
}

#[test]
fn thread_of__an_exact_id_cloud() {
    holds("ws_reads", "threadOf", "an exact id (cloud)", d::thread_of);
}

#[test]
fn thread_of__refused_by_the_host_cloud() {
    holds("ws_reads", "threadOf", "refused by the host (cloud)", d::thread_of);
}

#[test]
fn threads_of__an_exact_id() {
    holds("ws_reads", "threadsOf", "an exact id", d::threads_of);
}

#[test]
fn threads_of__a_prefix_naming_two() {
    holds("ws_reads", "threadsOf", "a prefix naming two", d::threads_of);
}

#[test]
fn threads_of__no_thread() {
    holds("ws_reads", "threadsOf", "no thread", d::threads_of);
}

#[test]
fn threads_of__no_thread_among_none() {
    holds("ws_reads", "threadsOf", "no thread among none", d::threads_of);
}

#[test]
fn threads_of__refused_by_the_host() {
    holds("ws_reads", "threadsOf", "refused by the host", d::threads_of);
}

#[test]
fn threads_of__none_named() {
    holds("ws_reads", "threadsOf", "none named", d::threads_of);
}

#[test]
fn threads_of__none_named_none_held() {
    holds("ws_reads", "threadsOf", "none named, none held", d::threads_of);
}

#[test]
fn threads_of__two_in_the_order_named() {
    holds("ws_reads", "threadsOf", "two in the order named", d::threads_of);
}

#[test]
fn threads_of__the_same_twice_is_two_rows() {
    holds("ws_reads", "threadsOf", "the same twice is two rows", d::threads_of);
}

#[test]
fn threads_of__the_first_bad_ref_stops_the_rest() {
    holds("ws_reads", "threadsOf", "the first bad ref stops the rest", d::threads_of);
}

#[test]
fn threads_of__one_listing_for_many_refs() {
    holds("ws_reads", "threadsOf", "one listing for many refs", d::threads_of);
}

#[test]
fn threads_of__an_exact_id_cloud() {
    holds("ws_reads", "threadsOf", "an exact id (cloud)", d::threads_of);
}

#[test]
fn threads_of__refused_by_the_host_cloud() {
    holds("ws_reads", "threadsOf", "refused by the host (cloud)", d::threads_of);
}

#[test]
fn workspace_of__by_id() {
    holds("ws_reads", "workspaceOf", "by id", d::workspace_of);
}

#[test]
fn workspace_of__keys_come_back_in_the_schema_s_order() {
    holds("ws_reads", "workspaceOf", "keys come back in the schema's order", d::workspace_of);
}

#[test]
fn workspace_of__an_unknown_key_is_dropped() {
    holds("ws_reads", "workspaceOf", "an unknown key is dropped", d::workspace_of);
}

#[test]
fn workspace_of__every_optional_field() {
    holds("ws_reads", "workspaceOf", "every optional field", d::workspace_of);
}

#[test]
fn workspace_of__an_empty_ref_is_sent_as_typed() {
    holds("ws_reads", "workspaceOf", "an empty ref is sent as typed", d::workspace_of);
}

#[test]
fn workspace_of__a_ref_with_spaces_and_unicode() {
    holds("ws_reads", "workspaceOf", "a ref with spaces and unicode", d::workspace_of);
}

#[test]
fn workspace_of__no_such_workspace() {
    holds("ws_reads", "workspaceOf", "no such workspace", d::workspace_of);
}

#[test]
fn workspace_of__a_name_two_workspaces_carry() {
    holds("ws_reads", "workspaceOf", "a name two workspaces carry", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_caller_may_not_drive() {
    holds("ws_reads", "workspaceOf", "a workspace this caller may not drive", d::workspace_of);
}

#[test]
fn workspace_of__an_answer_with_no_workspace() {
    holds("ws_reads", "workspaceOf", "an answer with no workspace", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_build_cannot_read_1() {
    holds("ws_reads", "workspaceOf", "a workspace this build cannot read 1", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_build_cannot_read_2() {
    holds("ws_reads", "workspaceOf", "a workspace this build cannot read 2", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_build_cannot_read_3() {
    holds("ws_reads", "workspaceOf", "a workspace this build cannot read 3", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_build_cannot_read_4() {
    holds("ws_reads", "workspaceOf", "a workspace this build cannot read 4", d::workspace_of);
}

#[test]
fn workspace_of__a_workspace_this_build_cannot_read_5() {
    holds("ws_reads", "workspaceOf", "a workspace this build cannot read 5", d::workspace_of);
}

#[test]
fn workspace_of__by_id_cloud() {
    holds("ws_reads", "workspaceOf", "by id (cloud)", d::workspace_of);
}

#[test]
fn thread_rows__nothing() {
    holds("ws_reads", "threadRows", "nothing", d::thread_rows);
}

#[test]
fn thread_rows__a_thread_on_a_local_folder_takes_the_branch_git_has_checked_() {
    holds("ws_reads", "threadRows", "a thread on a local folder takes the branch git has checked out", d::thread_rows);
}

#[test]
fn thread_rows__the_checked_out_branch_beats_the_worktree_record() {
    holds("ws_reads", "threadRows", "the checked out branch beats the worktree record", d::thread_rows);
}

#[test]
fn thread_rows__a_folder_below_the_repo_root_finds_the_repo() {
    holds("ws_reads", "threadRows", "a folder below the repo root finds the repo", d::thread_rows);
}

#[test]
fn thread_rows__a_detached_head_falls_to_the_worktree_record() {
    holds("ws_reads", "threadRows", "a detached head falls to the worktree record", d::thread_rows);
}

#[test]
fn thread_rows__a_detached_head_with_no_worktree_record_has_no_branch() {
    holds("ws_reads", "threadRows", "a detached head with no worktree record has no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_git_folder_with_no_head_file_falls_to_the_record() {
    holds("ws_reads", "threadRows", "a git folder with no head file falls to the record", d::thread_rows);
}

#[test]
fn thread_rows__a_folder_in_no_repo_has_no_branch() {
    holds("ws_reads", "threadRows", "a folder in no repo has no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_folder_that_is_not_there_has_no_branch() {
    holds("ws_reads", "threadRows", "a folder that is not there has no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_worktree_s_git_file_points_at_its_git_dir() {
    holds("ws_reads", "threadRows", "a worktree's .git file points at its git dir", d::thread_rows);
}

#[test]
fn thread_rows__a_worktree_s_git_file_with_a_relative_path() {
    holds("ws_reads", "threadRows", "a worktree's .git file with a relative path", d::thread_rows);
}

#[test]
fn thread_rows__a_git_file_naming_no_git_dir() {
    holds("ws_reads", "threadRows", "a .git file naming no git dir", d::thread_rows);
}

#[test]
fn thread_rows__the_ref_line_is_read_from_any_line_of_head_trimmed() {
    holds("ws_reads", "threadRows", "the ref line is read from any line of HEAD, trimmed", d::thread_rows);
}

#[test]
fn thread_rows__a_head_with_crlf_line_ends() {
    holds("ws_reads", "threadRows", "a HEAD with CRLF line ends", d::thread_rows);
}

#[test]
fn thread_rows__a_ref_outside_heads_is_no_branch() {
    holds("ws_reads", "threadRows", "a ref outside heads is no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_branch_with_slashes() {
    holds("ws_reads", "threadRows", "a branch with slashes", d::thread_rows);
}

#[test]
fn thread_rows__the_thread_s_own_folder_wins_over_the_workspace_folder() {
    holds("ws_reads", "threadRows", "the thread's own folder wins over the workspace folder", d::thread_rows);
}

#[test]
fn thread_rows__no_turn_folder_falls_to_the_workspace_folder() {
    holds("ws_reads", "threadRows", "no turn folder falls to the workspace folder", d::thread_rows);
}

#[test]
fn thread_rows__no_turn_folder_and_no_workspace_folder_falls_to_the_project_() {
    holds("ws_reads", "threadRows", "no turn folder and no workspace folder falls to the project path", d::thread_rows);
}

#[test]
fn thread_rows__two_threads_on_one_folder() {
    holds("ws_reads", "threadRows", "two threads on one folder", d::thread_rows);
}

#[test]
fn thread_rows__a_cloud_workspace_reads_no_git_here_and_takes_the_record() {
    holds("ws_reads", "threadRows", "a cloud workspace reads no git here and takes the record", d::thread_rows);
}

#[test]
fn thread_rows__a_cloud_workspace_with_no_record_has_no_branch() {
    holds("ws_reads", "threadRows", "a cloud workspace with no record has no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_workspace_of_no_kind_is_a_cloud_s() {
    holds("ws_reads", "threadRows", "a workspace of no kind is a cloud's", d::thread_rows);
}

#[test]
fn thread_rows__a_place_workspace_never_reads_a_folder_here_though_one_stand() {
    holds("ws_reads", "threadRows", "a place workspace never reads a folder here, though one stands there", d::thread_rows);
}

#[test]
fn thread_rows__a_place_workspace_takes_the_record() {
    holds("ws_reads", "threadRows", "a place workspace takes the record", d::thread_rows);
}

#[test]
fn thread_rows__a_gone_worktree_gives_no_branch() {
    holds("ws_reads", "threadRows", "a gone worktree gives no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_worktree_record_with_no_branch() {
    holds("ws_reads", "threadRows", "a worktree record with no branch", d::thread_rows);
}

#[test]
fn thread_rows__a_turn_inside_a_gone_worktree_lists_the_workspace_folder() {
    holds("ws_reads", "threadRows", "a turn inside a gone worktree lists the workspace folder", d::thread_rows);
}

#[test]
fn thread_rows__a_turn_outside_a_gone_worktree_keeps_its_folder() {
    holds("ws_reads", "threadRows", "a turn outside a gone worktree keeps its folder", d::thread_rows);
}

#[test]
fn thread_rows__a_turn_inside_a_gone_worktree_with_no_workspace_folder_lists() {
    holds("ws_reads", "threadRows", "a turn inside a gone worktree with no workspace folder lists the project path", d::thread_rows);
}

#[test]
fn thread_rows__a_live_worktree_keeps_the_turn_s_folder() {
    holds("ws_reads", "threadRows", "a live worktree keeps the turn's folder", d::thread_rows);
}

#[test]
fn thread_rows__a_thread_whose_workspace_is_not_listed() {
    holds("ws_reads", "threadRows", "a thread whose workspace is not listed", d::thread_rows);
}

#[test]
fn thread_rows__a_thread_whose_workspace_is_not_listed_and_no_folder() {
    holds("ws_reads", "threadRows", "a thread whose workspace is not listed and no folder", d::thread_rows);
}

#[test]
fn thread_rows__the_session_s_project_and_computer_not_the_workspace_s() {
    holds("ws_reads", "threadRows", "the session's project and computer, not the workspace's", d::thread_rows);
}

#[test]
fn thread_rows__a_row_with_no_project_or_computer_name() {
    holds("ws_reads", "threadRows", "a row with no project or computer name", d::thread_rows);
}

#[test]
fn thread_rows__rows_in_the_fold_s_order() {
    holds("ws_reads", "threadRows", "rows in the fold's order", d::thread_rows);
}

#[test]
fn thread_rows__the_workspace_list_is_asked_before_the_sessions() {
    holds("ws_reads", "threadRows", "the workspace list is asked before the sessions", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_by_name() {
    holds("ws_reads", "threadRows", "within a project by name", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_by_id() {
    holds("ws_reads", "threadRows", "within a project by id", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_whose_threads_sit_on_a_workspace_the_caller() {
    holds("ws_reads", "threadRows", "within a project whose threads sit on a workspace the caller cannot list", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_no_session_names_which_the_projects_list_ho() {
    holds("ws_reads", "threadRows", "within a project no session names, which the projects list holds", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_the_projects_list_holds_by_id() {
    holds("ws_reads", "threadRows", "within a project the projects list holds by id", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_nothing_names() {
    holds("ws_reads", "threadRows", "within a project nothing names", d::thread_rows);
}

#[test]
fn thread_rows__within_the_empty_word() {
    holds("ws_reads", "threadRows", "within the empty word", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_the_projects_list_refused_the_workspaces_na() {
    holds("ws_reads", "threadRows", "within a project, the projects list refused, the workspaces name it", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_nothing_names_the_projects_list_refused() {
    holds("ws_reads", "threadRows", "within a project nothing names, the projects list refused", d::thread_rows);
}

#[test]
fn thread_rows__within_a_project_that_is_found_in_the_sessions_asks_no_proje() {
    holds("ws_reads", "threadRows", "within a project that is found in the sessions asks no projects", d::thread_rows);
}

#[test]
fn thread_rows__the_workspace_list_refused() {
    holds("ws_reads", "threadRows", "the workspace list refused", d::thread_rows);
}

#[test]
fn thread_rows__the_session_list_refused() {
    holds("ws_reads", "threadRows", "the session list refused", d::thread_rows);
}

#[test]
fn thread_rows__nothing_cloud() {
    holds("ws_reads", "threadRows", "nothing (cloud)", d::thread_rows);
}

#[test]
fn state_line__phase_running() {
    holds("ws_reads", "stateLine", "phase running", d::state_line);
}

#[test]
fn state_line__phase_pausing() {
    holds("ws_reads", "stateLine", "phase pausing", d::state_line);
}

#[test]
fn state_line__phase_napping() {
    holds("ws_reads", "stateLine", "phase napping", d::state_line);
}

#[test]
fn state_line__phase_waking() {
    holds("ws_reads", "stateLine", "phase waking", d::state_line);
}

#[test]
fn state_line__phase_gone() {
    holds("ws_reads", "stateLine", "phase gone", d::state_line);
}

#[test]
fn state_line__phase_queued() {
    holds("ws_reads", "stateLine", "phase queued", d::state_line);
}

#[test]
fn state_line__phase_released() {
    holds("ws_reads", "stateLine", "phase released", d::state_line);
}

#[test]
fn state_line__phase_pausing_on_a_disk_pause() {
    holds("ws_reads", "stateLine", "phase pausing on a disk pause", d::state_line);
}

#[test]
fn state_line__phase_napping_on_a_disk_pause() {
    holds("ws_reads", "stateLine", "phase napping on a disk pause", d::state_line);
}

#[test]
fn state_line__phase_running_on_a_disk_pause() {
    holds("ws_reads", "stateLine", "phase running on a disk pause", d::state_line);
}

#[test]
fn state_line__phase_pausing_on_a_memory_pause() {
    holds("ws_reads", "stateLine", "phase pausing on a memory pause", d::state_line);
}

#[test]
fn state_line__phase_napping_on_a_memory_pause() {
    holds("ws_reads", "stateLine", "phase napping on a memory pause", d::state_line);
}

#[test]
fn state_line__phase_running_on_a_memory_pause() {
    holds("ws_reads", "stateLine", "phase running on a memory pause", d::state_line);
}

#[test]
fn state_line__phase_running_cloud() {
    holds("ws_reads", "stateLine", "phase running (cloud)", d::state_line);
}

#[test]
fn state_line__a_name_with_spaces_and_quotes() {
    holds("ws_reads", "stateLine", "a name with spaces and quotes", d::state_line);
}

#[test]
fn state_line__an_empty_name() {
    holds("ws_reads", "stateLine", "an empty name", d::state_line);
}

#[test]
fn rebuilt_line__a_running_machine() {
    holds("ws_reads", "rebuiltLine", "a running machine", d::rebuilt_line);
}

#[test]
fn rebuilt_line__the_pause_mode_is_not_read() {
    holds("ws_reads", "rebuiltLine", "the pause mode is not read", d::rebuilt_line);
}

#[test]
fn rebuilt_line__a_waking_machine() {
    holds("ws_reads", "rebuiltLine", "a waking machine", d::rebuilt_line);
}

#[test]
fn rebuilt_line__an_empty_machine_id() {
    holds("ws_reads", "rebuiltLine", "an empty machine id", d::rebuilt_line);
}

#[test]
fn state_line__a_name_with_spaces_and_quotes_cloud() {
    holds("ws_reads", "stateLine", "a name with spaces and quotes (cloud)", d::state_line);
}

#[test]
fn woke_line__the_listing_reads_running() {
    holds("ws_reads", "wokeLine", "the listing reads running", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_the_machine_up_and_not_answering() {
    holds("ws_reads", "wokeLine", "the listing reads the machine up and not answering", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_a_zombie() {
    holds("ws_reads", "wokeLine", "the listing reads a zombie", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_no_daemon() {
    holds("ws_reads", "wokeLine", "the listing reads no daemon", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_the_machine_paused_under_a_running_phase() {
    holds("ws_reads", "wokeLine", "the listing reads the machine paused under a running phase", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_the_machine_starting() {
    holds("ws_reads", "wokeLine", "the listing reads the machine starting", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_the_machine_gone() {
    holds("ws_reads", "wokeLine", "the listing reads the machine gone", d::woke_line);
}

#[test]
fn woke_line__the_listing_s_phase_is_what_it_is_read_by_not_the_record_s() {
    holds("ws_reads", "wokeLine", "the listing's phase is what it is read by, not the record's", d::woke_line);
}

#[test]
fn woke_line__the_workspace_is_not_in_the_listing() {
    holds("ws_reads", "wokeLine", "the workspace is not in the listing", d::woke_line);
}

#[test]
fn woke_line__the_listing_is_empty() {
    holds("ws_reads", "wokeLine", "the listing is empty", d::woke_line);
}

#[test]
fn woke_line__the_listing_is_empty_on_a_waking_phase() {
    holds("ws_reads", "wokeLine", "the listing is empty on a waking phase", d::woke_line);
}

#[test]
fn woke_line__the_second_of_two() {
    holds("ws_reads", "wokeLine", "the second of two", d::woke_line);
}

#[test]
fn woke_line__a_pause_that_keeps_memory_read_as_paused() {
    holds("ws_reads", "wokeLine", "a pause that keeps memory, read as paused", d::woke_line);
}

#[test]
fn woke_line__a_disk_pause_read_as_stopped_the_listing_says_asleep() {
    holds("ws_reads", "wokeLine", "a disk pause, read as stopped (the listing says asleep)", d::woke_line);
}

#[test]
fn woke_line__the_status_list_refused() {
    holds("ws_reads", "wokeLine", "the status list refused", d::woke_line);
}

#[test]
fn woke_line__the_listing_reads_running_cloud() {
    holds("ws_reads", "wokeLine", "the listing reads running (cloud)", d::woke_line);
}

#[test]
fn nap__a_running_workspace() {
    holds("ws_reads", "nap", "a running workspace", d::nap);
}

#[test]
fn nap__the_nap_is_asked_by_id_not_the_word_typed() {
    holds("ws_reads", "nap", "the nap is asked by id, not the word typed", d::nap);
}

#[test]
fn nap__no_such_workspace_asks_no_nap() {
    holds("ws_reads", "nap", "no such workspace asks no nap", d::nap);
}

#[test]
fn nap__the_nap_refused() {
    holds("ws_reads", "nap", "the nap refused", d::nap);
}

#[test]
fn nap__a_record_this_build_cannot_read_asks_no_nap() {
    holds("ws_reads", "nap", "a record this build cannot read asks no nap", d::nap);
}

#[test]
fn nap__already_napping() {
    holds("ws_reads", "nap", "already napping", d::nap);
}

#[test]
fn nap__the_answer_is_not_parsed_as_the_host_wrote_it() {
    holds("ws_reads", "nap", "the answer is not parsed, as the host wrote it", d::nap);
}

#[test]
fn resize__a_thread_s_machine() {
    holds("ws_reads", "resize", "a thread's machine", d::resize);
}

#[test]
fn resize__the_thread_is_named_by_a_prefix() {
    holds("ws_reads", "resize", "the thread is named by a prefix", d::resize);
}

#[test]
fn resize__the_thread_s_workspace_id_not_the_thread_s_is_sent() {
    holds("ws_reads", "resize", "the thread's workspace id, not the thread's, is sent", d::resize);
}

#[test]
fn resize__no_such_thread_asks_no_resize() {
    holds("ws_reads", "resize", "no such thread asks no resize", d::resize);
}

#[test]
fn resize__a_prefix_naming_two_asks_no_resize() {
    holds("ws_reads", "resize", "a prefix naming two asks no resize", d::resize);
}

#[test]
fn resize__the_size_is_not_on_offer() {
    holds("ws_reads", "resize", "the size is not on offer", d::resize);
}

#[test]
fn resize__a_turn_is_running() {
    holds("ws_reads", "resize", "a turn is running", d::resize);
}

#[test]
fn nap__a_running_workspace_cloud() {
    holds("ws_reads", "nap", "a running workspace (cloud)", d::nap);
}

#[test]
fn rebuild__a_gone_phase() {
    holds("ws_reads", "rebuild", "a gone phase", d::rebuild);
}

#[test]
fn rebuild__a_gone_phase_with_a_listing() {
    holds("ws_reads", "rebuild", "a gone phase with a listing", d::rebuild);
}

#[test]
fn rebuild__a_running_phase_whose_machine_the_listing_reads_gone() {
    holds("ws_reads", "rebuild", "a running phase whose machine the listing reads gone", d::rebuild);
}

#[test]
fn rebuild__a_zombie() {
    holds("ws_reads", "rebuild", "a zombie", d::rebuild);
}

#[test]
fn rebuild__a_wake_the_provider_refused_with_no_reason() {
    holds("ws_reads", "rebuild", "a wake the provider refused, with no reason", d::rebuild);
}

#[test]
fn rebuild__a_wake_the_provider_refused_with_a_reason() {
    holds("ws_reads", "rebuild", "a wake the provider refused, with a reason", d::rebuild);
}

#[test]
fn rebuild__a_wake_refused_on_a_running_workspace() {
    holds("ws_reads", "rebuild", "a wake refused on a running workspace", d::rebuild);
}

#[test]
fn rebuild__a_released_workspace() {
    holds("ws_reads", "rebuild", "a released workspace", d::rebuild);
}

#[test]
fn rebuild__running_and_reachable_refused() {
    holds("ws_reads", "rebuild", "running and reachable, refused", d::rebuild);
}

#[test]
fn rebuild__running_with_no_listing_refused() {
    holds("ws_reads", "rebuild", "running with no listing, refused", d::rebuild);
}

#[test]
fn rebuild__unreachable_is_not_gone_refused() {
    holds("ws_reads", "rebuild", "unreachable is not gone, refused", d::rebuild);
}

#[test]
fn rebuild__no_daemon_is_not_gone_refused() {
    holds("ws_reads", "rebuild", "no daemon is not gone, refused", d::rebuild);
}

#[test]
fn rebuild__napping_refused() {
    holds("ws_reads", "rebuild", "napping, refused", d::rebuild);
}

#[test]
fn rebuild__napping_on_a_memory_pause_refused() {
    holds("ws_reads", "rebuild", "napping on a memory pause, refused", d::rebuild);
}

#[test]
fn rebuild__pausing_refused() {
    holds("ws_reads", "rebuild", "pausing, refused", d::rebuild);
}

#[test]
fn rebuild__waking_refused() {
    holds("ws_reads", "rebuild", "waking, refused", d::rebuild);
}

#[test]
fn rebuild__queued_refused() {
    holds("ws_reads", "rebuild", "queued, refused", d::rebuild);
}

#[test]
fn rebuild__paused_machine_under_a_running_phase_refused() {
    holds("ws_reads", "rebuild", "paused machine under a running phase, refused", d::rebuild);
}

#[test]
fn rebuild__the_listing_of_another_workspace_is_not_read() {
    holds("ws_reads", "rebuild", "the listing of another workspace is not read", d::rebuild);
}

#[test]
fn rebuild__a_gone_workspace_whose_rebuild_came_back_running() {
    holds("ws_reads", "rebuild", "a gone workspace whose rebuild came back running", d::rebuild);
}

#[test]
fn rebuild__no_such_workspace_asks_no_status() {
    holds("ws_reads", "rebuild", "no such workspace asks no status", d::rebuild);
}

#[test]
fn rebuild__the_status_list_refused() {
    holds("ws_reads", "rebuild", "the status list refused", d::rebuild);
}

#[test]
fn rebuild__the_rebuild_refused() {
    holds("ws_reads", "rebuild", "the rebuild refused", d::rebuild);
}

#[test]
fn rebuild__a_gone_phase_cloud() {
    holds("ws_reads", "rebuild", "a gone phase (cloud)", d::rebuild);
}

#[test]
fn image_lines__none() {
    holds("ws_reads", "imageLines", "none", d::image_lines);
}

#[test]
fn image_lines__a_record_with_no_copies() {
    holds("ws_reads", "imageLines", "a record with no copies", d::image_lines);
}

#[test]
fn image_lines__a_record_with_a_vault_and_one_copy() {
    holds("ws_reads", "imageLines", "a record with a vault and one copy", d::image_lines);
}

#[test]
fn image_lines__a_copy_built_from_an_older_record() {
    holds("ws_reads", "imageLines", "a copy built from an older record", d::image_lines);
}

#[test]
fn image_lines__a_copy_with_no_hash() {
    holds("ws_reads", "imageLines", "a copy with no hash", d::image_lines);
}

#[test]
fn image_lines__no_vault_and_no_mark_so_no_standing() {
    holds("ws_reads", "imageLines", "no vault and no mark, so no standing", d::image_lines);
}

#[test]
fn image_lines__no_secrets_so_a_standing_without_a_vault() {
    holds("ws_reads", "imageLines", "no secrets, so a standing without a vault", d::image_lines);
}

#[test]
fn image_lines__a_copy_with_no_size() {
    holds("ws_reads", "imageLines", "a copy with no size", d::image_lines);
}

#[test]
fn image_lines__copies_in_the_host_s_order() {
    holds("ws_reads", "imageLines", "copies in the host's order", d::image_lines);
}

#[test]
fn image_lines__a_copy_of_zero_bytes() {
    holds("ws_reads", "imageLines", "a copy of zero bytes", d::image_lines);
}

#[test]
fn image_lines__a_copy_of_a_gigabyte_less_one_byte_and_a_gigabyte() {
    holds("ws_reads", "imageLines", "a copy of a gigabyte less one byte and a gigabyte", d::image_lines);
}

#[test]
fn image_lines__a_copy_of_1023_and_1024_bytes() {
    holds("ws_reads", "imageLines", "a copy of 1023 and 1024 bytes", d::image_lines);
}

#[test]
fn image_lines__a_record_of_the_place_it_stands_at_with_a_template() {
    holds("ws_reads", "imageLines", "a record of the place it stands at, with a template", d::image_lines);
}

#[test]
fn image_lines__none_cloud() {
    holds("ws_reads", "imageLines", "none (cloud)", d::image_lines);
}

#[test]
fn image_view__none() {
    holds("ws_reads", "imageView", "none", d::image_view);
}

#[test]
fn image_view__a_record_with_no_copies() {
    holds("ws_reads", "imageView", "a record with no copies", d::image_view);
}

#[test]
fn image_view__a_record_with_a_vault_and_one_copy() {
    holds("ws_reads", "imageView", "a record with a vault and one copy", d::image_view);
}

#[test]
fn image_view__a_copy_built_from_an_older_record() {
    holds("ws_reads", "imageView", "a copy built from an older record", d::image_view);
}

#[test]
fn image_view__none_cloud() {
    holds("ws_reads", "imageView", "none (cloud)", d::image_view);
}

#[test]
fn image_view__an_unknown_key_is_dropped() {
    holds("ws_reads", "imageView", "an unknown key is dropped", d::image_view);
}

#[test]
fn image_view__keys_come_back_in_the_schema_s_order() {
    holds("ws_reads", "imageView", "keys come back in the schema's order", d::image_view);
}

#[test]
fn image_view__the_frame_sent_is_image_get_with_no_params() {
    holds("ws_reads", "imageView", "the frame sent is image.get with no params", d::image_view);
}

#[test]
fn image_view__refused_by_the_host() {
    holds("ws_reads", "imageView", "refused by the host", d::image_view);
}

#[test]
fn image_view__an_unknown_key_is_dropped_cloud() {
    holds("ws_reads", "imageView", "an unknown key is dropped (cloud)", d::image_view);
}

#[test]
fn image_view__refused_by_the_host_cloud() {
    holds("ws_reads", "imageView", "refused by the host (cloud)", d::image_view);
}

#[test]
fn build_image_at__built_with_the_stages_streamed() {
    holds("ws_reads", "buildImageAt", "built, with the stages streamed", d::build_image_at);
}

#[test]
fn build_image_at__a_place_by_its_id() {
    holds("ws_reads", "buildImageAt", "a place by its id", d::build_image_at);
}

#[test]
fn build_image_at__a_word_that_names_no_place_is_sent_as_typed() {
    holds("ws_reads", "buildImageAt", "a word that names no place is sent as typed", d::build_image_at);
}

#[test]
fn build_image_at__forced() {
    holds("ws_reads", "buildImageAt", "forced", d::build_image_at);
}

#[test]
fn build_image_at__force_false_is_not_sent() {
    holds("ws_reads", "buildImageAt", "force false is not sent", d::build_image_at);
}

#[test]
fn build_image_at__already_built() {
    holds("ws_reads", "buildImageAt", "already built", d::build_image_at);
}

#[test]
fn build_image_at__a_failed_stage_with_its_detail() {
    holds("ws_reads", "buildImageAt", "a failed stage with its detail", d::build_image_at);
}

#[test]
fn build_image_at__a_failed_stage_with_no_detail() {
    holds("ws_reads", "buildImageAt", "a failed stage with no detail", d::build_image_at);
}

#[test]
fn build_image_at__a_stage_with_a_detail() {
    holds("ws_reads", "buildImageAt", "a stage with a detail", d::build_image_at);
}

#[test]
fn build_image_at__every_stage_in_order() {
    holds("ws_reads", "buildImageAt", "every stage in order", d::build_image_at);
}

#[test]
fn build_image_at__another_place_s_stages_and_other_frames_are_not_streamed() {
    holds("ws_reads", "buildImageAt", "another place's stages and other frames are not streamed", d::build_image_at);
}

#[test]
fn build_image_at__a_provider_named_in_any_case_takes_its_table_name() {
    holds("ws_reads", "buildImageAt", "a provider named in any case takes its table name", d::build_image_at);
}

#[test]
fn build_image_at__a_stage_frame_heard_after_the_build_returned_is_not_streamed() {
    holds("ws_reads", "buildImageAt", "a stage frame heard after the build returned is not streamed", d::build_image_at);
}

#[test]
fn build_image_at__the_place_list_names_nobody_and_the_build_is_refused() {
    holds("ws_reads", "buildImageAt", "the place list names nobody and the build is refused", d::build_image_at);
}

#[test]
fn build_image_at__the_place_list_refused() {
    holds("ws_reads", "buildImageAt", "the place list refused", d::build_image_at);
}

#[test]
fn build_image_at__no_image_stands_after_the_build() {
    holds("ws_reads", "buildImageAt", "no image stands after the build", d::build_image_at);
}

#[test]
fn build_image_at__no_image_after_a_build_of_boat_add_s_shape_an_empty_build_re() {
    holds("ws_reads", "buildImageAt", "no image after a build of boat-add's shape, an empty build record", d::build_image_at);
}

#[test]
fn build_image_at__the_build_refused_after_stages_were_streamed() {
    holds("ws_reads", "buildImageAt", "the build refused after stages were streamed", d::build_image_at);
}

#[test]
fn build_image_at__the_image_read_refused_after_the_build() {
    holds("ws_reads", "buildImageAt", "the image read refused after the build", d::build_image_at);
}

#[test]
fn build_image_at__the_host_closes_during_the_build() {
    holds("ws_reads", "buildImageAt", "the host closes during the build", d::build_image_at);
}

#[test]
fn build_image_at__the_host_stops_during_the_build() {
    holds("ws_reads", "buildImageAt", "the host stops during the build", d::build_image_at);
}

#[test]
fn build_image_at__built_with_the_stages_streamed_cloud() {
    holds("ws_reads", "buildImageAt", "built, with the stages streamed (cloud)", d::build_image_at);
}

#[test]
fn const_host_side_vault__the_sentence() {
    holds("ws_reads", "const_host_side_vault", "the sentence", d::const_host_side_vault);
}

#[test]
fn const_ssh_pipes_here_line__the_sentence() {
    holds("ws_reads", "const_ssh_pipes_here_line", "the sentence", d::const_ssh_pipes_here_line);
}

#[test]
fn ssh_workspace_of__a_name_not_an_alias_goes_to_the_resolve() {
    holds("ws_reads", "sshWorkspaceOf", "a name, not an alias, goes to the resolve", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__a_name_that_is_not_an_alias_asks_no_list() {
    holds("ws_reads", "sshWorkspaceOf", "a name that is not an alias asks no list", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_of_one_workspace() {
    holds("ws_reads", "sshWorkspaceOf", "an alias of one workspace", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_answers_the_host_s_list_entry_whole() {
    holds("ws_reads", "sshWorkspaceOf", "an alias answers the host's list entry whole", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_of_two_workspaces() {
    holds("ws_reads", "sshWorkspaceOf", "an alias of two workspaces", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_of_three_workspaces() {
    holds("ws_reads", "sshWorkspaceOf", "an alias of three workspaces", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_of_none_falls_to_the_resolve() {
    holds("ws_reads", "sshWorkspaceOf", "an alias of none falls to the resolve", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_alias_of_none_and_no_such_workspace() {
    holds("ws_reads", "sshWorkspaceOf", "an alias of none and no such workspace", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__the_prefix_alone() {
    holds("ws_reads", "sshWorkspaceOf", "the prefix alone", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__the_prefix_in_capitals_is_a_name() {
    holds("ws_reads", "sshWorkspaceOf", "the prefix in capitals is a name", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__a_name_with_no_letters_or_digits_is_work() {
    holds("ws_reads", "sshWorkspaceOf", "a name with no letters or digits is work", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__a_long_name_is_cut_at_40() {
    holds("ws_reads", "sshWorkspaceOf", "a long name is cut at 40", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__a_long_name_cut_at_a_dash_drops_the_dash() {
    holds("ws_reads", "sshWorkspaceOf", "a long name cut at a dash drops the dash", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__an_empty_list_falls_to_the_resolve() {
    holds("ws_reads", "sshWorkspaceOf", "an empty list falls to the resolve", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__the_list_refused() {
    holds("ws_reads", "sshWorkspaceOf", "the list refused", d::ssh_workspace_of);
}

#[test]
fn ssh_workspace_of__the_resolve_answers_a_record_this_build_cannot_read() {
    holds("ws_reads", "sshWorkspaceOf", "the resolve answers a record this build cannot read", d::ssh_workspace_of);
}

#[test]
fn const_host_side_vault__the_sentence_cloud() {
    holds("ws_reads", "const_host_side_vault", "the sentence (cloud)", d::const_host_side_vault);
}
