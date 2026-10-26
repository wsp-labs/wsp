// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from ws_rows.json: one test per recorded case.
use super::common::holds;
use super::drivers_ws_rows as d;

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
