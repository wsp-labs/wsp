// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written hard tests of the `ws_life` unit (`spec/ws-life.md`): the host invariant, the TS tests that name
//! these helpers (re-stated as recorded cases on a scripted host), and what a recording cannot hold: a countdown that
//! reads the clock.
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_cli::verbs::workspaces_help::{self as w, Woken};
use wsp_words::exit::{ExitClass, HostError, Kind};

use super::common::{holds, Driver, Script, ScriptedHost, Socket};
use super::drivers_ws_life as d;

const UNIT: &str = "ws_life";

fn driver_of(func: &str) -> Driver {
    match func {
        "renameWorkspace" => d::rename_workspace,
        "renamedWorkspaceLine" => d::renamed_workspace_line,
        "awake" => d::awake,
        "noDraftLine" => d::no_draft_line,
        "const_no_draft_fix" => d::const_no_draft_fix,
        "committed" => d::committed,
        "askedToFix" => d::asked_to_fix,
        "fixLine" => d::fix_line,
        "mergedIn" => d::merged_in,
        "startedFrom" => d::started_from,
        "reviewStarted" => d::review_started,
        "reviewPosted" => d::review_posted,
        "startedLines" => d::started_lines,
        "reviewLines" => d::review_lines,
        "threadLabel" => d::thread_label,
        "threadCwd" => d::thread_cwd,
        "folderOf" => d::folder_of,
        "withShared" => d::with_shared,
        "sharedOf" => d::shared_of,
        "const_verdicts" => d::const_verdicts,
        "mergedPr" => d::merged_pr,
        "updateLine" => d::update_line,
        "napAfterDeadLaunch" => d::nap_after_dead_launch,
        "withLine" => d::with_line,
        "stop" => d::stop,
        "stopLine" => d::stop_line,
        "forgetThread" => d::forget_thread,
        "threadForgotLine" => d::thread_forgot_line,
        "settleThreads" => d::settle_threads,
        "restoreThreads" => d::restore_threads,
        "settledLines" => d::settled_lines,
        "restoredLines" => d::restored_lines,
        "rename" => d::rename,
        "renameLine" => d::rename_line,
        other => panic!("no driver for {other}"),
    }
}

fn run(func: &str, name: &str) {
    holds(UNIT, func, name, driver_of(func));
}

fn recorded() -> Vec<Value> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_c/ws_life.json");
    let all: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture parses");
    all["cases"].as_array().expect("cases").clone()
}

fn recorded_case(func: &str, name: &str) -> Value {
    recorded().into_iter().find(|c| c["fn"] == func && c["case"] == name && c["cloud"] != json!(true)).unwrap_or_else(|| panic!("no case {func} / {name}"))
}

fn words_of(case: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["said", "stdout", "stderr"] {
        out.extend(case[key].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)));
    }
    out.extend(case["streamed"].as_str().map(String::from));
    out.extend(case["error"]["message"].as_str().map(String::from));
    out.extend(case["error"]["fix"].as_str().map(String::from));
    match &case["value"] {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => out.extend(a.iter().filter_map(|v| v.as_str().map(String::from))),
        Value::Object(o) => out.extend(o.get("said").and_then(Value::as_str).map(String::from)),
        _ => {}
    }
    out
}

/// Host invariant 7 (`packages/host/AGENTS.md`): with the cloud off no line names a cloud, and no word of this unit
/// moves with the setting. Each case recorded with the cloud on runs beside its twin recorded with it off; the words
/// are the same and none names Solari.
#[test]
fn invariant_7_the_cloud_setting_moves_no_word_of_ws_life() {
    let all = recorded();
    let twins: Vec<&Value> = all.iter().filter(|c| c["cloud"] == json!(true)).collect();
    assert!(twins.len() >= 10, "the unit records a cloud twin for a first case and every refusal");
    for on in twins {
        let name = on["case"].as_str().unwrap();
        let base = name.strip_suffix(" (cloud)").expect("a cloud twin is named by its twin");
        let off = all.iter().find(|c| c["fn"] == on["fn"] && c["case"] == base && c["cloud"] != json!(true)).unwrap_or_else(|| panic!("no twin of {name}"));
        for key in ["frames", "said", "stdout", "stderr", "streamed", "value", "error", "hung"] {
            assert_eq!(on[key], off[key], "{} / {name}: {key} moves with the cloud setting", on["fn"]);
        }
        let func = on["fn"].as_str().unwrap();
        run(func, name);
        run(func, base);
    }
    for case in &all {
        for line in words_of(case) {
            assert!(!line.contains("Solari"), "{} / {}: a line names a cloud: {line}", case["fn"], case["case"]);
        }
    }
}

