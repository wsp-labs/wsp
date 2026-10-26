// SPDX-License-Identifier: AGPL-3.0-only
//! A command's printed JSON is read by `parse_json`, which keeps its nesting on a stack of its own so that depth past
//! serde_json's 128 is read as JSON.parse reads it (slate-runs.ts:290, :784); on every text serde_json reads, the two agree.
use serde_json::Value;
use wsp_slates::slate_runs::road::{parse_json, JSON_DEPTH_MAX};

const SEEDS: &[&str] = &[
    r#"{"a":1,"b":[true,false,null],"c":{"d":"e\n\u00e9\ud83d\ude00"}}"#,
    r#"[1,-0,0.5,1e5,1E+5,-1.5e-3,12345678901234567890]"#,
    r#"  {"k" : [ ] , "j":{ }}  "#,
    r#""a string""#,
    r#"[[[[[[[["deep"]]]]]]]]"#,
    r#"{"a":{"a":{"a":{"a":1}}},"a":2}"#,
];

fn same(text: &str) {
    let got = parse_json(text);
    let want: Option<Value> = serde_json::from_str(text).ok();
    assert_eq!(got, want, "on {text:?}");
}

#[test]
fn it_reads_what_serde_json_reads_and_refuses_what_it_refuses() {
    for t in [
        "",
        " ",
        "1",
        "-",
        "01",
        "+1",
        ".5",
        "1.",
        "1e",
        "NaN",
        "tru",
        "true",
        "nul",
        "[1,]",
        "[,1]",
        "{\"a\":1,}",
        "{a:1}",
        "[1 2]",
        "1 2",
        "\"\\x\"",
        "\"a\nb\"",
        "\"\\ud800\"",
        "[",
        "]",
        "{",
        "{\"a\"}",
        "{\"a\":}",
        "[\"a\",\"b\"",
        "\t[\r\n1\n]\t",
        "[1e400]",
        "-0",
    ] {
        same(t);
    }
    for seed in SEEDS {
        same(seed);
    }
}

#[test]
fn it_agrees_with_serde_json_on_mutated_texts() {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |n: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % n as u64) as usize
    };
    let bytes = *b"[]{},:\"\\ 0123456789.-+eEtrufalsn\n";
    for _ in 0..20_000 {
        let mut text: Vec<u8> = SEEDS[next(SEEDS.len())].as_bytes().to_vec();
        for _ in 0..=next(3) {
            let at = next(text.len());
            match next(3) {
                0 => text[at] = bytes[next(bytes.len())],
                1 => text.insert(at, bytes[next(bytes.len())]),
                _ => {
                    text.remove(at);
                }
            }
            if text.is_empty() {
                text.push(b'1');
            }
        }
        if let Ok(text) = String::from_utf8(text) {
            same(&text);
        }
    }
}

#[test]
fn nesting_is_read_to_the_depth_limit_and_refused_past_it_on_a_small_stack() {
    let nested = |n: usize| format!("{}1{}", "[".repeat(n), "]".repeat(n));
    let ok = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let kept = parse_json(&nested(JSON_DEPTH_MAX));
            let refused = parse_json(&nested(JSON_DEPTH_MAX + 1)).is_none() && parse_json(&nested(100_000)).is_none();
            let kept_ok = kept.is_some();
            std::mem::drop(kept);
            kept_ok && refused
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(ok);
}
