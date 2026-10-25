// SPDX-License-Identifier: AGPL-3.0-only
//! A workspace's machine moves: nap, wake (asking the provider again on its cadence, a deleted disk replaced or a
//! sub-thread released, a stop the person pulls), resize, upgrade, rebuild, the record's own changes, the daemon,
//! delete and forget. spec/workspaces_phases.md; workspaces.ts:22-65 and :289-645 at 65f6b89c7. One test per row of the
//! spec's "Rules against tests" table, then the strong cases of the TS tests the test map lists (wake-asks-again,
//! disk-gone, pause, box-nap, disk-watch, forget, reconcile, runtime, folder-threads) that those names do not cover,
//! each naming the file:line it ports. Every sentence is the TS protocol function's own text, written out, since
//! wsp-words' bodies are not written yet. The wake's clock is the `TestClock`: `sleeps` waits on it.
#[path = "support/base.rs"]
mod base;
#[path = "support/phases.rs"]
mod phases_support;

use std::rc::Rc;

use base::{err_answer, kinded, plain, project, rec, rec_json, record, run, view_of};
use futures::future::FutureExt;
use futures::channel::oneshot;
use phases_support::{
    alive, asker, caps_with_sizes, disk_gone_error, done, format_seconds, missing, other, refusal_line, restore_unfinished, shape, size,
    the_person, unanswered, FakePhases, WakePlan,
};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use wsp_account::projects::workspaces::phases::{
    delete, disk_gone, forget, look, nap, rebuild, rename, resize, restart_daemon, stop_wake, touch, update_daemon, upgrade, wake, MachineRead,
    PhasesCtx, RESIZE_NAP_MS,
};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::types::wiring::DeleteSaid;
use wsp_wire::types::workspace_look::{WorkspaceGlyph, WorkspaceLook};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_words::exit::{HostError, Kind};

const EVERY: i64 = 60_000;
const WAKE_STOPPED: &str = "waking stopped; the machine is still paused";
const DELETED_REASON: &str = "machine deleted while the agent was working";
const BOAT_LINE: &str = "Boat deleted this machine's disk once it had been paused for 14 days, so it cannot wake";
const SAID: &str = "The sandbox stayed stopped for 14 days. Boat deleted its data.";
const THEME: &str = r#"{"dots":[{"angle":10,"radius":0.5}],"harmony":"single","grain":0.5,"opacity":0.8,"mode":"auto"}"#;

struct H {
    f: Rc<FakePhases>,
    ctx: Rc<dyn PhasesCtx>,
}

impl H {
    fn new() -> H {
        let f = FakePhases::new();
        f.base.hold_project(project("pr_cloud", "spoo", "solari", json!({})));
        f.base.hold_project(project("pr_here", "wsp", "here", json!({})));
        let ctx = f.ctx();
        H { f, ctx }
    }
    /// A napping fork named `a` on machine `m_a`, over `over`.
    fn cloud(&self, id: &str, over: Value) -> Entry {
        let mut o = json!({ "name": "a", "machineId": "m_a", "phase": "napping" });
        base::merge(&mut o, over);
        self.f.base.hold(record(id, "cloud", "pr_cloud", o))
    }
    fn local(&self, id: &str, over: Value) -> Entry {
        let mut o = json!({ "name": "a", "machineId": "m_a", "phase": "running" });
        base::merge(&mut o, over);
        self.f.base.hold(record(id, "local", "pr_here", o))
    }
    fn calls(&self, of: &[&str]) -> Vec<String> {
        self.f.base.names_of(of)
    }
    fn events(&self) -> Vec<Value> {
        self.f.base.events.borrow().clone()
    }
    fn persisted(&self) -> Vec<Value> {
        self.f.base.persisted.borrow().iter().map(|r| serde_json::to_value(r).unwrap()).collect()
    }
    fn writes(&self) -> Vec<String> {
        self.f.base.store.writes.borrow().clone()
    }
    fn args(&self, name: &str) -> Vec<Value> {
        self.f.base.args_of(name)
    }
    fn asks(&self, for_ms: i64, every_ms: i64) {
        self.f.base.answer_with("resume_asks", vec![json!({ "forMs": for_ms, "everyMs": every_ms })]);
    }
    fn now(&self) -> i64 {
        use wsp_core::clock::Clock;
        base::ClockOf(self.f.base.clock.clone()).now()
    }
    fn clock(&self) -> &base::TestClock {
        &self.f.base.clock
    }
    fn wake_task(&self, id: &'static str) -> JoinHandle<Result<WorkspaceView, HostError>> {
        let ctx = self.ctx.clone();
        tokio::task::spawn_local(async move { wake(&ctx, id, Some(the_person())).await })
    }
    /// Moves the clock a cadence at a time until the task ends.
    async fn pump<T>(&self, task: &JoinHandle<T>) {
        for _ in 0..300 {
            settle().await;
            if task.is_finished() {
                return;
            }
            if self.clock().pending() > 0 {
                self.clock().advance(EVERY);
            }
        }
        panic!("the task did not end");
    }
}

async fn settle() {
    for _ in 0..30 {
        tokio::task::yield_now().await;
    }
}

fn expected(over: Value) -> Value {
    let mut o = json!({ "name": "a", "machineId": "m_a", "phase": "napping" });
    base::merge(&mut o, over);
    serde_json::to_value(record("ws_a", "cloud", "pr_cloud", o)).unwrap()
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn view_json(h: &H, entry: &Entry) -> Value {
    json_of(&view_of(&h.f.base, &rec(entry)))
}

fn joined_error(rx: oneshot::Receiver<Result<WorkspaceView, HostError>>) -> wsp_core::types::wiring::Joined<WorkspaceView> {
    async move { rx.await.unwrap() }.boxed_local().shared()
}

fn fields_clear(entry: &Entry) {
    let e = entry.borrow();
    assert!(e.waking.is_none(), "waking is dropped");
    assert!(e.wake_stop.is_none(), "wakeStop is dropped");
    assert!(e.wake_said.is_none(), "wakeSaid is dropped");
    assert!(e.wake_ask.is_none(), "wakeAsk is dropped");
}

fn gone_refusal(name: &str, action: &str, words: &str) -> String {
    format!("{name}'s machine is gone with its disk, so work that was not pushed is lost; rebuild it to {action}, which brings back its home folder from the last saved nap ({words})")
}

fn gave_up(asks: &str, over: &str) -> String {
    format!("the provider answered none of {asks} over {over}; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image")
}

fn replaced_line() -> String {
    format!("{BOAT_LINE}; it runs on a new machine from the image now")
}

fn sub_json() -> Value {
    json!({ "on": "box", "repo": "acme/x", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/work/x" })
}

// ------------------------------------------------------------------------------------------------------ rule 2

// disk-gone.test.ts:99 and the sub-thread's release (workspaces.ts:36-46)
#[test]
fn disk_gone_releases_a_sub_thread() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "sub": sub_json() }));
        let warned = wsp_core::console::capture();
        let line = format!("{BOAT_LINE}; the next send starts a new machine from the image");
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        assert_eq!(err, kinded(&line, Kind::Conflict));
        assert_eq!(rec(&entry).disk_gone.as_deref(), Some(line.as_str()));
        assert_eq!(h.args("unfork"), vec![json!({ "id": "ws_a" })]);
        assert_eq!(h.args("sub_lost"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(h.args("tell_threads"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(h.calls(&["unfork", "sub_lost", "tell_threads", "ws_rebuild"]), ["unfork", "sub_lost", "tell_threads"]);
        assert_eq!(warned.lines(), [format!("wake of ws_a: m_a cannot resume: {SAID}")]);
        assert!(h.events().is_empty());
        // A delete the provider refuses is warned and the release goes on.
        h.f.unforks.set(vec![Err(other("the provider said no"))]);
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        assert_eq!(err, kinded(&line, Kind::Conflict));
        assert_eq!(
            warned.lines()[2],
            "wake of ws_a: m_a was not deleted: the provider said no"
        );
        assert_eq!(h.args("sub_lost").len(), 2);
    });
}

// disk-gone.test.ts:45 "starts a new machine from the image in the same wake, retires the old one and says the disk is gone"
#[test]
fn disk_gone_rebuilds_from_the_image_and_says_replaced() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "wakeRefused": "an old refusal" }));
        let warned = wsp_core::console::capture();
        let view = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap();
        let line = replaced_line();
        let record = expected(json!({ "phase": "running", "machineId": "m_new", "diskGone": line }));
        assert_eq!(rec_json(&entry), record);
        assert_eq!(h.persisted(), vec![record]);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" })]);
        assert_eq!(h.args("tell_threads"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(h.calls(&["ws_rebuild", "follow_machine", "persist", "emit", "tell_threads", "reach_of", "emit_status"]), [
            "ws_rebuild", "follow_machine", "persist", "emit", "tell_threads", "reach_of", "emit_status"
        ]);
        let status = &h.args("emit_status")[0];
        assert_eq!((&status["reach"], &status["words"]), (&json!("reachable"), &json!(line)));
        assert_eq!(
            warned.lines(),
            [format!("wake of ws_a: m_a cannot resume: {SAID}"), "wake of ws_a: m_a replaced by m_new".to_owned()]
        );
    });
}