/// `verbs-turns.test.ts` "is the caller's only where the machine was down before it and running after: a machine
/// already up, or one another caller is waking, was woken by neither". Re-stated on a scripted host; the TS test runs
/// on a fake runtime that cannot run here.
#[test]
fn ts_a_wake_belongs_to_the_caller_only_from_down_to_running() {
    let cases = [
        ("napping for send, the wake answers running", true),
        ("pausing for send, the wake answers running", true),
        ("running for send, the wake answers running", false),
        ("waking for send, the wake answers running", false),
        ("napping, the wake leaves it napping", false),
    ];
    for (name, woke) in cases {
        run("awake", name);
        assert_eq!(recorded_case("awake", name)["value"]["woke"], json!(woke), "{name}: the recorded woke");
    }
}

/// `verbs-turns.test.ts` "says it is waking on every state that is not running, whoever the wake belongs to".
#[test]
fn ts_it_says_waking_on_every_state_that_is_not_running() {
    for (name, said) in [
        ("napping for send, the wake answers running", 1),
        ("pausing for send, the wake answers running", 1),
        ("waking for send, the wake answers running", 1),
        ("queued for send, the wake answers running", 1),
        ("running for send, the wake answers running", 0),
    ] {
        run("awake", name);
        assert_eq!(recorded_case("awake", name)["said"].as_array().map_or(0, Vec::len), said, "{name}: lines said");
    }
}

/// A gone workspace is refused with the verb's own action word before any frame is sent or any line said.
#[test]
fn awake_gone_is_refused_before_any_frame() {
    for name in ["gone, with the provider's words", "gone, with no words", "gone, with empty words", "gone, a name with spaces and an empty action"] {
        run("awake", name);
        let c = recorded_case("awake", name);
        assert_eq!(c["frames"], json!([]), "{name}");
        assert!(c["said"].as_array().unwrap().is_empty(), "{name}");
        assert!(c["error"].get("kind").is_none(), "{name}: the gone refusal is a plain error");
    }
    run("mergedIn", "a gone lead is refused before any frame");
}

/// A released sub-thread has no machine to wake: its send makes a new one, and any other action wakes it.
#[test]
fn awake_released_send_asks_nothing_but_any_other_action_wakes() {
    run("awake", "released for send, the wake answers running");
    run("awake", "released for merge in, the wake answers running");
    assert_eq!(recorded_case("awake", "released for send, the wake answers running")["frames"], json!([]));
    assert_eq!(recorded_case("awake", "released for merge in, the wake answers running")["value"]["woke"], json!(false));
}

/// The disk line is told, and kept as `said`, only where the machine was down before the wake (paused or pausing).
#[test]
fn awake_tells_the_disk_line_only_from_a_down_state() {
    for (name, told, lines) in [
        ("napping with its disk gone, the wake answers running", true, 2),
        ("pausing with its disk gone, the wake answers running", true, 2),
        ("waking with its disk gone, the wake answers running", false, 1),
        ("running with its disk gone, the wake answers running", false, 0),
        ("napping with its disk gone, the wake leaves it napping", true, 2),
        ("napping with an empty disk line", true, 2),
    ] {
        run("awake", name);
        let c = recorded_case("awake", name);
        assert_eq!(c["value"].get("said").is_some(), told, "{name}");
        assert_eq!(c["said"].as_array().unwrap().len(), lines, "{name}");
    }
}

