// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_target.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_target as d;

#[test]
fn sub_for__an_empty_notice_says_one_empty_line() {
    holds("turns_target", "subFor", "an empty notice says one empty line", d::sub_for);
}

#[test]
fn sub_for__a_notice_with_a_trailing_newline_says_its_empty_last_line() {
    holds("turns_target", "subFor", "a notice with a trailing newline says its empty last line", d::sub_for);
}

#[test]
fn sub_for__the_stages_come_before_the_notice() {
    holds("turns_target", "subFor", "the stages come before the notice", d::sub_for);
}

#[test]
fn sub_for__the_create_refused() {
    holds("turns_target", "subFor", "the create refused", d::sub_for);
}

#[test]
fn sub_for__the_create_refused_after_a_stage() {
    holds("turns_target", "subFor", "the create refused after a stage", d::sub_for);
}

#[test]
fn sub_for__the_machine_list_refused() {
    holds("turns_target", "subFor", "the machine list refused", d::sub_for);
}

#[test]
fn thread_here__a_thread_in_this_computer_s_folder() {
    holds("turns_target", "threadHere", "a thread in this computer's folder", d::thread_here);
}

#[test]
fn thread_here__by_a_prefix() {
    holds("turns_target", "threadHere", "by a prefix", d::thread_here);
}

#[test]
fn thread_here__a_thread_in_a_folder_on_a_computer_the_person_joined() {
    holds("turns_target", "threadHere", "a thread in a folder on a computer the person joined", d::thread_here);
}

#[test]
fn thread_here__a_thread_on_a_machine_is_none() {
    holds("turns_target", "threadHere", "a thread on a machine is none", d::thread_here);
}

#[test]
fn thread_here__a_thread_whose_workspace_is_not_listed_is_none() {
    holds("turns_target", "threadHere", "a thread whose workspace is not listed is none", d::thread_here);
}

#[test]
fn thread_here__a_workspace_of_no_kind_reads_as_a_machine() {
    holds("turns_target", "threadHere", "a workspace of no kind reads as a machine", d::thread_here);
}

#[test]
fn thread_here__a_row_with_no_thread_id_is_named_by_its_own() {
    holds("turns_target", "threadHere", "a row with no thread id is named by its own", d::thread_here);
}

#[test]
fn thread_here__a_word_naming_no_thread_is_none() {
    holds("turns_target", "threadHere", "a word naming no thread is none", d::thread_here);
}

#[test]
fn thread_here__a_word_naming_two_threads_is_none() {
    holds("turns_target", "threadHere", "a word naming two threads is none", d::thread_here);
}

#[test]
fn thread_here__no_word_asks_nothing() {
    holds("turns_target", "threadHere", "no word asks nothing", d::thread_here);
}

#[test]
fn thread_here__a_thread_already_read_asks_for_no_listing() {
    holds("turns_target", "threadHere", "a thread already read asks for no listing", d::thread_here);
}

#[test]
fn thread_here__a_thread_already_read_with_no_threadid_is_named_by_its_id() {
    holds("turns_target", "threadHere", "a thread already read, with no threadId, is named by its id", d::thread_here);
}

#[test]
fn thread_here__a_thread_already_read_on_a_machine() {
    holds("turns_target", "threadHere", "a thread already read, on a machine", d::thread_here);
}

#[test]
fn thread_here__the_made_worktree_it_runs_in() {
    holds("turns_target", "threadHere", "the made worktree it runs in", d::thread_here);
}

#[test]
fn thread_here__a_worktree_wsp_did_not_make_is_not_named() {
    holds("turns_target", "threadHere", "a worktree wsp did not make is not named", d::thread_here);
}

#[test]
fn thread_here__a_made_worktree_that_is_gone_is_not_named() {
    holds("turns_target", "threadHere", "a made worktree that is gone is not named", d::thread_here);
}

#[test]
fn thread_here__a_made_worktree_of_no_branch() {
    holds("turns_target", "threadHere", "a made worktree of no branch", d::thread_here);
}

#[test]
fn thread_here__the_listing_of_threads_refused_is_none() {
    holds("turns_target", "threadHere", "the listing of threads refused is none", d::thread_here);
}

#[test]
fn thread_here__the_listing_of_workspaces_refused_is_the_answer() {
    holds("turns_target", "threadHere", "the listing of workspaces refused is the answer", d::thread_here);
}

#[test]
fn sub_deleting__no_thread() {
    holds("turns_target", "subDeleting", "no thread", d::sub_deleting);
}