// disk-gone.test.ts:76 "a new machine that does not start leaves the record napping on the old one and says both"
#[test]
fn disk_gone_with_no_new_machine_keeps_the_phase_and_says_why() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "waking" }));
        h.f.rebuilds.set(vec![Err(other("no machine"))]);
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        let words = format!("{BOAT_LINE}, and no new machine started: no machine");
        assert_eq!(err, plain(&words));
        assert_eq!(h.persisted(), vec![expected(json!({}))]);
        assert_eq!(rec(&entry).machine_id, "m_a");
        let status = &h.args("emit_status")[0];
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(words)));
        assert_eq!(h.calls(&["follow_machine", "emit", "tell_threads"]), Vec::<String>::new());
        assert!(h.events().is_empty());
    });
}

// ------------------------------------------------------------------------------------------------------ rule 3

// pause.test.ts:61 "nap and wake are refused with the provider's own sentence on a backend without a pause mode"
#[test]
fn nap_refuses_a_pauseless_kind_and_naps() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.answer_with("refuse_pauseless", vec![err_answer("a is this computer, and it cannot be paused on the provider it runs on", Some("conflict"))]);
        let err = nap(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a is this computer, and it cannot be paused on the provider it runs on", Kind::Conflict));
        assert_eq!(h.args("refuse_pauseless"), vec![json!({ "id": "ws_a", "act": "be paused" })]);
        assert!(h.args("nap_with").is_empty());
        h.f.base.script.borrow_mut().shift_remove("refuse_pauseless");
        let view = nap(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Napping);
        assert_eq!(h.args("nap_with"), vec![json!({ "id": "ws_a", "napWaitMs": null })]);
        assert_eq!(h.calls(&["spawn_guard", "entry_of", "refuse_pauseless", "nap_with"])[4..], ["spawn_guard", "entry_of", "refuse_pauseless", "nap_with"]);
        assert_eq!(h.args("spawn_guard")[0], json!({ "act": "pause", "scoped": false }));
        // A guard that refuses ends it before the entry is read.
        h.f.base.answer_with("spawn_guard", vec![err_answer("the cap", Some("conflict"))]);
        let err = nap(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("the cap", Kind::Conflict));
        assert_eq!(h.args("nap_with").len(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------- rules 4.1-4.7

#[test]
fn wake_of_a_tree_workspace_answers_talked_fields() {
    run(async {
        let h = H::new();
        h.f.base.hold(record("ws_far", "place", "pr_cloud", json!({ "name": "far", "phase": "running", "folder": "/srv/far", "home": "/home/far" })));
        h.f.base.answer_with("talks_to_tree_on", vec![json!(true)]);
        let found = wake(&h.ctx, "ws_far", Some(asker("t_lead1234567", "ws_lead", "t_root"))).await.unwrap();
        let mut full = json_of(&view_of(&h.f.base, &rec(&h.f.base.live.borrow()["ws_far"])));
        let keep = ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "gone"];
        full.as_object_mut().unwrap().retain(|k, _| keep.contains(&k.as_str()));
        assert_eq!(json_of(&found), full);
        assert!(!json_of(&found).as_object().unwrap().contains_key("folder"));
        // The person's own wake of it is the one that ran, with no scope.
        assert_eq!(h.args("entry_of"), vec![json!({ "id": "ws_far", "thread": null, "act": "wake", "scoped": false })]);
        assert_eq!(h.args("talks_to_tree_on").len(), 2);
    });
}

// pause.test.ts:123 "a wake asked during the pause waits for it and then wakes; a second nap joins the first"
#[test]
fn wake_joins_a_wake_in_flight() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let release = h.f.hold_wake();
        let first = h.wake_task("ws_a");
        settle().await;
        assert!(entry.borrow().waking.is_some(), "the wake is stored for later callers to join");
        assert!(entry.borrow().wake_stop.is_some(), "the stop token is stored");
        let second = h.wake_task("ws_a");
        settle().await;
        assert_eq!(h.args("ws_wake").len(), 1, "one resume for two callers");
        release.send(done(None)).unwrap();
        let (a, b) = (first.await.unwrap().unwrap(), second.await.unwrap().unwrap());
        assert_eq!(a, b);
        assert_eq!(a.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
        assert_eq!(h.events().len(), 1);
        fields_clear(&entry);
    });
}

#[test]
fn a_joined_wake_hands_its_failure_to_every_caller() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let release = h.f.hold_wake();
        let first = h.wake_task("ws_a");
        settle().await;
        let second = h.wake_task("ws_a");
        settle().await;
        release.send(Err(other("boom"))).unwrap();
        let (a, b) = (first.await.unwrap().unwrap_err(), second.await.unwrap().unwrap_err());
        assert_eq!(a, b);
        assert_eq!(a.message, "boom");
        fields_clear(&entry);
    });
}

#[test]
fn wake_of_gone_refuses_unless_recovered() {
    run(async {
        let h = H::new();
        let gone = "machine m_a is gone at the provider";
        let entry = h.cloud("ws_a", json!({ "phase": "gone", "gone": gone }));
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain(&gone_refusal("a", "wake", gone)));
        assert!(h.args("ws_wake").is_empty());
        // Recovered to running: the view, and nothing woken.
        h.f.base.answer_with("recover_gone", vec![json!("running")]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("ws_wake").is_empty());
        // Recovered to napping: the wake goes on and resumes it.
        *entry.borrow().record.borrow_mut() = record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "gone", "gone": gone }));
        h.f.base.script.borrow_mut().shift_remove("recover_gone");
        h.f.base.answer_with("recover_gone", vec![json!("napping")]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
    });
}

// gone.test.ts:696, runtime-gone.test.ts:56 a wake of a running record reads the machine and the record follows the fact
#[test]
fn wake_of_running_reads_the_machine_and_settles_gone() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.states.set(vec![Ok(MachineRead::Gone)]);
        h.f.base.answer_with("settle_gone", vec![json!(true)]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let at = format_seconds(1_000_000);
        assert_eq!(at, "1970-01-01T00:16:40Z");
        assert_eq!(err, plain(&gone_refusal("a", "wake", &format!("machine m_a is gone at the provider: the wake found it gone at {at}"))));
        assert_eq!(h.args("settle_gone"), vec![json!({ "id": "ws_a", "by": "wake", "at": 1_000_000, "answer": null })]);
        // A 404 on the read says what the provider said.
        h.f.base.live.borrow()["ws_a"].borrow().record.borrow_mut().phase = WorkspacePhase::Running;
        h.f.states.set(vec![Err(missing())]);
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain(&gone_refusal("a", "wake", &format!("machine m_a is gone at the provider: the wake found it gone at {at} (the sandbox is gone)"))));
        assert_eq!(h.args("settle_gone")[1]["answer"], json!("the sandbox is gone"));
        assert!(h.args("ws_wake").is_empty());
    });
}

#[test]
fn a_read_that_fails_or_a_gone_that_is_not_settled_leaves_a_running_record_running() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        // Any failure that is not a 404 reads running.
        h.f.states.set(vec![Err(other("socket hang up"))]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("settle_gone").is_empty());
        // A gone reading the sweep does not confirm leaves it as it was.
        h.f.states.set(vec![Ok(MachineRead::Gone)]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("settle_gone").len(), 1);
        assert!(h.args("ws_wake").is_empty());
        assert!(h.persisted().is_empty());
    });
}

