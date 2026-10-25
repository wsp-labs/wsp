// SPDX-License-Identifier: AGPL-3.0-only
//! Values the normalizer must leave to the diff: what a person set or a product rule fixes, which a wrong port answers
//! differently. Each case pairs a TS host answer from the goldens' tapes with a wrong port's, which must not read alike.

use serde_json::{json, Value};
use xtask::normalize::{canonical, Facts};

fn alike(ts: Value, port: Value, facts: &Facts) -> bool {
    canonical(&[ts], &[], facts) == canonical(&[port], &[], facts)
}

#[test]
fn the_threads_at_once_a_person_set_is_compared() {
    // runtime-places-settings/a-computer-s-threads-at-once-...: places.set here threads=1 answers cap {threads: 1}.
    let row = |cap: u64| json!({ "id": 2, "ok": true, "place": { "id": "here", "settings": { "threads": 1 }, "cap": { "threads": cap }, "capDefault": { "threads": 8 } } });
    assert!(!alike(row(1), row(8), &Facts::default()), "a port that ignores places.set threads reads like the TS host");
}

#[test]
fn how_the_host_came_up_is_compared() {
    // runtime-release-door/...: release.get's shape is how the host was started (verb, up or service), no reading.
    let release = |shape: &str| json!({ "id": 2, "ok": true, "release": { "shape": shape } });
    assert!(!alike(release("service"), release("up"), &Facts::default()), "service and up read alike");
}

#[test]
fn a_limit_a_tool_states_is_compared() {
    // hand/mcp-tools: the computers tool's description states THREAD_MEM_MB.
    let tool = |text: &str| json!({ "jsonrpc": "2.0", "id": 2, "result": { "tools": [{ "name": "computers", "description": text }] } });
    assert!(!alike(
        tool("caps threads at one thread per 2 GB of memory up to twice its cores"),
        tool("caps threads at one thread per 4 GB of memory up to twice its cores"),
        &Facts::default()
    ));
}

#[test]
fn a_port_the_tape_asked_for_is_compared_and_only_the_replay_s_own_is_blanked() {
    // cli/host-up/wsp-up-...-holds-says-who-holds-it: wsp up --port 41139 names that port back.
    let mut facts = Facts::default();
    facts.add("45001", "<port>");
    let said = |port: u16| json!({ "exit": 1, "stdout": "", "stderr": format!("app http://127.0.0.1:{port} is held by another program") });
    assert!(!alike(said(41139), said(41140), &facts), "a host that ignored --port reads like one that kept it");
}

#[test]
fn json_a_tool_answers_as_text_is_compared_as_written() {
    // The tool server's content[].text: the Rust record (mcp-record.test.ts) holds it byte for byte.
    let text = |t: &str| json!({ "jsonrpc": "2.0", "id": 3, "result": { "content": [{ "type": "text", "text": t }] } });
    assert!(!alike(text("{\n  \"ok\": true\n}"), text("{\"ok\":true}"), &Facts::default()), "pretty and compact read alike");
}

#[test]
fn a_short_id_that_happens_to_be_all_digits_reads_like_any_other() {
    // cli/skill/wsp-doctor failed a run against the TS host itself: its thread's short id came out 79097289, which
    // reads as a number and is kept, where 3fa0c2d1 is an id. Eight hex characters are all digits about 1 run in 43.
    let doctor = |id: &str| json!({ "exit": 1, "stdout": "", "stderr": format!("no checkpoint of the files at thread {id}'s turn end") });
    assert!(alike(doctor("3fa0c2d1"), doctor("79097289"), &Facts::default()), "the TS host disagrees with itself");
}

/// A replay that started at 2026-10-25T19:35:23.456Z.
fn started() -> Facts {
    Facts { started: Some(1_792_956_923_456), ..Facts::default() }
}

#[test]
fn a_time_written_as_a_rust_library_writes_it_is_compared() {
    // JavaScript's toISOString writes milliseconds and Z; chrono's to_rfc3339 writes +00:00, and nanoseconds when it has them.
    let row = |at: &str| json!({ "id": 2, "ok": true, "thread": { "createdAt": at } });
    let ts = row("2026-10-25T19:35:25.456Z");
    for port in ["2026-10-25T19:35:25.456+00:00", "2026-10-25T19:35:25.456789012Z", "2026-10-25T19:35:25Z"] {
        assert!(!alike(ts.clone(), row(port), &started()), "{port} reads like 2026-10-25T19:35:25.456Z");
    }
}

#[test]
fn a_time_sent_as_a_number_where_the_ts_host_writes_text_is_compared() {
    let row = |at: Value| json!({ "id": 2, "ok": true, "thread": { "createdAt": at } });
    assert!(!alike(row(json!("2026-10-25T19:35:25.456Z")), row(json!(1_792_956_925_456_i64)), &started()));
}

