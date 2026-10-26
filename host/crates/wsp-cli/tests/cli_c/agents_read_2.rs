// SPDX-License-Identifier: AGPL-3.0-only
// Written by tests/fixtures/cli_c/record/gentests.py from agents_read.json: one test per recorded case.
use super::common::holds;
use super::drivers_agents_read as d;

#[test]
fn plugin_changed__on_a_thread_s_computer() {
    holds("agents_read", "pluginChanged", "on a thread's computer", d::plugin_changed);
}

#[test]
fn plugin_changed__on_a_named_computer() {
    holds("agents_read", "pluginChanged", "on a named computer", d::plugin_changed);
}

#[test]
fn plugin_changed__a_thread_and_a_computer_are_refused() {
    holds("agents_read", "pluginChanged", "a thread and a computer are refused", d::plugin_changed);
}

#[test]
fn plugin_changed__the_host_refuses() {
    holds("agents_read", "pluginChanged", "the host refuses", d::plugin_changed);
}

#[test]
fn plugin_changed__the_reply_s_extra_keys_are_dropped() {
    holds("agents_read", "pluginChanged", "the reply's extra keys are dropped", d::plugin_changed);
}

#[test]
fn plugin_changed__the_socket_goes() {
    holds("agents_read", "pluginChanged", "the socket goes", d::plugin_changed);
}

#[test]
fn server_tools_of__here() {
    holds("agents_read", "serverToolsOf", "here", d::server_tools_of);
}

#[test]
fn server_tools_of__refresh_is_sent_only_when_true() {
    holds("agents_read", "serverToolsOf", "refresh is sent only when true", d::server_tools_of);
}

#[test]
fn server_tools_of__held() {
    holds("agents_read", "serverToolsOf", "held", d::server_tools_of);
}

#[test]
fn server_tools_of__refused_by_the_server() {
    holds("agents_read", "serverToolsOf", "refused by the server", d::server_tools_of);
}

#[test]
fn server_tools_of__on_a_thread() {
    holds("agents_read", "serverToolsOf", "on a thread", d::server_tools_of);
}

#[test]
fn server_tools_of__on_a_computer_with_a_project() {
    holds("agents_read", "serverToolsOf", "on a computer with a project", d::server_tools_of);
}

#[test]
fn server_tools_of__on_a_computer_with_no_project() {
    holds("agents_read", "serverToolsOf", "on a computer with no project", d::server_tools_of);
}

#[test]
fn server_tools_of__a_project_with_no_computer_is_not_sent() {
    holds("agents_read", "serverToolsOf", "a project with no computer is not sent", d::server_tools_of);
}

#[test]
fn server_tools_of__a_thread_and_a_computer_are_refused() {
    holds("agents_read", "serverToolsOf", "a thread and a computer are refused", d::server_tools_of);
}

#[test]
fn server_tools_of__an_empty_server_name_is_sent() {
    holds("agents_read", "serverToolsOf", "an empty server name is sent", d::server_tools_of);
}

#[test]
fn server_tools_of__the_host_refuses() {
    holds("agents_read", "serverToolsOf", "the host refuses", d::server_tools_of);
}

#[test]
fn server_tools_of__extra_keys_in_the_answer_are_dropped() {
    holds("agents_read", "serverToolsOf", "extra keys in the answer are dropped", d::server_tools_of);
}

#[test]
fn server_changed__add_here() {
    holds("agents_read", "serverChanged", "add here", d::server_changed);
}

#[test]
fn server_changed__remove_here() {
    holds("agents_read", "serverChanged", "remove here", d::server_changed);
}

#[test]
fn server_changed__toggle_here() {
    holds("agents_read", "serverChanged", "toggle here", d::server_changed);
}

#[test]
fn server_changed__the_body_s_key_order_is_kept_after_the_target() {
    holds("agents_read", "serverChanged", "the body's key order is kept after the target", d::server_changed);
}

#[test]
fn server_changed__on_a_thread_with_a_project() {
    holds("agents_read", "serverChanged", "on a thread with a project", d::server_changed);
}

#[test]
fn server_changed__on_a_computer_with_a_project_name() {
    holds("agents_read", "serverChanged", "on a computer with a project name", d::server_changed);
}

#[test]
fn server_changed__a_thread_and_a_computer_are_refused() {
    holds("agents_read", "serverChanged", "a thread and a computer are refused", d::server_changed);
}

