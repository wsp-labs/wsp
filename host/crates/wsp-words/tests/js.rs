// SPDX-License-Identifier: AGPL-3.0-only
//! JavaScript's text and number rules (spec chapter 08). Every expected value was printed by node 22.23.4 from the
//! same expression at a3b870a6e (`(x).toFixed(d)`, `Math.round(x)`, `String(x)`, `toLocaleString`, `.trim()`,
//! `.slice`, `.sort()`); none is computed in Rust.

use std::cmp::Ordering;
use wsp_words::js::{
    collate, is_js_whitespace, js_collapse_whitespace, js_round, js_trim, js_trim_end, js_trim_start, locale_number, number_string,
    to_fixed, utf16, Grouping,
};

mod utf16_units {
    use super::*;

    #[test]
    fn length_counts_a_char_above_the_bmp_as_two_units() {
        assert_eq!(utf16::len(""), 0);
        assert_eq!(utf16::len("abc"), 3);
        assert_eq!(utf16::len("a\u{1F600}b"), 4);
        assert_eq!(utf16::len("\u{e9}"), 1);
        assert_eq!(utf16::len("\u{2026}"), 1);
    }

    #[test]
    fn slice_follows_javascript_index_rules() {
        assert_eq!(utf16::slice("hello", 1, Some(3)), "el");
        assert_eq!(utf16::slice("hello", -3, None), "llo");
        assert_eq!(utf16::slice("hello", 0, Some(-1)), "hell");
        assert_eq!(utf16::slice("hello", 3, Some(1)), "");
        assert_eq!(utf16::slice("hello", 0, Some(99)), "hello");
        assert_eq!(utf16::slice("hello", -99, Some(2)), "he");
        assert_eq!(utf16::slice("", 0, Some(5)), "");
    }

    // Rule 8.5: node gives "\ude00b" and "a\ud83d" here; a Rust String cannot hold the lone half, so it is dropped.
    #[test]
    fn a_cut_inside_a_surrogate_pair_drops_the_lone_half() {
        assert_eq!(utf16::slice("a\u{1F600}b", -2, None), "b");
        assert_eq!(utf16::slice("a\u{1F600}b", 0, Some(2)), "a");
        assert_eq!(utf16::slice("a\u{1F600}b", 0, Some(3)), "a\u{1F600}");
        assert_eq!(utf16::cut("a\u{1F600}b", 2), "a");
        assert_eq!(utf16::tail("a\u{1F600}b", 2), "b");
        assert_eq!(utf16::tail("a\u{1F600}b", 3), "\u{1F600}b");
    }

    #[test]
    fn cut_and_tail_take_the_first_and_last_units() {
        assert_eq!(utf16::cut("hello", 0), "");
        assert_eq!(utf16::cut("hello", 2), "he");
        assert_eq!(utf16::cut("hello", 9), "hello");
        assert_eq!(utf16::tail("hello", 2), "lo");
        assert_eq!(utf16::tail("hello", 9), "hello");
        // slice(-0) would be the whole string; tail(_, 0) is the empty tail.
        assert_eq!(utf16::tail("hello", 0), "");
    }

    #[test]
    fn index_of_and_last_index_of_count_units() {
        assert_eq!(utf16::index_of("a\u{1F600} b c", " ", 0), Some(3));
        assert_eq!(utf16::index_of("a b c", " ", 2), Some(3));
        assert_eq!(utf16::index_of("abc", "z", 0), None);
        // "abc".indexOf("", 5) is 3: an empty needle is found at min(from, length).
        assert_eq!(utf16::index_of("abc", "", 5), Some(3));
        assert_eq!(utf16::last_index_of("a\u{1F600} b c", " "), Some(5));
        assert_eq!(utf16::last_index_of("abc", "z"), None);
        assert_eq!(utf16::last_index_of("abc", ""), Some(3));
    }

    #[test]
    fn pad_start_and_pad_end_fill_to_the_width_in_units() {
        assert_eq!(utf16::pad_start("5", 2, "0"), "05");
        assert_eq!(utf16::pad_start("5", 3, "ab"), "ab5");
        assert_eq!(utf16::pad_start("123", 2, "0"), "123");
        assert_eq!(utf16::pad_start("x", 5, ""), "x");
        assert_eq!(utf16::pad_end("ab", 5, "."), "ab...");
        // node gives "\ud83da\u{1F600}"; the lone half of the cut fill is dropped (rule 8.5).
        assert_eq!(utf16::pad_start("a\u{1F600}", 4, "\u{1F600}"), "a\u{1F600}");
    }

