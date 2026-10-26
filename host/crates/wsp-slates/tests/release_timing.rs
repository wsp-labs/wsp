// SPDX-License-Identifier: AGPL-3.0-only
//! The review's two longest slates, timed: `cargo test --release --test release_timing -- --ignored --nocapture`.
//! Node (v22.23.4) compiles the 43,000 holes in 1.5 s and the 63,000 numbers in 22 ms (review of #2448).
use std::time::Instant;

use wsp_slates::pending::slate::compile_slate_text;

#[test]
#[ignore = "a release timing; run with --release"]
fn the_review_s_129_kb_slate_of_43000_holes_compiles_within_node_s_time() {
    let text = format!("<slate><column><text>{}</text></column></slate>", "{1}".repeat(43_000));
    let at = Instant::now();
    let compiled = compile_slate_text(&text);
    let took = at.elapsed();
    println!("43000 holes, {} bytes: {} ms, {} errors", text.len(), took.as_millis(), compiled.errors.len());
    assert!(compiled.document.is_some());
    assert!(took.as_millis() < 1500, "node takes 1.5 s");
}

#[test]
#[ignore = "a release timing; run with --release"]
fn the_review_s_126_kb_list_of_63000_numbers_compiles_within_node_s_time() {
    let text = format!(r#"<slate><value name="a" start={{[{}1]}} /><column><text>x</text></column></slate>"#, "1,".repeat(63_000));
    let at = Instant::now();
    let compiled = compile_slate_text(&text);
    let took = at.elapsed();
    println!("63000 numbers, {} bytes: {} ms, {} errors", text.len(), took.as_millis(), compiled.errors.len());
    assert!(took.as_millis() < 1000);
}
