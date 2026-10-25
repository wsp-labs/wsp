// SPDX-License-Identifier: AGPL-3.0-only
//! The ssh keys unit against the TS code's own answers (tests/fixtures/roads/ssh_keys.json). Spec: spec/ssh_keys.md.
mod roads_common;

use indexmap::IndexMap;
use roads_common::local_run;
use roads_common::*;
use serde_json::Value;
use wsp_engine::ssh_backend::*;

fn values(v: &Value) -> IndexMap<String, String> {
    v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()
}

fn opt(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

#[test]
fn host_key_found_matches_the_ts() {
    for case in fixture("ssh_keys.json")["hostKeyFound"].as_array().unwrap() {
        let found = case["found"].as_str().unwrap();
        let algorithms = case["algorithms"].as_str().unwrap();
        assert_eq!(host_key_found(found, algorithms), opt(&case["key"]), "{algorithms:?} over {found:?}");
    }
}

#[test]
fn targets_and_files_match_the_ts() {
    let d = fixture("ssh_keys.json");
    for case in d["target"].as_array().unwrap() {
        assert_eq!(known_host_target(&values(&case["values"])), opt(&case["target"]), "{}", case["values"]);
    }
    for case in d["files"].as_array().unwrap() {
        let want: Vec<String> = case["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(known_host_files(&values(&case["values"]), Some("/home/me")), want, "{}", case["values"]);
    }
}

#[test]
fn a_tilde_is_the_passwd_home_not_home_env() {
    // Read from the passwd entry, as ssh expands a tilde: a $HOME pointed elsewhere moves nothing.
    let passwd_home = {
        let out = std::process::Command::new("sh").arg("-c").arg("getent passwd \"$(id -un)\" | cut -d: -f6").output().unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let saved = std::env::var("HOME").ok();
    std::env::set_var("HOME", "/nowhere/wsp-not-a-home");
    let files = known_host_files(&IndexMap::from([("userknownhostsfile".to_string(), "~/.ssh/known_hosts".to_string())]), None);
    match saved {
        Some(h) => std::env::set_var("HOME", h),
        None => std::env::remove_var("HOME"),
    }
    assert_eq!(files, vec![format!("{passwd_home}/.ssh/known_hosts")]);
}

#[test]
fn client_words_match_the_ts() {
    for case in fixture("ssh_keys.json")["clientWords"].as_array().unwrap() {
        assert_eq!(client_words(case["text"].as_str().unwrap()), case["words"].as_str().unwrap(), "{:?}", case["text"]);
    }
}

#[test]
fn refusal_lines_match_the_ts() {
    let reach = SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None };
    let want = fixture("ssh_keys.json")["refusal"].clone();
    let said = [res(255, "", ""), res(1, "", "debug1: only\n"), res(255, "", &"x".repeat(400))];
    for (said, want) in said.iter().zip(want.as_array().unwrap()) {
        assert_eq!(ssh_refusal_line(said, &reach), want.as_str().unwrap());
    }
    assert_eq!(ssh_refusal_line(&said[2], &reach).len(), SSH_LINE_CAP);
    assert_eq!(ssh_word_refusal("nonsense"), fixture("ssh_keys.json")["wordRefusal"].as_str().unwrap());
    assert_eq!(ssh_identity("ssh-ed25519 SHA256:x", "dev"), "ssh-ed25519 SHA256:x as dev");
}

#[test]
fn trusted_by_authority_needs_every_line_to_be_an_authoritys() {
    let ca = "@cert-authority [10.0.0.5]:2222 ssh-ed25519 AAAA";
    assert!(trusted_by_authority(&format!("# Host found: line 1 CA\n{ca}\n")));
    assert!(!trusted_by_authority(""));
    assert!(!trusted_by_authority("# only a comment\n"));
    assert!(!trusted_by_authority(&format!("{ca}\nh ssh-ed25519 AAAA\n")));
    // The marker needs its space: a word that only starts with it is not the marker.
    assert!(!trusted_by_authority("@cert-authorityx h ssh-ed25519 AAAA"));
}

#[tokio::test]
async fn the_key_files_are_asked_one_after_another_in_the_clients_order() {
    let config = "user dev\nhostname 10.0.0.5\nport 22\nuserknownhostsfile /a /b\nglobalknownhostsfile /c\nhostkeyalgorithms ssh-ed25519\n";
    let (run, asked) = local_run(move |file, args| {
        if file == "ssh" {
            res(0, config, "")
        } else if args.last().map(String::as_str) == Some("/b") {
            res(0, "h ssh-ed25519 QUJD\n", "")
        } else {
            res(1, "", "")
        }
    });
    let reach = SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None };
    assert_eq!(known_host_key(&reach, &run).await.as_deref(), Some("ssh-ed25519 SHA256:tdQEXD9Gb6kf4sxqvnkjKhpXzfEE96JucW4KHieJ33g"));
    let asked = asked.lock().unwrap().clone();
    let files: Vec<String> = asked[1..].iter().map(|a| a.last().unwrap().clone()).collect();
    assert_eq!(files, ["/a", "/b", "/c"]);
    assert!(asked[1..].iter().all(|a| a[..3] == ["ssh-keygen", "-F", "10.0.0.5"]));
}
