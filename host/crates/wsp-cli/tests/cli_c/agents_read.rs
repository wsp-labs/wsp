// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from agents_read.json: one test per recorded case.
use super::common::holds;
use super::drivers_agents_read as d;

#[test]
fn const_aimed_both_line__the_line() {
    holds("agents_read", "constAimedBothLine", "the line", d::const_aimed_both_line);
}

#[test]
fn const_project_unnamed_line__the_line() {
    holds("agents_read", "constProjectUnnamedLine", "the line", d::const_project_unnamed_line);
}

#[test]
fn const_tools_project_bare_line__the_line() {
    holds("agents_read", "constToolsProjectBareLine", "the line", d::const_tools_project_bare_line);
}

#[test]
fn const_plugin_kind_words__kinds_one_and_many() {
    holds("agents_read", "constPluginKindWords", "kinds, one and many", d::const_plugin_kind_words);
}

#[test]
fn const_server_tool_columns__the_two_columns() {
    holds("agents_read", "constServerToolColumns", "the two columns", d::const_server_tool_columns);
}

#[test]
fn aimed_usage__tool_servers() {
    holds("agents_read", "aimedUsage", "tool \"servers\"", d::aimed_usage);
}

#[test]
fn aimed_usage__tool_plugins() {
    holds("agents_read", "aimedUsage", "tool \"plugins\"", d::aimed_usage);
}

#[test]
fn aimed_usage__tool() {
    holds("agents_read", "aimedUsage", "tool \"\"", d::aimed_usage);
}

#[test]
fn aimed_usage__tool_a_b_n() {
    holds("agents_read", "aimedUsage", "tool \"a b\\n??\"", d::aimed_usage);
}

#[test]
fn project_off_computer_line__value_wsp() {
    holds("agents_read", "projectOffComputerLine", "value \"wsp\"", d::project_off_computer_line);
}

#[test]
fn project_off_computer_line__value() {
    holds("agents_read", "projectOffComputerLine", "value \"\"", d::project_off_computer_line);
}

#[test]
fn project_off_computer_line__value_a_b() {
    holds("agents_read", "projectOffComputerLine", "value \"a b\"", d::project_off_computer_line);
}

#[test]
fn project_off_computer_line__value_q() {
    holds("agents_read", "projectOffComputerLine", "value \"\\\"q\\\" ??\"", d::project_off_computer_line);
}

#[test]
fn plugin_turned_words__vercel_m_on_for_claude() {
    holds("agents_read", "pluginTurnedWords", "vercel@m on for claude", d::plugin_turned_words);
}

#[test]
fn plugin_turned_words__vercel_m_off_for_claude() {
    holds("agents_read", "pluginTurnedWords", "vercel@m off for claude", d::plugin_turned_words);
}

#[test]
fn plugin_turned_words__x_on_for_codex() {
    holds("agents_read", "pluginTurnedWords", "x on for codex", d::plugin_turned_words);
}

#[test]
fn plugin_turned_words__x_off_for_not_in_catalog() {
    holds("agents_read", "pluginTurnedWords", "x off for not-in-catalog", d::plugin_turned_words);
}

#[test]
fn plugin_turned_words__empty_on_for_empty() {
    holds("agents_read", "pluginTurnedWords", "empty on for empty", d::plugin_turned_words);
}

#[test]
fn plugin_turned_line__on_for_claude() {
    holds("agents_read", "pluginTurnedLine", "on for claude", d::plugin_turned_line);
}

#[test]
fn plugin_turned_line__off_for_claude() {
    holds("agents_read", "pluginTurnedLine", "off for claude", d::plugin_turned_line);
}

#[test]
fn plugin_turned_line__on_for_codex() {
    holds("agents_read", "pluginTurnedLine", "on for codex", d::plugin_turned_line);
}

#[test]
fn plugin_turned_line__off_for_not_in_catalog() {
    holds("agents_read", "pluginTurnedLine", "off for not-in-catalog", d::plugin_turned_line);
}

#[test]
fn unset_variable_line__variable_linear_token() {
    holds("agents_read", "unsetVariableLine", "variable \"LINEAR_TOKEN\"", d::unset_variable_line);
}

#[test]
fn unset_variable_line__variable() {
    holds("agents_read", "unsetVariableLine", "variable \"\"", d::unset_variable_line);
}

#[test]
fn unset_variable_line__variable_a_b_n() {
    holds("agents_read", "unsetVariableLine", "variable \"a b\\n??\"", d::unset_variable_line);
}

#[test]
fn not_header_line__pair_authorization() {
    holds("agents_read", "notHeaderLine", "pair \"Authorization\"", d::not_header_line);
}

#[test]
fn not_header_line__pair_var() {
    holds("agents_read", "notHeaderLine", "pair \"=VAR\"", d::not_header_line);
}