#[test]
fn sub_deleting__a_thread_that_is_no_sub_thread() {
    holds("turns_target", "subDeleting", "a thread that is no sub-thread", d::sub_deleting);
}

#[test]
fn sub_deleting__a_sub_thread_asks_the_host_first_as_a_check() {
    holds("turns_target", "subDeleting", "a sub-thread asks the host first, as a check", d::sub_deleting);
}

#[test]
fn sub_deleting__a_sub_thread_with_no_thread_id_is_named_by_its_row() {
    holds("turns_target", "subDeleting", "a sub-thread with no thread id is named by its row", d::sub_deleting);
}

#[test]
fn sub_deleting__the_question_as_the_host_words_it_whole() {
    holds("turns_target", "subDeleting", "the question as the host words it, whole", d::sub_deleting);
}

#[test]
fn sub_deleting__an_empty_question() {
    holds("turns_target", "subDeleting", "an empty question", d::sub_deleting);
}

#[test]
fn sub_deleting__the_host_refusing_work_not_pushed() {
    holds("turns_target", "subDeleting", "the host refusing: work not pushed", d::sub_deleting);
}

#[test]
fn sub_deleting__a_sub_thread_with_a_result() {
    holds("turns_target", "subDeleting", "a sub-thread with a result", d::sub_deleting);
}

