// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/runtime/test/daemon-token.test.ts (6 cases) and host-id.test.ts (7 cases), then pins the rest of
//! src/daemon_roots.rs, src/daemon_token.rs, src/host_id.rs and src/github_cache.rs against the TS's own answers in
//! tests/fixtures/core_rest/daemon_files.json.
mod rest;

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use rest::*;
use serde_json::{json, Value};
use wsp_core::console;
use wsp_core::daemon_roots::write_daemon_roots_script;
use wsp_core::daemon_token::{
    assert_token_shape, daemon_token_for, daemon_token_path_of, rotate_daemon_token, rotate_daemon_token_script, write_daemon_token_script,
    DAEMON_TOKEN_NONE, DAEMON_TOKEN_SET, TOKEN_NOT_HEX,
};
use wsp_core::github_cache::{memory_github_cache, GitHubCacheEntry};
use wsp_core::host_id::{host_identity, hostname, local_config_dir, template_host};
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::wire::daemon_version::DAEMON_ROOTS_PATH;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";

fn daemon_files() -> Value {
    fixture("daemon_files")
}

fn rows(v: &Value, key: &str) -> Vec<Value> {
    v[key].as_array().unwrap_or_else(|| panic!("{key} is not a list")).clone()
}

fn as_result(out: &Value) -> Result<Option<String>, String> {
    match (out.get("ok"), out.get("err")) {
        (Some(ok), None) => Ok(ok.as_str().map(str::to_string)),
        (None, Some(err)) => Err(err.as_str().unwrap().to_string()),
        _ => panic!("{out} is neither ok nor err"),
    }
}

fn opt_path(v: &Value) -> Option<&str> {
    v.as_str()
}

fn machine_named(path: Option<&str>) -> RecMachine {
    let mut m = RecMachine::plain(vec![]);
    m.id = "m1".into();
    m.daemon_token_path = path.map(str::to_string);
    m
}

fn entry(body: Value, tag: Option<&str>, fetched_at: i64) -> GitHubCacheEntry {
    GitHubCacheEntry { body, tag: tag.map(str::to_string), fetched_at }
}

// TS: daemon-token.test.ts:12
#[test]
fn takes_the_machines_own_file_where_it_names_one_whatever_road_asked_and_the_roads_otherwise() {
    let stand_in = "/tmp/stand-in/m1/.wsp-daemon-token";
    let named = machine_named(Some(stand_in));
    assert_eq!(daemon_token_path_of(&named, Some(DAEMON_TOKEN_PATH)).as_deref(), Some(stand_in));
    assert_eq!(daemon_token_path_of(&named, None).as_deref(), Some(stand_in));
    let unnamed = machine_named(None);
    assert_eq!(daemon_token_path_of(&unnamed, Some(DAEMON_TOKEN_PATH)).as_deref(), Some(DAEMON_TOKEN_PATH));
    assert_eq!(daemon_token_path_of(&unnamed, Some("/home/dev/.wsp/daemon-token")).as_deref(), Some("/home/dev/.wsp/daemon-token"));
    assert_eq!(daemon_token_path_of(&unnamed, None), None);
}

// TS: daemon-token.test.ts:28
#[test]
fn a_machines_own_token_is_the_seeds_one_answer_for_that_machine() {
    let m1 = daemon_token_for(TOKEN, "m1").unwrap();
    assert_eq!(m1, daemon_token_for(TOKEN, "m1").unwrap());
    assert_ne!(m1, daemon_token_for(TOKEN, "m2").unwrap());
    assert_ne!(m1, daemon_token_for(&"beefcafe".repeat(3), "m1").unwrap());
}

