// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:494-547 (notifyLine, notifyTail, notifyBody, waitTimedOutLine),
//! :603-653 (a turn's activity in one line each), :744-780 (titleLine, openingTitle), :809-854 (titlePrompt,
//! generatedTitle), :918-927 (refusedTurn) and thread-read.test.ts:452-473 (threadRowLines, fmtClock, the two
//! silences, turnEndLine). Skipped: absent-computer.test.ts:20 and :65 (test absentComputer and ownDaemonDown in
//! computer.ts; ROW_LINE_MAX is incidental), format.test.ts:857 (storedTitleSource; needs attachedFilesPrompt, absent
//! from the stubs), :904 (refusedTurn; needs signInRefusalLine, absent from the stubs), thread-read.test.ts:323 and :359
//! (test threadResult and threadReplyRows in thread.ts; notifyLine and NEWER_TURN_LINE are incidental), :387 and :407
//! (need threadMessages and threadMarkdown, absent from the stubs).

use regex::Regex;
use serde_json::{json, Value};
use wsp_words::pending_wire::types::thread_read::ThreadMessage;
use wsp_words::pending_wire::views::session_events::TurnResult;
use wsp_words::words::turn::*;
use wsp_words::words::units::{fmt_tokens, LIST_PRICE_WORD};

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn result(v: Value) -> TurnResult {
    wire(v)
}

