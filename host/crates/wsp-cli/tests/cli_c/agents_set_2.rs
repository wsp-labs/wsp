// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from agents_set.json: one test per recorded case.
use super::common::holds;
use super::drivers_agents_set as d;

#[test]
fn project_defaults_set__nothing_to_change() {
    holds("agents_set", "projectDefaultsSet", "nothing to change", d::project_defaults_set);
}

#[test]
fn project_defaults_set__empty_trackers_and_no_reset_are_nothing_to_change() {
    holds("agents_set", "projectDefaultsSet", "empty trackers and no reset are nothing to change", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_blank_after_worktree_command() {
    holds("agents_set", "projectDefaultsSet", "a blank after-worktree command", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_empty_after_worktree_command_is_blank() {
    holds("agents_set", "projectDefaultsSet", "an empty after-worktree command is blank", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_tab_and_a_newline_are_blank() {
    holds("agents_set", "projectDefaultsSet", "a tab and a newline are blank", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_after_worktree_command_is_sent_untrimmed() {
    holds("agents_set", "projectDefaultsSet", "an after-worktree command is sent untrimmed", d::project_defaults_set);
}

#[test]
fn project_defaults_set__after_worktree_beside_a_model_the_patch_order_is_fixed() {
    holds("agents_set", "projectDefaultsSet", "after-worktree beside a model, the patch order is fixed", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_blank_command_is_refused_before_a_bad_tracker() {
    holds("agents_set", "projectDefaultsSet", "a blank command is refused before a bad tracker", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_bad_tracker_is_refused_before_a_bad_reset_word() {
    holds("agents_set", "projectDefaultsSet", "a bad tracker is refused before a bad reset word", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_bad_reset_word_is_refused_before_an_access_word() {
    holds("agents_set", "projectDefaultsSet", "a bad reset word is refused before an access word", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_bad_access_word_is_refused_before_any_frame() {
    holds("agents_set", "projectDefaultsSet", "a bad access word is refused before any frame", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_access_word_is_refused_before_the_project_is_resolved_eve() {
    holds(
        "agents_set",
        "projectDefaultsSet",
        "an access word is refused before the project is resolved even when it is the only field",
        d::project_defaults_set,
    );
}

#[test]
fn project_defaults_set__nothing_to_change_is_refused_before_the_project_is_resolved() {
    holds("agents_set", "projectDefaultsSet", "nothing to change is refused before the project is resolved", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_lab_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"lab\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"acme/\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_map_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"/map\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_a_b_c_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"a/b/c\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_lab_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \" acme/lab\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_lab_names_no_repo_2() {
    holds("agents_set", "projectDefaultsSet", "tracker \"acme/lab \" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_lab_names_no_repo_3() {
    holds("agents_set", "projectDefaultsSet", "tracker \"acme /lab\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_t_lab_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"acme\\t/lab\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_n_lab_names_no_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker \"acme\\n/lab\" names no repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_acme_lab_is_a_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker acme/lab is a repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_a_b_is_a_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker a/b is a repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_my_org_my_repo_is_a_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker my-org/my.repo is a repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_u_e9_u_e9_is_a_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker U+E9/U+E9 is a repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__tracker_a_b_c_is_a_repo() {
    holds("agents_set", "projectDefaultsSet", "tracker a_b/c is a repo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__the_first_bad_tracker_is_named() {
    holds("agents_set", "projectDefaultsSet", "the first bad tracker is named", d::project_defaults_set);
}

#[test]
fn project_defaults_set__trackers_are_de_duplicated_in_first_seen_order() {
    holds("agents_set", "projectDefaultsSet", "trackers are de-duplicated in first-seen order", d::project_defaults_set);
}

#[test]
fn project_defaults_set__trackers_echo_with_the_whole_list() {
    holds("agents_set", "projectDefaultsSet", "trackers echo with the whole list", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_empty_trackers_list_with_a_reset_of_trackers_puts_it_back() {
    holds("agents_set", "projectDefaultsSet", "an empty trackers list with a reset of trackers puts it back", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_echo_with_an_empty_trackers_list_is_returned() {
    holds("agents_set", "projectDefaultsSet", "an echo with an empty trackers list is returned", d::project_defaults_set);
}

#[test]
fn project_defaults_set__reset_trackers_and_after_worktree() {
    holds("agents_set", "projectDefaultsSet", "reset trackers and after-worktree", d::project_defaults_set);
}

#[test]
fn project_defaults_set__reset_after_worktree_alone() {
    holds("agents_set", "projectDefaultsSet", "reset after-worktree alone", d::project_defaults_set);
}

#[test]
fn project_defaults_set__reset_every_field_in_the_patch_s_order() {
    holds("agents_set", "projectDefaultsSet", "reset every field, in the patch's order", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_value_wins_over_a_reset() {
    holds("agents_set", "projectDefaultsSet", "a value wins over a reset", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_reset_word_that_is_a_field_s_key_spelling() {
    holds("agents_set", "projectDefaultsSet", "a reset word that is a field's key spelling", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_singular_tracker_reset_word() {
    holds("agents_set", "projectDefaultsSet", "a singular tracker reset word", d::project_defaults_set);
}

#[test]
fn project_defaults_set__an_agent_s_reset_word_is_refused() {
    holds("agents_set", "projectDefaultsSet", "an agent's reset word is refused", d::project_defaults_set);
}

#[test]
fn project_defaults_set__every_field_set() {
    holds("agents_set", "projectDefaultsSet", "every field set", d::project_defaults_set);
}

#[test]
fn project_defaults_set__access_auto_edit() {
    holds("agents_set", "projectDefaultsSet", "access auto-edit", d::project_defaults_set);
}

#[test]
fn project_defaults_set__access_plan_goes_to_the_host() {
    holds("agents_set", "projectDefaultsSet", "access plan goes to the host", d::project_defaults_set);
}

#[test]
fn project_defaults_set__only_another_project_s_overrides_echo() {
    holds("agents_set", "projectDefaultsSet", "only another project's overrides echo", d::project_defaults_set);
}

#[test]
fn project_defaults_set__no_such_project() {
    holds("agents_set", "projectDefaultsSet", "no such project", d::project_defaults_set);
}

#[test]
fn project_defaults_set__the_host_refuses_the_overrides_the_defaults_are_not_read() {
    holds("agents_set", "projectDefaultsSet", "the host refuses the overrides: the defaults are not read", d::project_defaults_set);
}

#[test]
fn project_defaults_set__the_defaults_read_is_refused() {
    holds("agents_set", "projectDefaultsSet", "the defaults read is refused", d::project_defaults_set);
}

#[test]
fn project_defaults_set__a_project_name_with_control_characters_in_the_missing_defaul() {
    holds(
        "agents_set",
        "projectDefaultsSet",
        "a project name with control characters in the missing-defaults error",
        d::project_defaults_set,
    );
}

#[test]
fn project_defaults_set__the_defaults_carry_only_some_of_the_picks() {
    holds("agents_set", "projectDefaultsSet", "the defaults carry only some of the picks", d::project_defaults_set);
}

#[test]
fn project_defaults_set__the_same_with_the_cloud_on() {
    holds("agents_set", "projectDefaultsSet", "the same with the cloud on", d::project_defaults_set);
}

#[test]
fn project_defaults_set__the_op_is_not_in_the_record() {
    holds("agents_set", "projectDefaultsSet", "the op is not in the record", d::project_defaults_set);
}

#[test]
fn defaults_cell__nothing_read() {
    holds("agents_set", "defaultsCell", "nothing read", d::defaults_cell);
}

#[test]
fn defaults_cell__an_agent_only() {
    holds("agents_set", "defaultsCell", "an agent only", d::defaults_cell);
}

#[test]
fn defaults_cell__every_pick_none_from_the_project() {
    holds("agents_set", "defaultsCell", "every pick, none from the project", d::defaults_cell);
}

#[test]
fn defaults_cell__every_pick_from_the_project() {
    holds("agents_set", "defaultsCell", "every pick from the project", d::defaults_cell);
}

#[test]
fn defaults_cell__effort_is_not_in_the_cell() {
    holds("agents_set", "defaultsCell", "effort is not in the cell", d::defaults_cell);
}

#[test]
fn defaults_cell__the_model_absent_the_access_from_the_project() {
    holds("agents_set", "defaultsCell", "the model absent, the access from the project", d::defaults_cell);
}

#[test]
fn defaults_cell__the_agent_from_the_project_alone() {
    holds("agents_set", "defaultsCell", "the agent from the project alone", d::defaults_cell);
}

#[test]
fn defaults_cell__a_value_with_spaces_and_controls_is_not_cleaned() {
    holds("agents_set", "defaultsCell", "a value with spaces and controls is not cleaned", d::defaults_cell);
}

#[test]
fn defaults_cell__the_same_with_the_cloud_on() {
    holds("agents_set", "defaultsCell", "the same with the cloud on", d::defaults_cell);
}

#[test]
fn thread_defaults_lines__an_agent_only() {
    holds("agents_set", "threadDefaultsLines", "an agent only", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__every_pick() {
    holds("agents_set", "threadDefaultsLines", "every pick", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__the_agent_from_named() {
    holds("agents_set", "threadDefaultsLines", "the agent from named", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__the_agent_from_project() {
    holds("agents_set", "threadDefaultsLines", "the agent from project", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__the_agent_from_default() {
    holds("agents_set", "threadDefaultsLines", "the agent from default", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__the_agent_from_catalog() {
    holds("agents_set", "threadDefaultsLines", "the agent from catalog", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__an_agent_the_catalog_does_not_know_is_named_by_its_id() {
    holds("agents_set", "threadDefaultsLines", "an agent the catalog does not know is named by its id", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__a_model_alone_beside_the_agent() {
    holds("agents_set", "threadDefaultsLines", "a model alone beside the agent", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__an_effort_alone_beside_the_agent() {
    holds("agents_set", "threadDefaultsLines", "an effort alone beside the agent", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__an_access_alone_beside_the_agent_the_value_not_the_mode() {
    holds("agents_set", "threadDefaultsLines", "an access alone beside the agent: the value, not the mode", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__values_are_not_cleaned() {
    holds("agents_set", "threadDefaultsLines", "values are not cleaned", d::thread_defaults_lines);
}

#[test]
fn thread_defaults_lines__the_same_with_the_cloud_on() {
    holds("agents_set", "threadDefaultsLines", "the same with the cloud on", d::thread_defaults_lines);
}

#[test]
fn const_text__noskillhitsline() {
    holds("agents_set", "constText", "noSkillHitsLine", d::const_text);
}

#[test]
fn const_text__skill_hit_columns() {
    holds("agents_set", "constText", "SKILL_HIT_COLUMNS", d::const_text);
}

#[test]
fn const_text__agent_set_resets() {
    holds("agents_set", "constText", "AGENT_SET_RESETS", d::const_text);
}

#[test]
fn const_text__project_set_resets() {
    holds("agents_set", "constText", "PROJECT_SET_RESETS", d::const_text);
}

#[test]
fn const_text__agent_setup_resets() {
    holds("agents_set", "constText", "AGENT_SETUP_RESETS", d::const_text);
}

#[test]
fn const_text__agentsetnothingline() {
    holds("agents_set", "constText", "agentSetNothingLine", d::const_text);
}

#[test]
fn const_text__nothing_to_set_fix() {
    holds("agents_set", "constText", "NOTHING_TO_SET_FIX", d::const_text);
}

#[test]
fn const_text__env_name_fix() {
    holds("agents_set", "constText", "ENV_NAME_FIX", d::const_text);
}

#[test]
fn const_text__agentsetupnothingline() {
    holds("agents_set", "constText", "agentSetupNothingLine", d::const_text);
}

#[test]
fn const_text__setup_words() {
    holds("agents_set", "constText", "SETUP_WORDS", d::const_text);
}

#[test]
fn const_text__projectsetnothingline() {
    holds("agents_set", "constText", "projectSetNothingLine", d::const_text);
}

#[test]
fn const_text__tracker_shape_fix() {
    holds("agents_set", "constText", "TRACKER_SHAPE_FIX", d::const_text);
}

#[test]
fn const_text__afterworktreeblankline() {
    holds("agents_set", "constText", "afterWorktreeBlankLine", d::const_text);
}

#[test]
fn const_text__after_worktree_blank_fix() {
    holds("agents_set", "constText", "AFTER_WORKTREE_BLANK_FIX", d::const_text);
}

#[test]
fn const_text__from_words() {
    holds("agents_set", "constText", "FROM_WORDS", d::const_text);
}

#[test]
fn const_text__defaults_labels() {
    holds("agents_set", "constText", "DEFAULTS_LABELS", d::const_text);
}

#[test]
fn const_text__skill_change_words() {
    holds("agents_set", "constText", "SKILL_CHANGE_WORDS", d::const_text);
}

#[test]
fn const_text__agents_on_words() {
    holds("agents_set", "constText", "AGENTS_ON_WORDS", d::const_text);
}

#[test]
fn const_text__agents_read_words() {
    holds("agents_set", "constText", "AGENTS_READ_WORDS", d::const_text);
}

#[test]
fn const_text__the_same_words_with_the_cloud_on() {
    holds("agents_set", "constText", "the same words with the cloud on", d::const_text);
}

#[test]
fn sentence__previewcutline_65() {
    holds("agents_set", "sentence", "previewCutLine [65]", d::sentence);
}

#[test]
fn sentence__previewcutline_1() {
    holds("agents_set", "sentence", "previewCutLine [1]", d::sentence);
}

#[test]
fn sentence__previewcutline_0() {
    holds("agents_set", "sentence", "previewCutLine [0]", d::sentence);
}

#[test]
fn sentence__skillsshplacelessline_o_r_s() {
    holds("agents_set", "sentence", "skillsShPlacelessLine [\"o/r/s\"]", d::sentence);
}

#[test]
fn sentence__isinline_pdf_agents_skills_pdf() {
    holds("agents_set", "sentence", "isInLine [\"pdf\",\"~/.agents/skills/pdf\"]", d::sentence);
}

#[test]
fn sentence__isinalsoline_pdf_a_claude_s_b_codex_s_c() {
    holds("agents_set", "sentence", "isInAlsoLine [\"pdf\",\"~/a\",\"claude's ~/b, codex's ~/c\"]", d::sentence);
}

#[test]
fn sentence__agentcopywords_claude_code_claude_skills_pdf() {
    holds("agents_set", "sentence", "agentCopyWords [\"Claude Code\",\"~/.claude/skills/pdf\"]", d::sentence);
}

#[test]
fn sentence__gonefromline_pdf_a_b() {
    holds("agents_set", "sentence", "goneFromLine [\"pdf\",\"~/a, ~/b\"]", d::sentence);
}

#[test]
fn sentence__turnedinline_pdf_true_a_skill_md() {
    holds("agents_set", "sentence", "turnedInLine [\"pdf\",true,\"~/a/SKILL.md\"]", d::sentence);
}

#[test]
fn sentence__turnedinline_pdf_false_a_skill_md() {
    holds("agents_set", "sentence", "turnedInLine [\"pdf\",false,\"~/a/SKILL.md\"]", d::sentence);
}

#[test]
fn sentence__turnedline_pdf_true() {
    holds("agents_set", "sentence", "turnedLine [\"pdf\",true]", d::sentence);
}

#[test]
fn sentence__turnedline_pdf_false() {
    holds("agents_set", "sentence", "turnedLine [\"pdf\",false]", d::sentence);
}

#[test]
fn sentence__picker_words_hide_a_b() {
    holds("agents_set", "sentence", "PICKER_WORDS.hide [\"a b\"]", d::sentence);
}

#[test]
fn sentence__picker_words_order_a_b() {
    holds("agents_set", "sentence", "PICKER_WORDS.order [\"a b\"]", d::sentence);
}

#[test]
fn sentence__picker_words_custom_a_b() {
    holds("agents_set", "sentence", "PICKER_WORDS.custom [\"a b\"]", d::sentence);
}

#[test]
fn sentence__startsonownline_claude_code() {
    holds("agents_set", "sentence", "startsOnOwnLine [\"Claude Code\"]", d::sentence);
}

#[test]
fn sentence__startsonline_claude_code_opus_high() {
    holds("agents_set", "sentence", "startsOnLine [\"Claude Code\",\"opus; high\"]", d::sentence);
}

#[test]
fn sentence__defaultagentline_codex() {
    holds("agents_set", "sentence", "defaultAgentLine [\"codex\"]", d::sentence);
}

#[test]
fn sentence__defaultagentline_zed_x() {
    holds("agents_set", "sentence", "defaultAgentLine [\"zed-x\"]", d::sentence);
}

#[test]
fn sentence__envnameline_unset_env_1_bad() {
    holds("agents_set", "sentence", "envNameLine [\"--unset-env\",\"\\\"1 BAD\\\"\"]", d::sentence);
}

#[test]
fn sentence__setuponline_claude_code_true() {
    holds("agents_set", "sentence", "setupOnLine [\"Claude Code\",true]", d::sentence);
}

#[test]
fn sentence__setuponline_claude_code_false() {
    holds("agents_set", "sentence", "setupOnLine [\"Claude Code\",false]", d::sentence);
}

#[test]
fn sentence__trackershapeline_lab() {
    holds("agents_set", "sentence", "trackerShapeLine [\"lab\"]", d::sentence);
}

#[test]
fn sentence__trackershapeline_a_b_n() {
    holds("agents_set", "sentence", "trackerShapeLine [\"a\\\"b\\n\"]", d::sentence);
}

#[test]
fn sentence__trackersline_acme_lab() {
    holds("agents_set", "sentence", "trackersLine [[\"acme/lab\"]]", d::sentence);
}

#[test]
fn sentence__trackersline_acme_lab_acme_map() {
    holds("agents_set", "sentence", "trackersLine [[\"acme/lab\",\"acme/map\"]]", d::sentence);
}

#[test]
fn sentence__trackersline() {
    holds("agents_set", "sentence", "trackersLine [[]]", d::sentence);
}

#[test]
fn sentence__afterworktreeline_make_setup() {
    holds("agents_set", "sentence", "afterWorktreeLine [\"make setup\"]", d::sentence);
}

#[test]
fn sentence__nodefaultsansweredline_wsp() {
    holds("agents_set", "sentence", "noDefaultsAnsweredLine [\"wsp\"]", d::sentence);
}

#[test]
fn sentence__newthreadsheadline_wsp() {
    holds("agents_set", "sentence", "newThreadsHeadLine [\"wsp\"]", d::sentence);
}

#[test]
fn sentence__defaultsvalueline_agent_codex_this_project_s() {
    holds("agents_set", "sentence", "defaultsValueLine [\"agent\",\"Codex\",\"this project's\"]", d::sentence);
}

#[test]
fn sentence__newthreadsheadline_with_the_cloud_on() {
    holds("agents_set", "sentence", "newThreadsHeadLine with the cloud on", d::sentence);
}
