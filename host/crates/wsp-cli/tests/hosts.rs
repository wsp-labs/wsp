// SPDX-License-Identifier: AGPL-3.0-only
//! The hosts records and the one rule that decides which host a line runs against. Ported from
//! packages/host/test/hosts.test.ts and launch-pair.test.ts at 65f6b89c7 (the hostAddress cases belong to verbs.ts and
//! are cli-a's), with the words and folds checked against values the TS printed (tests/fixtures/hosts_oracle.json).
//! The spec is spec/hosts.md.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]
use std::fs;
use std::os::unix::fs::PermissionsExt;

use serde_json::Value;
use wsp_cli::hosts::*;
use wsp_words::exit::{HostError, Kind};

const HOST_KEY: &str = "SHA256:MVm4EO/x4dkERU6dZOt1s4N04aW619pwoUo/9Qpz40A";

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/hosts_oracle.json")).unwrap()
}

fn env(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct Dir {
    _tmp: tempfile::TempDir,
    path: String,
}

impl Dir {
    fn new() -> Dir {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        Dir { _tmp: tmp, path }
    }
    fn at(&self, name: &str) -> String {
        format!("{}/{name}", self.path)
    }
}

fn record(url: &str, id: &str) -> HostRecord {
    HostRecord {
        url: url.into(),
        device_id: id.into(),
        device_token: format!("tok-{id}"),
        host_key: Some(HOST_KEY.into()),
        paired_at: Some(Value::String("2026-09-11T10:00:00.000Z".into())),
        via: Via { kind: "account".into(), host_id: format!("h{id}") },
    }
}

fn on_account(url: &str, host_id: &str) -> HostRecord {
    HostRecord {
        url: url.into(),
        device_id: String::new(),
        device_token: String::new(),
        host_key: Some(HOST_KEY.into()),
        paired_at: Some(Value::String("2026-09-22T00:00:00.000Z".into())),
        via: Via { kind: "account".into(), host_id: host_id.into() },
    }
}

/// A state folder whose lock names this process: a host serving it as far as every reader is concerned.
fn served_state(dir: &Dir, port: u32) -> String {
    fs::write(dir.at("host.lock"), format!(r#"{{"pid":{},"port":{port},"startedAt":"2026-09-11T10:00:00.000Z"}}"#, std::process::id()))
        .unwrap();
    fs::write(dir.at("host-token"), "host-token\n").unwrap();
    dir.at("state.json")
}

/// A state folder holding a relay record: the host on this computer is one of the account's.
fn linked_state(dir: &Dir, host_id: &str) -> String {
    fs::write(dir.at("relay.json"), format!(r#"{{"relayUrl":"https://relay.example","hostId":"{host_id}","token":"t","name":"this-mac","linkedAt":"2026-09-22T00:00:00.000Z"}}"#)).unwrap();
    dir.at("state.json")
}

fn pick(host: Option<&str>, e: Env, home: &str) -> HostPick {
    HostPick { host: host.map(String::from), env: Some(e), home: Some(home.to_string()) }
}

fn alias_of(aim: &HostAim) -> Option<&str> {
    match aim {
        HostAim::Alias { alias, .. } => Some(alias),
        _ => None,
    }
}

fn url(u: &str, token: Option<&str>, key: Option<&str>) -> HostAim {
    HostAim::Url { url: u.into(), token: token.map(String::from), host_key: key.map(String::from) }
}

fn umask_022() {
    unsafe {
        libc::umask(0o022);
    }
}

// ---- the hosts file

/// hosts.test.ts:41.
#[test]
fn a_record_is_written_only_this_user_can_read_and_read_back_whole() {
    umask_022();
    let home = Dir::new();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1a2b3c4d")).unwrap();
    assert_eq!(read_host(&home.path, "box"), Some(record("http://box.local:4400", "d_1a2b3c4d")));
    assert_eq!(fs::metadata(format!("{}/box.json", hosts_dir(&home.path))).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(hosts_dir(&home.path)).unwrap().permissions().mode() & 0o777, 0o700);
}

/// hosts.ts:119 and rule 10.18: JSON.stringify(record, null, 2) and a newline, keys in the record's order.
#[test]
fn a_record_lands_as_two_space_json_with_a_newline_in_its_key_order() {
    let home = Dir::new();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    let text = fs::read_to_string(format!("{}/hosts/box.json", home.path)).unwrap();
    assert_eq!(
        text,
        "{\n  \"url\": \"http://box.local:4400\",\n  \"deviceId\": \"d_1\",\n  \"deviceToken\": \"tok-d_1\",\n  \"hostKey\": \"SHA256:MVm4EO/x4dkERU6dZOt1s4N04aW619pwoUo/9Qpz40A\",\n  \"pairedAt\": \"2026-09-11T10:00:00.000Z\",\n  \"via\": {\n    \"kind\": \"account\",\n    \"hostId\": \"hd_1\"\n  }\n}\n"
    );
    let mut keyless = record("http://box.local:4400", "d_2");
    keyless.host_key = None;
    write_host(&home.path, "box", &keyless).unwrap();
    assert!(!fs::read_to_string(format!("{}/hosts/box.json", home.path)).unwrap().contains("hostKey"));
}

/// hosts.test.ts:49.
#[test]
fn a_record_and_a_folder_an_older_build_left_wider_are_repaired_by_the_write() {
    umask_022();
    let home = Dir::new();
    let dir = hosts_dir(&home.path);
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(format!("{dir}/box.json"), "{}\n").unwrap();
    fs::set_permissions(format!("{dir}/box.json"), fs::Permissions::from_mode(0o644)).unwrap();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    assert_eq!(fs::metadata(format!("{dir}/box.json")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(read_host(&home.path, "box"), Some(record("http://box.local:4400", "d_1")));
}

/// hosts.test.ts:61.
#[test]
fn every_alias_is_listed_with_its_url_sorted_and_a_home_with_no_hosts_folder_lists_nothing() {
    let home = Dir::new();
    assert_eq!(list_hosts(&home.path), vec![]);
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    write_host(&home.path, "attic", &record("https://attic.example", "d_2")).unwrap();
    assert_eq!(
        list_hosts(&home.path),
        vec![
            HostEntry { alias: "attic".into(), url: "https://attic.example".into(), device_id: "d_2".into() },
            HostEntry { alias: "box".into(), url: "http://box.local:4400".into(), device_id: "d_1".into() },
        ]
    );
}

/// hosts.ts:129: localeCompare order, so case folds before it breaks ties (Box after attic, before cellar).
#[test]
fn the_listing_is_in_locale_order_not_byte_order() {
    let home = Dir::new();
    for alias in ["cellar", "Box", "attic", "box"] {
        write_host(&home.path, alias, &record("http://x", alias)).unwrap();
    }
    let aliases: Vec<String> = list_hosts(&home.path).into_iter().map(|h| h.alias).collect();
    assert_eq!(aliases, vec!["attic", "box", "Box", "cellar"]);
}

/// hosts.test.ts:72.
#[test]
fn remove_takes_the_file_away_and_answers_false_when_there_was_none() {
    let home = Dir::new();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    assert!(remove_host(&home.path, "box"));
    assert_eq!(read_host(&home.path, "box"), None);
    assert!(!fs::metadata(format!("{}/hosts/box.json", home.path)).is_ok());
    assert!(!remove_host(&home.path, "box"));
    assert!(!remove_host(&home.path, "../evil"));
}

/// hosts.ts:134-138: a file that is not a record is not removed.
#[test]
fn remove_leaves_a_file_that_does_not_read_as_a_record() {
    let home = Dir::new();
    fs::create_dir_all(hosts_dir(&home.path)).unwrap();
    fs::write(format!("{}/broken.json", hosts_dir(&home.path)), "{ not json").unwrap();
    assert!(!remove_host(&home.path, "broken"));
    assert_eq!(fs::read_to_string(format!("{}/broken.json", hosts_dir(&home.path))).unwrap(), "{ not json");
}

/// hosts.test.ts:80 and :115: a path is refused in one sentence before anything is written.
#[test]
fn a_word_that_could_never_be_a_file_under_the_hosts_folder_is_refused_in_one_sentence() {
    let home = Dir::new();
    let want = &oracle()["badAlias"];
    for word in ["../evil", "a/b", "", "-box", &"a".repeat(65)] {
        let refused: HostError = write_host(&home.path, word, &record("http://x", "d")).unwrap_err();
        let expected = want["message"].as_str().unwrap().replace("\"../evil\"", &serde_json::to_string(word).unwrap());
        assert_eq!(refused.message, expected, "{word:?}");
        assert_eq!(refused.kind, Some(Kind::Usage));
        assert_eq!(refused.fix.as_deref(), want["fix"].as_str());
    }
    assert!(!fs::metadata(home.at("evil.json")).is_ok());
    assert!(!fs::metadata(hosts_dir(&home.path)).is_ok(), "nothing was written, not even the folder");
    assert_eq!(read_host(&home.path, "../evil"), None);
}

/// hosts.test.ts:87: a file with no account behind it is no record.
#[test]
fn a_file_with_no_account_behind_it_reads_as_no_record() {
    let home = Dir::new();
    fs::create_dir_all(hosts_dir(&home.path)).unwrap();
    fs::write(
        format!("{}/lan.json", hosts_dir(&home.path)),
        r#"{"url":"http://192.168.1.9:4400","deviceId":"d","deviceToken":"t","pairedAt":"x"}"#,
    )
    .unwrap();
    fs::write(
        format!("{}/other.json", hosts_dir(&home.path)),
        r#"{"url":"u","deviceId":"d","deviceToken":"t","via":{"kind":"paired","hostId":"h"}}"#,
    )
    .unwrap();
    fs::write(format!("{}/nohost.json", hosts_dir(&home.path)), r#"{"url":"u","deviceId":"d","deviceToken":"t","via":{"kind":"account"}}"#)
        .unwrap();
    assert_eq!(read_host(&home.path, "lan"), None);
    assert_eq!(read_host(&home.path, "other"), None);
    assert_eq!(read_host(&home.path, "nohost"), None);
    assert_eq!(list_hosts(&home.path), vec![]);
}

/// hosts.ts:92-99 and rule 10.22: empty device fields and a missing pairedAt still read as a record.
#[test]
fn a_record_with_empty_device_fields_and_no_paired_at_still_reads() {
    let home = Dir::new();
    fs::create_dir_all(hosts_dir(&home.path)).unwrap();
    fs::write(
        format!("{}/attic.json", hosts_dir(&home.path)),
        r#"{"url":"https://hattic.example","deviceId":"","deviceToken":"","via":{"kind":"account","hostId":"hattic"}}"#,
    )
    .unwrap();
    let read = read_host(&home.path, "attic").unwrap();
    assert_eq!((read.device_id.as_str(), read.device_token.as_str(), read.paired_at.clone(), read.host_key.clone()), ("", "", None, None));
    assert_eq!(read.via, Via { kind: "account".into(), host_id: "hattic".into() });
}

/// hosts.test.ts:317: a hand-edited folder lists what reads and skips the rest.
#[test]
fn a_hosts_folder_somebody_hand_edited_lists_what_reads_and_skips_the_rest() {
    let home = Dir::new();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    fs::write(format!("{}/broken.json", hosts_dir(&home.path)), "{ not json").unwrap();
    fs::write(format!("{}/notes.txt", hosts_dir(&home.path)), "hello").unwrap();
    fs::write(format!("{}/..json", hosts_dir(&home.path)), "{}").unwrap();
    let aliases: Vec<String> = list_hosts(&home.path).into_iter().map(|h| h.alias).collect();
    assert_eq!(aliases, vec!["box"]);
    assert_eq!(read_host(&home.path, "broken"), None);
}

/// hosts.test.ts:96 and hosts_oracle.json aliasFrom: every fold lands on a name the checker takes.
#[test]
fn a_name_nobody_typed_folds_through_the_rule_a_typed_one_is_held_to() {
    for row in oracle()["aliasFrom"].as_array().unwrap() {
        let (name, folded) = (row[0].as_str().unwrap(), row[1].as_str().unwrap());
        assert_eq!(alias_from(name), folded, "{name:?}");
        assert!(alias_ok(folded), "{folded:?}");
        assert_eq!(alias_ok(name), row[2].as_bool().unwrap(), "{name:?}");
    }
    let home = Dir::new();
    for name in ["box.local", "[::1]", "///", "a..b", "a....b", "..", "a..", ".", "-.-", &"a".repeat(200)] {
        let folded = alias_from(name);
        write_host(&home.path, &folded, &record("http://x", "d")).unwrap();
        assert_eq!(read_host(&home.path, &folded), Some(record("http://x", "d")), "{name:?}");
    }
}

/// hosts_oracle.json aliasOk.
#[test]
fn an_alias_is_one_name_of_at_most_64_characters_opening_with_a_letter_or_digit() {
    for row in oracle()["aliasOk"].as_array().unwrap() {
        assert_eq!(alias_ok(row[0].as_str().unwrap()), row[1].as_bool().unwrap(), "{row}");
    }
    assert_eq!(ALIAS_MAX, 64);
}

/// hosts.test.ts:120: WSP_HOME names the home, and an empty one names none.
#[test]
fn wsp_home_is_the_named_home_and_an_empty_one_names_none() {
    assert_eq!(wsp_home(&env(&[("WSP_HOME", "/tmp/elsewhere")])), "/tmp/elsewhere");
    assert!(wsp_home(&env(&[])).ends_with("/.wsp"));
    assert_eq!(wsp_home(&env(&[("WSP_HOME", "")])), wsp_home(&env(&[])));
    assert_eq!(wsp_home(&env(&[("WSP_HOME", " ")])), " ");
    assert_eq!(wsp_home(&env(&[])), default_home());
    assert_eq!(hosts_dir("/h"), "/h/hosts");
    assert_eq!(app_logs_dir("/h"), "/h/logs");
}

/// launch-pair.test.ts:347: the default home is under the user's home directory.
#[test]
fn the_default_home_is_dot_wsp_under_the_users_home_directory() {
    let user = std::env::var("HOME").unwrap();
    assert_eq!(default_home(), format!("{user}/.wsp"));
}

/// hosts.test.ts:140 and :130, hosts_oracle.json window and wsUrl: loopback gets 5 s, anything else (unreadable words
/// too) 15 s, and the ws URL a dial opens.
#[test]
fn a_dial_waits_a_loopbacks_window_near_and_the_roads_window_far() {
    assert_eq!(dial_window_ms(&HostAim::Here), NEAR_WINDOW_MS);
    assert_eq!((NEAR_WINDOW_MS, FAR_WINDOW_MS), (5_000, 15_000));
    for row in oracle()["window"].as_array().unwrap() {
        let (u, want) = (row[0].as_str().unwrap(), row[1].as_u64().unwrap());
        assert_eq!(dial_window_ms(&url(u, None, None)), want, "{u}");
        assert_eq!(dial_window_ms(&HostAim::Alias { alias: "box".into(), record: record(u, "d") }), want, "{u}");
    }
    // hosts.test.ts:130: the address a dial opens, the protocol's rule re-exported here.
    for row in oracle()["wsUrl"].as_array().unwrap() {
        assert_eq!(ws_url_of(row[0].as_str().unwrap()).unwrap(), row[1].as_str().unwrap(), "{row}");
    }
}

// ---- which host a line runs against

/// hosts.test.ts:161.
#[test]
fn the_flag_beats_the_environment_beats_the_lock_here_beats_the_accounts_one_host() {
    let home = Dir::new();
    write_host(&home.path, "attic", &record("https://attic.example", "d_attic")).unwrap();
    let served_dir = Dir::new();
    let served = served_state(&served_dir, 4400);
    let gone = Dir::new().at("state.json");
    let none = Dir::new();
    let a = aimed_host(&served, &pick(Some("attic"), env(&[("WSP_HOST", "https://box.example")]), &home.path)).unwrap();
    assert_eq!(alias_of(&a), Some("attic"));
    assert_eq!(a, HostAim::Alias { alias: "attic".into(), record: record("https://attic.example", "d_attic") });
    assert_eq!(alias_of(&aimed_host(&served, &pick(None, env(&[("WSP_HOST", "attic")]), &home.path)).unwrap()), Some("attic"));
    assert_eq!(aimed_host(&served, &pick(None, env(&[]), &home.path)).unwrap(), HostAim::Here);
    assert_eq!(alias_of(&aimed_host(&gone, &pick(None, env(&[]), &home.path)).unwrap()), Some("attic"));
    assert_eq!(aimed_host(&gone, &pick(None, env(&[]), &none.path)).unwrap(), HostAim::Here);
}

/// hosts.test.ts:173.
#[test]
fn a_url_typed_where_an_alias_goes_is_a_host_nothing_is_stored_for() {
    let home = Dir::new();
    assert_eq!(
        aimed_host("/nowhere/state.json", &pick(Some("http://127.0.0.1:14400"), env(&[]), &home.path)).unwrap(),
        url("http://127.0.0.1:14400", None, None)
    );
    assert_eq!(
        aimed_host("/nowhere/state.json", &pick(None, env(&[("WSP_HOST", "https://attic.example")]), &home.path)).unwrap(),
        url("https://attic.example", None, None)
    );
}

/// hosts.test.ts:179.
#[test]
fn the_launch_pair_beats_any_state_file_or_alias_here_and_loses_to_the_flag_and_wsp_host() {
    let home = Dir::new();
    let carried = [("WSP_HOST_URL", "http://10.0.0.2:4700"), ("WSP_HOST_TOKEN", "scoped-token"), ("WSP_HOST_KEY", HOST_KEY)];
    let aimed = url("http://10.0.0.2:4700", Some("scoped-token"), Some(HOST_KEY));
    let gone = Dir::new().at("state.json");
    assert_eq!(aimed_host(&gone, &pick(None, env(&carried), &home.path)).unwrap(), aimed);
    let served_dir = Dir::new();
    assert_eq!(aimed_host(&served_state(&served_dir, 4600), &pick(None, env(&carried), &home.path)).unwrap(), aimed);
    write_host(&home.path, "attic", &record("https://attic.example", "d_attic")).unwrap();
    assert_eq!(aimed_host(&gone, &pick(None, env(&carried), &home.path)).unwrap(), aimed);
    assert_eq!(alias_of(&aimed_host(&gone, &pick(Some("attic"), env(&carried), &home.path)).unwrap()), Some("attic"));
    let mut with_host = carried.to_vec();
    with_host.push(("WSP_HOST", "attic"));
    assert_eq!(alias_of(&aimed_host(&gone, &pick(None, env(&with_host), &home.path)).unwrap()), Some("attic"));
    let none = Dir::new();
    assert_eq!(aimed_host(&gone, &pick(None, env(&[("WSP_HOST_URL", "http://10.0.0.2:4700")]), &none.path)).unwrap(), HostAim::Here);
    assert_eq!(
        aimed_host(&served_state(&served_dir, 4600), &pick(None, env(&[("WSP_HOST_URL", "http://10.0.0.2:4700")]), &home.path)).unwrap(),
        HostAim::Here
    );
}

/// hosts.test.ts:199.
#[test]
fn a_host_address_takes_the_token_the_environment_carries_for_that_same_address_only() {
    let home = Dir::new();
    let carried = [("WSP_HOST_URL", "http://10.0.0.2:4700"), ("WSP_HOST_TOKEN", "scoped-token"), ("WSP_HOST_KEY", HOST_KEY)];
    assert_eq!(
        aimed_host("/nowhere/state.json", &pick(Some("http://10.0.0.2:4700"), env(&carried), &home.path)).unwrap(),
        url("http://10.0.0.2:4700", Some("scoped-token"), Some(HOST_KEY))
    );
    assert_eq!(
        aimed_host("/nowhere/state.json", &pick(Some("http://other.example:4700"), env(&carried), &home.path)).unwrap(),
        url("http://other.example:4700", None, None)
    );
}

/// hosts.test.ts:207: an aim with a token and no key refuses, auth, naming where it came from.
#[test]
fn an_aim_at_a_host_with_no_key_held_refuses_whether_a_record_or_a_launch_carried_it() {
    let home = Dir::new();
    let mut keyless = record("https://box.example", "d_old");
    keyless.host_key = None;
    write_host(&home.path, "box", &keyless).unwrap();
    let r = aimed_host("/nowhere/state.json", &pick(Some("box"), env(&[]), &home.path)).unwrap_err();
    assert_eq!(r.message, wsp_wire::wire::place_link::host_no_key_line("box"));
    assert_eq!((r.kind, r.fix), (Some(Kind::Auth), None));
    let carried = [("WSP_HOST_URL", "https://box.example"), ("WSP_HOST_TOKEN", "scoped-token")];
    let none = Dir::new();
    let r = aimed_host("/nowhere/state.json", &pick(None, env(&carried), &none.path)).unwrap_err();
    assert_eq!(r.message, oracle()["noKey"]["message"].as_str().unwrap());
    assert_eq!(r.kind, Some(Kind::Auth));
    let r = aimed_host("/nowhere/state.json", &pick(Some("https://box.example"), env(&carried), &none.path)).unwrap_err();
    assert_eq!(r.message, wsp_wire::wire::place_link::host_no_key_line("https://box.example"));
    let here = [("WSP_HOST_URL", "http://127.0.0.1:4700"), ("WSP_HOST_TOKEN", "scoped-token")];
    assert_eq!(
        aimed_host("/nowhere/state.json", &pick(None, env(&here), &none.path)).unwrap(),
        url("http://127.0.0.1:4700", Some("scoped-token"), None)
    );
    let served_dir = Dir::new();
    assert_eq!(aimed_host(&served_state(&served_dir, 4800), &pick(None, env(&[]), &none.path)).unwrap(), HostAim::Here);
}

/// hosts.ts:347-353 with hosts_oracle.json window: servedHostname reads no ws:// address, so a ws:// loopback pair
/// with a token and no key is refused like any address off this computer.
#[test]
fn a_token_with_no_key_over_a_ws_loopback_address_is_refused_since_the_rule_reads_no_ws_address() {
    let none = Dir::new();
    let pair = [("WSP_HOST_URL", "ws://127.0.0.1:4700"), ("WSP_HOST_TOKEN", "scoped-token")];
    let r = aimed_host("/nowhere/state.json", &pick(None, env(&pair), &none.path)).unwrap_err();
    assert_eq!(r.kind, Some(Kind::Auth));
}

/// hosts.test.ts:225.
#[test]
fn an_alias_nothing_is_stored_for_is_refused_in_one_sentence_naming_the_ones_that_are() {
    let home = Dir::new();
    write_host(&home.path, "box", &record("http://box.local:4400", "d_1")).unwrap();
    let r = aimed_host("/nowhere/state.json", &pick(Some("attic"), env(&[]), &home.path)).unwrap_err();
    assert_eq!(
        no_such_host_line("attic", &home.path),
        "no host named attic is connected; this computer holds box, and wsp login puts the hosts on your account here."
    );
    assert_eq!(r.message, format!("{} Run wsp hosts for the list.", no_such_host_line("attic", &home.path)));
    assert_eq!((r.kind, r.fix.as_deref()), (Some(Kind::Usage), Some("Run wsp hosts for the list.")));
    assert_eq!(no_such_host_line("attic", &home.path).split('\n').count(), 1);
}

/// rule 10.22: an empty or blank --host refuses; a blank WSP_HOST is ignored; a padded name is trimmed.
#[test]
fn an_empty_host_word_refuses_a_blank_wsp_host_is_ignored_and_a_padded_name_is_trimmed() {
    let home = Dir::new();
    let want = &oracle()["empty"];
    for word in ["", " ", "\t"] {
        let r = named_host(&pick(Some(word), env(&[]), &home.path)).unwrap_err();
        assert_eq!(r.message, want["message"].as_str().unwrap(), "{word:?}");
        assert_eq!((r.kind, r.fix.as_deref()), (Some(Kind::Usage), want["fix"].as_str()));
    }
    let e = empty_host_refusal();
    assert_eq!(e.message, want["message"].as_str().unwrap());
    assert_eq!(named_host(&pick(None, env(&[("WSP_HOST", "  ")]), &home.path)).unwrap(), None);
    assert_eq!(named_host(&pick(None, env(&[("WSP_HOST", "")]), &home.path)).unwrap(), None);
    write_host(&home.path, "attic", &record("https://attic.example", "d")).unwrap();
    assert_eq!(alias_of(&named_host(&pick(Some(" attic "), env(&[]), &home.path)).unwrap().unwrap()), Some("attic"));
    assert_eq!(alias_of(&named_host(&pick(None, env(&[("WSP_HOST", " attic\n")]), &home.path)).unwrap().unwrap()), Some("attic"));
    assert_eq!(named_host(&pick(Some("http://a:1"), env(&[("WSP_HOST", "x")]), &home.path)).unwrap(), Some(url("http://a:1", None, None)));
}

// ---- the account hosts a line falls to

/// hosts.test.ts:246.
#[test]
fn a_line_goes_to_the_one_account_host_that_is_not_this_computer() {
    let home = Dir::new();
    write_host(&home.path, "attic", &on_account("https://hattic.example", "hattic")).unwrap();
    let s = Dir::new();
    let aim = aimed_host(&linked_state(&s, "hbox1"), &pick(None, env(&[]), &home.path)).unwrap();
    assert_eq!(aim, HostAim::Alias { alias: "attic".into(), record: on_account("https://hattic.example", "hattic") });
    assert_eq!(aimed_alias(&linked_state(&s, "hbox1"), &home.path).as_deref(), Some("attic"));
    assert_eq!(
        account_aim(&linked_state(&s, "hbox1"), &home.path),
        AccountAim::One { alias: "attic".into(), record: on_account("https://hattic.example", "hattic") }
    );
}

/// hosts.test.ts:256.
#[test]
fn a_host_starts_here_when_the_accounts_one_host_is_this_computer() {
    let home = Dir::new();
    write_host(&home.path, "this-mac", &on_account("https://hbox1.example", "hbox1")).unwrap();
    let s = Dir::new();
    let state = linked_state(&s, "hbox1");
    assert_eq!(aimed_host(&state, &pick(None, env(&[]), &home.path)).unwrap(), HostAim::Here);
    assert_eq!(account_hosts(&state, &home.path), vec![]);
    assert_eq!(
        account_records(&home.path),
        vec![HeldHost { alias: "this-mac".into(), record: on_account("https://hbox1.example", "hbox1") }]
    );
    assert_eq!(aimed_alias(&state, &home.path), None);
    assert_eq!(account_aim(&state, &home.path), AccountAim::None);
}

/// hosts.test.ts:264.
#[test]
fn several_account_hosts_refuse_in_one_sentence_naming_them_and_a_name_settles_it() {
    let home = Dir::new();
    write_host(&home.path, "attic", &on_account("https://hattic.example", "hattic")).unwrap();
    write_host(&home.path, "cellar", &on_account("https://hcellar.example", "hcellar")).unwrap();
    let s = Dir::new();
    let state = linked_state(&s, "hbox1");
    let r = aimed_host(&state, &pick(None, env(&[]), &home.path)).unwrap_err();
    let several = several_account_hosts_line(&["attic".into(), "cellar".into()]);
    assert_eq!(several, oracle()["lines"]["several"].as_str().unwrap());
    assert_eq!(r.message, format!("{several} {}", name_one_host()));
    assert_eq!((r.kind, r.fix), (Some(Kind::Usage), Some(name_one_host())));
    assert_eq!(name_one_host(), oracle()["lines"]["nameOne"].as_str().unwrap());
    assert_eq!(aimed_alias(&state, &home.path), None);
    assert_eq!(account_aim(&state, &home.path), AccountAim::Several { aliases: vec!["attic".into(), "cellar".into()] });
    assert_eq!(alias_of(&aimed_host(&state, &pick(Some("cellar"), env(&[]), &home.path)).unwrap()), Some("cellar"));
}

/// hosts.test.ts:275.
#[test]
fn a_host_starts_here_with_no_account_record_and_a_serving_host_beats_the_account() {
    let home = Dir::new();
    let s = Dir::new();
    assert_eq!(aimed_host(&linked_state(&s, "hbox1"), &pick(None, env(&[]), &home.path)).unwrap(), HostAim::Here);
    let served_dir = Dir::new();
    let served = served_state(&served_dir, 4600);
    write_host(&home.path, "attic", &on_account("https://hattic.example", "hattic")).unwrap();
    assert_eq!(aimed_host(&served, &pick(None, env(&[]), &home.path)).unwrap(), HostAim::Here);
}

/// hosts.test.ts:284: no relay asked, files only.
#[test]
fn the_account_is_read_off_the_hosts_folder_and_this_computers_record_alone() {
    let home = Dir::new();
    write_host(&home.path, "attic", &on_account("https://hattic.example", "hattic")).unwrap();
    assert_eq!(alias_of(&aimed_host("/nowhere/state.json", &pick(None, env(&[]), &home.path)).unwrap()), Some("attic"));
}

/// launch-pair.test.ts:347: an empty environment and a home with nothing in it aims here.
#[test]
fn a_line_with_no_pair_and_no_home_records_aims_here() {
    let home = Dir::new();
    assert_eq!(aimed_host(&home.at("state.json"), &pick(None, env(&[]), &home.path)).unwrap(), HostAim::Here);
}

// ---- the words

/// hosts_oracle.json lines: every sentence the hosts rule prints.
#[test]
fn the_hosts_words_are_the_ts_words() {
    let l = &oracle()["lines"];
    let s = |k: &str| l[k].as_str().unwrap().to_string();
    assert_eq!(no_such_host_among("attic", &[]), s("noSuch0"));
    assert_eq!(no_such_host_among("attic", &["box".into(), "cellar".into()]), s("noSuch2"));
    assert_eq!(read_the_hosts(), s("readThe"));
    assert_eq!(address_not_paired_line("http://x:1"), s("notPaired"));
    assert_eq!(device_refused_line("box"), s("refused"));
    assert_eq!(host_side_only_line("host pair", "box"), s("sideLine"));
    assert_eq!(host_side_only_fix(None), s("sideFix"));
    assert_eq!(host_side_only_fix(Some("Carrying a vault off is not either.")), s("sideFix2"));
    assert_eq!(state_ignored_line("box"), s("ignored"));
    assert_eq!(HOST_SIDE_ACCESS, "Handing out access is the one thing a paired computer cannot do from here.");
}

/// hosts.ts:240-248: the one way to build the no-answer sentence, stamped unreachable.
#[test]
fn a_host_that_did_not_answer_is_stamped_unreachable() {
    let e = no_answer_within("ws://box:1/ws", 5000);
    assert_eq!(e.message, oracle()["lines"]["noAnswer"].as_str().unwrap());
    assert_eq!((e.kind, e.fix), (Some(Kind::Unreachable), None));
    let e = no_answer_refusal("here", "the socket closed");
    assert_eq!(e.message, "the host at here did not answer: the socket closed");
    assert_eq!(e.kind, Some(Kind::Unreachable));
}

/// hosts.ts:358-371.
#[test]
fn an_aim_names_its_host_and_its_address_and_what_it_holds() {
    let alias = HostAim::Alias { alias: "box".into(), record: record("http://box.local:4400", "d_box") };
    let at = url("http://10.0.0.2:4700", Some("t"), None);
    assert_eq!(aim_name(&alias), "box");
    assert_eq!(aim_name(&at), "http://10.0.0.2:4700");
    assert_eq!(aim_name(&HostAim::Here), "this computer");
    assert_eq!(aim_address(&alias).as_deref(), Some("http://box.local:4400"));
    assert_eq!(aim_address(&at).as_deref(), Some("http://10.0.0.2:4700"));
    assert_eq!(aim_address(&HostAim::Here), None);
    assert_eq!(aim_holds(&alias), AimHolds { token: Some("tok-d_box".into()), host_key: Some(HOST_KEY.into()) });
    assert_eq!(aim_holds(&at), AimHolds { token: Some("t".into()), host_key: None });
    assert_eq!(aim_holds(&HostAim::Here), AimHolds { token: None, host_key: None });
}
