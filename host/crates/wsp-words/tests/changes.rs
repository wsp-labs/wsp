// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/changes.test.ts whole (lines 1-73): the checkout fact's words, the diff a draft is
//! asked from, the question itself, and the answer read back as a commit message.

use serde_json::{json, Value};
use wsp_wire::types::changes::Checkout;
use wsp_words::changes::*;

fn fact(fields: Value) -> Checkout {
    let mut base = json!({ "branch": "fix/cart", "ahead": 0, "behind": 0, "changed": 0, "readAt": 1 });
    for (k, v) in fields.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

fn counted(c: &Checkout) -> Vec<String> {
    checkout_counts(c)
}

mod the_counts_after_a_branch {
    use super::*;

    #[test]
    fn names_only_what_the_checkout_holds_uncommitted_never_how_far_the_branch_is_ahead() {
        assert_eq!(counted(&fact(json!({ "ahead": 2, "behind": 1, "changed": 3 }))), vec!["3 changed".to_string()]);
        assert_eq!(counted(&fact(json!({ "ahead": 4 }))), Vec::<String>::new());
        assert_eq!(counted(&fact(json!({}))), Vec::<String>::new());
    }

    #[test]
    fn says_the_changes_were_not_read_where_a_stopped_copy_s_edits_were_not() {
        assert_eq!(counted(&fact(json!({ "ahead": 1, "editsUnread": true }))), vec![checkout_words::UNREAD.to_string()]);
        assert_eq!(counted(&fact(json!({ "ahead": 4, "behind": 2, "countsUnknown": true, "changed": 2 }))), vec!["2 changed".to_string()]);
    }
}

mod the_diff_a_draft_is_asked_from {
    use super::*;

    #[test]
    fn is_left_whole_under_the_cap_and_cut_at_the_last_line_end_inside_it_over_the_cap() {
        assert_eq!(cut_diff("a\nb\n", Some(100.0)), "a\nb\n");
        assert_eq!(cut_diff("one\ntwo\nthree\n", Some(9.0)), "one\ntwo\n");
    }

    #[test]
    fn counts_bytes_so_a_letter_of_two_bytes_is_never_cut_in_half() {
        let cut = cut_diff("é\né\né\n", Some(5.0));
        assert_eq!(cut, "é\n");
        assert!(cut.len() <= 5, "{cut}");
    }
}

mod the_draft_question {
    use super::*;

    #[test]
    fn asks_for_a_subject_under_72_characters_a_blank_line_two_to_five_lines_of_why_and() {
        let asked = draft_prompt("diff --git a/x b/x\n+one\n", None);
        assert!(asked.contains("under 72 characters"), "{asked}");
        assert!(asked.contains("two to five lines"), "{asked}");
        assert!(asked.contains("No em dashes"), "{asked}");
        assert!(asked.ends_with("diff --git a/x b/x\n+one\n"), "{asked}");
        assert!(!asked.contains("The task"), "{asked}");
    }

    #[test]
    fn carries_the_task_the_thread_was_opened_with_where_there_is_one() {
        let asked = draft_prompt("+x\n", Some("Fix the flaky cart test"));
        assert!(asked.contains("The task the agent was given:\nFix the flaky cart test"), "{asked}");
    }
}

mod the_answer_read_as_a_commit_message {
    use super::*;

    #[test]
    fn keeps_the_subject_one_blank_line_and_the_body() {
        assert_eq!(
            commit_message("Round the cart total once\n\nThe total rounded per line.\nNow it rounds at the end.\n"),
            Some("Round the cart total once\n\nThe total rounded per line.\nNow it rounds at the end.".to_string())
        );
        assert_eq!(commit_message("Round the cart total once"), Some("Round the cart total once".to_string()));
    }

    #[test]
    fn takes_off_a_fence_or_quotes_around_the_message_and_turns_an_em_dash_into_a_comma() {
        assert_eq!(commit_message("```\nFix the cart\n\nWhy it broke.\n```"), Some("Fix the cart\n\nWhy it broke.".to_string()));
        assert_eq!(commit_message("\"Fix the cart\""), Some("Fix the cart".to_string()));
        assert_eq!(
            commit_message("Fix the cart \u{2014} rounding\n\nIt rounded twice \u{2014} once per line."),
            Some("Fix the cart, rounding\n\nIt rounded twice, once per line.".to_string())
        );
    }

    #[test]
    fn cuts_a_subject_past_72_characters_to_the_words_that_fit_and_reads_an_empty_answe() {
        let long = format!("{}\n\nbody", "word ".repeat(20).trim());
        let message = commit_message(&long).unwrap();
        let subject = message.split('\n').next().unwrap();
        assert!(subject.len() <= 72, "{subject}");
        assert!(subject.ends_with("word"), "{subject}");
        assert_eq!(commit_message("  \n "), None);
    }
}