/// `verbs-turns.test.ts` "naps it where nothing else is running there".
#[test]
fn ts_a_dead_launch_naps_the_machine_where_nothing_else_runs() {
    run("napAfterDeadLaunch", "naps it where nothing else is running there");
    let c = recorded_case("napAfterDeadLaunch", "naps it where nothing else is running there");
    assert_eq!(c["frames"][1], json!("{\"workspaceId\":\"ws_1\",\"id\":3,\"op\":\"workspaces.nap\"}"));
    assert_eq!(c["value"], json!("alpha is asleep again"));
}

/// `verbs-turns.test.ts` "says the machine is up with no countdown where the host answers no nap time for it", and
/// "answers the same where the host cannot be reached at all, so the failure the run is reporting is never lost to a
/// second one".
#[test]
fn ts_a_dead_launch_with_no_nap_time_or_no_host_says_stays_awake() {
    run("napAfterDeadLaunch", "leaves it up where another thread is working, with no nap time to say");
    run("napAfterDeadLaunch", "answers the same where the host cannot be reached at all");
    run("napAfterDeadLaunch", "the host closes during the thread read");
    run("napAfterDeadLaunch", "the nap refused");
}

/// A wake that this call did not make, or a turn the agent refused, owes the machine nothing and asks the host
/// nothing; a thread that did work is no dead launch.
#[test]
fn nap_after_dead_launch_owes_nothing_to_a_wake_it_did_not_make_or_a_turn_that_ran() {
    for name in ["a wake this call did not make owes nothing", "a turn the agent refused ran, so nothing is owed", "a thread whose turn did work is not a dead launch", "a thread of the turn that is still running did work"] {
        run("napAfterDeadLaunch", name);
        let c = recorded_case("napAfterDeadLaunch", name);
        assert!(c["value"].is_null() || c.get("value").is_none(), "{name}");
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn workspace(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "machineId": "m-1", "phase": "running", "kind": "cloud", "golden": "snap-g1", "createdAt": "2026-09-27T01:02:03.004Z", "project": { "id": "proj-1", "name": "alpha", "path": "/root/alpha", "computer": "place-9" } })
}

fn listing(id: &str, idle_at: Option<i64>) -> Value {
    let mut row = workspace(id, "alpha");
    let o = row.as_object_mut().unwrap();
    o.insert("machineState".into(), json!("running"));
    o.insert("size".into(), json!({ "cpu": 2, "memMb": 4096 }));
    o.insert("rateUsdPerHour".into(), json!(0.018));
    if let Some(at) = idle_at {
        o.insert("idleAt".into(), json!(at));
    }
    o.insert("reach".into(), json!({ "state": "reachable" }));
    row
}

fn ok(fields: Value) -> String {
    let mut frame = json!({ "id": 1, "ok": true });
    frame.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    frame.to_string()
}

/// `verbs-turns.test.ts` "leaves it up where another thread is working there, and says when the idle window takes it":
/// the countdown reads the clock, so it is a hand test. Half a minute past each whole reading, so the minutes the
/// line says do not turn on how long the test takes. `fmtUptime` reads whole minutes, then `Xh Ym`, then `Xd Yh`.
#[tokio::test]
async fn ts_a_dead_launch_says_when_the_idle_window_takes_the_machine() {
    let minute = 60_000i64;
    let table = [
        (0, "0m"),
        (minute, "1m"),
        (20 * minute, "20m"),
        (59 * minute, "59m"),
        (60 * minute, "1h 0m"),
        (23 * 60 * minute + 59 * minute, "23h 59m"),
        (24 * 60 * minute, "1d 0h"),
        (49 * 60 * minute, "2d 1h"),
    ];
    for (offset, said) in table {
        let mut replies: IndexMap<String, String> = IndexMap::new();
        replies.insert("sessions.list".into(), ok(json!({ "sessions": [
            { "id": "s_1", "workspaceId": "ws_1", "harness": "claude", "status": "failed", "threadId": "t_1" },
            { "id": "s_2", "workspaceId": "ws_1", "harness": "claude", "status": "running", "threadId": "t_other" }
        ] })));
        replies.insert("status.list".into(), ok(json!({ "statuses": [listing("ws_1", Some(now_ms() + offset + 30_000))] })));
        let log = Arc::new(Mutex::new(Vec::new()));
        let host = Socket(ScriptedHost::new(Script { replies, ..Script::default() }, log.clone(), Arc::new(Mutex::new(Vec::new()))));
        let woken = Woken { workspace: serde_json::from_value(workspace("ws_1", "alpha")).unwrap(), woke: true, said: None };
        let line = w::nap_after_dead_launch(&host, &woken, None).await;
        assert_eq!(line.as_deref(), Some(format!("alpha stays awake, naps in {said}").as_str()), "{offset} ms to go");
        let frames = log.lock().unwrap().clone();
        assert_eq!(frames, vec!["{\"workspaceId\":\"ws_1\",\"id\":2,\"op\":\"sessions.list\"}".to_string(), "{\"id\":3,\"op\":\"status.list\"}".to_string()], "the nap is never asked while another thread works");
    }
}

