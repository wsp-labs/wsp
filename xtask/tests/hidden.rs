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