#[test]
fn not_header_line__pair_name() {
    holds("agents_read", "notHeaderLine", "pair \"Name=\"", d::not_header_line);
}

#[test]
fn not_header_line__pair() {
    holds("agents_read", "notHeaderLine", "pair \"\"", d::not_header_line);
}

#[test]
fn not_header_line__pair_a_b_n() {
    holds("agents_read", "notHeaderLine", "pair \"a b\\n??\"", d::not_header_line);
}

#[test]
fn project_scope_line__word_user() {
    holds("agents_read", "projectScopeLine", "word \"user\"", d::project_scope_line);
}

#[test]
fn project_scope_line__word_home() {
    holds("agents_read", "projectScopeLine", "word \"home\"", d::project_scope_line);
}

#[test]
fn project_scope_line__word() {
    holds("agents_read", "projectScopeLine", "word \"\"", d::project_scope_line);
}

#[test]
fn project_scope_line__word_a_b_n() {
    holds("agents_read", "projectScopeLine", "word \"a b\\n??\"", d::project_scope_line);
}

#[test]
fn tools_added_line__agent_claude_code_file_claude_json() {
    holds("agents_read", "toolsAddedLine", "agent \"Claude Code\" file \"~/.claude.json\"", d::tools_added_line);
}

#[test]
fn tools_added_line__agent_codex_file() {
    holds("agents_read", "toolsAddedLine", "agent \"Codex\" file \"\"", d::tools_added_line);
}

#[test]
fn tools_added_line__agent_file_x() {
    holds("agents_read", "toolsAddedLine", "agent \"\" file \"~/x\"", d::tools_added_line);
}

#[test]
fn tools_added_line__agent_a_n_file() {
    holds("agents_read", "toolsAddedLine", "agent \"a\\n??\" file \"~/?\"", d::tools_added_line);
}

#[test]
fn signed_in_line__claude_code_in_signedin() {
    holds("agents_read", "signedInLine", "Claude Code in signedIn", d::signed_in_line);
}

#[test]
fn signed_in_line__claude_code_in_signedin_detail() {
    holds("agents_read", "signedInLine", "Claude Code in signedIn+detail", d::signed_in_line);
}

#[test]
fn signed_in_line__claude_code_in_signedin_detail_2() {
    holds("agents_read", "signedInLine", "Claude Code in signedIn+detail", d::signed_in_line);
}

#[test]
fn signed_in_line__codex_out_signedin() {
    holds("agents_read", "signedInLine", "Codex out signedIn", d::signed_in_line);
}

#[test]
fn signed_in_line__codex_out_signedin_said() {
    holds("agents_read", "signedInLine", "Codex out signedIn+said", d::signed_in_line);
}

#[test]
fn signed_in_line__codex_out_signedin_said_2() {
    holds("agents_read", "signedInLine", "Codex out signedIn+said", d::signed_in_line);
}

#[test]
fn signed_in_line__linear_in_signedin_detail_said() {
    holds("agents_read", "signedInLine", "linear in signedIn+detail+said", d::signed_in_line);
}

#[test]
fn signed_in_line__empty_out_signedin_said() {
    holds("agents_read", "signedInLine", "empty out signedIn+said", d::signed_in_line);
}

#[test]
fn project_asked__no_value() {
    holds("agents_read", "projectAsked", "no value", d::project_asked);
}

#[test]
fn project_asked__false() {
    holds("agents_read", "projectAsked", "false", d::project_asked);
}

#[test]
fn project_asked__false_with_a_computer() {
    holds("agents_read", "projectAsked", "false with a computer", d::project_asked);
}

#[test]
fn project_asked__true_is_the_thread_s_own_project() {
    holds("agents_read", "projectAsked", "true is the thread's own project", d::project_asked);
}

#[test]
fn project_asked__true_with_nothing_named() {
    holds("agents_read", "projectAsked", "true with nothing named", d::project_asked);
}

#[test]
fn project_asked__true_with_a_computer_is_refused() {
    holds("agents_read", "projectAsked", "true with a computer is refused", d::project_asked);
}

#[test]
fn project_asked__true_with_a_thread_and_a_computer_is_refused() {
    holds("agents_read", "projectAsked", "true with a thread and a computer is refused", d::project_asked);
}

#[test]
fn project_asked__the_empty_string_reads_as_true() {
    holds("agents_read", "projectAsked", "the empty string reads as true", d::project_asked);
}

#[test]
fn project_asked__the_empty_string_with_a_computer_is_refused() {
    holds("agents_read", "projectAsked", "the empty string with a computer is refused", d::project_asked);
}

#[test]
fn project_asked__a_name_with_a_computer() {
    holds("agents_read", "projectAsked", "a name with a computer", d::project_asked);
}

