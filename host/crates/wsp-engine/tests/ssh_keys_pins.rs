// SPDX-License-Identifier: AGPL-3.0-only
//! The known-hosts readers and ssh's own words on edge inputs against the TS (tests/fixtures/scenes/units.json;
//! spec/ssh_keys.md rules K-1..K-4).
mod roads_common;

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
        let said = ExecResult {
            exit_code: case["in"]["exitCode"].as_i64().unwrap(),
            stdout: String::new(),
            stderr: case["in"]["stderr"].as_str().unwrap().to_string(),
        };
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

/// K-5: the scan's port is `String(Number(port) || 22)`: 2222.5, -5 and 99999 go as written, 1e3 is 1000, 0x50 is 80,
/// and 0, "" and abc are 22; a scan that fails offers no key.
#[tokio::test]
async fn the_scan_port_is_js_number_or_22_with_no_check() {
    let reach = SshReach { user: "u".into(), host: "h".into(), port: 22, key_path: None };
    for case in pins()["keyscan"].as_array().unwrap() {
        let port = case["port"].as_str().unwrap().to_string();
        let (run, asked) = roads_common::local_run(move |file, _args| {
            if file == "ssh" {
                roads_common::res(0, &format!("hostname h\nport {port}\n"), "")
            } else {
                roads_common::res(1, "", "")
            }
        });
        let got = offered_host_key(&reach, &run).await;
        assert_eq!((got.key, got.stopped_by), (None, None));
        let scanned = asked.lock().unwrap().last().cloned().unwrap();
        let want: Vec<String> = case["scanned"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()).collect();
        assert_eq!(scanned, want, "port {}", case["port"]);
    }
}

/// K-6: a typed port is taken with `??`, so a typed 0 stays 0 and is refused; the config's port is `Number(port) || 22`
/// and then held to 1..=65535, so 70000, 2222.5 and -1 are refused with the config's user, while 0, abc and 1e3 read
/// 22, 22 and 1000.
#[tokio::test]
async fn the_word_reach_takes_a_typed_port_with_nullish_and_the_configs_with_or() {
    for case in pins()["wordReach"].as_array().unwrap() {
        let config_port = case["configPort"].as_str().unwrap().to_string();
        let (run, _asked) =
            roads_common::local_run(move |_f, _a| roads_common::res(0, &format!("hostname h.example\nuser dev\nport {config_port}\n"), ""));
        let typed = case["typed"].as_u64().map(|p| p as u16);
        let got = match ssh_word_reach("alias", typed, None, &run).await {
            Ok(r) => serde_json::json!({ "ok": r }),
            Err(e) => serde_json::json!({ "err": e.to_string() }),
        };
        assert_eq!(got, case["out"], "typed {:?} config {}", typed, case["configPort"]);
    }
}
