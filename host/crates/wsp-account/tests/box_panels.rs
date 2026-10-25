// SPDX-License-Identifier: AGPL-3.0-only
//! The box panels (spec/box_panels.md). Every expected value is the TS's: oracle/box_panels.oracle.ts drove the real
//! `boxPanelsOpener` through the sessions below against the same fake world, and wrote
//! tests/fixtures/oracle/box_panels.json. `test/box-panels.test.ts` (37 cases over a whole host and a fake computer)
//! reaches this file; the cases it decides are these sessions: the ports watch per folder and its cgroups (test:220
//! onward), the named watches past the daemon's cap, a daemon from before named watches, the process watch shared on
//! one link, the readings hold, and the terminal's folder and environment.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{live, oracle, record, run, settle};
use wsp_account::account::box_panels::{named_watch_refusal, proc_watch_scope, BoxPanels, BoxPanelsCtx, BoxPanelsOpener, Placements, StartedRow, Watchers};
use wsp_account::account::core_types::{AccountResult, Frame, Entry, Release, Served};
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
        self.started.iter().map(|(t, at)| StartedRow { workspace_id: "w1".into(), thread_id: Some(t.clone()), started_at: Some(*at) }).collect()
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
            .map(|t| serde_json::from_value(json!({ "thread": t, "workspace": "w1", "folder": "/srv/alpha", "cgroup": cgroup(t) })).unwrap())
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
        let entry = live(record(&json!({ "id": "w1", "name": "alpha-studio", "kind": "place", "project": "pA2", "machineId": "", "phase": "running", "place": "box1" })));
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

/// A JSON comparison that keeps key order, as the wire does (README A8).
fn same(got: &Value, want: &Value, what: &str) {
    assert_eq!(serde_json::to_string(got).unwrap(), serde_json::to_string(want).unwrap(), "{what}");
}

fn ids(n: usize, from: usize) -> Vec<String> {
    (from..from + n).map(|i| format!("t{i:02}")).collect()
}

fn session(name: &str) -> Value {
    oracle("box_panels")["sessions"][name].clone()
}

async fn send(pane: &Rc<dyn BoxPanels>, frame: Value) -> Value {
    Value::Object(pane.send(obj(frame)).await.unwrap())
}

#[test]
fn b1_proc_watch_scope_table() {
    for row in oracle("box_panels")["scope"].as_array().unwrap() {
        let asks: Vec<Option<Vec<String>>> = row["asks"].as_array().unwrap().iter().map(|a| a.as_array().map(|l| l.iter().map(|c| c.as_str().unwrap().to_string()).collect())).collect();
        assert_eq!(json!(proc_watch_scope(&asks)), row["answer"], "{}", row["name"]);
    }
}

#[test]
fn b2_named_watch_refusal_table() {
    for row in oracle("box_panels")["refusal"].as_array().unwrap() {
        let got = named_watch_refusal("studio", row["version"].as_i64()).map(Value::Object).unwrap_or(Value::Null);
        same(&got, &row["answer"], &format!("version {}", row["version"]));
    }
}

#[test]
fn b3_ports_three_threads() {
    let want = session("ports three threads");
    let w = world(&["ta", "tb", "tc"], &[("tb", 30.0), ("ta", 10.0)], Some(200));
    w.reply(|f| (op(f) == "ports.watch").then(|| obj(json!({ "ok": true, "ports": [{ "port": 3000, "pid": 1 }, { "port": 5173, "pid": 2 }] }))));
    run(async {
        let opener = w.opener();
        let p = w.pane(&opener);
        let reply = send(&p, json!({ "op": "ports.watch", "roots": [7, 7.5, "x", 9] })).await;
        same(&reply, &want["reply"], "reply");
        same(&w.take(), &want["served"], "served: the roots kept are the integers, the cgroups newest first");
        same(&w.placed(), &want["placed"], "placed");
    });
}

