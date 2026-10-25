// SPDX-License-Identifier: AGPL-3.0-only
//! The ssh dial unit against the TS code's own answers (tests/fixtures/roads/ssh_dial.json and ssh_carried.json,
//! written by oracle/roads.oracle.ts). Spec: spec/ssh_dial.md.
mod roads_common;

use roads_common::local_run;
use roads_common::*;
use serde_json::{json, Value};
use wsp_engine::errors::EngineError;
use wsp_engine::ssh_backend::*;

fn reaches() -> Vec<SshReach> {
    vec![
        SshReach { user: "dev".into(), host: "10.0.0.5".into(), port: 2222, key_path: Some("/tmp/k/id_ed25519".into()) },
        SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None },
        SshReach { user: "a b@c".into(), host: "Box.Example.COM".into(), port: 22, key_path: Some("/Users/John Smith/.ssh/id&x=1".into()) },
        SshReach { user: "root".into(), host: "[::1]".into(), port: 22, key_path: None },
        SshReach { user: "ü".into(), host: "héllo".into(), port: 65535, key_path: None },
    ]
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

#[test]
fn the_machine_id_is_the_url_the_ts_writes() {
    let want = fixture("ssh_dial.json")["machineId"].clone();
    for (reach, want) in reaches().iter().zip(want.as_array().unwrap()) {
        assert_eq!(ssh_machine_id(reach).unwrap(), want["ok"].as_str().unwrap(), "{reach:?}");
    }
}