// pause.test.ts:488 "a wake asked while the record still says running asks the provider once and resumes a machine it paused"
#[test]
fn wake_of_running_adopts_a_provider_pause() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.states.set(vec![Ok(MachineRead::Paused)]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.calls(&["machine_state", "adopt_pause", "refuse_pauseless", "ws_wake"]), ["machine_state", "adopt_pause", "refuse_pauseless", "ws_wake"]);
        assert_eq!(h.args("refuse_pauseless"), vec![json!({ "id": "ws_a", "act": "be woken" })]);
    });
}

// pause.test.ts:341 "pause on a record that says napping over a machine the provider holds running"
#[test]
fn wake_of_a_napping_record_whose_machine_runs_adopts_running() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("runs_under_napping", vec![json!(true)]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.calls(&["machine_state", "runs_under_napping", "adopt_running", "ws_wake", "refuse_pauseless"]), ["runs_under_napping", "adopt_running"]);
    });
}

#[test]
fn wake_of_running_proves_an_unchecked_machine() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        entry.borrow_mut().unchecked = true;
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.calls(&["machine_state", "proven", "ws_wake"]), ["machine_state", "proven"]);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(!entry.borrow().unchecked);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("proven").len(), 1, "a checked machine is not proven again");
    });
}

// pause.test.ts:123 a wake asked during the pause waits for it (spec 4.4), and a failed nap is no reason to refuse
#[test]
fn wake_waits_for_a_nap_in_flight_and_drops_its_failure() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let (tx, rx) = oneshot::channel();
        entry.borrow_mut().napping = Some(joined_error(rx));
        let task = h.wake_task("ws_a");
        settle().await;
        assert!(h.args("ws_wake").is_empty(), "the wake waits for the nap");
        tx.send(Err(plain("the provider refused the pause"))).unwrap();
        let view = task.await.unwrap().unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
    });
}

#[test]
fn wake_stops_at_a_sub_thread_or_a_blocked_copy() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("refuse_sub_wake", vec![err_answer("a sub-thread wakes by a send", Some("usage"))]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a sub-thread wakes by a send", Kind::Usage));
        h.f.base.script.borrow_mut().shift_remove("refuse_sub_wake");
        h.f.base.answer_with("copy_blocked", vec![err_answer("this computer blocks copies", Some("conflict"))]);
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("this computer blocks copies", Kind::Conflict));
        assert_eq!(h.calls(&["entry_of", "refuse_sub_wake", "copy_blocked", "persist", "ws_wake"]), [
            "entry_of", "refuse_sub_wake", "entry_of", "refuse_sub_wake", "copy_blocked"
        ]);
        assert!(h.persisted().is_empty());
    });
}

// ------------------------------------------------------------------------------------------------------ rule 4.8

// pause.test.ts:357 "wake takes the provider's running as the fact: the record becomes running with the line in"
#[test]
fn wake_resumes_persists_and_says_woken() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "wakeRefused": "an old refusal" }));
        entry.borrow_mut().delete_said = Some(DeleteSaid { phase: WorkspacePhase::Running, line: "a's machine m_a is still running".to_owned() });
        *h.f.engine_machine.borrow_mut() = Some("m_a2".to_owned());
        h.f.wakes.set(vec![WakePlan::Done(done(Some("the guest answered late")))]);
        let warned = wsp_core::console::capture();
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(view.machine_id, "m_a2");
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "waking", "wakeRefused": "an old refusal" })), expected(json!({ "phase": "running", "machineId": "m_a2" }))]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.woken", "workspaceId": "ws_a", "machineId": "m_a2" })]);
        assert_eq!(h.args("ws_wake")[0]["landed"], json!(false));
        let status = h.args("emit_status");
        assert_eq!((&status[0]["reach"], &status[0]["words"], &status[0]["phase"]), (&json!("napping"), &json!(null), &json!("waking")));
        assert_eq!((&status[1]["reach"], &status[1]["words"], &status[1]["phase"]), (&json!("reachable"), &json!("the guest answered late"), &json!("running")));
        assert_eq!(h.calls(&["refuse_pauseless", "persist", "emit_status", "ws_wake", "follow_machine", "emit"]), [
            "refuse_pauseless", "persist", "emit_status", "ws_wake", "follow_machine", "persist", "emit", "emit_status"
        ]);
        assert_eq!(warned.lines(), ["wake of ws_a: the guest answered late"]);
        assert!(entry.borrow().delete_said.is_none(), "deleteSaid is dropped");
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:116 "asks again on its own once a cadence, counting the asks on the row"
#[test]
fn wake_asks_again_on_the_cadence_from_its_start() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(4 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered())), WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        let began = h.now();
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        let view = task.await.unwrap().unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("sleeps").iter().map(|a| a["ms"].clone()).collect::<Vec<_>>(), [json!(EVERY), json!(EVERY)]);
        assert_eq!(began, 1_000_000);
        let asked: Vec<Value> = h.args("emit_status").iter().map(|a| a["wakeAsk"].clone()).collect();
        assert_eq!(asked[..3], [json!(null), json!({ "ask": 1, "of": 4 }), json!({ "ask": 2, "of": 4 })]);
        assert!(h.args("emit_status")[1]["wakeSaid"].is_null(), "wakeSaid is dropped between asks");
        assert_eq!(h.args("ws_wake").iter().map(|a| a["landed"].clone()).collect::<Vec<_>>(), [json!(false), json!(false), json!(false)]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.woken", "workspaceId": "ws_a", "machineId": "m_a" })]);
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:141 "counts the cadence from when the ask began, so a cap that eats half of it does not push the next ask out"
#[test]
fn wake_counts_the_cadence_from_when_the_ask_began() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold, WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        let first = h.f.hold_wake();
        let began = h.now();
        let task = h.wake_task("ws_a");
        settle().await;
        // The provider sits on the call for half the cadence and the cap cuts it off there.
        h.clock().advance(EVERY / 2);
        first.send(Err(unanswered())).unwrap();
        settle().await;
        assert_eq!(h.args("sleeps"), vec![json!({ "ms": EVERY / 2, "now": began + EVERY / 2 })]);
        assert_eq!(h.clock().due(), vec![began + EVERY]);
        h.pump(&task).await;
        task.await.unwrap().unwrap();
        // Ask 2 failed at once, a cadence in: what is left of its cadence is a whole one.
        assert_eq!(h.args("sleeps")[1]["ms"], json!(EVERY));
        assert_eq!(h.args("ws_wake").len(), 3);
    });
}

// wake-asks-again.test.ts:141 the other way round: a call that ran past the cadence is asked again at once
#[test]
fn wake_asks_again_at_once_when_the_ask_outran_the_cadence() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold, WakePlan::Done(done(None))]);
        let first = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        h.clock().advance(EVERY + EVERY / 2);
        first.send(Err(unanswered())).unwrap();
        settle().await;
        assert_eq!(h.args("sleeps")[0]["ms"], json!(0));
        h.clock().advance(1);
        settle().await;
        assert!(task.is_finished());
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_wake").len(), 2);
    });
}

// wake-asks-again.test.ts:207 "reads before it asks again, sends no second resume at one that came up"
#[test]
fn wake_reads_the_machine_before_asking_again() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        h.f.base.answer_with("took_the_resume", vec![json!(true)]);
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_wake").iter().map(|a| a["landed"].clone()).collect::<Vec<_>>(), [json!(false), json!(true)]);
        assert_eq!(h.calls(&["ws_wake", "sleeps", "took_the_resume"]), ["ws_wake", "sleeps", "took_the_resume", "ws_wake"]);
    });
}

// wake-asks-again.test.ts:171 and :245 gives up, and leaves the rebuild road on the record
#[test]
fn wake_gives_up_after_every_ask_and_writes_wake_refused() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let warned = wsp_core::console::capture();
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        let err = task.await.unwrap().unwrap_err();
        let line = gave_up("3 resume requests", "2m");
        assert_eq!(err, plain(&line));
        assert_eq!(h.args("ws_wake").len(), 3);
        assert_eq!(h.args("sleeps").len(), 2);
        let last = h.persisted().pop().unwrap();
        assert_eq!(last, expected(json!({ "phase": "napping", "wakeRefused": line })));
        assert_eq!(rec(&entry).wake_refused.as_deref(), Some(line.as_str()));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"], &status["wakeAsk"]), (&json!("napping"), &json!(line), &json!(null)));
        assert_eq!(warned.lines(), [format!("wake of ws_a failed: {line}")]);
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(h.events().is_empty());
        fields_clear(&entry);
        // A wake that lands takes the road away with the fault that made it.
        h.f.wakes.set(vec![WakePlan::Done(done(None))]);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(rec(&entry).wake_refused, None);
    });
}

