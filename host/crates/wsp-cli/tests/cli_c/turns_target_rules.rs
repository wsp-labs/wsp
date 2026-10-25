// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written tests of `turns_target`: the rules the recorded cases do not state on their own, each named for what it
//! protects. The recorded cases (`turns_target.rs`) hold the Rust to the TS value for value; these hold the invariants
//! (host invariant 7 with the cloud off, the guest rule), the order a machine is made in, and tie each ported TS test to its
//! recorded case. Spec: `spec/turns-target.md`, whose rule table names these tests.
use wsp_cli::verbs::turns_help as t;

use super::common::{case, cases, holds, Case};
use super::drivers_turns_target as d;

const UNIT: &str = "turns_target";

fn recorded(func: &str, name: &str) -> Case {
    case(UNIT, func, name, "<cwd>")
}

fn driver(func: &str) -> super::common::Driver {
    match func {
        "runTarget" => d::run_target,
        "projectOfFolder" => d::project_of_folder,
        "forkFor" => d::fork_for,
        "subFor" => d::sub_for,
        "threadHere" => d::thread_here,
        "subDeleting" => d::sub_deleting,
        "refuseMachineThread" => d::refuse_machine_thread,
        "threadAt" => d::thread_at,
        "childOf" => d::child_of,
        "sharingWith" => d::sharing_with,
        "threadDeleted" => d::thread_deleted,
        other => panic!("no driver for {other}"),
    }
}

fn run(func: &str, name: &str) {
    holds(UNIT, func, name, driver(func));
}

fn ops(c: &Case) -> Vec<String> {
    c.frames
        .iter()
        .filter_map(|f| serde_json::from_str::<serde_json::Value>(f).ok())
        .filter_map(|v| v["op"].as_str().map(String::from))
        .collect()
}

fn guest_line() -> String {
    recorded("runTarget", "run-beside: still names the workspace on a line from a machine that is no thread's").error.expect("a recorded refusal").message
}

fn has_token(c: &Case) -> bool {
    ["WSP_TURN", "WSP_HOST_TOKEN"].iter().any(|k| c.env.get(*k).is_some_and(|v| !v.is_empty()))
}

/// Host invariant 7 (with the cloud off no page, tool or skill line names a cloud): with the cloud off `run_target`,
/// `thread_at` and `child_of` never ask for a machine, never answer one and never name one in a refusal; a word that
/// names a machine is only a word that names no project or no thread. Every recorded cloud-off case is held to it, and
/// to the TS value, in Rust.
#[test]
fn cloud_off_never_names_a_cloud() {
    let mut seen = 0;
    for c in cases(UNIT).into_iter().filter(|c| !c.cloud && matches!(c.func.as_str(), "runTarget" | "threadAt" | "childOf")) {
        assert!(!ops(&c).iter().any(|op| op == "workspaces.resolve"), "{} / {}: a machine was asked for with the cloud off", c.func, c.case);
        if let Some(e) = &c.error {
            assert!(!e.message.contains(" is a machine"), "{} / {}: {}", c.func, c.case, e.message);
            assert!(!e.message.contains("no workspace"), "{} / {}: {}", c.func, c.case, e.message);
        }
        if let Some(v) = &c.value {
            assert!(v.get("workspace").is_none() || c.func != "runTarget", "{} / {}: a machine was answered with the cloud off", c.func, c.case);
        }
        run(&c.func, &c.case);
        seen += 1;
    }
    assert!(seen > 100, "{seen}");
}

/// The same words with the cloud on do name a machine, so the silence above is the setting and not the data.
#[test]
fn cloud_on_names_a_machine() {
    for name in ["a word naming a machine, cloud on", "a word naming a machine, with a branch, cloud on"] {
        let c = recorded("runTarget", name);
        assert!(c.cloud);
        assert!(ops(&c).iter().any(|op| op == "workspaces.resolve"), "{name}");
        run("runTarget", name);
    }
    let c = recorded("threadAt", "a machine's name, cloud on");
    assert!(c.error.expect("a refusal").message.contains("attic-work is a machine"));
    run("threadAt", "a machine's name, cloud on");
    run("childOf", "a child that is a machine, with the cloud");
}

/// Guest rule: a line from a machine (`elsewhere`) that is no thread's names the project it means and is refused before
/// anything is read: no frame is sent, and the folder it was typed in is never looked at.
#[test]
fn elsewhere_run_target_refuses_before_any_read() {
    let want = guest_line();
    let mut seen = 0;
    for c in cases(UNIT).into_iter().filter(|c| c.func == "runTarget" && c.args.get("elsewhere").and_then(|v| v.as_bool()) == Some(true) && c.args.get("ref").is_none_or(|v| v.is_null()) && !has_token(c)) {
        assert!(c.frames.is_empty(), "{}: the host was asked something", c.case);
        assert_eq!(c.error.as_ref().expect("a refusal").message, want, "{}", c.case);
        run("runTarget", &c.case);
        seen += 1;
    }
    assert!(seen >= 4, "{seen}");
}

