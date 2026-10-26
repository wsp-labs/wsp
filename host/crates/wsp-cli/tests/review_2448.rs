// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448).
use wsp_cli::json_whole::{parse_document, parse_frame, parse_value, stringify};

// json_whole.rs:34-45: past the port's cap a reply falls back to the 128-level lax reader and is answered as a tree with
// every container below level 128 emptied. Ruling 2 asks that one read to fail; node reads it whole. The cap is 500 levels
// (ruling 1 of the round 2 brief), so the reply is 501: a frame object over 500 levels of lists.
#[test]
fn a_reply_501_levels_deep_is_not_read_as_a_shallower_tree() {
    let text = format!(r#"{{"id":1,"values":{}1{}}}"#, "[".repeat(500), "]".repeat(500));
    let whole = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || parse_frame(&text).map(|v| stringify(&v) == text))
        .unwrap()
        .join()
        .unwrap();
    assert_ne!(whole, Some(false), "the reply reads as a different, shallower tree");
    assert_eq!(whole, None, "a reply past the cap is not read");
}

// parse_document and parse_value kept the same fallback: a container past 128 levels read as an empty one when the port refused.
#[test]
fn a_document_or_a_word_past_the_cap_is_not_read_as_a_shallower_tree() {
    let text = format!("{}1{}", "[".repeat(501), "]".repeat(501));
    let (document, word) =
        std::thread::Builder::new().stack_size(1 << 20).spawn(move || (parse_document(&text), parse_value(&text))).unwrap().join().unwrap();
    assert_eq!(document, Err("Maximum call stack size exceeded".to_owned()));
    assert_eq!(word, None);
}

#[test]
fn a_document_or_a_word_at_the_cap_is_read_whole() {
    let text = format!("{}1{}", "[".repeat(500), "]".repeat(500));
    let (document, word) = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn({
            let text = text.clone();
            move || (parse_document(&text), parse_value(&text))
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(stringify(&document.expect("read")), text);
    assert_eq!(stringify(&word.expect("read")), text);
}
