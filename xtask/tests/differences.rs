// SPDX-License-Identifier: AGPL-3.0-only
//! The four one-line changes to the TS host the harness was once blind to, each as the answers a wrong port gives
//! beside the TS host's, which must not read alike: a join signed over the wrong bytes, threads at once that ignore
//! what the person set, a limit a tool states, and a pairing code's lifetime. (The cap and the limit are in
//! hidden.rs.) Beside them, what must still read alike: two runs a few seconds apart, and a time the tape sent.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use ring::signature::KeyPair as _;
use serde_json::{json, Value};
use xtask::host::{checked_join, transcript, SIGNATURE_FAILS, SIGNATURE_HOLDS};
use xtask::normalize::{canonical, Facts};

fn host_key() -> (ring::signature::Ed25519KeyPair, String) {
    let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
    let key = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let mut der = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
    der.extend_from_slice(key.public_key().as_ref());
    (key, B64.encode(der))
}

#[test]
fn a_join_signed_over_the_wrong_bytes_reads_otherwise_than_one_signed_over_the_transcript() {
    // packages/runtime/src/places/link.ts:96 signs the transcript; the review's port signed its bytes reversed.
    let (key, public) = host_key();
    let sent = json!({ "id": 1, "op": "place.join", "publicKey": "AAAA", "nonce": "am9pbmVy", "ephemeral": "ZXBoZW1lcmFs" });
    let bytes = transcript("host", "p_1", "am9pbmVy", "aG9zdA==", "ZXBoZW1lcmFs", "aG9zdGVwaA==");
    let reply = |signed: &[u8]| {
        let mut r = json!({ "id": 1, "ok": true, "placeId": "p_1", "hostPublicKey": public, "nonce": "aG9zdA==", "ephemeral": "aG9zdGVwaA==",
            "signature": B64.encode(key.sign(signed).as_ref()) });
        checked_join(&sent, &mut r, None);
        r
    };
    let reversed: Vec<u8> = bytes.iter().rev().copied().collect();
    let (good, bad) = (reply(&bytes), reply(&reversed));
    assert_eq!(good["signature"], SIGNATURE_HOLDS);
    assert_eq!(bad["signature"], SIGNATURE_FAILS);
    assert_ne!(canonical(&[good], &[], &Facts::default()), canonical(&[bad], &[], &Facts::default()));
}

fn at(start: i64, ms: i64) -> String {
    // 2026-10-25T00:00:00Z is 1_792_886_400_000.
    let t = start + ms - 1_792_886_400_000;
    let (h, rest) = (t / 3_600_000, t % 3_600_000);
    format!("2026-10-25T{:02}:{:02}:{:02}.{:03}Z", h, rest / 60_000, (rest % 60_000) / 1000, rest % 1000)
}

fn started(ms: i64) -> Facts {
    Facts { started: Some(ms), ..Facts::default() }
}

#[test]
fn a_pairing_code_that_stands_an_hour_reads_otherwise_than_one_that_stands_ten_minutes() {
    // packages/protocol/src/wire/devices.ts:62 PAIR_CODE_TTL_MS; pair.issue answers expiresAt that far out.
    let start = 1_792_926_000_000;
    let issued = |ttl_ms: i64, late_ms: i64| json!({ "id": 2, "ok": true, "expiresAt": at(start, late_ms + ttl_ms) });
    let golden = canonical(&[issued(600_000, 1_200)], &[], &started(start));
    assert_eq!(golden["replies"][0]["expiresAt"], "<time+10m>");
    assert_ne!(golden, canonical(&[issued(3_600_000, 1_200)], &[], &started(start)), "ten minutes and an hour read alike");
    // A second run answers some seconds later into its own replay, and reads the same.
    let later = start + 86_400_000 + 37_000;
    assert_eq!(golden, canonical(&[json!({ "id": 2, "ok": true, "expiresAt": at(later, 600_000 + 9_000) })], &[], &started(later)));
}

