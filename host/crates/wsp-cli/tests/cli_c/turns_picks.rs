// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from turns_picks.json: one test per recorded case.
use super::common::holds;
use super::drivers_turns_picks as d;

#[test]
fn picks_of__nothing_named() {
    holds("turns_picks", "picksOf", "nothing named", d::picks_of);
}

#[test]
fn picks_of__a_model() {
    holds("turns_picks", "picksOf", "a model", d::picks_of);
}

#[test]
fn picks_of__an_effort() {
    holds("turns_picks", "picksOf", "an effort", d::picks_of);
}

#[test]
fn picks_of__an_empty_model_is_a_model() {
    holds("turns_picks", "picksOf", "an empty model is a model", d::picks_of);
}

#[test]
fn picks_of__an_empty_effort_is_an_effort() {
    holds("turns_picks", "picksOf", "an empty effort is an effort", d::picks_of);
}

#[test]
fn picks_of__access_ask() {
    holds("turns_picks", "picksOf", "access ask", d::picks_of);
}

#[test]
fn picks_of__access_auto_edit() {
    holds("turns_picks", "picksOf", "access auto-edit", d::picks_of);
}

#[test]
fn picks_of__access_full() {
    holds("turns_picks", "picksOf", "access full", d::picks_of);
}

#[test]
fn picks_of__access_plan() {
    holds("turns_picks", "picksOf", "access plan", d::picks_of);
}

#[test]
fn picks_of__fast() {
    holds("turns_picks", "picksOf", "fast", d::picks_of);
}

#[test]
fn picks_of__fast_false_is_left_out() {
    holds("turns_picks", "picksOf", "fast false is left out", d::picks_of);
}

#[test]
fn picks_of__all_four_in_the_order_model_effort_access_fast() {
    holds("turns_picks", "picksOf", "all four, in the order model effort access fast", d::picks_of);
}

#[test]
fn picks_of__an_access_that_is_no_word_of_wsp_s() {
    holds("turns_picks", "picksOf", "an access that is no word of wsp's", d::picks_of);
}

#[test]
fn picks_of__an_access_in_the_harness_s_own_spelling() {
    holds("turns_picks", "picksOf", "an access in the harness's own spelling", d::picks_of);
}

#[test]
fn picks_of__an_empty_access() {
    holds("turns_picks", "picksOf", "an empty access", d::picks_of);
}

#[test]
fn picks_of__an_access_in_capitals() {
    holds("turns_picks", "picksOf", "an access in capitals", d::picks_of);
}

#[test]
fn picks_of__a_bad_access_beside_a_good_model() {
    holds("turns_picks", "picksOf", "a bad access beside a good model", d::picks_of);
}

#[test]
fn pick_flags__no_flags() {
    holds("turns_picks", "pickFlags", "no flags", d::pick_flags);
}

#[test]
fn pick_flags__model_effort_access() {
    holds("turns_picks", "pickFlags", "model effort access", d::pick_flags);
}

#[test]
fn pick_flags__typed_in_the_other_order_read_in_the_table_s() {
    holds("turns_picks", "pickFlags", "typed in the other order, read in the table's", d::pick_flags);
}

#[test]
fn pick_flags__an_empty_model_is_kept() {
    holds("turns_picks", "pickFlags", "an empty model is kept", d::pick_flags);
}

#[test]
fn pick_flags__a_model_flag_given_as_a_switch_is_no_value() {
    holds("turns_picks", "pickFlags", "a model flag given as a switch is no value", d::pick_flags);
}

#[test]
fn pick_flags__a_model_flag_given_as_a_list_is_no_value() {
    holds("turns_picks", "pickFlags", "a model flag given as a list is no value", d::pick_flags);
}

#[test]
fn pick_flags__fast() {
    holds("turns_picks", "pickFlags", "fast", d::pick_flags);
}

#[test]
fn pick_flags__fast_typed_first_still_comes_last() {
    holds("turns_picks", "pickFlags", "fast typed first still comes last", d::pick_flags);
}

#[test]
fn pick_flags__fast_false_is_dropped() {
    holds("turns_picks", "pickFlags", "fast false is dropped", d::pick_flags);
}

#[test]
fn pick_flags__fast_as_a_word_is_dropped() {
    holds("turns_picks", "pickFlags", "fast as a word is dropped", d::pick_flags);
}

#[test]
fn pick_flags__fast_as_a_list_is_dropped() {
    holds("turns_picks", "pickFlags", "fast as a list is dropped", d::pick_flags);
}

#[test]
fn pick_flags__other_flags_are_not_picks() {
    holds("turns_picks", "pickFlags", "other flags are not picks", d::pick_flags);
}

#[test]
fn pick_flags__everything_and_an_unrelated_flag() {
    holds("turns_picks", "pickFlags", "everything and an unrelated flag", d::pick_flags);
}

#[test]
fn files_from__none() {
    holds("turns_picks", "filesFrom", "none", d::files_from);
}

#[test]
fn files_from__none_elsewhere() {
    holds("turns_picks", "filesFrom", "none, elsewhere", d::files_from);
}

#[test]
fn files_from__one_text_file_travels_untyped_under_its_own_name() {
    holds("turns_picks", "filesFrom", "one text file travels untyped under its own name", d::files_from);
}

#[test]
fn files_from__two_files_keep_the_order_named() {
    holds("turns_picks", "filesFrom", "two files keep the order named", d::files_from);
}

