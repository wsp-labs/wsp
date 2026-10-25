// SPDX-License-Identifier: AGPL-3.0-only
// JsRegex matches as JavaScript does, and a pattern fancy-regex refuses fails loudly (spec/jsonc.md).
use wsp_catalog::js_regex::JsRegex;
use wsp_catalog::signin::SignIn;
use wsp_catalog::signin_rows::SIGN_IN_ROWS;

#[test]
fn every_pattern_on_a_sign_in_row_compiles() {
    let mut patterns = 0;
    for row in SIGN_IN_ROWS.values() {
        let mut each: Vec<&JsRegex> = vec![];
        match row {
            SignIn::Token(t) => each.push(&t.token),
            SignIn::Login(l) => {
                each.extend(l.questions.iter().flatten().map(|q| &q.asks));
                each.extend(l.code.iter());
            }
            SignIn::None(_) => {}
        }
        for re in each {
            re.is_match("");
            patterns += 1;
        }
    }
    assert!(patterns >= 20, "{patterns}");
}

#[test]
#[should_panic(expected = "is not one fancy-regex compiles")]
fn a_pattern_that_cannot_compile_panics_and_never_reads_as_no_match() {
    JsRegex::new("(a").is_match("a");
}

#[test]
fn a_lookbehind_of_unbounded_width_finds_the_leftmost_match_after_it() {
    let re = JsRegex::new(r"(?<=code:\s*)[a-z]+");
    assert_eq!(re.find("code:\n\n  abc def").as_deref(), Some("abc"));
    assert_eq!(re.find("abc code:x").as_deref(), Some("x"));
    assert_eq!(re.find("abc code: code: xyz").as_deref(), Some("code"));
    assert_eq!(re.find("abc xyz"), None);
    assert!(!re.is_match("code"));
}

#[test]
fn a_lookbehind_that_may_match_nothing_lets_the_whole_text_match() {
    assert!(!JsRegex::new(r"(?<=a\s*)b").is_whole_match("b"));
    assert!(JsRegex::new(r"(?<=\s*)b").is_whole_match("b"));
}

#[test]
fn multiline_starts_a_line_after_any_line_end() {
    let start = JsRegex::with_flags("^b", "m");
    for text in ["a\nb", "a\rb", "a\u{2028}b", "a\u{2029}b", "a\r\nb", "b"] {
        assert!(start.is_match(text), "{text:?}");
    }
    assert!(!start.is_match("ab"));
    assert!(!JsRegex::new("^b").is_match("a\nb"));
    let end = JsRegex::with_flags("a$", "m");
    for text in ["a\nb", "a\rb", "a\u{2028}b", "a"] {
        assert!(end.is_match(text), "{text:?}");
    }
    assert!(!end.is_match("ab"));
    assert!(!JsRegex::new("a$").is_match("a\nb"));
}

#[test]
fn ignore_case_never_folds_the_long_s_or_the_kelvin_sign_to_ascii() {
    let s = JsRegex::with_flags("s", "i");
    assert!(s.is_match("S") && s.is_match("s") && !s.is_match("\u{17f}"));
    let k = JsRegex::with_flags("[k-l]", "i");
    assert!(k.is_match("K") && !k.is_match("\u{212a}"));
    let range = JsRegex::with_flags("[a-z]", "i");
    assert!(range.is_match("Q") && !range.is_match("\u{17f}"));
    assert!(JsRegex::with_flags("[^a-z]", "i").is_match("\u{17f}"));
    assert!(JsRegex::with_flags("(?<s>x)", "i").is_match("Xx"));
}