#[test]
fn an_hour_s_bucket_reads_as_its_day_and_hour_and_the_door_s_port_as_the_door_s() {
    let start: i64 = 1_792_926_000_000 + 1_234_567; // 2026-10-25T11:20:34Z
    let today: i64 = 1_792_886_400_000;
    // The replay reads the door's port off the file the host keeps beside its state, never off the answer.
    let mut facts = started(start);
    facts.add("41857", "<door-port>");
    let got = canonical(
        &[
            json!({ "id": 1, "series": [{ "t": today }, { "t": today + 3 * 3_600_000 }], "door": { "port": 41857, "addresses": ["http://127.0.0.1:41857"] },
            "said": "host        running (pid 7, up 682ms)" }),
        ],
        &[],
        &facts,
    );
    assert_eq!(
        got["replies"][0],
        json!({ "id": 1, "series": [{ "t": "<day in ms>" }, { "t": "<day 03:00 in ms>" }], "door": { "port": "<door-port>", "addresses": ["http://127.0.0.1:<door-port>"] },
            "said": "host        running (pid 7, up <moving>)" })
    );
}

#[test]
fn the_free_disk_wsp_computers_prints_reads_as_moving_and_every_other_column_is_compared() {
    let table = |free: &str, cores: u8| json!({ "exit": 0, "stdout": format!("COMPUTER  KIND           CORES  MEMORY  DISK FREE  ENGINE\nlab       this computer  {cores}      31 GB   {free}  none\n"), "stderr": "" });
    let read = |v| canonical(&[v], &[], &Facts::default());
    assert_eq!(read(table("115.4 GB", 8)), read(table("98.2 GB ", 8)));
    assert_ne!(read(table("115.4 GB", 8)), read(table("115.4 GB", 4)));
}

#[test]
fn a_day_s_bucket_reads_as_its_day_and_a_time_the_tape_sent_stays_as_sent() {
    let start = 1_792_926_000_000; // 2026-10-25T11:00:00Z
    let week: Vec<Value> = (0..2).map(|d| json!(1_792_886_400_000_i64 - d * 86_400_000)).collect();
    let mut facts = started(start);
    facts.sent.insert(1_700_000_000_000);
    let got = canonical(&[json!({ "id": 3, "days": week, "since": 1_700_000_000_000_i64, "at": start + 2_000 })], &[], &facts);
    assert_eq!(
        got["replies"][0],
        json!({ "id": 3, "days": ["<day in ms>", "<day-1 in ms>"], "since": 1_700_000_000_000_i64, "at": "<time in ms>" })
    );
}

#[test]
fn what_a_second_run_hands_out_otherwise_reads_alike_and_nothing_beside_it_does() {
    let mut facts = Facts::default();
    facts.add("4000", "<pid>");
    let run = |pid: u32, ticket: &str, port: u16, took: &str| {
        json!({ "id": 1, "row": { "pid": pid, "agentPid": pid + 1, "exit": 0 }, "ticket": ticket,
            "stderr": "wsp exec: no thread 00000000-0000-4000-8000-000000000001\n",
            "stdout": format!("host        running\napp         http://127.0.0.1:{port}\n\nstep             time      note\n----------\nlocal workspace  {took}      wsp-doctor\nTOTAL            {took}\n") })
    };
    let read = |v| canonical(&[v], &[], &facts);
    assert_eq!(
        read(run(1738, "V_wCBr_gYpItUpIIAQjUzrLhATwmgZbm", 38647, "80ms")),
        read(run(1757, "q2Rs7_xYpItUpIIAQjUzrLhATwmgZbmA", 39885, "45ms"))
    );
    let got = read(run(1738, "V_wCBr_gYpItUpIIAQjUzrLhATwmgZbm", 38647, "80ms"));
    assert_eq!(got["replies"][0]["stderr"], "wsp exec: no thread <uuid-1>\n", "the pid fact cut into the uuid");
    assert_eq!(got["replies"][0]["row"]["exit"], 0);
    // The note beside a step's time is compared.
    let mut other = run(1738, "V_wCBr_gYpItUpIIAQjUzrLhATwmgZbm", 38647, "80ms");
    other["stdout"] = Value::String(other["stdout"].as_str().unwrap().replace("wsp-doctor", "wsp-doctor-2"));
    assert_ne!(got, read(other));
}

#[test]
fn a_workspace_a_refusal_names_that_the_host_made_reads_alike_on_every_run() {
    // cli/skill/wsp-doctor: the host makes the workspace in the same command that refuses, so its id moves; only a
    // refusal that says there is no such thing names an id the request sent.
    let doctor = |ws: &str| json!({ "exit": 1, "stdout": "", "stderr": format!("no checkpoint at the turn end on workspace {ws}: the daemon did not start") });
    let read = |v| canonical(&[v], &[], &Facts::default());
    assert_eq!(read(doctor("ws_c98aae0d")), read(doctor("ws_acb53e3b")));
}
