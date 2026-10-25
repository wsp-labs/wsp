// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/places.test.ts, packages/protocol/test/protocol.test.ts and
//! packages/host/test/ssh-hosts.test.ts: place_link.
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_frames::Base64Bytes;
use wsp_wire::wire::place_link::*;
use wsp_wire::wire::runtime::{parse_runtime_request, RuntimeOp};

fn b64<const N: usize>(fill: u8) -> String {
    Base64Bytes::<N>::from_bytes(&[fill; N]).as_str().to_owned()
}

fn nonce() -> String {
    b64::<32>(7)
}
fn public_key() -> String {
    b64::<44>(3)
}
fn signature() -> String {
    b64::<64>(5)
}

fn report() -> Value {
    json!({
        "name": "old-macbook",
        "platform": "darwin",
        "arch": "arm64",
        "os": "Darwin 24.5.0",
        "shape": { "cpu": 4, "memMb": 8192 },
        "login": { "HOME": "/Users/maya", "USER": "maya", "PATH": "/usr/bin" },
        "runsWorkspaces": true,
        "engine": "none",
        "daemonVersion": 17,
        "agents": [],
        "wsp": ["/usr/local/bin/wsp"],
        "dialed": "http://192.168.1.20:4400",
    })
}

fn with(base: Value, key: &str, value: Value) -> Value {
    let mut v = base;
    v.as_object_mut().unwrap().insert(key.to_owned(), value);
    v
}

fn without(base: Value, key: &str) -> Value {
    let mut v = base;
    v.as_object_mut().unwrap().remove(key);
    v
}

fn prove(extra: Value) -> Value {
    let mut frame = json!({ "id": 2, "op": "place.prove", "signature": signature(), "report": report() });
    for (k, v) in extra.as_object().unwrap() {
        frame.as_object_mut().unwrap().insert(k.clone(), v.clone());
    }
    frame
}

/// A place.auth frame read through the runtime's own reader: the place id, nonce and ephemeral it carried.
fn auth(place_id: &str, nonce_fill: u8, ephemeral_fill: u8) -> (String, String, String) {
    let frame =
        json!({ "id": 1, "op": "place.auth", "placeId": place_id, "nonce": b64::<32>(nonce_fill), "ephemeral": b64::<32>(ephemeral_fill) });
    match reads_with(parse_runtime_request, frame).op {
        RuntimeOp::PlaceAuth(req) => (req.place_id, req.nonce.as_str().to_owned(), req.ephemeral.expect("ephemeral").as_str().to_owned()),
        other => panic!("{other:?}"),
    }
}

fn host_link(place_id: &str, a: &str, b: &str, challenger: &str, answerer: &str) -> Vec<u8> {
    place_link_transcript(LinkRole::Host, place_id, a, b, LinkEphemerals { challenger, answerer })
}

mod what_a_joining_computer_may_send {
    use super::*;