#[test]
fn a_refusal_naming_another_id_than_the_one_asked_about_is_compared() {
    // 247 goldens hold a refusal naming the id the request sent, as "no such workspace: <ws-1>"; a port that names the
    // wrong variable names another id, which the request never sent and which does not move between two runs.
    let refusal = |id: &str| json!({ "id": 2, "ok": false, "error": format!("no such workspace: {id}") });
    assert!(!alike(refusal("ws_00aa11"), refusal("ws_7f3e21"), &Facts::default()));
}

#[test]
fn a_moving_value_of_the_wrong_kind_is_compared() {
    let turn = |pid: Value| json!({ "id": 2, "ok": true, "turn": { "pid": pid } });
    assert!(!alike(turn(json!(41234)), turn(Value::Null), &Facts::default()), "a pid of null reads like a pid");
}

#[test]
fn a_ticket_that_is_empty_is_compared() {
    let ticket = |t: &str| json!({ "id": 2, "ok": true, "ticket": t, "expiresAt": 1_793_000_000_000_i64 });
    assert!(!alike(ticket("Fq3oN2x7Lr8sV1kZ0aYbW6tD5pC4mE9h"), ticket(""), &Facts::default()));
}

#[test]
fn json_lines_keep_their_last_newline() {
    // The contract: with --json, one object per line; a line a reader waits for ends in a newline.
    let out = |stdout: &str| json!({ "exit": 0, "stdout": stdout, "stderr": "" });
    assert!(!alike(out("{\"agent\":\"claude\"}\n{\"ok\":true}\n"), out("{\"agent\":\"claude\"}\n{\"ok\":true}"), &Facts::default()));
}

#[test]
fn a_door_port_no_host_listens_on_is_compared() {
    // The door's port is read off the answer itself, so whatever port it names is blanked.
    let mut facts = Facts::default();
    facts.add("41857", "<port>");
    let door = |port: u16| json!({ "id": 2, "ok": true, "door": { "port": port, "url": format!("ws://0.0.0.0:{port}/") } });
    assert!(!alike(door(45013), door(9999), &facts), "a door that names a port nobody listens on reads like the TS host");
}

#[test]
fn a_uuid_written_in_capitals_is_compared() {
    // hand/sessions-start-turn: sessions.start answers the session's uuid as randomUUID writes it, in small letters.
    let started = |id: &str| json!({ "id": 2, "ok": true, "sessionId": id });
    assert!(
        !alike(started("0a93b975-1c2d-4e5f-8a9b-0c1d2e3f4a5b"), started("0A93B975-1C2D-4E5F-8A9B-0C1D2E3F4A5B"), &Facts::default()),
        "a uuid in capitals reads like the TS host's"
    );
}

#[test]
fn a_workspace_id_of_another_length_is_compared() {
    // sub-spawn.ts mints ws_ and eight hex digits; a port's format!("ws_{:x}", u64) writes sixteen.
    let made = |id: &str| json!({ "id": 2, "ok": true, "workspace": { "id": id } });
    assert!(!alike(made("ws_1f2e3d4c"), made("ws_1f2e3d4c5b6a7980"), &Facts::default()), "a ws_ id of sixteen digits reads like one of eight");
}

#[test]
fn a_pairing_code_without_its_dash_is_compared() {
    let issued = |code: &str| json!({ "id": 2, "ok": true, "code": code });
    assert!(!alike(issued("K7QM-2XRP"), issued("K7QM2XRP"), &Facts::default()), "a code with no dash reads like the TS host's");
}

#[test]
fn an_expiry_half_a_minute_long_is_compared() {
    // pair.issue: a code stands PAIR_CODE_TTL_MS, ten minutes; a port's 630 s reads <time+10m> as well.
    let at = started().started.expect("started");
    let issued = |expires: i64| json!({ "id": 2, "ok": true, "expiresAt": expires });
    assert!(!alike(issued(at + 600_000), issued(at + 630_000), &started()), "a code that stands 10 m 30 s reads like one of 10 m");
}

#[test]
fn events_in_another_order_are_compared() {
    // hand/sessions-start-turn: the session's row goes running, then completed, both events at seq 13. The app keeps
    // the last row it is sent, so a port that sends them the other way round leaves the thread running.
    let row = |status: &str| json!({ "type": "session.row", "id": "0a93b975-1c2d-4e5f-8a9b-0c1d2e3f4a5b", "row": { "status": status }, "seq": 13 });
    let reply = json!({ "id": 2, "ok": true });
    let ts = canonical(&[reply.clone()], &[row("running"), row("completed")], &Facts::default());
    let port = canonical(&[reply], &[row("completed"), row("running")], &Facts::default());
    assert_ne!(ts, port, "a session left running reads like one that completed");
}