#[test]
fn server_changed__the_answer_is_cut_to_its_file() {
    holds("agents_read", "serverChanged", "the answer is cut to its file", d::server_changed);
}

#[test]
fn server_changed__the_host_refuses() {
    holds("agents_read", "serverChanged", "the host refuses", d::server_changed);
}

#[test]
fn server_changed__the_socket_goes() {
    holds("agents_read", "serverChanged", "the socket goes", d::server_changed);
}

#[test]
fn server_values__nothing_asked() {
    holds("agents_read", "serverValues", "nothing asked", d::server_values);
}

#[test]
fn server_values__one_variable() {
    holds("agents_read", "serverValues", "one variable", d::server_values);
}

#[test]
fn server_values__two_variables_in_the_order_named() {
    holds("agents_read", "serverValues", "two variables in the order named", d::server_values);
}

#[test]
fn server_values__a_variable_the_environment_lacks() {
    holds("agents_read", "serverValues", "a variable the environment lacks", d::server_values);
}

#[test]
fn server_values__the_first_lacking_variable_is_the_one_refused() {
    holds("agents_read", "serverValues", "the first lacking variable is the one refused", d::server_values);
}

#[test]
fn server_values__an_empty_value_is_a_value() {
    holds("agents_read", "serverValues", "an empty value is a value", d::server_values);
}

#[test]
fn server_values__a_name_repeated_keeps_its_first_place() {
    holds("agents_read", "serverValues", "a name repeated keeps its first place", d::server_values);
}

#[test]
fn server_values__a_name_that_is_a_whole_number_goes_first_in_the_object() {
    holds("agents_read", "serverValues", "a name that is a whole number goes first in the object", d::server_values);
}

#[test]
fn server_values__two_whole_number_names_go_first_in_numeric_order() {
    holds("agents_read", "serverValues", "two whole-number names go first in numeric order", d::server_values);
}

#[test]
fn server_values__one_header() {
    holds("agents_read", "serverValues", "one header", d::server_values);
}

#[test]
fn server_values__two_headers() {
    holds("agents_read", "serverValues", "two headers", d::server_values);
}

#[test]
fn server_values__a_header_and_a_variable() {
    holds("agents_read", "serverValues", "a header and a variable", d::server_values);
}

#[test]
fn server_values__a_header_whose_variable_is_lacking() {
    holds("agents_read", "serverValues", "a header whose variable is lacking", d::server_values);
}

#[test]
fn server_values__a_header_with_no_equals_sign() {
    holds("agents_read", "serverValues", "a header with no equals sign", d::server_values);
}

#[test]
fn server_values__a_header_with_nothing_before_the_equals_sign() {
    holds("agents_read", "serverValues", "a header with nothing before the equals sign", d::server_values);
}

#[test]
fn server_values__a_header_with_nothing_after_the_equals_sign() {
    holds("agents_read", "serverValues", "a header with nothing after the equals sign", d::server_values);
}

#[test]
fn server_values__a_header_that_is_a_lone_equals_sign() {
    holds("agents_read", "serverValues", "a header that is a lone equals sign", d::server_values);
}

#[test]
fn server_values__a_header_with_two_equals_signs_reads_the_rest_as_the_variabl() {
    holds("agents_read", "serverValues", "a header with two equals signs reads the rest as the variable", d::server_values);
}

#[test]
fn server_values__a_header_with_two_equals_signs_and_no_such_variable() {
    holds("agents_read", "serverValues", "a header with two equals signs and no such variable", d::server_values);
}

#[test]
fn server_values__the_variables_are_read_before_the_headers() {
    holds("agents_read", "serverValues", "the variables are read before the headers", d::server_values);
}

#[test]
fn server_values__a_bad_header_before_a_lacking_variable() {
    holds("agents_read", "serverValues", "a bad header before a lacking variable", d::server_values);
}

#[test]
fn server_values__the_headers_stop_at_the_first_bad_one() {
    holds("agents_read", "serverValues", "the headers stop at the first bad one", d::server_values);
}

#[test]
fn server_values__a_header_repeated_keeps_its_first_place_and_last_value() {
    holds("agents_read", "serverValues", "a header repeated keeps its first place and last value", d::server_values);
}

#[test]
fn server_values__an_environment_with_an_empty_name() {
    holds("agents_read", "serverValues", "an environment with an empty name", d::server_values);
}

