// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448).
use wsp_cli::json_whole::{parse_frame, stringify};

// json_whole.rs:34-45: past the port's 1,000 levels a reply falls back to the 128-level lax reader and is answered as a
// tree with every container below level 128 emptied. Ruling 2 asks that one read to fail; node reads it whole.
#[test]
fn a_reply_1001_levels_deep_is_not_read_as_a_shallower_tree() {
    let text = format!(r#"{{"id":1,"values":{}1{}}}"#, "[".repeat(1_000), "]".repeat(1_000));
    let whole = std::thread::Builder::new().stack_size(1 << 20).spawn(move || parse_frame(&text).map(|v| stringify(&v) == text)).unwrap().join().unwrap();
    assert_ne!(whole, Some(false), "the reply reads as a different, shallower tree");
}
