// SPDX-License-Identifier: AGPL-3.0-only
//! Review of what reads the person's real files: wsp-collect's live host and readers against the TS's answers.
//! Every expected value below is what node 22.23 printed for the same input over the same kind of tree.
use std::ffi::{CStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::sync::Mutex;

use wsp_collect::history::claude::ClaudeReader;
use wsp_collect::history::reader::HistoryReader;
use wsp_collect::host::{Host, HostExec, HostFs, Platform, StatKind};
use wsp_collect::live_host::{node_host, NodeExec, NodeFs};

/// Tests that change HOME or PATH hold this, and every test that reads PATH takes it too.
static ENV: Mutex<()> = Mutex::new(());

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}

fn host_of(home: &str) -> Host {
    Host {
        platform: Platform::Linux,
        home: home.to_string(),
        shell: None,
        terminal: None,
        xdg_config_home: None,
        claude_config_dir: None,
        codex_home: None,
        stores: None,
        seeds: None,
        fs: Rc::new(NodeFs),
        exec: Rc::new(NodeExec),
    }
}

fn passwd_home() -> String {
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        CStr::from_ptr((*pw).pw_dir).to_string_lossy().into_owned()
    }
}

fn write_exe(path: &str) {
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A folder holding a file named `ok`, one named with bytes that are not UTF-8, and a folder named so, with a file in it.
fn undecodable_names(dir: &std::path::Path) {
    fs::write(dir.join("ok"), "ok").unwrap();
    let bad = |bytes: &[u8]| std::path::PathBuf::from(OsString::from_vec(bytes.to_vec()));
    fs::write(dir.join(bad(b"\xff\xfeabc")), "bad1").unwrap();
    fs::create_dir(dir.join(bad(b"d\xc3"))).unwrap();
    fs::write(dir.join(bad(b"d\xc3")).join("inner"), "inner12345").unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn a_folder_holding_names_node_cannot_decode_is_sized_without_them() {
    let dir = tempfile::tempdir().unwrap();
    undecodable_names(dir.path());
    let stat = NodeFs.stat(dir.path().to_str().unwrap()).await.unwrap();
    assert_eq!((stat.kind == StatKind::Dir, stat.bytes), (true, 2), "node stat'ed the lossy names, found nothing, and counted only ok");
}

#[test]
fn home_unset_is_the_password_entry_as_node_answers() {
    let _g = env_lock();
    let saved = std::env::var_os("HOME");
    std::env::remove_var("HOME");
    let host = node_host().unwrap_or_else(|_| panic!("node_host"));
    if let Some(home) = saved {
        std::env::set_var("HOME", home);
    }
    assert_eq!(host.home, passwd_home(), "node's os.homedir() printed /root with HOME unset");
}

#[test]
fn an_env_value_that_is_not_utf8_is_read_as_node_reads_it() {
    let _g = env_lock();
    let names = ["HOME", "SHELL", "XDG_CONFIG_HOME", "CLAUDE_CONFIG_DIR", "CODEX_HOME"];
    let saved: Vec<_> = names.iter().map(|n| std::env::var_os(n)).collect();
    std::env::set_var("HOME", OsString::from_vec(b"/tmp/\xff".to_vec()));
    std::env::set_var("SHELL", OsString::from_vec(b"/bin/\xffsh".to_vec()));
    std::env::set_var("XDG_CONFIG_HOME", OsString::from_vec(b"/x\xc3".to_vec()));
    std::env::set_var("CLAUDE_CONFIG_DIR", OsString::from_vec(b"/cl\xfe".to_vec()));
    std::env::set_var("CODEX_HOME", OsString::from_vec(b"/c\xfe".to_vec()));
    let host = node_host().unwrap_or_else(|_| panic!("node_host"));
    for (name, old) in names.iter().zip(saved) {
        match old {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }
    let got = (host.home, host.shell, host.xdg_config_home, host.claude_config_dir, host.codex_home);
    let want = (
        "/tmp/\u{fffd}".to_string(),
        Some("/bin/\u{fffd}sh".to_string()),
        Some("/x\u{fffd}".to_string()),
        Some("/cl\u{fffd}".to_string()),
        Some("/c\u{fffd}".to_string()),
    );
    assert_eq!(got, want);
}

#[tokio::test(flavor = "current_thread")]
async fn which_reads_a_path_with_a_byte_node_decodes_lossily() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().to_str().unwrap().to_string();
    write_exe(&format!("{bin}/exe"));
    let saved = std::env::var_os("PATH");
    let mut path = b"\xff:".to_vec();
    path.extend_from_slice(bin.as_bytes());
    std::env::set_var("PATH", OsString::from_vec(path));
    let found = NodeExec.which("exe").await;
    match saved {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }
    assert!(found, "node printed true: the entry after the undecodable one still holds exe");
}

#[tokio::test(flavor = "current_thread")]
async fn which_joins_the_name_as_path_join_does() {
    let _g = env_lock();
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().to_str().unwrap().to_string();
    write_exe(&format!("{bin}/exe"));
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);
    let trailing = NodeExec.which("exe/").await;
    std::env::set_var("PATH", "x/..");
    let empty = NodeExec.which("").await;
    match saved {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }
    assert_eq!(
        (trailing, empty),
        (false, true),
        "node: join(dir, 'exe/') keeps its slash, so access fails on a file; join('x/..', '') is '.'"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_one_byte_past_the_longest_string_is_not_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big");
    fs::File::create(&path).unwrap().set_len(536_870_889).unwrap();
    let text = NodeFs.read_text(path.to_str().unwrap()).await;
    assert_eq!(
        text.map(|t| t.len()),
        None,
        "node: 536870888 bytes read, 536870889 is 'Cannot create a string longer than 0x1fffffe8 characters'"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn claude_session_files_under_a_root_with_a_double_slash_are_cut_by_utf16_units() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap();
    fs::create_dir_all(format!("{base}/projects/\u{e9}\u{e9}")).unwrap();
    fs::write(format!("{base}/projects/\u{e9}\u{e9}/s.jsonl"), "{}\n").unwrap();
    let host = host_of(base);
    let root = format!("{base}//projects");
    let files = ClaudeReader.files(&host, &root).await;
    let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let holders: Vec<String> = files.iter().map(|f| ClaudeReader.holder(&root, &f.path)).collect();
    assert_eq!((paths, holders), (vec![format!("{base}/projects/\u{e9}\u{e9}/s.jsonl")], vec!["\u{e9}".to_string()]));
}

#[tokio::test(flavor = "current_thread")]
async fn a_history_file_stamp_prints_the_modified_time_as_javascript_does_on_an_exact_tie() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let file = fs::File::create(&path).unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::new(2_147_483_648, 959_156_250)).unwrap();
    drop(file);
    let host = host_of(dir.path().to_str().unwrap());
    let stamped = wsp_collect::history::reader::stamp_of(&host, path.to_str().unwrap()).await.unwrap();
    assert_eq!(
        stamped.stamp, "2147483648959.1562:0",
        "mtimeMs is 2147483648959.15625, halfway between two 17-digit readings; String() takes the even one"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_read_stuck_on_a_fifo_leaves_the_callers_timer_free_to_fire() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("p");
    let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) }, 0);
    let unblock = fifo.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let _ = fs::OpenOptions::new().write(true).open(unblock);
    });
    let started = std::time::Instant::now();
    let raced = tokio::time::timeout(std::time::Duration::from_millis(100), NodeFs.read_text(fifo.to_str().unwrap())).await;
    let waited = started.elapsed();
    writer.join().unwrap();
    assert!(
        raced.is_err() && waited < std::time::Duration::from_secs(1),
        "node: Promise.race with a 100 ms timer answered 'timeout' at 103 ms; here the call held the thread for {waited:?}"
    );
}
