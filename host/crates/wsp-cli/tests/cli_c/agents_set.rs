// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from agents_set.json: one test per recorded case.
use super::common::holds;
use super::drivers_agents_set as d;

#[test]
fn search_skills_sh__hits_with_a_limit_an_extra_key_dropped() {
    holds("agents_set", "searchSkillsSh", "hits with a limit, an extra key dropped", d::search_skills_sh);
}

#[test]
fn search_skills_sh__no_limit_sends_no_limit() {
    holds("agents_set", "searchSkillsSh", "no limit sends no limit", d::search_skills_sh);
}

#[test]
fn search_skills_sh__limit_zero_is_sent() {
    holds("agents_set", "searchSkillsSh", "limit zero is sent", d::search_skills_sh);
}

#[test]
fn search_skills_sh__none() {
    holds("agents_set", "searchSkillsSh", "none", d::search_skills_sh);
}

#[test]
fn search_skills_sh__an_empty_query_is_sent_as_it_is() {
    holds("agents_set", "searchSkillsSh", "an empty query is sent as it is", d::search_skills_sh);
}

#[test]
fn search_skills_sh__refused_by_the_host() {
    holds("agents_set", "searchSkillsSh", "refused by the host", d::search_skills_sh);
}

#[test]
fn search_skills_sh__the_op_is_not_in_the_record() {
    holds("agents_set", "searchSkillsSh", "the op is not in the record", d::search_skills_sh);
}

#[test]
fn search_skills_sh__a_query_with_spaces_and_a_unicode_name() {
    holds("agents_set", "searchSkillsSh", "a query with spaces and a unicode name", d::search_skills_sh);
}