// wake-asks-again.test.ts:379 "a backend that declares no asking is asked once"
#[test]
fn wake_with_no_resume_asks_asks_once() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert!(err.message.starts_with("the provider answered none of 1 resume request over "), "{}", err.message);
        assert!(err.message.ends_with("; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image"));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert!(h.args("sleeps").is_empty());
        assert!(h.args("emit_status").iter().all(|a| a["wakeAsk"].is_null()));
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
    });
}

// wake-asks-again.test.ts:280 "stops when the person stops it from the row, while the host waits to ask again"
#[test]
fn wake_stopped_between_asks_answers_wake_stopped_and_notes_paused() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let warned = wsp_core::console::capture();
        let task = h.wake_task("ws_a");
        settle().await;
        assert_eq!(h.clock().pending(), 1, "the host is waiting to ask again");
        let ctx = h.ctx.clone();
        let stop = tokio::task::spawn_local(async move { stop_wake(&ctx, "ws_a", Some(the_person())).await });
        settle().await;
        let left = stop.await.unwrap().unwrap();
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(WAKE_STOPPED)));
        assert_eq!(h.args("ws_note_paused").len(), 1);
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(warned.lines().is_empty(), "a stop is not warned");
        // Nothing asks again after the stop: the cadence's timer is spent with no call behind it.
        assert_eq!(h.clock().pending(), 0);
        h.clock().advance(EVERY * 5);
        settle().await;
        assert_eq!(h.args("ws_wake").len(), 1);
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:306 "stops while the host is on a call by ending it"
#[test]
fn a_stop_while_the_host_is_on_a_call_ends_it() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold]);
        let _never_answered = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        let left = stop_wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert!(h.args("sleeps").is_empty(), "the stop ended the call before any wait");
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!(WAKE_STOPPED));
        fields_clear(&entry);
    });
}

// box-nap.test.ts:552 "a rebuild refuses in one line when a napped workspace cannot be woken" shares the engine's words: noCommandsYetLine
#[test]
fn wake_says_no_commands_yet_for_a_restore_unfinished() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(restore_unfinished("m_a", "still restoring")))]);
        let warned = wsp_core::console::capture();
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let line = "a is up and takes no commands yet (still restoring); wake it again in a minute or two";
        assert_eq!(err, plain(line));
        assert_eq!(warned.lines(), [format!("wake of ws_a failed: {line}")]);
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        assert_eq!(rec(&entry).wake_refused, None);
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(line)));
    });
}

// pause.test.ts:465 "a wake the provider refuses leaves the record paused, not waking, and the next wake tries again"
#[test]
fn a_wake_the_provider_refuses_leaves_the_record_paused_with_the_error_on_the_row() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(other("the provider refused the resume"))), WakePlan::Done(done(None))]);
        let warned = wsp_core::console::capture();
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.message, "the provider refused the resume");
        assert_eq!(warned.lines(), ["wake of ws_a failed: the provider refused the resume"]);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!("the provider refused the resume")));
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(h.events().is_empty());
        fields_clear(&entry);
        assert_eq!(wake(&h.ctx, "ws_a", None).await.unwrap().phase, WorkspacePhase::Running);
    });
}

// gone.test.ts:196 a resume the provider answers 404 for is a sighting like any other
#[test]
fn a_wake_the_provider_answers_404_for_settles_the_record_gone_where_the_reads_agree() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(missing()))]);
        h.f.base.answer_with("settle_gone", vec![json!(true)]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let at = format_seconds(1_000_000);
        assert_eq!(err, plain(&gone_refusal("a", "wake", &format!("machine m_a is gone at the provider: the wake found it gone at {at} (the sandbox is gone)"))));
        assert_eq!(h.args("settle_gone"), vec![json!({ "id": "ws_a", "by": "wake", "at": 1_000_000, "answer": "the sandbox is gone" })]);
        fields_clear(&entry);
        // Not settled: the provider's own error is the answer and the record falls back to what the engine says.
        *entry.borrow().record.borrow_mut() = record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "napping" }));
        h.f.base.script.borrow_mut().shift_remove("settle_gone");
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err.message, "machine not found");
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
    });
}

// disk-gone.test.ts:45 a wake met by a deleted disk goes through disk_gone, in the same wake
#[test]
fn a_wake_met_by_a_deleted_disk_replaces_the_machine_in_the_same_wake() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(disk_gone_error("m_a", BOAT_LINE, SAID)))]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(view.disk_gone.as_deref(), Some(replaced_line().as_str()));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert_eq!(rec(&entry).machine_id, "m_new");
        fields_clear(&entry);
    });
}

#[test]
fn wake_clears_its_join_fields_however_it_ends() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        entry.borrow_mut().wake_said = Some("said".to_owned());
        entry.borrow_mut().wake_ask = Some(wsp_core::types::wiring::WakeAsk { ask: 2, of: 3 });
        h.f.wakes.set(vec![WakePlan::Done(done(None))]);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        fields_clear(&entry);
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        let entry_b = h.f.base.live.borrow()["ws_b"].clone();
        entry_b.borrow_mut().wake_said = Some("said".to_owned());
        h.f.wakes.set(vec![WakePlan::Done(Err(other("boom")))]);
        wake(&h.ctx, "ws_b", None).await.unwrap_err();
        fields_clear(&entry_b);
        h.cloud("ws_c", json!({ "name": "c", "machineId": "m_c" }));
        let entry_c = h.f.base.live.borrow()["ws_c"].clone();
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let ctx = h.ctx.clone();
        let task = tokio::task::spawn_local(async move { wake(&ctx, "ws_c", None).await });
        settle().await;
        assert!(entry_c.borrow().wake_ask.is_some());
        stop_wake(&h.ctx, "ws_c", None).await.unwrap();
        task.await.unwrap().unwrap_err();
        fields_clear(&entry_c);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 5

// disk-watch.test.ts:293 "is refused for a size not sold, on a machine that cannot move, and while a turn runs there"
#[test]
fn resize_refuses_in_the_ts_order() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        let ask = size(4.0, 8192.0);
        let order = ["entry_of", "spawn_guard", "refuse_pauseless", "refuse_sub_wake", "can_resume_at", "backend_capabilities", "nap_with"];
        // The guard first, after the entry is read.
        h.f.base.answer_with("spawn_guard", vec![err_answer("the cap", Some("conflict"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("the cap", Kind::Conflict));
        h.f.base.script.borrow_mut().shift_remove("spawn_guard");
        assert_eq!(h.args("entry_of")[0], json!({ "id": "ws_a", "thread": null, "act": "wake", "scoped": false }));
        assert_eq!(h.args("spawn_guard")[0], json!({ "act": "resize", "scoped": false }));
        h.f.base.answer_with("refuse_pauseless", vec![err_answer("a cannot be moved to another size", Some("conflict"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("a cannot be moved to another size", Kind::Conflict));
        assert_eq!(h.args("refuse_pauseless")[0]["act"], json!("be moved to another size"));
        h.f.base.script.borrow_mut().shift_remove("refuse_pauseless");
        h.f.base.answer_with("refuse_sub_wake", vec![err_answer("a sub-thread is moved by its lead", Some("usage"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("a sub-thread is moved by its lead", Kind::Usage));
        h.f.base.script.borrow_mut().shift_remove("refuse_sub_wake");
        // A machine with no resumeAt.
        h.f.base.answer_with("can_resume_at", vec![json!(false)]);
        let no_move = refusal_line("Boat changes no machine's size once it is made", "Start a new thread at the size you want instead.");
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&no_move, Kind::Invalid));
        h.f.base.answer_with("provider_of", vec![json!(null)]);
        let none_named = refusal_line("the provider changes no machine's size once it is made", "Start a new thread at the size you want instead.");
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&none_named, Kind::Invalid));
        h.f.base.script.borrow_mut().shift_remove("can_resume_at");
        h.f.base.script.borrow_mut().shift_remove("provider_of");
        // A size the provider does not sell.
        let not_sold = refusal_line(
            "32x64 is not a size this provider offers; the sizes are 2x4 ($0.05/hr), 4x8 ($0.10/hr)",
            "Ask for one of those instead.",
        );
        assert_eq!(resize(&h.ctx, "ws_a", size(32.0, 65536.0), None).await.unwrap_err(), kinded(&not_sold, Kind::Invalid));
        // A turn running on a thread of the workspace.
        h.f.base.sessions.borrow_mut().push(serde_json::from_value(json!({ "id": "s1", "workspaceId": "ws_a", "harness": "claude", "status": "running", "threadId": "t_1" })).unwrap());
        h.f.running_threads.borrow_mut().push("t_1".to_owned());
        let busy = refusal_line("a has a turn running, and the stop a move to another size takes would end it", "Stop the turn or wait for it to end, then move it.");
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&busy, Kind::Conflict));
        assert_eq!(h.args("thread_runs"), vec![json!({ "thread": "t_1" })]);
        assert!(h.args("nap_with").is_empty());
        assert!(h.persisted().is_empty() && h.events().is_empty());
        let seen: Vec<String> = h.calls(&order).into_iter().take(6).collect();
        assert_eq!(seen, ["entry_of", "spawn_guard", "entry_of", "spawn_guard", "refuse_pauseless", "entry_of"]);
    });
}

