// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/target-line.test.ts, land-bytes.test.ts, disk-sync.test.ts and the refreshPreviewToken
//! cases of preview.test.ts. The Workspace.daemonReach and Workspace.portReach cases of preview.test.ts are not here:
//! they drive lifecycle.rs's Workspace, which this file's interface does not name.
#[path = "rest_common/mod.rs"]
mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::PreviewReach;
use wsp_words::agents_report::{no_home_refusal, no_runuser_refusal};

use common::{reply_text, FakeFetch, FakeMachine, Ran};
use wsp_engine::daemon_targets::{platform_of_system, Platform, DAEMON_TARGETS};
use wsp_engine::disk_sync::{disk_unsettled, disk_use_pct, sync_disk, DiskReading, SyncDiskError, DISK_SYNC_CMD, DISK_USE_CMD};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::land_bytes::{land_bytes, lands_bytes, LandBytesOptions};
use wsp_engine::machine::{BytesLanded, ExecOptions, ExecResult};
use wsp_engine::preview::refresh_preview_token;
use wsp_engine::target_line::{as_login, land_as_login, target_login, GivenLogin, TargetLogin};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn script(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

// ---- target-line

/// A computer's road that answers the probe with the lines given and records every command.
fn probed(lines: &[&str]) -> FakeMachine {
    FakeMachine::answering(lines)
}

fn given(home: Option<&str>, path: Option<&str>) -> GivenLogin {
    GivenLogin { home: home.map(String::from), path: path.map(String::from) }
}

// TS: packages/engine/test/target-line.test.ts:25
#[test]
fn reads_nodes_platform_off_the_target_tables_own_rows_and_nothing_for_a_system_no_row_has() {
    for row in DAEMON_TARGETS {
        assert_eq!(platform_of_system(row.system), Some(row.platform), "{}", row.system);
    }
    assert_eq!(platform_of_system("FreeBSD"), None);
}

// TS: packages/engine/test/target-line.test.ts:32
#[tokio::test]
async fn a_root_daemon_hands_every_line_to_the_owner_of_the_home_never_to_root_with_the_home_and_path_reported() {
    let machine = probed(&["Linux", "0", "root", "ada", "1", "/root", "/usr/bin:/bin"]);
    let login = target_login(&machine, &given(Some("/home/ada"), Some("/opt/wsp/bin:/usr/bin"))).await.unwrap();
    assert!(machine.cmds()[0].contains("'/home/ada'"), "{}", machine.cmds()[0]);
    assert_eq!(
        login,
        TargetLogin {
            platform: Platform::Linux,
            home: "/home/ada".into(),
            path: Some("/opt/wsp/bin:/usr/bin".into()),
            user: "ada".into(),
            run_as: Some("ada".into())
        }
    );
    let line = as_login(&login, "ls ~/.claude/skills");
    assert!(line.starts_with("runuser -u 'ada' -- bash -c "), "{line}");
    assert!(!line.contains("-u 'root'") && !line.contains("-u root"), "{line}");
    let unwrapped = TargetLogin { run_as: None, ..login };
    assert_eq!(as_login(&unwrapped, "true"), r#"export HOME='/home/ada' PATH='/opt/wsp/bin:/usr/bin'; cd "$HOME" 2>/dev/null; true"#);
}

// TS: packages/engine/test/target-line.test.ts:44
#[tokio::test]
async fn a_mac_a_login_that_is_not_root_and_a_fork_whose_home_is_roots_own_run_the_line_as_it_is() {
    let mac = target_login(&probed(&["Darwin", "0", "root", "ada", "0", "/Users/ada", "/usr/bin"]), &GivenLogin::default()).await.unwrap();
    assert_eq!(mac.run_as, None);
    assert!(as_login(&mac, "true").starts_with("export HOME='/Users/ada'"));
    let plain =
        target_login(&probed(&["Linux", "1000", "ada", "ada", "1", "/home/ada", "/usr/bin"]), &GivenLogin::default()).await.unwrap();
    assert_eq!(plain.run_as, None);
    let fork = target_login(&probed(&["Linux", "0", "root", "root", "1", "/root", "/usr/bin"]), &GivenLogin::default()).await.unwrap();
    assert_eq!(
        fork,
        TargetLogin { platform: Platform::Linux, home: "/root".into(), path: Some("/usr/bin".into()), user: "root".into(), run_as: None }
    );
}

// TS: packages/engine/test/target-line.test.ts:54
#[tokio::test]
async fn refuses_to_read_at_all_where_the_road_is_root_the_home_is_somebody_elses_and_there_is_no_runuser() {
    let machine = probed(&["Linux", "0", "root", "ada", "0", "/root", "/usr/bin"]);
    let err = target_login(&machine, &given(Some("/home/ada"), None)).await.unwrap_err();
    assert_eq!(err.to_string(), no_runuser_refusal("ada"));
}

// TS: packages/engine/test/target-line.test.ts:58
#[tokio::test]
async fn reads_the_owner_of_the_folder_a_linked_home_points_at_not_of_the_link() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    // A root daemon on Linux, as the probe asks it; stat is the real one.
    script(&bin.join("uname"), "#!/bin/sh\necho Linux\n");
    script(&bin.join("id"), "#!/bin/sh\ncase \"$1\" in -u) echo 0;; -un) echo root;; esac\n");
    // The link is this test's own; the folder behind it is root's.
    let home = root.path().join("home-link");
    std::os::unix::fs::symlink("/", &home).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let machine = FakeMachine::exec("m1", move |cmd, _| {
        let out = Command::new("/bin/bash").args(["-c", cmd]).env_clear().env("PATH", &path).output().unwrap();
        Ok(if out.status.success() {
            common::ok(&String::from_utf8_lossy(&out.stdout))
        } else {
            common::exit(1, "", &String::from_utf8_lossy(&out.stderr))
        })
    });
    let login = target_login(&machine, &given(Some(home.to_str().unwrap()), None)).await.unwrap();
    assert_eq!(login.user, "root");
    assert_eq!(login.run_as, None);
}

// TS: packages/engine/test/target-line.test.ts:83
#[tokio::test]
async fn refuses_where_a_root_road_names_a_home_that_is_not_there_rather_than_running_as_root() {
    let machine = probed(&["Linux", "0", "root", "", "1", "/root", "/usr/bin"]);
    let err = target_login(&machine, &given(Some("/home/gone"), None)).await.unwrap_err();
    assert_eq!(err.to_string(), no_home_refusal("/home/gone"));
    // A login that is not root with no home still runs as itself.
    let itself = target_login(&probed(&["Linux", "1000", "ada", "", "1", "/home/ada", "/usr/bin"]), &GivenLogin::default()).await.unwrap();
    assert_eq!(itself.user, "ada");
}

// TS: packages/engine/test/target-line.test.ts:89
#[test]
fn the_wrapped_line_runs_in_bash_as_the_logins_own_quoting_and_all() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let home = root.path().join("it's home");
    std::fs::create_dir(&home).unwrap();
    // runuser as a script: checks the words it is handed and runs what follows them, as the real one does.
    script(&bin.join("runuser"), "#!/bin/bash\n[ \"$1\" = -u ] && [ \"$2\" = ada ] && [ \"$3\" = -- ] || exit 9\nshift 3\nexec \"$@\"\n");
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let login = TargetLogin {
        platform: Platform::Linux,
        home: home.to_str().unwrap().to_string(),
        path: Some(path.clone()),
        user: "ada".into(),
        run_as: Some("ada".into()),
    };
    let line = as_login(&login, r#"printf '%s|%s' "$HOME" "$(pwd -P)""#);
    let out = Command::new("/bin/bash").args(["-c", &line]).env_clear().env("PATH", &path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let (said, wh) = text.split_once('|').unwrap();
    assert_eq!(said, home.to_str().unwrap());
    assert!(wh.ends_with("it's home"), "{wh}");
}

// ---- bytes landed as the login

type Log = Arc<Mutex<Vec<String>>>;

/// A machine that logs every exec and every landing in order; `fail` makes an exec whose text holds it exit 1.
fn landing_machine(fail: Option<&'static str>) -> (FakeMachine, Log, Log) {
    let ran: Log = Arc::new(Mutex::new(Vec::new()));
    let landed: Log = Arc::new(Mutex::new(Vec::new()));
    let exec_log = ran.clone();
    let mut m = FakeMachine::exec("box", move |cmd, _| {
        exec_log.lock().unwrap().push(cmd.to_string());
        Ok(match fail {
            Some(f) if cmd.contains(f) => common::exit(1, "", "Permission denied\n"),
            _ => common::ok(""),
        })
    });
    let (put_ran, put_landed) = (ran.clone(), landed.clone());
    m.put_bytes_fn = Some(Arc::new(move |path, _, _| {
        put_landed.lock().unwrap().push(path.to_string());
        put_ran.lock().unwrap().push(format!("land {path}"));
        Ok(None::<BytesLanded>)
    }));
    (m, ran, landed)
}

fn ada() -> TargetLogin {
    TargetLogin { platform: Platform::Linux, home: "/home/ada".into(), path: None, user: "ada".into(), run_as: Some("ada".into()) }
}

/// What `chmod 0?6?44|chmod [ago]*\+r` finds: a mode that opens the bytes to anyone.
fn opens_to_all(text: &str) -> bool {
    text.match_indices("chmod ").any(|(i, _)| {
        let rest = &text[i + 6..];
        let r = rest.strip_prefix('0').unwrap_or(rest);
        let r = r.strip_prefix('6').unwrap_or(r);
        r.starts_with("44") || rest.trim_start_matches(['a', 'g', 'o']).starts_with("+r")
    })
}

// TS: packages/engine/test/target-line.test.ts:124
#[tokio::test]
async fn stages_the_bytes_in_a_folder_only_the_login_can_open_writes_them_as_the_login_and_takes_the_folder_away() {
    let (machine, ran, landed) = landing_machine(None);
    land_as_login(&machine, &ada(), "/home/ada/.codex/config.toml", b"x", false, None).await.unwrap();
    let landed = landed.lock().unwrap().clone();
    assert_eq!(landed.len(), 1);
    let staging = landed[0].clone();
    let folder = staging[..staging.rfind('/').unwrap()].to_string();
    assert!(folder.starts_with("/tmp/wsp-land-"), "{folder}");
    // Private before a byte lands, the bytes landed while only root can open it, and only then handed to the login
    // with the file inside: never a folder the login owns that root then writes into by name.
    let ran = ran.lock().unwrap().clone();
    assert_eq!(
        ran[..3].to_vec(),
        vec![
            format!("mkdir -m 0700 '{folder}'"),
            format!("land {staging}"),
            format!("chmod 0600 '{staging}' && chown -R -- 'ada' '{folder}'")
        ]
    );
    assert!(!opens_to_all(&ran.join("\n")), "{ran:?}");
    assert!(ran[3].starts_with("runuser -u 'ada' -- bash -c "), "{}", ran[3]);
    assert!(ran[3].contains("/home/ada/.codex/config.toml"), "{}", ran[3]);
    assert_eq!(ran.last().unwrap(), &format!("rm -rf -- '{folder}'"));
}

// TS: packages/engine/test/target-line.test.ts:140
#[tokio::test]
async fn takes_away_no_folder_it_did_not_make_a_name_already_taken_refuses_the_landing_and_leaves_that_folder_be() {
    let (machine, ran, landed) = landing_machine(Some("mkdir"));
    let err = land_as_login(&machine, &ada(), "/home/ada/.codex/config.toml", b"x", false, None).await.unwrap_err();
    assert!(err.to_string().contains("no private folder"), "{err}");
    assert_eq!(landed.lock().unwrap().clone(), Vec::<String>::new());
    assert_eq!(ran.lock().unwrap().len(), 1);
}

// TS: packages/engine/test/target-line.test.ts:147
#[tokio::test]
async fn unpacks_a_folder_as_the_login_and_a_write_the_login_is_refused_still_takes_the_staging_folder_away() {
    let (machine, ran, _) = landing_machine(None);
    land_as_login(&machine, &ada(), "/home/ada/.agents/skills/pdf", b"tgz", true, None).await.unwrap();
    assert!(ran.lock().unwrap()[3].contains("tar -xzf"), "{:?}", ran.lock().unwrap());
    let (refused, refused_ran, _) = landing_machine(Some("runuser"));
    let err = land_as_login(&refused, &ada(), "/home/ada/.claude.json", b"{}", false, None).await.unwrap_err();
    assert!(err.to_string().contains("was not written as ada: Permission denied"), "{err}");
    assert!(refused_ran.lock().unwrap().last().unwrap().starts_with("rm -rf -- '/tmp/wsp-land-"), "{:?}", refused_ran.lock().unwrap());
}

// TS: packages/engine/test/target-line.test.ts:156
#[tokio::test]
async fn writes_a_real_file_through_the_line_it_builds_keeping_the_mode_a_file_there_already_had() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let dest = home.join(".claude.json");
    std::fs::write(&dest, "{}").unwrap();
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600)).unwrap();
    let modes: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let folders: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut machine = FakeMachine::exec("here", |cmd, _| {
        let out = Command::new("/bin/bash").args(["-c", cmd]).output().unwrap();
        Ok(if out.status.success() {
            common::ok(&String::from_utf8_lossy(&out.stdout))
        } else {
            common::exit(1, "", &String::from_utf8_lossy(&out.stderr))
        })
    });
    let (put_modes, put_folders) = (modes.clone(), folders.clone());
    machine.put_bytes_fn = Some(Arc::new(move |path, bytes, _| {
        let folder = Path::new(path).parent().unwrap().to_path_buf();
        put_folders.lock().unwrap().push(folder.to_str().unwrap().to_string());
        put_modes.lock().unwrap().push(std::fs::metadata(&folder).unwrap().permissions().mode() & 0o777);
        std::fs::write(path, bytes).unwrap();
        Ok(None)
    }));
    let login =
        TargetLogin { platform: Platform::Darwin, home: home.to_str().unwrap().to_string(), path: None, user: "ada".into(), run_as: None };
    land_as_login(&machine, &login, dest.to_str().unwrap(), br#"{"mcpServers":{}}"#, false, None).await.unwrap();
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), r#"{"mcpServers":{}}"#);
    // The bytes landed where nobody but the folder's owner could read them, and nothing of them is left in /tmp.
    assert_eq!(modes.lock().unwrap().clone(), vec![0o700]);
    let folders = folders.lock().unwrap().clone();
    assert_eq!(folders.len(), 1);
    assert!(!Path::new(&folders[0]).exists());
    let listing = Command::new("/bin/bash").args(["-c", &format!("ls -l '{}'", dest.display())]).output().unwrap();
    assert!(String::from_utf8_lossy(&listing.stdout).starts_with("-rw-------"), "{}", String::from_utf8_lossy(&listing.stdout));
}