    // node: ["Z","a","\u{1F600}","\u{e000}","\u{ffff}"].sort() is Z, a, the emoji, U+E000, U+FFFF; byte order puts the
    // emoji last.
    #[test]
    fn sort_order_is_utf16_units_not_bytes() {
        let mut v = vec!["\u{ffff}", "\u{1F600}", "a", "\u{e000}", "Z"];
        v.sort_by(|a, b| utf16::cmp(a, b));
        assert_eq!(v, vec!["Z", "a", "\u{1F600}", "\u{e000}", "\u{ffff}"]);
        assert_eq!(utf16::cmp("a", "a"), Ordering::Equal);
        assert_eq!(utf16::cmp("a", "ab"), Ordering::Less);
        assert_eq!(utf16::cmp("B", "a"), Ordering::Less);
    }
}

mod locale_order {
    use super::*;

    // node 22: the same words sorted with localeCompare.
    #[test]
    fn collate_orders_as_locale_compare_with_the_root_locale() {
        let mut v = vec![
            "b", "A", "a", "B", "_x", "1", "a-b", "ab", "a_b", "Z", "\u{e9}", "e", "f", "", "10", "9", "a b", "\u{e4}", "\u{df}", "ss",
            "Ab", "aB",
        ];
        v.sort_by(|a, b| collate(a, b));
        assert_eq!(
            v,
            vec![
                "", "_x", "1", "10", "9", "a", "A", "\u{e4}", "a b", "a_b", "a-b", "ab", "aB", "Ab", "b", "B", "e", "\u{e9}", "f", "ss",
                "\u{df}", "Z"
            ]
        );
        assert_eq!(collate("a", "A"), Ordering::Less);
        assert_eq!(collate("A", "a"), Ordering::Greater);
        assert_eq!(collate("a", "a"), Ordering::Equal);
        assert_eq!(collate("", ""), Ordering::Equal);
        assert_eq!(collate("10", "9"), Ordering::Less);
    }
}

mod whitespace {
    use super::*;

    // Rule 8.6: JavaScript trims U+FEFF and keeps U+0085; Rust's str::trim does the opposite.
    #[test]
    fn trim_uses_javascripts_whitespace_set() {
        assert_eq!(js_trim("\u{feff}a\u{85}"), "a\u{85}");
        assert_eq!(
            js_trim(" \t\n\u{b}\u{c}\r\u{a0}\u{1680}\u{2000}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}x\u{180e}"),
            "x\u{180e}"
        );
        assert_eq!(js_trim("   "), "");
        assert_eq!(js_trim(""), "");
        assert_eq!(js_trim_start("  a  "), "a  ");
        assert_eq!(js_trim_end("  a  "), "  a");
        assert_eq!(js_trim_end("\u{85}a\u{85}"), "\u{85}a\u{85}");
    }

    #[test]
    fn whitespace_set_matches_ecma_262_exactly() {
        for c in [
            '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2005}', '\u{200a}', '\u{2028}', '\u{2029}',
            '\u{202f}', '\u{205f}', '\u{3000}', '\u{feff}',
        ] {
            assert!(is_js_whitespace(c), "{:?} is whitespace to JavaScript", c);
        }
        for c in ['\u{85}', '\u{180e}', '\u{200b}', 'a', '\u{0}', '\u{1c}'] {
            assert!(!is_js_whitespace(c), "{:?} is not whitespace to JavaScript", c);
        }
    }

    #[test]
    fn collapse_turns_each_run_of_whitespace_into_one_space_and_trims_nothing() {
        assert_eq!(js_collapse_whitespace("  a \t\n b\u{feff}c  "), " a b c ");
        assert_eq!(js_collapse_whitespace("a\u{85}b"), "a\u{85}b");
        assert_eq!(js_collapse_whitespace(""), "");
    }
}

mod numbers {
    use super::*;