#[test]
fn parse_answers_what_the_ts_answered_for_every_address() {
    let as_json = |r: Result<SshReach, EngineError>| match r {
        Ok(reach) => json!({ "ok": serde_json::to_value(reach).unwrap() }),
        Err(e) => json!({ "err": e.to_string() }),
    };
    for case in fixture("ssh_dial.json")["parse"].as_array().unwrap() {
        let address = case["address"].as_str().unwrap();
        assert_eq!(as_json(parse_ssh_address(address, None, None)), case["plain"], "{address}");
        assert_eq!(as_json(parse_ssh_address(address, Some(2200), Some("/tmp/k"))), case["withPort"], "{address} with a port");
    }
    // The written form keeps the TS key order, keyPath last and absent without one.
    let reach = parse_ssh_address("dev@box", Some(2200), Some("/tmp/k")).unwrap();
    assert_eq!(serde_json::to_string(&reach).unwrap(), r#"{"user":"dev","host":"box","port":2200,"keyPath":"/tmp/k"}"#);
}

#[test]
fn names_and_login_words_match_the_ts() {
    let d = fixture("ssh_dial.json");
    let words: Vec<String> = reaches().iter().map(ssh_login_word).collect();
    assert_eq!(words, strings(&d["loginWord"]));
    let hosts = ["box.example.com", "box", "10.0.0.5", "10.0.0", "1.2.3.4.example", ".hidden", "box.", "[::1]"];
    let names: Vec<String> =
        hosts.iter().map(|h| ssh_machine_name(&SshReach { user: "u".into(), host: h.to_string(), port: 22, key_path: None })).collect();
    assert_eq!(names, strings(&d["machineName"]));
}

#[test]
fn ssh_args_match_the_ts_byte_for_byte() {
    let d = fixture("ssh_dial.json");
    let paths: Vec<String> = reaches().iter().map(|r| ssh_control_path(r, Some("/c"))).collect();
    assert_eq!(paths, strings(&d["controlPath"]));
    let mut cases = d["sshArgs"].as_array().unwrap().iter();
    for reach in reaches() {
        for as_ in [SshAs::Root, SshAs::RootByPassword, SshAs::Login] {
            for stdin in [false, true] {
                let want = cases.next().unwrap();
                let opts = SshArgsOptions { control_dir: Some("/c".into()), stdin, as_ };
                assert_eq!(ssh_args(&reach, "echo 'it works' $HOME", &opts), strings(&want["args"]), "{reach:?} {as_:?} stdin {stdin}");
            }
        }
    }
    let dial: Vec<Vec<String>> = reaches().iter().map(ssh_dial_args).collect();
    assert_eq!(dial, d["dialArgs"].as_array().unwrap().iter().map(strings).collect::<Vec<_>>());
    let carried =
        SshCarried { reach: reaches()[0].clone(), options: vec!["ProxyCommand=nc %h %p".into(), "IdentityFile=\"~/my key\"".into()] };
    assert_eq!(ssh_back_args(&carried, 4640, 4641), strings(&d["backArgs"]));
}

#[test]
fn box_word_strips_control_chars_and_cuts_by_utf16_units() {
    let inputs = ["\x1b]0;pwned\x07motd\x1b[2J", "a\u{85}b\u{9f}c\u{a0}d", &"x".repeat(60), &"é".repeat(60), &"😀".repeat(30)];
    let want = fixture("ssh_dial.json")["boxWord"].clone();
    for (input, want) in inputs.iter().zip(want.as_array().unwrap()) {
        let got = vec![box_word(input, 48), box_word(input, 300), box_word(input, 4)];
        assert_eq!(got, strings(want), "{input:?}");
    }
}

#[test]
fn missing_known_hosts_lines_match_the_ts_and_stay_in_two_lines() {
    let want = strings(&fixture("ssh_dial.json")["missingKnownHosts"]);
    assert_eq!(missing_known_hosts_line("root@box", "/etc/x"), want[0]);
    assert_eq!(missing_known_hosts_line(&"u".repeat(60), &"/x".repeat(50)), want[1]);
    assert!(want[1].chars().count() <= SSH_LINE_CAP);
}

/// The defaults `ssh -F /dev/null -G` answered in the oracle's run.
const DEFAULTS: &[&str] = &[
    "user root",
    "hostname 203.0.113.9",
    "port 22",
    "checkhostip no",
    "identitiesonly no",
    "hostkeyalgorithms ssh-ed25519",
    "identityfile ~/.ssh/id_rsa",
    "identityfile ~/.ssh/id_ed25519",
    "globalknownhostsfile /etc/ssh/ssh_known_hosts",
    "userknownhostsfile /Users/dev/.ssh/known_hosts",
    "addkeystoagent false",
];

#[tokio::test]
async fn carried_values_match_the_ts_for_every_config() {
    // The oracle's kept known hosts file was a temp file of its own run; this run makes its own and reads the
    // fixture with that path put in its place.
    let dir = tempfile::tempdir().unwrap();
    let kept = dir.path().join("kept");
    std::fs::write(&kept, "").unwrap();
    let kept = kept.to_str().unwrap().to_string();
    let reach = SshReach { user: "root".into(), host: "box".into(), port: 2222, key_path: Some("/k".into()) };
    for case in fixture("ssh_carried.json").as_array().unwrap() {
        let name = case["name"].as_str().unwrap().to_string();
        let oracle_kept = |s: &str| -> String {
            let re = regex_lite_replace(s);
            re.replace("{KEPT}", &kept)
        };
        let config: Vec<String> = case["config"].as_array().unwrap().iter().map(|l| oracle_kept(l.as_str().unwrap())).collect();
        let code = if name == "unreadable" { 255 } else { 0 };
        let (run, asked) = local_run(move |_file, args| {
            if args.iter().any(|a| a == "/dev/null") {
                res(0, &format!("{}\n", DEFAULTS.join("\n")), "")
            } else if code == 0 {
                res(0, &format!("{}\n", config.join("\n")), "")
            } else {
                res(255, "", "line 3: Bad configuration option: frob\n")
            }
        });
        let got = match carried_ssh_values(&reach, &run).await {
            Ok(c) => json!({ "ok": { "reach": serde_json::to_value(&c.reach).unwrap(), "options": c.options } }),
            Err(e) => json!({ "err": e.to_string(), "missingKnownHosts": matches!(e, EngineError::MissingKnownHosts { .. }) }),
        };
        let want: Value = serde_json::from_str(&oracle_kept(&case["answer"].to_string())).unwrap();
        assert_eq!(got, want, "{name}");
        // The config is read first, then the bare defaults, each with the dial args and the typed login.
        let asked = asked.lock().unwrap().clone();
        assert_eq!(asked[0], [vec!["ssh".to_string(), "-G".into()], ssh_dial_args(&reach), vec!["root@box".into()]].concat(), "{name}");
        if code == 0 {
            assert_eq!(asked[1][..4], ["ssh", "-G", "-F", "/dev/null"], "{name}");
        }
    }
}

/// The oracle's temp folder for its kept file, swapped for a marker so this run can put its own path in.
fn regex_lite_replace(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("/tmp/wsp-oracle-known-") {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail.find("/kept").map(|e| e + "/kept".len()).unwrap_or(tail.len());
        out.push_str("{KEPT}");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn the_control_folder_is_made_with_its_parents_and_survives_as_the_only_thing_left() {
    let home = tempfile::tempdir().unwrap();
    let nested = home.path().join("deep").join("er");
    let dir = make_ssh_control_dir(Some(nested.to_str().unwrap())).unwrap();
    assert_eq!(dir, nested.join(".wsp").join("ssh"));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    // A second make over the same folder answers it again and changes nothing in it.
    std::fs::write(dir.join("wsp-ssh-0000000000000000.sock"), "").unwrap();
    assert_eq!(make_ssh_control_dir(Some(nested.to_str().unwrap())).unwrap(), dir);
    assert!(dir.join("wsp-ssh-0000000000000000.sock").exists());
}

#[test]
fn the_control_path_hashes_user_host_and_port_only() {
    let a = SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None };
    let path = ssh_control_path(&a, Some("/d"));
    // sha256("dev@box:22"), first 16 hex.
    assert_eq!(path, "/d/wsp-ssh-263fa7591374adf7.sock");
    assert_eq!(ssh_control_path(&SshReach { key_path: Some("/k".into()), ..a.clone() }, Some("/d")), path);
    assert_ne!(ssh_control_path(&SshReach { user: "Dev".into(), ..a.clone() }, Some("/d")), path);
}