    // places.test.ts:97
    #[test]
    fn takes_a_whole_join_frame_the_key_it_will_prove_and_the_two_public_values_and_not() {
        let ephemeral = b64::<32>(8);
        let frame = json!({ "id": 1, "op": "place.join", "publicKey": public_key(), "nonce": nonce(), "ephemeral": ephemeral });
        let r = reads_with(parse_runtime_request, frame.clone());
        match r.op {
            RuntimeOp::PlaceJoin(req) => {
                assert_eq!(req.public_key.as_str(), public_key());
                assert_eq!(req.nonce.as_str(), nonce());
                assert_eq!(req.ephemeral.expect("ephemeral").as_str(), ephemeral);
            }
            other => panic!("{other:?}"),
        }
        // The code and the report ride the prove, inside the seal: the first frame carries neither.
        let first = reads_with_as(
            parse_runtime_request,
            with(with(frame, "code", json!("7QK3M2VD")), "report", report()),
            json!({
                "id": 1, "op": "place.join", "publicKey": public_key(), "nonce": nonce(), "ephemeral": ephemeral,
            }),
        );
        let mut keys: Vec<String> = serde_json::to_value(&first).unwrap().as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["ephemeral", "id", "nonce", "op", "publicKey"]);
    }

    // places.test.ts:104
    #[test]
    fn refuses_a_half_of_the_key_agreement_that_is_not_the_length_one_is_and_takes_a_fr() {
        let short = refused_with(
            parse_runtime_request,
            json!({ "id": 1, "op": "place.join", "publicKey": public_key(), "nonce": nonce(), "ephemeral": b64::<16>(8) }),
        );
        assert_eq!(short.path, path!["ephemeral"]);
        assert_eq!(short.code, "custom");
        // Absent reads, so the host can refuse it in its own sentence rather than with a word about a field.
        let r = reads_with(parse_runtime_request, json!({ "id": 1, "op": "place.join", "publicKey": public_key(), "nonce": nonce() }));
        match r.op {
            RuntimeOp::PlaceJoin(req) => assert!(req.ephemeral.is_none()),
            other => panic!("{other:?}"),
        }
    }

    // places.test.ts:110
    #[test]
    fn refuses_a_nonce_that_is_not_the_length_a_challenge_is_a_short_one_is_a_nonce_som() {
        let e = refused_with(
            parse_runtime_request,
            json!({ "id": 1, "op": "place.join", "publicKey": public_key(), "nonce": b64::<16>(7), "ephemeral": b64::<32>(8) }),
        );
        assert_eq!(e.path, path!["nonce"]);
        assert_eq!(e.code, "custom");
    }

    // places.test.ts:115
    #[test]
    fn refuses_a_key_that_is_not_an_ed25519_public_key_s_length() {
        let e = refused_with(
            parse_runtime_request,
            json!({ "id": 1, "op": "place.join", "publicKey": b64::<32>(3), "nonce": nonce(), "ephemeral": b64::<32>(8) }),
        );
        assert_eq!(e.path, path!["publicKey"]);
        assert_eq!(e.code, "custom");
    }

    // places.test.ts:120
    #[test]
    fn refuses_a_code_longer_than_any_this_host_mints_on_the_prove_that_carries_it() {
        let e = refused_with(parse_runtime_request, prove(json!({ "code": "x".repeat(65) })));
        assert_eq!(e.path, path!["code"]);
        assert_eq!(e.code, "too_big");
        let r = reads_with(parse_runtime_request, prove(json!({ "code": "7QK3M2VD" })));
        match r.op {
            RuntimeOp::PlaceProve(req) => assert_eq!(req.code.as_deref(), Some("7QK3M2VD")),
            other => panic!("{other:?}"),
        }
    }

    // places.test.ts:125
    #[test]
    fn refuses_a_report_whose_login_is_not_string_to_string_every_value_there_lands_in() {
        let bad = with(report(), "login", json!({ "HOME": 3 }));
        let e = refused_with(parse_runtime_request, json!({ "id": 2, "op": "place.prove", "signature": signature(), "report": bad }));
        assert_eq!(e.path, path!["report", "login", "HOME"]);
        assert_eq!(e.code, "invalid_type");
    }

    // places.test.ts:130
    #[test]
    fn refuses_a_report_that_dialed_something_other_than_an_http_address() {
        let bad = with(report(), "dialed", json!("ftp://192.168.1.20"));
        let e = refused_with(parse_runtime_request, json!({ "id": 2, "op": "place.prove", "signature": signature(), "report": bad }));
        assert_eq!(e.path, path!["report", "dialed"]);
        assert_eq!(e.code, "custom");
    }

    // places.test.ts:135
    #[test]
    fn takes_an_auth_frame_from_a_place_that_already_joined() {
        let frame = json!({ "id": 1, "op": "place.auth", "placeId": "p_ab12cd34", "nonce": nonce(), "ephemeral": b64::<32>(8) });
        let r = reads_with(parse_runtime_request, frame);
        match r.op {
            RuntimeOp::PlaceAuth(req) => {
                assert_eq!(req.place_id, "p_ab12cd34");
                assert_eq!(req.nonce.as_str(), nonce());
            }
            other => panic!("{other:?}"),
        }
    }
}