#[test]
fn server_values__a_value_with_a_newline_and_an_emoji() {
    holds("agents_read", "serverValues", "a value with a newline and an emoji", d::server_values);
}

#[test]
fn server_command__no_line() {
    holds("agents_read", "serverCommand", "no line", d::server_command);
}

#[test]
fn server_command__one_word() {
    holds("agents_read", "serverCommand", "one word", d::server_command);
}

#[test]
fn server_command__a_program_and_arguments() {
    holds("agents_read", "serverCommand", "a program and arguments", d::server_command);
}

#[test]
fn server_command__extra_spaces() {
    holds("agents_read", "serverCommand", "extra spaces", d::server_command);
}

#[test]
fn server_command__a_single_quoted_word() {
    holds("agents_read", "serverCommand", "a single-quoted word", d::server_command);
}

#[test]
fn server_command__a_double_quoted_word_with_an_escape() {
    holds("agents_read", "serverCommand", "a double-quoted word with an escape", d::server_command);
}

#[test]
fn server_command__a_backslash_outside_quotes() {
    holds("agents_read", "serverCommand", "a backslash outside quotes", d::server_command);
}

#[test]
fn server_command__an_unclosed_single_quote() {
    holds("agents_read", "serverCommand", "an unclosed single quote", d::server_command);
}

#[test]
fn server_command__an_unclosed_double_quote() {
    holds("agents_read", "serverCommand", "an unclosed double quote", d::server_command);
}

#[test]
fn server_command__an_empty_line() {
    holds("agents_read", "serverCommand", "an empty line", d::server_command);
}

#[test]
fn server_command__a_line_of_spaces() {
    holds("agents_read", "serverCommand", "a line of spaces", d::server_command);
}

#[test]
fn server_command__an_empty_quoted_word() {
    holds("agents_read", "serverCommand", "an empty quoted word", d::server_command);
}

#[test]
fn server_command__a_tab_separates_nothing() {
    holds("agents_read", "serverCommand", "a tab separates nothing", d::server_command);
}

#[test]
fn server_command__an_emoji_argument() {
    holds("agents_read", "serverCommand", "an emoji argument", d::server_command);
}

#[test]
fn server_scope__no_word_no_project() {
    holds("agents_read", "serverScope", "no word, no project", d::server_scope);
}

#[test]
fn server_scope__no_word_project_asked_reads_project() {
    holds("agents_read", "serverScope", "no word, project asked reads project", d::server_scope);
}

#[test]
fn server_scope__no_word_project_asked_with_a_name() {
    holds("agents_read", "serverScope", "no word, project asked with a name", d::server_scope);
}

#[test]
fn server_scope__no_word_project_not_asked() {
    holds("agents_read", "serverScope", "no word, project not asked", d::server_scope);
}

#[test]
fn server_scope__user() {
    holds("agents_read", "serverScope", "user", d::server_scope);
}

#[test]
fn server_scope__home() {
    holds("agents_read", "serverScope", "home", d::server_scope);
}

#[test]
fn server_scope__local() {
    holds("agents_read", "serverScope", "local", d::server_scope);
}

#[test]
fn server_scope__project() {
    holds("agents_read", "serverScope", "project", d::server_scope);
}

#[test]
fn server_scope__an_unknown_word() {
    holds("agents_read", "serverScope", "an unknown word", d::server_scope);
}

#[test]
fn server_scope__a_word_in_capitals() {
    holds("agents_read", "serverScope", "a word in capitals", d::server_scope);
}

#[test]
fn server_scope__an_empty_word() {
    holds("agents_read", "serverScope", "an empty word", d::server_scope);
}

#[test]
fn server_scope__a_word_with_quotes_and_an_emoji() {
    holds("agents_read", "serverScope", "a word with quotes and an emoji", d::server_scope);
}

#[test]
fn server_scope__project_with_project() {
    holds("agents_read", "serverScope", "project with --project", d::server_scope);
}

#[test]
fn server_scope__local_with_project() {
    holds("agents_read", "serverScope", "local with --project", d::server_scope);
}

#[test]
fn server_scope__user_with_project_is_refused() {
    holds("agents_read", "serverScope", "user with --project is refused", d::server_scope);
}

#[test]
fn server_scope__home_with_project_is_refused() {
    holds("agents_read", "serverScope", "home with --project is refused", d::server_scope);
}

