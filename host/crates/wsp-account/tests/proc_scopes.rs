// SPDX-License-Identifier: AGPL-3.0-only
//! proc_scopes (spec/proc_scopes.md): the threads this host names to a computer's daemon, proc-scopes.ts.
//!
//! The eight cases of test/proc-scopes.test.ts keep the literals that file holds (the pids of its real `sleep`
//! leaders are replaced by fixed ones, the probe by `group_exists`): :72, :90, :113, :124, :147, :156 are here, and
//! :195, :211 are `proc_scopes_terminal_*` in tests/channels_ptys.rs, which need both areas. What that file leaves
//! open, the order of the sources, the caps, a release that is not the owner's, a box's terminals, a refused scopes
//! frame and the standing channel's re-dial, is oracle/proc_scopes.oracle.ts: the TS run over the same worlds,
//! written as tests/fixtures/oracle/proc_scopes.json with each world beside its answer.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{json, Value};

use common::{oracle, plain, run, settle};
use wsp_account::account::core_types::{AccountResult, Frame};
use wsp_account::account::proc_scopes::{KeptRow, LatestRow, ProcScopesArea, ProcScopesCtx, ScopeRecord, ScopeRow, ThreadTerminal};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon::{ProcKind, ProcScope};

fn fx(name: &str) -> Value {
    oracle("proc_scopes")[name].clone()
}

/// A channel that records what it was sent and when it was closed (the number of frames sent by then), and answers
/// every frame ok. `fail_first` makes that many sends fail, as a daemon from before proc.scopes refuses it.
struct Chan {
    sent: RefCell<Vec<Value>>,
    closes: RefCell<Vec<usize>>,
    fail_first: Cell<usize>,
    ended: Cell<bool>,
}

impl Chan {
    fn new() -> Rc<Chan> {
        Rc::new(Chan { sent: RefCell::default(), closes: RefCell::default(), fail_first: Cell::new(0), ended: Cell::new(false) })
    }
    fn sent(&self) -> Vec<Value> {
        self.sent.borrow().clone()
    }
    fn ops(&self) -> Vec<String> {
        self.sent().iter().map(|f| f["op"].as_str().unwrap().to_string()).collect()
    }
}

#[async_trait(?Send)]
impl DaemonChannel for Chan {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.sent.borrow_mut().push(Value::Object(frame.clone()));
        if self.fail_first.get() > 0 {
            self.fail_first.set(self.fail_first.get() - 1);
            return Err(ChannelError::Unanswered { message: "connection lost".into(), cause: None });
        }
        Ok(json!({ "id": frame.get("id").cloned().unwrap_or(Value::Null), "ok": true }).as_object().unwrap().clone())
    }
    fn close(&self) {
        let at = self.sent.borrow().len();
        self.closes.borrow_mut().push(at);
    }
    async fn closed(&self) -> ChannelEnd {
        while !self.ended.get() {
            tokio::task::yield_now().await;
        }
        ChannelEnd { code: 1006, reason: String::new() }
    }
}

/// The context the oracle's `build` made: a world as plain JSON (the fixture holds each one beside its answer).
struct Scopes {
    turn_groups: Vec<(String, Vec<(i64, String)>)>,
    groups: HashSet<i64>,
    sessions: Vec<ScopeRow>,
    kept: RefCell<Vec<(String, KeptRow)>>,
    latest: HashMap<String, LatestRow>,
    thread_records: Vec<(String, String)>,
    terminals: Vec<ThreadTerminal>,
    records: HashMap<String, ScopeRecord>,
    owned: Vec<(String, i64, String)>,
    /// What each call of `channel_here` answers, in turn; none left fails (no daemon wired here).
    dials: RefCell<VecDeque<AccountResult<Rc<Chan>>>>,
    dialled: Cell<usize>,
}

fn kind_of(word: &str) -> WorkspaceKind {
    serde_json::from_value(json!(word)).unwrap()
}

fn proc_kind(word: &str) -> ProcKind {
    serde_json::from_value(json!(word)).unwrap()
}