#[test]
fn skill_hit_lines__none() {
    holds("agents_set", "skillHitLines", "none", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__one() {
    holds("agents_set", "skillHitLines", "one", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__columns_pad_to_the_widest_cell() {
    holds("agents_set", "skillHitLines", "columns pad to the widest cell", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__installs_zero_and_a_large_count() {
    holds("agents_set", "skillHitLines", "installs zero and a large count", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__a_tab_and_a_newline_in_a_name_become_spaces() {
    holds("agents_set", "skillHitLines", "a tab and a newline in a name become spaces", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__control_characters_and_escapes_are_dropped_skills_sh_test_ts() {
    holds(
        "agents_set",
        "skillHitLines",
        "control characters and escapes are dropped (skills-sh.test.ts what a skills verb prints)",
        d::skill_hit_lines,
    );
}

#[test]
fn skill_hit_lines__a_wide_character_counts_as_two_cells_of_width() {
    holds("agents_set", "skillHitLines", "a wide character counts as two cells of width", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__an_empty_name() {
    holds("agents_set", "skillHitLines", "an empty name", d::skill_hit_lines);
}

#[test]
fn skill_hit_lines__the_same_with_the_cloud_on() {
    holds("agents_set", "skillHitLines", "the same with the cloud on", d::skill_hit_lines);
}

#[test]
fn skill_shown__an_id_off_skills_sh_asks_skills_get() {
    holds("agents_set", "skillShown", "an id off skills.sh asks skills.get", d::skill_shown);
}

#[test]
fn skill_shown__an_id_off_skills_sh_with_no_project() {
    holds("agents_set", "skillShown", "an id off skills.sh with no project", d::skill_shown);
}

#[test]
fn skill_shown__a_skills_sh_id_with_an_empty_middle_is_still_one() {
    holds("agents_set", "skillShown", "a skills.sh id with an empty middle is still one", d::skill_shown);
}

#[test]
fn skill_shown__two_parts_name_a_skill_on_the_target() {
    holds("agents_set", "skillShown", "two parts name a skill on the target", d::skill_shown);
}

#[test]
fn skill_shown__four_parts_name_a_skill_on_the_target() {
    holds("agents_set", "skillShown", "four parts name a skill on the target", d::skill_shown);
}

#[test]
fn skill_shown__an_empty_name_asks_the_target() {
    holds("agents_set", "skillShown", "an empty name asks the target", d::skill_shown);
}

#[test]
fn skill_shown__an_id_off_skills_sh_with_a_thread_is_refused_before_any_fram() {
    holds("agents_set", "skillShown", "an id off skills.sh with a thread is refused before any frame", d::skill_shown);
}

#[test]
fn skill_shown__an_id_off_skills_sh_with_a_computer_is_refused_before_any_fr() {
    holds("agents_set", "skillShown", "an id off skills.sh with a computer is refused before any frame", d::skill_shown);
}

#[test]
fn skill_shown__an_id_off_skills_sh_with_a_project_of_the_thread_is_refused_() {
    holds("agents_set", "skillShown", "an id off skills.sh with a project of the thread is refused before any frame", d::skill_shown);
}

#[test]
fn skill_shown__an_id_off_skills_sh_with_a_project_by_name_is_refused_before() {
    holds("agents_set", "skillShown", "an id off skills.sh with a project by name is refused before any frame", d::skill_shown);
}

#[test]
fn skill_shown__here_skills_preview_of_the_name_no_project_key() {
    holds("agents_set", "skillShown", "here: skills.preview of the name, no project key", d::skill_shown);
}

#[test]
fn skill_shown__on_a_computer_s_project_by_name() {
    holds("agents_set", "skillShown", "on a computer's project by name", d::skill_shown);
}

#[test]
fn skill_shown__on_a_computer_with_the_project_asked_but_unnamed() {
    holds("agents_set", "skillShown", "on a computer with the project asked but unnamed", d::skill_shown);
}

#[test]
fn skill_shown__on_a_computer_no_project() {
    holds("agents_set", "skillShown", "on a computer, no project", d::skill_shown);
}

#[test]
fn skill_shown__on_a_computer_by_its_id() {
    holds("agents_set", "skillShown", "on a computer by its id", d::skill_shown);
}

#[test]
fn skill_shown__on_a_provider_by_its_name_in_other_case() {
    holds("agents_set", "skillShown", "on a provider by its name in other case", d::skill_shown);
}

#[test]
fn skill_shown__a_project_name_given_with_no_on_is_only_read_by_the_target_o() {
    holds("agents_set", "skillShown", "a project name given with no --on is only read by the target of a thread", d::skill_shown);
}

#[test]
fn skill_shown__in_a_workspace_by_thread_its_own_project() {
    holds("agents_set", "skillShown", "in a workspace by thread, its own project", d::skill_shown);
}

#[test]
fn skill_shown__in_a_workspace_by_thread_not_a_project_s() {
    holds("agents_set", "skillShown", "in a workspace by thread, not a project's", d::skill_shown);
}

#[test]
fn skill_shown__both_a_thread_and_a_computer() {
    holds("agents_set", "skillShown", "both a thread and a computer", d::skill_shown);
}

#[test]
fn skill_shown__no_such_computer() {
    holds("agents_set", "skillShown", "no such computer", d::skill_shown);
}

#[test]
fn skill_shown__another_version_answers_the_thread() {
    holds("agents_set", "skillShown", "another version answers the thread", d::skill_shown);
}

#[test]
fn skill_shown__the_host_refuses_the_name() {
    holds("agents_set", "skillShown", "the host refuses the name", d::skill_shown);
}

#[test]
fn skill_shown__a_thread_naming_nothing_the_cloud_off() {
    holds("agents_set", "skillShown", "a thread naming nothing, the cloud off", d::skill_shown);
}

#[test]
fn skill_shown__a_thread_naming_nothing_the_cloud_on() {
    holds("agents_set", "skillShown", "a thread naming nothing, the cloud on", d::skill_shown);
}

#[test]
fn skill_shown__the_host_refuses_an_id_off_skills_sh() {
    holds("agents_set", "skillShown", "the host refuses an id off skills.sh", d::skill_shown);
}

#[test]
fn skill_shown__the_same_with_the_cloud_on() {
    holds("agents_set", "skillShown", "the same with the cloud on", d::skill_shown);
}

#[test]
fn shown_text__a_short_text_whole() {
    holds("agents_set", "shownText", "a short text, whole", d::shown_text);
}

#[test]
fn shown_text__size_0() {
    holds("agents_set", "shownText", "size 0", d::shown_text);
}

#[test]
fn shown_text__size_1() {
    holds("agents_set", "shownText", "size 1", d::shown_text);
}

#[test]
fn shown_text__size_just_under_the_cap() {
    holds("agents_set", "shownText", "size just under the cap", d::shown_text);
}

#[test]
fn shown_text__size_at_the_cap_no_cut_line() {
    holds("agents_set", "shownText", "size at the cap, no cut line", d::shown_text);
}

#[test]
fn shown_text__size_one_over_the_cap_65_kb() {
    holds("agents_set", "shownText", "size one over the cap, 65 KB", d::shown_text);
}

#[test]
fn shown_text__size_66559_rounds_up_to_65_kb() {
    holds("agents_set", "shownText", "size 66559, rounds up to 65 KB", d::shown_text);
}

#[test]
fn shown_text__size_66560_is_exactly_65_kb() {
    holds("agents_set", "shownText", "size 66560 is exactly 65 KB", d::shown_text);
}

#[test]
fn shown_text__size_66561_rounds_up_to_66_kb() {
    holds("agents_set", "shownText", "size 66561 rounds up to 66 KB", d::shown_text);
}

#[test]
fn shown_text__size_70000_rounds_up_to_69_kb() {
    holds("agents_set", "shownText", "size 70000 rounds up to 69 KB", d::shown_text);
}

#[test]
fn shown_text__a_huge_size() {
    holds("agents_set", "shownText", "a huge size", d::shown_text);
}

#[test]
fn shown_text__an_empty_text_over_the_cap() {
    holds("agents_set", "shownText", "an empty text over the cap", d::shown_text);
}

#[test]
fn shown_text__control_characters_and_escapes_are_dropped_tab_and_newline_k() {
    holds("agents_set", "shownText", "control characters and escapes are dropped, tab and newline kept (skills-sh.test.ts)", d::shown_text);
}

#[test]
fn shown_text__the_same_hostile_text_over_the_cap() {
    holds("agents_set", "shownText", "the same hostile text over the cap", d::shown_text);
}

#[test]
fn shown_text__emoji_and_accents_are_kept() {
    holds("agents_set", "shownText", "emoji and accents are kept", d::shown_text);
}

#[test]
fn shown_text__the_same_with_the_cloud_on() {
    holds("agents_set", "shownText", "the same with the cloud on", d::shown_text);
}

#[test]
fn skill_added__copies_for_two_agents() {
    holds("agents_set", "skillAdded", "copies for two agents", d::skill_added);
}

#[test]
fn skill_added__alone_no_agents_key() {
    holds("agents_set", "skillAdded", "alone, no agents key", d::skill_added);
}

#[test]
fn skill_added__an_empty_agents_list_sends_no_agents_key() {
    holds("agents_set", "skillAdded", "an empty agents list sends no agents key", d::skill_added);
}

#[test]
fn skill_added__one_agent() {
    holds("agents_set", "skillAdded", "one agent", d::skill_added);
}

#[test]
fn skill_added__a_project_on_a_computer() {
    holds("agents_set", "skillAdded", "a project on a computer", d::skill_added);
}

#[test]
fn skill_added__a_project_on_a_computer_unnamed() {
    holds("agents_set", "skillAdded", "a project on a computer, unnamed", d::skill_added);
}

#[test]
fn skill_added__the_thread_s_own_project() {
    holds("agents_set", "skillAdded", "the thread's own project", d::skill_added);
}

#[test]
fn skill_added__the_host_refuses_the_machine_napping() {
    holds("agents_set", "skillAdded", "the host refuses, the machine napping", d::skill_added);
}

#[test]
fn skill_added__both_a_thread_and_a_computer() {
    holds("agents_set", "skillAdded", "both a thread and a computer", d::skill_added);
}

#[test]
fn skill_added__no_such_computer() {
    holds("agents_set", "skillAdded", "no such computer", d::skill_added);
}

#[test]
fn skill_added__an_id_off_skills_sh_asks_the_target_like_a_name() {
    holds("agents_set", "skillAdded", "an id off skills.sh asks the target like a name", d::skill_added);
}

#[test]
fn skill_added__the_cloud_on_changes_nothing() {
    holds("agents_set", "skillAdded", "the cloud on changes nothing", d::skill_added);
}

#[test]
fn added_line__alone() {
    holds("agents_set", "addedLine", "alone", d::added_line);
}

#[test]
fn added_line__one_agent_copy() {
    holds("agents_set", "addedLine", "one agent copy", d::added_line);
}

#[test]
fn added_line__two_copies_comma_joined_in_the_order_answered() {
    holds("agents_set", "addedLine", "two copies, comma joined, in the order answered", d::added_line);
}

#[test]
fn added_line__an_agent_the_catalog_does_not_know_is_named_by_its_id() {
    holds("agents_set", "addedLine", "an agent the catalog does not know is named by its id", d::added_line);
}

#[test]
fn added_line__a_name_with_no_slash() {
    holds("agents_set", "addedLine", "a name with no slash", d::added_line);
}

#[test]
fn added_line__a_trailing_slash_leaves_the_name_empty() {
    holds("agents_set", "addedLine", "a trailing slash leaves the name empty", d::added_line);
}

#[test]
fn added_line__an_empty_skill() {
    holds("agents_set", "addedLine", "an empty skill", d::added_line);
}

#[test]
fn added_line__the_name_is_made_printable() {
    holds("agents_set", "addedLine", "the name is made printable", d::added_line);
}

#[test]
fn added_line__folders_print_without_escapes_or_line_breaks_skills_sh_test_() {
    holds("agents_set", "addedLine", "folders print without escapes or line breaks (skills-sh.test.ts)", d::added_line);
}

#[test]
fn added_line__an_empty_path() {
    holds("agents_set", "addedLine", "an empty path", d::added_line);
}

#[test]
fn skill_changed__remove_the_thread_s_own_project() {
    holds("agents_set", "skillChanged", "remove, the thread's own project", d::skill_changed);
}

#[test]
fn skill_changed__remove_here_no_project_no_on_key() {
    holds("agents_set", "skillChanged", "remove here, no project, no on key", d::skill_changed);
}

#[test]
fn skill_changed__remove_ignores_a_turn_word() {
    holds("agents_set", "skillChanged", "remove ignores a turn word", d::skill_changed);
}

#[test]
fn skill_changed__remove_reads_removed_not_paths() {
    holds("agents_set", "skillChanged", "remove reads removed, not paths", d::skill_changed);
}

#[test]
fn skill_changed__remove_of_nothing() {
    holds("agents_set", "skillChanged", "remove of nothing", d::skill_changed);
}

#[test]
fn skill_changed__remove_refused() {
    holds("agents_set", "skillChanged", "remove refused", d::skill_changed);
}

#[test]
fn skill_changed__remove_with_both_aimed() {
    holds("agents_set", "skillChanged", "remove with both aimed", d::skill_changed);
}

#[test]
fn skill_changed__toggle_off_on_a_computer() {
    holds("agents_set", "skillChanged", "toggle off on a computer", d::skill_changed);
}

#[test]
fn skill_changed__toggle_on_sends_on_true() {
    holds("agents_set", "skillChanged", "toggle on sends on true", d::skill_changed);
}

#[test]
fn skill_changed__toggle_with_no_turn_sends_no_on_key() {
    holds("agents_set", "skillChanged", "toggle with no turn sends no on key", d::skill_changed);
}

#[test]
fn skill_changed__toggle_reads_paths_not_removed() {
    holds("agents_set", "skillChanged", "toggle reads paths, not removed", d::skill_changed);
}

#[test]
fn skill_changed__toggle_a_project_s_by_name_on_a_computer() {
    holds("agents_set", "skillChanged", "toggle a project's by name on a computer", d::skill_changed);
}

#[test]
fn skill_changed__toggle_refused() {
    holds("agents_set", "skillChanged", "toggle refused", d::skill_changed);
}

#[test]
fn skill_changed__toggle_in_the_thread_s_workspace_a_project_s() {
    holds("agents_set", "skillChanged", "toggle in the thread's workspace, a project's", d::skill_changed);
}

#[test]
fn skill_changed__no_such_computer() {
    holds("agents_set", "skillChanged", "no such computer", d::skill_changed);
}

#[test]
fn skill_changed__the_cloud_on_changes_nothing() {
    holds("agents_set", "skillChanged", "the cloud on changes nothing", d::skill_changed);
}

#[test]
fn removed_line__one_folder() {
    holds("agents_set", "removedLine", "one folder", d::removed_line);
}

#[test]
fn removed_line__two_folders_comma_joined() {
    holds("agents_set", "removedLine", "two folders, comma joined", d::removed_line);
}

#[test]
fn removed_line__no_folder() {
    holds("agents_set", "removedLine", "no folder", d::removed_line);
}

#[test]
fn removed_line__folders_print_without_escapes_or_line_breaks_skills_sh_test_() {
    holds("agents_set", "removedLine", "folders print without escapes or line breaks (skills-sh.test.ts)", d::removed_line);
}

#[test]
fn removed_line__a_tab_in_the_name_and_a_newline_in_a_folder() {
    holds("agents_set", "removedLine", "a tab in the name and a newline in a folder", d::removed_line);
}

#[test]
fn default_agent_set__set() {
    holds("agents_set", "defaultAgentSet", "set", d::default_agent_set);
}

#[test]
fn default_agent_set__an_agent_the_host_runs_no_thread_of() {
    holds("agents_set", "defaultAgentSet", "an agent the host runs no thread of", d::default_agent_set);
}

#[test]
fn default_agent_set__an_empty_agent_is_sent_as_it_is() {
    holds("agents_set", "defaultAgentSet", "an empty agent is sent as it is", d::default_agent_set);
}

#[test]
fn default_agent_set__the_op_is_not_in_the_record() {
    holds("agents_set", "defaultAgentSet", "the op is not in the record", d::default_agent_set);
}

#[test]
fn agent_defaults_set__model_effort_and_access() {
    holds("agents_set", "agentDefaultsSet", "model, effort and access", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_picker_s_lists_merged_over_what_stands() {
    holds("agents_set", "agentDefaultsSet", "the picker's lists merged over what stands", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_picker_s_lists_with_none_standing() {
    holds("agents_set", "agentDefaultsSet", "the picker's lists with none standing", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__put_back() {
    holds("agents_set", "agentDefaultsSet", "put back", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__nothing_to_change() {
    holds("agents_set", "agentDefaultsSet", "nothing to change", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__empty_lists_and_no_reset_are_nothing_to_change() {
    holds("agents_set", "agentDefaultsSet", "empty lists and no reset are nothing to change", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_model_and_empty_lists_reads_no_preferences() {
    holds("agents_set", "agentDefaultsSet", "a model and empty lists reads no preferences", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_agent_s_own_spelling_of_access() {
    holds("agents_set", "agentDefaultsSet", "an agent's own spelling of access", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_empty_access_word() {
    holds("agents_set", "agentDefaultsSet", "an empty access word", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_word_the_agent_has_no_mode_for() {
    holds("agents_set", "agentDefaultsSet", "a word the agent has no mode for", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__access_ask() {
    holds("agents_set", "agentDefaultsSet", "access ask", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__access_auto_edit() {
    holds("agents_set", "agentDefaultsSet", "access auto-edit", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__access_full() {
    holds("agents_set", "agentDefaultsSet", "access full", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_reset_word_outside_the_list() {
    holds("agents_set", "agentDefaultsSet", "a reset word outside the list", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_project_s_reset_word_is_refused() {
    holds("agents_set", "agentDefaultsSet", "a project's reset word is refused", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_first_bad_reset_word_is_named() {
    holds("agents_set", "agentDefaultsSet", "the first bad reset word is named", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_reset_word_in_other_case_is_refused() {
    holds("agents_set", "agentDefaultsSet", "a reset word in other case is refused", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_value_wins_over_a_reset_of_the_same_field() {
    holds("agents_set", "agentDefaultsSet", "a value wins over a reset of the same field", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__reset_effort_and_access() {
    holds("agents_set", "agentDefaultsSet", "reset effort and access", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__reset_all_four() {
    holds("agents_set", "agentDefaultsSet", "reset all four", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__reset_models_beats_the_lists_and_reads_no_preferences() {
    holds("agents_set", "agentDefaultsSet", "reset models beats the lists and reads no preferences", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__reset_models_with_a_model_set() {
    holds("agents_set", "agentDefaultsSet", "reset models with a model set", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_same_reset_twice() {
    holds("agents_set", "agentDefaultsSet", "the same reset twice", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_list_with_a_model_and_a_reset_of_another_field() {
    holds("agents_set", "agentDefaultsSet", "a list with a model and a reset of another field", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__show_takes_a_model_out_of_the_kept_hide() {
    holds("agents_set", "agentDefaultsSet", "show takes a model out of the kept hide", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__show_of_the_last_hidden_model_leaves_an_empty_picker() {
    holds("agents_set", "agentDefaultsSet", "show of the last hidden model leaves an empty picker", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__hide_and_show_of_the_same_model_hide_is_added_after_the_filt() {
    holds("agents_set", "agentDefaultsSet", "hide and show of the same model: hide is added after the filter", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__hide_is_de_duplicated_against_what_stands_and_itself() {
    holds("agents_set", "agentDefaultsSet", "hide is de-duplicated against what stands and itself", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__hide_with_the_kept_list_keeping_its_order_first() {
    holds("agents_set", "agentDefaultsSet", "hide with the kept list keeping its order first", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__order_replaces_the_kept_order() {
    holds("agents_set", "agentDefaultsSet", "order replaces the kept order", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__order_is_not_de_duplicated() {
    holds("agents_set", "agentDefaultsSet", "order is not de-duplicated", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_kept_order_stays_when_only_hide_is_given() {
    holds("agents_set", "agentDefaultsSet", "the kept order stays when only hide is given", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_empty_order_keeps_the_kept_order() {
    holds("agents_set", "agentDefaultsSet", "an empty order keeps the kept order", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__addmodel_after_what_stands_de_duplicated() {
    holds("agents_set", "agentDefaultsSet", "addModel after what stands, de-duplicated", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__dropmodel_takes_from_the_kept_custom_list() {
    holds("agents_set", "agentDefaultsSet", "dropModel takes from the kept custom list", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__dropmodel_of_the_only_custom_model_sends_an_empty_models_obj() {
    holds("agents_set", "agentDefaultsSet", "dropModel of the only custom model sends an empty models object", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__dropmodel_with_nothing_standing_sends_an_empty_models_object() {
    holds("agents_set", "agentDefaultsSet", "dropModel with nothing standing sends an empty models object", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__dropmodel_and_addmodel_of_the_same_model_add_comes_after_the() {
    holds("agents_set", "agentDefaultsSet", "dropModel and addModel of the same model: add comes after the drop", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__every_list_at_once_keeps_hide_order_custom_in_that_order() {
    holds("agents_set", "agentDefaultsSet", "every list at once keeps hide, order, custom in that order", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_kept_lists_of_another_agent_are_not_read() {
    holds("agents_set", "agentDefaultsSet", "the kept lists of another agent are not read", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_agent_standing_with_no_models() {
    holds("agents_set", "agentDefaultsSet", "an agent standing with no models", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__a_model_and_a_list_together_the_patch_keeps_model_first_mode() {
    holds("agents_set", "agentDefaultsSet", "a model and a list together: the patch keeps model first, models last", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_answer_has_no_entry_for_the_agent() {
    holds("agents_set", "agentDefaultsSet", "the answer has no entry for the agent", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_answer_has_the_agent_s_whole_defaults() {
    holds("agents_set", "agentDefaultsSet", "the answer has the agent's whole defaults", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_host_refuses_the_models() {
    holds("agents_set", "agentDefaultsSet", "the host refuses the models", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__reading_the_preferences_is_refused_nothing_is_set() {
    holds("agents_set", "agentDefaultsSet", "reading the preferences is refused: nothing is set", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__model_names_with_spaces_and_unicode_are_sent_as_they_are() {
    holds("agents_set", "agentDefaultsSet", "model names with spaces and unicode are sent as they are", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_empty_model_name_is_sent_as_it_is() {
    holds("agents_set", "agentDefaultsSet", "an empty model name is sent as it is", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__effort_only() {
    holds("agents_set", "agentDefaultsSet", "effort only", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__an_agent_not_in_the_catalog() {
    holds("agents_set", "agentDefaultsSet", "an agent not in the catalog", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_op_is_not_in_the_record() {
    holds("agents_set", "agentDefaultsSet", "the op is not in the record", d::agent_defaults_set);
}

#[test]
fn agent_defaults_set__the_same_with_the_cloud_on() {
    holds("agents_set", "agentDefaultsSet", "the same with the cloud on", d::agent_defaults_set);
}

#[test]
fn agent_defaults_line__its_own_defaults() {
    holds("agents_set", "agentDefaultsLine", "its own defaults", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__model_only() {
    holds("agents_set", "agentDefaultsLine", "model only", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__model_effort_access() {
    holds("agents_set", "agentDefaultsLine", "model, effort, access", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__access_only() {
    holds("agents_set", "agentDefaultsLine", "access only", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__hide_only() {
    holds("agents_set", "agentDefaultsLine", "hide only", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__hide_order_custom_in_that_order() {
    holds("agents_set", "agentDefaultsLine", "hide, order, custom in that order", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__every_field() {
    holds("agents_set", "agentDefaultsLine", "every field", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__empty_lists_say_nothing() {
    holds("agents_set", "agentDefaultsLine", "empty lists say nothing", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__an_empty_models_object() {
    holds("agents_set", "agentDefaultsLine", "an empty models object", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__an_empty_list_among_full_ones() {
    holds("agents_set", "agentDefaultsLine", "an empty list among full ones", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__an_agent_the_catalog_does_not_know() {
    holds("agents_set", "agentDefaultsLine", "an agent the catalog does not know", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__opencode() {
    holds("agents_set", "agentDefaultsLine", "opencode", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__model_names_are_not_cleaned() {
    holds("agents_set", "agentDefaultsLine", "model names are not cleaned", d::agent_defaults_line);
}

#[test]
fn agent_defaults_line__the_same_with_the_cloud_on() {
    holds("agents_set", "agentDefaultsLine", "the same with the cloud on", d::agent_defaults_line);
}

#[test]
fn env_name_of__foo_is_a_name() {
    holds("agents_set", "envNameOf", "FOO is a name", d::env_name_of);
}

#[test]
fn env_name_of__foo_bar_is_a_name() {
    holds("agents_set", "envNameOf", "foo_bar is a name", d::env_name_of);
}

#[test]
fn env_name_of__x_is_a_name() {
    holds("agents_set", "envNameOf", "_X is a name", d::env_name_of);
}

#[test]
fn env_name_of__a1_is_a_name() {
    holds("agents_set", "envNameOf", "A1 is a name", d::env_name_of);
}

#[test]
fn env_name_of__a_is_a_name() {
    holds("agents_set", "envNameOf", "a is a name", d::env_name_of);
}

#[test]
fn env_name_of__is_a_name() {
    holds("agents_set", "envNameOf", "_ is a name", d::env_name_of);
}

#[test]
fn env_name_of__z9_z_is_a_name() {
    holds("agents_set", "envNameOf", "Z9_z is a name", d::env_name_of);
}

#[test]
fn env_name_of__is_refused() {
    holds("agents_set", "envNameOf", "\"\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__1x_is_refused() {
    holds("agents_set", "envNameOf", "\"1X\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__1_bad_is_refused() {
    holds("agents_set", "envNameOf", "\"1 BAD\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_b_is_refused() {
    holds("agents_set", "envNameOf", "\"a b\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_b_is_refused_2() {
    holds("agents_set", "envNameOf", "\"A-B\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_b_is_refused_3() {
    holds("agents_set", "envNameOf", "\"A=B\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_is_refused() {
    holds("agents_set", "envNameOf", "\"$A\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__u_c9_is_refused() {
    holds("agents_set", "envNameOf", "\"U+C9\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_b_is_refused_4() {
    holds("agents_set", "envNameOf", "\"A.B\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_is_refused_2() {
    holds("agents_set", "envNameOf", "\" A\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__a_n_is_refused() {
    holds("agents_set", "envNameOf", "\"A\\n\" is refused", d::env_name_of);
}

#[test]
fn env_name_of__the_refusal_opens_with_the_line_s_own_name() {
    holds("agents_set", "envNameOf", "the refusal opens with the line's own name", d::env_name_of);
}

#[test]
fn env_name_of__a_name_that_decides_how_an_agent_starts_is_a_name_here() {
    holds("agents_set", "envNameOf", "a name that decides how an agent starts is a name here", d::env_name_of);
}

#[test]
fn agent_setup_set__set_here() {
    holds("agents_set", "agentSetupSet", "set here", d::agent_setup_set);
}

#[test]
fn agent_setup_set__on_a_computer() {
    holds("agents_set", "agentSetupSet", "on a computer", d::agent_setup_set);
}

#[test]
fn agent_setup_set__no_such_computer() {
    holds("agents_set", "agentSetupSet", "no such computer", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_config_folder_that_is_not_absolute() {
    holds("agents_set", "agentSetupSet", "a config folder that is not absolute", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_tilde_config_folder_is_not_absolute() {
    holds("agents_set", "agentSetupSet", "a tilde config folder is not absolute", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_dot_config_folder_is_not_absolute() {
    holds("agents_set", "agentSetupSet", "a dot config folder is not absolute", d::agent_setup_set);
}

#[test]
fn agent_setup_set__an_empty_config_folder_is_refused() {
    holds("agents_set", "agentSetupSet", "an empty config folder is refused", d::agent_setup_set);
}

#[test]
fn agent_setup_set__an_absolute_config_folder_is_sent_as_it_is_never_resolved() {
    holds("agents_set", "agentSetupSet", "an absolute config folder is sent as it is, never resolved", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_name_a_shell_would_not_read() {
    holds("agents_set", "agentSetupSet", "a name a shell would not read", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_first_bad_unset_name_is_named() {
    holds("agents_set", "agentSetupSet", "the first bad unset name is named", d::agent_setup_set);
}

#[test]
fn agent_setup_set__nothing_to_change() {
    holds("agents_set", "agentSetupSet", "nothing to change", d::agent_setup_set);
}

#[test]
fn agent_setup_set__empty_args_and_empty_lists_are_nothing_to_change() {
    holds("agents_set", "agentSetupSet", "empty args and empty lists are nothing to change", d::agent_setup_set);
}

#[test]
fn agent_setup_set__refused_by_the_host() {
    holds("agents_set", "agentSetupSet", "refused by the host", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_reset_word_outside_the_list() {
    holds("agents_set", "agentSetupSet", "a reset word outside the list", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_project_s_reset_word_is_refused() {
    holds("agents_set", "agentSetupSet", "a project's reset word is refused", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_first_bad_reset_word_is_named() {
    holds("agents_set", "agentSetupSet", "the first bad reset word is named", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_bad_reset_word_is_refused_before_the_computer_is_looked_up() {
    holds("agents_set", "agentSetupSet", "a bad reset word is refused before the computer is looked up", d::agent_setup_set);
}

#[test]
fn agent_setup_set__nothing_to_change_still_looks_the_computer_up_first() {
    holds("agents_set", "agentSetupSet", "nothing to change still looks the computer up first", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_bad_config_still_looks_the_computer_up_first() {
    holds("agents_set", "agentSetupSet", "a bad config still looks the computer up first", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_bad_unset_name_still_looks_the_computer_up_first() {
    holds("agents_set", "agentSetupSet", "a bad unset name still looks the computer up first", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_value_on_a_computer() {
    holds("agents_set", "agentSetupSet", "a value on a computer", d::agent_setup_set);
}

#[test]
fn agent_setup_set__values_ride_env() {
    holds("agents_set", "agentSetupSet", "values ride env", d::agent_setup_set);
}

#[test]
fn agent_setup_set__values_alone_are_a_change() {
    holds("agents_set", "agentSetupSet", "values alone are a change", d::agent_setup_set);
}

#[test]
fn agent_setup_set__values_and_unset_names_values_first() {
    holds("agents_set", "agentSetupSet", "values and unset names, values first", d::agent_setup_set);
}

#[test]
fn agent_setup_set__an_unset_name_that_is_also_a_value_is_null_in_the_value_s_pl() {
    holds("agents_set", "agentSetupSet", "an unset name that is also a value is null in the value's place", d::agent_setup_set);
}

#[test]
fn agent_setup_set__an_unset_name_twice_is_one_key() {
    holds("agents_set", "agentSetupSet", "an unset name twice is one key", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_value_s_name_is_not_checked_here() {
    holds("agents_set", "agentSetupSet", "a value's name is not checked here", d::agent_setup_set);
}

#[test]
fn agent_setup_set__enabled_true_alone() {
    holds("agents_set", "agentSetupSet", "enabled true alone", d::agent_setup_set);
}

#[test]
fn agent_setup_set__enabled_false_alone() {
    holds("agents_set", "agentSetupSet", "enabled false alone", d::agent_setup_set);
}

#[test]
fn agent_setup_set__an_empty_program_is_sent_as_it_is() {
    holds("agents_set", "agentSetupSet", "an empty program is sent as it is", d::agent_setup_set);
}

#[test]
fn agent_setup_set__reset_program() {
    holds("agents_set", "agentSetupSet", "reset program", d::agent_setup_set);
}

#[test]
fn agent_setup_set__reset_config() {
    holds("agents_set", "agentSetupSet", "reset config", d::agent_setup_set);
}

#[test]
fn agent_setup_set__reset_all_three_in_the_patch_s_own_order() {
    holds("agents_set", "agentSetupSet", "reset all three, in the patch's own order", d::agent_setup_set);
}

#[test]
fn agent_setup_set__a_value_wins_over_a_reset() {
    holds("agents_set", "agentSetupSet", "a value wins over a reset", d::agent_setup_set);
}

#[test]
fn agent_setup_set__empty_args_with_a_reset_put_args_back() {
    holds("agents_set", "agentSetupSet", "empty args with a reset put args back", d::agent_setup_set);
}

#[test]
fn agent_setup_set__args_without_a_reset() {
    holds("agents_set", "agentSetupSet", "args without a reset", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_row_comes_back_with_its_setup() {
    holds("agents_set", "agentSetupSet", "the row comes back with its setup", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_row_has_no_setup() {
    holds("agents_set", "agentSetupSet", "the row has no setup", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_host_takes_the_setup_of_an_agent_not_in_the_catalog() {
    holds("agents_set", "agentSetupSet", "the host takes the setup of an agent not in the catalog", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_op_is_not_in_the_record() {
    holds("agents_set", "agentSetupSet", "the op is not in the record", d::agent_setup_set);
}

#[test]
fn agent_setup_set__the_same_with_the_cloud_on() {
    holds("agents_set", "agentSetupSet", "the same with the cloud on", d::agent_setup_set);
}

#[test]
fn agent_setup_lines__no_setup_reads_as_on() {
    holds("agents_set", "agentSetupLines", "no setup reads as on", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__on_with_nothing_set() {
    holds("agents_set", "agentSetupLines", "on with nothing set", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__off() {
    holds("agents_set", "agentSetupLines", "off", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__every_fact() {
    holds("agents_set", "agentSetupLines", "every fact", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__a_moved_config_folder_asks_for_a_sign_in() {
    holds("agents_set", "agentSetupLines", "a moved config folder asks for a sign-in", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__a_moved_config_folder_with_no_config_dir_says_nothing() {
    holds("agents_set", "agentSetupLines", "a moved config folder with no config dir says nothing", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__config_dir_not_moved_says_nothing_more() {
    holds("agents_set", "agentSetupLines", "config dir not moved says nothing more", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__empty_args_print_an_empty_list() {
    holds("agents_set", "agentSetupLines", "empty args print an empty list", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__one_variable() {
    holds("agents_set", "agentSetupLines", "one variable", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__control_characters_in_program_folder_and_args_are_cleaned() {
    holds("agents_set", "agentSetupLines", "control characters in program, folder and args are cleaned", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__the_row_s_own_name_is_used_not_the_catalog_s() {
    holds("agents_set", "agentSetupLines", "the row's own name is used, not the catalog's", d::agent_setup_lines);
}

#[test]
fn agent_setup_lines__the_same_with_the_cloud_on() {
    holds("agents_set", "agentSetupLines", "the same with the cloud on", d::agent_setup_lines);
}

#[test]
fn project_defaults_set__set() {
    holds("agents_set", "projectDefaultsSet", "set", d::project_defaults_set);
}

#[test]
fn project_defaults_set__put_back() {
    holds("agents_set", "projectDefaultsSet", "put back", d::project_defaults_set);
}

#[test]
fn project_defaults_set__no_defaults_answered() {
    holds("agents_set", "projectDefaultsSet", "no defaults answered", d::project_defaults_set);
}

#[test]
fn project_defaults_set__defaults_answered_for_another_project_only() {
    holds("agents_set", "projectDefaultsSet", "defaults answered for another project only", d::project_defaults_set);
}