// ---- land-bytes

fn caps(signed_urls: bool) -> Capabilities {
    Capabilities {
        live_clone_forks: false,
        pause_mode: None,
        replaces_machine: false,
        preview_urls: false,
        signed_urls,
        callback_relay: false,
        disk_snapshots: false,
        images: false,
        snapshots_any_life: false,
        snapshot_listing: false,
        templates: false,
        sizes: vec![],
        kept: false,
        copies: false,
        own_network: false,
    }
}

// TS: packages/engine/test/land-bytes.test.ts:22
#[tokio::test]
async fn takes_the_backends_own_road_when_it_has_one_with_the_callers_own_bound_on_it() {
    let mut machine = FakeMachine::new("m1");
    machine.put_bytes_fn = Some(Arc::new(|_, _, _| Ok(None)));
    land_bytes(&machine, "/root/x.tgz", b"payload", LandBytesOptions::default()).await.unwrap();
    land_bytes(&machine, "/root/y.tgz", b"more", LandBytesOptions { fetch: None, timeout_ms: Some(5_000) }).await.unwrap();
    assert_eq!(
        machine.puts.lock().unwrap().clone(),
        vec![("/root/x.tgz".to_string(), b"payload".to_vec(), None), ("/root/y.tgz".to_string(), b"more".to_vec(), Some(5_000))]
    );
}