fn base_live() -> Value {
    fx("main")["world"]["live"].clone()
}

/// `WorldJson` of the oracle with every key a scenario leaves out empty.
fn world(w: &Value) -> Rc<Scopes> {
    let list = |k: &str| w[k].as_array().cloned().unwrap_or_default();
    Rc::new(Scopes {
        turn_groups: list("turnGroups").iter().map(|g| (g[0].as_str().unwrap().to_string(), g[1].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap(), p[1].as_str().unwrap().to_string())).collect())).collect(),
        groups: list("groupsAlive").iter().map(|g| g.as_i64().unwrap()).collect(),
        sessions: list("sessions")
            .iter()
            .map(|s| ScopeRow {
                workspace_id: s["workspace"].as_str().unwrap().into(),
                thread_id: s["thread"].as_str().map(String::from),
                status: s["status"].as_str().unwrap().into(),
                started_at: s["startedAt"].as_f64(),
                pid: s["pid"].as_i64(),
            })
            .collect(),
        kept: RefCell::new(list("kept").iter().map(|k| (k[0].as_str().unwrap().to_string(), KeptRow { workspace_id: k[1]["workspace"].as_str().unwrap().into(), pid: k[1]["pid"].as_i64(), warm_unclaimed: k[1]["warmUnclaimed"] == json!(true) })).collect()),
        latest: w["latest"].as_object().map(|m| m.iter().map(|(t, l)| (t.clone(), LatestRow { workspace_id: l["workspace"].as_str().unwrap().into(), cwd: l["cwd"].as_str().map(String::from) })).collect()).unwrap_or_default(),
        thread_records: list("threadRecords").iter().map(|r| (r[0].as_str().unwrap().to_string(), r[1].as_str().unwrap().to_string())).collect(),
        terminals: list("terminals").iter().map(|t| ThreadTerminal { thread: t["thread"].as_str().unwrap().into(), workspace: t["workspace"].as_str().unwrap().into(), pid: t["pid"].as_i64().unwrap() }).collect(),
        records: w["live"].as_array().unwrap().iter().map(|l| (l["id"].as_str().unwrap().to_string(), ScopeRecord { kind: kind_of(l["kind"].as_str().unwrap()), place: l["place"].as_str().map(String::from), checkout: l["path"].as_str().unwrap().into() })).collect(),
        owned: list("owned").iter().map(|o| (o[0].as_str().unwrap().to_string(), o[1].as_i64().unwrap(), o[2].as_str().unwrap().to_string())).collect(),
        dials: RefCell::default(),
        dialled: Cell::new(0),
    })
}

fn empty_world() -> Value {
    json!({ "live": base_live() })
}

#[async_trait(?Send)]
impl ProcScopesCtx for Scopes {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        self.turn_groups.clone()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.groups.contains(&pgid)
    }
    fn sessions(&self) -> Vec<ScopeRow> {
        self.sessions.clone()
    }
    fn kept_agents(&self) -> Vec<(String, KeptRow)> {
        self.kept.borrow().clone()
    }
    fn latest_on(&self, thread_id: &str) -> Option<LatestRow> {
        self.latest.get(thread_id).cloned()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.clone()
    }
    fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        self.terminals.clone()
    }
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord> {
        self.records.get(workspace_id).cloned()
    }
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool {
        kind == WorkspaceKind::Local
    }
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.dialled.set(self.dialled.get() + 1);
        match self.dials.borrow_mut().pop_front() {
            Some(Ok(c)) => Ok(c),
            Some(Err(e)) => Err(e),
            None => Err(plain("this host wired no daemon here").into()),
        }
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

/// The area over a world, with the world's `ownProcess` calls made in order.
fn area_of(w: &Rc<Scopes>) -> Rc<ProcScopesArea> {
    let area = ProcScopesArea::new(w.clone());
    for (thread, pid, kind) in &w.owned {
        area.own_process(thread, *pid, proc_kind(kind));
    }
    area
}