// The transcript functions are wsp-frames' own and already work, so each case first reads place.auth frames through
// the runtime's reader and builds the transcripts from the values read: the assertion on the bytes is the one the
// TS test makes, and the test fails until the frame reads.
mod the_bytes_both_sides_of_a_link_sign {
    use super::*;

    // places.test.ts:146
    #[test]
    fn differs_by_the_role_so_neither_side_s_signature_can_be_replayed_back_at_it_as_th() {
        let (p, a, challenger) = auth("p_1", 1, 3);
        let (_, b, answerer) = auth("p_1", 2, 4);
        let as_place = place_link_transcript(LinkRole::Place, &p, &a, &b, LinkEphemerals { challenger: &challenger, answerer: &answerer });
        assert_ne!(host_link(&p, &a, &b, &challenger, &answerer), as_place);
    }

    // places.test.ts:150
    #[test]
    fn differs_by_the_order_of_the_two_nonces_so_a_transcript_is_one_direction_of_one_l() {
        let (p, a, challenger) = auth("p_1", 1, 3);
        let (_, b, answerer) = auth("p_1", 2, 4);
        assert_ne!(host_link(&p, &a, &b, &challenger, &answerer), host_link(&p, &b, &a, &challenger, &answerer));
    }

    // places.test.ts:154
    #[test]
    fn differs_by_the_place_so_a_signature_for_one_place_proves_nothing_about_another() {
        let (p1, a, challenger) = auth("p_1", 1, 3);
        let (p2, b, answerer) = auth("p_2", 2, 4);
        assert_ne!(host_link(&p1, &a, &b, &challenger, &answerer), host_link(&p2, &a, &b, &challenger, &answerer));
    }

    // places.test.ts:158
    #[test]
    fn covers_both_halves_of_the_key_agreement_so_a_carrier_that_swapped_either_of_them() {
        let (p, a, challenger) = auth("p_1", 1, 3);
        let (_, b, answerer) = auth("p_1", 2, 4);
        let (_, _, swapped) = auth("p_1", 2, 5);
        let whole = host_link(&p, &a, &b, &challenger, &answerer);
        assert_ne!(whole, host_link(&p, &a, &b, &challenger, &swapped));
        assert_ne!(whole, host_link(&p, &a, &b, &answerer, &challenger));
    }

    // places.test.ts:164
    #[test]
    fn is_the_same_bytes_for_the_same_reading_so_the_two_sides_cannot_drift() {
        let (p, a, challenger) = auth("p_1", 1, 3);
        let (_, b, answerer) = auth("p_1", 2, 4);
        assert_eq!(host_link(&p, &a, &b, &challenger, &answerer), host_link(&p, &a, &b, &challenger, &answerer));
    }
}

mod the_steps_of_an_install_on_a_computer_over_ssh {
    use super::*;

    // places.test.ts:201 (only the placeNoLinkLine assertions; PLACE_INSTALL, PLACE_ADD_WORDS and
    // placeStillInstalledLine belong to other modules)
    #[test]
    fn names_the_daemon_as_what_goes_on_and_the_system_unit_wsp_join_installs_for_it_an() {
        let line = place_no_link_line("vps");
        assert!(line.contains("vps took the daemon and has not dialled this host yet"), "{line}");
        assert!(line.contains("wsp computers shows it the moment it does"), "{line}");
    }
}

mod what_a_join_carrying_the_app_s_own_ask_may_send {
    use super::*;