// TS: packages/engine/test/land-bytes.test.ts:33
#[tokio::test]
async fn puts_the_bytes_at_the_signed_url_when_that_is_the_only_road() {
    let mut machine = FakeMachine::new("m1");
    machine.upload_url_fn = Arc::new(|path| Ok(format!("https://put.example{path}")));
    let fetch = FakeFetch::fixed(vec![("PUT /root/x.tgz", reply_text(200, ""))]);
    land_bytes(&machine, "/root/x.tgz", b"payload", LandBytesOptions { fetch: Some(fetch.clone()), timeout_ms: None }).await.unwrap();
    let puts: Vec<(String, String)> = fetch.calls().iter().map(|c| (c.url.clone(), c.method.clone())).collect();
    assert_eq!(puts, vec![("https://put.example/root/x.tgz".to_string(), "PUT".to_string())]);
}

// TS: packages/engine/test/land-bytes.test.ts:45
#[tokio::test]
async fn names_the_file_and_the_machine_when_the_signed_put_is_refused() {
    let mut machine = FakeMachine::new("m1");
    machine.upload_url_fn = Arc::new(|_| Ok("https://put.example/x".to_string()));
    let fetch = FakeFetch::fixed(vec![("PUT /x", reply_text(503, ""))]);
    let err = land_bytes(&machine, "/root/x.tgz", b"p", LandBytesOptions { fetch: Some(fetch), timeout_ms: None }).await.unwrap_err();
    assert!(err.to_string().contains("/root/x.tgz did not land on m1: HTTP 503"), "{err}");
}

