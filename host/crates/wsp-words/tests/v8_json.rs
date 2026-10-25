// SPDX-License-Identifier: AGPL-3.0-only
//! V8's JSON.parse as the iterative reader answers it: every value and every message below is what node 22 printed for the
//! text (a number past the double range printed as a half with its sign, which is how the reader holds Infinity for zod's
//! checks).

use wsp_words::js::number_string;
use wsp_words::v8_json::{parse_json, parse_json_lax, KEPT_DEPTH};

/// The text the case table writes a result as: `ok:` and JSON.stringify's text, or `err:` and the message.
fn render(text: &str) -> String {
    match parse_json(text) {
        Ok(value) => format!("ok:{}", wsp_words::exit::json_line(&value, None)),
        Err(why) => format!("err:{why}"),
    }
}

const CASES: &[(&str, &str)] = &[
    ("{}", "ok:{}"),
    ("[]", "ok:[]"),
    ("\"s\"", "ok:\"s\""),
    ("0", "ok:0"),
    ("-0", "ok:0"),
    ("1", "ok:1"),
    ("1.5", "ok:1.5"),
    ("1e2", "ok:100"),
    ("1E+2", "ok:100"),
    ("0.5e-3", "ok:0.0005"),
    ("123456789012345678901234567890", "ok:1.2345678901234568e+29"),
    ("-1.25e10", "ok:-12500000000"),
    ("true", "ok:true"),
    ("false", "ok:false"),
    ("null", "ok:null"),
    (" \t\r\n[ 1 , 2 ] \n", "ok:[1,2]"),
    ("{\"a\":[1,2,{\"b\":null}],\"c\":\"x\\u00e9\\n\\\"q\\\"\\/\\\\\"}", "ok:{\"a\":[1,2,{\"b\":null}],\"c\":\"x\u{e9}\\n\\\"q\\\"/\\\\\"}"),
    ("{\"a\":1,\"a\":2}", "ok:{\"a\":2}"),
    ("{\"b\":1,\"a\":{\"c\":[]},\"b\":3}", "ok:{\"b\":3,\"a\":{\"c\":[]}}"),
    ("[[],[[]],{}]", "ok:[[],[[]],{}]"),
    ("\"\\ud83d\\ude00\"", "ok:\"\u{1f600}\""),
    ("\"\\u0041\"", "ok:\"A\""),
    ("[1,\n2,\n3]", "ok:[1,2,3]"),
    ("\r{x", "err:Expected property name or '}' in JSON at position 2 (line 2 column 2)"),
    ("\r\n{x", "err:Expected property name or '}' in JSON at position 3 (line 2 column 2)"),
    ("\n\r{x", "err:Expected property name or '}' in JSON at position 3 (line 3 column 2)"),
    ("\n\n{x", "err:Expected property name or '}' in JSON at position 3 (line 3 column 2)"),
    ("\r\r{x", "err:Expected property name or '}' in JSON at position 3 (line 3 column 2)"),
    ("[1,\r\n2,\r\nx]", "err:Unexpected token 'x', \"[1,\r\n2,\r\nx]\" is not valid JSON"),
    ("{\"a\":\r1,}", "err:Expected double-quoted property name in JSON at position 8 (line 2 column 3)"),
    ("\n  [1 2]", "err:Expected ',' or ']' after array element in JSON at position 6 (line 2 column 6)"),
    ("{\"a\" 1}", "err:Expected ':' after property name in JSON at position 5 (line 1 column 6)"),
    ("{'a':1}", "err:Expected property name or '}' in JSON at position 1 (line 1 column 2)"),
    ("[1,]", "err:Unexpected token ']', \"[1,]\" is not valid JSON"),
    ("{\"a\":1,}", "err:Expected double-quoted property name in JSON at position 7 (line 1 column 8)"),
    ("[", "err:Unexpected end of JSON input"),
    ("{", "err:Expected property name or '}' in JSON at position 1 (line 1 column 2)"),
    ("{\"a\"", "err:Expected ':' after property name in JSON at position 4 (line 1 column 5)"),
    ("{\"a\":", "err:Unexpected end of JSON input"),
    ("[1", "err:Expected ',' or ']' after array element in JSON at position 2 (line 1 column 3)"),
    ("[1,", "err:Unexpected end of JSON input"),
    ("tru", "err:Unexpected end of JSON input"),
    ("nul", "err:Unexpected end of JSON input"),
    ("fals", "err:Unexpected end of JSON input"),
    ("truee", "err:Unexpected non-whitespace character after JSON at position 4 (line 1 column 5)"),
    ("nulll", "err:Unexpected non-whitespace character after JSON at position 4 (line 1 column 5)"),
    ("\"abc", "err:Unterminated string in JSON at position 4 (line 1 column 5)"),
    ("\"a\nb\"", "err:Bad control character in string literal in JSON at position 2 (line 1 column 3)"),
    ("\"a\tb\"", "err:Bad control character in string literal in JSON at position 2 (line 1 column 3)"),
    ("\"\\x\"", "err:Bad escaped character in JSON at position 2 (line 1 column 3)"),
    ("\"\\u12\"", "err:Bad Unicode escape in JSON at position 5 (line 1 column 6)"),
    ("\"\\uzzzz\"", "err:Bad Unicode escape in JSON at position 3 (line 1 column 4)"),
    ("\"\\", "err:Unexpected end of JSON input"),
    ("\"\\\u{100}\"", "err:Unexpected token '\u{100}', \"\"\\\u{100}\"\" is not valid JSON"),
    ("\"\\\u{ff}\"", "err:Bad escaped character in JSON at position 2 (line 1 column 3)"),
    ("\"\\\u{e9}\"", "err:Bad escaped character in JSON at position 2 (line 1 column 3)"),
    ("\"\\\u{3042}x\"", "err:Unexpected token '\u{3042}', \"\"\\\u{3042}x\"\" is not valid JSON"),
    ("-", "err:No number after minus sign in JSON at position 1 (line 1 column 2)"),
    ("-x", "err:No number after minus sign in JSON at position 1 (line 1 column 2)"),
    ("01", "err:Unexpected number in JSON at position 1 (line 1 column 2)"),
    ("1.", "err:Unterminated fractional number in JSON at position 2 (line 1 column 3)"),
    ("1.e2", "err:Unterminated fractional number in JSON at position 2 (line 1 column 3)"),
    ("1e", "err:Exponent part is missing a number in JSON at position 2 (line 1 column 3)"),
    ("1e+", "err:Exponent part is missing a number in JSON at position 3 (line 1 column 4)"),
    (".5", "err:Unexpected token '.', \".5\" is not valid JSON"),
    ("+1", "err:Unexpected token '+', \"+1\" is not valid JSON"),
    ("--1", "err:No number after minus sign in JSON at position 1 (line 1 column 2)"),
    ("0x10", "err:Unexpected non-whitespace character after JSON at position 1 (line 1 column 2)"),
    ("1 2", "err:Unexpected non-whitespace character after JSON at position 2 (line 1 column 3)"),
    ("[1] x", "err:Unexpected non-whitespace character after JSON at position 4 (line 1 column 5)"),
    ("{} {}", "err:Unexpected non-whitespace character after JSON at position 3 (line 1 column 4)"),
    ("{\"a\":1}x", "err:Unexpected non-whitespace character after JSON at position 7 (line 1 column 8)"),
    ("NaN", "err:\"NaN\" is not valid JSON"),
    ("Infinity", "err:\"Infinity\" is not valid JSON"),
    ("undefined", "err:\"undefined\" is not valid JSON"),
    ("[object Object]", "err:\"[object Object]\" is not valid JSON"),
    ("", "err:Unexpected end of JSON input"),
    (" ", "err:Unexpected end of JSON input"),
    ("x", "err:Unexpected token 'x', \"x\" is not valid JSON"),
    ("xyzxyzxyzxyzxyzxyzxyzxyzxyzxyz", "err:Unexpected token 'x', \"xyzxyzxyzx\"... is not valid JSON"),
    ("[1,2,3,4,5,6,7,8,9,10,11,12,x]", "err:Unexpected token 'x', ...\",10,11,12,x]\" is not valid JSON"),
    ("{\"aaaaaaaaaa\":1,\"bbbbbbbbbb\":x}", "err:Unexpected token 'x', ...\"bbbbbbbb\":x}\" is not valid JSON"),
    ("ABCDEFGHIJKLMNOPQRSTUVWXYZ", "err:Unexpected token 'A', \"ABCDEFGHIJ\"... is not valid JSON"),
    ("\u{3042}", "err:Unexpected token '\u{3042}', \"\u{3042}\" is not valid JSON"),
    ("\u{3042}\u{3044}\u{3046}\u{3048}\u{304a}\u{304b}\u{304d}\u{304f}\u{3051}\u{3053}\u{3055}\u{3057}x", "err:Unexpected token '\u{3042}', \"\u{3042}\u{3044}\u{3046}\u{3048}\u{304a}\u{304b}\u{304d}\u{304f}\u{3051}\u{3053}\u{3055}\u{3057}x\" is not valid JSON"),
    ("}", "err:Unexpected token '}', \"}\" is not valid JSON"),
    ("]", "err:Unexpected token ']', \"]\" is not valid JSON"),
    (":", "err:Unexpected token ':', \":\" is not valid JSON"),
    (",", "err:Unexpected token ',', \",\" is not valid JSON"),
    ("{,}", "err:Expected property name or '}' in JSON at position 1 (line 1 column 2)"),
    ("[,]", "err:Unexpected token ',', \"[,]\" is not valid JSON"),
    ("{\"a\":}", "err:Unexpected token '}', \"{\"a\":}\" is not valid JSON"),
    ("{:1}", "err:Expected property name or '}' in JSON at position 1 (line 1 column 2)"),
    ("{\"a\":1 \"b\":2}", "err:Expected ',' or '}' after property value in JSON at position 7 (line 1 column 8)"),
    ("[1 \"a\"]", "err:Expected ',' or ']' after array element in JSON at position 3 (line 1 column 4)"),
    ("1e400", "ok:0.5"),
    ("-1e400", "ok:-0.5"),
    ("[1e999,2]", "ok:[0.5,2]"),
    ("{\"a\":1e999}", "ok:{\"a\":0.5}"),
    ("1e-400", "ok:0"),
];