#[test]
fn b4_ports_twenty_threads_place_and_shrink() {
    let want = session("ports twenty threads");
    let w = world(&ids(20, 0).iter().map(String::as_str).collect::<Vec<_>>(), &[], Some(200));
    run(async {
        let opener = w.opener();
        let p = w.pane(&opener);
        let first = send(&p, json!({ "op": "ports.watch", "roots": [] })).await;
        same(&first, &want["first"], "first reply");
        same(&w.placed(), &want["placedFirst"], "sixteen in the first watch, four in the second");
        same(&w.take(), &want["servedFirst"], "two watches, the second named w1-2");
        w.threads.borrow_mut().shift_remove("t03");
        w.threads.borrow_mut().insert("t20".into(), "w1".into());
        w.push().await;
        same(&w.placed(), &want["placedMoved"], "a cgroup keeps its watch; the new one takes the room t03 left");
        same(&w.take(), &want["servedMoved"], "moved");
        for t in ids(15, 5) {
            w.threads.borrow_mut().shift_remove(&t);
        }
        w.push().await;
        same(&w.placed(), &want["placedShrunk"], "shrunk placements");
        same(&w.take(), &want["servedShrunk"], "the watch no longer needed is named again over nothing");
        w.push().await;
        same(&w.take(), &want["servedSame"], "nothing moved, nothing sent");
    });
}

#[test]
fn b5_ports_merged_once() {
    let want = session("ports holders");
    let w = world(&ids(17, 0).iter().map(String::as_str).collect::<Vec<_>>(), &[], Some(200));
    w.reply(|f| {
        (op(f) == "ports.watch").then(|| {
            if f["watch"] == json!("w1") {
                obj(json!({ "ok": true, "ports": [{ "port": 3000, "pid": 1 }] }))
            } else {
                obj(json!({ "ok": true, "ports": [{ "port": 3000, "pid": 9 }, { "port": 4000, "pid": 3 }] }))
            }
        })
    });
    run(async {
        let opener = w.opener();
        let p = w.pane(&opener);
        same(&send(&p, json!({ "op": "ports.watch", "roots": [] })).await, &want["reply"], "a port in two shares answered once, its first row kept");
    });
}

#[test]
fn b6_port_holders_table() {
    let want = session("ports holders");
    let w = world(&ids(17, 0).iter().map(String::as_str).collect::<Vec<_>>(), &[], Some(200));
    w.reply(|f| {
        (op(f) == "ports.watch").then(|| {
            if f["watch"] == json!("w1") {
                obj(json!({ "ok": true, "ports": [{ "port": 3000, "pid": 1 }] }))
            } else {
                obj(json!({ "ok": true, "ports": [{ "port": 3000, "pid": 9 }, { "port": 4000, "pid": 3 }] }))
            }
        })
    });
    run(async {
        let opener = w.opener();
        let p = w.pane(&opener);
        send(&p, json!({ "op": "ports.watch", "roots": [] })).await;
        for e in want["heard"].as_array().unwrap() {
            let event = json!({ "type": e["type"], "port": e["port"], "watch": e["watch"] });
            assert_eq!(json!(p.heard(&event)), e["heard"], "{event}");
        }
    });
}

#[test]
fn b3_a_daemon_behind_is_refused_before_anything_is_sent() {
    for (version, name) in [(Some(135), "ports behind 135"), (None, "ports behind unknown")] {
        let want = session(name);
        let w = world(&["ta"], &[], version);
        run(async {
            let opener = w.opener();
            let p = w.pane(&opener);
            same(&send(&p, json!({ "op": "ports.watch", "roots": [] })).await, &want["reply"], name);
            same(&w.take(), &want["served"], name);
        });
    }
}

