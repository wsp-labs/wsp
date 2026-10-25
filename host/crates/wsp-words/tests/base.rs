// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format-cases.test.ts:19-26 (the plural rows), format.test.ts:712-743 (plural,
//! fmtThreads), :725-741 (listedName, nameList), :781-793 (cutLine) and :879-886 (lastLine). Skipped: activity.test.ts:137
//! and thread-defaults.test.ts:157-170 (`marked` is a local name there, not the function).

use wsp_words::words::base::*;

const ELL: &str = "\u{2026}";

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

mod the_shared_format_cases_in_daemon_fixtures_contract_formats_json {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Cases {
        plural: Vec<(f64, String, String)>,
    }

    #[test]
    fn s_s_reads_j() {
        let cases: Cases = serde_json::from_str(include_str!("../../../../daemon/fixtures/contract/formats.json")).unwrap();
        assert!(!cases.plural.is_empty());
        for (n, noun, want) in &cases.plural {
            assert_eq!(&plural(*n, noun), want, "{n} {noun} reads {want:?}");
        }
    }
}

mod plural_fmt_threads_and_forget_notice {
    use super::*;

    #[test]
    fn is_the_one_rule_for_a_count_and_its_noun_and_fmt_threads_reads_it() {
        assert_eq!([0.0, 1.0, 2.0].iter().map(|n| plural(*n, "row")).collect::<Vec<_>>(), vec!["0 rows", "1 row", "2 rows"]);
        assert_eq!(plural(1.0, "tool call"), "1 tool call");
        assert_eq!([1.0, 2.0].iter().map(|n| fmt_threads(*n)).collect::<Vec<_>>(), vec![plural(1.0, "thread"), plural(2.0, "thread")]);
    }

    #[test]
    fn counts_threads_with_the_noun_and_names_what_a_forget_takes_off_this_computer() {
        assert_eq!([0.0, 1.0, 2.0].iter().map(|n| fmt_threads(*n)).collect::<Vec<_>>(), vec!["0 threads", "1 thread", "2 threads"]);
        // The TS also asserts forgetNotice(1) here; forgetNotice is not in the base stubs.
    }
}

mod listed_name_and_name_list {
    use super::*;

    #[test]
    fn leaves_a_name_that_carries_no_comma_alone_and_joins_a_list_with_the_separator() {
        assert_eq!(listed_name("ripgrep"), "ripgrep");
        assert_eq!(name_list(&["ripgrep", "just", "GitHub CLI"]), "ripgrep, just, GitHub CLI");
        assert_eq!(name_list(&[] as &[&str]), "");
    }

    #[test]
    fn quotes_a_name_that_carries_the_separator_so_a_free_text_label_reads_as_one_entry() {
        assert_eq!(listed_name("swift-format, swiftlint"), "\"swift-format, swiftlint\"");
        assert_eq!(name_list(&["swift-format, swiftlint", "just"]), "\"swift-format, swiftlint\", just");
        // A plain join leaves the label's own comma reading as a third entry; that is what the quotes take away.
        assert_eq!(["swift-format, swiftlint", "just"].join(", ").split(", ").count(), 3);
    }

    #[test]
    fn escapes_a_quote_the_name_itself_carries_so_the_quoting_cannot_be_read_as_the_end() {
        assert_eq!(listed_name("the \"fast\", grep"), "\"the \\\"fast\\\", grep\"");
    }
}

mod cut_line {
    use super::*;

    #[test]
    fn leaves_a_line_inside_the_room_whole_and_cuts_a_longer_one_at_a_word_boundary_the() {
        assert_eq!(cut_line("no answer 2 h, threads go on", 30), "no answer 2 h, threads go on");
        assert_eq!(cut_line("putting the helper back on this machine", 30), format!("putting the helper back on{ELL}"));
        assert!(units(&cut_line("putting the helper back on this machine", 30)) <= 30);
        // One word longer than the room is cut inside itself rather than dropped.
        assert_eq!(cut_line(&"a".repeat(40), 10), format!("{}{ELL}", "a".repeat(9)));
    }

    #[test]
    fn takes_the_separator_the_cut_broke_on_off_the_edge() {
        assert_eq!(cut_line("$0.00 today, edge slow, naps in 14m", 24), format!("$0.00 today, edge slow{ELL}"));
        assert_eq!(cut_line("one, two, three, four, five, six", 20), format!("one, two, three{ELL}"));
        // The TS also reads openingTitle("Ship the sidebar row and the composer line, then the word cut") here, a
        // thread title cut on a comma; openingTitle is not in the base stubs.
    }
}