#[test]
fn every_text_reads_or_fails_as_node_reads_it() {
    let mut wrong = Vec::new();
    for (text, want) in CASES {
        let got = render(text);
        if got != *want {
            wrong.push(format!("{text:?}\n  node: {want}\n  here: {got}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), CASES.len(), wrong.join("\n"));
}

// node: JSON.parse("[".repeat(200000)) => "err:Unexpected end of JSON input"
#[test]
fn an_unclosed_nest_two_hundred_thousand_deep_fails_as_node_fails() {
    assert_eq!(parse_json(&"[".repeat(200_000)).unwrap_err(), "Unexpected end of JSON input");
}

#[test]
fn a_nest_deeper_than_the_reader_keeps_reads_whole_and_is_cut_to_empty_containers() {
    let depth = 5_000;
    let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let mut value = parse_json(&text).unwrap();
    let mut levels = 0;
    while let Some(inner) = value.as_array_mut().and_then(|items| items.pop()) {
        levels += 1;
        value = inner;
    }
    assert_eq!(levels, KEPT_DEPTH);
}

#[test]
fn a_number_past_the_range_reads_as_null_where_only_the_shape_is_read() {
    assert_eq!(parse_json_lax("{\"a\":1e999}").unwrap(), serde_json::json!({ "a": null }));
    assert_eq!(parse_json_lax("[1e999,-1e999]").unwrap(), serde_json::json!([null, null]));
    assert_eq!(number_string(1e21), "1e+21");
}

#[test]
fn a_text_that_does_not_parse_is_none() {
    assert_eq!(parse_json_lax("{x"), None);
}