#[test]
fn b5_one_share_refused_answers_its_refusal() {
    let want = session("ports one share refused");
    let w = world(&ids(17, 0).iter().map(String::as_str).collect::<Vec<_>>(), &[], Some(200));
    w.reply(|f| (op(f) == "ports.watch" && f["watch"] == json!("w1-2")).then(|| obj(json!({ "ok": false, "error": "too many watches" }))));
    run(async {
        let opener = w.opener();
        let p = w.pane(&opener);
        same(&send(&p, json!({ "op": "ports.watch", "roots": [] })).await, &want["reply"], "the refusal whole");
        same(&w.take(), &want["served"], "both shares were asked");
    });
}

#[test]
fn b7_procs_two_panes() {
    let want = session("procs two panes");
    let steps = want.as_array().unwrap();
    let w = world(&["ta", "tb"], &[], Some(200));
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        let b = w.pane(&opener);
        same(&send(&a, json!({ "op": "proc.watch", "cgroups": [cgroup("ta"), 5] })).await, &steps[0]["reply"], "a watches");
        same(&w.take(), &steps[0]["served"], "scopes, then a's cgroup alone");
        same(&send(&b, json!({ "op": "proc.watch", "cgroups": [cgroup("tb")] })).await, &steps[1]["reply"], "b watches");
        same(&w.take(), &steps[1]["served"], "scopes, then both panes' cgroups merged");
        let heard = &steps[2];
        assert_eq!(json!(a.heard(&json!({ "type": "proc.snapshot" }))), heard["snapshot"]);
        assert_eq!(json!(a.heard(&json!({ "type": "proc.changes" }))), heard["changes"]);
        assert_eq!(json!(a.heard(&json!({ "type": "sys.sample" }))), heard["sys"]);
        assert_eq!(json!(a.heard(&json!({ "type": "pty.data" }))), heard["other"]);
        same(&send(&a, json!({ "op": "proc.unwatch" })).await, &steps[3]["reply"], "a unwatches");
        settle().await;
        same(&w.take(), &steps[3]["served"], "the watch asked again with b's cgroup alone, no scopes");
        assert_eq!(json!(a.heard(&json!({ "type": "proc.snapshot" }))), steps[4]["snapshot"]);
        same(&send(&a, json!({ "op": "proc.unwatch" })).await, &steps[5]["reply"], "again");
        settle().await;
        same(&w.take(), &steps[5]["served"], "a second unwatch says nothing");
        w.threads.borrow_mut().insert("tc".into(), "w1".into());
        w.push().await;
        same(&w.take(), &steps[6]["served"], "a thread starts: the watching pane names the scopes again");
        w.push().await;
        same(&w.take(), &steps[7]["served"], "nothing moved, nothing sent");
        b.close();
        settle().await;
        same(&w.take(), &steps[8]["served"], "the last pane leaves: proc.unwatch");
        b.close();
        settle().await;
        same(&w.take(), &steps[9]["served"], "a second close says nothing");
    });
}

#[test]
fn b7_procs_whole_and_refused() {
    let want = session("procs whole and refused");
    let steps = want.as_array().unwrap();
    let w = world(&["ta"], &[], Some(200));
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        let b = w.pane(&opener);
        same(&send(&a, json!({ "op": "proc.watch", "cgroups": [cgroup("ta")] })).await, &steps[0]["reply"], "a");
        same(&w.take(), &steps[0]["served"], "a's watch");
        same(&send(&b, json!({ "op": "proc.watch" })).await, &steps[1]["reply"], "b asks for every process");
        same(&w.take(), &steps[1]["served"], "the watch with no cgroups key");
        w.reply(|f| (op(f) == "proc.watch").then(|| obj(json!({ "ok": false, "error": "proc reads off" }))));
        same(&send(&b, json!({ "op": "proc.watch", "cgroups": [] })).await, &steps[2]["reply"], "b refused");
        settle().await;
        same(&w.take(), &steps[2]["served"], "b's ask taken back: the watch asked again with a's");
        assert_eq!(json!(b.heard(&json!({ "type": "proc.snapshot" }))), steps[2]["heard"]);
    });
}

