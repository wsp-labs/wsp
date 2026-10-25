// SPDX-License-Identifier: AGPL-3.0-only
//! The upload's reader and folders on edge inputs against the TS (tests/fixtures/scenes/units.json; spec/exec_put.md
//! rules U-1, U-2).
mod roads_common;

use roads_common::*;
use wsp_engine::exec_detached::*;

const UNIT: &str = "exec_put";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// U-1: a line is trimmed, split on single spaces, and kept where its first word is the marker and a second word
/// exists, an empty second word included: `M  x y` is key "" with "x y"; a tab is no separator; the last of a key
/// wins and keeps its first place.
#[test]
fn markers_split_on_single_spaces_and_an_empty_word_is_a_key() {
    for case in pins()["markers"].as_array().unwrap() {
        let got: Vec<(String, String)> = markers_of(case["in"].as_str().unwrap(), "M").into_iter().collect();
        let want: Vec<(String, String)> =
            case["out"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect();
        assert_eq!(got, want, "{}", case["in"]);
    }
}

/// U-2: the folder each file's `mkdir -p` names is `posix.dirname`: `.` for a bare name, repeated slashes kept inside
/// and stripped at the end, `a/.` kept.
#[tokio::test]
async fn folders_are_posix_dirname() {
    for case in pins()["dirnames"].as_array().unwrap() {
        let machine = ScriptedMachine::new("m1", |_c, _o| Ok(res(0, "", "")));
        let path = case["path"].as_str().unwrap();
        put_files(machine.as_ref(), &[GuestWrite { path: path.to_string(), text: "x".into(), append: false }], PutFilesOptions::default())
            .await
            .unwrap();
        let want: Vec<String> = case["sent"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect();
        assert_eq!(machine.cmds(), want, "{path}");
    }
}
