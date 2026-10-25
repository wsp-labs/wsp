// SPDX-License-Identifier: AGPL-3.0-only
//! The ssh machine's scripts and reads on edge inputs against the TS (tests/fixtures/scenes/units.json;
//! spec/ssh_machine.md rules M-1..M-3).
mod roads_common;

use roads_common::*;
use wsp_engine::machine::Machine;
use wsp_engine::ssh_backend::*;

const UNIT: &str = "ssh_machine";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// M-1: the landing's folder is `posix.dirname`: inner repeated slashes are kept, trailing ones go, `a/` is `.`.
#[test]
fn the_landing_folder_is_posix_dirname() {
    for case in pins()["putScript"].as_array().unwrap() {
        assert_eq!(
            put_bytes_script(case["path"].as_str().unwrap(), 3, "/tmp/wsp-put-x"),
            case["out"].as_str().unwrap(),
            "{}",
            case["path"]
        );
    }
}

/// M-2: exactly one trailing dot is stripped before the loopback test: `localhost..` is not this computer.
#[test]
fn one_trailing_dot_is_stripped() {
    for case in pins()["dials"].as_array().unwrap() {
        let reach = SshReach { user: "u".into(), host: case["host"].as_str().unwrap().to_string(), port: 22, key_path: None };
        assert_eq!(ssh_dials_this_computer(&reach, &["mac".to_string()]), case["out"].as_bool().unwrap(), "{}", case["host"]);
    }
}

/// M-3: the sudo mark is found with JS `m`: a line may end in `\r\n` or U+2028, the first mark wins, and a mark
/// inside a line or with a trailing space is no mark.
#[tokio::test]
async fn the_sudo_mark_is_read_with_javascript_line_rules() {
    let reach = SshReach { user: "u".into(), host: "h".into(), port: 22, key_path: None };
    for case in pins()["sudo"].as_array().unwrap() {
        let stdout = case["stdout"].as_str().unwrap().to_string();
        let (t, _) = transport(move |_r, _s, _o| res(0, &stdout, ""));
        let got = match read_ssh_sudo(&reach, &t).await {
            Ok(road) => serde_json::json!({ "ok": format!("{road:?}").to_lowercase() }),
            Err(e) => serde_json::json!({ "err": e.to_string() }),
        };
        assert_eq!(got, case["out"], "{:?}", case["stdout"]);
    }
}

/// M-4: a read as the login wins over any password in either nesting order (ssh-backend.ts:930): the outer wrapper
/// runs first, and a script it marked as the login passes every inner one untouched.
#[tokio::test]
async fn as_the_login_beats_a_password_in_either_nesting() {
    use std::sync::Arc;
    let (t, carried) = transport(|_r, _s, _o| res(0, "home /root\n", ""));
    let base = SshBackend::new(SshBackendOptions {
        transport: Some(t),
        host_key: Some(Arc::new(|_r| futures::FutureExt::boxed(async { None }))),
        ..Default::default()
    });
    let reach = SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None };
    let login_outside = base.riding(SshRiding::SudoPassword("p".into())).riding(SshRiding::AsLogin);
    login_outside.adopt(&reach).await.unwrap().machine.exec("true", Default::default()).await.unwrap();
    let login_inside = base.riding(SshRiding::AsLogin).riding(SshRiding::SudoPassword("p".into()));
    login_inside.adopt(&reach).await.unwrap().machine.exec("true", Default::default()).await.unwrap();
    let rides: Vec<SshRide> = carried.lock().unwrap().iter().map(|c| c.opts.ride.clone()).collect();
    assert_eq!(rides, vec![SshRide::Login; 4]);
}