    // places.test.ts:265
    #[test]
    fn takes_a_client_the_window_s_token_is_minted_for_and_refuses_one_with_no_name_on() {
        let r = reads_with(parse_runtime_request, prove(json!({ "code": "7QK3M2VD", "client": { "name": "old-macbook" } })));
        match r.op {
            RuntimeOp::PlaceProve(req) => assert_eq!(req.client.expect("client").name, "old-macbook"),
            other => panic!("{other:?}"),
        }
        let e = refused_with(parse_runtime_request, prove(json!({ "code": "7QK3M2VD", "client": { "name": "" } })));
        assert_eq!(e.path, path!["client", "name"]);
        assert_eq!(e.code, "too_small");
    }

    // places.test.ts:270
    #[test]
    fn answers_the_primary_computer_s_name_and_its_half_of_the_key_agreement_and_the_wi() {
        let base = json!({ "placeId": "p_1", "hostPublicKey": public_key(), "nonce": nonce(), "signature": signature(), "ephemeral": b64::<32>(8), "hostName": "zingzy-mbp" });
        let reply: PlaceJoinReply = reads(base.clone());
        assert_eq!(reply.host_name, "zingzy-mbp");
        let empty = refused::<PlaceJoinReply>(with(base.clone(), "hostName", json!("")));
        assert_eq!(empty.path, path!["hostName"]);
        assert_eq!(empty.code, "too_small");
        // The reply to frame one is the last that travels in the clear, so nothing of the person's is on it.
        let none = refused::<PlaceJoinReply>(without(base, "ephemeral"));
        assert_eq!(none.path, path!["ephemeral"]);
        assert_eq!(none.code, "invalid_type");
        let token = refused::<PlaceJoinDevice>(json!({ "deviceId": "d_1", "deviceToken": "" }));
        assert_eq!(token.path, path!["deviceToken"]);
        assert_eq!(token.code, "too_small");
        let device: PlaceJoinDevice = reads(json!({ "deviceId": "d_1", "deviceToken": "t" }));
        assert_eq!(device.device_token, "t");
    }

    // places.test.ts:281
    #[test]
    fn takes_the_word_for_how_a_computer_copies_a_project_and_a_report_from_one_that_sa() {
        for copies in ["reflink", "snapshot", "plain"] {
            let r: PlaceReport = reads(with(report(), "copies", json!(copies)));
            assert_eq!(serde_json::to_value(&r.copies).unwrap(), json!(copies), "{copies}");
        }
        // A computer that runs no workspaces has no copy to describe, and a word nothing makes is not one.
        let none: PlaceReport = reads(report());
        assert_eq!(none.copies, None);
        let e = refused::<PlaceReport>(with(report(), "copies", json!("hardlink")));
        assert_eq!(e.path, path!["copies"]);
        assert_eq!(e.code, "invalid_enum_value");
    }

    // places.test.ts:290
    #[test]
    fn takes_the_agents_a_computer_found_on_itself_up_to_the_cap_the_sentence_they_land() {
        let r: PlaceReport = reads(with(report(), "agents", json!(["claude", "codex"])));
        assert_eq!(r.agents, ["claude", "codex"]);
        let e = refused::<PlaceReport>(with(report(), "agents", json!(vec!["claude"; 33])));
        assert_eq!(e.path, path!["agents"]);
        assert_eq!(e.code, "too_big");
    }
}

mod what_a_computer_says_about_its_agents_and_what_a_person_reads_off_it {
    use super::*;

    fn linux_report() -> Value {
        json!({
            "name": "spoo", "platform": "linux", "arch": "x64", "os": "Linux 6.8.0",
            "shape": { "cpu": 4, "memMb": 4096 },
            "login": { "HOME": "/root", "USER": "root", "PATH": "/usr/bin" },
            "runsWorkspaces": true, "engine": "none", "daemonVersion": 54,
            "wsp": ["/usr/bin/wsp"], "agents": ["claude", "codex"], "dialed": "https://relay.example.com",
        })
    }