#[test]
fn refuse_machine_thread__a_thread_on_a_machine_is_refused_with_the_machine_s_name() {
    holds("turns_target", "refuseMachineThread", "a thread on a machine is refused with the machine's name", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_thread_in_this_computer_s_folder_is_left_alone() {
    holds("turns_target", "refuseMachineThread", "a thread in this computer's folder is left alone", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_thread_in_a_folder_on_a_joined_computer_is_left_alone() {
    holds("turns_target", "refuseMachineThread", "a thread in a folder on a joined computer is left alone", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_thread_whose_workspace_is_not_listed_is_left_alone() {
    holds("turns_target", "refuseMachineThread", "a thread whose workspace is not listed is left alone", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_workspace_of_no_kind_reads_as_a_machine_and_is_refused() {
    holds("turns_target", "refuseMachineThread", "a workspace of no kind reads as a machine and is refused", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_word_naming_no_thread_is_left_to_the_caller_s_own_refusal() {
    holds("turns_target", "refuseMachineThread", "a word naming no thread is left to the caller's own refusal", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_word_naming_two_threads_is_left_to_the_caller_s_own_refusa() {
    holds("turns_target", "refuseMachineThread", "a word naming two threads is left to the caller's own refusal", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__a_thread_by_a_prefix_on_a_machine() {
    holds("turns_target", "refuseMachineThread", "a thread by a prefix on a machine", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__the_thread_listing_refused_leaves_the_refusal_to_the_caller() {
    holds("turns_target", "refuseMachineThread", "the thread listing refused leaves the refusal to the caller", d::refuse_machine_thread);
}

#[test]
fn refuse_machine_thread__the_workspace_listing_refused_is_the_answer() {
    holds("turns_target", "refuseMachineThread", "the workspace listing refused is the answer", d::refuse_machine_thread);
}

#[test]
fn thread_at__a_thread_by_id() {
    holds("turns_target", "threadAt", "a thread by id", d::thread_at);
}

#[test]
fn thread_at__a_thread_by_id_cloud_on() {
    holds("turns_target", "threadAt", "a thread by id, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_thread_by_a_prefix() {
    holds("turns_target", "threadAt", "a thread by a prefix", d::thread_at);
}

#[test]
fn thread_at__a_thread_by_a_prefix_cloud_on() {
    holds("turns_target", "threadAt", "a thread by a prefix, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_thread_s_own_workspace_is_asked_for() {
    holds("turns_target", "threadAt", "the thread's own workspace is asked for", d::thread_at);
}

#[test]
fn thread_at__the_thread_s_own_workspace_is_asked_for_cloud_on() {
    holds("turns_target", "threadAt", "the thread's own workspace is asked for, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_row_with_no_thread_id_asks_with_its_own_id() {
    holds("turns_target", "threadAt", "a row with no thread id asks with its own id", d::thread_at);
}

#[test]
fn thread_at__a_row_with_no_thread_id_asks_with_its_own_id_cloud_on() {
    holds("turns_target", "threadAt", "a row with no thread id asks with its own id, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_workspace_the_host_answers_in_another_version() {
    holds("turns_target", "threadAt", "a workspace the host answers in another version", d::thread_at);
}

#[test]
fn thread_at__a_workspace_the_host_answers_in_another_version_cloud_on() {
    holds("turns_target", "threadAt", "a workspace the host answers in another version, cloud on", d::thread_at);
}

#[test]
fn thread_at__no_workspace_in_the_answer() {
    holds("turns_target", "threadAt", "no workspace in the answer", d::thread_at);
}

#[test]
fn thread_at__no_workspace_in_the_answer_cloud_on() {
    holds("turns_target", "threadAt", "no workspace in the answer, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_workspace_the_host_refuses() {
    holds("turns_target", "threadAt", "a workspace the host refuses", d::thread_at);
}

#[test]
fn thread_at__a_workspace_the_host_refuses_cloud_on() {
    holds("turns_target", "threadAt", "a workspace the host refuses, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_thread_listing_refused() {
    holds("turns_target", "threadAt", "the thread listing refused", d::thread_at);
}

#[test]
fn thread_at__the_thread_listing_refused_cloud_on() {
    holds("turns_target", "threadAt", "the thread listing refused, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_word_naming_two_threads_is_not_a_refusal_of_the_rest() {
    holds("turns_target", "threadAt", "a word naming two threads is not a refusal of the rest", d::thread_at);
}

#[test]
fn thread_at__a_word_naming_two_threads_is_not_a_refusal_of_the_rest_cloud() {
    holds("turns_target", "threadAt", "a word naming two threads is not a refusal of the rest, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_word_naming_nothing_and_no_project() {
    holds("turns_target", "threadAt", "a word naming nothing and no project", d::thread_at);
}

#[test]
fn thread_at__a_word_naming_nothing_and_no_project_cloud_on() {
    holds("turns_target", "threadAt", "a word naming nothing and no project, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_project_s_name() {
    holds("turns_target", "threadAt", "a project's name", d::thread_at);
}

#[test]
fn thread_at__a_project_s_name_cloud_on() {
    holds("turns_target", "threadAt", "a project's name, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_project_s_id() {
    holds("turns_target", "threadAt", "a project's id", d::thread_at);
}

#[test]
fn thread_at__a_project_s_id_cloud_on() {
    holds("turns_target", "threadAt", "a project's id, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_project_s_name_said_for_a_line_that_also_takes_a_computer() {
    holds("turns_target", "threadAt", "a project's name, said for a line that also takes a computer", d::thread_at);
}

#[test]
fn thread_at__a_project_s_name_said_for_a_line_that_also_takes_a_computer_() {
    holds("turns_target", "threadAt", "a project's name, said for a line that also takes a computer, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_name() {
    holds("turns_target", "threadAt", "a machine's name", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_name_cloud_on() {
    holds("turns_target", "threadAt", "a machine's name, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_id() {
    holds("turns_target", "threadAt", "a machine's id", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_id_cloud_on() {
    holds("turns_target", "threadAt", "a machine's id, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_name_said_for_a_line_that_also_takes_a_computer() {
    holds("turns_target", "threadAt", "a machine's name, said for a line that also takes a computer", d::thread_at);
}

#[test]
fn thread_at__a_machine_s_name_said_for_a_line_that_also_takes_a_computer_() {
    holds("turns_target", "threadAt", "a machine's name, said for a line that also takes a computer, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_folder_workspace_s_name_is_no_machine() {
    holds("turns_target", "threadAt", "a folder workspace's name is no machine", d::thread_at);
}

#[test]
fn thread_at__a_folder_workspace_s_name_is_no_machine_cloud_on() {
    holds("turns_target", "threadAt", "a folder workspace's name is no machine, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_joined_computer_s_workspace_name_is_no_machine() {
    holds("turns_target", "threadAt", "a joined computer's workspace name is no machine", d::thread_at);
}

#[test]
fn thread_at__a_joined_computer_s_workspace_name_is_no_machine_cloud_on() {
    holds("turns_target", "threadAt", "a joined computer's workspace name is no machine, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_workspace_of_no_kind_is_a_machine() {
    holds("turns_target", "threadAt", "a workspace of no kind is a machine", d::thread_at);
}

#[test]
fn thread_at__a_workspace_of_no_kind_is_a_machine_cloud_on() {
    holds("turns_target", "threadAt", "a workspace of no kind is a machine, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_word_that_is_both_a_project_and_a_machine_is_a_project() {
    holds("turns_target", "threadAt", "a word that is both a project and a machine is a project", d::thread_at);
}

#[test]
fn thread_at__a_word_that_is_both_a_project_and_a_machine_is_a_project_clo() {
    holds("turns_target", "threadAt", "a word that is both a project and a machine is a project, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_the_machines_are_still_read() {
    holds("turns_target", "threadAt", "the project list refused: the machines are still read", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_the_machines_are_still_read_cloud_o() {
    holds("turns_target", "threadAt", "the project list refused: the machines are still read, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_and_the_workspaces_not_listing_a_ma() {
    holds("turns_target", "threadAt", "the project list refused and the workspaces not listing a machine", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_and_the_workspaces_not_listing_a_ma_2() {
    holds("turns_target", "threadAt", "the project list refused and the workspaces not listing a machine, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_a_project_of_the_thread_s_own_works() {
    holds("turns_target", "threadAt", "the project list refused: a project of the thread's own workspaces", d::thread_at);
}

#[test]
fn thread_at__the_project_list_refused_a_project_of_the_thread_s_own_works_2() {
    holds("turns_target", "threadAt", "the project list refused: a project of the thread's own workspaces, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_machine_list_refused_no_machine_is_named() {
    holds("turns_target", "threadAt", "the machine list refused: no machine is named", d::thread_at);
}

#[test]
fn thread_at__the_machine_list_refused_no_machine_is_named_cloud_on() {
    holds("turns_target", "threadAt", "the machine list refused: no machine is named, cloud on", d::thread_at);
}

#[test]
fn thread_at__no_projects_and_no_machines() {
    holds("turns_target", "threadAt", "no projects and no machines", d::thread_at);
}

#[test]
fn thread_at__no_projects_and_no_machines_cloud_on() {
    holds("turns_target", "threadAt", "no projects and no machines, cloud on", d::thread_at);
}

#[test]
fn thread_at__the_empty_word_names_every_thread() {
    holds("turns_target", "threadAt", "the empty word names every thread", d::thread_at);
}

#[test]
fn thread_at__the_empty_word_names_every_thread_cloud_on() {
    holds("turns_target", "threadAt", "the empty word names every thread, cloud on", d::thread_at);
}

#[test]
fn thread_at__a_line_named_with_its_own_words() {
    holds("turns_target", "threadAt", "a line named with its own words", d::thread_at);
}

#[test]
fn thread_at__a_line_named_with_its_own_words_cloud_on() {
    holds("turns_target", "threadAt", "a line named with its own words, cloud on", d::thread_at);
}

#[test]
fn child_of__a_child_in_a_folder_of_its_own() {
    holds("turns_target", "childOf", "a child in a folder of its own", d::child_of);
}

#[test]
fn child_of__a_child_by_a_prefix() {
    holds("turns_target", "childOf", "a child by a prefix", d::child_of);
}

#[test]
fn child_of__a_child_beside_its_lead_is_refused() {
    holds("turns_target", "childOf", "a child beside its lead is refused", d::child_of);
}

#[test]
fn child_of__a_child_beside_its_lead_named_by_a_row_with_no_thread_id() {
    holds("turns_target", "childOf", "a child beside its lead named by a row with no thread id", d::child_of);
}

#[test]
fn child_of__the_lead_s_own_thread_named_as_its_child() {
    holds("turns_target", "childOf", "the lead's own thread named as its child", d::child_of);
}

#[test]
fn child_of__the_lead_with_no_thread_id_is_named_by_its_row() {
    holds("turns_target", "childOf", "the lead with no thread id is named by its row", d::child_of);
}

#[test]
fn child_of__a_child_naming_no_thread() {
    holds("turns_target", "childOf", "a child naming no thread", d::child_of);
}

#[test]
fn child_of__a_child_that_is_a_project() {
    holds("turns_target", "childOf", "a child that is a project", d::child_of);
}

#[test]
fn child_of__a_child_that_is_a_project_with_the_cloud() {
    holds("turns_target", "childOf", "a child that is a project, with the cloud", d::child_of);
}

#[test]
fn child_of__a_child_that_is_a_machine_with_the_cloud() {
    holds("turns_target", "childOf", "a child that is a machine, with the cloud", d::child_of);
}

#[test]
fn child_of__a_child_that_is_a_machine_with_no_cloud() {
    holds("turns_target", "childOf", "a child that is a machine, with no cloud", d::child_of);
}

#[test]
fn child_of__a_child_the_host_refuses() {
    holds("turns_target", "childOf", "a child the host refuses", d::child_of);
}

#[test]
fn child_of__a_child_s_workspace_of_another_version() {
    holds("turns_target", "childOf", "a child's workspace of another version", d::child_of);
}

#[test]
fn sharing_with__the_others_of_the_folder_in_the_host_s_order() {
    holds("turns_target", "sharingWith", "the others of the folder, in the host's order", d::sharing_with);
}

#[test]
fn sharing_with__none_besides_itself() {
    holds("turns_target", "sharingWith", "none besides itself", d::sharing_with);
}

#[test]
fn sharing_with__none_at_all() {
    holds("turns_target", "sharingWith", "none at all", d::sharing_with);
}

#[test]
fn sharing_with__a_thread_of_another_folder_is_left_out() {
    holds("turns_target", "sharingWith", "a thread of another folder is left out", d::sharing_with);
}

#[test]
fn sharing_with__a_thread_s_turns_are_one_thread() {
    holds("turns_target", "sharingWith", "a thread's turns are one thread", d::sharing_with);
}

#[test]
fn sharing_with__a_row_with_no_thread_id_is_a_thread_of_its_own_id() {
    holds("turns_target", "sharingWith", "a row with no thread id is a thread of its own id", d::sharing_with);
}

#[test]
fn sharing_with__the_thread_asking_has_no_thread_id_its_row_id_is_the_one_lef() {
    holds("turns_target", "sharingWith", "the thread asking has no thread id: its row id is the one left out", d::sharing_with);
}

#[test]
fn sharing_with__the_listing_is_asked_for_the_workspace() {
    holds("turns_target", "sharingWith", "the listing is asked for the workspace", d::sharing_with);
}

#[test]
fn sharing_with__the_listing_refused() {
    holds("turns_target", "sharingWith", "the listing refused", d::sharing_with);
}

#[test]
fn thread_deleted__with_its_worktree_and_two_threads() {
    holds("turns_target", "threadDeleted", "with its worktree and two threads", d::thread_deleted);
}

#[test]
fn thread_deleted__a_thread_in_the_folder_nothing_else() {
    holds("turns_target", "threadDeleted", "a thread in the folder, nothing else", d::thread_deleted);
}

#[test]
fn thread_deleted__the_host_s_order_is_not_the_answer_s() {
    holds("turns_target", "threadDeleted", "the host's order is not the answer's", d::thread_deleted);
}

#[test]
fn thread_deleted__extra_fields_are_dropped() {
    holds("turns_target", "threadDeleted", "extra fields are dropped", d::thread_deleted);
}

#[test]
fn thread_deleted__the_processes_ended() {
    holds("turns_target", "threadDeleted", "the processes ended", d::thread_deleted);
}

#[test]
fn thread_deleted__processes_kept() {
    holds("turns_target", "threadDeleted", "processes kept", d::thread_deleted);
}

#[test]
fn thread_deleted__processes_asked() {
    holds("turns_target", "threadDeleted", "processes asked", d::thread_deleted);
}

#[test]
fn thread_deleted__a_check() {
    holds("turns_target", "threadDeleted", "a check", d::thread_deleted);
}

#[test]
fn thread_deleted__processes_and_a_check() {
    holds("turns_target", "threadDeleted", "processes and a check", d::thread_deleted);
}

#[test]
fn thread_deleted__an_empty_list_of_ended_is_kept() {
    holds("turns_target", "threadDeleted", "an empty list of ended is kept", d::thread_deleted);
}

#[test]
fn thread_deleted__a_sub_thread_s_machine_deleted_on_its_cloud() {
    holds("turns_target", "threadDeleted", "a sub-thread's machine deleted on its cloud", d::thread_deleted);
}

#[test]
fn thread_deleted__a_machine_word_that_is_not_deleted_is_dropped() {
    holds("turns_target", "threadDeleted", "a machine word that is not deleted is dropped", d::thread_deleted);
}

#[test]
fn thread_deleted__a_cloud_named_without_a_machine() {
    holds("turns_target", "threadDeleted", "a cloud named without a machine", d::thread_deleted);
}

#[test]
fn thread_deleted__no_threads_is_zero() {
    holds("turns_target", "threadDeleted", "no threads is zero", d::thread_deleted);
}

#[test]
fn thread_deleted__refused_for_processes() {
    holds("turns_target", "threadDeleted", "refused for processes", d::thread_deleted);
}

#[test]
fn thread_deleted__refused_for_a_worktree_with_changes() {
    holds("turns_target", "threadDeleted", "refused for a worktree with changes", d::thread_deleted);
}

#[test]
fn thread_deleted__a_sub_thread_refused_over_work_not_pushed() {
    holds("turns_target", "threadDeleted", "a sub-thread refused over work not pushed", d::thread_deleted);
}
