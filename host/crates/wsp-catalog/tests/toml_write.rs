// SPDX-License-Identifier: AGPL-3.0-only
//! `toml_write::stringify` against smol-toml's `stringify` (stringify.js) run by node: each case is a TOML text and the
//! text node writes for the tree it reads. `TOML_DIFF_CORPUS` names a larger corpus of the same shape, run as well.
use wsp_catalog::smol_toml::parse_toml;
use wsp_catalog::toml_write::stringify;

#[derive(serde::Deserialize)]
struct Case {
    text: String,
    out: String,
}

fn check(cases: &[Case]) {
    let mut wrong = Vec::new();
    for c in cases {
        let tree = parse_toml(&c.text).expect("the text parses");
        let got = stringify(&tree);
        if got != c.out {
            wrong.push(format!("{:?}\n  node: {:?}\n  port: {:?}", c.text, c.out, got));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} differ:\n{}",
        wrong.len(),
        cases.len(),
        wrong.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn stringify_writes_the_bytes_smol_toml_writes() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/toml_write.json")).unwrap();
    check(&cases);
}

#[test]
fn stringify_writes_the_bytes_smol_toml_writes_over_a_corpus() {
    let Ok(path) = std::env::var("TOML_DIFF_CORPUS") else {
        return;
    };
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    check(&cases);
}

// F3 and F4 of the re-review (rereview_2448.rs), through stringify itself: smol-toml writes every char of a string as it is
// and an offset as authored, `+00:00` and `-00:00` apart, in an array as in a table.
#[test]
fn stringify_keeps_a_plane_16_char_and_both_zero_offsets() {
    let text =
        "a = \"x\u{10F800}\u{10FFFD}\u{10FFFF}y\"\nb = [ 1979-05-27T07:32:00-00:00, 1979-05-27T07:32:00+00:00, 1979-05-27T07:32:00Z ]\n";
    let node = "a = \"x\u{10F800}\u{10FFFD}\u{10FFFF}y\"\nb = [ 1979-05-27T07:32:00.000-00:00, 1979-05-27T07:32:00.000+00:00, 1979-05-27T07:32:00.000Z ]\n";
    assert_eq!(stringify(&parse_toml(text).expect("the text parses")), node);
}