// TS: daemon-token.test.ts:36
#[test]
fn a_machines_own_token_is_hex_and_a_seed_that_is_not_hex_is_refused_where_it_is_given() {
    let token = daemon_token_for(TOKEN, "m1").unwrap();
    assert_eq!(token.len(), 64);
    assert!(token.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')), "{token} is not lowercase hex");
    let refused = daemon_token_for("it's not hex", "m1").unwrap_err();
    assert!(refused.message.contains("hex"), "{}", refused.message);
}

// TS: daemon-token.test.ts:43
#[test]
fn the_write_script_holds_the_token_only_in_its_assignment_owner_readable_and_replaced_whole() {
    let script = write_daemon_token_script(TOKEN, None).unwrap();
    let lines: Vec<&str> = script.split('\n').collect();
    assert_eq!(lines[0], format!("WSP_DAEMON_TOKEN='{TOKEN}'"));
    assert!(!lines[1..].join("\n").contains(TOKEN));
    let umask = lines.iter().position(|l| *l == "umask 077").expect("a umask 077 line");
    let printf = lines.iter().position(|l| l.starts_with("printf")).expect("a printf line");
    assert!(umask < printf, "umask at {umask}, printf at {printf}");
    assert!(script.contains(&format!("> {DAEMON_TOKEN_PATH}.next")), "{script}");
    assert!(script.contains(&format!("mv -f {DAEMON_TOKEN_PATH}.next {DAEMON_TOKEN_PATH}")), "{script}");
}

// TS: daemon-token.test.ts:53
#[test]
fn rotate_leaves_a_machine_without_a_daemon_alone_and_names_each_outcome_on_stdout() {
    let script = rotate_daemon_token_script(TOKEN, None).unwrap();
    let head = format!("test -f {DAEMON_TOKEN_PATH} || {{ echo {DAEMON_TOKEN_NONE}; exit 0; }}\n");
    assert!(script.starts_with(&head), "{script}");
    assert!(script.contains(&write_daemon_token_script(TOKEN, None).unwrap()), "{script}");
    assert!(script.ends_with(&format!("\necho {DAEMON_TOKEN_SET}")), "{script}");
}

// TS: daemon-token.test.ts:60
#[test]
fn the_scripts_refuse_anything_but_hex_so_the_quoting_is_always_enough() {
    for bad in ["", "it's-not-hex-at-all-000"] {
        let e = assert_token_shape(bad).unwrap_err();
        assert!(e.message.contains("hex"), "{bad:?}: {}", e.message);
    }
    let e = write_daemon_token_script("'; rm -rf /; echo '0123456789abcdef", None).unwrap_err();
    assert!(e.message.contains("hex"), "{}", e.message);
    assert_eq!(assert_token_shape(TOKEN), Ok(()));
}

// TS: host-id.test.ts:12
#[test]
fn template_host_is_the_per_install_hex_id_alone_never_the_hostname() {
    assert_eq!(template_host("zingzys-MacBook-Pro.local:9f3a1c2b"), "9f3a1c2b");
    assert_eq!(template_host("dev.box:00ff00ff"), "00ff00ff");
}

// TS: host-id.test.ts:17
#[test]
fn template_host_reduces_an_identity_that_fell_back_to_the_bare_hostname_to_the_same_character_class() {
    let reduced = template_host("zingzys-MacBook-Pro.local");
    assert_eq!(reduced, "zingzysmacbookprolocal");
    assert!(reduced.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()), "{reduced}");
}

// TS: host-id.test.ts:29
#[test]
fn host_identity_is_the_hostname_plus_a_per_install_id_made_once_in_the_given_dir_and_read_back_after() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("wsp");
    let first = host_identity(Some(&dir));
    let prefix = format!("{}:", hostname());
    assert!(first.starts_with(&prefix), "{first}");
    let id = &first[prefix.len()..];
    assert!(id.len() == 8 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')), "{id:?} is not eight lowercase hex digits");
    assert!(dir.join("host-id").exists());
    assert_eq!(host_identity(Some(&dir)), first);
    assert_eq!(std::fs::read_to_string(dir.join("host-id")).unwrap().trim(), id);
}

// TS: host-id.test.ts:41
#[test]
fn a_second_read_writes_nothing_over_the_id_it_finds_the_same_bytes_and_the_same_mtime() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("wsp");
    let path = dir.join("host-id");
    let first = host_identity(Some(&dir));
    let made = std::fs::read_to_string(&path).unwrap();
    assert!(
        made.len() == 9 && made.ends_with('\n') && made[..8].bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "{made:?} is not eight hex digits and a newline"
    );
    let before = std::fs::metadata(&path).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(host_identity(Some(&dir)), first);
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), made);
    assert_eq!((after.len(), after.modified().unwrap()), (before.len(), before.modified().unwrap()));
}

// TS: host-id.test.ts:62
#[test]
fn the_id_and_the_folder_it_sits_in_are_this_users_alone_whatever_the_umask() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("wsp");
    let was = unsafe { libc::umask(0o022) };
    host_identity(Some(&dir));
    unsafe { libc::umask(was) };
    assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(dir.join("host-id")).unwrap().permissions().mode() & 0o777, 0o600);
}

