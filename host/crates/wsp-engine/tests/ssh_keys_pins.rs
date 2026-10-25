// SPDX-License-Identifier: AGPL-3.0-only
//! The known-hosts readers and ssh's own words on edge inputs against the TS (tests/fixtures/scenes/units.json;
//! spec/ssh_keys.md rules K-1..K-4).
use indexmap::IndexMap;
use wsp_engine::machine::ExecResult;
use wsp_engine::ssh_backend::*;

const UNIT: &str = "ssh_keys";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// K-1: the port is read with JS Number and has no range check: "" and "0" are port 0, -1 and 99999 are kept, 0x16 is
/// 22 (bare host), Infinity is not an integer (bare host), white space is trimmed.
#[test]
fn the_known_host_target_reads_the_port_with_js_number() {
    for case in pins()["target"].as_array().unwrap() {
        let got = known_host_target(&map(&[("hostname", "h"), ("port", case["port"].as_str().unwrap())]));
        assert_eq!(got.as_deref(), case["out"].as_str(), "port {}", case["port"]);
    }
}

/// K-2: `~/` is joined to the home with `path.join`, which normalizes: a trailing slash on the home, `//`, `.` and
/// `..` all fold, and `~/` alone is the home.
#[test]
fn a_tilde_is_joined_and_normalized_as_path_join_does() {
    for case in pins()["files"].as_array().unwrap() {
        let got = known_host_files(&map(&[("userknownhostsfile", case["file"].as_str().unwrap())]), case["home"].as_str());
        let want: Vec<String> = case["out"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(got, want, "{} under {}", case["file"], case["home"]);
    }
}

/// K-3: the refusal keeps the last 300 units of the kept words with no second trim, so a leading newline can stay;
/// with nothing kept it names the exit code as given.
#[test]
fn the_refusal_line_keeps_its_last_300_units_untrimmed() {
    let reach = SshReach { user: "u".into(), host: "h".into(), port: 22, key_path: None };
    for case in pins()["refusal"].as_array().unwrap() {
        let said = ExecResult { exit_code: case["in"]["exitCode"].as_i64().unwrap(), stdout: String::new(), stderr: case["in"]["stderr"].as_str().unwrap().to_string() };
        assert_eq!(ssh_refusal_line(&said, &reach), case["out"].as_str().unwrap());
    }
}

/// K-4: the noise patterns are JavaScript's: `.` stops at `\r`, `$` is the end of the text, `\d` is ASCII. A bind line
/// ending in `\r` and a note with a `\r` inside are kept, and `debug` with an Arabic-Indic digit is kept.
#[test]
fn client_words_matches_with_javascript_regex_rules() {
    for case in pins()["clientWords"].as_array().unwrap() {
        assert_eq!(client_words(case["in"].as_str().unwrap()), case["out"].as_str().unwrap(), "{:?}", case["in"]);
    }
}
