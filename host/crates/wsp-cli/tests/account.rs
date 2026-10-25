// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of `account`, `serving_home`, `repo_root`, `version`, `cloud` and `daemon_fix`. The two serving-home cases port
//! `packages/host/test/serving-home.test.ts:73-118`; the rest pin the TS source, which has no test of its own.
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};
use tempfile::TempDir;
use wsp_cli::account::*;
use wsp_cli::cloud::{cloud_on, cloud_on_in};
use wsp_cli::daemon_fix::{daemon_fix_line, release_line_of, release_update_line, road_of, Road};
use wsp_cli::host_lock::serving_host;
use wsp_cli::hosts::Env;
use wsp_cli::repo_root::{git_root_of, main_worktree_of};
use wsp_cli::serving_home::*;
use wsp_cli::version::{installed_version, state_writer_here, VERSION};
use wsp_store::keys::{key_fingerprint, PlaceKeyPair};
use wsp_store::store::StateWriter;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;

fn tmp() -> TempDir {
    tempfile::Builder::new().prefix("wsp-cli-account-").tempdir().unwrap()
}

fn real(dir: &TempDir) -> String {
    fs::canonicalize(dir.path()).unwrap().to_string_lossy().into_owned()
}

fn at(base: &str, rel: &str) -> String {
    format!("{base}/{rel}")
}

fn put(path: &str, text: &str) {
    fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn mode(path: &str) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn umask_022() {
    unsafe {
        libc::umask(0o022);
    }
}

fn names(dir: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    found.sort();
    found
}

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn relay_record() -> RelayRecord {
    RelayRecord {
        relay_url: "https://relay.example".into(),
        host_id: "host-1".into(),
        token: "tok-1".into(),
        name: Some(json!("box")),
        login: Some(json!("zingzy")),
        hostname: Some(json!("box.example.dev")),
        device_key: Some(json!({"fingerprint": "SHA256:abc", "publicKey": "cHVi"})),
        linked_at: Some(json!("2026-10-25T10:00:00.000Z")),
    }
}

fn client_record() -> RelayClientRecord {
    RelayClientRecord {
        relay_url: "https://relay.example".into(),
        token: "tok-2".into(),
        name: Some(json!("laptop")),
        login: Some(json!("zingzy")),
        fingerprint: Some(json!("SHA256:def")),
        linked_at: Some(json!("2026-10-25T11:00:00.000Z")),
    }
}

const BAD_FILES: [&str; 7] = ["", "not json", "null", "[]", "42", "\"text\"", "{}"];

// account.ts:76-78

#[test]
fn write_json_file_writes_two_space_json_and_a_newline_and_leaves_no_temp_file() {
    // account.ts:76-78: `${JSON.stringify(value, null, 2)}\n`
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "out");
    let path = at(&home, "a.json");
    write_json_file(&path, &json!({"a": [1, 2], "b": {"c": null}, "e": [], "o": {}, "u": "é"})).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": null\n  },\n  \"e\": [],\n  \"o\": {},\n  \"u\": \"é\"\n}\n"
    );
    assert_eq!(names(&home), vec!["a.json".to_string()]);
}

#[test]
fn write_json_file_lands_at_0600_in_a_0700_folder_it_makes() {
    // account.ts:76-78 through writeOwn: the owner's file under the owner's folder, whatever the umask
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "made");
    let path = at(&home, "a.json");
    write_json_file(&path, &json!({"k": 1})).unwrap();
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&home), 0o700);
}

#[test]
fn write_json_file_repairs_a_wider_folder_and_replaces_a_wider_file_at_0600() {
    // account.ts:76-78 through writeOwn: a folder at 0755 goes to 0700, a file at 0644 is replaced by one at 0600
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "wide");
    let path = at(&home, "a.json");
    put(&path, "old");
    fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    write_json_file(&path, &json!({"k": 2})).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "{\n  \"k\": 2\n}\n");
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&home), 0o700);
}

// account.ts:80-94

#[test]
fn relay_record_path_is_relay_json_beside_the_state_file() {
    // account.ts:80-82: join(dirname(statePath), "relay.json")
    assert_eq!(relay_record_path("/h/.wsp/state.json"), "/h/.wsp/relay.json");
    assert_eq!(relay_record_path("/state.json"), "/relay.json");
    assert_eq!(relay_record_path("rel/state.json"), "rel/relay.json");
    assert_eq!(relay_record_path("state.json"), "relay.json");
}

#[test]
fn relay_record_round_trips_and_lands_as_the_struct_writes_it() {
    // account.ts:84-90: the record written is the record read, as indented JSON with the writer's key order
    umask_022();
    let dir = tmp();
    let state = at(&at(&real(&dir), "box"), "state.json");
    assert_eq!(read_relay_record(&state), None);
    write_relay_record(&state, &relay_record()).unwrap();
    let path = relay_record_path(&state);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "{\n  \"relayUrl\": \"https://relay.example\",\n  \"hostId\": \"host-1\",\n  \"token\": \"tok-1\",\n  \"name\": \"box\",\n  \"login\": \"zingzy\",\n  \"hostname\": \"box.example.dev\",\n  \"deviceKey\": {\n    \"fingerprint\": \"SHA256:abc\",\n    \"publicKey\": \"cHVi\"\n  },\n  \"linkedAt\": \"2026-10-25T10:00:00.000Z\"\n}\n"
    );
    assert_eq!(read_relay_record(&state), Some(relay_record()));
}

#[test]
fn relay_record_lands_at_0600_in_a_0700_folder() {
    // account.ts:88-90: the token opens the relay, so it goes through the owner's writer
    umask_022();
    let dir = tmp();
    let folder = at(&real(&dir), "fresh");
    let state = at(&folder, "state.json");
    write_relay_record(&state, &relay_record()).unwrap();
    assert_eq!(mode(&relay_record_path(&state)), 0o600);
    assert_eq!(mode(&folder), 0o700);
}