    #[test]
    fn math_round_takes_a_half_toward_positive_infinity() {
        let cases: [(f64, f64); 9] = [
            (2.5, 3.0),
            (-2.5, -2.0),
            (0.5, 1.0),
            (-0.5, -0.0),
            (1.5, 2.0),
            (-1.5, -1.0),
            (0.49999999999999994, 0.0),
            (-0.49999999999999994, -0.0),
            (4503599627370495.5, 4503599627370496.0),
        ];
        for (x, want) in cases {
            let got = js_round(x);
            assert_eq!(got, want, "Math.round({x})");
            assert_eq!(got.is_sign_negative(), want.is_sign_negative(), "sign of Math.round({x})");
        }
        assert!(js_round(f64::NAN).is_nan());
        assert_eq!(js_round(f64::INFINITY), f64::INFINITY);
    }

    #[test]
    fn to_fixed_rounds_on_the_exact_double_with_ties_away_from_zero() {
        let cases: [(f64, u32, &str); 30] = [
            (2.5, 0, "3"),
            (-2.5, 0, "-3"),
            (0.125, 2, "0.13"),
            (1.005, 2, "1.00"),
            (0.5, 0, "1"),
            (-0.5, 0, "-1"),
            (1.45, 1, "1.4"),
            (10.235, 2, "10.23"),
            (123.456, 1, "123.5"),
            (123.456, 2, "123.46"),
            (0.0, 0, "0"),
            (0.0, 3, "0.000"),
            (-0.0, 2, "0.00"),
            (-0.0001, 2, "-0.00"),
            (-1.234e-7, 3, "-0.000"),
            (0.05, 1, "0.1"),
            (0.05, 2, "0.05"),
            (9.995, 0, "10"),
            (9.995, 1, "10.0"),
            (9.995, 2, "9.99"),
            (9.995, 3, "9.995"),
            (2.5, 2, "2.50"),
            (1e21, 0, "1e+21"),
            (1e21, 2, "1e+21"),
            (-1e21, 0, "-1e+21"),
            (0.125, 0, "0"),
            (0.125, 1, "0.1"),
            (1.45, 2, "1.45"),
            (1.45, 3, "1.450"),
            (123.456, 0, "123"),
        ];
        for (x, d, want) in cases {
            assert_eq!(to_fixed(x, d), want, "({x}).toFixed({d})");
        }
        assert_eq!(to_fixed(f64::NAN, 2), "NaN");
    }

