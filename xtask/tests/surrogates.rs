// SPDX-License-Identifier: AGPL-3.0-only
//! A tape line is read as `JSON.parse` reads it, so a lone surrogate in a key or a value is a frame the harness holds,
//! sends and compares code unit for code unit (docs/rust-host-spec/08-text-numbers-time.md, rule 8.26).

use serde_json::{json, Value};
use std::path::Path;
use xtask::diff::diff;
use xtask::json;
use xtask::tape::{self, Tape};

const HIGH: &str = r#"{"in":{"id":4,"op":"projects.resolve","ref":"a\ud83db\u0000\t\""}}"#;
const LOW: &str = r#"{"in":{"id":5,"op":"projects.resolve","ref":"a\ude00b","\ude00k":"\udbffx"}}"#;

fn read(line: &str) -> Tape {
    Tape::parse("lone", line, "lone.tape", 1).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_lone_high_or_low_surrogate_in_a_key_or_a_value_is_a_tape_line() {
    let tape = read(&format!("{HIGH}\n{LOW}\n"));
    assert_eq!(tape.ins.len(), 2);
    assert_eq!(tape.ins[0]["op"], "projects.resolve");
    assert_eq!(tape.ins[1]["op"], "projects.resolve");
}

#[test]
fn every_tape_of_the_repository_and_every_tape_block_of_the_spec_is_read() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let tapes = tape::load(&root).unwrap_or_else(|e| panic!("{e}"));
    assert!(tapes.iter().any(|t| t.id == "spec/08-text-numbers-time/1"), "the spec's lone surrogate tape is among them");
}

#[test]
fn a_frame_goes_on_the_wire_as_the_code_units_the_line_wrote() {
    let tape = read(&format!("{HIGH}\n{LOW}\n"));
    assert_eq!(json::write(&tape.ins[0]), r#"{"id":4,"op":"projects.resolve","ref":"a\ud83db\u0000\t\""}"#);
    assert_eq!(json::write(&tape.ins[1]), r#"{"id":5,"op":"projects.resolve","ref":"a\ude00b","\ude00k":"\udbffx"}"#);
    let read_back = json::read(&json::write(&tape.ins[1])).unwrap();
    assert_eq!(read_back, tape.ins[1]);
}

#[test]
fn a_pair_stays_one_character_and_an_escaped_backslash_stays_text() {
    let pair = json::read(r#""\ud83d\ude00 \\ud83d \\\ud83d""#).unwrap();
    assert_eq!(pair, Value::String("\u{1F600} \\ud83d \\\u{10F83D}".into()));
    assert_eq!(json::write(&pair), format!(r#""{} \\ud83d \\\ud83d""#, '\u{1F600}'));
}

#[test]
fn halves_compare_code_unit_for_code_unit_and_a_difference_prints_as_an_escape() {
    let (a, b) = (json::read(r#"{"e":"a\ud83db"}"#).unwrap(), json::read(r#"{"e":"a\ud83eb"}"#).unwrap());
    assert!(diff(&a, &a).is_empty());
    let found = diff(&a, &b);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].golden.as_str(), found[0].live.as_str()), (r#""a\ud83db""#, r#""a\ud83eb""#));
    // A replacement character is not the unit it replaced.
    assert_eq!(diff(&a, &json!({ "e": "a\u{FFFD}b" })).len(), 1);
    // A lone high half then a lone low half are two units, and a pair is one character.
    assert_ne!(json::read(r#""\ude00\ud83d""#).unwrap(), json::read(r#""\ud83d\ude00""#).unwrap());
}

#[test]
fn text_that_looks_like_the_stand_in_for_a_half_is_refused_not_mistaken_for_one() {
    for line in ["\"\u{10F83D}\"", r#""\udbfe\udc00""#] {
        let err = json::read(line).unwrap_err();
        assert!(err.contains("stands in for a lone surrogate"), "{err}");
    }
}