#[test]
fn relay_record_reads_with_only_url_host_id_and_token_and_keeps_the_other_fields_as_held() {
    // account.ts:56-57 checks three strings and hands the object on unchecked: a name of 5 still reads
    let dir = tmp();
    let base = real(&dir);
    let state = at(&base, "state.json");
    put(&relay_record_path(&state), r#"{"relayUrl":"u","hostId":"h","token":"t"}"#);
    assert_eq!(
        read_relay_record(&state),
        Some(RelayRecord {
            relay_url: "u".into(),
            host_id: "h".into(),
            token: "t".into(),
            name: None,
            login: None,
            hostname: None,
            device_key: None,
            linked_at: None
        })
    );
    put(&relay_record_path(&state), r#"{"relayUrl":"u","hostId":"h","token":"t","name":5,"login":null,"deviceKey":{"x":1}}"#);
    let read = read_relay_record(&state).unwrap();
    assert_eq!(read.name, Some(json!(5)));
    assert_eq!(read.device_key, Some(json!({"x": 1})));
}

#[test]
fn relay_record_missing_token_host_id_or_url_or_holding_a_non_string_reads_as_none() {
    // account.ts:56-57
    let dir = tmp();
    let state = at(&real(&dir), "state.json");
    for text in [
        r#"{"relayUrl":"u","hostId":"h"}"#,
        r#"{"relayUrl":"u","token":"t"}"#,
        r#"{"hostId":"h","token":"t"}"#,
        r#"{"relayUrl":"u","hostId":"h","token":7}"#,
        r#"{"relayUrl":1,"hostId":"h","token":"t"}"#,
        r#"{"relayUrl":"u","hostId":null,"token":"t"}"#,
    ] {
        put(&relay_record_path(&state), text);
        assert_eq!(read_relay_record(&state), None, "{text}");
    }
}

#[test]
fn relay_record_that_is_not_an_object_of_the_shape_reads_as_none() {
    // account.ts:67-73: readJsonFile answers nothing for an unreadable, unparsable or wrong-shaped file
    let dir = tmp();
    let state = at(&real(&dir), "state.json");
    for text in BAD_FILES {
        put(&relay_record_path(&state), text);
        assert_eq!(read_relay_record(&state), None, "{text:?}");
    }
}

#[test]
fn remove_relay_record_takes_the_file_away_and_a_missing_one_is_no_error() {
    // account.ts:92-94: rmSync with force
    let dir = tmp();
    let base = real(&dir);
    let state = at(&base, "state.json");
    remove_relay_record(&state).unwrap();
    write_relay_record(&state, &relay_record()).unwrap();
    put(&at(&base, "keep.json"), "{}");
    remove_relay_record(&state).unwrap();
    assert_eq!(read_relay_record(&state), None);
    assert_eq!(names(&base), vec!["keep.json".to_string()]);
    remove_relay_record(&state).unwrap();
}

// account.ts:98-112

#[test]
fn client_record_path_is_relay_client_json_in_the_home() {
    // account.ts:98-100: join(home, "relay-client.json"), which is not the host's relay.json
    assert_eq!(client_record_path("/h/.wsp"), "/h/.wsp/relay-client.json");
    assert_eq!(client_record_path("/h/.wsp/"), "/h/.wsp/relay-client.json");
    assert_eq!(client_record_path("rel"), "rel/relay-client.json");
}

#[test]
fn relay_client_round_trips_and_lands_as_the_struct_writes_it() {
    // account.ts:102-108
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "home");
    assert_eq!(read_relay_client(&home), None);
    write_relay_client(&home, &client_record()).unwrap();
    assert_eq!(
        fs::read_to_string(client_record_path(&home)).unwrap(),
        "{\n  \"relayUrl\": \"https://relay.example\",\n  \"token\": \"tok-2\",\n  \"name\": \"laptop\",\n  \"login\": \"zingzy\",\n  \"fingerprint\": \"SHA256:def\",\n  \"linkedAt\": \"2026-10-25T11:00:00.000Z\"\n}\n"
    );
    assert_eq!(read_relay_client(&home), Some(client_record()));
}

#[test]
fn relay_client_lands_at_0600_in_a_0700_home() {
    // account.ts:106-108: the token lists the account's hosts, so it goes through the owner's writer
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "home");
    write_relay_client(&home, &client_record()).unwrap();
    assert_eq!(mode(&client_record_path(&home)), 0o600);
    assert_eq!(mode(&home), 0o700);
}