// TS: packages/engine/test/land-bytes.test.ts:52
#[test]
fn says_whether_bytes_can_land_at_all_so_nothing_offers_a_daemon_where_they_could_not() {
    assert!(lands_bytes(&caps(true), &FakeMachine::new("m1")));
    assert!(!lands_bytes(&caps(false), &FakeMachine::new("m1")));
    let mut with_road = FakeMachine::new("m1");
    with_road.put_bytes_fn = Some(Arc::new(|_, _, _| Ok(None)));
    assert!(lands_bytes(&caps(false), &with_road));
}

// ---- disk-sync

/// The inline bound exec-detached.ts's INLINE_EXEC_MS holds (exec_detached.rs is not ported yet).
const INLINE_EXEC_MS: u64 = 20_000;

fn printed(stdout: &str) -> ExecResult {
    common::ok(stdout)
}

fn guest(answer: ExecResult) -> FakeMachine {
    FakeMachine::exec("m1", move |_, _| Ok(answer.clone()))
}

fn sync_ran() -> Ran {
    Ran { cmd: DISK_SYNC_CMD.to_string(), opts: ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() } }
}

// TS: packages/engine/test/disk-sync.test.ts:85
#[tokio::test]
async fn runs_the_one_sync_command_under_the_inline_bound_and_answers_both_counters_at_zero_as_settled() {
    let machine = guest(printed("Dirty: 0\nWriteback: 0\n"));
    let read = sync_disk(&machine).await.unwrap();
    assert_eq!(read, Some(DiskReading { dirty_kb: 0.0, writeback_kb: 0.0 }));
    assert!(!disk_unsettled(read.as_ref()));
    assert_eq!(machine.ran(), vec![sync_ran()]);
}