#[test]
fn project_asked__a_name_with_a_computer_and_a_thread() {
    holds("agents_read", "projectAsked", "a name with a computer and a thread", d::project_asked);
}

#[test]
fn project_asked__a_name_with_no_computer_is_refused() {
    holds("agents_read", "projectAsked", "a name with no computer is refused", d::project_asked);
}

#[test]
fn project_asked__a_name_with_a_thread_and_no_computer_is_refused() {
    holds("agents_read", "projectAsked", "a name with a thread and no computer is refused", d::project_asked);
}

#[test]
fn project_asked__a_name_with_an_emoji_and_quotes_off_a_computer() {
    holds("agents_read", "projectAsked", "a name with an emoji and quotes off a computer", d::project_asked);
}

#[test]
fn tools_project__no_value() {
    holds("agents_read", "toolsProject", "no value", d::tools_project);
}

#[test]
fn tools_project__no_value_with_a_computer() {
    holds("agents_read", "toolsProject", "no value with a computer", d::tools_project);
}

#[test]
fn tools_project__the_empty_string_is_refused() {
    holds("agents_read", "toolsProject", "the empty string is refused", d::tools_project);
}

#[test]
fn tools_project__the_empty_string_with_a_computer_is_refused_the_same() {
    holds("agents_read", "toolsProject", "the empty string with a computer is refused the same", d::tools_project);
}

#[test]
fn tools_project__a_name_with_a_computer() {
    holds("agents_read", "toolsProject", "a name with a computer", d::tools_project);
}

#[test]
fn tools_project__a_name_with_no_computer_is_refused() {
    holds("agents_read", "toolsProject", "a name with no computer is refused", d::tools_project);
}

#[test]
fn tools_project__a_name_with_a_thread_and_a_computer() {
    holds("agents_read", "toolsProject", "a name with a thread and a computer", d::tools_project);
}

#[test]
fn agents_target__neither_names_a_thread_nor_a_computer() {
    holds("agents_read", "agentsTarget", "neither names a thread nor a computer", d::agents_target);
}

#[test]
fn agents_target__neither_with_a_project_asked_still_names_this_computer() {
    holds("agents_read", "agentsTarget", "neither, with a project asked, still names this computer", d::agents_target);
}

#[test]
fn agents_target__both_are_refused_before_any_frame() {
    holds("agents_read", "agentsTarget", "both are refused before any frame", d::agents_target);
}

#[test]
fn agents_target__a_thread_by_its_id_prefix() {
    holds("agents_read", "agentsTarget", "a thread by its id prefix", d::agents_target);
}

#[test]
fn agents_target__a_thread_by_its_whole_id() {
    holds("agents_read", "agentsTarget", "a thread by its whole id", d::agents_target);
}

#[test]
fn agents_target__a_thread_also_asked_with_a_project_name_is_the_workspace_alo() {
    holds("agents_read", "agentsTarget", "a thread also asked with a project name is the workspace alone", d::agents_target);
}

#[test]
fn agents_target__a_thread_that_names_nothing() {
    holds("agents_read", "agentsTarget", "a thread that names nothing", d::agents_target);
}

#[test]
fn agents_target__a_thread_that_names_nothing_the_cloud_on() {
    holds("agents_read", "agentsTarget", "a thread that names nothing, the cloud on", d::agents_target);
}

#[test]
fn agents_target__a_word_that_is_a_project_s_name() {
    holds("agents_read", "agentsTarget", "a word that is a project's name", d::agents_target);
}

#[test]
fn agents_target__a_word_that_is_a_project_s_name_the_cloud_on() {
    holds("agents_read", "agentsTarget", "a word that is a project's name, the cloud on", d::agents_target);
}

#[test]
fn agents_target__a_word_that_is_a_machine_s_name_the_cloud_on() {
    holds("agents_read", "agentsTarget", "a word that is a machine's name, the cloud on", d::agents_target);
}

#[test]
fn agents_target__a_word_that_is_a_machine_s_name_the_cloud_off() {
    holds("agents_read", "agentsTarget", "a word that is a machine's name, the cloud off", d::agents_target);
}

#[test]
fn agents_target__a_thread_whose_workspace_does_not_read_is_another_version() {
    holds("agents_read", "agentsTarget", "a thread whose workspace does not read is another version", d::agents_target);
}

#[test]
fn agents_target__a_computer_by_its_name() {
    holds("agents_read", "agentsTarget", "a computer by its name", d::agents_target);
}

#[test]
fn agents_target__a_computer_with_a_project() {
    holds("agents_read", "agentsTarget", "a computer with a project", d::agents_target);
}

#[test]
fn agents_target__a_computer_with_an_empty_project_name_is_sent_as_given() {
    holds("agents_read", "agentsTarget", "a computer with an empty project name is sent as given", d::agents_target);
}