#[test]
fn resize_to_the_same_size_does_nothing() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = resize(&h.ctx, "ws_a", size(2.0, 4096.0), None).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("nap_with").is_empty() && h.args("ws_wake").is_empty());
        assert!(h.persisted().is_empty() && h.events().is_empty());
    });
}

// disk-watch.test.ts:234 "naps the machine and wakes it at the size asked, the record and its rate following"
#[test]
fn resize_naps_with_the_long_wait_then_wakes_at_the_new_size() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running", "sub": { "on": "box", "repo": "acme/x", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/work/x", "rateUsdPerHour": 0.05 } }));
        h.f.shapes.set(vec![Some(shape(4.0, 8192.0))]);
        assert_eq!(RESIZE_NAP_MS, 30 * 60_000);
        let view = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("nap_with"), vec![json!({ "id": "ws_a", "napWaitMs": 1_800_000 })]);
        assert!(entry.borrow().nap_wait_ms.is_none() && entry.borrow().resize_to.is_none());
        let wake_call = &h.args("ws_wake")[0];
        assert_eq!((&wake_call["resizeTo"], &wake_call["size"], &wake_call["shape"]), (&json!({ "cpu": 4, "memMb": 8192 }), &json!({ "cpu": 4, "memMb": 8192 }), &json!(null)));
        let last = h.persisted().pop().unwrap();
        assert_eq!(last["size"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(last["shape"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(last["sub"]["rateUsdPerHour"], json!(0.1));
        assert_eq!(rec_json(&entry), last);
        assert_eq!(h.calls(&["nap_with", "ws_wake", "shape_of", "status_now"]), ["nap_with", "ws_wake", "shape_of", "status_now"]);
    });
}

// disk-watch.test.ts:259 "says so when the machine came back at another size than asked"
#[test]
fn resize_that_landed_elsewhere_follows_the_machine_and_refuses() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.shapes.set(vec![Some(shape(2.0, 4096.0))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err, kinded("a came back at 2x4, not the 4x8 asked for. Run wsp resize again.", Kind::Conflict));
        let last = h.persisted().pop().unwrap();
        assert_eq!(last["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(last["shape"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(rec_json(&entry), last);
        assert_eq!(h.args("status_now").len(), 1);
    });
}

// disk-watch.test.ts:273 "keeps the size the provider moved it to when the wake fails after the resume landed"
#[test]
fn resize_whose_wake_fails_follows_the_old_size() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.wakes.set(vec![WakePlan::Done(Err(other("still restoring")))]);
        h.f.shapes.set(vec![Some(shape(8.0, 16384.0))]);
        let err = resize(&h.ctx, "ws_a", size(8.0, 16384.0).clone(), None).await;
        // 8x16 is not offered by the default capabilities: the provider's own two sizes are the ones asked of.
        assert_eq!(err.unwrap_err().message, "8x16 is not a size this provider offers; the sizes are 2x4 ($0.05/hr), 4x8 ($0.10/hr). Ask for one of those instead.");
        // An offered size whose wake fails after the resume landed keeps the size the shape says.
        *h.f.caps.borrow_mut() = caps_with_sizes();
        h.f.shapes.set(vec![Some(shape(4.0, 8192.0))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err.message, "still restoring");
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(rec_json(&entry)["shape"], json!({ "cpu": 4, "memMb": 8192 }));
        assert!(entry.borrow().resize_to.is_none());
        // A shape nobody read leaves the size as it was asked before, and no shape on the record.
        *entry.borrow().record.borrow_mut() = record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "running" }));
        h.f.shapes.set(vec![None]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err.message, "still restoring");
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert!(rec_json(&entry).get("shape").is_none());
        assert!(entry.borrow().resize_to.is_none());
    });
}

#[test]
fn resize_clears_the_long_wait_when_the_nap_fails() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.answer_with("nap_with", vec![err_answer("the provider refused the pause", Some("conflict"))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err, kinded("the provider refused the pause", Kind::Conflict));
        assert!(entry.borrow().nap_wait_ms.is_none());
        assert!(entry.borrow().resize_to.is_none());
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert!(h.args("ws_wake").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------- rules 6 and 7

// wake-asks-again.test.ts:373 "a stop asked of a workspace with no wake in flight answers with the record as it stands"
#[test]
fn stop_wake_pulls_the_stop_and_waits() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = stop_wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        // With a wake held on the provider, the stop ends it and the caller waits for it to end.
        *entry.borrow().record.borrow_mut() = record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "napping" }));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let _held = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        let token = entry.borrow().wake_stop.clone().expect("a wake in flight holds its stop");
        let left = stop_wake(&h.ctx, "ws_a", None).await.unwrap();
        assert!(token.is_cancelled());
        assert!(task.is_finished(), "the stop returned after the wake ended");
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
    });
}

// runtime-builders.test.ts:608 and machine-gates.test.ts:60
#[test]
fn upgrade_follows_the_new_machine_and_says_upgraded() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = upgrade(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(h.args("refuse_cannot"), vec![json!({ "id": "ws_a", "cap": "replacesMachine", "act": "have its machine replaced" })]);
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "machineId": "m_new" }))]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" })]);
        assert_eq!(h.calls(&["entry_of", "refuse_cannot", "ws_upgrade", "follow_machine", "persist", "emit"]), ["entry_of", "refuse_cannot", "ws_upgrade", "follow_machine", "persist", "emit"]);
        // A provider that stands no machine in refuses before the machine is touched.
        h.f.base.answer_with("refuse_cannot", vec![err_answer("a cannot have its machine replaced on the provider it runs on", Some("conflict"))]);
        let err = upgrade(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("a cannot have its machine replaced on the provider it runs on", Kind::Conflict));
        assert_eq!(h.args("ws_upgrade").len(), 1);
        // An upgrade that fails persists and says nothing.
        h.f.base.script.borrow_mut().shift_remove("refuse_cannot");
        h.f.upgrades.set(vec![Err(other("no machine"))]);
        let err = upgrade(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err.message, "no machine");
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events().len(), 1);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 8

// box-nap.test.ts:533 "a rebuild of a napped workspace wakes it and takes the vault off the woken machine"
#[test]
fn rebuild_of_a_napped_keeping_backend_wakes_first() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        let warned = wsp_core::console::capture();
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(h.args("refuse_cannot")[0], json!({ "id": "ws_a", "cap": "replacesMachine", "act": "be rebuilt" }));
        // It woke first, then took the vault off the running machine: an upgrade, not a rebuild.
        assert_eq!(h.calls(&["recover_gone", "ws_wake", "ws_upgrade", "ws_rebuild", "follow_machine", "emit"]), ["recover_gone", "ws_wake", "ws_upgrade", "follow_machine", "emit"]);
        assert_eq!(h.events().last().unwrap(), &json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" }));
        let reason = "rebuilt: m_a replaced by m_new";
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!(reason));
        assert_eq!(warned.lines(), [format!("rebuild of ws_a: {reason}")]);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Running);
    });
}