#[test]
fn files_from__the_same_file_twice_is_two_attachments() {
    holds("turns_picks", "filesFrom", "the same file twice is two attachments", d::files_from);
}

#[test]
fn files_from__a_name_with_spaces_and_accents() {
    holds("turns_picks", "filesFrom", "a name with spaces and accents", d::files_from);
}

#[test]
fn files_from__a_path_with_dot_dot_is_the_file_it_reaches() {
    holds("turns_picks", "filesFrom", "a path with dot dot is the file it reaches", d::files_from);
}

#[test]
fn files_from__a_path_with_a_doubled_slash() {
    holds("turns_picks", "filesFrom", "a path with a doubled slash", d::files_from);
}

#[test]
fn files_from__a_one_byte_file() {
    holds("turns_picks", "filesFrom", "a one byte file", d::files_from);
}

#[test]
fn files_from__an_empty_file_is_refused_by_name() {
    holds("turns_picks", "filesFrom", "an empty file is refused by name", d::files_from);
}

#[test]
fn files_from__a_file_that_is_not_there() {
    holds("turns_picks", "filesFrom", "a file that is not there", d::files_from);
}

#[test]
fn files_from__a_relative_path_that_is_not_there() {
    holds("turns_picks", "filesFrom", "a relative path that is not there", d::files_from);
}

#[test]
fn files_from__a_folder_is_no_file() {
    holds("turns_picks", "filesFrom", "a folder is no file", d::files_from);
}

#[test]
fn files_from__the_case_folder_itself_is_no_file() {
    holds("turns_picks", "filesFrom", "the case folder itself is no file", d::files_from);
}

#[test]
fn files_from__the_first_missing_file_is_the_one_named() {
    holds("turns_picks", "filesFrom", "the first missing file is the one named", d::files_from);
}

#[test]
fn files_from__a_missing_file_beside_an_empty_one_the_missing_one_first() {
    holds("turns_picks", "filesFrom", "a missing file beside an empty one: the missing one first", d::files_from);
}

#[test]
fn files_from__elsewhere_refuses_before_any_path_is_touched() {
    holds("turns_picks", "filesFrom", "elsewhere refuses before any path is touched", d::files_from);
}

#[test]
fn files_from__elsewhere_refuses_a_path_that_is_not_there_in_the_same_words() {
    holds("turns_picks", "filesFrom", "elsewhere refuses a path that is not there in the same words", d::files_from);
}

#[test]
fn files_from__elsewhere_refuses_a_folder_in_the_same_words() {
    holds("turns_picks", "filesFrom", "elsewhere refuses a folder in the same words", d::files_from);
}

#[test]
fn files_from__elsewhere_false_reads() {
    holds("turns_picks", "filesFrom", "elsewhere false reads", d::files_from);
}

#[test]
fn files_from__1_files_fit() {
    holds("turns_picks", "filesFrom", "1 files fit", d::files_from);
}

#[test]
fn files_from__4_files_fit() {
    holds("turns_picks", "filesFrom", "4 files fit", d::files_from);
}

#[test]
fn files_from__5_files_fit() {
    holds("turns_picks", "filesFrom", "5 files fit", d::files_from);
}

#[test]
fn files_from__six_files_are_one_too_many() {
    holds("turns_picks", "filesFrom", "six files are one too many", d::files_from);
}

#[test]
fn files_from__seven_files_say_seven() {
    holds("turns_picks", "filesFrom", "seven files say seven", d::files_from);
}

#[test]
fn files_from__six_files_with_one_empty_the_count_is_the_first_thing_wrong() {
    holds("turns_picks", "filesFrom", "six files with one empty: the count is the first thing wrong", d::files_from);
}

#[test]
fn files_from__six_paths_with_one_missing_the_missing_file_is_refused_befor() {
    holds("turns_picks", "filesFrom", "six paths with one missing: the missing file is refused before the count", d::files_from);
}

#[test]
fn files_from__two_empty_files_the_first_is_the_one_named() {
    holds("turns_picks", "filesFrom", "two empty files: the first is the one named", d::files_from);
}

#[test]
fn files_from__a_file_at_the_cap_of_10_mib_travels() {
    holds("turns_picks", "filesFrom", "a file at the cap of 10 MiB travels", d::files_from);
}

#[test]
fn files_from__a_file_one_byte_under_the_cap_travels() {
    holds("turns_picks", "filesFrom", "a file one byte under the cap travels", d::files_from);
}

#[test]
fn files_from__a_file_one_byte_over_the_cap_is_refused_with_its_weight() {
    holds("turns_picks", "filesFrom", "a file one byte over the cap is refused with its weight", d::files_from);
}

#[test]
fn files_from__a_file_far_over_the_cap() {
    holds("turns_picks", "filesFrom", "a file far over the cap", d::files_from);
}

#[test]
fn files_from__a_file_of_1_5_mib_over_the_cap() {
    holds("turns_picks", "filesFrom", "a file of 1.5 MiB over the cap", d::files_from);
}

#[test]
fn files_from__the_over_cap_file_is_refused_before_the_empty_one_behind_it() {
    holds("turns_picks", "filesFrom", "the over-cap file is refused before the empty one behind it", d::files_from);
}

#[test]
fn files_from__the_empty_file_is_refused_before_the_over_cap_one_behind_it() {
    holds("turns_picks", "filesFrom", "the empty file is refused before the over-cap one behind it", d::files_from);
}