#[test]
fn server_scope__an_unknown_word_with_project_is_refused_as_a_scope_first() {
    holds("agents_read", "serverScope", "an unknown word with --project is refused as a scope first", d::server_scope);
}

#[test]
fn server_scope__a_word_with_project_false() {
    holds("agents_read", "serverScope", "a word with --project false", d::server_scope);
}

#[test]
fn add_tools__added() {
    holds("agents_read", "addTools", "added", d::add_tools);
}

#[test]
fn add_tools__the_answer_is_cut_to_its_file() {
    holds("agents_read", "addTools", "the answer is cut to its file", d::add_tools);
}

#[test]
fn add_tools__an_empty_agent_is_sent() {
    holds("agents_read", "addTools", "an empty agent is sent", d::add_tools);
}

#[test]
fn add_tools__an_agent_the_catalog_does_not_know() {
    holds("agents_read", "addTools", "an agent the catalog does not know", d::add_tools);
}

#[test]
fn add_tools__refused() {
    holds("agents_read", "addTools", "refused", d::add_tools);
}

#[test]
fn add_tools__the_socket_goes() {
    holds("agents_read", "addTools", "the socket goes", d::add_tools);
}

#[test]
fn draw_rows__one_frame_written_once_with_its_lines_joined() {
    holds("agents_read", "drawRows", "one frame, written once with its lines joined", d::draw_rows);
}

#[test]
fn draw_rows__under_json_the_value_is_the_line_and_the_rows_are_not_printe() {
    holds("agents_read", "drawRows", "under --json the value is the line and the rows are not printed", d::draw_rows);
}

#[test]
fn draw_rows__no_rows_prints_an_empty_line() {
    holds("agents_read", "drawRows", "no rows prints an empty line", d::draw_rows);
}

#[test]
fn draw_rows__one_row() {
    holds("agents_read", "drawRows", "one row", d::draw_rows);
}

#[test]
fn draw_rows__rows_with_an_emoji_and_a_newline_are_joined_as_given() {
    holds("agents_read", "drawRows", "rows with an emoji and a newline are joined as given", d::draw_rows);
}

#[test]
fn draw_rows__the_frame_is_handed_the_socket_and_sends_its_read() {
    holds("agents_read", "drawRows", "the frame is handed the socket and sends its read", d::draw_rows);
}

#[test]
fn draw_rows__the_frame_s_refusal_rides_out_with_nothing_printed() {
    holds("agents_read", "drawRows", "the frame's refusal rides out with nothing printed", d::draw_rows);
}

#[test]
fn draw_rows__watch_off_is_not_a_watch() {
    holds("agents_read", "drawRows", "--watch off is not a watch", d::draw_rows);
}

#[test]
fn draw_rows__watch_with_json_is_refused_before_any_dial() {
    holds("agents_read", "drawRows", "--watch with --json is refused before any dial", d::draw_rows);
}

#[test]
fn draw_rows__watch_with_no_terminal_to_redraw_on_is_refused_before_any_di() {
    holds("agents_read", "drawRows", "--watch with no terminal to redraw on is refused before any dial", d::draw_rows);
}

#[test]
fn draw_rows__watch_with_json_and_a_terminal_is_refused_for_json_first() {
    holds("agents_read", "drawRows", "--watch with --json and a terminal is refused for --json first", d::draw_rows);
}

#[test]
fn draw_rows__watch_with_a_different_line_s_words() {
    holds("agents_read", "drawRows", "--watch with a different line's words", d::draw_rows);
}

#[test]
fn draw_rows__watch_draws_one_frame_and_a_stop_ends_it_one_dial() {
    holds("agents_read", "drawRows", "--watch draws one frame and a stop ends it, one dial", d::draw_rows);
}

#[test]
fn draw_rows__watch_draws_every_frame_over_one_dial() {
    holds("agents_read", "drawRows", "--watch draws every frame over one dial", d::draw_rows);
}

#[test]
fn draw_rows__watch_with_no_rows_draws_nothing_but_the_cursor() {
    holds("agents_read", "drawRows", "--watch with no rows draws nothing but the cursor", d::draw_rows);
}

#[test]
fn draw_rows__watch_whose_frame_is_refused_restores_the_cursor_and_refuses() {
    holds("agents_read", "drawRows", "--watch whose frame is refused restores the cursor and refuses", d::draw_rows);
}