/// A line out of a thread carries the thread's token and runs beside that thread: nothing is asked, whatever computer the
/// line was typed on, and the token beats the guest rule.
#[test]
fn a_thread_token_runs_beside_the_thread_and_asks_nothing() {
    let mut seen = 0;
    for c in cases(UNIT).into_iter().filter(|c| c.func == "runTarget" && c.args.get("ref").is_none_or(|v| v.is_null()) && has_token(c)) {
        assert!(c.frames.is_empty(), "{}", c.case);
        assert!(c.value.as_ref().is_some_and(|v| v.get("here").is_some()), "{}", c.case);
        run("runTarget", &c.case);
        seen += 1;
    }
    assert!(seen >= 12, "{seen}");
}

/// A named project is read before the token is: a ref a thread gives names its own project, whichever computer another
/// project of the same name is on.
#[test]
fn a_ref_is_read_before_the_token() {
    for name in ["a project here, a thread asking: the ref is read before the token", "run-beside: takes the thread's own project where a project on another computer shares its name"] {
        assert!(!recorded("runTarget", name).frames.is_empty(), "{name}");
        run("runTarget", name);
    }
}

/// A machine is made last: no read of where a run goes, and no refusal of the line, ever sends `workspaces.create`; `fork_for`
/// sends it after the names taken, the landing and the subscription, in that order.
#[test]
fn a_machine_is_made_after_every_read() {
    for c in cases(UNIT).into_iter().filter(|c| c.func == "runTarget") {
        assert!(!ops(&c).iter().any(|op| op == "workspaces.create"), "{}: run_target made a machine", c.case);
    }
    let mut made = 0;
    for c in cases(UNIT).into_iter().filter(|c| c.func == "forkFor") {
        let got = ops(&c);
        if got.iter().any(|op| op == "workspaces.create") {
            assert_eq!(got, ["workspaces.list", "workspaces.landing", "events.subscribe", "workspaces.create"], "{}", c.case);
            made += 1;
        }
    }
    assert!(made >= 20);
    for c in cases(UNIT).into_iter().filter(|c| c.func == "subFor") {
        let got = ops(&c);
        if got.iter().any(|op| op == "workspaces.create") {
            assert_eq!(got, ["workspaces.list", "events.subscribe", "workspaces.create"], "{}", c.case);
        }
    }
}

/// The name a machine takes: off the task (first line, five words, forty characters), numbered where taken, as the
/// app and `wsp start` number one.
#[test]
fn a_machine_is_named_off_the_task_and_a_taken_name_is_numbered() {
    let sent = |c: &Case| c.frames.iter().find(|f| f.contains("\"op\":\"workspaces.create\"")).cloned().expect("a create");
    for (name, want) in [
        ("a machine named off the task", "\"name\":\"fix the login page before\""),
        ("the same task again is numbered", "\"name\":\"fix the login page before 2\""),
        ("the number goes up past the taken ones", "\"name\":\"fix the login page before 3\""),
        ("a name taken in other words is not taken", "\"name\":\"fix the login page before\""),
        ("a task of five words and a sixth", "\"name\":\"one two three four five\""),
        ("only the first line names it", "\"name\":\"first line here\""),
    ] {
        assert!(sent(&recorded("forkFor", name)).contains(want), "{name}");
        run("forkFor", name);
    }
    let cut = "a".repeat(39);
    assert!(sent(&recorded("forkFor", "the name is cut at forty characters, where the cut lands on a space")).contains(&format!("\"name\":\"{cut}\"")));
    assert!(sent(&recorded("forkFor", "the cut name taken is numbered")).contains(&format!("\"name\":\"{cut} 2\"")));
    run("forkFor", "the name is cut at forty characters, where the cut lands on a space");
    run("forkFor", "the cut name taken is numbered");
}

/// `sub` of a sub-thread's create is written on, size, from, branch, harness whatever order the ask was made in.
#[test]
fn sub_for_writes_the_sub_in_one_order() {
    let name = "every field of the ask, in the frame's order whatever the ask's";
    let c = recorded("subFor", name);
    let create = c.frames.iter().find(|f| f.contains("\"workspaces.create\"")).expect("a create");
    assert!(create.contains("\"sub\":{\"on\":\"boat\",\"size\":\"big\",\"from\":\"feat/up\",\"branch\":\"feat/x\",\"harness\":\"codex\"}"), "{create}");
    run("subFor", name);
}

/// A sub-thread's stages are heard until `stop`, and not after it.
#[test]
fn sub_for_stops_listening_when_stopped() {
    let heard = recorded("subFor", "a stage after the answer is said until stop");
    assert_eq!(heard.said, ["late"]);
    let stopped = recorded("subFor", "a stage after the answer is not said once stop is called");
    assert!(stopped.said.is_empty());
    run("subFor", "a stage after the answer is said until stop");
    run("subFor", "a stage after the answer is not said once stop is called");
}