    // protocol.test.ts:208
    #[test]
    fn a_report_carries_each_agent_s_version_line_and_the_files_under_the_logins_folder() {
        let said = with(
            with(linux_report(), "agentVersions", json!({ "claude": "2.1.270 (Claude Code)", "codex": "codex-cli 0.153.0" })),
            "logins",
            json!(["codex/auth.json"]),
        );
        let r: PlaceReport = reads(said.clone());
        assert_eq!(r.logins, Some(vec!["codex/auth.json".to_owned()]));
        assert_eq!(r.agent_versions.expect("versions").get("codex").map(String::as_str), Some("codex-cli 0.153.0"));
        // A daemon older than these two fields sends neither, which reads as unknown rather than as none.
        let old: PlaceReport = reads(linux_report());
        assert_eq!(old.logins, None);
        let walks = refused::<PlaceReport>(with(said.clone(), "logins", json!(["../../root/.ssh/id_ed25519"])));
        assert_eq!(walks.path, path!["logins", 0]);
        assert_eq!(walks.code, "custom");
        let absolute = refused::<PlaceReport>(with(said.clone(), "logins", json!(["/wsp/logins/codex/auth.json"])));
        assert_eq!(absolute.path, path!["logins", 0]);
        let seventeen = refused::<PlaceReport>(with(said.clone(), "agentVersions", json!(17)));
        assert_eq!(seventeen.path, path!["agentVersions"]);
        assert_eq!(seventeen.code, "invalid_type");
        let long = refused::<PlaceReport>(with(said.clone(), "agentVersions", json!({ "claude": "x".repeat(65) })));
        assert_eq!(long.path, path!["agentVersions", "claude"]);
        assert_eq!(long.code, "too_big");
        // The map is keyed by the agents list, so it is under that list's own cap: one rule, and the daemon's own
        // deserialiser holds a report to the same number.
        let many: serde_json::Map<String, Value> = (0..33).map(|n| (format!("a{n}"), json!("1.0.0"))).collect();
        let too_many = refused::<PlaceReport>(with(said.clone(), "agentVersions", Value::Object(many.clone())));
        assert_eq!(too_many.path, path!["agentVersions"]);
        assert_eq!(too_many.code, "custom");
        let capped: serde_json::Map<String, Value> = many.into_iter().take(32).collect();
        let _: PlaceReport = reads(with(said, "agentVersions", Value::Object(capped)));
    }

    // protocol.test.ts:224
    #[test]
    fn a_report_carries_its_disk_s_size_beside_its_free_bytes_and_one_from_a_daemon_tha() {
        let gb: i64 = 1024 * 1024 * 1024;
        let sized = with(with(linux_report(), "diskFreeBytes", json!(9 * gb)), "diskSizeBytes", json!(75 * gb));
        let r: PlaceReport = reads(sized.clone());
        assert_eq!(r.disk_size_bytes, Some(75 * gb));
        let old: PlaceReport = reads(linux_report());
        assert_eq!(old.disk_size_bytes, None);
        let e = refused::<PlaceReport>(with(sized, "diskSizeBytes", json!(-1)));
        assert_eq!(e.path, path!["diskSizeBytes"]);
        assert_eq!(e.code, "too_small");
    }
}

mod the_ssh_hosts_offered_where_a_computer_is_added {
    use super::*;

    // packages/host/test/ssh-hosts.test.ts:170 (only the isGitForge assertions; sshHostsIn is the host's own)
    #[test]
    fn hides_the_git_forges_and_their_subdomains_from_both_files_by_alias_or_by_hostnam() {
        for host in GIT_FORGE_HOSTS {
            assert!(is_git_forge(host), "{host}");
        }
        assert!(is_git_forge("eu.gitlab.com"));
        assert!(is_git_forge("GitHub.com"));
        assert!(!is_git_forge("mygithub.com"));
    }
}