mod last_line {
    use super::*;

    #[test]
    fn is_the_text_s_last_non_empty_line_with_its_whitespace_collapsed_which_is_what_a() {
        assert_eq!(last_line("Ran the gate.\n\nAll 12 tests green.\n"), Some("All 12 tests green.".to_string()));
        assert_eq!(last_line("  Server  is\tlive at :3000.  \r\n\n"), Some("Server is live at :3000.".to_string()));
        assert_eq!(last_line("one line"), Some("one line".to_string()));
        assert_eq!(last_line("\n \n"), None);
        assert_eq!(last_line(""), None);
    }
}

// Cases spec/base.md pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::base::*;

    fn small() -> BoatSizeWords {
        BoatSizeWords { r#type: "small".into(), cpu: 2.0, mem_mb: 4096.0, rate_usd_per_hour: 0.09 }
    }

    fn medium() -> BoatSizeWords {
        BoatSizeWords { r#type: "medium".into(), cpu: 4.0, mem_mb: 8192.0, rate_usd_per_hour: 0.018 }
    }

    fn limits(
        trial: bool,
        plan_usd: Option<f64>,
        machines_at_once: Option<f64>,
        starts: Option<(f64, f64, f64)>,
        credit_balance_hours: Option<f64>,
    ) -> BoatLimitsRead {
        BoatLimitsRead {
            trial,
            plan_usd,
            machines_at_once,
            starts: starts.map(|(m, h, d)| BoatStarts {
                minute: StartsLimit { limit: m },
                hour: StartsLimit { limit: h },
                day: StartsLimit { limit: d },
            }),
            credit_balance_hours,
        }
    }

    #[test]
    fn plural_writes_the_count_as_javascript_does_and_takes_an_s_unless_exactly_one() {
        assert_eq!(plural(1.5, "file"), "1.5 files"); // plural(1.5, "file")
        assert_eq!(plural(-1.0, "file"), "-1 files"); // plural(-1, "file")
        assert_eq!(plural(1e21, "file"), "1e+21 files"); // plural(1e21, "file")
        assert_eq!(lower_first(""), ""); // lowerFirst("")
        assert_eq!(lower_first("\u{c9}clair Here"), "\u{e9}clair Here"); // lowerFirst("Éclair Here")
        assert_eq!(lower_first("\u{1F600}X"), "\u{1f600}X"); // lowerFirst("😀X")
        assert_eq!(listed_name("a \"b\", c"), "\"a \\\"b\\\", c\""); // listedName("a \"b\", c")
        assert_eq!(listed_name("tab\tname, x"), "\"tab\\tname, x\""); // listedName("tab\tname, x")
        assert_eq!(name_list(&[] as &[&str]), ""); // nameList([])
    }

    #[test]
    fn cut_line_counts_utf16_units_cuts_at_a_word_and_never_splits_a_pair() {
        assert_eq!(cut_line("", 0), ""); // cutLine("", 0)
        assert_eq!(cut_line("abc", 3), "abc"); // cutLine("abc", 3)
        assert_eq!(cut_line("abcd", 3), "ab\u{2026}"); // cutLine("abcd", 3)
        assert_eq!(cut_line("supercalifragilistic", 8), "superca\u{2026}"); // cutLine("supercalifragilistic", 8)
        assert_eq!(cut_line("ab\u{1F600}cdef", 4), "ab\u{2026}"); // cutLine("ab😀cdef", 4)
        assert_eq!(cut_line("one, two; three: four", 12), "one, two\u{2026}"); // cutLine("one, two; three: four", 12)
        assert_eq!(cut_line("one two three", 8), "one two\u{2026}"); // cutLine("one two three", 8)
        assert_eq!(cut_line("one two three", 9), "one two\u{2026}"); // cutLine("one two three", 9)
        assert_eq!(cut_line("a\u{1F600}\u{1F600}\u{1F600}", 4), "a\u{1f600}\u{2026}"); // cutLine("a😀😀😀", 4)
        assert_eq!(cut_line("abc,,, def", 5), "abc\u{2026}"); // cutLine("abc,,, def", 5)
        assert_eq!(words_within("one two", 3), Some("one".to_string())); // wordsWithin("one two", 3)
        assert_eq!(words_within("one two", 2), None); // wordsWithin("one two", 2)
        assert_eq!(words_within(" one", 5), None); // wordsWithin(" one", 5)
        assert_eq!(words_within("one, two three", 9), Some("one, two".to_string())); // wordsWithin("one, two three", 9)
        assert_eq!(words_within("", 5), None); // wordsWithin("", 5)
    }

    #[test]
    fn last_line_skips_blank_lines_by_javascripts_whitespace_and_collapses_runs() {
        assert_eq!(last_line(""), None); // lastLine("")
        assert_eq!(last_line("\n  \r\n\t"), None); // lastLine("\n  \r\n\t")
        assert_eq!(last_line("a\r\nb  c\u{a0} d\n\n"), Some("b c d".to_string())); // lastLine("a\r\nb  c\u00a0 d\n\n")
        assert_eq!(last_line("x\n\u{feff}"), Some("x".to_string())); // lastLine("x\n\ufeff")
        assert_eq!(last_line("x\n\u{85}"), Some("\u{85}".to_string())); // lastLine("x\n\u0085")
    }

    #[test]
    fn boat_added_line_says_only_what_the_account_answered() {
        assert_eq!(boat_added_line(None, &small()), "Boat added; small 2 vCPU 4 GB at $0.09/h until you pick another size."); // boatAddedLine(undefined, { type: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.09 })
        assert_eq!(boat_added_line(Some(&limits(true, None, None, None, None)), &small()), "Boat added: trial; small and default sizes."); // boatAddedLine({ trial: true }, { type: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.09 })
        assert_eq!(
            boat_added_line(Some(&limits(true, None, Some(1.0), Some((12.0, 60.0, 200.0)), Some(2.25))), &small()),
            "Boat added: trial (2.3 h, 1 machine at once, 12 starts a minute, 60 an hour, 200 a day); small and default sizes."
        ); // boatAddedLine({ trial: true, creditBalanceHours: 2.25, machinesAtOnce: 1, starts: { minute: { limit: 12 }, hour: { limit: 60 }, day: { limit: 200 } } }, { type: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.09 })
        assert_eq!(
            boat_added_line(Some(&limits(true, None, None, None, Some(3.0))), &small()),
            "Boat added: trial (3 h); small and default sizes."
        ); // boatAddedLine({ trial: true, creditBalanceHours: 3 }, { type: "small", cpu: 2, memMb: 4096, rateUsdPerHour: 0.09 })
        assert_eq!(
            boat_added_line(Some(&limits(false, None, None, None, None)), &medium()),
            "Boat added: a paid plan; medium 4 vCPU 8 GB at $0.018/h until you pick another size."
        ); // boatAddedLine({ trial: false }, { type: "medium", cpu: 4, memMb: 8192, rateUsdPerHour: 0.018 })
        assert_eq!(boat_added_line(Some(&limits(false, Some(20.0), Some(3.0), Some((12.0, 60.0, 200.0)), None)), &medium()), "Boat added: $20 plan, 3 machines at once, 12 starts a minute, 60 an hour, 200 a day; medium 4 vCPU 8 GB at $0.018/h until you pick another size."); // boatAddedLine({ trial: false, planUsd: 20, machinesAtOnce: 3, starts: { minute: { limit: 12 }, hour: { limit: 60 }, day: { limit: 200 } } }, { type: "medium", cpu: 4, memMb: 8192, rateUsdPerHour: 0.018 })
        assert_eq!(boat_forks_no_first_line(&["--size"]), "Boat forks no first machine, so --size does nothing here."); // boatForksNoFirstLine(["--size"])
        assert_eq!(boat_forks_no_first_line(&["--size", "--name"]), "Boat forks no first machine, so --size and --name do nothing here."); // boatForksNoFirstLine(["--size", "--name"])
        assert_eq!(boat_forks_no_first_line(&[] as &[&str]), "Boat forks no first machine, so  do nothing here.");
        // boatForksNoFirstLine([])
    }

    #[test]
    fn marked_reads_a_mark_the_error_carries_and_no_other() {
        let mut e = wsp_words::exit::HostError::new("no node");
        e.marks.push("wspMachineLacks".into());
        assert!(marked(&e, "wspMachineLacks"));
        assert!(!marked(&e, "wspMachineUnanswered"));
        assert!(!marked(&wsp_words::exit::HostError::new("plain"), "wspMachineLacks"));
    }
}
