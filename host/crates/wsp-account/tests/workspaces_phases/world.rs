
use std::rc::Rc;
use base::{err_answer, kinded, plain, project, rec, rec_json, record, run, view_of};
use futures::channel::oneshot;
use futures::future::FutureExt;
use phases_support::{
    alive, asker, caps_with_sizes, disk_gone_error, done, format_seconds, missing, other, refusal_line, restore_unfinished, shape, size,
    the_person, unanswered, FakePhases, WakePlan,
};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use wsp_account::projects::workspaces::phases::{
    delete, disk_gone, forget, look, nap, rebuild, rename, resize, restart_daemon, stop_wake, touch, update_daemon, upgrade, wake,
    MachineRead, PhasesCtx, RESIZE_NAP_MS,
};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::clock::FakeClock;
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
        self.f.base.clock.now()
    }
    fn clock(&self) -> &FakeClock {
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