// box-nap.test.ts:552 "a rebuild refuses in one line when a napped workspace cannot be woken, and keeps the machine"
#[test]
fn a_rebuild_whose_wake_fails_refuses_and_keeps_the_machine() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        h.f.wakes.set(vec![WakePlan::Done(Err(other("still restoring")))]);
        let err = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a could not be woken to back up before the rebuild: still restoring", Kind::Conflict));
        assert!(h.args("ws_upgrade").is_empty() && h.args("ws_rebuild").is_empty());
        assert_eq!(rec(&entry).machine_id, "m_a");
        assert!(h.events().is_empty());
    });
}

// disk-gone.test.ts:174 "a rebuild of a paused workspace whose disk is gone makes one new machine, not two"
#[test]
fn a_rebuild_whose_wake_met_a_deleted_disk_makes_one_machine_not_two() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        h.f.wakes.set(vec![WakePlan::Done(Err(disk_gone_error("m_a", BOAT_LINE, SAID)))]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert!(h.args("ws_upgrade").is_empty());
    });
}

// runtime-create.test.ts:548 and box-nap.test.ts:488
#[test]
fn rebuild_from_running_takes_a_vault_then_falls_back() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running", "wakeRefused": "an old refusal" }));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(h.calls(&["ws_wake", "ws_upgrade", "ws_rebuild"]), ["ws_upgrade"]);
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "machineId": "m_new" }))]);
        // The vault over its cap: warned, and the machine is replaced with none.
        *entry.borrow().record.borrow_mut() = record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "running" }));
        *h.f.engine_machine.borrow_mut() = None;
        h.f.upgrades.set(vec![Err(other("vault over cap"))]);
        let warned = wsp_core::console::capture();
        rebuild(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.calls(&["ws_upgrade", "ws_rebuild"])[1..], ["ws_upgrade", "ws_rebuild"]);
        assert_eq!(
            warned.lines(),
            [
                "rebuild of ws_a: the running machine gave no vault (vault over cap); replacing with none".to_owned(),
                "rebuild of ws_a: rebuilt: m_a replaced by m_new".to_owned()
            ]
        );
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_a replaced by m_new"));
    });
}

// runtime-create.test.ts:548 and :581 "rebuild of a workspace that never napped imports nothing and says so"
#[test]
fn rebuild_reports_whether_a_nap_vault_was_there() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let imported = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        h.f.base.store.blobs.borrow_mut().entry("vaults".to_owned()).or_default().insert("ws_b".to_owned(), vec![1, 2, 3]);
        let none = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(none.machine_id, "m_new");
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_a replaced by m_new, no vault to import"));
        assert_eq!(h.calls(&["ws_wake", "ws_upgrade", "ws_rebuild"]), ["ws_rebuild"]);
        *h.f.engine_machine.borrow_mut() = None;
        rebuild(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_b replaced by m_new, nap-time vault imported"));
        assert_eq!(h.events().len(), 2);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Running);
        assert_eq!(rec(&imported).machine_id, "m_new");
    });
}

// gone.test.ts:955 and runtime-gone.test.ts:56: a record recovered out of gone is answered, not rebuilt
#[test]
fn a_rebuild_of_a_gone_record_that_recovers_answers_the_view() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "gone", "gone": "machine m_a is gone at the provider" }));
        h.f.base.answer_with("recover_gone", vec![json!("running")]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert!(h.args("ws_rebuild").is_empty() && h.args("ws_upgrade").is_empty());
        assert!(h.events().is_empty());
    });
}

#[test]
fn rebuild_waits_for_a_wake_in_flight_and_drops_its_failure() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let (tx, rx) = oneshot::channel();
        entry.borrow_mut().waking = Some(joined_error(rx));
        let ctx = h.ctx.clone();
        let task = tokio::task::spawn_local(async move { rebuild(&ctx, "ws_a", None).await });
        settle().await;
        assert!(h.args("ws_rebuild").is_empty());
        tx.send(Err(plain("the wake failed"))).unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert_eq!(h.calls(&["refuse_cannot", "recover_gone", "ws_rebuild"]), ["refuse_cannot", "recover_gone", "ws_rebuild"]);
    });
}

// ----------------------------------------------------------------------------------------------------- rules 9, 10

// reconcile.test.ts:374 "names the record, keeps its id and machine, says so once, and the next listing carries it"
#[test]
fn rename_writes_the_record_and_the_name_row_and_emits() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = rename(&h.ctx, "ws_a", " fresh ", Some(the_person())).await.unwrap();
        assert_eq!(view.name, "fresh");
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "name": "fresh" }))]);
        assert_eq!(rec_json(&entry), h.persisted()[0]);
        assert_eq!(h.f.base.store.row("workspace-names", "ws_a"), Some(json!({ "workspaceId": "ws_a", "name": "fresh", "project": "pr_cloud" })));
        assert_eq!(h.writes(), ["put workspace-names/ws_a"]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.renamed", "workspaceId": "ws_a", "name": "fresh" })]);
        assert_eq!(h.args("name_refusal"), vec![json!({ "name": "fresh" })]);
        // reconcile.test.ts:434: the same name after nameGiven writes nothing and emits nothing.
        let again = rename(&h.ctx, "ws_a", "  fresh", None).await.unwrap();
        assert_eq!(json_of(&again), view_json(&h, &entry));
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events().len(), 1);
        assert_eq!(h.args("name_refusal").len(), 1);
    });
}

// reconcile.test.ts:395 "takes the name the fork's own rule takes: one another workspace holds"
#[test]
fn rename_refuses_a_name_taken() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let taken = "other is already a workspace; pick another name, or delete it first";
        h.f.base.answer_with("name_refusal", vec![json!(taken)]);
        let err = rename(&h.ctx, "ws_a", "other", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded(taken, Kind::Conflict));
        assert_eq!(rec(&entry).name, "a");
        assert!(h.persisted().is_empty() && h.events().is_empty() && h.writes().is_empty());
    });
}

// reconcile.test.ts:331 and :357 "holds the theme and the glyph beside the name" and "null clears one fact back to none"
#[test]
fn look_sets_theme_and_glyph_and_emits_nulls() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let theme: Value = serde_json::from_str(THEME).unwrap();
        let both = WorkspaceLook { theme: Some(Some(serde_json::from_value(theme.clone()).unwrap())), glyph: Some(Some(WorkspaceGlyph::Bug)) };
        let view = look(&h.ctx, "ws_a", both, Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(rec_json(&entry)["theme"], theme);
        assert_eq!(rec_json(&entry)["glyph"], json!("bug"));
        assert_eq!(h.args("put_look").iter().map(|a| a["key"].clone()).collect::<Vec<_>>(), [json!("theme"), json!("glyph")]);
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": theme, "glyph": "bug" })]);
        // null clears one fact and leaves the other; the record loses the key rather than holding a null.
        let clear = WorkspaceLook { theme: Some(None), glyph: None };
        look(&h.ctx, "ws_a", clear, None).await.unwrap();
        assert!(rec_json(&entry).get("theme").is_none());
        assert_eq!(rec_json(&entry)["glyph"], json!("bug"));
        assert_eq!(h.events().pop().unwrap(), json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": null, "glyph": "bug" }));
        let clear_both = WorkspaceLook { theme: None, glyph: Some(None) };
        look(&h.ctx, "ws_a", clear_both, None).await.unwrap();
        assert_eq!(h.events().pop().unwrap(), json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": null, "glyph": null }));
        assert_eq!(h.persisted().len(), 3);
    });
}

// ---------------------------------------------------------------------------------------------------- rules 11, 15

// runtime-reach.test.ts:116 and :763
#[test]
fn update_daemon_refuses_in_the_ts_order() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("served_by_its_computer", vec![json!("here")]);
        h.f.base.answer_with("has_deploy_daemon", vec![json!(false)]);
        let err = update_daemon(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("a has no daemon of its own: Mac answers its files and git through this host"));
        h.f.base.script.borrow_mut().shift_remove("served_by_its_computer");
        let err = update_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("this runtime cannot deploy a daemon; the host wires the bundle"));
        h.f.base.script.borrow_mut().shift_remove("has_deploy_daemon");
        let err = update_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("wake a before updating its daemon"));
        assert!(h.args("deploy_daemon_on").is_empty());
        entry.borrow().record.borrow_mut().phase = WorkspacePhase::Running;
        update_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("deploy_daemon_on"), vec![json!({ "id": "ws_a" })]);
        h.f.base.answer_with("deploy_daemon_on", vec![err_answer("the deploy failed", None)]);
        assert_eq!(update_daemon(&h.ctx, "ws_a", None).await.unwrap_err(), plain("the deploy failed"));
    });
}

