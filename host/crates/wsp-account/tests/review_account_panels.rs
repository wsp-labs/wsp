// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the box panels over generated sessions. `oracle/zz-review-account-panels-fuzz.oracle.ts` drove the real
//! `boxPanelsOpener` through 200 seeded sessions of 30 steps (ports and process watches with odd roots and cgroups,
//! terminals, readings holds and their refusals, pushes as threads come and go, a link that is replaced, a pane that closes,
//! a daemon that refuses a reply) and wrote each step's answer, the frames served and the placements as
//! tests/fixtures/oracle/zz_review_account_panels_fuzz.json. The world is the one tests/box_panels.rs builds.
#![allow(dead_code, unused_imports)]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{live, oracle, record, run, settle};
use wsp_account::account::box_panels::{
    named_watch_refusal, proc_watch_scope, BoxPanels, BoxPanelsCtx, BoxPanelsOpener, Placements, StartedRow, Watchers,
};
use wsp_account::account::core_types::{AccountResult, Entry, Frame, Release, Served};
use wsp_wire::wire::daemon::ProcScope;

type Reply = Rc<dyn Fn(&Frame) -> Option<Frame>>;

/// The oracle's `world()`: a workspace w1 on box1 with these threads, sessions for some, and a daemon whose replies a
/// session scripts.
struct World {
    threads: RefCell<IndexMap<String, String>>,
    started: Vec<(String, f64)>,
    version: Option<i64>,
    link: RefCell<Rc<()>>,
    sys_refusals: Cell<u32>,
    released: Rc<RefCell<Vec<String>>>,
    deleted: RefCell<Vec<Rc<dyn Fn(&str)>>>,
    served: Rc<RefCell<Vec<Value>>>,
    replies: Rc<RefCell<Vec<Reply>>>,
    placements: Placements,
    watchers: Watchers,
}

fn world(threads: &[&str], started: &[(&str, f64)], version: Option<i64>) -> Rc<World> {
    Rc::new(World {
        threads: RefCell::new(threads.iter().map(|t| (t.to_string(), "w1".to_string())).collect()),
        started: started.iter().map(|(t, at)| (t.to_string(), *at)).collect(),
        version,
        link: RefCell::new(Rc::new(())),
        sys_refusals: Cell::new(0),
        released: Rc::new(RefCell::new(vec![])),
        deleted: RefCell::new(vec![]),
        served: Rc::new(RefCell::new(vec![])),
        replies: Rc::new(RefCell::new(vec![])),
        placements: Rc::new(RefCell::new(IndexMap::new())),
        watchers: Rc::new(RefCell::new(IndexMap::new())),
    })
}

fn cgroup(thread: &str) -> String {
    format!("/wsp-threads/{thread}")
}