    #[test]
    fn number_string_writes_numbers_as_javascript_does() {
        let cases: [(f64, &str); 16] = [
            (1e21, "1e+21"),
            (1e-7, "1e-7"),
            (-0.0, "0"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.23e-18, "1.23e-18"),
            (1.5, "1.5"),
            (100.0, "100"),
            (1e20, "100000000000000000000"),
            (123456789012345680000.0, "123456789012345680000"),
            (5e-324, "5e-324"),
            (0.000001, "0.000001"),
            (1.5e-7, "1.5e-7"),
            (9007199254740992.0, "9007199254740992"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (-12.25, "-12.25"),
        ];
        for (x, want) in cases {
            assert_eq!(number_string(x), want, "String({x})");
        }
        assert_eq!(number_string(f64::NAN), "NaN");
    }

    // ICU rounds the shortest decimal half away from zero: 1.005 and 2.675 go up here where toFixed goes down.
    #[test]
    fn en_us_with_two_digits_rounds_the_shortest_decimal_half_up() {
        let cases: [(f64, &str); 14] = [
            (0.0, "0.00"),
            (1e-7, "0.00"),
            (0.004, "0.00"),
            (0.005, "0.01"),
            (0.015, "0.02"),
            (0.125, "0.13"),
            (1.005, "1.01"),
            (2.675, "2.68"),
            (9.995, "10.00"),
            (999.995, "1,000.00"),
            (1234.5, "1,234.50"),
            (1234567.891, "1,234,567.89"),
            (-1.005, "-1.01"),
            (-0.001, "-0.00"),
        ];
        for (x, want) in cases {
            assert_eq!(locale_number(x, Grouping::EnUs, 2, 2), want, "({x}).toLocaleString(\"en-US\", 2..2)");
        }
    }

    #[test]
    fn en_us_with_no_options_keeps_up_to_three_digits_and_drops_trailing_zeros() {
        let cases: [(f64, &str); 10] = [
            (0.0, "0"),
            (1.0, "1"),
            (999.0, "999"),
            (1000.0, "1,000"),
            (1234567.0, "1,234,567"),
            (1234.5678, "1,234.568"),
            (0.0005, "0.001"),
            (1.0005, "1.001"),
            (-1234.0, "-1,234"),
            (12345678.9, "12,345,678.9"),
        ];
        for (x, want) in cases {
            assert_eq!(locale_number(x, Grouping::EnUs, 0, 3), want, "({x}).toLocaleString(\"en-US\")");
        }
        assert_eq!(locale_number(1234.5, Grouping::EnUs, 0, 0), "1,235");
        assert_eq!(locale_number(2.5, Grouping::EnUs, 0, 0), "3");
    }

    #[test]
    fn en_in_groups_the_last_three_digits_then_pairs() {
        assert_eq!(locale_number(0.0, Grouping::EnIn, 0, 0), "0");
        assert_eq!(locale_number(1000.0, Grouping::EnIn, 0, 0), "1,000");
        assert_eq!(locale_number(144740.0, Grouping::EnIn, 0, 0), "1,44,740");
        assert_eq!(locale_number(100000.0, Grouping::EnIn, 0, 0), "1,00,000");
        assert_eq!(locale_number(144740.5, Grouping::EnIn, 2, 2), "1,44,740.50");
        assert_eq!(locale_number(12345678.9, Grouping::EnIn, 2, 2), "1,23,45,678.90");
        assert_eq!(locale_number(99999.999, Grouping::EnIn, 0, 0), "1,00,000");
        assert_eq!(locale_number(-5000.0, Grouping::EnIn, 0, 0), "-5,000");
    }

    #[derive(serde::Serialize)]
    struct Holder {
        #[serde(serialize_with = "wsp_words::js::serialize_f64")]
        n: f64,
        #[serde(serialize_with = "wsp_words::js::serialize_opt_f64")]
        o: Option<f64>,
    }

    // Rule 4.25: integral numbers have no ".0", so a serialized struct equals the json! literal the TS writes.
    #[test]
    fn serialized_numbers_are_written_as_javascript_writes_them() {
        let cases: [(f64, Option<f64>, &str); 6] = [
            (1.0, None, r#"{"n":1,"o":null}"#),
            (-0.0, Some(2.0), r#"{"n":0,"o":2}"#),
            (0.5, Some(-3.0), r#"{"n":0.5,"o":-3}"#),
            (9007199254740992.0, None, r#"{"n":9007199254740992,"o":null}"#),
            (1e300, None, r#"{"n":1e300,"o":null}"#),
            (12.25, Some(1e-7), r#"{"n":12.25,"o":1e-7}"#),
        ];
        for (n, o, want) in cases {
            let value = serde_json::to_value(Holder { n, o }).unwrap();
            assert_eq!(value, serde_json::from_str::<serde_json::Value>(want).unwrap(), "{n} {o:?}");
        }
        assert_eq!(serde_json::to_value(Holder { n: 1.0, o: Some(5.0) }).unwrap(), serde_json::json!({ "n": 1, "o": 5 }));
    }
}

mod replace_first {
    use wsp_words::js::replace_first;

    #[test]
    fn reads_the_dollar_forms_of_a_string_replacement_as_javascript_does() {
        // node 22: "a{bin}b".replace("{bin}", r) for each r below.
        assert_eq!(replace_first("a{bin}b", "{bin}", "'/x/$&y'"), "a'/x/{bin}y'b");
        assert_eq!(replace_first("a{bin}b", "{bin}", "'$$'"), "a'$'b");
        assert_eq!(replace_first("a{bin}b", "{bin}", "'$`|$'"), "a'a|bb");
        assert_eq!(replace_first("a{bin}b", "{bin}", "$1 $<n> $0 $"), "a$1 $<n> $0 $b");
        assert_eq!(replace_first("{bin}", "{bin}", "$"), "$");
        assert_eq!(replace_first("x", "{bin}", "y"), "x");
        assert_eq!(replace_first("{bin}{bin}", "{bin}", "$&$&"), "{bin}{bin}{bin}");
    }
}

mod upper_first {
    use wsp_words::js::upper_first;

    #[test]
    fn upper_cases_the_first_utf16_unit_as_charat_does() {
        // node 22: `${u.charAt(0).toUpperCase()}${u.slice(1)}`
        assert_eq!(upper_first("ß"), "SS");
        assert_eq!(upper_first("ŉ"), "ʼN");
        assert_eq!(upper_first("ﬁne"), "FIne");
        assert_eq!(upper_first("éa"), "Éa");
        assert_eq!(upper_first(""), "");
        // a first character outside the BMP is a lone surrogate to charAt, which has no case: the text comes back whole.
        assert_eq!(upper_first("\u{10428}x"), "\u{10428}x");
    }
}
