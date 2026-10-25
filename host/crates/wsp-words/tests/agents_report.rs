// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/agents-report.test.ts: the control-character case (:70-77), the command-words case
//! (:130-137) and the one assertion of the tools-answer case (:48) that calls serverToolsLateRefusal. The rest of that
//! file parses wire shapes (zod) and stays with wsp-wire.

use wsp_words::agents_report::*;

mod the_agents_report_on_the_wire {
    use super::*;

    #[test]
    fn a_tools_answer_carries_the_tools_or_the_harness_that_holds_the_sign_in_or_why_nothing_ca() {
        assert_eq!(server_tools_late_refusal(20_000.0), "Did not answer in 20 s.");
    }

    #[test]
    fn a_name_holding_a_control_character_is_caught_by_one_predicate_whichever_one_it_holds_and() {
        for c in ["\u{0}", "\u{3}", "\u{15}", "\r", "\n", "\u{1b}", "\u{7f}", "\u{80}", "\u{9b}", "\u{9f}"] {
            assert!(has_control_char(&format!("notion{c}echo hi")), "{c:?}");
        }
        assert_eq!(without_control_chars("no\u{1b}[2Jti\u{9b}31mon\u{85}\u{7f} \u{fc}"), "no[2Jti31mon \u{fc}");
        assert!(!has_control_char("notion-2 (work)"));
        assert!(!has_control_char("\u{fc}n\u{ef}code"));
        assert_eq!(
            control_name_refusal("~/.claude.json"),
            "~/.claude.json names a server with a control character in its name, which was left out."
        );
    }

    #[test]
    fn a_command_line_a_person_types_splits_into_its_words_quotes_kept_together_and_nothing_exp() {
        let words = |line: &str| command_words(line);
        let owned = |w: &[&str]| Some(w.iter().map(|s| s.to_string()).collect::<Vec<String>>());
        assert_eq!(words("npx -y @acme/mcp"), owned(&["npx", "-y", "@acme/mcp"]));
        assert_eq!(words("  uvx  'a b'  \"c \\\" d\" e\\ f $HOME "), owned(&["uvx", "a b", "c \" d", "e f", "$HOME"]));
        assert_eq!(words("node '' x"), owned(&["node", "", "x"]));
        assert_eq!(words(""), owned(&[]));
        assert_eq!(words("x \"unclosed"), None);
        assert_eq!(words("x 'unclosed"), None);
    }
}