#[test]
fn files_from__a_png_travels_as_an_image_whatever_it_is_called() {
    holds("turns_picks", "filesFrom", "a png travels as an image whatever it is called", d::files_from);
}

#[test]
fn files_from__a_png_that_is_eight_bytes_and_no_more() {
    holds("turns_picks", "filesFrom", "a png that is eight bytes and no more", d::files_from);
}

#[test]
fn files_from__a_png_with_a_body() {
    holds("turns_picks", "filesFrom", "a png with a body", d::files_from);
}

#[test]
fn files_from__a_png_cut_at_seven_bytes_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a png cut at seven bytes is a plain file", d::files_from);
}

#[test]
fn files_from__a_png_whose_eighth_byte_is_wrong_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a png whose eighth byte is wrong is a plain file", d::files_from);
}

#[test]
fn files_from__a_jpeg_of_three_bytes() {
    holds("turns_picks", "filesFrom", "a jpeg of three bytes", d::files_from);
}

#[test]
fn files_from__a_jpeg_with_a_body() {
    holds("turns_picks", "filesFrom", "a jpeg with a body", d::files_from);
}

#[test]
fn files_from__a_jpeg_cut_at_two_bytes_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a jpeg cut at two bytes is a plain file", d::files_from);
}

#[test]
fn files_from__a_gif_is_told_by_its_first_four_bytes_as_text_too() {
    holds("turns_picks", "filesFrom", "a gif is told by its first four bytes, as text too", d::files_from);
}

#[test]
fn files_from__a_gif_of_four_bytes() {
    holds("turns_picks", "filesFrom", "a gif of four bytes", d::files_from);
}

#[test]
fn files_from__a_gif_cut_at_three_bytes_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a gif cut at three bytes is a plain file", d::files_from);
}

#[test]
fn files_from__a_lowercase_gif_word_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a lowercase gif word is a plain file", d::files_from);
}

#[test]
fn files_from__a_webp_is_told_by_its_twelve_byte_head() {
    holds("turns_picks", "filesFrom", "a webp is told by its twelve byte head", d::files_from);
}

#[test]
fn files_from__a_webp_with_a_body() {
    holds("turns_picks", "filesFrom", "a webp with a body", d::files_from);
}

#[test]
fn files_from__a_webp_cut_at_eleven_bytes_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a webp cut at eleven bytes is a plain file", d::files_from);
}

#[test]
fn files_from__a_riff_file_that_is_not_webp_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "a riff file that is not webp is a plain file", d::files_from);
}

#[test]
fn files_from__riff_alone_is_a_plain_file() {
    holds("turns_picks", "filesFrom", "riff alone is a plain file", d::files_from);
}

#[test]
fn files_from__an_image_and_a_file_keep_the_order_named() {
    holds("turns_picks", "filesFrom", "an image and a file keep the order named", d::files_from);
}

#[test]
fn files_from__an_image_at_the_cap_travels() {
    holds("turns_picks", "filesFrom", "an image at the cap travels", d::files_from);
}

#[test]
fn files_from__an_image_one_byte_over_the_cap_is_refused_as_an_image() {
    holds("turns_picks", "filesFrom", "an image one byte over the cap is refused as an image", d::files_from);
}

#[test]
fn files_from__a_file_named_like_an_image_but_over_the_cap_is_refused_as_a_() {
    holds("turns_picks", "filesFrom", "a file named like an image but over the cap is refused as a file", d::files_from);
}

#[test]
fn files_from__an_empty_png_name_is_an_empty_file() {
    holds("turns_picks", "filesFrom", "an empty png name is an empty file", d::files_from);
}

#[test]
fn checked_start__an_empty_task_is_refused_before_the_host_is_asked() {
    holds("turns_picks", "checkedStart", "an empty task is refused before the host is asked", d::checked_start);
}

#[test]
fn checked_start__a_task_of_spaces_and_newlines_is_empty() {
    holds("turns_picks", "checkedStart", "a task of spaces and newlines is empty", d::checked_start);
}

#[test]
fn checked_start__an_empty_task_on_a_workspace_asks_nothing_either() {
    holds("turns_picks", "checkedStart", "an empty task on a workspace asks nothing either", d::checked_start);
}

#[test]
fn checked_start__a_task_of_one_letter() {
    holds("turns_picks", "checkedStart", "a task of one letter", d::checked_start);
}

#[test]
fn checked_start__on_this_computer_the_picks_are_left_to_the_start() {
    holds("turns_picks", "checkedStart", "on this computer the picks are left to the start", d::checked_start);
}

#[test]
fn checked_start__on_this_computer_even_a_bad_access_word_is_left_to_the_start() {
    holds("turns_picks", "checkedStart", "on this computer even a bad access word is left to the start", d::checked_start);
}

#[test]
fn checked_start__on_this_computer_a_named_agent_in_the_list_passes() {
    holds("turns_picks", "checkedStart", "on this computer a named agent in the list passes", d::checked_start);
}

#[test]
fn checked_start__on_this_computer_a_named_agent_not_in_the_list_is_refused_wi() {
    holds("turns_picks", "checkedStart", "on this computer a named agent not in the list is refused with the list", d::checked_start);
}

#[test]
fn checked_start__a_named_agent_beside_none_at_all_says_none() {
    holds("turns_picks", "checkedStart", "a named agent beside none at all says none", d::checked_start);
}