/// `withLine` keeps the exit class: the kind is the one stamp it carries over, so a usage refusal stays a usage
/// refusal with a line more, and a plain error stays the provider's.
#[test]
fn with_line_keeps_the_exit_class_of_the_error_it_adds_to() {
    let usage = w::with_line(HostError::with_kind("bad flag", Kind::Usage), Some("more"));
    assert_eq!(usage.message, "bad flag\nmore");
    assert_eq!(usage.kind.as_ref().map(Kind::class), Some(ExitClass::Usage));
    let missing = w::with_line(HostError::with_kind("no thread zzz", Kind::NotFound), Some("tell the person"));
    assert_eq!(missing.kind, Some(Kind::NotFound));
    let plain = w::with_line(HostError::new("it failed"), Some("more"));
    assert!(plain.kind.is_none());
    let mut fixed = HostError::with_kind("x", Kind::Usage);
    fixed.fix = Some("Name one.".into());
    assert!(w::with_line(fixed, Some("y")).fix.is_none(), "the fix is not carried over");
    let untouched = w::with_line(HostError::with_kind("same", Kind::Auth), None);
    assert_eq!((untouched.message.as_str(), untouched.kind), ("same", Some(Kind::Auth)));
}

/// A row from before threads is refused in its own words before any frame is dialled; a check is sent only where true.
#[test]
fn forget_thread_refuses_a_row_without_a_thread_id_before_any_frame() {
    run("forgetThread", "a row from before threads is refused before any frame");
    run("forgetThread", "a row from before threads, a check asked");
    run("forgetThread", "a check asked");
    run("forgetThread", "a check not asked is not sent");
    assert_eq!(recorded_case("forgetThread", "a row from before threads is refused before any frame")["frames"], json!([]));
}

/// Settle and restore name threads by their row id, never by their thread id; stop names the thread's latest turn.
#[test]
fn threads_are_named_to_the_host_by_the_id_each_op_takes() {
    run("settleThreads", "the row id is sent, not the thread id");
    run("restoreThreads", "the row id is sent");
    run("forgetThread", "the thread id, not the row id, is sent");
    run("stop", "the latest turn's session id is the one stopped");
    run("rename", "the session id of the thread's latest turn is sent");
}

/// Nothing is asked of the host that a refusal before it forbids: an unknown or ambiguous thread asks no stop, a
/// word outside the access list asks no start, a draft with no message asks no commit.
#[test]
fn refusals_before_the_host_send_no_later_frame() {
    run("stop", "no such thread asks no stop");
    run("stop", "a prefix naming two asks no stop");
    run("startedFrom", "an access that is no word is refused before any frame");
    run("startedFrom", "an empty access is no word");
    run("startedFrom", "an access in capitals is no word");
    run("committed", "no message, no draft, with the note");
    run("committed", "no message, no draft, no note");
    run("reviewPosted", "the draft edit refused asks no post");
    run("mergedIn", "the wake refused by the host asks no merge");
    run("renameWorkspace", "no such workspace asks no rename");
}