fn row(v: Value) -> ThreadMessage {
    wire(v)
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn utc(_: f64) -> i64 {
    0
}

fn clocks(duration_ms: Option<f64>, waited_ms: Option<f64>, cost_usd: Option<f64>) -> TurnClocks {
    TurnClocks { duration_ms, waited_ms, cost_usd }
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

const AT: f64 = 1_788_930_312_000.0;

mod notify_line {
    use super::*;

    const THREAD: &str = "c452d1e8-7a1b-4f2c-9e3d-000000000001";

    #[test]
    fn names_the_thread_by_its_first_eight_characters_then_the_outcome_duration_and_cos() {
        assert_eq!(
            notify_line(
                THREAD,
                &result(json!({ "status": "completed", "durationMs": 492_000, "costUsd": 1.94, "text": "Ran the gate.\n\nAll 12 tests green.\n" })),
                None
            ),
            "thread c452d1e8 finished (completed, 8m 12s, $1.94): All 12 tests green."
        );
    }

    #[test]
    fn failed_and_interrupted_carry_their_words_an_error_stands_in_for_a_reply_that_has() {
        assert_eq!(
            notify_line(THREAD, &result(json!({ "status": "failed", "durationMs": 3_000, "error": "the harness died" })), None),
            "thread c452d1e8 finished (failed, 3.0s): the harness died"
        );
        assert_eq!(
            notify_line(THREAD, &result(json!({ "status": "interrupted", "durationMs": 12_000, "costUsd": 0.03, "text": "Stopped mid-way." })), None),
            "thread c452d1e8 finished (interrupted, 12s, $0.03): Stopped mid-way."
        );
        assert_eq!(
            notify_line(THREAD, &result(json!({ "status": "failed", "error": "machine paused while the agent was working" })), None),
            "thread c452d1e8 finished (failed): machine paused while the agent was working"
        );
        assert_eq!(notify_line(THREAD, &result(json!({ "status": "interrupted" })), None), "thread c452d1e8 finished (interrupted)");
    }

    #[test]
    fn a_turn_that_did_not_complete_says_why_over_its_reply_s_last_line_one_that_did_sa() {
        assert_eq!(
            notify_line(
                THREAD,
                &result(json!({
                    "status": "failed",
                    "durationMs": 12_000,
                    "costUsd": 0.02,
                    "text": "Waiting for the gate to finish.",
                    "error": "stopped after 15m 00s with no output for 10m"
                })),
                None
            ),
            "thread c452d1e8 finished (failed, 12s, $0.02): stopped after 15m 00s with no output for 10m"
        );
        assert_eq!(
            notify_line(
                THREAD,
                &result(json!({ "status": "completed", "durationMs": 12_000, "text": "All green.", "error": "[ede_diagnostic] noise" })),
                None
            ),
            "thread c452d1e8 finished (completed, 12s): All green."
        );
    }

    #[test]
    fn notify_tail_is_the_line_s_tail_alone_the_one_rule_the_wait_s_reply_field_reads() {
        assert_eq!(
            notify_tail(&result(json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests   green.\n" }))),
            some("All 12 tests green.")
        );
        assert_eq!(
            notify_tail(&result(json!({
                "status": "failed",
                "text": "Waiting for the gate.",
                "error": "stopped after 15m 00s with no output for 10m"
            }))),
            some("stopped after 15m 00s with no output for 10m")
        );
        assert_eq!(
            notify_tail(&result(json!({ "status": "completed", "text": "All green.", "error": "[ede_diagnostic] noise" }))),
            some("All green.")
        );
        assert_eq!(notify_tail(&result(json!({ "status": "interrupted" }))), None);
    }

    #[test]
    fn the_whole_length_carries_the_final_message_entire_line_breaks_and_all_under_the() {
        assert_eq!(
            notify_line(
                THREAD,
                &result(json!({ "status": "completed", "durationMs": 492_000, "costUsd": 1.94, "text": "Ran the gate.\n\nAll 12 tests green.\n" })),
                Some(NotifyLength::Whole)
            ),
            "thread c452d1e8 finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green."
        );
        // A turn that did not complete still says why first, and a reply with nothing in it still leaves the line bare.
        assert_eq!(
            notify_line(
                THREAD,
                &result(json!({
                    "status": "failed",
                    "durationMs": 12_000,
                    "text": "Waiting for the gate.\nStill waiting.",
                    "error": "stopped after 15m 00s with no output for 10m"
                })),
                Some(NotifyLength::Whole)
            ),
            "thread c452d1e8 finished (failed, 12s): stopped after 15m 00s with no output for 10m"
        );
        assert_eq!(
            notify_line(THREAD, &result(json!({ "status": "completed", "text": "   \n\n  " })), Some(NotifyLength::Whole)),
            "thread c452d1e8 finished (completed)"
        );
        // Tail is the length a line takes when none is named, so every reader that had one keeps it.
        assert_eq!(
            notify_line(THREAD, &result(json!({ "status": "completed", "text": "one\ntwo" })), None),
            notify_line(THREAD, &result(json!({ "status": "completed", "text": "one\ntwo" })), Some(NotifyLength::Tail))
        );
    }

    #[test]
    fn notify_body_is_the_one_rule_both_lengths_read_the_tail_is_the_whole_cut_to_its_l() {
        let r = result(json!({ "status": "completed", "text": "Ran the gate.\n\nAll 12 tests   green.\n" }));
        assert_eq!(notify_body(&r, Some(NotifyLength::Whole)), some("Ran the gate.\n\nAll 12 tests   green."));
        assert_eq!(notify_body(&r, Some(NotifyLength::Tail)), some("All 12 tests green."));
        assert_eq!(notify_body(&r, None), notify_tail(&r));
    }
}

mod wait_timed_out_line {
    use super::*;

    const THREAD: &str = "c452d1e8-7a1b-4f2c-9e3d-000000000001";

    #[test]
    fn names_one_thread_by_its_first_eight_characters_and_counts_more_then_says_how_lon() {
        assert_eq!(wait_timed_out_line(&[THREAD], 600_000.0), "thread c452d1e8 still running after 10m");
        assert_eq!(wait_timed_out_line(&[THREAD, "5e6f7a8b-0000"], 30_000.0), "2 threads still running after 30s");
        assert_eq!(wait_timed_out_line(&[THREAD], 50.0), "thread c452d1e8 still running after 50ms");
    }
}

mod a_turn_s_activity_in_one_line_each {
    use super::*;

    #[test]
    fn ends_a_turn_on_the_same_words_the_app_s_footer_shows_in_the_app_s_order_and_leav() {
        assert_eq!(
            turn_settled_line(&result(json!({ "status": "completed", "durationMs": 72_000, "costUsd": 0.22 })), None),
            "completed  Worked for 1m 12s  $0.22"
        );
        assert_eq!(turn_settled_line(&result(json!({ "status": "failed" })), None), "failed");
        assert_eq!(
            turn_settled_line(&result(json!({ "status": "interrupted", "durationMs": 1_500 })), None),
            "interrupted  Worked for 1.5s"
        );
        assert_eq!(turn_settled_parts(&clocks(None, None, None), None, None), Vec::<String>::new());
        assert_eq!(turn_settled_parts(&clocks(Some(72_000.0), None, Some(0.22)), None, None), strs(&["Worked for 1m 12s", "$0.22"]));
    }

    #[test]
    fn says_a_turn_s_model_and_tokens_in_short_figures_and_nothing_the_agent_did_not_re() {
        assert_eq!(
            [0.0, 251.0, 999.0, 1_000.0, 4_269.0, 12_400.0, 22_564.0, 200_000.0, 999_600.0, 1_000_000.0, 1_250_000.0, 687_400_000.0, 7_369_100_000.0]
                .map(fmt_tokens),
            ["0", "251", "999", "1k", "4.27k", "12.4k", "22.6k", "200k", "1M", "1M", "1.25M", "687M", "7.37B"]
        );
        let sonnet = TurnFacts { model: some("claude-sonnet-4-5"), tokens: Some((12_400.0, 251.0)) };
        assert_eq!(turn_facts_parts(&sonnet, None), strs(&["claude-sonnet-4-5", "12.4k in", "251 out"]));
        // The app names the model the way its picker does; the command line has only the id.
        assert_eq!(turn_facts_parts(&sonnet, Some("Sonnet 4.5")), strs(&["Sonnet 4.5", "12.4k in", "251 out"]));
        assert_eq!(turn_facts_parts(&TurnFacts::default(), None), Vec::<String>::new());
        assert_eq!(turn_facts_parts(&TurnFacts { model: None, tokens: Some((10.0, 3.0)) }, None), strs(&["10 in", "3 out"]));
        // A read transcript's finished line says the same facts the footer does.
        assert_eq!(
            turn_end_line(&result(json!({
                "status": "completed",
                "durationMs": 72_000,
                "costUsd": 0.22,
                "model": "gpt-5.5",
                "tokens": { "input": 24_763, "output": 122 }
            }))),
            "completed  Worked for 1m 12s  $0.22  gpt-5.5  24.8k in  122 out"
        );
    }

    #[test]
    fn counts_work_in_worked_for_the_minutes_a_turn_stood_on_a_question_come_off_it_and() {
        // The harness reports wall time from launch to result, prompts included; the person's minutes are not the turn's.
        assert_eq!(
            turn_settled_parts(&clocks(Some(215_000.0), Some(211_000.0), Some(0.05)), None, None),
            strs(&["Worked for 4.0s", "waited on you 3m 31s", "$0.05"])
        );
        // A wait that is not most of the turn is taken off all the same; the turn has nothing to explain.
        assert_eq!(turn_settled_parts(&clocks(Some(72_000.0), Some(12_000.0), None), None, None), strs(&["Worked for 1m"]));
        // A turn nothing of it waited on reads exactly as it did before.
        assert_eq!(turn_settled_parts(&clocks(Some(72_000.0), None, None), None, None), strs(&["Worked for 1m 12s"]));
        // A harness whose figure is shorter than the wait the runtime clocked cannot make a turn work negative time.
        assert_eq!(
            turn_settled_parts(&clocks(Some(1_000.0), Some(60_000.0), None), None, None),
            strs(&["Worked for 1ms", "waited on you 1.0s"])
        );
        assert_eq!(waited_on_you_part(211_000.0), "waited on you 3m 31s");
        // The line a thread's end sends counts the same minutes the footer does; two readings of one turn cannot differ.
        assert_eq!(
            notify_line(
                "thr_abcd1234",
                &result(json!({ "status": "completed", "durationMs": 215_000, "waitedMs": 211_000, "costUsd": 0.05, "text": "done" })),
                None
            ),
            "thread thr_abcd finished (completed, 4.0s, $0.05): done"
        );
    }

    #[test]
    fn says_what_the_threads_a_thread_opened_spent_beside_its_own_figure_and_says_what() {
        assert_eq!(opened_spend_part(2.3), "$2.30 in threads it opened");
        assert_eq!(turn_spend_part(1.14, None), "$1.14 this turn");
        // One turn against whole threads: with both on the line each says its own scope.
        assert_eq!(
            turn_settled_parts(&clocks(Some(72_000.0), None, Some(1.14)), Some(2.3), None),
            strs(&["Worked for 1m 12s", "$1.14 this turn", "$2.30 in threads it opened"])
        );
        // Nothing opened anything, so there is no second figure to weigh the first against and the cost stands bare.
        assert_eq!(turn_settled_parts(&clocks(Some(72_000.0), None, Some(1.14)), None, None), strs(&["Worked for 1m 12s", "$1.14"]));
        assert_eq!(turn_settled_parts(&clocks(Some(72_000.0), None, Some(1.14)), Some(0.0), None), strs(&["Worked for 1m 12s", "$1.14"]));
    }

    #[test]
    fn a_surface_whose_turns_cost_the_person_nothing_puts_the_word_on_the_turn_s_own_fi() {
        assert_eq!(
            turn_settled_parts(&clocks(Some(72_000.0), None, Some(0.13)), None, Some(LIST_PRICE_WORD)),
            strs(&["Worked for 1m 12s", "$0.13 list price"])
        );
        assert_eq!(
            turn_settled_parts(&clocks(Some(72_000.0), None, Some(1.14)), Some(2.3), Some(LIST_PRICE_WORD)),
            strs(&["Worked for 1m 12s", "$1.14 list price this turn", "$2.30 in threads it opened"])
        );
        // The line a stream with no footer prints never carries a second figure, so it is untouched.
        assert_eq!(
            turn_settled_line(&result(json!({ "status": "completed", "durationMs": 72_000, "costUsd": 1.14 })), None),
            "completed  Worked for 1m 12s  $1.14"
        );
    }
}

mod title_line {
    use super::*;

    #[test]
    fn is_the_prompt_s_first_non_empty_line_with_its_whitespace_collapsed_so_a_multi_pa() {
        assert_eq!(
            title_line("You are a builder for the wsp repo.\n\nTicket: wsp-labs/wsp-map#292.\nBuild: the fix."),
            "You are a builder for the wsp repo."
        );
        assert_eq!(title_line("\r\n  \n\tReply  with\texactly   the word pong.  \r\n"), "Reply with exactly the word pong.");
        assert_eq!(title_line("one line"), "one line");
        assert_eq!(title_line("\n \n"), "");
    }
}

mod opening_title {
    use super::*;

    const BRIEF: &str = "You are a builder for the wsp repo, which is at /Users/dev/wsp on this Mac: read the ticket, then run `pnpm test` and report.\n\nTicket: wsp-labs/wsp-map#408.";

    #[test]
    fn is_the_opening_turn_s_first_sentence_so_a_brief_that_starts_with_a_whole_paragra() {
        assert_eq!(opening_title("Bump the lockfile. Then run the gate."), "Bump the lockfile.");
        assert_eq!(opening_title("Is the gate green? Say so."), "Is the gate green?");
        assert_eq!(
            opening_title("You are a builder for the wsp repo.\n\nTicket: wsp-labs/wsp-map#292."),
            "You are a builder for the wsp repo."
        );
        assert_eq!(opening_title("\r\n  \n\tReply  with\texactly   the word pong.  \r\n"), "Reply with exactly the word pong.");
        assert_eq!(opening_title("Bump to 1.2.3 and run the gate"), "Bump to 1.2.3 and run the gate");
    }

    #[test]
    fn cuts_a_long_first_sentence_at_a_word_boundary_to_at_most_48_characters_with_the() {
        assert_eq!(opening_title(BRIEF), "You are a builder for the wsp repo, which is at\u{2026}");
        assert!(utf16(&opening_title(BRIEF)) <= 48, "{}", opening_title(BRIEF));
        let exact = "Rename the thread by its opening words and stop.";
        assert_eq!(utf16(exact), 48);
        assert_eq!(opening_title(exact), exact);
        assert_eq!(opening_title(&format!("{} now.", &exact[..exact.len() - 1])), "Rename the thread by its opening words and stop\u{2026}");
        assert_eq!(
            opening_title("Ticket: wsp-map#408, four fixes in one round, the header glyph first."),
            "Ticket: wsp-map#408, four fixes in one round\u{2026}"
        );
    }

    #[test]
    fn cuts_one_word_longer_than_the_room_inside_it_and_an_empty_turn_is_an_empty_title() {
        let token = "a".repeat(60);
        assert_eq!(opening_title(&token), format!("{}\u{2026}", "a".repeat(47)));
        assert_eq!(opening_title("\n \n"), "");
    }
}

mod title_prompt {
    use super::*;

    #[test]
    fn asks_for_a_short_title_in_words_from_the_opening_turn_alone_cut_so_a_brief_never() {
        let prompt = title_prompt(&"a".repeat(900), None);
        assert!(prompt.contains("3 to 6 words"), "{prompt}");
        assert!(prompt.contains("The opening turn:"), "{prompt}");
        assert!(!prompt.contains("The reply:"), "{prompt}");
        assert!(prompt.contains(&format!("{}\u{2026}", "a".repeat(600))), "{prompt}");
        assert!(!prompt.contains(&"a".repeat(601)), "{prompt}");
    }

    #[test]
    fn carries_the_reply_too_when_the_caller_has_one_cut_the_same_way() {
        let prompt = title_prompt(&"a".repeat(900), Some(&"b".repeat(900)));
        assert!(prompt.contains("The reply:"), "{prompt}");
        assert!(prompt.contains(&format!("{}\u{2026}", "b".repeat(600))), "{prompt}");
        assert!(!prompt.contains(&"b".repeat(601)), "{prompt}");
    }
}

mod generated_title {
    use super::*;

    #[test]
    fn takes_a_one_line_answer_with_the_quotes_and_the_stop_a_model_wraps_it_in_taken_o() {
        assert_eq!(generated_title("  Seed thread titles from opening turn\n"), some("Seed thread titles from opening turn"));
        assert_eq!(generated_title("\"Thread titles through the harness\""), some("Thread titles through the harness"));
        assert_eq!(generated_title("\u{201c}Thread titles through the harness\u{201d}"), some("Thread titles through the harness"));
        assert_eq!(generated_title("Thread titles through the harness."), some("Thread titles through the harness"));
    }

    #[test]
    fn cuts_an_over_long_one_line_answer_to_its_first_words_under_the_cap_rather_than_r() {
        // 45 characters, the longest a claude-sonnet-5 answer ran when asked for 34 (measured over 80 threads).
        let long = "Wire the sidebar rows to the daemon's streams";
        assert_eq!(utf16(long), 45);
        assert_eq!(generated_title(long), some("Wire the sidebar rows to the daemon's"));
        assert!(utf16(&generated_title(long).unwrap()) <= GENERATED_TITLE_MAX);
        // A word that ends exactly at the cap is kept whole, and a comma left at the cut comes off with it.
        assert_eq!(generated_title(&format!("{} tail", "a".repeat(GENERATED_TITLE_MAX))), Some("a".repeat(GENERATED_TITLE_MAX)));
        assert_eq!(
            generated_title(&format!("{}, and then some", "a".repeat(GENERATED_TITLE_MAX - 2))),
            Some("a".repeat(GENERATED_TITLE_MAX - 2))
        );
        assert_eq!(generated_title(&"a".repeat(GENERATED_TITLE_MAX)), Some("a".repeat(GENERATED_TITLE_MAX)));
    }

    #[test]
    fn refuses_an_answer_that_is_not_a_title_so_the_thread_keeps_the_words_its_opening() {
        assert_eq!(generated_title("Sure! Here is a title:\nThread titles through the harness"), None);
        assert_eq!(generated_title("Thread titles\nthrough the harness"), None);
        // One word longer than the cap has no boundary to cut at, and its head would be no title.
        assert_eq!(generated_title(&"a".repeat(GENERATED_TITLE_MAX + 1)), None);
        assert_eq!(generated_title("   "), None);
        assert_eq!(generated_title("\".\""), None);
    }
}

mod refused_turn {
    use super::*;

    #[test]
    fn a_refusal_wsp_knows_no_road_out_of_keeps_the_agent_s_sentence_alone_and_is_class() {
        let road = RefusalRoad { road: some(""), cause: None };
        assert_eq!(
            refused_turn(&result(json!({ "status": "completed", "text": "  Overloaded  ", "tokens": { "input": 0, "output": 0 } })), Some(&road)),
            result(json!({ "status": "failed", "tokens": { "input": 0, "output": 0 }, "error": "Overloaded" }))
        );
        assert_eq!(
            refused_turn(&result(json!({ "status": "completed", "error": "the provider refused" })), None),
            result(json!({ "status": "failed", "error": "the provider refused" }))
        );
        assert_eq!(refused_turn(&result(json!({ "status": "completed" })), None), result(json!({ "status": "failed" })));
    }

    #[test]
    fn a_result_carrying_both_a_reply_and_a_harness_error_entry_says_the_reply_once_the() {
        assert_eq!(
            refused_turn(&result(json!({ "status": "completed", "text": "Not logged in", "error": "Not logged in" })), None),
            result(json!({ "status": "failed", "error": "Not logged in" }))
        );
    }
}

mod the_printout_a_reader_sees {
    use super::*;

    #[test]
    fn prints_a_reply_of_many_lines_as_the_agent_wrote_it_and_a_row_the_runtime_stamped() {
        assert_eq!(
            thread_row_lines(&row(json!({ "who": "agent", "at": AT, "text": "green\nbuilt it" })), &utc),
            vec![format!("agent {}", fmt_clock(Some(AT), &utc)), "green".to_string(), "built it".to_string()]
        );
        assert_eq!(thread_row_lines(&row(json!({ "who": "person", "text": "no clock" })), &utc), strs(&["person", "no clock"]));
    }

    #[test]
    fn reads_the_clock_to_the_second_in_the_zone_of_the_computer_reading_it_and_nothing() {
        // The TS compares with the zone of the test process; the zone here is UTC, so the clock of AT is 05:05:12.
        assert_eq!(fmt_clock(Some(AT), &utc), "05:05:12");
        assert!(Regex::new(r"^\d\d:\d\d:\d\d$").unwrap().is_match(&fmt_clock(Some(AT), &utc)));
        assert_eq!(fmt_clock(None, &utc), "");
    }

    #[test]
    fn says_which_silence_it_is_no_message_in_the_transcript_or_a_thread_that_has_not_r() {
        assert_eq!(no_messages_line("1a2b3c4d-0000"), "thread 1a2b3c4d has no messages in the transcript this host holds");
        assert_eq!(no_reply_line("1a2b3c4d-0000"), "thread 1a2b3c4d has not replied yet");
    }

    #[test]
    fn ends_a_turn_s_row_on_the_footer_the_chat_shows_with_the_reason_where_it_did_not() {
        assert_eq!(
            turn_end_line(&result(json!({ "status": "completed", "durationMs": 724_000, "costUsd": 0.41 }))),
            "completed  Worked for 12m 4s  $0.41"
        );
        assert_eq!(turn_end_line(&result(json!({ "status": "interrupted" }))), "interrupted");
        assert_eq!(turn_end_line(&result(json!({ "status": "failed", "error": "the harness died" }))), "failed: the harness died");
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::turn::*;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    use serde_json::json;
    use wsp_words::pending_wire::views::session::{TitleSource, TurnRefusal};
    use wsp_words::pending_wire::views::session_events::TurnResult;

    fn res(v: serde_json::Value) -> TurnResult {
        serde_json::from_value(v).unwrap()
    }

    fn utc(_at: f64) -> i64 {
        0
    }

    fn source_word(s: TitleSource) -> String {
        serde_json::to_value(s).unwrap().as_str().unwrap().to_string()
    }

    #[test]
    fn notify_reply_and_body_read_null_empty_and_blank_text_as_the_ts_does() {
        assert_eq!(notify_reply(&res(json!({"status":"completed"})), None), None); // notifyReply({"status":"completed"})
        assert_eq!(notify_reply(&res(json!({"status":"completed"})), Some(NotifyLength::Whole)), None); // notifyReply({"status":"completed"}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"completed"})), None), None); // notifyBody({"status":"completed"})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":""})), None), None); // notifyReply({"status":"completed","text":""})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":""})), Some(NotifyLength::Whole)), None); // notifyReply({"status":"completed","text":""}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"completed","text":""})), None), None); // notifyBody({"status":"completed","text":""})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":"  \n "})), None), None); // notifyReply({"status":"completed","text":"  \n "})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":"  \n "})), Some(NotifyLength::Whole)), None); // notifyReply({"status":"completed","text":"  \n "}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"completed","text":"  \n "})), None), None); // notifyBody({"status":"completed","text":"  \n "})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":"a\n\nb  c\n"})), None), Some("b c".to_string())); // notifyReply({"status":"completed","text":"a\n\nb  c\n"})
        assert_eq!(notify_reply(&res(json!({"status":"completed","text":"a\n\nb  c\n"})), Some(NotifyLength::Whole)), Some("a\n\nb  c".to_string())); // notifyReply({"status":"completed","text":"a\n\nb  c\n"}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"completed","text":"a\n\nb  c\n"})), None), Some("b c".to_string())); // notifyBody({"status":"completed","text":"a\n\nb  c\n"})
        assert_eq!(notify_reply(&res(json!({"status":"failed","error":"boom","text":"partial\nlast"})), None), Some("last".to_string())); // notifyReply({"status":"failed","error":"boom","text":"partial\nlast"})
        assert_eq!(notify_reply(&res(json!({"status":"failed","error":"boom","text":"partial\nlast"})), Some(NotifyLength::Whole)), Some("partial\nlast".to_string())); // notifyReply({"status":"failed","error":"boom","text":"partial\nlast"}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"failed","error":"boom","text":"partial\nlast"})), None), Some("boom".to_string())); // notifyBody({"status":"failed","error":"boom","text":"partial\nlast"})
        assert_eq!(notify_reply(&res(json!({"status":"failed"})), None), None); // notifyReply({"status":"failed"})
        assert_eq!(notify_reply(&res(json!({"status":"failed"})), Some(NotifyLength::Whole)), None); // notifyReply({"status":"failed"}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"failed"})), None), None); // notifyBody({"status":"failed"})
        assert_eq!(notify_reply(&res(json!({"status":"interrupted","text":"x"})), None), Some("x".to_string())); // notifyReply({"status":"interrupted","text":"x"})
        assert_eq!(notify_reply(&res(json!({"status":"interrupted","text":"x"})), Some(NotifyLength::Whole)), Some("x".to_string())); // notifyReply({"status":"interrupted","text":"x"}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"interrupted","text":"x"})), None), Some("x".to_string())); // notifyBody({"status":"interrupted","text":"x"})
        assert_eq!(notify_reply(&res(json!({"status":"completed","error":"warn","text":" "})), None), None); // notifyReply({"status":"completed","error":"warn","text":" "})
        assert_eq!(notify_reply(&res(json!({"status":"completed","error":"warn","text":" "})), Some(NotifyLength::Whole)), None); // notifyReply({"status":"completed","error":"warn","text":" "}, "whole")
        assert_eq!(notify_body(&res(json!({"status":"completed","error":"warn","text":" "})), None), Some("warn".to_string())); // notifyBody({"status":"completed","error":"warn","text":" "})
    }

    #[test]
    fn listings_cut_at_their_caps_and_take_a_list_marker_off() {
        assert_eq!(listed_last_line(None), None); // listedLastLine(undefined)
        assert_eq!(listed_last_line(Some("")), None); // listedLastLine("")
        assert_eq!(listed_last_line(Some("- item one")), Some("item one".to_string())); // listedLastLine("- item one")
        assert_eq!(listed_last_line(Some("12. twelve")), Some("twelve".to_string())); // listedLastLine("12. twelve")
        assert_eq!(listed_last_line(Some("1234567890. too long a number")), Some("1234567890. too long a number".to_string())); // listedLastLine("1234567890. too long a number")
        assert_eq!(listed_last_line(Some("*  spaced")), Some("spaced".to_string())); // listedLastLine("*  spaced")
        assert_eq!(listed_last_line(Some("-no space")), Some("-no space".to_string())); // listedLastLine("-no space")
        assert_eq!(listed_last_line(Some("+ \u{1f600}")), Some("\u{1f600}".to_string())); // listedLastLine("+ 😀")
        assert_eq!(listed_last_line(Some("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")), Some("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\u{2026}".to_string())); // listedLastLine("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
        assert_eq!(listed_last_line(Some("one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two ")), Some("one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two\u{2026}".to_string())); // listedLastLine("one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two one two ")
        assert_eq!(listed_failure(None), "turn ended without a result"); // listedFailure(undefined)
        assert_eq!(listed_failure(Some("")), "turn ended without a result"); // listedFailure("")
        assert_eq!(listed_failure(Some("\n\n  first  line \nsecond")), "first line"); // listedFailure("\n\n  first  line \nsecond")
        assert_eq!(listed_failure(Some("yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy")), "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy\u{2026}"); // listedFailure("yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy")
        assert_eq!(subagent_asked(&format!("  {}", "w ".repeat(200))), "w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w w\u{2026}"); // subagentAsked("  " + "w ".repeat(200))
        assert_eq!(js(&turn_lines(&res(json!({"status":"failed","text":"- done"})), Some("reason here"))), "{\"lastLine\":\"done\",\"failure\":\"reason here\"}"); // JSON.stringify(turnLines({status:"failed",text:"- done"}, "reason here"))
        assert_eq!(js(&turn_lines(&res(json!({"status":"completed"})), Some("ignored"))), "{}"); // JSON.stringify(turnLines({status:"completed"}, "ignored"))
        assert_eq!(js(&turn_lines(&res(json!({"status":"failed","error":"e1\ne2"})), None)), "{\"failure\":\"e1\"}"); // JSON.stringify(turnLines({status:"failed",error:"e1\ne2"}))
    }

    #[test]
    fn the_notify_line_names_eight_units_of_the_thread() {
        assert_eq!(notify_line("\u{1f600}abcdefghij", &res(json!({"status":"completed","durationMs":61000,"costUsd":0.004,"text":"ok"})), None), "thread \u{1f600}abcdef finished (completed, 1m 1s, $0.00): ok"); // notifyLine("😀abcdefghij", {status:"completed",durationMs:61000,costUsd:0.004,text:"ok"})
        assert_eq!(notify_line("abc", &res(json!({"status":"failed","durationMs":5000,"waitedMs":9000})), None), "thread abc finished (failed, 1ms)"); // notifyLine("abc", {status:"failed",durationMs:5000,waitedMs:9000})
        assert_eq!(unread_line("\n  \n  hello there \nmore"), "the agent never read this message: hello there"); // unreadLine("\n  \n  hello there \nmore")
        assert_eq!(unread_line(""), "the agent never read this message: "); // unreadLine("")
        assert_eq!(wait_timed_out_line(&[] as &[&str], 1000.0), "0 threads still running after 1.0s"); // waitTimedOutLine([], 1000)
    }

    #[test]
    fn worked_waited_and_spend_parts_keep_their_order() {
        assert_eq!(turn_worked_ms(&TurnClocks { duration_ms: Some(5000.0), waited_ms: Some(9000.0), cost_usd: None }), Some(Worked { worked: 0.0, waited: 5000.0 })); // JSON.stringify(turnWorkedMs({durationMs:5000,waitedMs:9000}))
        assert_eq!(turn_worked_ms(&TurnClocks { duration_ms: Some(5000.0), waited_ms: Some(-3.0), cost_usd: None }), Some(Worked { worked: 5000.0, waited: 0.0 })); // JSON.stringify(turnWorkedMs({durationMs:5000,waitedMs:-3}))
        assert_eq!(turn_worked_ms(&TurnClocks { duration_ms: None, waited_ms: Some(3.0), cost_usd: None }), None); // JSON.stringify(turnWorkedMs({waitedMs:3}) ?? null)
        assert_eq!(js(&turn_settled_parts(&TurnClocks { duration_ms: Some(4000.0), waited_ms: Some(210000.0), cost_usd: Some(0.5) }, Some(1.25), Some("list price"))), "[\"Worked for 1ms\",\"waited on you 4.0s\",\"$0.50 list price this turn\",\"$1.25 in threads it opened\"]"); // JSON.stringify(turnSettledParts({durationMs:4000,waitedMs:210000,costUsd:0.5}, 1.25, "list price"))
        assert_eq!(js(&turn_settled_parts(&TurnClocks { duration_ms: Some(4000.0), waited_ms: Some(4000.0), cost_usd: None }, Some(0.0), None)), "[\"Worked for 1ms\",\"waited on you 4.0s\"]"); // JSON.stringify(turnSettledParts({durationMs:4000,waitedMs:4000}, 0))
        assert_eq!(js(&turn_settled_parts(&TurnClocks::default(), Some(2.0), None)), "[\"$2.00 in threads it opened\"]"); // JSON.stringify(turnSettledParts({}, 2))
        assert_eq!(js(&turn_facts_parts(&TurnFacts { model: Some(String::new()), tokens: None }, None)), "[]"); // JSON.stringify(turnFactsParts({model:""}, undefined))
        assert_eq!(js(&turn_facts_parts(&TurnFacts { model: Some("m".into()), tokens: Some((999500.0, 3.0)) }, Some("Label"))), "[\"Label\",\"1M in\",\"3 out\"]"); // JSON.stringify(turnFactsParts({model:"m",tokens:{input:999500,output:3}}, "Label"))
        assert_eq!(turn_end_line(&res(json!({"status":"interrupted","durationMs":1000,"model":"m","tokens":{"input":1,"output":2},"error":"stopped"}))), "interrupted  Worked for 1.0s  m  1 in  2 out: stopped"); // turnEndLine({status:"interrupted",durationMs:1000,model:"m",tokens:{input:1,output:2},error:"stopped"})
        assert_eq!(turn_end_line(&res(json!({"status":"completed","error":"ignored"}))), "completed"); // turnEndLine({status:"completed",error:"ignored"})
    }

    #[test]
    fn compaction_retry_plan_and_changes_lines() {
        assert_eq!(compacted_line(None, None), "Compacted the context"); // compactedLine(undefined, undefined)
        assert_eq!(compacted_line(Some(5000.0), None), "Compacted the context"); // compactedLine(5000, undefined)
        assert_eq!(compacted_line(None, Some(1200.0)), "Compacted the context to 1.2k tokens"); // compactedLine(undefined, 1200)
        assert_eq!(compacted_line(Some(150000.0), Some(12000.0)), "Compacted the context, 150k to 12k tokens"); // compactedLine(150000, 12000)
        assert_eq!(anthropic_retry_why(Some("overloaded"), Some(529.0)), "Anthropic was overloaded"); // anthropicRetryWhy("overloaded", 529)
        assert_eq!(anthropic_retry_why(Some("rate_limit"), None), "Anthropic's rate limit was reached"); // anthropicRetryWhy("rate_limit", undefined)
        assert_eq!(anthropic_retry_why(Some("server_error"), None), "Anthropic answered with a server error"); // anthropicRetryWhy("server_error", undefined)
        assert_eq!(anthropic_retry_why(Some("server_error"), Some(500.0)), "Anthropic answered with a server error (500)"); // anthropicRetryWhy("server_error", 500)
        assert_eq!(anthropic_retry_why(Some("unknown"), None), "Anthropic did not answer"); // anthropicRetryWhy("unknown", undefined)
        assert_eq!(anthropic_retry_why(None, Some(502.0)), "Anthropic answered 502"); // anthropicRetryWhy(undefined, 502)
        assert_eq!(retry_line(&Retry { attempt: 2.0, of: None, why: None }), "Trying again (2)"); // retryLine({attempt:2})
        assert_eq!(retry_line(&Retry { attempt: 2.0, of: Some(10.0), why: Some("anthropic was overloaded".into()) }), "Anthropic was overloaded, trying again (2 of 10)"); // retryLine({attempt:2,of:10,why:"anthropic was overloaded"})
        assert_eq!(retry_line(&Retry { attempt: 1.0, of: None, why: Some(String::new()) }), ", trying again (1)"); // retryLine({attempt:1,why:""})
        assert_eq!(plan_steps_line(&[]), "Plan: 0 of 0 steps done"); // planStepsLine([])
        assert_eq!(plan_steps_line(&[PlanStep { text: "a".into(), state: StepState::Done }, PlanStep { text: "b".into(), state: StepState::Working }, PlanStep { text: "c".into(), state: StepState::Pending }]), "Plan: 1 of 3 steps done\n- [x] a\n- [ ] b (working)\n- [ ] c"); // planStepsLine([{text:"a",state:"done"},{text:"b",state:"working"},{text:"c",state:"pending"}])
        assert_eq!(turn_changes_line(&TurnChanges { files: vec![], others: None, shared: false }), "Changed 0 files, +0 -0"); // turnChangesLine({files:[]})
        assert_eq!(turn_changes_line(&TurnChanges { files: vec![FileLines { additions: 3.0, deletions: 1.0 }], others: Some(0), shared: false }), "Changed 1 file, +3 -1"); // turnChangesLine({files:[{additions:3,deletions:1}],others:[]})
        assert_eq!(turn_changes_line(&TurnChanges { files: vec![], others: Some(2), shared: false }), "No files from this thread's edits; 2 files also changed in this folder"); // turnChangesLine({files:[],others:[1,2]})
        assert_eq!(turn_changes_line(&TurnChanges { files: vec![FileLines { additions: 1.0, deletions: 0.0 }, FileLines { additions: 2.0, deletions: 5.0 }], others: None, shared: true }), "2 files changed in this folder, +3 -5; other threads worked in it too"); // turnChangesLine({files:[{additions:1,deletions:0},{additions:2,deletions:5}],shared:true})
        assert_eq!(turn_changes_line(&TurnChanges { files: vec![FileLines { additions: 1.0, deletions: 0.0 }], others: Some(1), shared: true }), "Changed 1 file, +1 -0; 1 file also changed in this folder"); // turnChangesLine({files:[{additions:1,deletions:0}],others:[1],shared:true})
    }

    #[test]
    fn a_refused_turn_keeps_only_the_fields_it_had() {
        assert_eq!(js(&refused_turn(&res(json!({"status":"completed","text":"  no  ","error":"e","durationMs":5,"model":"m"})), Some(&RefusalRoad { road: Some("sign in".into()), cause: Some(TurnRefusal::SignIn) }))), "{\"status\":\"failed\",\"durationMs\":5,\"model\":\"m\",\"error\":\"no; sign in\",\"refusal\":\"sign-in\"}"); // JSON.stringify(refusedTurn({status:"completed",text:"  no  ",error:"e",durationMs:5,model:"m"}, {road:"sign in", cause:"sign-in"}))
        assert_eq!(js(&refused_turn(&res(json!({"status":"completed"})), None)), "{\"status\":\"failed\"}"); // JSON.stringify(refusedTurn({status:"completed"}))
        assert_eq!(js(&refused_turn(&res(json!({"status":"failed","error":"e","costUsd":0})), Some(&RefusalRoad { road: Some(String::new()), cause: None }))), "{\"status\":\"failed\",\"costUsd\":0,\"error\":\"e\"}"); // JSON.stringify(refusedTurn({status:"failed",error:"e",costUsd:0}, {road:""}))
    }

    #[test]
    fn titles_cut_and_kept_openings_never_split_a_pair() {
        assert_eq!(title_line(""), ""); // titleLine("")
        assert_eq!(title_line("  \n\n Hello   world \n next"), "Hello world"); // titleLine("  \n\n Hello   world \n next")
        assert_eq!(title_line("one\r\ntwo"), "one"); // titleLine("one\r\ntwo")
        assert_eq!(opening_title("Fix it. Then more."), "Fix it."); // openingTitle("Fix it. Then more.")
        assert_eq!(opening_title("Version 1.2 is out! Yes"), "Version 1.2 is out!"); // openingTitle("Version 1.2 is out! Yes")
        assert_eq!(opening_title("Why? Because"), "Why?"); // openingTitle("Why? Because")
        assert_eq!(opening_title("no stop at all here and it is quite a long line of words to cut down"), "no stop at all here and it is quite a long line\u{2026}"); // openingTitle("no stop at all here and it is quite a long line of words to cut down")
        assert_eq!(opening_title("e.g.this"), "e.g.this"); // openingTitle("e.g.this")
        assert_eq!(opening_title(""), ""); // openingTitle("")
        assert_eq!(opening_title("Done."), "Done."); // openingTitle("Done.")
        assert_eq!(opening_title("a.b. c"), "a.b."); // openingTitle("a.b. c")
        assert_eq!(kept_opening(&"a".repeat(600)) == "a".repeat(600), true); // keptOpening("a".repeat(600)) === "a".repeat(600)
        assert_eq!(wsp_words::js::utf16::len(&kept_opening(&"a".repeat(601))) as f64, 600.0); // keptOpening("a".repeat(601)).length
        assert_eq!(wsp_words::js::utf16::len(&kept_opening(&format!("{}\u{1f600}b", "a".repeat(599)))) as f64, 599.0); // keptOpening("a".repeat(599) + "😀" + "b").length
        assert_eq!(wsp_words::js::utf16::len(&kept_opening(&format!("{}\u{1f600}b", "a".repeat(598)))) as f64, 600.0); // keptOpening("a".repeat(598) + "😀" + "b").length
        assert_eq!(opening_cut(&"a".repeat(598)), false); // openingCut("a".repeat(598))
        assert_eq!(opening_cut(&"a".repeat(599)), true); // openingCut("a".repeat(599))
    }

    #[test]
    fn a_stored_title_is_the_seed_or_the_persons() {
        assert_eq!(source_word(stored_title_source("Fix the bug", Some("Fix the bug"))), "seed"); // storedTitleSource("Fix the bug", "Fix the bug")
        assert_eq!(source_word(stored_title_source("Fix the bug Attached files:", Some("Fix the bug"))), "seed"); // storedTitleSource("Fix the bug Attached files:", "Fix the bug")
        assert_eq!(source_word(stored_title_source("Fix", Some("Fix the bug"))), "seed"); // storedTitleSource("Fix", "Fix the bug")
        assert_eq!(source_word(stored_title_source("Fix the bug please", Some("Fix the bug"))), "person"); // storedTitleSource("Fix the bug please", "Fix the bug")
        assert_eq!(source_word(stored_title_source("", Some("x"))), "person"); // storedTitleSource("", "x")
        assert_eq!(source_word(stored_title_source("My own name", Some("Fix"))), "person"); // storedTitleSource("My own name", "Fix")
        assert_eq!(source_word(stored_title_source("Your earlier session for this thread was lost, so it continues here in a new one. rest", None)), "seed"); // storedTitleSource("Your earlier session for this thread was lost, so it continues here in a new one. rest", undefined)
        assert_eq!(source_word(stored_title_source("Anything", None)), "person"); // storedTitleSource("Anything", undefined)
        assert_eq!(source_word(stored_title_source("  Fix   the\nbug ", Some("Fix the bug"))), "seed"); // storedTitleSource("  Fix   the\nbug ", "Fix the bug")
        assert_eq!(source_word(stored_title_source("wwwwwwwwww more words", Some(&format!("wwwwwwwwww {}", "z".repeat(600))))), "person"); // storedTitleSource("wwwwwwwwww more words", "wwwwwwwwww " + "z".repeat(600))
    }

    #[test]
    fn the_title_question_and_its_answer() {
        assert_eq!(title_prompt("  open  ", None), "Name this coding agent thread in 3 to 6 words, no more than 34 characters, sentence case, no quotes and no full stop.\nDo not repeat the opening words, and do not say anything is done, fixed or working.\nAnswer with the title alone and nothing else; a longer answer is cut short.\n\nThe opening turn:\nopen"); // titlePrompt("  open  ", undefined)
        assert_eq!(title_prompt(&"x".repeat(601), Some("  r ")), "Name this coding agent thread in 3 to 6 words, no more than 34 characters, sentence case, no quotes and no full stop.\nDo not repeat the opening words, and do not say anything is done, fixed or working.\nAnswer with the title alone and nothing else; a longer answer is cut short.\n\nThe opening turn:\nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\u{2026}\n\nThe reply:\nr"); // titlePrompt("x".repeat(601), "  r ")
        assert_eq!(generated_title(""), None); // generatedTitle("")
        assert_eq!(generated_title(" \n "), None); // generatedTitle(" \n ")
        assert_eq!(generated_title("Fix the build"), Some("Fix the build".to_string())); // generatedTitle("Fix the build")
        assert_eq!(generated_title("\"Quoted title\""), Some("Quoted title".to_string())); // generatedTitle("\"Quoted title\"")
        assert_eq!(generated_title("\u{201c}Curly quotes\u{201d}"), Some("Curly quotes".to_string())); // generatedTitle("“Curly quotes”")
        assert_eq!(generated_title("'single'."), Some("'single'".to_string())); // generatedTitle("'single'.")
        assert_eq!(generated_title("Ends with stops..."), Some("Ends with stops".to_string())); // generatedTitle("Ends with stops...")
        assert_eq!(generated_title("two\nlines"), None); // generatedTitle("two\nlines")
        assert_eq!(generated_title("\"\""), None); // generatedTitle("\"\"")
        assert_eq!(generated_title("..."), None); // generatedTitle("...")
        assert_eq!(generated_title("A title that is far too long to be a title at all really"), Some("A title that is far too long to be a".to_string())); // generatedTitle("A title that is far too long to be a title at all really")
        assert_eq!(generated_title("Supercalifragilisticexpialidociousandmorebeyondforty"), None); // generatedTitle("Supercalifragilisticexpialidociousandmorebeyondforty")
        assert_eq!(generated_title("\"unbalanced"), Some("\"unbalanced".to_string())); // generatedTitle("\"unbalanced")
    }

    #[test]
    fn the_other_lines_read_as_the_ts_writes_them() {
        assert_eq!(fmt_clock(None, &utc), ""); // fmtClock(undefined)
        assert_eq!(fmt_clock(Some(0.0), &utc), "00:00:00"); // fmtClock(0)
        assert_eq!(fmt_clock(Some(1788744070500.0), &utc), "01:21:10"); // fmtClock(1788744070500)
        assert_eq!(no_messages_line("0123456789"), "thread 01234567 has no messages in the transcript this host holds"); // noMessagesLine("0123456789")
        assert_eq!(no_reply_line("ab"), "thread ab has not replied yet"); // noReplyLine("ab")
        assert_eq!(lost_session_prompt("T", "P"), "Your earlier session for this thread was lost, so it continues here in a new one. The thread so far, oldest first:\n\nT\n\nThe new message:\n\nP"); // lostSessionPrompt("T", "P")
    }
}
