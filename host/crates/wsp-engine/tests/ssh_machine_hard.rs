// SPDX-License-Identifier: AGPL-3.0-only
//! The ssh machine unit against the TS code's own answers (tests/fixtures/roads/ssh_machine.json, ssh_read.json).
//! Spec: spec/ssh_machine.md.
mod roads_common;

use std::sync::{Arc, Mutex};

use roads_common::*;
use serde_json::{json, Value};
use wsp_engine::machine::Machine;
use wsp_engine::ssh_backend::*;

fn reach() -> SshReach {
    SshReach { user: "dev".into(), host: "box".into(), port: 22, key_path: None }
}

#[test]
fn scripts_match_the_ts_byte_for_byte() {
    let d = fixture("ssh_machine.json");
    let vars: Vec<String> = d["storeVars"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(ssh_store_vars(), vars);
    assert_eq!(login_read(), d["loginRead"].as_str().unwrap());
    assert_eq!(ssh_read_script(), d["readScript"].as_str().unwrap());
    assert_eq!(ssh_facts_script(), d["factsScript"].as_str().unwrap());
    assert_eq!(SSH_SUDO_READ, d["sudoRead"].as_str().unwrap());
    assert_eq!(SSH_SUDO_TRY, d["sudoTry"].as_str().unwrap());
    let scripts = [
        put_bytes_script("/home/maya/.wsp/daemon.tgz", 5, "/home/maya/.wsp/daemon.tgz.wsp-in-abababababab"),
        put_bytes_script("file", 0, "file.t"),
        put_bytes_script("/it's here/f", 12, "/it's here/f.t"),
    ];
    let want: Vec<String> = d["putBytesScript"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(scripts.to_vec(), want);
}

#[test]
fn plain_path_matches_the_ts() {
    for case in fixture("ssh_machine.json")["plainPath"].as_array().unwrap() {
        assert_eq!(plain_path(case["path"].as_str()), case["plain"].as_str().unwrap(), "{:?}", case["path"]);
    }
}

#[test]
fn dials_this_computer_matches_the_ts() {
    for case in fixture("ssh_machine.json")["dialsThis"].as_array().unwrap() {
        let host = case["host"].as_str().unwrap();
        let names: Vec<String> = case["names"].as_array().unwrap().iter().map(|n| n.as_str().unwrap().to_string()).collect();
        let r = SshReach { user: "u".into(), host: host.into(), port: 22, key_path: None };
        assert_eq!(ssh_dials_this_computer(&r, &names), case["here"].as_bool().unwrap(), "{host} among {names:?}");
    }
}

fn read_json(read: &SshMachineRead) -> Value {
    let mut out = json!({ "login": serde_json::to_value(&read.login).unwrap(), "shape": serde_json::to_value(&read.shape).unwrap() });
    for (key, value) in [("system", &read.system), ("arch", &read.arch), ("shell", &read.shell)] {
        if let Some(v) = value {
            out[key] = json!(v);
        }
    }
    out
}

#[tokio::test]
async fn read_answers_match_the_ts_with_login_key_order() {
    let cases = fixture("ssh_read.json");
    for case in cases.as_array().unwrap() {
        let stdout = case["stdout"].as_str().unwrap().to_string();
        let failing = stdout.is_empty();
        let (t, carried) = transport(move |_r, _s, _o| if failing { res(255, " out ", "debug1: a\n") } else { res(0, &stdout, "") });
        let got = match read_ssh_machine(&reach(), &t).await {
            Ok(read) => json!({ "ok": read_json(&read) }),
            Err(e) => json!({ "err": e.to_string(), "missingKnownHosts": false }),
        };
        assert_eq!(got, case["read"], "{:?}", case["stdout"]);
        if let Some(login) = case["read"]["ok"]["login"].as_object() {
            // Key order is part of the record: the stores first, then HOME, USER and PATH.
            assert_eq!(serde_json::to_string(&got["ok"]["login"]).unwrap(), serde_json::to_string(login).unwrap());
        }
        let carried = carried.lock().unwrap();
        assert_eq!(carried[0].script, ssh_read_script());
        assert_eq!(carried[0].opts.timeout_ms, Some(30_000));
        assert_eq!(carried[0].opts.ride, SshRide::Root);
    }
}

#[tokio::test]
async fn a_success_clears_the_held_warning() {
    let answer = Arc::new(Mutex::new(res(255, "", "ssh: connect to host box port 22: Connection refused\n")));
    let a = answer.clone();
    let (t, _carried) = transport(move |_r, _s, _o| a.lock().unwrap().clone());
    let warned = Arc::new(Mutex::new(Vec::<String>::new()));
    let w = warned.clone();
    let machine = SshMachine::with_warn(reach(), t, Arc::new(move |line: &str| w.lock().unwrap().push(line.to_string()))).unwrap();
    assert!(machine.facts().await.is_err());
    *answer.lock().unwrap() = res(0, "pretty Debian\nuptime 10.5\nhome /root\n", "");
    let facts = machine.facts().await.unwrap();
    assert_eq!((facts.os.as_str(), facts.uptime_ms.0, facts.folder.as_str()), ("Debian", 10_500.0, "/root"));
    *answer.lock().unwrap() = res(255, "", "ssh: connect to host box port 22: Connection refused\n");
    assert!(machine.facts().await.is_err());
    // The same refusal twice, a success between: it is said again, since the success forgot the first.
    let warned = warned.lock().unwrap().clone();
    assert_eq!(warned.len(), 2);
    assert_eq!(warned[0], warned[1]);
    assert_eq!(warned[0], "dev@box did not say what it is over ssh (exit 255): ssh: connect to host box port 22: Connection refused");
}

#[tokio::test]
async fn a_riding_backend_keeps_its_first_password_and_reads_as_the_login_stay_bare() {
    let (t, carried) = transport(|_r, _s, _o| res(0, "home /root\nWSP_SUDO free\n", ""));
    let base = SshBackend::new(SshBackendOptions {
        transport: Some(t),
        host_key: Some(Arc::new(|_r| futures::FutureExt::boxed(async { None }))),
        ..Default::default()
    });
    let twice = base.riding(SshRiding::SudoPassword("first".into())).riding(SshRiding::SudoPassword("second".into()));
    let adopted = twice.adopt(&reach()).await.unwrap();
    adopted.machine.exec("true", Default::default()).await.unwrap();
    assert_eq!(twice.sudo_for(&reach()).await.unwrap(), SshSudo::Free);
    let as_login = base.riding(SshRiding::AsLogin);
    as_login.adopt(&reach()).await.unwrap();
    let rides: Vec<SshRide> = carried.lock().unwrap().iter().map(|c| c.opts.ride.clone()).collect();
    assert_eq!(
        rides,
        vec![SshRide::RootByPassword("first".into()), SshRide::RootByPassword("first".into()), SshRide::Login, SshRide::Login,]
    );
}

#[tokio::test]
async fn adopt_reads_no_key_when_the_read_fails() {
    let (t, _c) = transport(|_r, _s, _o| res(255, "", "dev@box: Permission denied (publickey).\n"));
    let keys = Arc::new(Mutex::new(0));
    let k = keys.clone();
    let backend = SshBackend::new(SshBackendOptions {
        transport: Some(t),
        host_key: Some(Arc::new(move |_r| {
            *k.lock().unwrap() += 1;
            futures::FutureExt::boxed(async { Some("ssh-ed25519 SHA256:x".to_string()) })
        })),
        ..Default::default()
    });
    let e = backend.adopt(&reach()).await.err().unwrap();
    assert_eq!(e.to_string(), "dev@box did not answer over ssh: dev@box: Permission denied (publickey).");
    assert_eq!(*keys.lock().unwrap(), 0);
}

#[tokio::test]
async fn sudo_answers_a_mark_whatever_the_exit_and_reads_the_try_in_order() {
    let (t, _c) = transport(|_r, s, _o| {
        if s == SSH_SUDO_READ {
            res(1, "noise\nWSP_SUDO tty\n", "x")
        } else {
            res(1, "WSP_SUDO taken\n", "must have a tty\n1 incorrect password attempt")
        }
    });
    assert_eq!(read_ssh_sudo(&reach(), &t).await.unwrap(), SshSudo::Tty);
    // stdout's taken is read before stderr's tty and wrong.
    assert_eq!(try_ssh_sudo(&reach(), "pw", &t).await, SshSudo::Taken);
    let (quiet, _c) = transport(|_r, _s, _o| res(0, "   \n", ""));
    assert_eq!(read_ssh_sudo(&reach(), &quiet).await.err().unwrap().to_string(), "dev@box did not answer over ssh: ");
    let (half, _c) = transport(|_r, _s, _o| res(0, "WSP_SUDO taken\n", ""));
    assert_eq!(read_ssh_sudo(&reach(), &half).await.err().unwrap().to_string(), "dev@box did not answer over ssh: WSP_SUDO taken");
}

#[tokio::test]
async fn put_bytes_lands_under_a_name_of_its_own_and_says_no_pieces() {
    let (t, carried) = transport(|_r, _s, _o| res(0, "WSP_BYTES_OK\n", ""));
    let machine = SshMachine::new(reach(), t).unwrap();
    assert_eq!(machine.put_bytes("/root/f", b"abc", Some(900)).await.unwrap(), None);
    let c = carried.lock().unwrap()[0].clone();
    let tmp = c.script.lines().find_map(|l| l.strip_prefix("cat > '")).unwrap().trim_end_matches('\'').to_string();
    let suffix = tmp.strip_prefix("/root/f.wsp-in-").unwrap();
    assert!(suffix.len() == 12 && suffix.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()), "{tmp}");
    assert_eq!(c.script, put_bytes_script("/root/f", 3, &tmp));
    assert_eq!((c.opts.stdin.as_deref(), c.opts.timeout_ms), (Some(&b"abc"[..]), Some(900)));
}