// TS: host-id.test.ts:71
#[test]
fn xdg_config_home_names_the_default_dir_so_no_test_reaches_the_developers_real_config_dir() {
    let xdg = tempfile::tempdir().unwrap();
    let was = std::env::var_os("XDG_CONFIG_HOME");
    std::env::set_var("XDG_CONFIG_HOME", xdg.path());
    let set = local_config_dir();
    std::env::set_var("XDG_CONFIG_HOME", "");
    let empty = local_config_dir();
    match was {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }
    assert_eq!(set, xdg.path().join("wsp"));
    assert!(empty.starts_with(std::env::var_os("HOME").unwrap_or_default()), "{} is not under the home dir", empty.display());
}

// TS: host-id.test.ts:78
#[test]
fn host_identity_falls_back_to_the_hostname_alone_when_the_dir_cannot_be_made_and_says_so_once() {
    let parent = tempfile::tempdir().unwrap();
    let blocker = parent.path().join("not-a-dir");
    std::fs::write(&blocker, "a file stands where the parent should be").unwrap();
    let dir = blocker.join("wsp");
    let warned = console::capture();
    assert_eq!(host_identity(Some(&dir)), hostname());
    assert_eq!(host_identity(Some(&dir)), hostname());
    let lines = warned.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let shape = regex::Regex::new(r"^host id not kept in .*; holds carry the hostname alone$").unwrap();
    assert!(shape.is_match(&lines[0]), "{}", lines[0]);
    assert!(!dir.exists());
}

// fixture: daemon_files.json rootsScript
#[test]
fn the_roots_script_is_the_ts_script_for_every_row() {
    let files = daemon_files();
    let all = rows(&files, "rootsScript");
    assert_eq!(all.len(), 6);
    for row in all {
        let roots: Vec<String> = wire(&row["roots"]);
        let got = write_daemon_roots_script(&roots, opt_path(&row["path"]));
        assert_eq!(got, row["script"].as_str().unwrap(), "rootsScript row roots {roots:?} path {}", row["path"]);
    }
}

// fixture: daemon_files.json tokenFor
#[test]
fn the_token_for_a_seed_and_a_machine_is_the_ts_answer_for_every_row() {
    let files = daemon_files();
    for row in rows(&files, "tokenFor") {
        let (seed, machine) = (row["seed"].as_str().unwrap(), row["machine"].as_str().unwrap());
        let got = daemon_token_for(seed, machine).map_err(|e| e.message);
        let want = as_result(&row["out"]).map(|ok| ok.unwrap());
        assert_eq!(got, want, "tokenFor row seed {seed:?} machine {machine:?}");
    }
}

// fixture: daemon_files.json shape
#[test]
fn the_token_shape_check_is_the_ts_answer_for_every_row() {
    let files = daemon_files();
    let all = rows(&files, "shape");
    assert_eq!(all.len(), 8);
    for row in all {
        let token = row["token"].as_str().unwrap();
        let got = assert_token_shape(token).map_err(|e| e.message);
        let want = as_result(&row["out"]).map(|_| ());
        assert_eq!(got, want, "shape row {token:?}");
    }
}

// fixture: daemon_files.json writeScript
#[test]
fn the_token_write_script_is_the_ts_script_for_every_row() {
    let files = daemon_files();
    for row in rows(&files, "writeScript") {
        let token = row["token"].as_str().unwrap();
        let got = write_daemon_token_script(token, opt_path(&row["path"])).map_err(|e| e.message);
        let want = as_result(&row["out"]).map(|ok| ok.unwrap());
        assert_eq!(got, want, "writeScript row token {token:?} path {}", row["path"]);
    }
}

// fixture: daemon_files.json rotateScript
#[test]
fn the_token_rotate_script_is_the_ts_script_for_every_row() {
    let files = daemon_files();
    for row in rows(&files, "rotateScript") {
        let token = row["token"].as_str().unwrap();
        let got = rotate_daemon_token_script(token, opt_path(&row["path"])).map_err(|e| e.message);
        let want = as_result(&row["out"]).map(|ok| ok.unwrap());
        assert_eq!(got, want, "rotateScript row token {token:?} path {}", row["path"]);
    }
}

// fixture: daemon_files.json constants
#[test]
fn the_daemon_file_constants_are_the_ts_values() {
    let files = daemon_files();
    let want = &files["constants"];
    assert_eq!(
        json!({ "set": DAEMON_TOKEN_SET, "none": DAEMON_TOKEN_NONE, "path": DAEMON_TOKEN_PATH, "rootsPath": DAEMON_ROOTS_PATH }),
        *want
    );
    assert_eq!(TOKEN_NOT_HEX, "a daemon token is lowercase hex");
}