// local-workspace.test.ts:1387 and :1405 "says a host that wires no such road cannot start one"
#[test]
fn restart_daemon_drops_the_polled_reach_and_pushes_status() {
    run(async {
        let h = H::new();
        h.local("ws_a", json!({}));
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        h.f.base.answer_with("has_restart_daemon", vec![json!(false)]);
        let err = restart_daemon(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("a's daemon runs on this computer, which this host does not hold the process of"));
        let err = restart_daemon(&h.ctx, "ws_b", None).await.unwrap_err();
        assert_eq!(err, plain("b's daemon runs on a machine wsp forks, which this host does not hold the process of"));
        assert!(h.args("restart_daemon_of").is_empty());
        h.f.base.script.borrow_mut().shift_remove("has_restart_daemon");
        restart_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.calls(&["restart_daemon_of", "shared_daemon", "shared_daemon_replaced", "forget_polled_reach", "push_status"]), [
            "restart_daemon_of", "shared_daemon", "forget_polled_reach", "push_status"
        ]);
        // local-workspace.test.ts:484 a shared daemon is replaced, even where the restart fails, and the reach stays.
        h.f.base.answer_with("shared_daemon", vec![json!(true)]);
        restart_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("shared_daemon_replaced").len(), 1);
        h.f.base.answer_with("restart_daemon_of", vec![err_answer("the daemon would not start", None)]);
        let err = restart_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("the daemon would not start"));
        assert_eq!(h.args("shared_daemon_replaced").len(), 2);
        assert_eq!(h.args("forget_polled_reach").len(), 2);
        assert_eq!(h.args("push_status").len(), 2);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 12

// runtime-index.test.ts / runtime.test.ts:63 a failed create is cleared with delete
#[test]
fn delete_of_a_failed_create_emits_deleted() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("forget_failed_create", vec![json!(true)]);
        delete(&h.ctx, "ws_failed", Some(the_person())).await.unwrap();
        assert_eq!(h.events(), vec![json!({ "type": "workspace.deleted", "workspaceId": "ws_failed" })]);
        assert_eq!(h.args("spawn_guard"), vec![json!({ "act": "delete", "scoped": false })]);
        assert_eq!(h.calls(&["spawn_guard", "forget_failed_create", "entry_of", "cut_off"]), ["spawn_guard", "forget_failed_create"]);
        // A live id is no failed create, whoever asks: the failed list is not asked.
        h.cloud("ws_a", json!({ "phase": "released" }));
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("forget_failed_create").len(), 1);
        assert_eq!(h.events().len(), 1);
    });
}

// the spec's edge case: a thread's delete of a failed create goes through entryOf and so refuses
#[test]
fn delete_of_a_failed_create_by_a_thread_goes_through_entry_of() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("forget_failed_create", vec![json!(true)]);
        let err = delete(&h.ctx, "ws_failed", Some(asker("t_lead1234567", "ws_lead", "t_root"))).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_failed", Kind::NotFound));
        assert!(h.args("forget_failed_create").is_empty());
        assert!(h.events().is_empty());
        assert_eq!(h.args("spawn_guard"), vec![json!({ "act": "delete", "scoped": true })]);
    });
}

// reconcile.test.ts:291 "a second delete of the same workspace joins the first: one kill, one drop"
#[test]
fn delete_joins_a_delete_in_flight() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let (tx, rx) = oneshot::channel();
        *h.f.cut_gate.borrow_mut() = Some(rx);
        let c1 = h.ctx.clone();
        let first = tokio::task::spawn_local(async move { delete(&c1, "ws_a", None).await });
        settle().await;
        assert!(entry.borrow().deleting.is_some(), "the delete is stored for later callers to join");
        let c2 = h.ctx.clone();
        let second = tokio::task::spawn_local(async move { delete(&c2, "ws_a", None).await });
        settle().await;
        tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(h.args("cut_off").len(), 1);
        assert_eq!(h.args("unfork").len(), 1);
        assert_eq!(h.args("drop_workspace").len(), 1);
        assert!(entry.borrow().deleting.is_none());
        // And a failure is every caller's.
        let entry = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        let (tx, rx) = oneshot::channel();
        *h.f.cut_gate.borrow_mut() = Some(rx);
        h.f.base.answer_with("drop_workspace", vec![err_answer("the store refused", None)]);
        let (c1, c2) = (h.ctx.clone(), h.ctx.clone());
        let first = tokio::task::spawn_local(async move { delete(&c1, "ws_b", None).await });
        settle().await;
        let second = tokio::task::spawn_local(async move { delete(&c2, "ws_b", None).await });
        settle().await;
        tx.send(()).unwrap();
        assert_eq!(first.await.unwrap().unwrap_err(), plain("the store refused"));
        assert_eq!(second.await.unwrap().unwrap_err(), plain("the store refused"));
        assert!(entry.borrow().deleting.is_none());
    });
}

// sub-queue.test.ts:300 "a held spawn's record deleted goes with its start" and sub-queue.test.ts:519
#[test]
fn delete_of_a_queued_spawn_stops_it() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "queued" }));
        h.f.base.answer_with("delete_queued", vec![json!("stopped")]);
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.args("delete_queued"), vec![json!({ "id": "ws_a" })]);
        assert_eq!(h.calls(&["cut_off", "unfork", "machine_kill", "drop_workspace"]), Vec::<String>::new());
        assert!(entry.borrow().deleting.is_none());
        // A failed stop is the answer.
        h.f.base.script.borrow_mut().shift_remove("delete_queued");
        h.f.base.answer_with("delete_queued", vec![err_answer("the start already landed", Some("conflict"))]);
        let err = delete(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("the start already landed", Kind::Conflict));
        // A queued record no spawn holds is deleted as any, asking nothing of a machine it has none of.
        h.f.base.script.borrow_mut().shift_remove("delete_queued");
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.calls(&["cut_off", "unfork", "machine_kill", "drop_workspace"]), ["cut_off", "drop_workspace"]);
    });
}

// folder-threads.test.ts:800 "refuses a worktree wsp made while it holds a file no commit has, and removes it once clean"
#[test]
fn delete_refuses_a_worktree_with_changes_before_anything() {
    run(async {
        let h = H::new();
        let entry = h.local("ws_a", json!({ "worktree": { "path": "/wt/a", "branch": "wsp/a", "made": true } }));
        h.f.paths.borrow_mut().push("/wt/a".to_owned());
        h.f.base.answer_with("refuse_changed", vec![err_answer("a has a file no commit holds", Some("conflict"))]);
        let err = delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a has a file no commit holds", Kind::Conflict));
        assert_eq!(h.args("refuse_changed"), vec![json!({ "path": "/wt/a" })]);
        assert_eq!(h.calls(&["cut_off", "end_sessions", "drop_workspace"]), Vec::<String>::new());
        assert!(entry.borrow().deleting.is_none());
        // The rule reads only a worktree wsp made, that stands, on disk.
        h.f.base.script.borrow_mut().shift_remove("refuse_changed");
        let variants = [
            json!({ "path": "/wt/a", "made": false }),
            json!({ "path": "/wt/a", "made": true, "gone": true }),
            json!({ "path": "/wt/elsewhere", "made": true }),
        ];
        for (i, tree) in variants.iter().enumerate() {
            let id = format!("ws_v{i}");
            h.local(&id, json!({ "name": id, "worktree": tree }));
            delete(&h.ctx, &id, None).await.unwrap();
        }
        assert_eq!(h.args("refuse_changed").len(), 1);
        assert!(h.args("remove_worktree_ending").is_empty());
    });
}