// TS: packages/engine/test/disk-sync.test.ts:93
#[tokio::test]
async fn answers_what_a_writer_still_running_left_once_and_never_waits_for_it_to_stop() {
    let machine = guest(printed("Dirty: 12288\nWriteback: 64\n"));
    let read = sync_disk(&machine).await.unwrap();
    assert_eq!(read, Some(DiskReading { dirty_kb: 12288.0, writeback_kb: 64.0 }));
    assert!(disk_unsettled(read.as_ref()));
    assert_eq!(machine.ran().len(), 1);
}

// TS: packages/engine/test/disk-sync.test.ts:101
#[tokio::test]
async fn a_sync_that_exits_non_zero_throws_the_typed_failure_with_the_machines_answer_in_the_sentence() {
    let machine = guest(common::exit(1, "", "sync: error syncing '/': Input/output error"));
    let err = sync_disk(&machine).await.unwrap_err();
    let answer = "it exited 1 and said: sync: error syncing '/': Input/output error";
    let line = "the disk could not be synced (it exited 1 and said: sync: error syncing '/': Input/output error); nothing was snapshotted";
    assert_eq!(err, SyncDiskError::Sync { answer: answer.into(), line: line.into() });
    assert_eq!(err.to_string(), line);
}

// TS: packages/engine/test/disk-sync.test.ts:110
#[tokio::test]
async fn a_guest_that_synced_and_printed_no_counters_has_synced_no_reading_nothing_unsettled() {
    let machine = guest(printed(""));
    let read = sync_disk(&machine).await.unwrap();
    assert_eq!(read, None);
    assert!(!disk_unsettled(read.as_ref()));
}