#[test]
fn b7_link_replaced_says_nothing() {
    let want = session("procs link replaced");
    let w = world(&["ta"], &[], Some(200));
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        send(&a, json!({ "op": "proc.watch", "cgroups": [cgroup("ta")] })).await;
        w.take();
        *w.link.borrow_mut() = Rc::new(());
        a.close();
        settle().await;
        same(&w.take(), &want["served"], "nothing to the link that replaced it");
    });
}

#[test]
fn b8_readings_hold_refuse_release() {
    let want = session("readings");
    let steps = want.as_array().unwrap();
    let w = world(&["ta"], &[], Some(200));
    w.sys_refusals.set(1);
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        same(&send(&a, json!({ "op": "sys.watch" })).await, &steps[0]["reply"], "refused");
        assert_eq!(json!(a.heard(&json!({ "type": "sys.sample" }))), steps[0]["heard"]);
        same(&send(&a, json!({ "op": "sys.watch" })).await, &steps[1]["reply"], "taken");
        assert_eq!(json!(a.heard(&json!({ "type": "sys.sample" }))), steps[1]["heard"]);
        same(&send(&a, json!({ "op": "sys.watch" })).await, &steps[2]["reply"], "again");
        same(&send(&a, json!({ "op": "sys.unwatch" })).await, &steps[3]["reply"], "unwatch");
        settle().await;
        assert_eq!(json!(*w.released.borrow()), steps[3]["released"], "one hold, released once");
        assert_eq!(json!(a.heard(&json!({ "type": "sys.sample" }))), steps[3]["heard"]);
        same(&send(&a, json!({ "op": "sys.unwatch" })).await, &steps[4]["reply"], "unwatch again");
        settle().await;
        assert_eq!(json!(*w.released.borrow()), steps[4]["released"]);
        same(&w.take(), &steps[5]["served"], "readings never reach the link's frames");
    });
}

#[test]
fn b10_terminal_and_pass_through() {
    let want = session("terminal and pass through");
    let w = world(&["ta"], &[], Some(200));
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        let replies = vec![
            send(&a, json!({ "op": "pty.create", "cols": 80, "env": { "PATH": "/opt/bin", "TERM": "xterm" } })).await,
            send(&a, json!({ "op": "pty.create", "cwd": "/srv/alpha/sub", "env": null })).await,
            send(&a, json!({ "op": "pty.create", "cwd": 7 })).await,
            send(&a, json!({ "op": "container.stop", "id": "c1" })).await,
            send(&a, json!({ "op": "fs.list", "path": "/" })).await,
        ];
        same(&json!(replies), &want["replies"], "replies");
        same(&w.take(), &want["served"], "the turn's env under the pane's, the folder unless a string cwd");
        for pair in want["answers"].as_array().unwrap() {
            assert_eq!(json!(a.answers(pair[0].as_str().unwrap())), pair[1], "{}", pair[0]);
        }
    });
}

#[test]
fn b11_close_and_delete() {
    let want = session("close and delete");
    let w = world(&["ta"], &[], Some(200));
    run(async {
        let opener = w.opener();
        let a = w.pane(&opener);
        let b = w.pane(&opener);
        send(&a, json!({ "op": "ports.watch", "roots": [] })).await;
        assert_eq!(json!(w.watchers.borrow().get("w1").map_or(0, Vec::len)), want["before"]["pushes"]);
        same(&w.placed(), &want["before"]["placed"], "placed");
        a.close();
        assert_eq!(json!(w.watchers.borrow().get("w1").map_or(0, Vec::len)), want["afterClose"]["pushes"], "the pane's push left");
        let listeners: Vec<Rc<dyn Fn(&str)>> = w.deleted.borrow().clone();
        for l in listeners {
            l("w1");
        }
        assert_eq!(json!(w.placements.borrow().contains_key("w1")), want["afterDelete"]["placements"], "the folder's placements went");
        b.close();
    });
}