#[test]
fn checked_start__a_named_agent_beside_one() {
    holds("turns_picks", "checkedStart", "a named agent beside one", d::checked_start);
}

#[test]
fn checked_start__an_agent_name_is_exact() {
    holds("turns_picks", "checkedStart", "an agent name is exact", d::checked_start);
}

#[test]
fn checked_start__an_empty_agent_name_names_no_agent() {
    holds("turns_picks", "checkedStart", "an empty agent name names no agent", d::checked_start);
}

#[test]
fn checked_start__no_agent_named_takes_the_table_marked_default_and_passes() {
    holds("turns_picks", "checkedStart", "no agent named takes the table marked default and passes", d::checked_start);
}

#[test]
fn checked_start__no_agents_at_all_passes_on_this_computer() {
    holds("turns_picks", "checkedStart", "no agents at all passes on this computer", d::checked_start);
}

#[test]
fn checked_start__the_host_refusing_the_lists_is_the_answer() {
    holds("turns_picks", "checkedStart", "the host refusing the lists is the answer", d::checked_start);
}

#[test]
fn checked_start__a_workspace_s_own_lists_are_asked_for_by_its_id() {
    holds("turns_picks", "checkedStart", "a workspace's own lists are asked for by its id", d::checked_start);
}

#[test]
fn checked_start__a_workspace_and_a_good_model_effort_and_access() {
    holds("turns_picks", "checkedStart", "a workspace and a good model, effort and access", d::checked_start);
}

#[test]
fn checked_start__a_workspace_and_a_model_the_list_lacks() {
    holds("turns_picks", "checkedStart", "a workspace and a model the list lacks", d::checked_start);
}

#[test]
fn checked_start__a_workspace_and_an_effort_the_list_lacks() {
    holds("turns_picks", "checkedStart", "a workspace and an effort the list lacks", d::checked_start);
}

#[test]
fn checked_start__an_effort_the_named_model_does_not_take() {
    holds("turns_picks", "checkedStart", "an effort the named model does not take", d::checked_start);
}

#[test]
fn checked_start__an_effort_of_a_model_that_takes_none() {
    holds("turns_picks", "checkedStart", "an effort of a model that takes none", d::checked_start);
}

#[test]
fn checked_start__fast_on_a_model_with_no_fast_mode() {
    holds("turns_picks", "checkedStart", "fast on a model with no fast mode", d::checked_start);
}

#[test]
fn checked_start__fast_on_a_model_with_a_fast_mode() {
    holds("turns_picks", "checkedStart", "fast on a model with a fast mode", d::checked_start);
}

#[test]
fn checked_start__fast_with_no_model_is_read_against_the_default_model() {
    holds("turns_picks", "checkedStart", "fast with no model is read against the default model", d::checked_start);
}

#[test]
fn checked_start__an_access_the_agent_maps() {
    holds("turns_picks", "checkedStart", "an access the agent maps", d::checked_start);
}

#[test]
fn checked_start__an_access_the_agent_maps_to_no_mode() {
    holds("turns_picks", "checkedStart", "an access the agent maps to no mode", d::checked_start);
}

#[test]
fn checked_start__an_access_the_agent_maps_to_no_mode_ask() {
    holds("turns_picks", "checkedStart", "an access the agent maps to no mode, ask", d::checked_start);
}

#[test]
fn checked_start__an_agent_that_maps_no_access_at_all() {
    holds("turns_picks", "checkedStart", "an agent that maps no access at all", d::checked_start);
}

#[test]
fn checked_start__a_word_that_is_no_access_word_is_refused_before_the_lists_ar() {
    holds("turns_picks", "checkedStart", "a word that is no access word is refused before the lists are read against", d::checked_start);
}

#[test]
fn checked_start__a_bad_access_word_beside_a_bad_model_the_access_word_first() {
    holds("turns_picks", "checkedStart", "a bad access word beside a bad model: the access word first", d::checked_start);
}

#[test]
fn checked_start__a_bad_model_beside_a_bad_access_mode_the_model_first() {
    holds("turns_picks", "checkedStart", "a bad model beside a bad access mode: the model first", d::checked_start);
}

#[test]
fn checked_start__lists_that_name_no_models_take_any_model() {
    holds("turns_picks", "checkedStart", "lists that name no models take any model", d::checked_start);
}

#[test]
fn checked_start__a_table_row_says_whose_list_it_quotes() {
    holds("turns_picks", "checkedStart", "a table row says whose list it quotes", d::checked_start);
}

#[test]
fn checked_start__a_table_row_s_model_that_takes_no_effort_says_the_table_said() {
    holds("turns_picks", "checkedStart", "a table row's model that takes no effort says the table said so", d::checked_start);
}

#[test]
fn checked_start__a_table_row_s_effort_off_the_list() {
    holds("turns_picks", "checkedStart", "a table row's effort off the list", d::checked_start);
}

#[test]
fn checked_start__a_workspace_with_no_agent_named_reads_the_default_s_table() {
    holds("turns_picks", "checkedStart", "a workspace with no agent named reads the default's table", d::checked_start);
}

#[test]
fn checked_start__a_workspace_with_no_row_marked_default_reads_no_table_every_() {
    holds("turns_picks", "checkedStart", "a workspace with no row marked default reads no table: every pick passes", d::checked_start);
}