fn here(area: &Rc<ProcScopesArea>) -> Value {
    serde_json::to_value(area.scopes_here()).unwrap()
}

fn on(area: &Rc<ProcScopesArea>, place: &str) -> Value {
    serde_json::to_value(area.scopes_on(place)).unwrap()
}

fn same(actual: Value, expected: &Value) {
    assert_eq!(actual.to_string(), expected.to_string());
}

fn threads(scopes: &Value) -> Vec<String> {
    scopes.as_array().unwrap().iter().map(|s| s["thread"].as_str().unwrap().to_string()).collect()
}

fn scoped(area: &Rc<ProcScopesArea>, chan: &Rc<Chan>) -> Rc<dyn DaemonChannel> {
    let of = area.clone();
    area.scoped_procs(Rc::new(move || of.scopes_here()), chan.clone())
}

async fn send(c: &Rc<dyn DaemonChannel>, f: Value) -> Value {
    Value::Object(c.send(f.as_object().unwrap().clone()).await.expect("the channel answered"))
}

// P1, P3, TS: test/proc-scopes.test.ts:72
#[test]
fn p1_p3_scopes_here_carry_agents_groups_pids_and_folders() {
    run(async {
        let (agent, kept, gone) = (5000, 5001, 4_194_303);
        let w = world(&json!({
            "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }, { "id": "ws_b", "kind": "local", "path": "/work/ws_b" }],
            "groupsAlive": [agent, kept],
            "sessions": [
                { "thread": "t_tree", "workspace": "ws_a", "status": "running", "startedAt": 1, "pid": agent },
                { "thread": "t_old", "workspace": "ws_b", "status": "completed", "startedAt": 5 },
            ],
            "kept": [["t_kept", { "workspace": "ws_a", "pid": kept }]],
            "turnGroups": [["ws_a", [[agent, "t_tree"], [gone, "t_tree"]]]],
            "latest": { "t_tree": { "workspace": "ws_a", "cwd": "/work/ws_a/.wsp/t_tree" } },
            "owned": [["t_tree", 777, "slate"], ["t_tree", 778, "tool"]],
        }));
        let area = area_of(&w);
        // The TS test's literal.
        let expected = json!([
            { "thread": "t_tree", "workspace": "ws_a", "agents": [agent], "groups": [agent], "folder": "/work/ws_a/.wsp/t_tree", "pids": [{ "pid": 777, "kind": "slate" }, { "pid": 778, "kind": "tool" }] },
            { "thread": "t_kept", "workspace": "ws_a", "agents": [kept], "folder": "/work/ws_a" },
            { "thread": "t_old", "workspace": "ws_b", "folder": "/work/ws_b" },
        ]);
        same(here(&area), &expected);
        // And the TS run over the same world says the same.
        let f = fx("main");
        same(expected, &f["scopes"]);
    });
}

// P1, P3: the order of the sources, the workspace a draft names, the newest thread of each folder.
#[test]
fn p1_sources_in_first_seen_order_and_the_newest_thread_of_each_folder() {
    run(async {
        let f = fx("order");
        let area = area_of(&world(&f["world"]));
        same(here(&area), &f["scopes"]);
    });
}

// P3
#[test]
fn p3_a_workspace_this_host_does_not_hold_has_no_folder() {
    run(async {
        let f = fx("folders");
        let area = area_of(&world(&f["world"]));
        same(here(&area), &f["scopes"]);
    });
}