// fixture: daemon_files.json templateHost
#[test]
fn template_host_is_the_ts_answer_for_every_pair() {
    let files = daemon_files();
    let pairs = rows(&files, "templateHost");
    assert_eq!(pairs.len(), 9);
    for pair in pairs {
        let (host_id, want) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
        assert_eq!(template_host(host_id), want, "templateHost pair {host_id:?}");
    }
}

// TS: daemon-token.ts:59 (rotateDaemonToken, no byte road)
#[test]
fn rotate_on_a_machine_with_no_byte_road_runs_the_one_script_and_answers_true_where_the_daemon_was_there() {
    run_local(async {
        for path in [None, Some("/home/dev/.wsp/daemon-token")] {
            let machine = RecMachine::new(vec![("test -f", result(0, &format!("{DAEMON_TOKEN_SET}\n"), ""))]);
            let rotated = rotate_daemon_token(machine.as_ref(), TOKEN, path).await.unwrap();
            assert!(rotated, "path {path:?}");
            assert_eq!(machine.cmds(), vec![rotate_daemon_token_script(TOKEN, path).unwrap()], "path {path:?}");
        }
    });
}

// TS: daemon-token.ts:59 (rotateDaemonToken, no byte road)
#[test]
fn rotate_on_a_machine_with_no_byte_road_answers_false_where_there_was_no_daemon_or_the_script_failed() {
    run_local(async {
        let none = RecMachine::new(vec![("test -f", result(0, &format!("{DAEMON_TOKEN_NONE}\n"), ""))]);
        assert!(!rotate_daemon_token(none.as_ref(), TOKEN, None).await.unwrap());
        assert_eq!(none.cmds(), vec![rotate_daemon_token_script(TOKEN, None).unwrap()]);

        let failed = RecMachine::new(vec![("test -f", result(1, &format!("{DAEMON_TOKEN_SET}\n"), "mv: no space"))]);
        assert!(!rotate_daemon_token(failed.as_ref(), TOKEN, None).await.unwrap());
        assert_eq!(failed.cmds(), vec![rotate_daemon_token_script(TOKEN, None).unwrap()]);
    });
}

// TS: daemon-token.ts:59 (rotateDaemonToken, assertTokenShape first)
#[test]
fn rotate_refuses_a_token_that_is_not_hex_before_anything_runs() {
    run_local(async {
        let machine = RecMachine::new(vec![("test -f", result(0, DAEMON_TOKEN_SET, ""))]);
        let refused = rotate_daemon_token(machine.as_ref(), "'; rm -rf /; echo '", None).await.unwrap_err();
        assert_eq!(refused.message, TOKEN_NOT_HEX);
        assert_eq!(machine.cmds(), Vec::<String>::new());
    });
}

// TS: github-cache.ts:18
#[test]
fn the_memory_github_cache_answers_nothing_for_a_key_it_never_held() {
    let cache = memory_github_cache();
    assert_eq!(cache.get("repos/acme/web"), None);
}

// TS: github-cache.ts:18
#[test]
fn the_memory_github_cache_answers_the_entry_it_was_given_and_a_second_set_replaces_it() {
    let cache = memory_github_cache();
    let first = entry(json!({ "stars": 3 }), Some("W/\"abc\""), 1_700_000_000_000);
    cache.set("repos/acme/web", first.clone());
    let got = cache.get("repos/acme/web").expect("the entry was kept");
    assert_eq!(got.body, json!({ "stars": 3 }));
    assert_eq!(got.tag.as_deref(), Some("W/\"abc\""));
    assert_eq!(got.fetched_at, 1_700_000_000_000);
    assert_eq!(got, first);

    let second = entry(json!({ "stars": 4 }), None, 1_700_000_060_000);
    cache.set("repos/acme/web", second.clone());
    assert_eq!(cache.get("repos/acme/web"), Some(second));
}

// TS: github-cache.ts:18
#[test]
fn the_memory_github_cache_delete_removes_one_key_and_leaves_another_and_an_absent_key_does_nothing() {
    let cache = memory_github_cache();
    let web = entry(json!([1, 2]), Some("a"), 10);
    let api = entry(json!({ "name": "api" }), Some("b"), 20);
    cache.set("web", web);
    cache.set("api", api.clone());
    cache.delete("web");
    assert_eq!(cache.get("web"), None);
    assert_eq!(cache.get("api"), Some(api.clone()));
    cache.delete("never-held");
    assert_eq!(cache.get("api"), Some(api));
    assert_eq!(cache.get("never-held"), None);
}