#[test]
fn checked_start__a_workspace_with_no_agents_reads_no_table() {
    holds("turns_picks", "checkedStart", "a workspace with no agents reads no table", d::checked_start);
}

#[test]
fn checked_start__a_workspace_and_a_named_agent_reads_that_agent_s_table() {
    holds("turns_picks", "checkedStart", "a workspace and a named agent reads that agent's table", d::checked_start);
}

#[test]
fn checked_start__a_workspace_and_a_named_agent_its_model_not_the_default_s() {
    holds("turns_picks", "checkedStart", "a workspace and a named agent: its model not the default's", d::checked_start);
}

#[test]
fn checked_start__an_agent_off_on_the_workspace_s_computer_is_the_start_s_to_r() {
    holds(
        "turns_picks",
        "checkedStart",
        "an agent off on the workspace's computer is the start's to refuse when the host runs it",
        d::checked_start,
    );
}

#[test]
fn checked_start__an_agent_no_list_names_with_the_host_s_list_read_again() {
    holds("turns_picks", "checkedStart", "an agent no list names, with the host's list read again", d::checked_start);
}

#[test]
fn checked_start__an_agent_named_only_by_the_workspace_s_lists_is_no_adapter_w() {
    holds(
        "turns_picks",
        "checkedStart",
        "an agent named only by the workspace's lists is no adapter when the host lacks it",
        d::checked_start,
    );
}

#[test]
fn checked_start__an_agent_the_host_has_none_of_says_none() {
    holds("turns_picks", "checkedStart", "an agent the host has none of says none", d::checked_start);
}

#[test]
fn checked_start__the_second_read_refused_is_the_answer() {
    holds("turns_picks", "checkedStart", "the second read refused is the answer", d::checked_start);
}

