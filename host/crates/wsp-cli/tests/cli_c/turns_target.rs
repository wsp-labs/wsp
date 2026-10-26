// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_target.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_target as d;

#[test]
fn run_target__a_project_here_by_name() {
    holds("turns_target", "runTarget", "a project here by name", d::run_target);
}

#[test]
fn run_target__a_project_here_by_name_cloud_on() {
    holds("turns_target", "runTarget", "a project here by name, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_by_id() {
    holds("turns_target", "runTarget", "a project here by id", d::run_target);
}

#[test]
fn run_target__a_project_here_by_id_cloud_on() {
    holds("turns_target", "runTarget", "a project here by id, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_branch() {
    holds("turns_target", "runTarget", "a project here with a branch", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_branch_cloud_on() {
    holds("turns_target", "runTarget", "a project here with a branch, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_folder() {
    holds("turns_target", "runTarget", "a project here with a folder", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_folder_cloud_on() {
    holds("turns_target", "runTarget", "a project here with a folder, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_folder_and_a_branch_branch_first() {
    holds("turns_target", "runTarget", "a project here with a folder and a branch, branch first", d::run_target);
}

#[test]
fn run_target__a_project_here_with_a_folder_and_a_branch_branch_first_cloud() {
    holds("turns_target", "runTarget", "a project here with a folder and a branch, branch first, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_a_relative_folder_is_the_start_s_to_refuse() {
    holds("turns_target", "runTarget", "a project here: a relative folder is the start's to refuse", d::run_target);
}

#[test]
fn run_target__a_project_here_a_relative_folder_is_the_start_s_to_refuse_cl() {
    holds("turns_target", "runTarget", "a project here: a relative folder is the start's to refuse, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_with_an_empty_branch_and_an_empty_folder() {
    holds("turns_target", "runTarget", "a project here with an empty branch and an empty folder", d::run_target);
}

#[test]
fn run_target__a_project_here_with_an_empty_branch_and_an_empty_folder_clou() {
    holds("turns_target", "runTarget", "a project here with an empty branch and an empty folder, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_whose_second_read_is_refused_has_no_path() {
    holds("turns_target", "runTarget", "a project here whose second read is refused has no path", d::run_target);
}

#[test]
fn run_target__a_project_here_whose_second_read_is_refused_has_no_path_clou() {
    holds("turns_target", "runTarget", "a project here whose second read is refused has no path, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_a_thread_asking_the_ref_is_read_before_the_to() {
    holds("turns_target", "runTarget", "a project here, a thread asking: the ref is read before the token", d::run_target);
}

#[test]
fn run_target__a_project_here_a_thread_asking_the_ref_is_read_before_the_to_2() {
    holds("turns_target", "runTarget", "a project here, a thread asking: the ref is read before the token, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_here_a_caller_on_a_machine() {
    holds("turns_target", "runTarget", "a project here, a caller on a machine", d::run_target);
}

#[test]
fn run_target__a_project_here_a_caller_on_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "a project here, a caller on a machine, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_the_host_s_resolve_refuses_is_found_in_the_list_by() {
    holds("turns_target", "runTarget", "a project the host's resolve refuses is found in the list by name", d::run_target);
}

#[test]
fn run_target__a_project_the_host_s_resolve_refuses_is_found_in_the_list_by_2() {
    holds("turns_target", "runTarget", "a project the host's resolve refuses is found in the list by name, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_the_host_s_resolve_refuses_is_found_in_the_list_by_3() {
    holds("turns_target", "runTarget", "a project the host's resolve refuses is found in the list by id", d::run_target);
}

#[test]
fn run_target__a_project_the_host_s_resolve_refuses_is_found_in_the_list_by_4() {
    holds("turns_target", "runTarget", "a project the host's resolve refuses is found in the list by id, cloud on", d::run_target);
}

#[test]
fn run_target__two_projects_of_one_name_in_the_list_the_first() {
    holds("turns_target", "runTarget", "two projects of one name in the list: the first", d::run_target);
}

#[test]
fn run_target__two_projects_of_one_name_in_the_list_the_first_cloud_on() {
    holds("turns_target", "runTarget", "two projects of one name in the list: the first, cloud on", d::run_target);
}

#[test]
fn run_target__the_list_s_name_is_matched_before_its_id() {
    holds("turns_target", "runTarget", "the list's name is matched before its id", d::run_target);
}

#[test]
fn run_target__the_list_s_name_is_matched_before_its_id_cloud_on() {
    holds("turns_target", "runTarget", "the list's name is matched before its id, cloud on", d::run_target);
}

#[test]
fn run_target__with_the_projects_not_listable_the_thread_s_own_workspaces_n() {
    holds("turns_target", "runTarget", "with the projects not listable, the thread's own workspaces name one", d::run_target);
}

#[test]
fn run_target__with_the_projects_not_listable_the_thread_s_own_workspaces_n_2() {
    holds("turns_target", "runTarget", "with the projects not listable, the thread's own workspaces name one, cloud on", d::run_target);
}

#[test]
fn run_target__with_the_projects_not_listable_the_first_workspace_of_a_proj() {
    holds("turns_target", "runTarget", "with the projects not listable, the first workspace of a project names it", d::run_target);
}

#[test]
fn run_target__with_the_projects_not_listable_the_first_workspace_of_a_proj_2() {
    holds(
        "turns_target",
        "runTarget",
        "with the projects not listable, the first workspace of a project names it, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_on_a_computer_the_person_joined_runs_in_its_folder() {
    holds("turns_target", "runTarget", "a project on a computer the person joined runs in its folder there", d::run_target);
}

#[test]
fn run_target__a_project_on_a_computer_the_person_joined_runs_in_its_folder_2() {
    holds("turns_target", "runTarget", "a project on a computer the person joined runs in its folder there, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_elsewhere_that_opens_in_its_folder_a_branch_asked_() {
    holds("turns_target", "runTarget", "a project elsewhere that opens in its folder, a branch asked, is not refused", d::run_target);
}

#[test]
fn run_target__a_project_elsewhere_that_opens_in_its_folder_a_branch_asked__2() {
    holds(
        "turns_target",
        "runTarget",
        "a project elsewhere that opens in its folder, a branch asked, is not refused, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_elsewhere_whose_landing_is_local() {
    holds("turns_target", "runTarget", "a project elsewhere whose landing is local", d::run_target);
}

#[test]
fn run_target__a_project_elsewhere_whose_landing_is_local_cloud_on() {
    holds("turns_target", "runTarget", "a project elsewhere whose landing is local, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_forks_a_machine() {
    holds("turns_target", "runTarget", "a project on a cloud forks a machine", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_forks_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "a project on a cloud forks a machine, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_branch_is_refused() {
    holds("turns_target", "runTarget", "a project on a cloud with a branch is refused", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_branch_is_refused_cloud_on() {
    holds("turns_target", "runTarget", "a project on a cloud with a branch is refused, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_folder_forks_and_leaves_the_fold() {
    holds("turns_target", "runTarget", "a project on a cloud with a folder forks and leaves the folder to the start", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_folder_forks_and_leaves_the_fold_2() {
    holds(
        "turns_target",
        "runTarget",
        "a project on a cloud with a folder forks and leaves the folder to the start, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_whose_landing_names_no_kind_forks() {
    holds("turns_target", "runTarget", "a project whose landing names no kind forks", d::run_target);
}

#[test]
fn run_target__a_project_whose_landing_names_no_kind_forks_cloud_on() {
    holds("turns_target", "runTarget", "a project whose landing names no kind forks, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_whose_landing_is_refused_forks() {
    holds("turns_target", "runTarget", "a project whose landing is refused forks", d::run_target);
}

#[test]
fn run_target__a_project_whose_landing_is_refused_forks_cloud_on() {
    holds("turns_target", "runTarget", "a project whose landing is refused forks, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_whose_landing_is_no_op_the_host_knows_forks() {
    holds("turns_target", "runTarget", "a project whose landing is no op the host knows forks", d::run_target);
}

#[test]
fn run_target__a_project_whose_landing_is_no_op_the_host_knows_forks_cloud_() {
    holds("turns_target", "runTarget", "a project whose landing is no op the host knows forks, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_elsewhere_named_by_its_id() {
    holds("turns_target", "runTarget", "a project elsewhere named by its id", d::run_target);
}

#[test]
fn run_target__a_project_elsewhere_named_by_its_id_cloud_on() {
    holds("turns_target", "runTarget", "a project elsewhere named by its id, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_found_only_in_the_list() {
    holds("turns_target", "runTarget", "a project on a cloud found only in the list", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_found_only_in_the_list_cloud_on() {
    holds("turns_target", "runTarget", "a project on a cloud found only in the list, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_another_name_forks() {
    holds("turns_target", "runTarget", "a project on a cloud with a machine of another name forks", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_another_name_forks_cl() {
    holds("turns_target", "runTarget", "a project on a cloud with a machine of another name forks, cloud on", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_its_name_the_machine_() {
    holds(
        "turns_target",
        "runTarget",
        "a project on a cloud with a machine of its name: the machine with a cloud, the fork without",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_its_name_the_machine__2() {
    holds(
        "turns_target",
        "runTarget",
        "a project on a cloud with a machine of its name: the machine with a cloud, the fork without, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_its_name_and_a_branch() {
    holds(
        "turns_target",
        "runTarget",
        "a project on a cloud with a machine of its name and a branch: the branch is refused first",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_on_a_cloud_with_a_machine_of_its_name_and_a_branch_2() {
    holds(
        "turns_target",
        "runTarget",
        "a project on a cloud with a machine of its name and a branch: the branch is refused first, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__a_project_on_a_cloud_typed_by_id_beside_a_machine_of_that_id() {
    holds("turns_target", "runTarget", "a project on a cloud typed by id beside a machine of that id's name", d::run_target);
}

#[test]
fn run_target__a_project_on_a_cloud_typed_by_id_beside_a_machine_of_that_id_2() {
    holds("turns_target", "runTarget", "a project on a cloud typed by id beside a machine of that id's name, cloud on", d::run_target);
}

#[test]
fn run_target__the_machine_list_refused_on_a_cloud_project_is_the_answer_wi() {
    holds("turns_target", "runTarget", "the machine list refused on a cloud project is the answer with a cloud", d::run_target);
}

#[test]
fn run_target__the_machine_list_refused_on_a_cloud_project_is_the_answer_wi_2() {
    holds("turns_target", "runTarget", "the machine list refused on a cloud project is the answer with a cloud, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing() {
    holds("turns_target", "runTarget", "a word that names nothing", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_cloud_on() {
    holds("turns_target", "runTarget", "a word that names nothing, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_host_s_own_refusal_on_the_seco() {
    holds("turns_target", "runTarget", "a word that names nothing, the host's own refusal on the second read", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_host_s_own_refusal_on_the_seco_2() {
    holds("turns_target", "runTarget", "a word that names nothing, the host's own refusal on the second read, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_answers() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read answers", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_answers_cloud_on() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read answers, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_refused_of_another() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read refused of another kind", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_refused_of_another_2() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read refused of another kind, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_refused_with_no_ki() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read refused with no kind", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_the_second_read_refused_with_no_ki_2() {
    holds("turns_target", "runTarget", "a word that names nothing, the second read refused with no kind, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_and_a_list_that_lists_nothing() {
    holds("turns_target", "runTarget", "a word that names nothing and a list that lists nothing", d::run_target);
}

#[test]
fn run_target__a_word_that_names_nothing_and_a_list_that_lists_nothing_clou() {
    holds("turns_target", "runTarget", "a word that names nothing and a list that lists nothing, cloud on", d::run_target);
}

#[test]
fn run_target__an_empty_word_names_nothing() {
    holds("turns_target", "runTarget", "an empty word names nothing", d::run_target);
}

#[test]
fn run_target__an_empty_word_names_nothing_cloud_on() {
    holds("turns_target", "runTarget", "an empty word names nothing, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine() {
    holds("turns_target", "runTarget", "a word naming a machine", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "a word naming a machine, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_with_a_branch() {
    holds("turns_target", "runTarget", "a word naming a machine, with a branch", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_with_a_branch_cloud_on() {
    holds("turns_target", "runTarget", "a word naming a machine, with a branch, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_with_a_folder() {
    holds("turns_target", "runTarget", "a word naming a machine, with a folder", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_with_a_folder_cloud_on() {
    holds("turns_target", "runTarget", "a word naming a machine, with a folder, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_of_a_shape_this_wsp_does_not_read() {
    holds("turns_target", "runTarget", "a word naming a machine of a shape this wsp does not read", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_of_a_shape_this_wsp_does_not_read_cl() {
    holds("turns_target", "runTarget", "a word naming a machine of a shape this wsp does not read, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_the_host_refuses() {
    holds("turns_target", "runTarget", "a word naming a machine the host refuses", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_machine_the_host_refuses_cloud_on() {
    holds("turns_target", "runTarget", "a word naming a machine the host refuses, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_project_hidden_from_the_thread() {
    holds("turns_target", "runTarget", "a word naming a project hidden from the thread", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_project_hidden_from_the_thread_cloud_on() {
    holds("turns_target", "runTarget", "a word naming a project hidden from the thread, cloud on", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_project_hidden_from_the_thread_the_list_refu() {
    holds("turns_target", "runTarget", "a word naming a project hidden from the thread, the list refused too", d::run_target);
}

#[test]
fn run_target__a_word_naming_a_project_hidden_from_the_thread_the_list_refu_2() {
    holds("turns_target", "runTarget", "a word naming a project hidden from the thread, the list refused too, cloud on", d::run_target);
}

#[test]
fn run_target__run_beside_starts_beside_the_thread_whose_line_it_is_a_line_() {
    holds(
        "turns_target",
        "runTarget",
        "run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (no folder named)",
        d::run_target,
    );
}

#[test]
fn run_target__run_beside_starts_beside_the_thread_whose_line_it_is_a_line__2() {
    holds(
        "turns_target",
        "runTarget",
        "run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (a folder named)",
        d::run_target,
    );
}

#[test]
fn run_target__run_beside_still_names_the_workspace_on_a_line_from_a_machin() {
    holds("turns_target", "runTarget", "run-beside: still names the workspace on a line from a machine that is no thread's", d::run_target);
}

#[test]
fn run_target__run_beside_takes_the_thread_s_own_project_where_a_project_on() {
    holds(
        "turns_target",
        "runTarget",
        "run-beside: takes the thread's own project where a project on another computer shares its name",
        d::run_target,
    );
}

#[test]
fn run_target__no_project_a_turn_token_beside_the_thread_asking_nothing_is_() {
    holds("turns_target", "runTarget", "no project, a turn token: beside the thread asking, nothing is asked", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_beside_the_thread_asking_nothing_is__2() {
    holds("turns_target", "runTarget", "no project, a turn token: beside the thread asking, nothing is asked, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_and_a_folder_and_a_branch() {
    holds("turns_target", "runTarget", "no project, a turn token and a folder and a branch", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_and_a_folder_and_a_branch_cloud_on() {
    holds("turns_target", "runTarget", "no project, a turn token and a folder and a branch, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_on_a_caller_on_a_machine() {
    holds("turns_target", "runTarget", "no project, a turn token on a caller on a machine", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_on_a_caller_on_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "no project, a turn token on a caller on a machine, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_and_no_folder_of_the_caller_s() {
    holds("turns_target", "runTarget", "no project, a turn token and no folder of the caller's", d::run_target);
}

#[test]
fn run_target__no_project_a_turn_token_and_no_folder_of_the_caller_s_cloud_() {
    holds("turns_target", "runTarget", "no project, a turn token and no folder of the caller's, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_beside_the_thread_asking_nothing_is_() {
    holds("turns_target", "runTarget", "no project, a host token: beside the thread asking, nothing is asked", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_beside_the_thread_asking_nothing_is__2() {
    holds("turns_target", "runTarget", "no project, a host token: beside the thread asking, nothing is asked, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_and_a_folder_and_a_branch() {
    holds("turns_target", "runTarget", "no project, a host token and a folder and a branch", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_and_a_folder_and_a_branch_cloud_on() {
    holds("turns_target", "runTarget", "no project, a host token and a folder and a branch, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_on_a_caller_on_a_machine() {
    holds("turns_target", "runTarget", "no project, a host token on a caller on a machine", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_on_a_caller_on_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "no project, a host token on a caller on a machine, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_and_no_folder_of_the_caller_s() {
    holds("turns_target", "runTarget", "no project, a host token and no folder of the caller's", d::run_target);
}

#[test]
fn run_target__no_project_a_host_token_and_no_folder_of_the_caller_s_cloud_() {
    holds("turns_target", "runTarget", "no project, a host token and no folder of the caller's, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_beside_the_thread_asking_nothing_is_a() {
    holds("turns_target", "runTarget", "no project, both tokens: beside the thread asking, nothing is asked", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_beside_the_thread_asking_nothing_is_a_2() {
    holds("turns_target", "runTarget", "no project, both tokens: beside the thread asking, nothing is asked, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_and_a_folder_and_a_branch() {
    holds("turns_target", "runTarget", "no project, both tokens and a folder and a branch", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_and_a_folder_and_a_branch_cloud_on() {
    holds("turns_target", "runTarget", "no project, both tokens and a folder and a branch, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_on_a_caller_on_a_machine() {
    holds("turns_target", "runTarget", "no project, both tokens on a caller on a machine", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_on_a_caller_on_a_machine_cloud_on() {
    holds("turns_target", "runTarget", "no project, both tokens on a caller on a machine, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_and_no_folder_of_the_caller_s() {
    holds("turns_target", "runTarget", "no project, both tokens and no folder of the caller's", d::run_target);
}

#[test]
fn run_target__no_project_both_tokens_and_no_folder_of_the_caller_s_cloud_o() {
    holds("turns_target", "runTarget", "no project, both tokens and no folder of the caller's, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_turn_token_is_no_token() {
    holds("turns_target", "runTarget", "no project, an empty turn token is no token", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_turn_token_is_no_token_cloud_on() {
    holds("turns_target", "runTarget", "no project, an empty turn token is no token, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_host_token_is_no_token() {
    holds("turns_target", "runTarget", "no project, an empty host token is no token", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_host_token_is_no_token_cloud_on() {
    holds("turns_target", "runTarget", "no project, an empty host token is no token, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_turn_token_on_a_caller_on_a_machine() {
    holds("turns_target", "runTarget", "no project, an empty turn token on a caller on a machine", d::run_target);
}

#[test]
fn run_target__no_project_an_empty_turn_token_on_a_caller_on_a_machine_clou() {
    holds("turns_target", "runTarget", "no project, an empty turn token on a caller on a machine, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_other_variables_are_not_tokens() {
    holds("turns_target", "runTarget", "no project, other variables are not tokens", d::run_target);
}

#[test]
fn run_target__no_project_other_variables_are_not_tokens_cloud_on() {
    holds("turns_target", "runTarget", "no project, other variables are not tokens, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_caller_on_a_machine_with_no_token_is_told_to_na() {
    holds(
        "turns_target",
        "runTarget",
        "no project, a caller on a machine with no token is told to name the workspace, and nothing is asked",
        d::run_target,
    );
}

#[test]
fn run_target__no_project_a_caller_on_a_machine_with_no_token_is_told_to_na_2() {
    holds(
        "turns_target",
        "runTarget",
        "no project, a caller on a machine with no token is told to name the workspace, and nothing is asked, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__no_project_a_caller_on_a_machine_no_folder() {
    holds("turns_target", "runTarget", "no project, a caller on a machine, no folder", d::run_target);
}

#[test]
fn run_target__no_project_a_caller_on_a_machine_no_folder_cloud_on() {
    holds("turns_target", "runTarget", "no project, a caller on a machine, no folder, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_caller_on_this_computer_with_no_folder() {
    holds("turns_target", "runTarget", "no project, a caller on this computer with no folder", d::run_target);
}

#[test]
fn run_target__no_project_a_caller_on_this_computer_with_no_folder_cloud_on() {
    holds("turns_target", "runTarget", "no project, a caller on this computer with no folder, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_elsewhere_false_is_this_computer() {
    holds("turns_target", "runTarget", "no project, elsewhere false is this computer", d::run_target);
}

#[test]
fn run_target__no_project_elsewhere_false_is_this_computer_cloud_on() {
    holds("turns_target", "runTarget", "no project, elsewhere false is this computer, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_project_s_folder() {
    holds("turns_target", "runTarget", "no project, the caller in a project's folder", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_project_s_folder_cloud_on() {
    holds("turns_target", "runTarget", "no project, the caller in a project's folder, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project_cloud_on() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project_with_a_bran() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project with a branch", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project_with_a_bran_2() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project with a branch, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project_with_a_fold() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project with a folder named", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_in_a_folder_of_the_project_with_a_fold_2() {
    holds("turns_target", "runTarget", "no project, the caller in a folder of the project with a folder named, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_s_folder_with_a_trailing_slash_of_the_() {
    holds("turns_target", "runTarget", "no project, the caller's folder with a trailing slash of the project's", d::run_target);
}

#[test]
fn run_target__no_project_the_caller_s_folder_with_a_trailing_slash_of_the__2() {
    holds("turns_target", "runTarget", "no project, the caller's folder with a trailing slash of the project's, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_that_only_shares_the_project_s_name_as_a() {
    holds("turns_target", "runTarget", "no project, a folder that only shares the project's name as a prefix is not in it", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_that_only_shares_the_project_s_name_as_a_2() {
    holds(
        "turns_target",
        "runTarget",
        "no project, a folder that only shares the project's name as a prefix is not in it, cloud on",
        d::run_target,
    );
}

#[test]
fn run_target__no_project_the_deepest_project_holding_the_folder_wins() {
    holds("turns_target", "runTarget", "no project, the deepest project holding the folder wins", d::run_target);
}

#[test]
fn run_target__no_project_the_deepest_project_holding_the_folder_wins_cloud() {
    holds("turns_target", "runTarget", "no project, the deepest project holding the folder wins, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_two_projects_at_one_path_the_first_listed() {
    holds("turns_target", "runTarget", "no project, two projects at one path: the first listed", d::run_target);
}

#[test]
fn run_target__no_project_two_projects_at_one_path_the_first_listed_cloud_o() {
    holds("turns_target", "runTarget", "no project, two projects at one path: the first listed, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_project_on_another_computer_never_holds_the_fol() {
    holds("turns_target", "runTarget", "no project, a project on another computer never holds the folder", d::run_target);
}

#[test]
fn run_target__no_project_a_project_on_another_computer_never_holds_the_fol_2() {
    holds("turns_target", "runTarget", "no project, a project on another computer never holds the folder, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_in_none_of_the_projects() {
    holds("turns_target", "runTarget", "no project, a folder in none of the projects", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_in_none_of_the_projects_cloud_on() {
    holds("turns_target", "runTarget", "no project, a folder in none of the projects, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_no_projects_at_all() {
    holds("turns_target", "runTarget", "no project, no projects at all", d::run_target);
}

#[test]
fn run_target__no_project_no_projects_at_all_cloud_on() {
    holds("turns_target", "runTarget", "no project, no projects at all, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_the_projects_not_listable() {
    holds("turns_target", "runTarget", "no project, the projects not listable", d::run_target);
}

#[test]
fn run_target__no_project_the_projects_not_listable_cloud_on() {
    holds("turns_target", "runTarget", "no project, the projects not listable, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_of_the_project_s_repo_carries_its_own_() {
    holds("turns_target", "runTarget", "no project, a worktree of the project's repo carries its own folder", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_of_the_project_s_repo_carries_its_own__2() {
    holds("turns_target", "runTarget", "no project, a worktree of the project's repo carries its own folder, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_with_a_branch_asked_leaves_the_folder_() {
    holds("turns_target", "runTarget", "no project, a worktree with a branch asked leaves the folder off", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_with_a_branch_asked_leaves_the_folder__2() {
    holds("turns_target", "runTarget", "no project, a worktree with a branch asked leaves the folder off, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_with_a_folder_asked_leaves_the_caller_() {
    holds("turns_target", "runTarget", "no project, a worktree with a folder asked leaves the caller's off", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_with_a_folder_asked_leaves_the_caller__2() {
    holds("turns_target", "runTarget", "no project, a worktree with a folder asked leaves the caller's off, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_of_a_worktree_of_the_project_s_repo() {
    holds("turns_target", "runTarget", "no project, a folder of a worktree of the project's repo", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_of_a_worktree_of_the_project_s_repo_clou() {
    holds("turns_target", "runTarget", "no project, a folder of a worktree of the project's repo, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_of_a_repo_no_project_holds() {
    holds("turns_target", "runTarget", "no project, a worktree of a repo no project holds", d::run_target);
}

#[test]
fn run_target__no_project_a_worktree_of_a_repo_no_project_holds_cloud_on() {
    holds("turns_target", "runTarget", "no project, a worktree of a repo no project holds, cloud on", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_with_no_repository() {
    holds("turns_target", "runTarget", "no project, a folder with no repository", d::run_target);
}

#[test]
fn run_target__no_project_a_folder_with_no_repository_cloud_on() {
    holds("turns_target", "runTarget", "no project, a folder with no repository, cloud on", d::run_target);
}

#[test]
fn project_of_folder__no_projects() {
    holds("turns_target", "projectOfFolder", "no projects", d::project_of_folder);
}

#[test]
fn project_of_folder__the_folder_is_the_project_s() {
    holds("turns_target", "projectOfFolder", "the folder is the project's", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_inside_it() {
    holds("turns_target", "projectOfFolder", "a folder inside it", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_beside_it() {
    holds("turns_target", "projectOfFolder", "a folder beside it", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_above_it() {
    holds("turns_target", "projectOfFolder", "a folder above it", d::project_of_folder);
}

#[test]
fn project_of_folder__the_project_s_path_with_a_trailing_slash() {
    holds("turns_target", "projectOfFolder", "the project's path with a trailing slash", d::project_of_folder);
}

#[test]
fn project_of_folder__the_project_s_path_with_a_trailing_slash_the_folder_is_the_p() {
    holds(
        "turns_target",
        "projectOfFolder",
        "the project's path with a trailing slash, the folder is the path without it",
        d::project_of_folder,
    );
}

#[test]
fn project_of_folder__the_deepest_of_two_holding_projects_wins() {
    holds("turns_target", "projectOfFolder", "the deepest of two holding projects wins", d::project_of_folder);
}

#[test]
fn project_of_folder__the_deepest_wins_whichever_is_listed_first() {
    holds("turns_target", "projectOfFolder", "the deepest wins whichever is listed first", d::project_of_folder);
}

#[test]
fn project_of_folder__two_at_one_path_the_first_listed() {
    holds("turns_target", "projectOfFolder", "two at one path: the first listed", d::project_of_folder);
}

#[test]
fn project_of_folder__a_project_on_another_computer_never_holds_a_folder() {
    holds("turns_target", "projectOfFolder", "a project on another computer never holds a folder", d::project_of_folder);
}

#[test]
fn project_of_folder__a_project_on_another_computer_beside_one_here() {
    holds("turns_target", "projectOfFolder", "a project on another computer beside one here", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_of_the_repo() {
    holds("turns_target", "projectOfFolder", "a worktree of the repo", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_whose_git_file_carries_other_lines() {
    holds("turns_target", "projectOfFolder", "a worktree whose git file carries other lines", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_deep_in_a_worktree() {
    holds("turns_target", "projectOfFolder", "a folder deep in a worktree", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_of_a_repo_whose_project_is_on_another_computer() {
    holds("turns_target", "projectOfFolder", "a worktree of a repo whose project is on another computer", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_of_a_repo_no_project_s_top_is() {
    holds("turns_target", "projectOfFolder", "a worktree of a repo no project's top is", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_and_a_project_with_no_git_record() {
    holds("turns_target", "projectOfFolder", "a worktree and a project with no git record", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_whose_git_file_has_no_gitdir_line() {
    holds("turns_target", "projectOfFolder", "a worktree whose git file has no gitdir line", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_whose_git_dir_is_not_under_git_worktrees() {
    holds("turns_target", "projectOfFolder", "a worktree whose git dir is not under .git/worktrees", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_whose_git_dir_is_a_plain_folder_named_worktrees() {
    holds("turns_target", "projectOfFolder", "a worktree whose git dir is a plain folder named worktrees", d::project_of_folder);
}

#[test]
fn project_of_folder__a_worktree_with_a_relative_gitdir() {
    holds("turns_target", "projectOfFolder", "a worktree with a relative gitdir", d::project_of_folder);
}

#[test]
fn project_of_folder__the_repository_s_own_folder_above_a_project_in_it() {
    holds("turns_target", "projectOfFolder", "the repository's own folder above a project in it", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_in_the_repository_beside_a_project_in_it() {
    holds("turns_target", "projectOfFolder", "a folder in the repository beside a project in it", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_in_a_repository_no_project_is_in() {
    holds("turns_target", "projectOfFolder", "a folder in a repository no project is in", d::project_of_folder);
}

#[test]
fn project_of_folder__a_folder_with_no_repository_above_it() {
    holds("turns_target", "projectOfFolder", "a folder with no repository above it", d::project_of_folder);
}

#[test]
fn fork_for__a_machine_named_off_the_task() {
    holds("turns_target", "forkFor", "a machine named off the task", d::fork_for);
}

#[test]
fn fork_for__the_same_task_again_is_numbered() {
    holds("turns_target", "forkFor", "the same task again is numbered", d::fork_for);
}

#[test]
fn fork_for__the_number_goes_up_past_the_taken_ones() {
    holds("turns_target", "forkFor", "the number goes up past the taken ones", d::fork_for);
}

#[test]
fn fork_for__a_name_taken_in_other_words_is_not_taken() {
    holds("turns_target", "forkFor", "a name taken in other words is not taken", d::fork_for);
}

#[test]
fn fork_for__the_name_is_cut_at_forty_characters_where_the_cut_lands_on_a() {
    holds("turns_target", "forkFor", "the name is cut at forty characters, where the cut lands on a space", d::fork_for);
}

#[test]
fn fork_for__the_cut_name_taken_is_numbered() {
    holds("turns_target", "forkFor", "the cut name taken is numbered", d::fork_for);
}

#[test]
fn fork_for__a_name_cut_at_exactly_forty_characters() {
    holds("turns_target", "forkFor", "a name cut at exactly forty characters", d::fork_for);
}

#[test]
fn fork_for__a_task_of_five_words_and_a_sixth() {
    holds("turns_target", "forkFor", "a task of five words and a sixth", d::fork_for);
}

#[test]
fn fork_for__only_the_first_line_names_it() {
    holds("turns_target", "forkFor", "only the first line names it", d::fork_for);
}

#[test]
fn fork_for__blank_lines_before_the_first_are_skipped_by_the_trim() {
    holds("turns_target", "forkFor", "blank lines before the first are skipped by the trim", d::fork_for);
}

#[test]
fn fork_for__a_single_word() {
    holds("turns_target", "forkFor", "a single word", d::fork_for);
}

#[test]
fn fork_for__an_empty_task_names_an_empty_machine() {
    holds("turns_target", "forkFor", "an empty task names an empty machine", d::fork_for);
}

#[test]
fn fork_for__an_empty_task_beside_a_taken_empty_name() {
    holds("turns_target", "forkFor", "an empty task beside a taken empty name", d::fork_for);
}

#[test]
fn fork_for__an_accented_task() {
    holds("turns_target", "forkFor", "an accented task", d::fork_for);
}

#[test]
fn fork_for__tabs_between_the_words() {
    holds("turns_target", "forkFor", "tabs between the words", d::fork_for);
}

#[test]
fn fork_for__the_stages_go_to_say_as_they_arrive() {
    holds("turns_target", "forkFor", "the stages go to say as they arrive", d::fork_for);
}

#[test]
fn fork_for__a_stage_of_another_machine_is_not_said() {
    holds("turns_target", "forkFor", "a stage of another machine is not said", d::fork_for);
}

#[test]
fn fork_for__a_frame_that_is_no_stage_is_not_said() {
    holds("turns_target", "forkFor", "a frame that is no stage is not said", d::fork_for);
}

#[test]
fn fork_for__a_stage_heard_while_the_events_are_subscribed_is_said_when_t() {
    holds("turns_target", "forkFor", "a stage heard while the events are subscribed is said when the follow starts", d::fork_for);
}

#[test]
fn fork_for__a_stage_heard_after_the_answer_is_not_said() {
    holds("turns_target", "forkFor", "a stage heard after the answer is not said", d::fork_for);
}

#[test]
fn fork_for__a_notice_rides_the_created_line() {
    holds("turns_target", "forkFor", "a notice rides the created line", d::fork_for);
}

#[test]
fn fork_for__a_notice_of_two_lines_stays_one_said_line() {
    holds("turns_target", "forkFor", "a notice of two lines stays one said line", d::fork_for);
}

#[test]
fn fork_for__a_created_machine_with_a_folder_says_that_folder() {
    holds("turns_target", "forkFor", "a created machine with a folder says that folder", d::fork_for);
}

#[test]
fn fork_for__a_project_on_this_computer_forks_nothing_and_is_asked_no_siz() {
    holds("turns_target", "forkFor", "a project on this computer forks nothing and is asked no size", d::fork_for);
}

#[test]
fn fork_for__the_landing_refused_costs_no_stage_and_no_machine() {
    holds("turns_target", "forkFor", "the landing refused costs no stage and no machine", d::fork_for);
}

#[test]
fn fork_for__the_machine_list_refused() {
    holds("turns_target", "forkFor", "the machine list refused", d::fork_for);
}

#[test]
fn fork_for__the_create_refused() {
    holds("turns_target", "forkFor", "the create refused", d::fork_for);
}

#[test]
fn fork_for__the_create_refused_after_a_stage() {
    holds("turns_target", "forkFor", "the create refused after a stage", d::fork_for);
}

#[test]
fn sub_for__named_off_the_task() {
    holds("turns_target", "subFor", "named off the task", d::sub_for);
}

#[test]
fn sub_for__named_by_its_title() {
    holds("turns_target", "subFor", "named by its title", d::sub_for);
}

#[test]
fn sub_for__the_title_is_not_cut() {
    holds("turns_target", "subFor", "the title is not cut", d::sub_for);
}

#[test]
fn sub_for__an_empty_title_is_a_title() {
    holds("turns_target", "subFor", "an empty title is a title", d::sub_for);
}

#[test]
fn sub_for__a_title_taken_is_numbered() {
    holds("turns_target", "subFor", "a title taken is numbered", d::sub_for);
}

#[test]
fn sub_for__a_name_taken_is_numbered() {
    holds("turns_target", "subFor", "a name taken is numbered", d::sub_for);
}

#[test]
fn sub_for__the_numbers_up_to_a_gap() {
    holds("turns_target", "subFor", "the numbers up to a gap", d::sub_for);
}

#[test]
fn sub_for__the_project_goes_as_its_own_id() {
    holds("turns_target", "subFor", "the project goes as its own id", d::sub_for);
}

#[test]
fn sub_for__no_project_goes_as_an_empty_word() {
    holds("turns_target", "subFor", "no project goes as an empty word", d::sub_for);
}

#[test]
fn sub_for__every_field_of_the_ask_in_the_frame_s_order_whatever_the_ask() {
    holds("turns_target", "subFor", "every field of the ask, in the frame's order whatever the ask's", d::sub_for);
}

#[test]
fn sub_for__a_size_alone() {
    holds("turns_target", "subFor", "a size alone", d::sub_for);
}

#[test]
fn sub_for__a_base_alone() {
    holds("turns_target", "subFor", "a base alone", d::sub_for);
}

#[test]
fn sub_for__a_branch_alone() {
    holds("turns_target", "subFor", "a branch alone", d::sub_for);
}

#[test]
fn sub_for__a_harness_alone() {
    holds("turns_target", "subFor", "a harness alone", d::sub_for);
}

#[test]
fn sub_for__empty_words_are_carried_as_they_are() {
    holds("turns_target", "subFor", "empty words are carried as they are", d::sub_for);
}

#[test]
fn sub_for__the_stages_go_to_say_before_the_answer() {
    holds("turns_target", "subFor", "the stages go to say before the answer", d::sub_for);
}

#[test]
fn sub_for__a_stage_of_another_machine_is_not_said() {
    holds("turns_target", "subFor", "a stage of another machine is not said", d::sub_for);
}

#[test]
fn sub_for__a_frame_that_is_no_stage_is_not_said() {
    holds("turns_target", "subFor", "a frame that is no stage is not said", d::sub_for);
}

#[test]
fn sub_for__a_stage_s_own_message_is_said_whole_not_trimmed() {
    holds("turns_target", "subFor", "a stage's own message is said whole, not trimmed", d::sub_for);
}

#[test]
fn sub_for__a_stage_heard_while_the_events_are_subscribed_is_said() {
    holds("turns_target", "subFor", "a stage heard while the events are subscribed is said", d::sub_for);
}

#[test]
fn sub_for__a_stage_after_the_answer_is_said_until_stop() {
    holds("turns_target", "subFor", "a stage after the answer is said until stop", d::sub_for);
}

#[test]
fn sub_for__a_stage_after_the_answer_is_not_said_once_stop_is_called() {
    holds("turns_target", "subFor", "a stage after the answer is not said once stop is called", d::sub_for);
}

#[test]
fn sub_for__the_notice_is_said_a_line_at_a_time() {
    holds("turns_target", "subFor", "the notice is said a line at a time", d::sub_for);
}

#[test]
fn sub_for__a_notice_of_one_line() {
    holds("turns_target", "subFor", "a notice of one line", d::sub_for);
}