// Edges no TS test pins, each printed by the TS module at a3b870a6e.
mod edges_the_ts_module_printed {
    use super::*;

    const FILE: &str =
        r#"{"placeId":"p","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"K","keyPath":"/k","joinedAt":"t"}"#;

    // place-link.ts:428-447: the six fields are checked, joinedAt is not, extra keys stay; anything else is none.
    #[test]
    fn parse_place_file_checks_six_fields_and_keeps_the_rest() {
        let read = parse_place_file(FILE).unwrap();
        assert_eq!(
            (read.place_id.as_str(), read.host_urls.clone(), read.joined_at.clone()),
            ("p", vec!["http://a".to_string()], Some(serde_json::json!("t")))
        );
        let odd = parse_place_file(
            r#"{"placeId":"p","name":"n","hostName":"h","hostUrls":[],"hostPublicKey":"K","keyPath":"/k","joinedAt":5,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(odd.joined_at, Some(serde_json::json!(5)));
        assert_eq!(place_file_text(&odd), "{\n  \"placeId\": \"p\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [],\n  \"hostPublicKey\": \"K\",\n  \"keyPath\": \"/k\",\n  \"joinedAt\": 5,\n  \"extra\": 1\n}\n");
        assert!(parse_place_file(r#"{"placeId":"p","name":"n","hostName":"h","hostPublicKey":"K","keyPath":"/k"}"#).is_none());
        assert!(
            parse_place_file(r#"{"placeId":"p","name":"n","hostName":"h","hostUrls":[1],"hostPublicKey":"K","keyPath":"/k"}"#).is_none()
        );
        for not_one in ["null", "[]", "{", "", "\"p\""] {
            assert!(parse_place_file(not_one).is_none(), "{not_one}");
        }
        let without =
            parse_place_file(r#"{"placeId":"p","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"K","keyPath":"/k"}"#)
                .unwrap();
        assert_eq!(without.joined_at, None);
        assert_eq!(place_file_text(&read), "{\n  \"placeId\": \"p\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [\n    \"http://a\"\n  ],\n  \"hostPublicKey\": \"K\",\n  \"keyPath\": \"/k\",\n  \"joinedAt\": \"t\"\n}\n");
    }

    // place-link.ts:451-470: the text after the first line that is exactly WSP_PLACE_HELD, past what the login's rc
    // printed before it; a file that parses is a join, `WSP_PLACE_LEFT key` alone is the key left broken, anything
    // else the place file left broken; no marker, or nothing after it, is a read that never ran.
    #[test]
    fn the_held_read_finds_its_own_line_and_names_what_a_join_left() {
        let home = "/home/x";
        let joined = |s: &str| match read_held_place(s, home) {
            Some(HeldPlace::Joined(f)) => f.place_id,
            other => panic!("{other:?}"),
        };
        let broken = |s: &str| match read_held_place(s, home) {
            Some(HeldPlace::Broken(path)) => path,
            other => panic!("{other:?}"),
        };
        assert_eq!(read_held_place("motd\n", home), None);
        assert_eq!(read_held_place("WSP_PLACE_HELD\n", home), None);
        assert_eq!(joined(&format!("WSP_PLACE_HELD\n{FILE}")), "p");
        assert_eq!(joined(&format!("motd\nWSP_PLACE_HELD\n{FILE}\n")), "p");
        assert_eq!(broken("WSP_PLACE_HELD\ngarbage"), "/home/x/.wsp/place.json");
        assert_eq!(broken("WSP_PLACE_HELD\nWSP_PLACE_LEFT key\n"), "/home/x/.wsp/place-key.pem");
        assert_eq!(broken("WSP_PLACE_HELD\n  WSP_PLACE_LEFT key  "), "/home/x/.wsp/place-key.pem");
        assert_eq!(broken("WSP_PLACE_HELD\nWSP_PLACE_LEFT file\n"), "/home/x/.wsp/place.json");
        // The first marker is the one read, so a second one inside the text is just text.
        assert_eq!(broken(&format!("a\nWSP_PLACE_HELD\nx\nWSP_PLACE_HELD\n{FILE}")), "/home/x/.wsp/place.json");
        // held_place_file reads the same text and answers the file or nothing; a marker mid-line is no marker.
        assert_eq!(held_place_file(&format!("motd\nWSP_PLACE_HELD\n{FILE}\n")).map(|f| f.place_id), Some("p".to_string()));
        assert!(held_place_file(&format!("xWSP_PLACE_HELD\n{FILE}")).is_none());
        assert!(held_place_file(&format!("WSP_PLACE_HELD{FILE}")).is_none());
    }

    #[test]
    fn place_file_names_one_of_these_computers_and_this_host_s_key() {
        let f = parse_place_file(FILE).unwrap();
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(place_file_names(Some(&f), &ids(&["p"]), "K"));
        assert!(!place_file_names(Some(&f), &ids(&["q"]), "K"));
        assert!(!place_file_names(Some(&f), &ids(&["p"]), "k"));
        assert!(!place_file_names(None, &ids(&["p"]), "K"));
    }

    // place-link.ts:354-365: a port and a trailing root dot come off, the case goes down, and a host is under a
    // domain when it is it or ends with "." and it.
    #[test]
    fn bare_host_host_under_and_is_git_forge() {
        for (host, bare) in [
            ("GitHub.com:22", "github.com"),
            ("github.com.", "github.com"),
            ("github.com.:", "github.com"),
            ("a:b:c", "a:b:c"),
            ("[::1]:22", "[::1]"),
            ("x.com:", "x.com"),
        ] {
            assert_eq!(bare_host(host), bare, "{host}");
        }
        assert!(host_under("eu.gitlab.com", "gitlab.com"));
        assert!(!host_under("mygithub.com", "github.com"));
        assert!(host_under("GITHUB.COM.", "github.com"));
        assert!(host_under("github.com:443", "github.com"));
        for (host, forge) in
            [("sr.ht", true), ("git.sr.ht", true), ("ssh.dev.azure.com", true), ("dev.azure.com", false), ("example.org", false)]
        {
            assert_eq!(is_git_forge(host), forge, "{host}");
        }
    }

    #[test]
    fn the_engine_word_and_the_plural_sentences() {
        assert_eq!(
            [engine_word(true, true), engine_word(false, true), engine_word(false, false)],
            [PlaceEngine::Docker, PlaceEngine::Podman, PlaceEngine::None]
        );
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(plugins_kept_line("box", &names(&["a"])), "a is still on box: wsp could not take it off");
        assert_eq!(plugins_kept_line("box", &names(&["a", "b"])), "a, b are still on box: wsp could not take them off");
        assert_eq!(plugins_kept_line("box", &[]), " are still on box: wsp could not take them off");
        assert_eq!(
            two_places_refusal("box", &names(&["p1", "p2"])),
            "box: this host holds 2 places by that name; name one by its id (p1, p2)."
        );
        let line = join_address_line();
        assert_eq!(
            (line.what.as_str(), line.fix.as_str()),
            ("That is not an address.", "Type it as the other screen shows it, like 192.168.1.20:4420.")
        );
    }

    // place-link.ts:315-325: a door has a port, at least one url address and a key.
    #[test]
    fn a_door_view_needs_an_address() {
        let e = refused::<PlaceDoorView>(serde_json::json!({ "port": 4420, "addresses": [], "hostKey": "k" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("too_small", path!["addresses"]));
        let door: PlaceDoorView = reads(
            serde_json::json!({ "port": 4420, "addresses": ["http://192.168.1.20:4420"], "hostKey": "k", "relay": "https://x.relay.test" }),
        );
        assert_eq!(door.relay.as_deref(), Some("https://x.relay.test"));
    }
}