#[async_trait(?Send)]
impl BoxPanelsCtx for World {
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.deleted.borrow_mut().push(f);
    }
    fn session_rows(&self) -> Vec<StartedRow> {
        self.started
            .iter()
            .map(|(t, at)| StartedRow { workspace_id: "w1".into(), thread_id: Some(t.clone()), started_at: Some(*at) })
            .collect()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.threads.borrow().iter().map(|(t, w)| (t.clone(), w.clone())).collect()
    }
    fn link(&self, _place_id: &str) -> Option<Rc<()>> {
        Some(self.link.borrow().clone())
    }
    fn name_of(&self, _place_id: &str) -> String {
        "studio".into()
    }
    async fn daemon_version(&self, _place_id: &str) -> Option<i64> {
        self.version
    }
    async fn watch_sys(&self, _place_id: &str) -> Result<Release, String> {
        if self.sys_refusals.get() > 0 {
            self.sys_refusals.set(self.sys_refusals.get() - 1);
            return Err("this computer reads no load".into());
        }
        let released = self.released.clone();
        Ok(Rc::new(move || released.borrow_mut().push("sys".into())))
    }
    fn scopes_on(&self, _place_id: &str) -> Vec<ProcScope> {
        self.threads
            .borrow()
            .keys()
            .map(|t| {
                serde_json::from_value(json!({ "thread": t, "workspace": "w1", "folder": "/srv/alpha", "cgroup": cgroup(t) })).unwrap()
            })
            .collect()
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        IndexMap::from([("WSP_THREAD_HOME".to_string(), "/srv/alpha".to_string()), ("PATH".to_string(), "/usr/bin".to_string())])
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

impl World {
    /// The daemon on the link: logs each frame whole, answers the first scripted reply that takes it, else ok.
    fn daemon(self: &Rc<Self>) -> Served {
        let served = self.served.clone();
        let replies = self.replies.clone();
        Rc::new(move |f: Frame| {
            served.borrow_mut().push(Value::Object(f.clone()));
            let said = replies.borrow().iter().find_map(|r| r(&f));
            let reply = said.unwrap_or_else(|| match f.get("op").and_then(Value::as_str) {
                Some("ports.watch") => obj(json!({ "ok": true, "ports": [] })),
                Some("pty.create") => obj(json!({ "ok": true, "ptyId": "pty_1", "pid": 4242 })),
                _ => obj(json!({ "ok": true })),
            });
            async move { AccountResult::Ok(reply) }.boxed_local()
        })
    }
    fn opener(self: &Rc<Self>) -> Rc<BoxPanelsOpener> {
        BoxPanelsOpener::new(self.clone(), self.watchers.clone(), self.placements.clone())
    }
    fn pane(self: &Rc<Self>, opener: &Rc<BoxPanelsOpener>) -> Rc<dyn BoxPanels> {
        let entry = live(record(
            &json!({ "id": "w1", "name": "alpha-studio", "kind": "place", "project": "pA2", "machineId": "", "phase": "running", "place": "box1" }),
        ));
        opener.open(&entry, "box1", self.daemon(), "/srv/alpha")
    }
    fn reply(&self, r: impl Fn(&Frame) -> Option<Frame> + 'static) {
        self.replies.borrow_mut().push(Rc::new(r));
    }
    /// The frames served since the last take, as the oracle's `served.splice(0)`.
    fn take(&self) -> Value {
        Value::Array(self.served.borrow_mut().drain(..).collect())
    }
    fn placed(&self) -> Value {
        json!(self.placements.borrow().get("w1").cloned().unwrap_or_default())
    }
    async fn push(&self) {
        let pushes: Vec<Rc<dyn Fn()>> = self.watchers.borrow().get("w1").cloned().unwrap_or_default();
        for p in pushes {
            p();
        }
        settle().await;
    }
}

fn obj(v: Value) -> Frame {
    v.as_object().unwrap().clone()
}

fn op(f: &Frame) -> &str {
    f.get("op").and_then(Value::as_str).unwrap_or("")
}

fn ports_for(watch: &Value) -> Value {
    match watch.as_str() {
        Some("w1") => json!([{ "port": 3000, "pid": 1 }, { "port": 5173, "pid": 2 }]),
        Some("w1-2") => json!([{ "port": 3000, "pid": 9 }, { "port": 4000, "pid": 3 }]),
        Some("w1-3") => json!([{ "port": 4000, "pid": 4 }]),
        _ => json!([]),
    }
}

fn canon(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), canon(x))).collect()),
        _ => v.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-C1. box-panels.ts:172-176 (`push`). A thread that started names its cgroup in the pane's ports watch again with
// `void watchPorts(named.roots)`, which sends its frames inside the push. box_panels.rs `on_push` captures `named.roots`
// at the push and sends from a task it queues with `ctx.spawn`, so a `ports.watch` the pane's client sends before the task
// runs goes first, and the queued one then sends the older roots and writes them back as the pane's `named`. Node
// (zz-review-account-panels.oracle.ts "a push of a thread that started, then the pane names its roots again"): the push's
// frame with roots [7], then the client's with roots [7,9], and the next push has nothing to say.

#[test]
fn a_push_is_sent_before_the_roots_the_client_names_next() {
    run(async {
        let w = world(&["ta"], &[], Some(200));
        let opener = w.opener();
        let pane = w.pane(&opener);
        pane.send(obj(json!({ "op": "ports.watch", "roots": [7] }))).await.unwrap();
        w.take();
        w.threads.borrow_mut().insert("tb".to_string(), "w1".to_string());
        let pushes: Vec<Rc<dyn Fn()>> = w.watchers.borrow().get("w1").cloned().unwrap_or_default();
        for push in pushes {
            push();
        }
        pane.send(obj(json!({ "op": "ports.watch", "roots": [7, 9] }))).await.unwrap();
        settle().await;
        let frames: Vec<Value> = w
            .take()
            .as_array()
            .unwrap()
            .iter()
            .map(|f| json!({ "op": f["op"], "roots": f["roots"], "cgroups": f["cgroups"] }))
            .collect();
        assert_eq!(
            serde_json::to_string(&frames).unwrap(),
            r#"[{"op":"ports.watch","roots":[7],"cgroups":["/wsp-threads/ta","/wsp-threads/tb"]},{"op":"ports.watch","roots":[7,9],"cgroups":["/wsp-threads/ta","/wsp-threads/tb"]}]"#
        );
    });
}