#[test]
fn agents_target__this_computer_by_the_name_it_carries() {
    holds("agents_read", "agentsTarget", "this computer by the name it carries", d::agents_target);
}

#[test]
fn agents_target__a_computer_by_its_id() {
    holds("agents_read", "agentsTarget", "a computer by its id", d::agents_target);
}

#[test]
fn agents_target__a_computer_that_is_none() {
    holds("agents_read", "agentsTarget", "a computer that is none", d::agents_target);
}

#[test]
fn agents_target__the_computers_are_not_readable() {
    holds("agents_read", "agentsTarget", "the computers are not readable", d::agents_target);
}

#[test]
fn agents_target__an_empty_computer_name() {
    holds("agents_read", "agentsTarget", "an empty computer name", d::agents_target);
}

#[test]
fn agents_report__this_computer_every_part() {
    holds("agents_read", "agentsReport", "this computer, every part", d::agents_report);
}

#[test]
fn agents_report__a_report_with_only_what_is_required() {
    holds("agents_read", "agentsReport", "a report with only what is required", d::agents_report);
}

#[test]
fn agents_report__a_thread() {
    holds("agents_read", "agentsReport", "a thread", d::agents_report);
}

#[test]
fn agents_report__a_computer() {
    holds("agents_read", "agentsReport", "a computer", d::agents_report);
}

#[test]
fn agents_report__a_napping_workspace_s_kept_report() {
    holds("agents_read", "agentsReport", "a napping workspace's kept report", d::agents_report);
}

#[test]
fn agents_report__both_a_thread_and_a_computer_are_refused() {
    holds("agents_read", "agentsReport", "both a thread and a computer are refused", d::agents_report);
}

#[test]
fn agents_report__the_host_refuses_the_read() {
    holds("agents_read", "agentsReport", "the host refuses the read", d::agents_report);
}

#[test]
fn agents_report__the_read_is_refused_for_a_napping_machine() {
    holds("agents_read", "agentsReport", "the read is refused for a napping machine", d::agents_report);
}

#[test]
fn agents_report__a_project_word_is_refused_in_the_thread_s_place() {
    holds("agents_read", "agentsReport", "a project word is refused in the thread's place", d::agents_report);
}

#[test]
fn agents_report__keys_this_build_does_not_know_are_dropped() {
    holds("agents_read", "agentsReport", "keys this build does not know are dropped", d::agents_report);
}

#[test]
fn report_facts__only_what_is_required() {
    holds("agents_read", "reportFacts", "only what is required", d::report_facts);
}

#[test]
fn report_facts__every_optional_fact() {
    holds("agents_read", "reportFacts", "every optional fact", d::report_facts);
}

#[test]
fn report_facts__stale_without_a_pause_mode() {
    holds("agents_read", "reportFacts", "stale without a pause mode", d::report_facts);
}

#[test]
fn report_facts__a_pause_mode_without_stale() {
    holds("agents_read", "reportFacts", "a pause mode without stale", d::report_facts);
}

#[test]
fn report_facts__projects_that_are_none() {
    holds("agents_read", "reportFacts", "projects that are none", d::report_facts);
}

#[test]
fn report_facts__a_thread_s_target() {
    holds("agents_read", "reportFacts", "a thread's target", d::report_facts);
}

#[test]
fn report_facts__a_computer_s_target_with_a_project() {
    holds("agents_read", "reportFacts", "a computer's target with a project", d::report_facts);
}

#[test]
fn report_facts__rows_are_left_out() {
    holds("agents_read", "reportFacts", "rows are left out", d::report_facts);
}

#[test]
fn agent_row_lines__no_agents_leaves_the_head_alone() {
    holds("agents_read", "agentRowLines", "no agents leaves the head alone", d::agent_row_lines);
}

#[test]
fn agent_row_lines__one_signed_in_agent() {
    holds("agents_read", "agentRowLines", "one signed in agent", d::agent_row_lines);
}

#[test]
fn agent_row_lines__the_four_rows_of_the_recorded_report() {
    holds("agents_read", "agentRowLines", "the four rows of the recorded report", d::agent_row_lines);
}

#[test]
fn agent_row_lines__not_installed_reads_not_found_whatever_the_sign_in() {
    holds("agents_read", "agentRowLines", "not installed reads not found whatever the sign-in", d::agent_row_lines);
}