/// The deepest project holding a folder wins; where two hold it at one depth the first listed does.
#[test]
fn project_of_folder_takes_the_deepest_and_then_the_first() {
    for name in ["the deepest of two holding projects wins", "the deepest wins whichever is listed first", "two at one path: the first listed", "a project on another computer never holds a folder"] {
        run("projectOfFolder", name);
    }
}

/// A thread on a machine goes with its machine: refused in the runtime's words, a usage error with no fix of its own.
#[test]
fn a_machine_thread_is_refused_as_a_usage_error_with_no_fix() {
    let name = "a thread on a machine is refused with the machine's name";
    let e = recorded("refuseMachineThread", name).error.expect("a refusal");
    assert_eq!(e.kind.as_deref(), Some("usage"));
    assert_eq!(e.fix, None);
    assert_eq!(e.message, "a thread on attic-work goes with its machine; wsp delete attic-work takes both");
    run("refuseMachineThread", name);
}

/// `sessions.delete` is sent `threadId`, `processes`, `check` in that order, and only what was asked.
#[test]
fn thread_deleted_sends_the_thread_then_what_was_asked() {
    let sent = |name: &str| recorded("threadDeleted", name).frames.into_iter().next().expect("a delete");
    assert_eq!(sent("with its worktree and two threads"), "{\"threadId\":\"t-3\",\"id\":2,\"op\":\"sessions.delete\"}");
    assert_eq!(sent("the processes ended"), "{\"threadId\":\"t-3\",\"processes\":\"stop\",\"id\":2,\"op\":\"sessions.delete\"}");
    assert_eq!(sent("a check"), "{\"threadId\":\"t-3\",\"check\":true,\"id\":2,\"op\":\"sessions.delete\"}");
    assert_eq!(sent("processes and a check"), "{\"threadId\":\"t-3\",\"processes\":\"stop\",\"check\":false,\"id\":2,\"op\":\"sessions.delete\"}");
    for name in ["with its worktree and two threads", "the processes ended", "a check", "processes and a check"] {
        run("threadDeleted", name);
    }
}

/// `thread_at` is the entry the other units call; it reads the cloud setting from the process, which `thread_at_in` is given.
#[test]
fn thread_at_keeps_the_signature_the_other_units_call() {
    let _ = t::thread_at;
    let _ = t::thread_at_in;
}

/// The TS tests that call these helpers directly, each as the recorded cases that hold its intent (the names are the
/// fixture's: a rename there breaks this list). `verbs-turns.test.ts` runs on a fake runtime and cannot run here; its
/// intents are restated as recorded cases of `fork_for`, `run_target` and `checked_start`.
#[test]
fn ported_ts_tests_are_recorded_cases() {
    let ported: [(&str, &str, &str); 12] = [
        ("run-beside.test.ts: starts beside the thread whose line it is (1)", "runTarget", "run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (no folder named)"),
        ("run-beside.test.ts: starts beside the thread whose line it is (2)", "runTarget", "run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (a folder named)"),
        ("run-beside.test.ts: still names the workspace on a line from a machine that is no thread's", "runTarget", "run-beside: still names the workspace on a line from a machine that is no thread's"),
        ("run-beside.test.ts: takes the thread's own project where a project on another computer shares its name", "runTarget", "run-beside: takes the thread's own project where a project on another computer shares its name"),
        ("verbs-turns.test.ts: a project elsewhere forks a machine named off the task", "forkFor", "a machine named off the task"),
        ("verbs-turns.test.ts: the same task again is a second machine, numbered", "forkFor", "the same task again is numbered"),
        ("verbs-turns.test.ts: numbers a taken name even where the cut of the task lands on a space (first)", "forkFor", "the name is cut at forty characters, where the cut lands on a space"),
        ("verbs-turns.test.ts: numbers a taken name even where the cut of the task lands on a space (second)", "forkFor", "the cut name taken is numbered"),
        ("verbs-turns.test.ts: a project elsewhere reads the whole line before it forks (the target is read, nothing made)", "runTarget", "a project on a cloud forks a machine"),
        ("verbs-turns.test.ts: a project elsewhere with a machine of its name", "runTarget", "a project on a cloud with a machine of its name: the machine with a cloud, the fork without, cloud on"),
        ("verbs-turns.test.ts: a refusal costs no machine", "forkFor", "the landing refused costs no stage and no machine"),
        ("verbs-turns.test.ts: the create refused after a stage", "forkFor", "the create refused after a stage"),
    ];
    for (ts, func, name) in ported {
        assert_eq!(recorded(func, name).func, func, "{ts}");
        run(func, name);
    }
}