#[test]
fn relay_client_with_a_host_id_key_even_null_reads_as_none() {
    // account.ts:60: `hostId === undefined`, so a host's record is no client record and null is not undefined
    let dir = tmp();
    let home = real(&dir);
    for text in [
        r#"{"relayUrl":"u","token":"t","hostId":"h"}"#,
        r#"{"relayUrl":"u","token":"t","hostId":null}"#,
        r#"{"relayUrl":"u","token":"t","hostId":""}"#,
    ] {
        put(&client_record_path(&home), text);
        assert_eq!(read_relay_client(&home), None, "{text}");
    }
    put(&client_record_path(&home), r#"{"relayUrl":"u","token":"t"}"#);
    assert_eq!(
        read_relay_client(&home),
        Some(RelayClientRecord { relay_url: "u".into(), token: "t".into(), name: None, login: None, fingerprint: None, linked_at: None })
    );
}

#[test]
fn relay_client_missing_token_or_url_or_holding_a_non_string_reads_as_none() {
    // account.ts:58-62
    let dir = tmp();
    let home = real(&dir);
    for text in [r#"{"relayUrl":"u"}"#, r#"{"token":"t"}"#, r#"{"relayUrl":"u","token":null}"#, r#"{"relayUrl":2,"token":"t"}"#] {
        put(&client_record_path(&home), text);
        assert_eq!(read_relay_client(&home), None, "{text}");
    }
    for text in BAD_FILES {
        put(&client_record_path(&home), text);
        assert_eq!(read_relay_client(&home), None, "{text:?}");
    }
}

#[test]
fn remove_relay_client_takes_only_the_client_file_and_a_missing_one_is_no_error() {
    // account.ts:110-112: rmSync with force, and the device key stays when a sign-out takes the token away
    let dir = tmp();
    let home = real(&dir);
    remove_relay_client(&home).unwrap();
    write_relay_client(&home, &client_record()).unwrap();
    device_key_here(&home).unwrap();
    remove_relay_client(&home).unwrap();
    assert_eq!(read_relay_client(&home), None);
    assert_eq!(names(&home), vec!["device-key.json".to_string()]);
    remove_relay_client(&home).unwrap();
}

// account.ts:114-134

#[test]
fn device_key_path_is_device_key_json_in_the_home() {
    // account.ts:114-116
    assert_eq!(device_key_path("/h/.wsp"), "/h/.wsp/device-key.json");
    assert_eq!(device_key_path("/h/.wsp/"), "/h/.wsp/device-key.json");
}

#[test]
fn read_device_key_pair_reads_a_file_holding_both_strings_and_nothing_else() {
    // account.ts:64-65, 118-120: `publicKey` and `privateKeyPem` strings
    let dir = tmp();
    let home = real(&dir);
    put(&device_key_path(&home), r#"{"publicKey":"pub","privateKeyPem":"pem","extra":1}"#);
    assert_eq!(read_device_key_pair(&home), Some(PlaceKeyPair { public_key: "pub".into(), private_key_pem: "pem".into() }));
    for text in [
        r#"{"publicKey":"pub"}"#,
        r#"{"privateKeyPem":"pem"}"#,
        r#"{"publicKey":"pub","privateKeyPem":3}"#,
        r#"{"publicKey":null,"privateKeyPem":"pem"}"#,
    ] {
        put(&device_key_path(&home), text);
        assert_eq!(read_device_key_pair(&home), None, "{text}");
    }
    for text in BAD_FILES {
        put(&device_key_path(&home), text);
        assert_eq!(read_device_key_pair(&home), None, "{text:?}");
    }
    fs::remove_file(device_key_path(&home)).unwrap();
    assert_eq!(read_device_key_pair(&home), None);
}

#[test]
fn device_key_here_mints_once_and_answers_the_same_pair_from_then_on() {
    // account.ts:125-131
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "home");
    let first = device_key_here(&home).unwrap();
    assert!(first.private_key_pem.starts_with("-----BEGIN PRIVATE KEY-----"), "{}", first.private_key_pem);
    assert!(!first.public_key.is_empty());
    let second = device_key_here(&home).unwrap();
    assert_eq!(second, first);
    assert_eq!(read_device_key_pair(&home), Some(first.clone()));
    let third = device_key_here(&home).unwrap();
    assert_eq!(third, first);
    assert_eq!(names(&home), vec!["device-key.json".to_string()]);
}

#[test]
fn device_key_here_writes_the_pair_as_public_key_then_private_key_pem_at_0600_in_a_0700_home() {
    // account.ts:125-131: `{"publicKey", "privateKeyPem"}` through the owner's writer
    umask_022();
    let dir = tmp();
    let home = at(&real(&dir), "home");
    let pair = device_key_here(&home).unwrap();
    let want = format!(
        "{{\n  \"publicKey\": {},\n  \"privateKeyPem\": {}\n}}\n",
        serde_json::to_string(&pair.public_key).unwrap(),
        serde_json::to_string(&pair.private_key_pem).unwrap()
    );
    assert_eq!(fs::read_to_string(device_key_path(&home)).unwrap(), want);
    assert_eq!(mode(&device_key_path(&home)), 0o600);
    assert_eq!(mode(&home), 0o700);
}

#[test]
fn device_key_here_answers_a_held_pair_untouched_and_replaces_one_that_does_not_read() {
    // account.ts:126-127: a held pair is returned as is; a file that is no pair is written over with a fresh one
    umask_022();
    let dir = tmp();
    let home = real(&dir);
    let held = r#"{"publicKey":"held-public","privateKeyPem":"held-pem"}"#;
    put(&device_key_path(&home), held);
    let pair = device_key_here(&home).unwrap();
    assert_eq!(pair, PlaceKeyPair { public_key: "held-public".into(), private_key_pem: "held-pem".into() });
    assert_eq!(fs::read_to_string(device_key_path(&home)).unwrap(), held);

    put(&device_key_path(&home), r#"{"publicKey":"only-public"}"#);
    let fresh = device_key_here(&home).unwrap();
    assert_ne!(fresh.public_key, "only-public");
    assert_eq!(read_device_key_pair(&home), Some(fresh));
}

#[test]
fn device_key_here_mints_a_different_pair_for_each_home() {
    // account.ts:122-124: one key per wsp home
    let dir = tmp();
    let base = real(&dir);
    let a = device_key_here(&at(&base, "a")).unwrap();
    let b = device_key_here(&at(&base, "b")).unwrap();
    assert_ne!(a.public_key, b.public_key);
    assert_ne!(a.private_key_pem, b.private_key_pem);
}

#[test]
fn device_key_of_names_the_public_half_by_its_fingerprint() {
    // account.ts:134
    let pair = PlaceKeyPair { public_key: "MCowBQYDK2VwAyEAabc".into(), private_key_pem: "never read".into() };
    let key = device_key_of(&pair);
    assert_eq!(key, DeviceKey { fingerprint: key_fingerprint("MCowBQYDK2VwAyEAabc"), public_key: "MCowBQYDK2VwAyEAabc".into() });
    assert!(key.fingerprint.starts_with("SHA256:"));
    assert_eq!(serde_json::to_value(&key).unwrap(), json!({"fingerprint": key.fingerprint, "publicKey": "MCowBQYDK2VwAyEAabc"}));
    let minted = wsp_store::keys::new_place_key_pair();
    assert_eq!(device_key_of(&minted).fingerprint, key_fingerprint(&minted.public_key));
}

// serving-home.ts

fn lock_on(home: &str, pid: u32) {
    let lock = json!({"pid": pid, "port": 4400, "address": "127.0.0.1", "startedAt": "2026-09-11T10:00:00.000Z"});
    put(&at(home, "host.lock"), &serde_json::to_string(&lock).unwrap());
}

fn dead_pid() -> u32 {
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    assert!(child.wait().unwrap().success());
    pid
}

#[derive(Debug, PartialEq)]
struct Reading {
    home: String,
    serving: bool,
}

/// serving-home.test.ts:57-62: the sh spelling put the same question on the same folders.
fn in_sh(user: &str, env: &[(&str, &str)]) -> Reading {
    let script = format!("{SERVING_HOME_SH}\nif wsp_serving \"$home\"; then echo serving; else echo none; fi\necho \"$home\"");
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", user)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let mut lines = text.trim().split('\n');
    let serving = lines.next().unwrap() == "serving";
    Reading { home: lines.next().unwrap().to_string(), serving }
}

/// serving-home.test.ts:65-70: both spellings, held to the same answer before the answer is read.
fn reading(user: &str, env: &[(&str, &str)]) -> Reading {
    let home = serving_home(&env_of(env), user);
    let here = Reading { serving: serving_host(&at(&home, "state.json")).is_some(), home };
    assert_eq!(in_sh(user, env), here);
    here
}

struct Computer {
    _dir: TempDir,
    user: String,
    own: String,
    moved: String,
}

/// serving-home.test.ts:29-37.
fn computer() -> Computer {
    let dir = tmp();
    let base = real(&dir);
    let user = at(&base, "user");
    let own = at(&user, ".wsp");
    let moved = at(&base, "moved");
    fs::create_dir_all(&own).unwrap();
    fs::create_dir_all(&moved).unwrap();
    Computer { _dir: dir, user, own, moved }
}

#[test]
fn the_serving_home_is_wsp_home_when_it_names_one_and_this_computers_own_otherwise_in_sh_as_here() {
    // serving-home.test.ts:73-95
    let Computer { user, own, moved, .. } = computer();
    assert_eq!(reading(&user, &[]), Reading { home: own.clone(), serving: false });

    lock_on(&moved, std::process::id());
    assert_eq!(reading(&user, &[]), Reading { home: own.clone(), serving: false });
    assert_eq!(reading(&user, &[("WSP_HOME", &moved)]), Reading { home: moved.clone(), serving: true });

    assert_eq!(reading(&user, &[("WSP_HOME", &own)]), Reading { home: own.clone(), serving: false });

    lock_on(&moved, dead_pid());
    assert_eq!(reading(&user, &[("WSP_HOME", &moved)]), Reading { home: moved.clone(), serving: false });

    lock_on(&own, std::process::id());
    assert_eq!(reading(&user, &[]), Reading { home: own.clone(), serving: true });
}

#[test]
fn the_serving_home_reads_an_empty_wsp_home_as_unset_in_sh_as_here() {
    // serving-home.ts:19 `homeNamed`, and sh's `${WSP_HOME:-$HOME/.wsp}`: unset and empty are one answer
    let Computer { user, own, .. } = computer();
    assert_eq!(reading(&user, &[("WSP_HOME", "")]), Reading { home: own.clone(), serving: false });
    lock_on(&own, std::process::id());
    assert_eq!(reading(&user, &[("WSP_HOME", "")]), Reading { home: own, serving: true });
}

#[test]
fn serving_home_sh_is_two_lines_joined_by_one_newline_with_none_at_the_end() {
    // serving-home.ts:48-51: wsp_serving's definition, then the line that sets $home
    let lines: Vec<&str> = SERVING_HOME_SH.split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("wsp_serving() { [ -f \"$1/host.lock\" ] || return 1; pid=$(sed -n "));
    assert!(lines[0].ends_with("kill -0 \"$pid\" 2>/dev/null; }"));
    assert_eq!(lines[1], "home=\"${WSP_HOME:-$HOME/.wsp}\"");
    let Computer { user, own, .. } = computer();
    assert_eq!(serving_home(&env_of(&[]), &user), own);
}

#[test]
fn serving_home_sh_reads_no_host_from_a_lock_with_no_pid_a_dead_pid_or_no_lock() {
    // serving-home.ts:48-51: `[ -n "$pid" ] && kill -0 "$pid"`, as the TS reading counts a lock of a gone process none
    let Computer { user, own, .. } = computer();
    assert_eq!(in_sh(&user, &[]), Reading { home: own.clone(), serving: false });
    put(&at(&own, "host.lock"), "{\"port\":4400}");
    assert_eq!(in_sh(&user, &[]).serving, false);
    lock_on(&own, dead_pid());
    assert_eq!(in_sh(&user, &[]).serving, false);
    lock_on(&own, std::process::id());
    assert_eq!(reading(&user, &[]).serving, true);
}

#[test]
fn serving_elsewhere_names_the_state_a_host_here_serves_when_a_line_works_on_another_and_nothing_when_they_are_the_same() {
    // serving-home.test.ts:97-118
    let Computer { _dir, user, own, moved } = computer();
    let checkout = tmp();
    let elsewhere = at(&at(&real(&checkout), ".wsp"), "state.json");
    let own_state = at(&own, "state.json");
    let moved_state = at(&moved, "state.json");

    assert_eq!(serving_elsewhere(&elsewhere, &env_of(&[]), &user), None);

    lock_on(&own, std::process::id());
    assert_eq!(serving_elsewhere(&elsewhere, &env_of(&[]), &user), Some(own_state.clone()));
    assert_eq!(serving_elsewhere(&own_state, &env_of(&[]), &user), None);

    lock_on(&moved, std::process::id());
    assert_eq!(serving_elsewhere(&elsewhere, &env_of(&[("WSP_HOME", &moved)]), &user), Some(moved_state));
    assert_eq!(serving_elsewhere(&elsewhere, &env_of(&[]), &user), Some(own_state));

    let link = at(Path::new(&own).parent().unwrap().to_str().unwrap(), ".wsp-link");
    symlink(&own, &link).unwrap();
    assert_eq!(serving_elsewhere(&at(&link, "state.json"), &env_of(&[]), &user), None);
}

#[test]
fn serving_elsewhere_is_none_when_the_host_under_this_computers_home_is_gone() {
    // serving-home.ts:38-41: `servingHost(served) !== undefined`, and a lock of a dead pid is no host
    let Computer { user, own, .. } = computer();
    let checkout = tmp();
    let elsewhere = at(&at(&real(&checkout), ".wsp"), "state.json");
    lock_on(&own, dead_pid());
    assert_eq!(serving_elsewhere(&elsewhere, &env_of(&[]), &user), None);
}

#[test]
fn default_home_in_is_dot_wsp_under_the_given_user_folder() {
    // serving-home.ts:15: join(user, ".wsp")
    assert_eq!(default_home_in("/home/z"), "/home/z/.wsp");
    assert_eq!(default_home_in("/home/z/"), "/home/z/.wsp");
    assert_eq!(default_home_in("/"), "/.wsp");
    assert_eq!(default_home_in("rel"), "rel/.wsp");
    assert_eq!(default_home_in(""), ".wsp");
}

#[test]
fn home_named_treats_unset_and_empty_as_one_answer_and_a_space_as_a_home() {
    // serving-home.ts:19
    assert_eq!(home_named(None), None);
    assert_eq!(home_named(Some("")), None);
    assert_eq!(home_named(Some(" ")), Some(" ".to_string()));
    assert_eq!(home_named(Some("/moved")), Some("/moved".to_string()));
    assert_eq!(home_named(Some("rel/home")), Some("rel/home".to_string()));
}

#[test]
fn serving_home_is_wsp_home_when_it_names_one_and_the_users_dot_wsp_otherwise() {
    // serving-home.ts:22-24: `homeNamed(env[HOME_ENV]) ?? defaultHomeIn(user)`
    assert_eq!(serving_home(&env_of(&[]), "/home/z"), "/home/z/.wsp");
    assert_eq!(serving_home(&env_of(&[("WSP_HOME", "")]), "/home/z"), "/home/z/.wsp");
    assert_eq!(serving_home(&env_of(&[("WSP_HOME", "/srv/wsp")]), "/home/z"), "/srv/wsp");
    assert_eq!(serving_home(&env_of(&[("WSP_HOME", " ")]), "/home/z"), " ");
    assert_eq!(serving_home(&env_of(&[("HOME", "/other"), ("WSP_HOMES", "/x"), ("wsp_home", "/y")]), "/home/z"), "/home/z/.wsp");
}

#[test]
fn real_state_resolves_a_file_or_its_folder_through_links_and_leaves_what_nothing_made_as_written() {
    // serving-home.ts:28-32
    let dir = tmp();
    let base = real(&dir);
    let folder = at(&base, "real");
    put(&at(&folder, "state.json"), "{}");
    let link = at(&base, "link");
    symlink(&folder, &link).unwrap();

    assert_eq!(real_state(&at(&link, "state.json")), at(&folder, "state.json"));
    assert_eq!(real_state(&at(&link, "state.json")), real_state(&at(&folder, "state.json")));

    assert_eq!(real_state(&at(&link, "not-yet.json")), at(&folder, "not-yet.json"));

    let file_link = at(&base, "file-link.json");
    symlink(at(&folder, "state.json"), &file_link).unwrap();
    assert_eq!(real_state(&file_link), at(&folder, "state.json"));

    let nothing = "/nonexistent-wsp-cli-test/deeper/state.json";
    assert_eq!(real_state(nothing), nothing);
}

// repo-root.ts

fn mkdirs(path: &str) {
    fs::create_dir_all(path).unwrap();
}

#[test]
fn git_root_of_finds_the_nearest_folder_holding_a_git_folder_or_file() {
    // repo-root.ts:9-14: existsSync(join(at, ".git")), the folder itself first
    let dir = tmp();
    let base = real(&dir);
    mkdirs(&at(&base, "repo/.git"));
    mkdirs(&at(&base, "repo/a/b"));
    assert_eq!(git_root_of(&at(&base, "repo")), Some(at(&base, "repo")));
    assert_eq!(git_root_of(&at(&base, "repo/a/b")), Some(at(&base, "repo")));
    assert_eq!(git_root_of(&at(&base, "repo/a/not/made/yet")), Some(at(&base, "repo")));

    put(&at(&base, "repo/a/wt/.git"), "gitdir: /x/.git/worktrees/wt\n");
    mkdirs(&at(&base, "repo/a/wt/src"));
    assert_eq!(git_root_of(&at(&base, "repo/a/wt/src")), Some(at(&base, "repo/a/wt")));
    assert_eq!(git_root_of(&at(&base, "repo/a/b")), Some(at(&base, "repo")));
}

#[test]
fn git_root_of_returns_the_folder_as_written_when_it_holds_the_git_entry() {
    // repo-root.ts:10-11: `at` is `folder` on the first pass, a trailing slash included
    let dir = tmp();
    let base = real(&dir);
    mkdirs(&at(&base, "repo/.git"));
    let written = format!("{}/", at(&base, "repo"));
    assert_eq!(git_root_of(&written), Some(written.clone()));
}

#[test]
fn git_root_of_a_folder_with_no_git_above_it_is_none_down_to_the_root() {
    // repo-root.ts:12: `dirname(at) === at` at "/" ends the walk
    assert_eq!(git_root_of("/"), None);
    assert_eq!(git_root_of("/nonexistent-wsp-cli-test/a/b"), None);
}

#[test]
fn main_worktree_of_a_repo_with_a_git_folder_is_the_root_itself() {
    // repo-root.ts:22: a `.git` directory makes the root the main folder, from any depth
    let dir = tmp();
    let base = real(&dir);
    mkdirs(&at(&base, "main/.git"));
    mkdirs(&at(&base, "main/src/deep"));
    assert_eq!(main_worktree_of(&at(&base, "main")), Some(at(&base, "main")));
    assert_eq!(main_worktree_of(&at(&base, "main/src/deep")), Some(at(&base, "main")));
}

#[test]
fn main_worktree_of_a_linked_worktree_with_an_absolute_gitdir_is_the_main_folder() {
    // repo-root.ts:23-27: `gitdir: <main>/.git/worktrees/x` names <main>
    let dir = tmp();
    let base = real(&dir);
    mkdirs(&at(&base, "main/.git/worktrees/x"));
    put(&at(&base, "wt/.git"), &format!("gitdir: {}/main/.git/worktrees/x\n", base));
    mkdirs(&at(&base, "wt/src"));
    assert_eq!(main_worktree_of(&at(&base, "wt")), Some(at(&base, "main")));
    assert_eq!(main_worktree_of(&at(&base, "wt/src")), Some(at(&base, "main")));
}

#[test]
fn main_worktree_of_a_linked_worktree_with_a_relative_gitdir_resolves_it_against_the_worktree() {
    // repo-root.ts:25: `resolve(root, pointed)` for a path that is not absolute
    let dir = tmp();
    let base = real(&dir);
    mkdirs(&at(&base, "main/.git/worktrees/x"));
    mkdirs(&at(&base, "trees/wt"));
    put(&at(&base, "trees/wt/.git"), "gitdir: ../../main/.git/worktrees/x\n");
    assert_eq!(main_worktree_of(&at(&base, "trees/wt")), Some(at(&base, "main")));
}

#[test]
fn main_worktree_of_reads_the_gitdir_line_wherever_it_stands_in_the_file_and_trims_it() {
    // repo-root.ts:23: /^gitdir:\s*(.+)$/m, trimmed
    let dir = tmp();
    let base = real(&dir);
    put(&at(&base, "wt/.git"), &format!("# note\ngitdir:   {}/main/.git/worktrees/x  \nmore\n", base));
    assert_eq!(main_worktree_of(&at(&base, "wt")), Some(at(&base, "main")));
}

#[test]
fn main_worktree_of_a_gitdir_not_under_dot_git_worktrees_is_none() {
    // repo-root.ts:27: the git directory's parent must end in /.git/worktrees
    let dir = tmp();
    let base = real(&dir);
    for pointed in [
        format!("{base}/elsewhere/x"),
        format!("{base}/main/.git/modules/x"),
        format!("{base}/main/.git"),
        format!("{base}/main/worktrees/x"),
        format!("{base}/main/.git/worktrees"),
    ] {
        put(&at(&base, "wt/.git"), &format!("gitdir: {pointed}\n"));
        assert_eq!(main_worktree_of(&at(&base, "wt")), None, "{pointed}");
    }
}

#[test]
fn main_worktree_of_a_git_file_with_no_gitdir_line_is_none() {
    // repo-root.ts:24: no match, `pointed` is undefined
    let dir = tmp();
    let base = real(&dir);
    for text in ["", "nothing here\n", "  gitdir: /x/.git/worktrees/y\n", "xgitdir: /x/.git/worktrees/y\n"] {
        put(&at(&base, "wt/.git"), text);
        assert_eq!(main_worktree_of(&at(&base, "wt")), None, "{text:?}");
    }
}

#[test]
fn main_worktree_of_a_gitdir_at_the_filesystem_root_names_the_root() {
    // repo-root.ts:28: dirname(dirname(dirname("/.git/worktrees/x"))) is "/" in node
    let dir = tmp();
    let base = real(&dir);
    put(&at(&base, "wt/.git"), "gitdir: /.git/worktrees/x\n");
    assert_eq!(main_worktree_of(&at(&base, "wt")), Some("/".to_string()));
}

#[test]
fn main_worktree_of_a_folder_in_no_repository_is_none() {
    // repo-root.ts:19-20
    assert_eq!(main_worktree_of("/"), None);
    assert_eq!(main_worktree_of("/nonexistent-wsp-cli-test/a"), None);
}

// version.ts

fn host_package_version() -> String {
    let text = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/host/package.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["version"].as_str().unwrap().to_string()
}

#[test]
fn version_is_the_version_packages_host_package_json_carries_and_installed_version_reads_it() {
    // version.ts:9-11: VERSION is installedVersion() of ../package.json
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/host/package.json");
    let want = host_package_version();
    assert_eq!(installed_version(path), Some(want.clone()));
    assert_eq!(VERSION, want);
}

#[test]
fn installed_version_reads_the_version_string_off_a_package_json_now() {
    // version.ts:9: a reinstall under a running host changes this reading
    let dir = tmp();
    let path = at(&real(&dir), "package.json");
    put(&path, r#"{"name":"x","version":"1.2.3"}"#);
    assert_eq!(installed_version(&path), Some("1.2.3".to_string()));
    put(&path, r#"{"version":"4.5.6-rc.1"}"#);
    assert_eq!(installed_version(&path), Some("4.5.6-rc.1".to_string()));
    put(&path, r#"{"version":""}"#);
    assert_eq!(installed_version(&path), Some(String::new()));
}

#[test]
fn installed_version_of_a_missing_unparsable_or_versionless_file_is_none() {
    // version.ts:9: the TS throws in each case; the port answers none
    let dir = tmp();
    let base = real(&dir);
    let path = at(&base, "package.json");
    assert_eq!(installed_version(&path), None);
    for text in ["", "not json", "null", "[]", "{}", r#"{"version":3}"#, r#"{"version":null}"#, r#"{"version":["1.0.0"]}"#] {
        put(&path, text);
        assert_eq!(installed_version(&path), None, "{text:?}");
    }
    assert_eq!(installed_version(&base), None);
}

fn writer(bin: &str) -> StateWriter {
    StateWriter { wsp: VERSION.to_string(), daemon: JsInt::from(DAEMON_VERSION), bin: bin.to_string() }
}

#[test]
fn state_writer_here_names_the_version_the_daemon_and_argv_1_as_it_was_given_when_absolute() {
    // version.ts:16: {wsp: VERSION, daemon: DAEMON_VERSION, bin}
    let got = state_writer_here(&argv(&["node", "/usr/lib/node_modules/@wsp-labs/wsp/bin/wsp.js"]), "/usr/bin/node", "/work");
    assert_eq!(got, writer("/usr/lib/node_modules/@wsp-labs/wsp/bin/wsp.js"));
}

#[test]
fn state_writer_here_resolves_a_relative_argv_1_against_the_working_folder() {
    // version.ts:16: node's path.resolve
    let node = argv(&["node"]);
    let with = |script: &str| {
        let mut run = node.clone();
        run.push(script.to_string());
        state_writer_here(&run, "/usr/bin/node", "/work/dir")
    };
    assert_eq!(with("bin/wsp.js"), writer("/work/dir/bin/wsp.js"));
    assert_eq!(with("./wsp.js"), writer("/work/dir/wsp.js"));
    assert_eq!(with("../other/wsp.js"), writer("/work/other/wsp.js"));
    assert_eq!(with("a/./b/../c.js"), writer("/work/dir/a/c.js"));
    assert_eq!(with("a/b/"), writer("/work/dir/a/b"));
    assert_eq!(with("/abs/../wsp.js"), writer("/wsp.js"));
    assert_eq!(state_writer_here(&argv(&["node", "../x.js"]), "/usr/bin/node", "/"), writer("/x.js"));
}

#[test]
fn state_writer_here_without_an_argv_1_names_the_executable() {
    // version.ts:16: `process.argv[1] === undefined ? process.execPath : ...`
    assert_eq!(state_writer_here(&argv(&["node"]), "/usr/local/bin/wsp", "/work"), writer("/usr/local/bin/wsp"));
    assert_eq!(state_writer_here(&[], "/usr/local/bin/wsp", "/work"), writer("/usr/local/bin/wsp"));
}

// cloud.ts

#[test]
fn cloud_on_in_is_true_only_for_wsp_cloud_exactly_1() {
    // cloud.ts:17 with protocol cloudFromEnv: `env[CLOUD_ENV] === "1"`
    assert!(cloud_on_in(&env_of(&[("WSP_CLOUD", "1")])));
    assert!(cloud_on_in(&env_of(&[("OTHER", "x"), ("WSP_CLOUD", "1")])));
    for value in ["", "true", " 1", "1 ", "0", "01", "yes", "on", "11", "1\n"] {
        assert!(!cloud_on_in(&env_of(&[("WSP_CLOUD", value)])), "{value:?}");
    }
    assert!(!cloud_on_in(&env_of(&[])));
    assert!(!cloud_on_in(&env_of(&[("OTHER", "1")])));
    assert!(!cloud_on_in(&env_of(&[("wsp_cloud", "1"), ("WSP_CLOUDS", "1")])));
}

#[test]
fn cloud_on_reads_the_process_environment_and_answers_the_same_each_time() {
    // cloud.ts:17: read once when it loads and kept for the process's life
    let want = std::env::var("WSP_CLOUD").as_deref() == Ok("1");
    assert_eq!(cloud_on(), want);
    assert_eq!(cloud_on(), want);
}

// daemon-fix.ts

const NPM_BIN: &str = "/usr/lib/node_modules/@wsp-labs/wsp/bin/wsp.js";
const APP_BIN: &str = "/Applications/wsp.app/Contents/Resources/wsp";
const CHECKOUT_BIN: &str = "/home/z/wsp/packages/host/dist/bin.js";

#[test]
fn road_of_a_path_with_a_node_modules_part_is_npm() {
    // daemon-fix.ts:18-20
    assert_eq!(road_of(&argv(&["node", NPM_BIN]), None), Road::Npm);
    assert_eq!(road_of(&argv(&["node", "node_modules/.bin/wsp"]), None), Road::Npm);
    assert_eq!(road_of(&argv(&["node", "/proj/node_modules"]), None), Road::Npm);
}

#[test]
fn road_of_needs_the_whole_part_to_be_node_modules() {
    // daemon-fix.ts:20: `parts.includes("node_modules")`, an exact part
    assert_eq!(road_of(&argv(&["node", "/a/my_node_modules/b"]), None), Road::Checkout);
    assert_eq!(road_of(&argv(&["node", "/a/node_modules2/b"]), None), Road::Checkout);
    assert_eq!(road_of(&argv(&["node", "/a/b/node_modules.js"]), None), Road::Checkout);
}

#[test]
fn road_of_a_path_with_a_part_ending_dot_app_is_the_app() {
    // daemon-fix.ts:21
    assert_eq!(road_of(&argv(&["node", APP_BIN]), None), Road::App);
    assert_eq!(road_of(&argv(&["node", "/Applications/Wsp.app"]), None), Road::App);
    assert_eq!(road_of(&argv(&["node", "/x/.app/y"]), None), Road::App);
    assert_eq!(road_of(&argv(&["node", "/x/foo.app.bak/y"]), None), Road::Checkout);
    assert_eq!(road_of(&argv(&["node", "/x/foo.apps/y"]), None), Road::Checkout);
}

#[test]
fn road_of_npm_wins_over_app_when_a_path_is_both() {
    // daemon-fix.ts:20-21: node_modules is tested first
    assert_eq!(road_of(&argv(&["node", "/Applications/wsp.app/Contents/node_modules/wsp/bin.js"]), None), Road::Npm);
}

#[test]
fn road_of_anything_else_is_a_checkout() {
    // daemon-fix.ts:22
    assert_eq!(road_of(&argv(&["node", CHECKOUT_BIN]), None), Road::Checkout);
    assert_eq!(road_of(&argv(&["node", "bin.js"]), None), Road::Checkout);
}

#[test]
fn road_of_prefers_the_shim_over_argv_1() {
    // daemon-fix.ts:18: `run.shim ?? run.argv[1] ?? ""`
    assert_eq!(road_of(&argv(&["node", NPM_BIN]), Some(APP_BIN)), Road::App);
    assert_eq!(road_of(&argv(&["node", CHECKOUT_BIN]), Some(NPM_BIN)), Road::Npm);
    assert_eq!(road_of(&argv(&["node", NPM_BIN]), Some(CHECKOUT_BIN)), Road::Checkout);
    assert_eq!(road_of(&[], Some(NPM_BIN)), Road::Npm);
}

#[test]
fn road_of_an_empty_shim_is_still_the_shim_and_reads_as_a_checkout() {
    // daemon-fix.ts:18: `??` falls through on undefined only, so "" does not fall to argv[1]
    assert_eq!(road_of(&argv(&["node", NPM_BIN]), Some("")), Road::Checkout);
}

#[test]
fn road_of_an_empty_argv_or_one_with_no_script_is_a_checkout() {
    // daemon-fix.ts:18: `?? ""` splits to one empty part
    assert_eq!(road_of(&[], None), Road::Checkout);
    assert_eq!(road_of(&argv(&["node"]), None), Road::Checkout);
    assert_eq!(road_of(&argv(&["/usr/lib/node_modules/node"]), None), Road::Checkout);
}

#[test]
fn daemon_fix_line_is_the_npm_install_for_npm_the_app_update_for_the_app_and_a_cargo_build_for_a_checkout() {
    // daemon-fix.ts:28-34
    assert_eq!(daemon_fix_line(&argv(&["node", NPM_BIN]), None), "npm i -g @wsp-labs/wsp");
    assert_eq!(daemon_fix_line(&argv(&["node", APP_BIN]), None), "updating the wsp app");
    assert_eq!(
        daemon_fix_line(&argv(&["node", CHECKOUT_BIN]), None),
        "a cargo build of the daemon and node packages/wspx/scripts/daemon-binary.mjs --from its binary"
    );
    assert_eq!(
        daemon_fix_line(&[], None),
        "a cargo build of the daemon and node packages/wspx/scripts/daemon-binary.mjs --from its binary"
    );
    assert_eq!(daemon_fix_line(&argv(&["node", CHECKOUT_BIN]), Some(NPM_BIN)), "npm i -g @wsp-labs/wsp");
}

#[test]
fn release_update_line_names_the_release_for_npm_and_ignores_it_for_the_app_and_a_checkout() {
    // daemon-fix.ts:29-36
    assert_eq!(release_update_line(&argv(&["node", NPM_BIN]), None, "0.4.1"), "npm i -g @wsp-labs/wsp@0.4.1");
    assert_eq!(release_update_line(&argv(&["node", APP_BIN]), None, "0.4.1"), "updating the wsp app");
    assert_eq!(release_update_line(&argv(&["node", CHECKOUT_BIN]), None, "0.4.1"), "a pull of the checkout and a build");
    assert_eq!(release_update_line(&argv(&["node", CHECKOUT_BIN]), Some(APP_BIN), "9.9.9"), "updating the wsp app");
    assert_eq!(release_update_line(&[], None, "0.4.1"), "a pull of the checkout and a build");
}

#[test]
fn release_line_of_a_road_is_that_roads_release_line() {
    // daemon-fix.ts:40
    assert_eq!(release_line_of(Road::Npm, "1.2.3"), "npm i -g @wsp-labs/wsp@1.2.3");
    assert_eq!(release_line_of(Road::Npm, ""), "npm i -g @wsp-labs/wsp@");
    assert_eq!(release_line_of(Road::App, "1.2.3"), "updating the wsp app");
    assert_eq!(release_line_of(Road::Checkout, "1.2.3"), "a pull of the checkout and a build");
}