#[test]
fn agent_row_lines__not_installed_with_an_unknown_sign_in() {
    holds("agents_read", "agentRowLines", "not installed with an unknown sign-in", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_installed_agent_with_an_unknown_sign_in() {
    holds("agents_read", "agentRowLines", "an installed agent with an unknown sign-in", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_installed_agent_that_is_not_signed_in() {
    holds("agents_read", "agentRowLines", "an installed agent that is not signed in", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_installed_agent_on_a_vault_key() {
    holds("agents_read", "agentRowLines", "an installed agent on a vault key", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_version_wins_over_the_first_run_word() {
    holds("agents_read", "agentRowLines", "a version wins over the first-run word", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_version_installs_on_first_run() {
    holds("agents_read", "agentRowLines", "no version, installs on first run", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_version_version_unreadable() {
    holds("agents_read", "agentRowLines", "no version, version unreadable", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_version_and_both_words_first_run_wins() {
    holds("agents_read", "agentRowLines", "no version and both words, first run wins", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_version_at_all() {
    holds("agents_read", "agentRowLines", "no version at all", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_empty_version_is_a_version() {
    holds("agents_read", "agentRowLines", "an empty version is a version", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_latest() {
    holds("agents_read", "agentRowLines", "no latest", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_latest() {
    holds("agents_read", "agentRowLines", "a latest", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_update_with_its_command() {
    holds("agents_read", "agentRowLines", "an update with its command", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_update() {
    holds("agents_read", "agentRowLines", "no update", d::agent_row_lines);
}

#[test]
fn agent_row_lines__no_path() {
    holds("agents_read", "agentRowLines", "no path", d::agent_row_lines);
}

#[test]
fn agent_row_lines__wsp_tools_off() {
    holds("agents_read", "agentRowLines", "wsp tools off", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_name_wider_than_the_head() {
    holds("agents_read", "agentRowLines", "a name wider than the head", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_emoji_in_a_name_counts_two_cells() {
    holds("agents_read", "agentRowLines", "an emoji in a name counts two cells", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_path_with_accents() {
    holds("agents_read", "agentRowLines", "a path with accents", d::agent_row_lines);
}

#[test]
fn agent_row_lines__an_agent_id_the_catalog_does_not_know_prints_its_own_name() {
    holds("agents_read", "agentRowLines", "an agent id the catalog does not know prints its own name", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_stale_report_that_is_napping() {
    holds("agents_read", "agentRowLines", "a stale report that is napping", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_stale_report_that_naps_in_memory() {
    holds("agents_read", "agentRowLines", "a stale report that naps in memory", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_stale_report_that_naps_on_disk() {
    holds("agents_read", "agentRowLines", "a stale report that naps on disk", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_pause_mode_without_stale_says_nothing() {
    holds("agents_read", "agentRowLines", "a pause mode without stale says nothing", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_refused_reader() {
    holds("agents_read", "agentRowLines", "a refused reader", d::agent_row_lines);
}

#[test]
fn agent_row_lines__two_refused_readers_in_order() {
    holds("agents_read", "agentRowLines", "two refused readers in order", d::agent_row_lines);
}

#[test]
fn agent_row_lines__a_refused_line_is_made_a_cell() {
    holds("agents_read", "agentRowLines", "a refused line is made a cell", d::agent_row_lines);
}

#[test]
fn agent_row_lines__stale_then_refused_in_that_order() {
    holds("agents_read", "agentRowLines", "stale then refused, in that order", d::agent_row_lines);
}

#[test]
fn skill_row_lines__no_skills() {
    holds("agents_read", "skillRowLines", "no skills", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_user_skill() {
    holds("agents_read", "skillRowLines", "a user skill", d::skill_row_lines);
}

#[test]
fn skill_row_lines__the_recorded_three() {
    holds("agents_read", "skillRowLines", "the recorded three", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_project_skill_names_its_project() {
    holds("agents_read", "skillRowLines", "a project skill names its project", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_skill_with_a_project_but_a_user_scope_reads_project() {
    holds("agents_read", "skillRowLines", "a skill with a project but a user scope reads project", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_skill_with_a_project_and_a_plugin_scope_reads_project() {
    holds("agents_read", "skillRowLines", "a skill with a project and a plugin scope reads project", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_plugin_skill() {
    holds("agents_read", "skillRowLines", "a plugin skill", d::skill_row_lines);
}

#[test]
fn skill_row_lines__two_folders_joined_by_a_comma() {
    holds("agents_read", "skillRowLines", "two folders joined by a comma", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_link_shows_its_target() {
    holds("agents_read", "skillRowLines", "a link shows its target", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_link_and_a_plain_folder() {
    holds("agents_read", "skillRowLines", "a link and a plain folder", d::skill_row_lines);
}

#[test]
fn skill_row_lines__an_empty_link_target_is_still_a_link() {
    holds("agents_read", "skillRowLines", "an empty link target is still a link", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_path_with_control_characters_is_a_cell() {
    holds("agents_read", "skillRowLines", "a path with control characters is a cell", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_name_with_a_newline() {
    holds("agents_read", "skillRowLines", "a name with a newline", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_project_name_with_control_characters() {
    holds("agents_read", "skillRowLines", "a project name with control characters", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_wide_name_against_the_head() {
    holds("agents_read", "skillRowLines", "a wide name against the head", d::skill_row_lines);
}

#[test]
fn skill_row_lines__an_emoji_name_counts_two_cells() {
    holds("agents_read", "skillRowLines", "an emoji name counts two cells", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_napping_report() {
    holds("agents_read", "skillRowLines", "a napping report", d::skill_row_lines);
}

#[test]
fn skill_row_lines__no_skills_and_a_refusal() {
    holds("agents_read", "skillRowLines", "no skills and a refusal", d::skill_row_lines);
}

#[test]
fn skill_row_lines__a_description_is_not_printed() {
    holds("agents_read", "skillRowLines", "a description is not printed", d::skill_row_lines);
}

#[test]
fn server_row_lines__no_servers() {
    holds("agents_read", "serverRowLines", "no servers", d::server_row_lines);
}

#[test]
fn server_row_lines__an_http_server() {
    holds("agents_read", "serverRowLines", "an http server", d::server_row_lines);
}

#[test]
fn server_row_lines__a_stdio_server() {
    holds("agents_read", "serverRowLines", "a stdio server", d::server_row_lines);
}

#[test]
fn server_row_lines__the_recorded_four() {
    holds("agents_read", "serverRowLines", "the recorded four", d::server_row_lines);
}

#[test]
fn server_row_lines__a_launch_row() {
    holds("agents_read", "serverRowLines", "a launch row", d::server_row_lines);
}

#[test]
fn server_row_lines__a_launch_row_beats_disabled() {
    holds("agents_read", "serverRowLines", "a launch row beats disabled", d::server_row_lines);
}

#[test]
fn server_row_lines__a_launch_row_beats_off() {
    holds("agents_read", "serverRowLines", "a launch row beats off", d::server_row_lines);
}

#[test]
fn server_row_lines__a_launch_row_beats_not_in_recipe() {
    holds("agents_read", "serverRowLines", "a launch row beats not in recipe", d::server_row_lines);
}

#[test]
fn server_row_lines__disabled() {
    holds("agents_read", "serverRowLines", "disabled", d::server_row_lines);
}

#[test]
fn server_row_lines__disabled_and_not_in_recipe() {
    holds("agents_read", "serverRowLines", "disabled and not in recipe", d::server_row_lines);
}

#[test]
fn server_row_lines__off_while_enabled() {
    holds("agents_read", "serverRowLines", "off while enabled", d::server_row_lines);
}

#[test]
fn server_row_lines__off_and_not_in_recipe() {
    holds("agents_read", "serverRowLines", "off and not in recipe", d::server_row_lines);
}

#[test]
fn server_row_lines__disabled_beats_off_and_prints_no_off_line() {
    holds("agents_read", "serverRowLines", "disabled beats off and prints no off line", d::server_row_lines);
}

#[test]
fn server_row_lines__not_in_recipe_with_an_auth() {
    holds("agents_read", "serverRowLines", "not in recipe with an auth", d::server_row_lines);
}

#[test]
fn server_row_lines__in_recipe_says_nothing() {
    holds("agents_read", "serverRowLines", "in recipe says nothing", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_open() {
    holds("agents_read", "serverRowLines", "auth open", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_env_key() {
    holds("agents_read", "serverRowLines", "auth env-key", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_connected() {
    holds("agents_read", "serverRowLines", "auth connected", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_signed_in() {
    holds("agents_read", "serverRowLines", "auth signed-in", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_needs_sign_in() {
    holds("agents_read", "serverRowLines", "auth needs-sign-in", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_failed() {
    holds("agents_read", "serverRowLines", "auth failed", d::server_row_lines);
}

#[test]
fn server_row_lines__auth_unknown() {
    holds("agents_read", "serverRowLines", "auth unknown", d::server_row_lines);
}

#[test]
fn server_row_lines__project_scope_names_its_project() {
    holds("agents_read", "serverRowLines", "project scope names its project", d::server_row_lines);
}

#[test]
fn server_row_lines__local_scope_names_its_project_as_local() {
    holds("agents_read", "serverRowLines", "local scope names its project as local", d::server_row_lines);
}

#[test]
fn server_row_lines__a_user_scope_with_a_project_reads_project() {
    holds("agents_read", "serverRowLines", "a user scope with a project reads project", d::server_row_lines);
}

#[test]
fn server_row_lines__a_home_scope() {
    holds("agents_read", "serverRowLines", "a home scope", d::server_row_lines);
}

#[test]
fn server_row_lines__a_project_name_with_control_characters() {
    holds("agents_read", "serverRowLines", "a project name with control characters", d::server_row_lines);
}

#[test]
fn server_row_lines__the_off_sentence_once_for_two_rows() {
    holds("agents_read", "serverRowLines", "the off sentence once for two rows", d::server_row_lines);
}

#[test]
fn server_row_lines__two_off_sentences_in_row_order() {
    holds("agents_read", "serverRowLines", "two off sentences in row order", d::server_row_lines);
}

#[test]
fn server_row_lines__an_off_sentence_made_a_cell() {
    holds("agents_read", "serverRowLines", "an off sentence made a cell", d::server_row_lines);
}

#[test]
fn server_row_lines__off_lines_come_before_the_stale_and_refused_lines() {
    holds("agents_read", "serverRowLines", "off lines come before the stale and refused lines", d::server_row_lines);
}

#[test]
fn server_row_lines__an_agent_the_catalog_does_not_know() {
    holds("agents_read", "serverRowLines", "an agent the catalog does not know", d::server_row_lines);
}

#[test]
fn server_row_lines__an_agent_the_catalog_knows_by_name() {
    holds("agents_read", "serverRowLines", "an agent the catalog knows by name", d::server_row_lines);
}

#[test]
fn server_row_lines__an_empty_off_sentence_is_an_off_server_with_an_empty_line() {
    holds("agents_read", "serverRowLines", "an empty off sentence is an off server with an empty line", d::server_row_lines);
}

#[test]
fn server_row_lines__a_wide_name_against_the_head() {
    holds("agents_read", "serverRowLines", "a wide name against the head", d::server_row_lines);
}

#[test]
fn server_row_lines__an_emoji_in_a_name_counts_two_cells() {
    holds("agents_read", "serverRowLines", "an emoji in a name counts two cells", d::server_row_lines);
}

#[test]
fn server_row_lines__a_cloud_machine_s_report_servers_that_need_the_person_s_sign() {
    holds("agents_read", "serverRowLines", "a cloud machine's report: servers that need the person's sign-in are off", d::server_row_lines);
}

#[test]
fn server_row_lines__no_servers_and_a_refusal() {
    holds("agents_read", "serverRowLines", "no servers and a refusal", d::server_row_lines);
}

#[test]
fn plugin_row_lines__no_plugins_key_reads_as_none() {
    holds("agents_read", "pluginRowLines", "no plugins key reads as none", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__an_empty_list() {
    holds("agents_read", "pluginRowLines", "an empty list", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__one_plugin_on() {
    holds("agents_read", "pluginRowLines", "one plugin on", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__one_plugin_off() {
    holds("agents_read", "pluginRowLines", "one plugin off", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__the_recorded_four() {
    holds("agents_read", "pluginRowLines", "the recorded four", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__missing_and_on() {
    holds("agents_read", "pluginRowLines", "missing and on", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__missing_and_off() {
    holds("agents_read", "pluginRowLines", "missing and off", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__missing_uninstalled() {
    holds("agents_read", "pluginRowLines", "missing uninstalled", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_project_s_plugin_names_its_project() {
    holds("agents_read", "pluginRowLines", "a project's plugin names its project", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_local_plugin_names_its_project_as_local() {
    holds("agents_read", "pluginRowLines", "a local plugin names its project as local", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_user_plugin_with_a_project_reads_project() {
    holds("agents_read", "pluginRowLines", "a user plugin with a project reads project", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__no_version() {
    holds("agents_read", "pluginRowLines", "no version", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_version() {
    holds("agents_read", "pluginRowLines", "a version", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__an_id_with_control_characters_is_a_cell() {
    holds("agents_read", "pluginRowLines", "an id with control characters is a cell", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_wide_id_against_the_head() {
    holds("agents_read", "pluginRowLines", "a wide id against the head", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__an_emoji_id_counts_two_cells() {
    holds("agents_read", "pluginRowLines", "an emoji id counts two cells", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__a_napping_report() {
    holds("agents_read", "pluginRowLines", "a napping report", d::plugin_row_lines);
}

#[test]
fn plugin_row_lines__no_plugins_and_a_refusal() {
    holds("agents_read", "pluginRowLines", "no plugins and a refusal", d::plugin_row_lines);
}

#[test]
fn brought_words__nothing() {
    holds("agents_read", "broughtWords", "nothing", d::brought_words);
}

#[test]
fn brought_words__one_skill() {
    holds("agents_read", "broughtWords", "one skill", d::brought_words);
}

#[test]
fn brought_words__two_skills() {
    holds("agents_read", "broughtWords", "two skills", d::brought_words);
}

#[test]
fn brought_words__one_of_every_kind() {
    holds("agents_read", "broughtWords", "one of every kind", d::brought_words);
}

#[test]
fn brought_words__two_of_every_kind() {
    holds("agents_read", "broughtWords", "two of every kind", d::brought_words);
}

#[test]
fn brought_words__only_the_language_server() {
    holds("agents_read", "broughtWords", "only the language server", d::brought_words);
}

#[test]
fn brought_words__only_apps() {
    holds("agents_read", "broughtWords", "only apps", d::brought_words);
}

#[test]
fn brought_words__kinds_in_the_table_s_order_not_the_object_s() {
    holds("agents_read", "broughtWords", "kinds in the table's order, not the object's", d::brought_words);
}

#[test]
fn brought_words__an_empty_string_member_still_counts() {
    holds("agents_read", "broughtWords", "an empty-string member still counts", d::brought_words);
}

#[test]
fn brought_words__a_dozen() {
    holds("agents_read", "broughtWords", "a dozen", d::brought_words);
}

#[test]
fn tool_lines__tools_with_a_first_line_only_of_a_long_description() {
    holds("agents_read", "toolLines", "tools with a first line only of a long description", d::tool_lines);
}

#[test]
fn tool_lines__a_held_sign_in() {
    holds("agents_read", "toolLines", "a held sign-in", d::tool_lines);
}

#[test]
fn tool_lines__a_held_sign_in_with_tools() {
    holds("agents_read", "toolLines", "a held sign-in with tools", d::tool_lines);
}

#[test]
fn tool_lines__a_holder_the_catalog_does_not_know() {
    holds("agents_read", "toolLines", "a holder the catalog does not know", d::tool_lines);
}

#[test]
fn tool_lines__refused_by_the_server() {
    holds("agents_read", "toolLines", "refused by the server", d::tool_lines);
}

#[test]
fn tool_lines__refused_wins_over_tools() {
    holds("agents_read", "toolLines", "refused wins over tools", d::tool_lines);
}

#[test]
fn tool_lines__refused_with_a_holder() {
    holds("agents_read", "toolLines", "refused with a holder", d::tool_lines);
}

#[test]
fn tool_lines__not_yet_asked() {
    holds("agents_read", "toolLines", "not yet asked", d::tool_lines);
}

#[test]
fn tool_lines__no_tools() {
    holds("agents_read", "toolLines", "no tools", d::tool_lines);
}

#[test]
fn tool_lines__a_tool_with_no_description_reads_a_dash() {
    holds("agents_read", "toolLines", "a tool with no description reads a dash", d::tool_lines);
}

#[test]
fn tool_lines__an_empty_description_is_a_description() {
    holds("agents_read", "toolLines", "an empty description is a description", d::tool_lines);
}

#[test]
fn tool_lines__a_description_of_a_newline_then_text_reads_empty() {
    holds("agents_read", "toolLines", "a description of a newline then text reads empty", d::tool_lines);
}

#[test]
fn tool_lines__a_description_with_a_carriage_return() {
    holds("agents_read", "toolLines", "a description with a carriage return", d::tool_lines);
}

#[test]
fn tool_lines__a_description_with_a_tab_and_control_characters() {
    holds("agents_read", "toolLines", "a description with a tab and control characters", d::tool_lines);
}

#[test]
fn tool_lines__a_wide_tool_name_against_the_head() {
    holds("agents_read", "toolLines", "a wide tool name against the head", d::tool_lines);
}

#[test]
fn tool_lines__an_emoji_tool_name_counts_two_cells() {
    holds("agents_read", "toolLines", "an emoji tool name counts two cells", d::tool_lines);
}

#[test]
fn tool_lines__a_server_name_with_an_emoji_and_a_colon() {
    holds("agents_read", "toolLines", "a server name with an emoji and a colon", d::tool_lines);
}

#[test]
fn tool_lines__auth_open() {
    holds("agents_read", "toolLines", "auth open", d::tool_lines);
}

#[test]
fn tool_lines__auth_env_key() {
    holds("agents_read", "toolLines", "auth env-key", d::tool_lines);
}

#[test]
fn tool_lines__auth_connected() {
    holds("agents_read", "toolLines", "auth connected", d::tool_lines);
}

#[test]
fn tool_lines__auth_signed_in() {
    holds("agents_read", "toolLines", "auth signed-in", d::tool_lines);
}

#[test]
fn tool_lines__auth_needs_sign_in() {
    holds("agents_read", "toolLines", "auth needs-sign-in", d::tool_lines);
}

#[test]
fn tool_lines__auth_failed() {
    holds("agents_read", "toolLines", "auth failed", d::tool_lines);
}

#[test]
fn tool_lines__auth_unknown() {
    holds("agents_read", "toolLines", "auth unknown", d::tool_lines);
}

#[test]
fn plugin_changed__on_here() {
    holds("agents_read", "pluginChanged", "on, here", d::plugin_changed);
}

#[test]
fn plugin_changed__off_here() {
    holds("agents_read", "pluginChanged", "off, here", d::plugin_changed);
}

#[test]
fn plugin_changed__another_agent_off() {
    holds("agents_read", "pluginChanged", "another agent, off", d::plugin_changed);
}