// folder-threads.test.ts:766 and :787, reconcile.test.ts:175, pause.test.ts:237
#[test]
fn delete_takes_everything_away_in_the_ts_order() {
    run(async {
        let h = H::new();
        let entry = h.local("ws_a", json!({ "worktree": { "path": "/wt/a", "branch": "wsp/a", "made": true } }));
        entry.borrow_mut().delete_said = Some(DeleteSaid { phase: WorkspacePhase::Running, line: "an old line".to_owned() });
        h.f.paths.borrow_mut().push("/wt/a".to_owned());
        h.f.base.threads.borrow_mut().extend([("t_1".to_owned(), "ws_a".to_owned()), ("t_x".to_owned(), "ws_other".to_owned()), ("t_2".to_owned(), "ws_a".to_owned())]);
        for (id, thread, ws) in [("s1", "t_1", "ws_a"), ("s3", "t_3", "ws_a"), ("s4", "t_4", "ws_other"), ("s5", "t_5", "ws_a")] {
            h.f.base.sessions.borrow_mut().push(serde_json::from_value(json!({ "id": id, "workspaceId": ws, "harness": "claude", "status": "completed", "threadId": thread })).unwrap());
        }
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        let order = ["spawn_guard", "entry_of", "refuse_changed", "cut_off", "end_sessions", "drop_checkpoints", "remove_worktree_ending", "drop_thread_files", "machine_kill", "unfork", "drop_workspace"];
        assert_eq!(h.calls(&order), [
            "spawn_guard", "entry_of", "refuse_changed", "cut_off", "end_sessions", "drop_checkpoints", "drop_checkpoints", "drop_checkpoints", "drop_checkpoints",
            "remove_worktree_ending", "drop_thread_files", "machine_kill", "drop_workspace"
        ]);
        assert_eq!(h.args("end_sessions"), vec![json!({ "id": "ws_a", "reason": DELETED_REASON })]);
        // The thread records of the workspace, then the session rows' threads the records do not name.
        assert_eq!(h.args("drop_checkpoints").iter().map(|a| a["thread"].clone()).collect::<Vec<_>>(), [json!("t_1"), json!("t_2"), json!("t_3"), json!("t_5")]);
        assert_eq!(h.args("drop_thread_files"), vec![json!({ "id": "ws_a", "threads": ["t_1", "t_2"] })]);
        assert!(h.args("unfork").is_empty(), "a computer wsp does not run is killed, not unforked");
        assert!(entry.borrow().deleting.is_none() && entry.borrow().delete_said.is_none());
        assert!(!h.f.base.live.borrow().contains_key("ws_a"));
    });
}

// reconcile.test.ts:548 "keeps the record, listed and deletable, when the machine will not die" and runtime.test.ts:542
#[test]
fn delete_of_a_fork_unforks_its_machine_and_drops_no_checkpoints() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.threads.borrow_mut().push(("t_1".to_owned(), "ws_a".to_owned()));
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.calls(&["cut_off", "end_sessions", "drop_checkpoints", "remove_worktree_ending", "drop_thread_files", "machine_kill", "unfork", "drop_workspace"]), [
            "cut_off", "end_sessions", "drop_thread_files", "unfork", "drop_workspace"
        ]);
        // A machine the provider cannot find is gone already; another failure is the delete's, and the record stays.
        h.local("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        h.f.kills.set(vec![Err(missing())]);
        delete(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.args("drop_workspace").len(), 2);
        let entry = h.local("ws_c", json!({ "name": "c", "machineId": "m_c" }));
        h.f.kills.set(vec![Err(other("the kill was refused"))]);
        let err = delete(&h.ctx, "ws_c", None).await.unwrap_err();
        assert_eq!(err.message, "the kill was refused");
        assert_eq!(h.args("drop_workspace").len(), 2);
        assert!(h.f.base.live.borrow().contains_key("ws_c"));
        assert!(entry.borrow().deleting.is_none());
    });
}

#[test]
fn delete_asks_nothing_of_a_machine_that_is_not_standing() {
    run(async {
        let h = H::new();
        let released = h.cloud("ws_r", json!({ "phase": "released", "machineId": "" }));
        let lost = h.cloud("ws_l", json!({ "phase": "running", "sub": { "on": "box", "repo": "acme/x", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/work/x", "cloudGone": true } }));
        delete(&h.ctx, "ws_r", Some(the_person())).await.unwrap();
        delete(&h.ctx, "ws_l", None).await.unwrap();
        assert!(h.args("unfork").is_empty() && h.args("machine_kill").is_empty());
        assert_eq!(h.args("drop_workspace"), vec![json!({ "id": "ws_r" }), json!({ "id": "ws_l" })]);
        assert!(released.borrow().deleting.is_none() && lost.borrow().deleting.is_none());
    });
}

// runtime.test.ts:460 "a delete the provider answers and never acts on keeps the record, fails naming the machine" and :483
#[test]
fn delete_refused_by_a_machine_alive_says_why_on_the_record() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.unforks.set(vec![Err(alive("m_a", "running"))]);
        let warned = wsp_core::console::capture();
        let err = delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let line = "a's machine m_a is still running after three asks; run wsp delete again or delete it at the provider";
        assert_eq!(err, kinded(line, Kind::MachineAlive));
        let said = entry.borrow().delete_said.clone().expect("the line stands on the record");
        assert_eq!((said.phase, said.line.as_str()), (WorkspacePhase::Running, line));
        assert_eq!(warned.lines(), [line]);
        assert!(h.args("drop_workspace").is_empty());
        assert!(h.f.base.live.borrow().contains_key("ws_a"));
        assert!(entry.borrow().deleting.is_none());
        // The next delete starts by dropping the line, and one the provider takes drops the record.
        h.f.unforks.set(vec![Ok(())]);
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert!(entry.borrow().delete_said.is_none());
        assert_eq!(h.args("drop_workspace").len(), 1);
        // Any other failure of the delete propagates and says nothing on the record.
        let other_entry = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        h.f.unforks.set(vec![Err(other("the provider is down"))]);
        let err = delete(&h.ctx, "ws_b", None).await.unwrap_err();
        assert_eq!(err.message, "the provider is down");
        assert!(other_entry.borrow().delete_said.is_none());
        assert!(h.f.base.live.borrow().contains_key("ws_b"));
    });
}

// ---------------------------------------------------------------------------------------------------- rules 13, 14

// forget.test.ts:20, :41, :51 and :69
#[test]
fn forget_refuses_an_undriven_kind_and_a_machine_still_standing() {
    run(async {
        let h = H::new();
        h.local("ws_a", json!({}));
        let err = forget(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a runs on this computer, which is not at a provider; run wsp delete a", Kind::Conflict));
        assert!(h.args("read_gone").is_empty());
        h.f.base.hold(record("ws_p", "place", "pr_cloud", json!({ "name": "p", "machineId": "m_p" })));
        let err = forget(&h.ctx, "ws_p", None).await.unwrap_err();
        assert_eq!(err, kinded("p runs on a computer you joined, which is not at a provider; run wsp delete p", Kind::Conflict));
        // A machine any read still finds is kept whole, with the reason.
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m1", "phase": "running" }));
        for state in ["running", "paused"] {
            h.f.base.script.borrow_mut().shift_remove("read_gone");
            h.f.base.answer_with("read_gone", vec![json!(state)]);
            let err = forget(&h.ctx, "ws_b", None).await.unwrap_err();
            assert_eq!(err, kinded(&format!("b's machine m1 is still {state}; pause it or delete it at the provider first"), Kind::Conflict));
        }
        assert!(h.args("end_sessions").is_empty() && h.args("drop_workspace").is_empty());
        assert_eq!(h.args("read_gone").iter().map(|a| a["machine"].clone()).collect::<Vec<_>>(), [json!("m1"), json!("m1")]);
        // A gone one is dropped, its sessions ended with the delete's reason first.
        h.f.base.script.borrow_mut().shift_remove("read_gone");
        forget(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.calls(&["read_gone", "end_sessions", "drop_workspace"]).split_off(2), ["read_gone", "end_sessions", "drop_workspace"]);
        assert_eq!(h.args("end_sessions"), vec![json!({ "id": "ws_b", "reason": DELETED_REASON })]);
        assert_eq!(h.args("drop_workspace"), vec![json!({ "id": "ws_b" })]);
    });
}

// idle.test.ts:375 "workspaces.touch over the wire starts the window over"
#[test]
fn touch_touches_the_idle_window() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        touch(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.calls(&["entry_of", "idle_touch"]), ["entry_of", "idle_touch"]);
        assert_eq!(h.args("idle_touch"), vec![json!({ "id": "ws_a" })]);
        let err = touch(&h.ctx, "ws_none", None).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_none", Kind::NotFound));
        assert_eq!(h.args("idle_touch").len(), 1);
    });
}