#[test]
fn checked_start__a_named_agent_the_workspace_s_table_has_passes_picks_checked() {
    holds("turns_picks", "checkedStart", "a named agent the workspace's table has passes picks checked against it", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_no_agent_named_reads_the_project_s_own() {
    holds("turns_picks", "checkedStart", "a project's fork with no agent named reads the project's own agent", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_its_agent_s_model_refused() {
    holds("turns_picks", "checkedStart", "a project's fork with its agent's model refused", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_whose_default_agent_is_claude() {
    holds("turns_picks", "checkedStart", "a project's fork whose default agent is claude", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_no_defaults_for_it_falls_to_the_marked() {
    holds("turns_picks", "checkedStart", "a project's fork with no defaults for it falls to the marked default table", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_no_defaults_at_all() {
    holds("turns_picks", "checkedStart", "a project's fork with no defaults at all", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_named_for_an_agent_no_list_carries_reads_no() {
    holds("turns_picks", "checkedStart", "a project's fork named for an agent no list carries reads no table", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_an_agent_named_never_reads_the_default() {
    holds("turns_picks", "checkedStart", "a project's fork with an agent named never reads the defaults", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_a_bad_agent_named() {
    holds("turns_picks", "checkedStart", "a project's fork with a bad agent named", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_a_bad_access_word() {
    holds("turns_picks", "checkedStart", "a project's fork with a bad access word", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_an_access_the_agent_maps_to_no_mode() {
    holds("turns_picks", "checkedStart", "a project's fork with an access the agent maps to no mode", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_every_pick_good() {
    holds("turns_picks", "checkedStart", "a project's fork with every pick good", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_nothing_to_pick() {
    holds("turns_picks", "checkedStart", "a project's fork with nothing to pick", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_and_a_workspace_both_the_lists_of_the_works() {
    holds(
        "turns_picks",
        "checkedStart",
        "a project's fork and a workspace both: the lists of the workspace, the defaults of the project",
        d::checked_start,
    );
}

#[test]
fn checked_start__the_defaults_read_refused_is_the_answer() {
    holds("turns_picks", "checkedStart", "the defaults read refused is the answer", d::checked_start);
}

#[test]
fn checked_start__a_project_s_fork_with_the_lists_naming_no_agents() {
    holds("turns_picks", "checkedStart", "a project's fork with the lists naming no agents", d::checked_start);
}

#[test]
fn checked_start__an_empty_task_beside_a_fork_asks_nothing() {
    holds("turns_picks", "checkedStart", "an empty task beside a fork asks nothing", d::checked_start);
}

#[test]
fn project_defaults_of__none() {
    holds("turns_picks", "projectDefaultsOf", "none", d::project_defaults_of);
}

#[test]
fn project_defaults_of__one_project_the_agent_alone() {
    holds("turns_picks", "projectDefaultsOf", "one project, the agent alone", d::project_defaults_of);
}

#[test]
fn project_defaults_of__the_order_of_the_projects_is_the_host_s() {
    holds("turns_picks", "projectDefaultsOf", "the order of the projects is the host's", d::project_defaults_of);
}

#[test]
fn project_defaults_of__every_field_written_in_the_host_s_own_order_comes_out_in_the() {
    holds(
        "turns_picks",
        "projectDefaultsOf",
        "every field, written in the host's own order, comes out in the schema's",
        d::project_defaults_of,
    );
}

#[test]
fn project_defaults_of__each_place_a_pick_comes_from() {
    holds("turns_picks", "projectDefaultsOf", "each place a pick comes from", d::project_defaults_of);
}

#[test]
fn project_defaults_of__a_key_the_schema_does_not_know_is_dropped() {
    holds("turns_picks", "projectDefaultsOf", "a key the schema does not know is dropped", d::project_defaults_of);
}

#[test]
fn project_defaults_of__the_host_refusing() {
    holds("turns_picks", "projectDefaultsOf", "the host refusing", d::project_defaults_of);
}

#[test]
fn project_defaults_of__an_op_the_host_has_no_record_of() {
    holds("turns_picks", "projectDefaultsOf", "an op the host has no record of", d::project_defaults_of);
}

#[test]
fn preferences_of__with_the_agents_lists_shaped() {
    holds("turns_picks", "preferencesOf", "with the agents' lists shaped", d::preferences_of);
}

#[test]
fn preferences_of__with_none_standing() {
    holds("turns_picks", "preferencesOf", "with none standing", d::preferences_of);
}

#[test]
fn preferences_of__the_host_refusing() {
    holds("turns_picks", "preferencesOf", "the host refusing", d::preferences_of);
}

#[test]
fn preferences_of__an_op_the_host_has_no_record_of() {
    holds("turns_picks", "preferencesOf", "an op the host has no record of", d::preferences_of);
}

#[test]
fn worktree_for__made() {
    holds("turns_picks", "worktreeFor", "made", d::worktree_for);
}

#[test]
fn worktree_for__held_elsewhere_already() {
    holds("turns_picks", "worktreeFor", "held elsewhere already", d::worktree_for);
}

#[test]
fn worktree_for__the_schema_s_order_whatever_the_host_s() {
    holds("turns_picks", "worktreeFor", "the schema's order, whatever the host's", d::worktree_for);
}

#[test]
fn worktree_for__a_branch_named_with_a_space_and_an_accent() {
    holds("turns_picks", "worktreeFor", "a branch named with a space and an accent", d::worktree_for);
}

#[test]
fn worktree_for__an_empty_branch_is_sent_as_it_is() {
    holds("turns_picks", "worktreeFor", "an empty branch is sent as it is", d::worktree_for);
}

#[test]
fn worktree_for__refused() {
    holds("turns_picks", "worktreeFor", "refused", d::worktree_for);
}

#[test]
fn worktree_for__a_project_the_host_does_not_know() {
    holds("turns_picks", "worktreeFor", "a project the host does not know", d::worktree_for);
}

#[test]
fn opening_of__a_workspace_nothing_else() {
    holds("turns_picks", "openingOf", "a workspace, nothing else", d::opening_of);
}

#[test]
fn opening_of__a_workspace_with_no_options_argument() {
    holds("turns_picks", "openingOf", "a workspace with no options argument", d::opening_of);
}

#[test]
fn opening_of__a_workspace_and_an_empty_prompt_is_sent_as_it_is() {
    holds("turns_picks", "openingOf", "a workspace and an empty prompt is sent as it is", d::opening_of);
}

#[test]
fn opening_of__a_workspace_with_a_prompt_of_newlines_and_quotes() {
    holds("turns_picks", "openingOf", "a workspace with a prompt of newlines and quotes", d::opening_of);
}

#[test]
fn opening_of__a_workspace_with_every_option_in_the_frame_s_order() {
    holds("turns_picks", "openingOf", "a workspace with every option, in the frame's order", d::opening_of);
}

#[test]
fn opening_of__a_workspace_with_every_option_and_a_turn_token() {
    holds("turns_picks", "openingOf", "a workspace with every option and a turn token", d::opening_of);
}

#[test]
fn opening_of__a_workspace_the_turn_token_alone() {
    holds("turns_picks", "openingOf", "a workspace, the turn token alone", d::opening_of);
}

#[test]
fn opening_of__an_empty_turn_token_is_no_token() {
    holds("turns_picks", "openingOf", "an empty turn token is no token", d::opening_of);
}

#[test]
fn opening_of__the_host_token_is_not_the_turn_token() {
    holds("turns_picks", "openingOf", "the host token is not the turn token", d::opening_of);
}

#[test]
fn opening_of__other_variables_are_not_read() {
    holds("turns_picks", "openingOf", "other variables are not read", d::opening_of);
}

#[test]
fn opening_of__a_workspace_and_a_harness() {
    holds("turns_picks", "openingOf", "a workspace and a harness", d::opening_of);
}

#[test]
fn opening_of__an_empty_harness_is_a_harness() {
    holds("turns_picks", "openingOf", "an empty harness is a harness", d::opening_of);
}

#[test]
fn opening_of__a_workspace_and_a_folder() {
    holds("turns_picks", "openingOf", "a workspace and a folder", d::opening_of);
}

#[test]
fn opening_of__a_workspace_and_a_relative_folder_is_refused() {
    holds("turns_picks", "openingOf", "a workspace and a relative folder is refused", d::opening_of);
}

#[test]
fn opening_of__an_empty_folder_is_no_absolute_folder() {
    holds("turns_picks", "openingOf", "an empty folder is no absolute folder", d::opening_of);
}

#[test]
fn opening_of__a_folder_with_a_trailing_slash_is_kept_as_it_is() {
    holds("turns_picks", "openingOf", "a folder with a trailing slash is kept as it is", d::opening_of);
}

#[test]
fn opening_of__a_notify_list_of_one() {
    holds("turns_picks", "openingOf", "a notify list of one", d::opening_of);
}

#[test]
fn opening_of__an_empty_notify_list_is_sent_as_an_empty_list() {
    holds("turns_picks", "openingOf", "an empty notify list is sent as an empty list", d::opening_of);
}

#[test]
fn opening_of__a_notify_list_keeps_its_order_and_its_repeats() {
    holds("turns_picks", "openingOf", "a notify list keeps its order and its repeats", d::opening_of);
}

#[test]
fn opening_of__a_title() {
    holds("turns_picks", "openingOf", "a title", d::opening_of);
}

#[test]
fn opening_of__an_empty_title_is_a_title() {
    holds("turns_picks", "openingOf", "an empty title is a title", d::opening_of);
}

#[test]
fn opening_of__replaces() {
    holds("turns_picks", "openingOf", "replaces", d::opening_of);
}

#[test]
fn opening_of__the_picks_alone_in_the_picks_order() {
    holds("turns_picks", "openingOf", "the picks alone, in the picks' order", d::opening_of);
}

#[test]
fn opening_of__the_picks_written_in_the_other_order_come_out_in_the_same_on() {
    holds("turns_picks", "openingOf", "the picks written in the other order come out in the same one", d::opening_of);
}

#[test]
fn opening_of__an_access_word_of_the_harness_s_own_is_refused() {
    holds("turns_picks", "openingOf", "an access word of the harness's own is refused", d::opening_of);
}

#[test]
fn opening_of__fast_false_is_left_out() {
    holds("turns_picks", "openingOf", "fast false is left out", d::opening_of);
}

#[test]
fn opening_of__a_file_travels_as_an_attachment_after_the_title_and_replaces() {
    holds("turns_picks", "openingOf", "a file travels as an attachment, after the title and replaces, before the picks", d::opening_of);
}

#[test]
fn opening_of__two_files() {
    holds("turns_picks", "openingOf", "two files", d::opening_of);
}

#[test]
fn opening_of__an_image_and_a_file() {
    holds("turns_picks", "openingOf", "an image and a file", d::opening_of);
}

#[test]
fn opening_of__no_files_is_no_attachments() {
    holds("turns_picks", "openingOf", "no files is no attachments", d::opening_of);
}

#[test]
fn opening_of__a_file_that_is_not_there() {
    holds("turns_picks", "openingOf", "a file that is not there", d::opening_of);
}

#[test]
fn opening_of__elsewhere_with_a_file_is_refused() {
    holds("turns_picks", "openingOf", "elsewhere with a file is refused", d::opening_of);
}

#[test]
fn opening_of__elsewhere_with_no_file_is_not() {
    holds("turns_picks", "openingOf", "elsewhere with no file is not", d::opening_of);
}

#[test]
fn opening_of__elsewhere_with_an_empty_file_list_is_not() {
    holds("turns_picks", "openingOf", "elsewhere with an empty file list is not", d::opening_of);
}

#[test]
fn opening_of__elsewhere_false_reads_the_file() {
    holds("turns_picks", "openingOf", "elsewhere false reads the file", d::opening_of);
}

#[test]
fn opening_of__a_bad_folder_beside_a_bad_file_the_folder_is_refused_first() {
    holds("turns_picks", "openingOf", "a bad folder beside a bad file: the folder is refused first", d::opening_of);
}

#[test]
fn opening_of__a_bad_folder_beside_a_bad_access_the_folder_first() {
    holds("turns_picks", "openingOf", "a bad folder beside a bad access: the folder first", d::opening_of);
}

#[test]
fn opening_of__a_bad_file_beside_a_bad_access_the_file_first() {
    holds("turns_picks", "openingOf", "a bad file beside a bad access: the file first", d::opening_of);
}

#[test]
fn opening_of__a_resume() {
    holds("turns_picks", "openingOf", "a resume", d::opening_of);
}

#[test]
fn opening_of__a_resume_of_a_copy() {
    holds("turns_picks", "openingOf", "a resume of a copy", d::opening_of);
}

#[test]
fn opening_of__a_resume_leaves_the_folder_off() {
    holds("turns_picks", "openingOf", "a resume leaves the folder off", d::opening_of);
}

#[test]
fn opening_of__a_resume_with_a_relative_folder_is_not_refused_the_folder_is() {
    holds("turns_picks", "openingOf", "a resume with a relative folder is not refused: the folder is not read", d::opening_of);
}

#[test]
fn opening_of__a_resume_after_the_attachments_before_the_picks() {
    holds("turns_picks", "openingOf", "a resume after the attachments, before the picks", d::opening_of);
}

#[test]
fn opening_of__a_resume_and_a_turn_token() {
    holds("turns_picks", "openingOf", "a resume and a turn token", d::opening_of);
}

#[test]
fn opening_of__here_with_nothing_named() {
    holds("turns_picks", "openingOf", "here with nothing named", d::opening_of);
}

#[test]
fn opening_of__here_with_a_project() {
    holds("turns_picks", "openingOf", "here with a project", d::opening_of);
}

#[test]
fn opening_of__here_with_a_project_and_a_branch() {
    holds("turns_picks", "openingOf", "here with a project and a branch", d::opening_of);
}

#[test]
fn opening_of__here_with_a_branch_and_no_project() {
    holds("turns_picks", "openingOf", "here with a branch and no project", d::opening_of);
}

#[test]
fn opening_of__here_with_a_folder_the_folder_is_the_target_s() {
    holds("turns_picks", "openingOf", "here with a folder: the folder is the target's", d::opening_of);
}

#[test]
fn opening_of__here_with_a_folder_and_the_options_folder_the_target_s_wins() {
    holds("turns_picks", "openingOf", "here with a folder and the options' folder: the target's wins", d::opening_of);
}

#[test]
fn opening_of__here_with_no_folder_never_takes_the_options_folder() {
    holds("turns_picks", "openingOf", "here with no folder never takes the options' folder", d::opening_of);
}

#[test]
fn opening_of__here_with_a_relative_folder_is_refused() {
    holds("turns_picks", "openingOf", "here with a relative folder is refused", d::opening_of);
}

#[test]
fn opening_of__here_with_a_relative_folder_beside_a_resume_is_not_read() {
    holds("turns_picks", "openingOf", "here with a relative folder beside a resume is not read", d::opening_of);
}

#[test]
fn opening_of__here_with_project_branch_folder_and_every_option() {
    holds("turns_picks", "openingOf", "here with project, branch, folder and every option", d::opening_of);
}

#[test]
fn opening_of__here_the_project_s_id_alone_is_carried() {
    holds("turns_picks", "openingOf", "here, the project's id alone is carried", d::opening_of);
}

#[test]
fn opening_of__a_fork_carries_no_target_and_the_options_folder() {
    holds("turns_picks", "openingOf", "a fork carries no target and the options' folder", d::opening_of);
}

#[test]
fn opening_of__a_fork_with_nothing_else() {
    holds("turns_picks", "openingOf", "a fork with nothing else", d::opening_of);
}

#[test]
fn opening_of__a_fork_reads_its_files_before_the_machine_is_made() {
    holds("turns_picks", "openingOf", "a fork reads its files before the machine is made", d::opening_of);
}

#[test]
fn opening_of__a_fork_with_a_file_that_is_not_there() {
    holds("turns_picks", "openingOf", "a fork with a file that is not there", d::opening_of);
}

#[test]
fn opening_of__a_fork_with_every_option() {
    holds("turns_picks", "openingOf", "a fork with every option", d::opening_of);
}

#[test]
fn opening_of__a_fork_with_a_relative_folder() {
    holds("turns_picks", "openingOf", "a fork with a relative folder", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork() {
    holds("turns_picks", "openingOf", "a thread's fork", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_with_no_turn_counted() {
    holds("turns_picks", "openingOf", "a thread's fork with no turn counted", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_onto_a_branch() {
    holds("turns_picks", "openingOf", "a thread's fork onto a branch", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_with_no_turn_counted_onto_a_branch() {
    holds("turns_picks", "openingOf", "a thread's fork with no turn counted onto a branch", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_with_every_option() {
    holds("turns_picks", "openingOf", "a thread's fork with every option", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_takes_the_options_folder() {
    holds("turns_picks", "openingOf", "a thread's fork takes the options' folder", d::opening_of);
}

#[test]
fn opening_of__a_thread_s_fork_with_a_relative_folder() {
    holds("turns_picks", "openingOf", "a thread's fork with a relative folder", d::opening_of);
}

#[test]
fn thread_fork__by_id() {
    holds("turns_picks", "threadFork", "by id", d::thread_fork);
}

#[test]
fn thread_fork__by_a_prefix() {
    holds("turns_picks", "threadFork", "by a prefix", d::thread_fork);
}

#[test]
fn thread_fork__by_id_with_a_turn_counted_as_a_word() {
    holds("turns_picks", "threadFork", "by id with a turn counted as a word", d::thread_fork);
}

#[test]
fn thread_fork__by_id_with_a_turn_counted_as_a_number() {
    holds("turns_picks", "threadFork", "by id with a turn counted as a number", d::thread_fork);
}

#[test]
fn thread_fork__the_first_turn() {
    holds("turns_picks", "threadFork", "the first turn", d::thread_fork);
}

#[test]
fn thread_fork__a_branch() {
    holds("turns_picks", "threadFork", "a branch", d::thread_fork);
}

#[test]
fn thread_fork__a_turn_and_a_branch() {
    holds("turns_picks", "threadFork", "a turn and a branch", d::thread_fork);
}

#[test]
fn thread_fork__an_empty_branch_is_a_branch() {
    holds("turns_picks", "threadFork", "an empty branch is a branch", d::thread_fork);
}

#[test]
fn thread_fork__the_thread_s_agent_is_the_one_asked_for() {
    holds("turns_picks", "threadFork", "the thread's agent is the one asked for", d::thread_fork);
}

#[test]
fn thread_fork__a_named_agent_over_the_thread_s() {
    holds("turns_picks", "threadFork", "a named agent over the thread's", d::thread_fork);
}

#[test]
fn thread_fork__a_named_agent_the_host_lacks() {
    holds("turns_picks", "threadFork", "a named agent the host lacks", d::thread_fork);
}

#[test]
fn thread_fork__a_row_with_no_thread_id_is_named_by_its_own_id() {
    holds("turns_picks", "threadFork", "a row with no thread id is named by its own id", d::thread_fork);
}

#[test]
fn thread_fork__good_picks() {
    holds("turns_picks", "threadFork", "good picks", d::thread_fork);
}

#[test]
fn thread_fork__a_model_the_thread_s_agent_does_not_list() {
    holds("turns_picks", "threadFork", "a model the thread's agent does not list", d::thread_fork);
}

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