// P2
#[test]
fn p2_scopes_here_caps_scopes_and_pids() {
    run(async {
        let c = fx("constants");
        assert_eq!((c["PROC_SCOPES_MAX"].as_i64(), c["PROC_SCOPE_PIDS_MAX"].as_i64()), (Some(256), Some(64)));
        // One thread holding 70 agents, 70 groups and 70 owned pids: each list is cut to 64, in insertion order.
        let heavy = |n: i64, from: i64| (0..n).map(|i| from + i).collect::<Vec<_>>();
        let w = world(&json!({
            "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }],
            "groupsAlive": heavy(70, 9000),
            "turnGroups": [["ws_a", heavy(70, 9000).iter().map(|g| json!([g, "th_heavy"])).collect::<Vec<_>>()]],
            "sessions": heavy(70, 1000).iter().map(|pid| json!({ "thread": "th_heavy", "workspace": "ws_a", "status": "running", "startedAt": 1, "pid": pid })).collect::<Vec<_>>(),
            "owned": heavy(70, 3000).iter().map(|pid| json!(["th_heavy", pid, "tool"])).collect::<Vec<_>>(),
        }));
        let scopes = here(&area_of(&w));
        let only = &scopes[0];
        let f = fx("pid_caps");
        let digest = |list: &Value| json!({ "length": list.as_array().unwrap().len(), "first": list[0], "last": list.as_array().unwrap().last().unwrap() });
        same(json!({ "agents": digest(&only["agents"]), "groups": digest(&only["groups"]), "pids": digest(&only["pids"]) }), &f);
        // 300 live threads: 256 scopes, the first 256 in order.
        let live = |n: usize| {
            (0..n).map(|i| json!({ "thread": format!("th_{i:03}"), "workspace": "ws_a", "status": "running", "startedAt": i, "pid": 20_000 + i })).collect::<Vec<_>>()
        };
        let w = world(&json!({ "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }], "sessions": live(300) }));
        let scopes = here(&area_of(&w));
        let names = threads(&scopes);
        let f = fx("scope_cap_live");
        same(json!({ "count": names.len(), "first": names[0], "last": names.last().unwrap() }), &f);
    });
}

// P2: live threads go first, so a folder's newest thread gives way under the cap.
#[test]
fn p2_live_threads_go_first() {
    run(async {
        let live = |n: usize| {
            (0..n).map(|i| json!({ "thread": format!("th_{i:03}"), "workspace": "ws_a", "status": "running", "startedAt": i, "pid": 20_000 + i })).collect::<Vec<_>>()
        };
        let folders = |names: &[&str]| names.iter().map(|n| json!({ "id": format!("ws_{n}"), "kind": "local", "path": format!("/work/{n}") })).collect::<Vec<_>>();
        let done = |n: &str, at: i64| json!({ "thread": n, "workspace": format!("ws_{n}"), "status": "completed", "startedAt": at });
        // 255 live and three folders' newest: the first of them fits.
        let mut sessions = vec![done("n1", 1), done("n2", 2)];
        sessions.extend(live(255));
        sessions.push(done("n3", 3));
        let mut lives = vec![json!({ "id": "ws_a", "kind": "local", "path": "/work/ws_a" })];
        lives.extend(folders(&["n1", "n2", "n3"]));
        let names = threads(&here(&area_of(&world(&json!({ "live": lives, "sessions": sessions })))));
        let f = fx("scope_cap_mixed");
        same(json!({ "count": names.len(), "first": names[0], "last": names.last().unwrap(), "tail": names[names.len() - 3..], "head": names[..2] }), &f);
        // 254 live and two: both fit.
        let mut sessions = vec![done("n1", 1), done("n2", 2)];
        sessions.extend(live(254));
        let mut lives = vec![json!({ "id": "ws_a", "kind": "local", "path": "/work/ws_a" })];
        lives.extend(folders(&["n1", "n2"]));
        let names = threads(&here(&area_of(&world(&json!({ "live": lives, "sessions": sessions })))));
        let f = fx("scope_cap_room");
        same(json!({ "count": names.len(), "tail": names[names.len() - 3..] }), &f);
        // The order the live ones come in, and the non-live after them, is the order case's.
        let order = fx("order");
        let names = threads(&order["scopes"]);
        let live_count = names.iter().position(|n| n == "th_11").unwrap();
        assert_eq!(&names[live_count..], ["th_11", "th_3", "th_13"], "the newest thread of each folder, not live, after every live thread");
    });
}

// P4, TS: test/proc-scopes.test.ts:113
#[test]
fn p4_a_warm_agent_is_named_until_claimed() {
    run(async {
        let w = world(&json!({ "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }], "kept": [["t_warm", { "workspace": "ws_a", "warmUnclaimed": true }]] }));
        let area = area_of(&w);
        let find = |scopes: Value| scopes.as_array().unwrap().iter().find(|s| s["thread"] == json!("t_warm")).cloned();
        assert_eq!(find(here(&area)), Some(json!({ "thread": "t_warm", "workspace": "ws_a", "folder": "/work/ws_a", "warm": true })));
        w.kept.borrow_mut()[0].1.warm_unclaimed = false;
        assert_eq!(find(here(&area)), None);
        let f = fx("warm");
        same(find(here(&area)).unwrap_or(Value::Null), &f["claimed"]);
    });
}

// P5
#[test]
fn p5_a_release_takes_only_its_own_pid() {
    run(async {
        let log = fx("release");
        let w = world(&empty_world());
        let area = area_of(&w);
        let chan = Chan::new();
        let scoped = scoped(&area, &chan);
        send(&scoped, json!({ "id": 1, "op": "proc.watch" })).await;
        let (mut first, mut second): (Rc<dyn Fn()>, Rc<dyn Fn()>) = (Rc::new(|| {}), Rc::new(|| {}));
        let mut step = 0;
        macro_rules! step {
            ($body:expr) => {{
                let before = chan.sent().len();
                $body;
                settle().await;
                let told: Vec<Value> = chan.sent()[before..].iter().map(|f| f["scopes"].clone()).collect();
                let got = json!({ "name": log[step]["name"], "scopes": told, "now": here(&area) });
                assert_eq!(got.to_string(), log[step].to_string());
                step += 1;
            }};
        }
        step!(first = area.own_process("t_x", 900, proc_kind("slate")));
        step!(second = area.own_process("t_y", 900, proc_kind("tool")));
        step!(first());
        step!(second());
        step!(second());
        step!(first = area.own_process("t_x", 901, proc_kind("slate")));
        step!(first());
        step!(first());
        assert_eq!(step, log.as_array().unwrap().len());
    });
}

// P6, TS: test/proc-scopes.test.ts:156
#[test]
fn p6_scopes_on_a_box_newest_first() {
    run(async {
        let w = world(&json!({
            "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }, { "id": "ws_box", "kind": "place", "place": "pl_1", "path": "/box/ws_box" }],
            "sessions": [
                { "thread": "t_new", "workspace": "ws_box", "status": "running", "startedAt": 9 },
                { "thread": "t_here", "workspace": "ws_a", "status": "running", "startedAt": 10 },
            ],
            "threadRecords": [["t_quiet", "ws_box"]],
        }));
        let area = area_of(&w);
        let cgroup = |t: &str| fx("scopes_on")["cgroup"][t].clone();
        assert_eq!(cgroup("t_new"), json!("/wsp-threads/t_new"));
        let expected = json!([
            { "thread": "t_new", "workspace": "ws_box", "folder": "/box/ws_box", "cgroup": "/wsp-threads/t_new" },
            { "thread": "t_quiet", "workspace": "ws_box", "folder": "/box/ws_box", "cgroup": "/wsp-threads/t_quiet" },
        ]);
        same(on(&area, "pl_1"), &expected);
        assert_eq!(on(&area, "pl_none"), json!([]));
    });
}

// P6: the newest start per thread, the folder off the latest row, the terminals, the other computers' threads left out.
#[test]
fn p6_terminals_on_a_box() {
    run(async {
        let f = fx("scopes_on");
        let area = area_of(&world(&f["world"]));
        same(on(&area, "pl_1"), &f["pl_1"]);
        same(on(&area, "pl_2"), &f["pl_2"]);
        same(on(&area, "pl_none"), &f["pl_none"]);
    });
}

// P7, TS: test/proc-scopes.test.ts:90
#[test]
fn p7_named_before_a_watch_and_as_they_move() {
    run(async {
        let w = world(&json!({ "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }] }));
        let area = area_of(&w);
        let chan = Chan::new();
        let scoped = scoped(&area, &chan);
        let release = area.own_process("t_x", 900, proc_kind("slate"));
        assert!(chan.sent().is_empty());
        send(&scoped, json!({ "id": 1, "op": "proc.watch" })).await;
        assert_eq!(chan.ops(), ["proc.scopes", "proc.watch"]);
        assert_eq!(chan.sent()[0]["scopes"], json!([{ "thread": "t_x", "pids": [{ "pid": 900, "kind": "slate" }] }]));
        area.proc_scopes_moved();
        assert_eq!(chan.sent().len(), 2);
        release();
        settle().await;
        assert_eq!(chan.ops(), ["proc.scopes", "proc.watch", "proc.scopes"]);
        assert_eq!(chan.sent()[2]["scopes"], json!([]));
        send(&scoped, json!({ "id": 2, "op": "proc.unwatch" })).await;
        area.own_process("t_x", 901, proc_kind("tool"));
        settle().await;
        assert_eq!(chan.ops(), ["proc.scopes", "proc.watch", "proc.scopes", "proc.unwatch"]);
        assert_eq!(chan.sent()[0], json!({ "id": null, "op": "proc.scopes", "scopes": [{ "thread": "t_x", "pids": [{ "pid": 900, "kind": "slate" }] }] }));
    });
}

// P7: a refused scopes frame is ignored, a second watch is told again, the push leaves at close and at the channel's end.
#[test]
fn p7_a_refused_scopes_frame_is_ignored_and_the_push_leaves_at_close_and_at_the_end() {
    run(async {
        let f = fx("scoped");
        let area = area_of(&world(&empty_world()));
        let chan = Chan::new();
        chan.fail_first.set(1);
        let scoped = scoped(&area, &chan);
        let mut log: Vec<Value> = vec![];
        let mut next = 0;
        macro_rules! step {
            ($body:expr) => {{
                let before = chan.sent().len();
                let reply: Value = $body;
                settle().await;
                let got = json!({ "name": f["log"][next]["name"], "reply": reply, "thrown": null, "sent": chan.sent()[before..].to_vec() });
                log.push(got);
                next += 1;
            }};
        }
        area.own_process("t_x", 900, proc_kind("slate"));
        step!(send(&scoped, json!({ "id": 1, "op": "proc.watch", "cgroups": ["a"] })).await);
        step!(send(&scoped, json!({ "id": 2, "op": "proc.watch" })).await);
        step!({
            area.own_process("t_x", 901, proc_kind("tool"));
            Value::Null
        });
        step!({
            area.proc_scopes_moved();
            Value::Null
        });
        step!(send(&scoped, json!({ "id": 3, "op": "proc.unwatch" })).await);
        step!({
            area.own_process("t_x", 902, proc_kind("tool"));
            Value::Null
        });
        step!(send(&scoped, json!({ "id": 4, "op": "pty.list" })).await);
        step!(send(&scoped, json!({ "id": 5, "op": "proc.watch" })).await);
        step!({
            scoped.close();
            Value::Null
        });
        step!({
            area.own_process("t_x", 903, proc_kind("tool"));
            Value::Null
        });
        for (i, got) in log.iter().enumerate() {
            assert_eq!(got.to_string(), f["log"][i].to_string(), "step {i}: {}", f["log"][i]["name"]);
        }
        assert_eq!(json!(*chan.closes.borrow()), f["closes"]);
        // The channel's own end takes the push away too.
        let ended = Chan::new();
        let s2 = scoped_over(&area, &ended);
        send(&s2, json!({ "id": 1, "op": "proc.watch" })).await;
        let watched = ended.sent().len();
        ended.ended.set(true);
        settle().await;
        area.own_process("t_x", 904, proc_kind("tool"));
        settle().await;
        assert_eq!(json!(ended.sent().len() - watched), f["afterEnd"]);
    });
}

fn scoped_over(area: &Rc<ProcScopesArea>, chan: &Rc<Chan>) -> Rc<dyn DaemonChannel> {
    scoped(area, chan)
}

// P8, TS: test/proc-scopes.test.ts:124
#[test]
fn p8_this_computers_daemon_is_told_while_a_root_stands() {
    run(async {
        let w = world(&json!({ "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }] }));
        let chan = Chan::new();
        w.dials.borrow_mut().push_back(Ok(chan.clone()));
        let area = area_of(&w);
        area.proc_scopes_moved();
        settle().await;
        assert_eq!((w.dialled.get(), chan.sent()), (0, vec![]));
        let release = area.own_process("t_x", 900, proc_kind("slate"));
        settle().await;
        area.proc_scopes_moved();
        settle().await;
        assert_eq!(w.dialled.get(), 1);
        assert_eq!(chan.sent(), vec![json!({ "id": null, "op": "proc.scopes", "scopes": [{ "thread": "t_x", "pids": [{ "pid": 900, "kind": "slate" }] }] })]);
        let second = area.own_process("t_x", 901, proc_kind("tool"));
        settle().await;
        assert_eq!(chan.sent().len(), 2);
        second();
        release();
        settle().await;
        assert_eq!((w.dialled.get(), chan.sent().len(), chan.closes.borrow().clone()), (1, 4, vec![4]));
        assert_eq!(chan.sent()[3], json!({ "id": null, "op": "proc.scopes", "scopes": [] }));
    });
}

// P8, TS: test/proc-scopes.test.ts:147
#[test]
fn p8_never_dialled_without_a_daemon_here() {
    run(async {
        let w = world(&json!({ "live": [{ "id": "ws_a", "kind": "local", "path": "/work/ws_a" }] }));
        let area = area_of(&w);
        area.own_process("t_x", 900, proc_kind("slate"));
        settle().await;
        assert_eq!(area.scopes_here().len(), 1);
        assert_eq!(serde_json::to_value(area.scopes_here()).unwrap(), json!([{ "thread": "t_x", "pids": [{ "pid": 900, "kind": "slate" }] }]));
        assert_eq!(fx("no_daemon")["scopes"], json!([{ "thread": "t_x", "pids": [{ "pid": 900, "kind": "slate" }] }]));
    });
}

// P8
#[test]
fn p8_a_failed_dial_dials_again() {
    run(async {
        let f = fx("standing");
        let w = world(&empty_world());
        let area = area_of(&w);
        let chans = [Chan::new(), Chan::new(), Chan::new()];
        w.dials.borrow_mut().push_back(Err(plain("no daemon here").into()));
        w.dials.borrow_mut().push_back(Ok(chans[1].clone()));
        w.dials.borrow_mut().push_back(Ok(chans[2].clone()));
        let mut two: Rc<dyn Fn()> = Rc::new(|| {});
        let mut next = 0;
        macro_rules! step {
            ($body:expr) => {{
                $body;
                settle().await;
                // The oracle counts a dial that threw as an opened channel with nothing sent.
                let opened = w.dialled.get();
                let sent: Vec<Value> = (0..opened).map(|i| if i == 0 { json!([]) } else { json!(chans[i].sent()) }).collect();
                let closes: Vec<Value> = (0..opened).map(|i| if i == 0 { json!([]) } else { json!(chans[i].closes.borrow().clone()) }).collect();
                let got = json!({ "name": f[next]["name"], "opened": opened, "sent": sent, "closes": closes });
                assert_eq!(got.to_string(), f[next].to_string(), "step {next}: {}", f[next]["name"]);
                next += 1;
            }};
        }
        step!(area.proc_scopes_moved());
        step!({
            area.own_process("t_x", 900, proc_kind("slate"));
        });
        step!({
            two = area.own_process("t_x", 901, proc_kind("tool"));
        });
        step!(area.proc_scopes_moved());
        chans[1].ended.set(true);
        settle().await;
        step!({
            area.own_process("t_x", 902, proc_kind("tool"));
        });
        step!(two());
        assert_eq!(next, f.as_array().unwrap().len());
    });
}
