// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the login lines, staging and disk reads against the TS's own output (tests/fixtures/oracle/
//! login.json, small.json). spec/machine_reads.md.
#[path = "rest_common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use common::{exit, ok, other, FakeMachine};
use wsp_engine::daemon_targets::Platform;
use wsp_engine::disk_sync::{disk_use_of, sync_disk, DiskReading, DiskUse, SyncDiskError};
use wsp_engine::errors::Result;
use wsp_engine::target_line::{as_login, login_shell_path, stage_as_login, target_login, GivenLogin, TargetLogin};

fn oracle(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(name);
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn login_of(v: &Value) -> TargetLogin {
    TargetLogin {
        platform: if v["platform"] == "darwin" { Platform::Darwin } else { Platform::Linux },
        home: v["home"].as_str().unwrap().into(),
        path: v["path"].as_str().map(String::from),
        user: v["user"].as_str().unwrap().into(),
        run_as: v["runAs"].as_str().map(String::from),
    }
}

// spec/machine_reads.md V1, V2
#[tokio::test]
async fn probes_write_the_ts_lines_byte_for_byte() {
    for c in oracle("login.json")["probes"].as_array().unwrap() {
        let m = FakeMachine::answering(&["Linux", "1000", "ada", "ada", "1", "/home/ada", "/usr/bin"]);
        let given = GivenLogin { home: c["given"]["HOME"].as_str().map(String::from), path: c["given"]["PATH"].as_str().map(String::from) };
        let got = target_login(&m, &given).await.unwrap();
        assert_eq!(m.cmds(), vec![c["cmd"].as_str().unwrap().to_string()]);
        assert_eq!(m.ran()[0].opts.timeout_ms, Some(20_000));
        assert_eq!(got, login_of(&c["login"]), "{}", c["given"]);
    }
    let failing = FakeMachine::exec("m1", |_, _| Ok(exit(1, "", "  \nnope\nmore")));
    assert_eq!(
        target_login(&failing, &GivenLogin::default()).await.unwrap_err().to_string(),
        "the computer did not say who its lines run as: nope"
    );
    // Lines missing from the answer read as empty: no PATH, the owner falls back to the own name.
    let short = FakeMachine::answering(&["Darwin", "501", "ada"]);
    assert_eq!(
        target_login(&short, &GivenLogin::default()).await.unwrap(),
        TargetLogin { platform: Platform::Darwin, home: String::new(), path: None, user: "ada".into(), run_as: None }
    );
}

// spec/machine_reads.md V5
#[test]
fn as_login_writes_the_ts_lines() {
    for c in oracle("login.json")["asLogin"].as_array().unwrap() {
        assert_eq!(as_login(&login_of(&c["t"]), c["line"].as_str().unwrap()), c["out"].as_str().unwrap());
    }
}

// spec/machine_reads.md V6
#[tokio::test]
async fn login_shell_path_takes_the_last_line_and_nothing_on_failure() {
    let o = oracle("login.json");
    let t = TargetLogin {
        platform: Platform::Linux,
        home: "/home/a'da".into(),
        path: Some("/usr/bin:/bin".into()),
        user: "ada".into(),
        run_as: Some("ada".into()),
    };
    let m = FakeMachine::exec("m1", |_, _| Ok(ok("hello\n  /usr/local/bin:/usr/bin  ")));
    assert_eq!(login_shell_path(&m, &t).await, Some("/usr/local/bin:/usr/bin".into()));
    assert_eq!(m.cmds(), vec![o["loginShellCmd"].as_str().unwrap().to_string()]);
    assert_eq!(m.ran()[0].opts.timeout_ms, Some(20_000));
    let trailing = FakeMachine::exec("m1", |_, _| Ok(ok("/usr/bin\n")));
    assert_eq!(login_shell_path(&trailing, &t).await, None, "the last line is the empty one after the newline");
    let failed = FakeMachine::exec("m1", |_, _| Ok(exit(124, "/usr/bin", "")));
    assert_eq!(login_shell_path(&failed, &t).await, None);
    let thrown = FakeMachine::exec("m1", |_, _| Err(other("gone")));
    assert_eq!(login_shell_path(&thrown, &t).await, None);
    let mac = FakeMachine::new("m1");
    assert_eq!(login_shell_path(&mac, &TargetLogin { platform: Platform::Darwin, ..t.clone() }).await, None);
    assert!(mac.ran().is_empty());
}

// spec/machine_reads.md V7
#[tokio::test]
async fn staging_removes_its_folder_whatever_happened() {
    let t = TargetLogin { platform: Platform::Linux, home: "/home/ada".into(), path: None, user: "ada".into(), run_as: Some("ada".into()) };
    let mut m = FakeMachine::new("m1");
    m.put_bytes_fn = Some(Arc::new(|_, _, _| Ok(None)));
    let seen = Arc::new(Mutex::new(None::<(String, String)>));
    let s2 = seen.clone();
    let err = stage_as_login(
        &m,
        &m,
        &t,
        "the daemon",
        b"bytes",
        move |file, dir| {
            *s2.lock().unwrap() = Some((file, dir));
            async { Result::<()>::Err(other("use failed")) }
        },
        Some(9),
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "use failed");
    let (file, dir) = seen.lock().unwrap().clone().unwrap();
    assert!(
        dir.starts_with("/tmp/wsp-land-")
            && dir.len() == "/tmp/wsp-land-".len() + 12
            && dir["/tmp/wsp-land-".len()..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "{dir}"
    );
    assert_eq!(file, format!("{dir}/bytes"));
    let cmds = m.cmds();
    assert_eq!(
        cmds,
        vec![format!("mkdir -m 0700 '{dir}'"), format!("chmod 0600 '{file}' && chown -R -- 'ada' '{dir}'"), format!("rm -rf -- '{dir}'")]
    );
    assert!(m.ran().iter().all(|r| r.opts.timeout_ms == Some(9)));
    assert_eq!(m.puts.lock().unwrap().clone(), vec![(file.clone(), b"bytes".to_vec(), Some(9))]);
    // A folder that could not be made is never removed: nothing of this call is there to take.
    let refused = FakeMachine::exec("m2", |_, _| Ok(exit(1, "", "mkdir: denied")));
    let err = stage_as_login(&refused, &refused, &t, "the daemon", b"x", |_, _| async { Result::<()>::Ok(()) }, None).await.unwrap_err();
    assert_eq!(err.to_string(), "no private folder to stage the daemon in: mkdir: denied");
    assert_eq!(refused.cmds().len(), 1);
    assert_eq!(refused.ran()[0].opts.timeout_ms, None);
}

// spec/machine_reads.md V14
#[test]
fn disk_use_matches_the_ts_oracle() {
    for c in oracle("small.json")["diskUse"].as_array().unwrap() {
        let want = if c["use"].is_null() {
            None
        } else {
            Some(DiskUse {
                used_bytes: c["use"]["usedBytes"].as_f64().unwrap(),
                free_bytes: c["use"]["freeBytes"].as_f64().unwrap(),
                pct: c["use"]["pct"].as_f64().unwrap(),
            })
        };
        assert_eq!(disk_use_of(c["stdout"].as_str().unwrap()), want, "{}", c["stdout"]);
    }
}

// spec/machine_reads.md V13
#[tokio::test]
async fn sync_disk_reads_the_counters_and_says_what_a_failed_sync_said() {
    let m = FakeMachine::exec("m1", |_, _| Ok(ok("Dirty: 12\nWriteback: 0\n")));
    assert_eq!(sync_disk(&m).await.unwrap(), Some(DiskReading { dirty_kb: 12.0, writeback_kb: 0.0 }));
    assert_eq!(m.ran()[0].opts.timeout_ms, Some(20_000));
    let half = FakeMachine::exec("m1", |_, _| Ok(ok("Dirty: 12\n")));
    assert_eq!(sync_disk(&half).await.unwrap(), None);
    let failed = FakeMachine::exec("m1", |_, _| Ok(exit(1, " out ", " err ")));
    match sync_disk(&failed).await.unwrap_err() {
        SyncDiskError::Sync { answer, line } => {
            assert_eq!(answer, "it exited 1 and said: out err");
            assert_eq!(line, wsp_words::words::init::disk_sync_failed_line("it exited 1 and said: out err"));
        }
        other => panic!("{other:?}"),
    }
    let silent = FakeMachine::exec("m1", |_, _| Ok(exit(2, "", "")));
    assert!(
        matches!(sync_disk(&silent).await.unwrap_err(), SyncDiskError::Sync { answer, .. } if answer == "it printed nothing and exited 2")
    );
    let thrown = FakeMachine::exec("m1", |_, _| Err(other("road down")));
    assert_eq!(sync_disk(&thrown).await.unwrap_err(), SyncDiskError::Exec(other("road down")));
}