// TS: packages/engine/test/disk-sync.test.ts:117
#[tokio::test]
async fn an_exec_the_provider_refused_passes_as_that_refusal_kind_and_status_intact() {
    let gone = EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: "Sandbox not found".into(),
        request_id: None,
        window: None,
        said: None,
    });
    let thrown = gone.clone();
    let machine = FakeMachine::exec("m1", move |_, _| Err(thrown.clone()));
    let err = sync_disk(&machine).await.unwrap_err();
    assert_eq!(err, SyncDiskError::Exec(gone));
}

// TS: packages/engine/test/disk-sync.test.ts:123
#[tokio::test]
async fn the_command_runs_on_this_linux_machines_bash_and_prints_both_counters_in_the_shape_the_reader_parses() {
    if !Path::new("/proc/meminfo").exists() {
        return;
    }
    let out = Command::new("bash").args(["-c", DISK_SYNC_CMD]).output().unwrap();
    let machine = guest(printed(&String::from_utf8_lossy(&out.stdout)));
    let read = sync_disk(&machine).await.unwrap();
    let read = read.expect("both counters read");
    assert!(read.dirty_kb.is_finite() && read.writeback_kb.is_finite());
}

fn close(a: Option<f64>, b: f64) {
    let a = a.expect("a reading");
    assert!((a - b).abs() < 5e-11, "{a} is not close to {b}");
}

// TS: packages/engine/test/disk-sync.test.ts:132
#[test]
fn is_the_share_used_unrounded_off_dfs_used_and_free_counts_and_nothing_for_a_reading_that_is_not_two_counts() {
    close(disk_use_pct("896000 104000\n"), 89.6);
    assert_eq!(disk_use_pct("900000 100000\n"), Some(90.0));
    close(disk_use_pct("901000 99000\n"), 90.1);
    for odd in ["", "0 0", "12", "a b"] {
        assert_eq!(disk_use_pct(odd), None, "{odd:?}");
    }
}

// TS: packages/engine/test/disk-sync.test.ts:139
#[test]
fn reads_this_computers_own_root_the_way_the_guests_is_read() {
    let out = Command::new("sh").args(["-c", DISK_USE_CMD]).output().unwrap();
    let pct = disk_use_pct(&String::from_utf8_lossy(&out.stdout)).expect("a reading of this computer's root");
    assert!(pct > 0.0);
    assert!(pct < 100.0);
}

// ---- preview

/// A route that outlives every test here by an hour, as a provider's token would.
const FRESH_MS: i64 = 60 * 60_000;

fn reach_at(expires_at: i64, tag: &str) -> PreviewReach {
    PreviewReach {
        url: format!("https://{tag}-7070.preview.getsolari.com?pt_token=t"),
        token: "t".into(),
        expires_at: JsNumber(expires_at as f64),
    }
}

// TS: packages/engine/test/preview.test.ts:29
#[tokio::test]
async fn keeps_a_fresh_reach_without_minting() {
    let now = now_ms();
    let mut machine = FakeMachine::new("m1");
    machine.preview_fn = Some(Arc::new(move |_| Ok(reach_at(now + FRESH_MS, "new"))));
    let current = reach_at(now + 30 * 60_000, "a");
    assert_eq!(refresh_preview_token(&machine, 7070, Some(&current), now).await.unwrap(), current);
    assert_eq!(machine.previews.lock().unwrap().clone(), Vec::<u16>::new());
}

// TS: packages/engine/test/preview.test.ts:37
#[tokio::test]
async fn remints_when_under_10_minutes_remain_older_than_about_50_min() {
    let now = now_ms();
    let fresh = reach_at(now + FRESH_MS, "new");
    let minted = fresh.clone();
    let mut machine = FakeMachine::new("m1");
    machine.preview_fn = Some(Arc::new(move |_| Ok(minted.clone())));
    let stale = reach_at(now + 5 * 60_000, "a");
    assert_eq!(refresh_preview_token(&machine, 7070, Some(&stale), now).await.unwrap(), fresh);
    assert_eq!(machine.previews.lock().unwrap().clone(), vec![7070u16]);
}

// TS: packages/engine/test/preview.test.ts:46
#[tokio::test]
async fn refuses_on_a_backend_without_preview_support() {
    let machine = FakeMachine::new("m1");
    let err = refresh_preview_token(&machine, 7070, None, now_ms()).await.unwrap_err();
    assert!(err.to_string().to_lowercase().contains("preview"), "{err}");
    assert_eq!(machine.previews.lock().unwrap().clone(), Vec::<u16>::new());
}
