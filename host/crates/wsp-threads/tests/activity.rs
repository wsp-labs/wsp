// SPDX-License-Identifier: AGPL-3.0-only
//! spec/activity.md. TS cases: host/test/activity.test.ts (24), runtime/test/box-activity.test.ts (7).
//!
//! The TS drives the command line over a fake daemon; here `Activity::new(host)` runs over the `FakeHost`'s
//! `ActivityCtx` and a fake daemon of this file (a port of host/test/fake-procs.ts) answers each channel the host
//! opens, logging every frame it got in order and whether the channel was closed.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use support::*;
use wsp_account::account::core_types::AccountError;
use wsp_core::types::internal::DaemonRefusal;
use wsp_wire::views::activity::ActivityScope;
use wsp_wire::views::activity::{
    box_behind_activity_line, box_end_line, end_reused_line, not_joined_activity_fix, not_read_line, not_started_line, not_threads_line,
    LeftThread, BOX_BEHIND_ACTIVITY_FIX, BOX_END_FIX, CLOUD_ACTIVITY_FIX, END_AGAIN_FIX, END_GRACE_MS, NOT_READ_FIX, NOT_STARTED_FIX,
    NOT_THREADS_FIX,
};
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::exit::{not_found_refusal, usage_refusal};
use wsp_words::thread_cgroup::thread_cgroup;
use wsp_words::views::activity::{
    agent_end_fix, agent_end_line, cloud_activity_line, not_joined_activity_line, processes_left_refusal, LeftVerb,
};
use wsp_words::words::computer::{thread_word, THIS_COMPUTER};

const T1: &str = "3f2a9c10-1111-4111-8111-111111111111";
const T2: &str = "7b41e0d2-2222-4222-8222-222222222222";
const T3: &str = "c90d5a77-3333-4333-8333-333333333333";
const STARTED: i64 = T0;
const VITE_ID: &str = "88000";

// ---------------------------------------------------------------------------------------------------------------
// The fake daemon: fake-procs.ts, answering the frames of every channel the host opens.
// ---------------------------------------------------------------------------------------------------------------

/// One channel as the daemon saw it: whether it came through the kill guard, every frame, and whether it was closed.
#[derive(Clone, Debug, PartialEq)]
struct ChanLog {
    guarded: bool,
    frames: Vec<Value>,
    closed: bool,
}

fn chan(guarded: bool, frames: Vec<Value>) -> ChanLog {
    ChanLog { guarded, frames, closed: true }
}

struct Daemon {
    clock: FakeClock,
    pid: u32,
    procs: RefCell<Vec<Value>>,
    ports: RefCell<Value>,
    cgroups: RefCell<Option<Value>>,
    lists: Cell<bool>,
    modes: Cell<bool>,
    stubborn: RefCell<Vec<u32>>,
    list_error: RefCell<Option<AccountError>>,
    ports_error: RefCell<Option<AccountError>>,
    watch_events: RefCell<Option<Vec<Value>>>,
    after_read: RefCell<Option<Box<dyn FnOnce(&Daemon)>>>,
    pty_kill: RefCell<Rc<dyn Fn(&Daemon, &str) -> Result<Map<String, Value>, AccountError>>>,
    kill_reply: RefCell<Option<Result<Map<String, Value>, AccountError>>>,
    hold_kills: Cell<usize>,
    first_answer_saw: Cell<Option<usize>>,
    kills: RefCell<Vec<Value>>,
    signals: RefCell<Vec<(u32, String)>>,
    chans: RefCell<Vec<ChanLog>>,
    direct: RefCell<Vec<Value>>,
    seq: RefCell<Vec<String>>,
}

struct Chan {
    d: Rc<Daemon>,
    idx: usize,
    sink: DaemonEventListener,
}

impl DaemonAsk for Chan {
    fn ask(&self, frame: Value) -> LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>> {
        self.d.chans.borrow_mut()[self.idx].frames.push(frame.clone());
        self.d.seq.borrow_mut().push(format!("c{}:{}", self.idx, frame["op"].as_str().unwrap_or("")));
        self.d.answer(&frame, Some(&self.sink))
    }
    fn close(&self) {
        self.d.chans.borrow_mut()[self.idx].closed = true;
        self.d.seq.borrow_mut().push(format!("c{}:close", self.idx));
    }
}

fn reply(payload: Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(1));
    m.insert("ok".into(), json!(true));
    for (k, v) in payload.as_object().expect("a payload object") {
        m.insert(k.clone(), v.clone());
    }
    m
}

fn refused(code: Option<&str>, message: impl Into<String>) -> AccountError {
    AccountError::Refusal(DaemonRefusal { code: code.map(String::from), message: message.into() })
}

impl Daemon {
    fn new(clock: &FakeClock) -> Rc<Daemon> {
        Rc::new(Daemon {
            clock: clock.clone(),
            pid: 1,
            procs: RefCell::new(Vec::new()),
            ports: RefCell::new(json!([])),
            cgroups: RefCell::new(None),
            lists: Cell::new(true),
            modes: Cell::new(true),
            stubborn: RefCell::new(Vec::new()),
            list_error: RefCell::new(None),
            ports_error: RefCell::new(None),
            watch_events: RefCell::new(None),
            after_read: RefCell::new(None),
            pty_kill: RefCell::new(Rc::new(|_d: &Daemon, _p: &str| Ok(reply(json!({}))))),
            kill_reply: RefCell::new(None),
            hold_kills: Cell::new(0),
            first_answer_saw: Cell::new(None),
            kills: RefCell::new(Vec::new()),
            signals: RefCell::new(Vec::new()),
            chans: RefCell::new(Vec::new()),
            direct: RefCell::new(Vec::new()),
            seq: RefCell::new(Vec::new()),
        })
    }

    fn set_procs(&self, rows: Vec<Value>) {
        *self.procs.borrow_mut() = rows;
    }

    fn set_ports(&self, ports: &[(u32, u32)]) {
        *self.ports.borrow_mut() =
            Value::Array(ports.iter().map(|(port, pid)| json!({ "port": port, "pid": pid, "uid": 0, "loopback": false })).collect());
    }

    fn open(self: &Rc<Self>, sink: DaemonEventListener, guarded: bool) -> Rc<dyn DaemonAsk> {
        let idx = self.chans.borrow().len();
        self.chans.borrow_mut().push(ChanLog { guarded, frames: Vec::new(), closed: false });
        Rc::new(Chan { d: self.clone(), idx, sink })
    }

    fn chans(&self) -> Vec<ChanLog> {
        self.chans.borrow().clone()
    }

    fn tree_of(&self, pid: u32) -> Vec<u32> {
        let rows = self.procs.borrow().clone();
        let mut out = vec![pid];
        for r in rows.iter().filter(|r| r["ppid"] == pid && r["pid"] != pid) {
            out.extend(self.tree_of(r["pid"].as_u64().unwrap() as u32));
        }
        out
    }

    fn snapshot(&self) -> Value {
        let procs = self.procs.borrow().clone();
        let mut m = json!({ "daemon": self.pid, "total": procs.len(), "at": self.clock.now(), "procs": procs });
        if let Some(c) = self.cgroups.borrow().clone() {
            m["cgroups"] = c;
        }
        m
    }

    fn run_after_read(&self) {
        let after = self.after_read.borrow_mut().take();
        if let Some(f) = after {
            f(self);
        }
    }

    /// A frame sent outside any channel (the host's `withDaemon` on a box).
    fn ask_direct(self: &Rc<Self>, frame: Value) -> LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>> {
        self.direct.borrow_mut().push(frame.clone());
        self.seq.borrow_mut().push(format!("direct:{}", frame["op"].as_str().unwrap_or("")));
        self.answer(&frame, None)
    }

    fn answer(
        self: &Rc<Self>,
        frame: &Value,
        sink: Option<&DaemonEventListener>,
    ) -> LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>> {
        let op = frame["op"].as_str().unwrap_or("").to_string();
        let result: Result<Map<String, Value>, AccountError> = match op.as_str() {
            "ports.watch" => match self.ports_error.borrow().clone() {
                Some(e) => Err(e),
                None => Ok(reply(json!({ "ports": self.ports.borrow().clone() }))),
            },
            "proc.list" => {
                if let Some(e) = self.list_error.borrow().clone() {
                    Err(e)
                } else if !self.lists.get() {
                    Err(refused(None, unknown_op_line("proc.list")))
                } else {
                    let answered = Ok(reply(self.snapshot()));
                    self.run_after_read();
                    answered
                }
            }
            "proc.watch" => {
                let events = match self.watch_events.borrow().clone() {
                    Some(e) => e,
                    None => {
                        let mut snap = self.snapshot();
                        snap["type"] = json!("proc.snapshot");
                        vec![snap]
                    }
                };
                if let Some(sink) = sink {
                    let sink = sink.clone();
                    let d = self.clone();
                    tokio::task::spawn_local(async move {
                        for e in events {
                            sink(e.as_object().expect("an event object").clone());
                        }
                        d.run_after_read();
                    });
                }
                Ok(reply(json!({})))
            }
            "proc.kill" => self.kill(frame),
            "pty.kill" => {
                let hook = self.pty_kill.borrow().clone();
                hook(self, frame["ptyId"].as_str().unwrap_or(""))
            }
            other => Err(refused(None, format!("the fake daemon answers no {other}"))),
        };
        if op == "proc.kill" && self.hold_kills.get() > 0 {
            let d = self.clone();
            let want = self.hold_kills.get();
            return async move {
                for _ in 0..200 {
                    if d.kills.borrow().len() >= want {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                if d.first_answer_saw.get().is_none() {
                    d.first_answer_saw.set(Some(d.kills.borrow().len()));
                }
                result
            }
            .boxed_local();
        }
        futures::future::ready(result).boxed_local()
    }

    fn kill(&self, frame: &Value) -> Result<Map<String, Value>, AccountError> {
        self.kills.borrow_mut().push(frame.clone());
        if let Some(scripted) = self.kill_reply.borrow().clone() {
            return scripted;
        }
        let pid = frame["pid"].as_u64().unwrap_or(0) as u32;
        let rows = self.procs.borrow().clone();
        let Some(row) = rows.iter().find(|p| p["pid"] == pid) else {
            return Err(refused(Some("not-found"), format!("no process {pid}")));
        };
        let not_listed = || refused(Some("not-found"), format!("pid {pid} is not the process listed"));
        let modes = self.modes.get();
        if modes {
            if let Some(id) = frame.get("identity").and_then(Value::as_str) {
                if row["id"].as_str() != Some(id) {
                    return Err(not_listed());
                }
            } else if let Some(at) = frame["startedAt"].as_i64() {
                if (row["startedAt"].as_i64().unwrap_or(0) - at).abs() > 2_000 {
                    return Err(not_listed());
                }
            }
        }
        let signal = frame["signal"].as_str().unwrap_or("").to_string();
        let taken = if modes && frame["mode"] == "tree" { self.tree_of(pid) } else { vec![pid] };
        for t in &taken {
            self.signals.borrow_mut().push((*t, signal.clone()));
        }
        let stubborn = self.stubborn.borrow().clone();
        let forced: Vec<u32> =
            if modes && signal == "TERM" { taken.iter().copied().filter(|t| stubborn.contains(t)).collect() } else { Vec::new() };
        for t in &forced {
            self.signals.borrow_mut().push((*t, "KILL".into()));
        }
        self.procs.borrow_mut().retain(|p| {
            let at = p["pid"].as_u64().unwrap() as u32;
            !taken.contains(&at) || (signal == "TERM" && !modes && stubborn.contains(&at))
        });
        Ok(reply(if modes { json!({ "signalled": taken, "forced": forced, "left": [] }) } else { json!({}) }))
    }
}

fn host_err(e: AccountError) -> HostError {
    match e {
        AccountError::Host(h) => h,
        AccountError::Refusal(r) => HostError::new(r.message),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The process tables, as the TS cases write them, and the rows an ActivityView names them by.
// ---------------------------------------------------------------------------------------------------------------

fn proc_row(pid: u32, extra: Value) -> Value {
    let mut d = json!({ "pid": pid, "ppid": 1, "user": "ada", "state": "S", "comm": "node", "cmdline": "node", "cpu": 0, "rss": 1_000_000, "startedAt": STARTED });
    for (k, v) in extra.as_object().expect("extra is an object") {
        d[k] = v.clone();
    }
    d
}

fn owned(by: &str, thread: &str) -> Value {
    json!({ "by": by, "thread": thread })
}

fn daemon_proc() -> Value {
    proc_row(1, json!({ "ppid": 0, "comm": "wsp-daemon", "cmdline": "wsp-daemon serve", "rss": 9_000_000 }))
}

/// threadWithServer's table (activity.test.ts:28): the thread's agent with its tool server, a dev server with its
/// child, the daemon, and the person's own shell that nobody started through wsp.
fn main_table(thread: &str) -> Vec<Value> {
    let owner = owned("fork", thread);
    vec![
        daemon_proc(),
        proc_row(
            4100,
            json!({ "comm": "2.1.296", "cmdline": "claude -p --input-format stream-json", "rss": 300_000_000, "cpu": 3, "kind": "agent", "owner": owner, "startedAt": STARTED }),
        ),
        proc_row(
            4150,
            json!({ "ppid": 4100, "cmdline": "node wsp mcp --scoped", "rss": 70_000_000, "owner": owner, "startedAt": STARTED + 500 }),
        ),
        proc_row(
            4200,
            json!({ "cmdline": "node /lab/node_modules/.bin/vite", "rss": 80_000_000, "cpu": 10, "kind": "dev", "owner": owner, "startedAt": STARTED + 1_000, "id": VITE_ID }),
        ),
        proc_row(
            4300,
            json!({ "ppid": 4200, "comm": "esbuild", "cmdline": "/lab/node_modules/esbuild/bin/esbuild --service", "rss": 20_000_000, "owner": owner, "startedAt": STARTED + 2_000, "id": "88001" }),
        ),
        proc_row(5000, json!({ "ppid": 4999, "comm": "zsh", "cmdline": "-zsh", "rss": 4_000_000 })),
    ]
}

fn vitest_proc(thread: &str) -> Value {
    proc_row(
        4400,
        json!({ "cmdline": "node /lab/node_modules/.bin/vitest", "kind": "test", "owner": owned("fork", thread), "startedAt": STARTED + 3_000, "id": "88002" }),
    )
}

fn shell_procs(thread: &str) -> Vec<Value> {
    let owner = owned("root", thread);
    vec![
        proc_row(
            5100,
            json!({ "comm": "zsh", "cmdline": "-zsh", "pty": "pty_1", "kind": "terminal", "owner": owner, "startedAt": STARTED + 5_000, "id": "88100" }),
        ),
        proc_row(
            5110,
            json!({ "ppid": 5100, "cmdline": "node /lab/node_modules/.bin/vite --port 5199", "kind": "terminal", "owner": owner, "startedAt": STARTED + 5_100, "id": "88110" }),
        ),
    ]
}

/// One row of an ActivityView.
#[allow(clippy::too_many_arguments)]
fn shown(
    pid: u32,
    ppid: u32,
    depth: u32,
    started: i64,
    kind: &str,
    name: &str,
    command: &str,
    memory: u64,
    cpu: u32,
    ports: &[u32],
) -> Value {
    json!({ "pid": pid, "ppid": ppid, "depth": depth, "startedAt": started, "kind": kind, "name": name, "command": command, "memory": memory, "cpu": cpu, "ports": ports })
}

fn vite() -> Value {
    shown(4200, 1, 0, STARTED + 1_000, "dev", "node", "node /lab/node_modules/.bin/vite", 80_000_000, 10, &[5173])
}
fn esbuild() -> Value {
    shown(4300, 4200, 1, STARTED + 2_000, "dev", "esbuild", "/lab/node_modules/esbuild/bin/esbuild --service", 20_000_000, 0, &[])
}
fn agent() -> Value {
    shown(4100, 1, 0, STARTED, "agent", "2.1.296", "claude -p --input-format stream-json", 300_000_000, 3, &[])
}
fn tool() -> Value {
    shown(4150, 4100, 1, STARTED + 500, "tool", "node", "node wsp mcp --scoped", 70_000_000, 0, &[])
}
fn daemon_row() -> Value {
    shown(1, 0, 0, STARTED, "wsp", "wsp-daemon", "wsp-daemon serve", 9_000_000, 0, &[])
}
fn vitest() -> Value {
    shown(4400, 1, 0, STARTED + 3_000, "test", "node", "node /lab/node_modules/.bin/vitest", 1_000_000, 0, &[])
}
fn shell() -> Value {
    shown(5100, 1, 0, STARTED + 5_000, "terminal", "zsh", "-zsh", 1_000_000, 0, &[])
}
fn shell_child() -> Value {
    shown(5110, 5100, 1, STARTED + 5_100, "dev", "node", "node /lab/node_modules/.bin/vite --port 5199", 1_000_000, 0, &[])
}
fn python() -> Value {
    shown(4800, 1, 0, STARTED + 8_000, "dev", "python3", "python3 -m http.server", 1_000_000, 0, &[])
}

fn thread_json(id: &str, title: &str, workspace: &str, memory: u64, cpu: u32, processes: Vec<Value>) -> Value {
    json!({ "threadId": id, "title": title, "workspaceId": workspace, "memory": memory, "cpu": cpu, "processes": processes })
}

fn view_json(computer: &str, at: i64, threads: Vec<Value>, wsp: Vec<Value>, unowned: Vec<Value>) -> Value {
    json!({ "computer": computer, "at": at, "threads": threads, "wsp": wsp, "unowned": unowned })
}

/// The whole read activity.test.ts:48 expects: the thread's tree, then wsp's own, and nothing of the person's shell.
fn main_thread(id: &str, title: &str) -> Value {
    thread_json(id, title, "w1", 470_000_000, 13, vec![vite(), esbuild(), agent(), tool()])
}

fn left_thread(id: &str, title: &str, workspace: &str, memory: u64, cpu: u32, processes: Vec<Value>, on: Option<&str>) -> LeftThread {
    let mut v = thread_json(id, title, workspace, memory, cpu, processes);
    if let Some(on) = on {
        v["on"] = json!(on);
    }
    serde_json::from_value(v).expect("a left thread")
}

fn json_of<T: serde::Serialize>(t: &T) -> Value {
    serde_json::to_value(t).expect("serializes")
}

fn ended(pid: u32, rows: Vec<Value>, forced: &[u32], left: &[u32]) -> Value {
    json!({ "pid": pid, "ended": rows, "forced": forced, "left": left })
}

fn kill_frame(pid: u32, started: i64, identity: Option<&str>) -> Value {
    let mut f = json!({ "op": "proc.kill", "pid": pid, "signal": "TERM", "mode": "tree", "startedAt": started });
    if let Some(i) = identity {
        f["identity"] = json!(i);
    }
    f
}

fn watch_frames() -> Vec<Value> {
    vec![json!({ "op": "ports.watch" }), json!({ "op": "proc.watch" })]
}

fn list_frames() -> Vec<Value> {
    vec![json!({ "op": "ports.watch" }), json!({ "op": "proc.list" })]
}

// ---------------------------------------------------------------------------------------------------------------
// The lab: a FakeHost with threads and a daemon on this computer, and a box beside it.
// ---------------------------------------------------------------------------------------------------------------

struct Lab {
    host: Rc<FakeHost>,
    here: Rc<Daemon>,
    boxd: Rc<Daemon>,
    facts: Rc<RefCell<HashMap<String, ThreadFacts>>>,
    door: Activity,
}

fn facts_of(thread: &str, workspace: &str, title: &str) -> ThreadFacts {
    serde_json::from_value(json!({
        "id": thread, "workspaceId": workspace, "harness": "claude", "startedBy": "person", "status": "completed", "title": title,
        "sessionId": format!("s-{thread}"), "turns": 1, "ran": true,
    }))
    .expect("thread facts")
}

fn place_view(id: &str, name: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": id, "kind": "computer", "name": name, "default": false })).expect("a place view")
}

fn scope(doc: Value) -> ProcScope {
    serde_json::from_value(doc).expect("a proc scope")
}

fn scoped_by(thread: &str, workspace: &str) -> Caller {
    Caller::Scoped {
        origin: WorkspaceOrigin::Here,
        by: serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": thread }))
            .expect("a thread scope"),
    }
}

fn relayed() -> Caller {
    Caller::Road(WorkspaceOrigin::Relayed)
}

fn is_relayed(c: Option<&Caller>) -> bool {
    matches!(c, Some(Caller::Road(WorkspaceOrigin::Relayed)))
}

fn ask_of(thread: Option<&str>, computer: Option<&str>) -> ActivityAsk {
    ActivityAsk { computer: computer.map(String::from), thread: thread.map(String::from), thread_ids: None }
}

fn end_of(pid: u32, started: i64) -> EndAsk {
    EndAsk { computer: None, pid, started_at: started as f64 }
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

impl Lab {
    fn new() -> Lab {
        let host = FakeHost::new();
        let here = Daemon::new(&host.clock);
        let boxd = Daemon::new(&host.clock);
        host.hooks.set::<dyn Fn() -> bool>("ActivityCtx::has_local_daemon", Rc::new(|| true));
        let d = here.clone();
        host.hooks.set::<dyn Fn(DaemonEventListener, bool) -> LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>(
            "ActivityCtx::local_channel",
            Rc::new(move |sink: DaemonEventListener, guarded: bool| {
                let c = d.open(sink, guarded);
                async move { Ok(c) }.boxed_local()
            }),
        );
        let facts: Rc<RefCell<HashMap<String, ThreadFacts>>> = Rc::new(RefCell::new(HashMap::new()));
        let held = facts.clone();
        host.hooks
            .set::<dyn Fn(&str) -> Option<ThreadFacts>>("ActivityCtx::thread_facts", Rc::new(move |t: &str| held.borrow().get(t).cloned()));
        let door = Activity::new(host.clone());
        Lab { host, here, boxd, facts, door }
    }

    fn thread_in(&self, thread: &str, workspace: &str, workspace_extra: Value, title: &str, started: i64, status: &str) {
        if self.host.live(workspace).is_none() {
            self.host.add_entry(entry(workspace, workspace_extra));
        }
        let session = format!("s-{thread}");
        self.host.add_row(
            &session,
            row(
                view(&session, workspace, json!({ "threadId": thread, "startedAt": started, "status": status })),
                &format!("turn-{thread}"),
            ),
        );
        self.host.add_record(thread, workspace, thread_record(json!({ "harness": "claude" })));
        self.facts.borrow_mut().insert(thread.to_string(), facts_of(thread, workspace, title));
    }

    /// A thread whose turns ran in a folder on this computer.
    fn local_thread(&self, thread: &str, workspace: &str, title: &str, started: i64, status: &str) {
        self.thread_in(thread, workspace, json!({}), title, started, status);
    }

    /// A thread whose folder is on the joined box, place `box`.
    fn box_thread(&self, thread: &str, workspace: &str, title: &str, started: i64) {
        self.thread_in(thread, workspace, json!({ "kind": "place", "place": "box" }), title, started, "completed");
    }

    fn scopes(&self, scopes: Vec<Value>) {
        let all: Vec<ProcScope> = scopes.into_iter().map(scope).collect();
        self.host.hooks.set::<dyn Fn() -> Vec<ProcScope>>("ActivityCtx::scopes_here", Rc::new(move || all.clone()));
    }

    /// The box `hetzner`, place id `box`, joined and linked, its daemon answering `boxd`.
    fn join_box(&self) {
        let h = &self.host.hooks;
        h.set::<dyn Fn() -> bool>("ActivityCtx::has_door", Rc::new(|| true));
        h.set::<dyn Fn(&str) -> Option<String>>("ActivityCtx::place_name", Rc::new(|p: &str| (p == "box").then(|| "hetzner".to_string())));
        h.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Vec<PlaceView>, HostError>>>(
            "ActivityCtx::place_rows",
            Rc::new(|| async { Ok(vec![place_view("box", "hetzner")]) }.boxed_local()),
        );
        h.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|p: &str| p == "box"));
        h.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|p: &str| p == "box"));
        let d = self.boxd.clone();
        h.set::<dyn Fn(&str, DaemonEventListener) -> Option<LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>>(
            "ActivityCtx::box_channel",
            Rc::new(move |_p: &str, sink: DaemonEventListener| {
                let c = d.open(sink, false);
                Some(async move { Ok(c) }.boxed_local())
            }),
        );
        let d = self.boxd.clone();
        h.set::<dyn Fn(&Entry, Value) -> LocalBoxFuture<'static, Result<Map<String, Value>, HostError>>>(
            "ActivityCtx::with_daemon",
            Rc::new(move |_e: &Entry, frame: Value| {
                let asked = d.ask_direct(frame);
                async move { asked.await.map_err(host_err) }.boxed_local()
            }),
        );
    }

    fn drives(&self, f: impl Fn(WorkspaceKind, Option<&Caller>) -> bool + 'static) {
        self.host.hooks.set::<dyn Fn(WorkspaceKind, Option<&Caller>) -> bool>(
            "ActivityCtx::drives",
            Rc::new(move |k: WorkspaceKind, c: Option<&Caller>| f(k, c)),
        );
    }

    fn kept(&self, thread: &'static str, pid: u32) {
        self.host.hooks.set::<dyn Fn(&str) -> Option<u32>>("ActivityCtx::kept_pid", Rc::new(move |t: &str| (t == thread).then_some(pid)));
    }

    /// This computer's main scenario: T1 finished in w1 beside a daemon listing `main_table(T1)` and one listener.
    fn main() -> Lab {
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "start the dev server", STARTED, "completed");
        lab.here.set_procs(main_table(T1));
        lab.here.set_ports(&[(5173, 4200)]);
        lab
    }

    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError> {
        self.door.read(ask, caller).await
    }

    async fn end(&self, pid: u32, started: i64, caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        self.door.end(end_of(pid, started), caller).await
    }

    async fn meet(
        &self,
        threads: &[&str],
        processes: ProcessesAsk,
        verb: MeetVerb,
        caller: Option<Caller>,
    ) -> Result<Vec<ActivityEnded>, HostError> {
        self.door.meet(&ids(threads), processes, verb, caller).await
    }
}

fn view_of(r: Result<ActivityView, HostError>) -> Value {
    json_of(&r.expect("the read answers"))
}

fn ended_of(r: Result<ActivityEnded, HostError>) -> Value {
    json_of(&r.expect("the end answers"))
}

fn all_ended(r: Result<Vec<ActivityEnded>, HostError>) -> Value {
    json_of(&r.expect("the meet answers"))
}

/// exit.ts:72: a refusal's message is `refusalLine(happened, fix)`.
fn joined(happened: &str, fix: &str) -> String {
    wsp_words::words::thread::refusal_line(happened, fix)
}

fn said_of<T: std::fmt::Debug>(r: Result<T, HostError>) -> (String, Option<String>, Option<Kind>) {
    said(&r.expect_err("the call is refused"))
}

fn ids_of(view: &Value) -> Vec<String> {
    view["threads"].as_array().unwrap().iter().map(|t| t["threadId"].as_str().unwrap().to_string()).collect()
}

// ===============================================================================================================
// One read of this computer
// ===============================================================================================================

// AC10-AC14, AC25: activity.test.ts:48
#[test]
fn lists_a_threads_tree_then_wsps_own_and_leaves_out_what_nobody_started() {
    run_local(async {
        let lab = Lab::main();
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![daemon_row()], vec![]));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
        assert_eq!(lab.host.hooks.called("local_channel"), vec!["local_channel(false)"]);
        let one = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(
            one,
            view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]),
            "a thread asked alone has no wsp or unowned rows"
        );
    });
}

// AC14: activity.test.ts:81 (the host's read whole, with its host pid and this computer's name)
#[test]
fn a_read_names_this_computer_and_stamps_the_time_of_the_snapshot() {
    run_local(async {
        let lab = Lab::main();
        lab.host.clock.advance(7_000);
        lab.here.set_procs(vec![daemon_proc()]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0 + 7_000, vec![], vec![daemon_row()], vec![]));
    });
}

// AC10-AC13: activity.test.ts:93
#[test]
fn lists_the_newest_thread_first() {
    run_local(async {
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "one", STARTED, "completed");
        lab.local_thread(T2, "w2", "two", STARTED + 1_000, "completed");
        lab.here
            .set_procs(vec![proc_row(4100, json!({ "owner": owned("fork", T1) })), proc_row(4101, json!({ "owner": owned("fork", T2) }))]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        let one = |id: &str, title: &str, ws: &str, pid: u32| {
            thread_json(id, title, ws, 1_000_000, 0, vec![shown(pid, 1, 0, STARTED, "other", "node", "node", 1_000_000, 0, &[])])
        };
        assert_eq!(view["threads"], json!([one(T2, "two", "w2", 4101), one(T1, "one", "w1", 4100)]));
    });
}

// AC13: the rule alone (no TS case)
#[test]
fn running_threads_come_first_and_equal_starts_keep_their_order() {
    run_local(async {
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "a", STARTED + 2_000, "completed");
        lab.local_thread(T2, "w2", "b", STARTED, "running");
        lab.local_thread(T3, "w3", "c", STARTED + 2_000, "completed");
        lab.here.set_procs(vec![
            proc_row(4100, json!({ "owner": owned("fork", T1) })),
            proc_row(4101, json!({ "owner": owned("fork", T2) })),
            proc_row(4102, json!({ "owner": owned("fork", T3) })),
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(ids_of(&view), vec![T2, T1, T3]);
    });
}

// AC11: the scope's own workspace beats the latest row's, a warm scope names no thread
#[test]
fn a_scope_names_its_workspace_and_a_warm_one_is_left_out() {
    run_local(async {
        let lab = Lab::main();
        lab.scopes(vec![
            json!({ "thread": T1, "workspace": "w-from-scope" }),
            json!({ "thread": "warm-agent-0000", "warm": true, "agents": [4100] }),
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(
            view["threads"],
            json!([thread_json(T1, "start the dev server", "w-from-scope", 470_000_000, 13, vec![vite(), esbuild(), agent(), tool()])])
        );
        assert!(
            !lab.host.hooks.called("thread_facts").contains(&"thread_facts(warm-agent-0000)".to_string()),
            "a warm agent no send claimed has no title read"
        );
    });
}

// AC11: the title is the thread's word where the facts hold none
#[test]
fn a_thread_with_no_facts_is_titled_by_its_word() {
    run_local(async {
        let lab = Lab::main();
        lab.facts.borrow_mut().clear();
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view["threads"][0]["title"], json!(thread_word(T1)));
    });
}

// AC10: a running turn's run file names its agent
#[test]
fn a_running_turns_agent_pid_names_its_agent_to_the_read() {
    run_local(async {
        let dir = tempfile::tempdir().expect("a folder");
        let run = dir.path().join("run-1");
        let mine = std::process::id();
        std::fs::write(format!("{}.agent", run.display()), format!("{mine}\n")).expect("the agent file");
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "go", STARTED, "running");
        lab.scopes(vec![json!({ "thread": T1, "agents": [mine] })]);
        lab.host.rows.borrow()[&format!("s-{T1}")].borrow_mut().run = Some(run.display().to_string());
        lab.here.set_procs(vec![proc_row(mine, json!({ "comm": "agent-x", "cmdline": "agent-x" }))]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(
            view["threads"],
            json!([thread_json(
                T1,
                "go",
                "w1",
                1_000_000,
                0,
                vec![shown(mine, 1, 0, STARTED, "agent", "agent-x", "agent-x", 1_000_000, 0, &[])]
            )])
        );
    });
}

// AC5
#[test]
fn ports_are_grouped_by_pid_and_sorted() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports.borrow_mut() = json!([
            { "port": 5174, "pid": 4200, "uid": 0, "loopback": false },
            { "port": 5173, "pid": 4200, "uid": 0, "loopback": false },
            { "port": 9000, "pid": "4300", "uid": 0, "loopback": false },
            { "pid": 4300, "uid": 0, "loopback": false },
            { "port": 5175, "pid": 4100, "uid": 0, "loopback": false },
        ]);
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        let rows = &view["threads"][0]["processes"];
        let ports: Vec<Value> = rows.as_array().unwrap().iter().map(|p| json!([p["pid"], p["ports"]])).collect();
        assert_eq!(ports, vec![json!([4200, [5173, 5174]]), json!([4300, []]), json!([4100, [5175]]), json!([4150, []])]);
    });
}

// AC5
#[test]
fn a_ports_answer_that_is_not_a_list_is_no_ports() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports.borrow_mut() = json!("nothing");
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        let all: Vec<Value> = view["threads"][0]["processes"].as_array().unwrap().iter().map(|p| p["ports"].clone()).collect();
        assert_eq!(all, vec![json!([]), json!([]), json!([]), json!([])]);
    });
}

// AC5: ports are asked first, before any process frame
#[test]
fn the_listeners_are_asked_before_the_processes() {
    run_local(async {
        let lab = Lab::main();
        lab.read(ActivityAsk::default(), None).await.expect("a read");
        assert_eq!(lab.here.seq.borrow().clone(), vec!["c0:ports.watch", "c0:proc.watch", "c0:close"]);
    });
}

// AC6, AC8: activity.test.ts:363
#[test]
fn a_daemon_with_no_list_is_watched_instead() {
    run_local(async {
        let lab = Lab::main();
        lab.here.lists.set(false);
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        let left = left_thread(T1, "start the dev server", "w1", 470_000_000, 13, vec![vite(), esbuild()], None);
        assert_eq!(refusal, processes_left_refusal(LeftVerb::Settle, &[left], Some(&IndexSet::new())));
        assert_eq!(
            lab.here.chans(),
            vec![chan(false, vec![json!({ "op": "ports.watch" }), json!({ "op": "proc.list" }), json!({ "op": "proc.watch" })])]
        );
    });
}

// AC6
#[test]
fn any_other_failure_of_the_list_is_the_reads() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.list_error.borrow_mut() = Some(refused(None, "the sampler is down"));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await);
        assert_eq!(said, ("the sampler is down".to_string(), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())], "no watch after a list that failed otherwise");
    });
}

// AC7: a one-shot read of a box that has no list is refused, never watched
#[test]
fn a_read_that_may_not_watch_fails_with_the_unknown_op_line() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.boxd.lists.set(false);
        let said = said_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(said, usage_said(&box_behind_activity_line("hetzner"), BOX_BEHIND_ACTIVITY_FIX));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
    });
}

// AC8
#[test]
fn no_snapshot_in_ten_seconds_fails_the_read() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.watch_events.borrow_mut() = Some(vec![]);
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.read(ActivityAsk::default(), None).await });
        settle().await;
        lab.host.clock.advance(9_999);
        settle().await;
        assert!(!task.is_finished(), "the read waits out ten seconds");
        lab.host.clock.advance(1);
        settle().await;
        assert!(task.is_finished(), "the read gives up on the clock, not on a real timer");
        let said = said_of(task.await.expect("the task ran"));
        assert_eq!(said, (format!("{THIS_COMPUTER}'s daemon sent no processes in 10 s"), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
        assert_eq!(lab.host.clock.pending(), 0, "the wait's timer is cleared");
    });
}

// AC8: the first snapshot ends the wait and clears the timer
#[test]
fn a_snapshot_inside_the_wait_is_the_read_and_clears_the_timer() {
    run_local(async {
        let lab = Lab::main();
        lab.read(ActivityAsk::default(), None).await.expect("a read");
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC8
#[test]
fn events_of_other_types_and_snapshots_that_do_not_parse_are_ignored() {
    run_local(async {
        let lab = Lab::main();
        let mut good = lab.here.snapshot();
        good["type"] = json!("proc.snapshot");
        *lab.here.watch_events.borrow_mut() = Some(vec![
            json!({ "type": "proc.changes", "at": 1, "daemon": 1, "total": 0, "procs": [] }),
            json!({ "type": "proc.snapshot", "procs": "not rows" }),
            good,
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![daemon_row()], vec![]));
    });
}

// AC9
#[test]
fn the_channel_is_closed_after_a_failed_read() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports_error.borrow_mut() = Some(refused(None, "boom"));
        let said = said_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(said, ("boom".to_string(), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, vec![json!({ "op": "ports.watch" })])]);
    });
}

// AC4
#[test]
fn a_host_with_no_daemon_here_reads_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn() -> bool>("ActivityCtx::has_local_daemon", Rc::new(|| false));
        let none = (wsp_threads::activity::NO_DAEMON_HERE.to_string(), None, None);
        assert_eq!(said_of(lab.read(ActivityAsk::default(), None).await), none);
        assert_eq!(said_of(lab.end(4200, STARTED + 1_000, None).await), none);
        assert_eq!(said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await), none);
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// AC22: the read waits for the host
#[test]
fn a_read_and_an_end_wait_for_the_host_before_anything_else() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "ActivityCtx::ready",
            Rc::new(|| async { Err(HostError::new("the host is not ready")) }.boxed_local()),
        );
        let not_ready = ("the host is not ready".to_string(), None, None);
        assert_eq!(said_of(lab.read(ActivityAsk::default(), None).await), not_ready);
        assert_eq!(said_of(lab.end(4200, STARTED + 1_000, None).await), not_ready);
        assert_eq!(lab.host.hooks.calls(), vec!["ready", "ready"]);
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// ===============================================================================================================
// Which computer
// ===============================================================================================================

// AC1
#[test]
fn this_computers_name_in_any_case_is_here() {
    run_local(async {
        let lab = Lab::main();
        for computer in [None, Some("here"), Some("this computer"), Some("THIS COMPUTER")] {
            let view = view_of(lab.read(ask_of(None, computer), None).await);
            assert_eq!(view["computer"], json!(THIS_COMPUTER), "{computer:?}");
        }
        let named = Lab::main();
        named.host.hooks.set::<dyn Fn() -> bool>("ActivityCtx::has_door", Rc::new(|| true));
        named.host.hooks.set::<dyn Fn(&str) -> Option<String>>(
            "ActivityCtx::place_name",
            Rc::new(|p: &str| (p == "here").then(|| "Mac mini".to_string())),
        );
        for computer in [None, Some("here"), Some("MAC MINI"), Some("mac mini"), Some("this computer")] {
            let view = view_of(named.read(ask_of(None, computer), None).await);
            assert_eq!(view["computer"], json!("Mac mini"), "{computer:?}");
        }
        assert_eq!(named.here.chans().len(), 5);
    });
}

// AC2: activity.test.ts:395
#[test]
fn a_computer_this_host_does_not_hold_is_refused() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.read(ask_of(None, Some("nowhere")), None).await);
        assert_eq!(
            said,
            usage_said(&no_such_place_refusal("nowhere", &[]), "Name one wsp computers lists."),
            "a host with no door holds no rows"
        );
        lab.join_box();
        let said = said_of(lab.read(ask_of(None, Some("nowhere")), None).await);
        assert_eq!(said, usage_said(&no_such_place_refusal("nowhere", &["hetzner".to_string()]), "Name one wsp computers lists."));
        assert_eq!(lab.here.chans(), vec![]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC2
#[test]
fn a_computer_matches_by_id_or_by_name_in_any_case() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        for computer in ["box", "hetzner", "HETZNER"] {
            let view = view_of(lab.read(ask_of(None, Some(computer)), None).await);
            assert_eq!(view["computer"], json!("hetzner"), "{computer}");
        }
        assert_eq!(lab.boxd.chans().len(), 3);
    });
}

// AC2
#[test]
fn a_box_this_host_has_not_joined_is_not_read() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        let said = said_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(said, usage_said(&not_read_line("hetzner"), NOT_READ_FIX));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC3: box-activity.test.ts:64
#[test]
fn an_end_on_a_box_signals_nothing() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.boxd.set_procs(vec![box_row(7100, 1, "node", "node vite", &thread_cgroup(T1))]);
        let mut ask = end_of(7100, STARTED + 7_100);
        ask.computer = Some("hetzner".into());
        let said = said_of(lab.door.end(ask, None).await);
        assert_eq!(said, usage_said(&box_end_line("hetzner"), BOX_END_FIX));
        assert_eq!(lab.boxd.chans(), vec![]);
        assert_eq!(lab.boxd.kills.borrow().clone(), Vec::<Value>::new());
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
        assert_eq!(lab.boxd.procs.borrow().len(), 1);
    });
}

// ===============================================================================================================
// Who reaches what
// ===============================================================================================================

// AC15, AC25: activity.test.ts:374
#[test]
fn a_relayed_caller_reads_nothing_outside_a_thread_and_ends_none() {
    run_local(async {
        let lab = Lab::main();
        lab.here
            .procs
            .borrow_mut()
            .push(proc_row(4700, json!({ "cmdline": "node watch.js", "owner": owned("env", "t-gone"), "startedAt": STARTED + 7_000 })));
        let unowned = shown(4700, 1, 0, STARTED + 7_000, "other", "node", "node watch.js", 1_000_000, 0, &[]);
        let person = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(person["unowned"], json!([unowned]));
        lab.drives(|_k: WorkspaceKind, c: Option<&Caller>| !is_relayed(c));
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Option<HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, c: Option<&Caller>| {
                is_relayed(c).then(|| HostError::with_kind("a relayed road drives no local workspace", Kind::Usage))
            }),
        );
        let seen = view_of(lab.read(ActivityAsk::default(), Some(relayed())).await);
        assert_eq!(seen, view_json(THIS_COMPUTER, T0, vec![], vec![], vec![]));
        let said = said_of(lab.end(4700, STARTED + 7_000, Some(relayed())).await);
        assert_eq!(
            said,
            (joined(&not_threads_line(4700), &NOT_THREADS_FIX.to_string()), Some(NOT_THREADS_FIX.to_string()), Some(Kind::Usage))
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans().len(), 3, "two reads and the end's read; no kill channel");
    });
}

// AC15, AC25: a thread reads its own threads and no command line outside them
#[test]
fn a_thread_reads_its_own_threads_and_not_what_runs_outside_them() {
    run_local(async {
        let lab = Lab::main();
        let view = view_of(lab.read(ActivityAsk::default(), Some(scoped_by(T1, "w1"))).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]));
    });
}

// AC15: drives is asked with the kind of the computer read
#[test]
fn what_runs_outside_a_thread_is_asked_of_what_the_caller_drives_on_that_kind() {
    run_local(async {
        let lab = Lab::main();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        lab.join_box();
        lab.boxd.set_procs(vec![daemon_proc()]);
        let kinds: Rc<RefCell<Vec<WorkspaceKind>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = kinds.clone();
        lab.drives(move |k: WorkspaceKind, _c: Option<&Caller>| {
            seen.borrow_mut().push(k);
            k != WorkspaceKind::Place
        });
        let here = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(here["wsp"], json!([daemon_row()]));
        let there = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(there, view_json("hetzner", T0, vec![], vec![], vec![]), "a box the caller does not drive shows no wsp row");
        assert!(kinds.borrow().contains(&WorkspaceKind::Local) && kinds.borrow().contains(&WorkspaceKind::Place));
    });
}

// AC16, AC22: a thread the caller may not settle is not found
#[test]
fn a_thread_asked_that_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|_t: &str, _c: Option<&Caller>| false));
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(lab.here.chans(), vec![], "nothing is read for a thread that is not reached");
    });
}

// AC16: the workspace rule's refusal makes a thread not reached
#[test]
fn a_workspace_the_rule_refuses_the_caller_makes_its_thread_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Option<HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, _c: Option<&Caller>| Some(HostError::new("not yours"))),
        );
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
    });
}

// AC22: no record and no facts
#[test]
fn a_thread_nobody_holds_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        let mut ask = ActivityAsk::default();
        ask.thread_ids = Some(vec![T1.to_string(), T3.to_string()]);
        let said = said_of(lab.read(ask, None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T3))));
    });
}

// AC16: no workspace record at all reads what the caller drives, by the computer's kind
#[test]
fn a_thread_with_no_live_workspace_is_reached_by_what_the_caller_drives() {
    run_local(async {
        let lab = Lab::main();
        lab.host.entries.borrow_mut().clear();
        let kinds: Rc<RefCell<Vec<WorkspaceKind>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = kinds.clone();
        lab.drives(move |k: WorkspaceKind, _c: Option<&Caller>| {
            seen.borrow_mut().push(k);
            false
        });
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(kinds.borrow().first(), Some(&WorkspaceKind::Local));
    });
}

// AC25: of the threads asked only those on this computer are kept, and wsp and unowned are emptied
#[test]
fn threads_asked_that_run_elsewhere_are_left_out_of_a_read_here() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let mut ask = ActivityAsk::default();
        ask.thread_ids = Some(vec![T1.to_string(), T2.to_string()]);
        let view = view_of(lab.read(ask, None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC25: of every thread only the reached ones are listed
#[test]
fn threads_the_caller_does_not_reach_are_left_out_of_a_read_of_everything() {
    run_local(async {
        let lab = Lab::main();
        lab.local_thread(T2, "w2", "other", STARTED + 10, "completed");
        let mut rows = main_table(T1);
        rows.push(proc_row(4600, json!({ "owner": owned("fork", T2), "startedAt": STARTED + 3_000 })));
        lab.here.set_procs(rows);
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|t: &str, _c: Option<&Caller>| t == T1));
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(ids_of(&view), vec![T1]);
        assert_eq!(view["wsp"], json!([daemon_row()]));
    });
}

// AC23: box-activity.test.ts:85
#[test]
fn names_the_unjoined_box_a_thread_runs_on() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, usage_said(&not_joined_activity_line(T1, "hetzner"), &not_joined_activity_fix("hetzner")));
        assert!(!said.0.contains("cloud"));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC23: box-activity.test.ts:96
#[test]
fn says_cloud_machine_for_a_provider_this_host_does_not_fork_on() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        lab.thread_in(T1, "w1", json!({ "kind": "cloud", "place": "box" }), "Fix it", STARTED, "completed");
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, usage_said(&cloud_activity_line(T1), CLOUD_ACTIVITY_FIX));
    });
}

// AC24
#[test]
fn a_computer_named_that_is_not_the_threads_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let said = said_of(lab.read(ask_of(Some(T1), Some("hetzner")), None).await);
        assert_eq!(said, nf(&format!("no thread {} on hetzner", thread_word(T1))));
        let said = said_of(lab.read(ask_of(Some(T2), Some("here")), None).await);
        assert_eq!(said, nf(&format!("no thread {} on {THIS_COMPUTER}", thread_word(T2))));
        assert_eq!(lab.here.chans(), vec![]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// ===============================================================================================================
// A box
// ===============================================================================================================

fn box_row(pid: u32, ppid: u32, comm: &str, cmdline: &str, cgroup: &str) -> Value {
    json!({ "pid": pid, "ppid": ppid, "user": "root", "state": "S", "comm": comm, "cmdline": cmdline, "cpu": 1, "rss": 50_000_000, "startedAt": STARTED + pid as i64, "cgroup": cgroup })
}

fn box_shown(pid: u32, ppid: u32, depth: u32, name: &str, command: &str, ports: &[u32]) -> Value {
    shown(pid, ppid, depth, STARTED + pid as i64, "dev", name, command, 50_000_000, 1, ports)
}

/// hetzner holding a finished thread whose agent left vite and its esbuild child in the thread's cgroup, and a process of
/// a cgroup no thread here names beside them (box-activity.test.ts:17).
fn box_lab() -> Lab {
    let lab = Lab::new();
    lab.join_box();
    lab.box_thread(T1, "w1", "start the dev server", STARTED);
    let mine = thread_cgroup(T1);
    lab.boxd.set_procs(vec![
        box_row(7100, 1, "node", "node /root/lab/node_modules/.bin/vite", &mine),
        box_row(7101, 7100, "esbuild", "/root/lab/node_modules/esbuild/bin/esbuild --service", &mine),
        box_row(7200, 1, "node", "node /root/else/node_modules/.bin/vite --port 5174", &thread_cgroup("other-thread")),
    ]);
    lab.boxd.set_ports(&[(5173, 7100)]);
    lab
}

fn box_thread_json() -> Value {
    thread_json(
        T1,
        "start the dev server",
        "w1",
        100_000_000,
        2,
        vec![
            box_shown(7100, 1, 0, "node", "node /root/lab/node_modules/.bin/vite", &[5173]),
            box_shown(7101, 7100, 1, "esbuild", "/root/lab/node_modules/esbuild/bin/esbuild --service", &[]),
        ],
    )
}

// AC18, AC9: box-activity.test.ts:35
#[test]
fn lists_a_boxs_threads_by_their_cgroups() {
    run_local(async {
        let lab = box_lab();
        lab.host.clock.advance(9_000);
        let view = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(view, view_json("hetzner", T0 + 9_000, vec![box_thread_json()], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())], "once, over the link, never a watch");
        assert_eq!(lab.host.hooks.called("box_channel"), vec!["box_channel(box)"]);
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
    });
}

// AC18: box-activity.test.ts:47
#[test]
fn totals_a_box_thread_by_its_cgroup() {
    run_local(async {
        let lab = box_lab();
        *lab.boxd.cgroups.borrow_mut() = Some(json!([{ "cgroup": thread_cgroup(T1), "memory": 30_000_000 }]));
        let mut thread = box_thread_json();
        thread["memory"] = json!(30_000_000);
        let by_computer = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(by_computer["threads"], json!([thread.clone()]));
        let by_thread = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(by_thread["threads"], json!([thread]));
    });
}

// AC18, AC24, AC25: box-activity.test.ts:56
#[test]
fn reads_a_thread_alone_on_its_box() {
    run_local(async {
        let lab = box_lab();
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(view, view_json("hetzner", T0, vec![box_thread_json()], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
    });
}

// AC7, AC18: box-activity.test.ts:71
#[test]
fn a_box_with_no_list_is_refused_in_the_usage_class() {
    run_local(async {
        let lab = box_lab();
        lab.boxd.lists.set(false);
        let expected = usage_said(&box_behind_activity_line("hetzner"), BOX_BEHIND_ACTIVITY_FIX);
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), expected);
        assert_eq!(said_of(lab.read(ask_of(Some(T1), None), None).await), expected);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
    });
}

// AC18: a box with no link or no channel is away
#[test]
fn a_box_with_no_link_or_no_channel_is_away() {
    run_local(async {
        let lab = box_lab();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| false));
        let away = ("hetzner is away".to_string(), None, None);
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), away);
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| true));
        lab.host.hooks.set::<dyn Fn(&str, DaemonEventListener) -> Option<LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>>(
            "ActivityCtx::box_channel",
            Rc::new(|_p: &str, _s: DaemonEventListener| None),
        );
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), away);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC20, AC18
#[test]
fn a_boxs_threads_unnamed_come_newest_first() {
    run_local(async {
        let lab = box_lab();
        lab.box_thread(T2, "w2", "second", STARTED + 5_000);
        lab.local_thread(T3, "w3", "here", STARTED + 9_000, "completed");
        let mut rows = lab.boxd.procs.borrow().clone();
        rows.push(box_row(7300, 1, "node", "node /root/two/node_modules/.bin/vite", &thread_cgroup(T2)));
        lab.boxd.set_procs(rows);
        let view = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(ids_of(&view), vec![T2, T1]);
        assert_eq!(
            view["threads"][0],
            thread_json(
                T2,
                "second",
                "w2",
                50_000_000,
                1,
                vec![box_shown(7300, 1, 0, "node", "node /root/two/node_modules/.bin/vite", &[])]
            )
        );
    });
}

// ===============================================================================================================
// End
// ===============================================================================================================

fn usage_said(line: &str, fix: &str) -> (String, Option<String>, Option<Kind>) {
    said(&usage_refusal(line, fix))
}

fn nf(message: &str) -> (String, Option<String>, Option<Kind>) {
    said(&not_found_refusal(message))
}

fn agent_scope(thread: &str, title: &str) -> ActivityScope {
    ActivityScope {
        thread_id: thread.to_string(),
        title: title.to_string(),
        workspace_id: String::new(),
        roots: vec![],
        ..Default::default()
    }
}

// AC26
#[test]
fn end_frees_a_kept_agent_and_signals_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.scopes(vec![json!({ "thread": T1, "agents": [4100] })]);
        lab.kept(T1, 4100);
        let answer = ended_of(lab.end(4100, STARTED, None).await);
        assert_eq!(answer, json!({ "pid": 4100, "ended": [agent()], "forced": [], "left": [], "freed": T1 }));
        assert_eq!(lab.host.hooks.called("reap_kept_now"), vec![format!("reap_kept_now({T1})")]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())], "a fresh read and no kill channel");
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC11, AC26: the agent a running turn runs is not the kept one
#[test]
fn a_running_threads_agent_is_not_kept_and_its_end_is_refused() {
    run_local(async {
        let lab = Lab::main();
        lab.host.rows.borrow()[&format!("s-{T1}")].borrow_mut().view.status = serde_json::from_value(json!("running")).unwrap();
        lab.scopes(vec![json!({ "thread": T1, "agents": [4100] })]);
        lab.kept(T1, 4100);
        let said = said_of(lab.end(4100, STARTED, None).await);
        let scope = agent_scope(T1, "start the dev server");
        assert_eq!(said, (joined(&agent_end_line(4100.0, &scope), &agent_end_fix(&scope)), Some(agent_end_fix(&scope)), Some(Kind::Usage)));
        assert_eq!(lab.host.hooks.called("reap_kept_now"), Vec::<String>::new());
        assert_eq!(lab.here.chans().len(), 1);
    });
}

// AC26: activity.test.ts:106
#[test]
fn end_refuses_the_agent_and_sends_nothing() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.end(4100, STARTED, None).await);
        let scope = agent_scope(T1, "start the dev server");
        assert_eq!(said, (joined(&agent_end_line(4100.0, &scope), &agent_end_fix(&scope)), Some(agent_end_fix(&scope)), Some(Kind::Usage)));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
    });
}

// AC26: activity.test.ts:115
#[test]
fn end_refuses_a_pid_that_names_another_process_now() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.end(4200, STARTED - 60_000, None).await);
        let now: wsp_frames::ProcEntry =
            serde_json::from_value(proc_row(4200, json!({ "comm": "node", "startedAt": STARTED + 1_000 }))).expect("a process row");
        assert_eq!(
            said,
            (
                joined(&end_reused_line(4200, &now, STARTED - 60_000), &END_AGAIN_FIX.to_string()),
                Some(END_AGAIN_FIX.to_string()),
                Some(Kind::NotFound)
            )
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
    });
}

// AC26: activity.test.ts:123
#[test]
fn end_refuses_a_process_wsp_did_not_start() {
    run_local(async {
        let lab = Lab::main();
        for started in [STARTED, 1] {
            let said = said_of(lab.end(5000, started, None).await);
            assert_eq!(
                said,
                (joined(&not_started_line(5000), &NOT_STARTED_FIX.to_string()), Some(NOT_STARTED_FIX.to_string()), Some(Kind::Usage))
            );
        }
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames()), chan(false, watch_frames())]);
    });
}

// AC27: activity.test.ts:133
#[test]
fn end_takes_the_process_and_its_tree_in_one_kill() {
    run_local(async {
        let lab = Lab::main();
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[], &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames()), chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID))])]);
        let frame = lab.here.kills.borrow()[0].to_string();
        assert_eq!(
            frame,
            format!(
                r#"{{"op":"proc.kill","pid":4200,"signal":"TERM","mode":"tree","startedAt":{},"identity":"{VITE_ID}"}}"#,
                STARTED + 1_000
            ),
            "in this key order"
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![(4200, "TERM".to_string()), (4300, "TERM".to_string())]);
    });
}

// AC27: a process the daemon gave no identity is sent with none
#[test]
fn end_sends_no_identity_for_a_process_the_daemon_gave_none() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(proc_row(
            4600,
            json!({ "cmdline": "node serve.js", "kind": "dev", "owner": owned("fork", T1), "startedAt": STARTED + 4_000 }),
        ));
        let answer = ended_of(lab.end(4600, STARTED + 4_000, None).await);
        assert_eq!(
            answer,
            ended(4600, vec![shown(4600, 1, 0, STARTED + 4_000, "dev", "node", "node serve.js", 1_000_000, 0, &[])], &[], &[])
        );
        let frame = lab.here.kills.borrow()[0].to_string();
        assert_eq!(frame, format!(r#"{{"op":"proc.kill","pid":4600,"signal":"TERM","mode":"tree","startedAt":{}}}"#, STARTED + 4_000));
    });
}

// AC27: activity.test.ts:148
#[test]
fn end_signals_through_the_guard() {
    run_local(async {
        let lab = Lab::main();
        lab.end(4200, STARTED + 1_000, None).await.expect("the end answers");
        assert_eq!(
            lab.host.hooks.called("local_channel"),
            vec!["local_channel(false)", "local_channel(true)"],
            "the read is plain, the kill goes through the guard"
        );
        assert_eq!(lab.here.chans().iter().map(|c| c.guarded).collect::<Vec<_>>(), vec![false, true]);
    });
}

// AC27: activity.test.ts:159
#[test]
fn end_names_what_the_daemon_forced() {
    run_local(async {
        let lab = Lab::main();
        lab.here.stubborn.borrow_mut().push(4300);
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[4300], &[]));
        assert_eq!(lab.here.kills.borrow().len(), 1, "the grace and the KILL after it are the daemon's");
        assert_eq!(
            lab.here.signals.borrow().clone(),
            vec![(4200, "TERM".to_string()), (4300, "TERM".to_string()), (4300, "KILL".to_string())]
        );
    });
}

// AC27: activity.test.ts:173 and :186
#[test]
fn end_signals_nothing_when_the_pid_holds_another_process() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.after_read.borrow_mut() = Some(Box::new(|d: &Daemon| {
            for p in d.procs.borrow_mut().iter_mut() {
                if p["pid"] == 4200 {
                    p["comm"] = json!("sleep");
                    p["cmdline"] = json!("sleep 600");
                    p["id"] = json!("99999");
                }
            }
        }));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![], &[], &[]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert!(lab.here.procs.borrow().iter().any(|p| p["pid"] == 4200));
        assert_eq!(lab.here.kills.borrow().len(), 1, "the daemon was asked and refused it");
    });
}

// AC27: any other refusal is the End's
#[test]
fn a_refusal_of_the_kill_other_than_not_found_is_the_ends() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() = Some(Err(refused(Some("denied"), "operation not permitted")));
        let said = said_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(said, ("operation not permitted".to_string(), None, None));
        assert_eq!(
            lab.here.chans(),
            vec![chan(false, watch_frames()), chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID))])],
            "the kill channel is closed after the failure"
        );
    });
}

// AC27: forced and left are named once, and ended is the plan's rows the daemon signalled
#[test]
fn forced_and_left_are_named_once_and_ended_is_what_was_signalled() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() =
            Some(Ok(reply(json!({ "signalled": [4300, 4200, 4300], "forced": [4300, 4300], "left": [4300, 4300] }))));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[4300], &[4300]), "ended follows the plan's order, each row once");
    });
}

// AC27: a daemon that signalled only the head
#[test]
fn ended_holds_only_the_rows_the_daemon_signalled() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() = Some(Ok(reply(json!({ "signalled": [4200], "forced": [], "left": [] }))));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite()], &[], &[]));
    });
}

// AC27: activity.test.ts:215
#[test]
fn end_on_a_daemon_before_kill_modes_names_the_pid_alone() {
    run_local(async {
        let lab = Lab::main();
        lab.here.modes.set(false);
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite()], &[], &[]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![(4200, "TERM".to_string())]);
    });
}

// ===============================================================================================================
// Meet: before a settle or a delete takes threads
// ===============================================================================================================

fn t1_left(processes: Vec<Value>, memory: u64) -> LeftThread {
    left_thread(T1, "start the dev server", "w1", memory, 13, processes, None)
}

fn refusal_of(verb: LeftVerb, left: &[LeftThread], theirs: &[u32]) -> HostError {
    let set: IndexSet<u32> = theirs.iter().copied().collect();
    processes_left_refusal(verb, left, Some(&set))
}

fn signalled(lab: &Lab) -> Vec<u32> {
    lab.here.signals.borrow().iter().map(|(p, _)| *p).collect()
}

fn table_pids(d: &Daemon) -> Vec<u64> {
    d.procs.borrow().iter().map(|p| p["pid"].as_u64().unwrap()).collect()
}

// AC28-AC32: activity.test.ts:225
#[test]
fn settle_stop_ends_every_tree_in_one_go() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(vitest_proc(T1));
        lab.here.hold_kills.set(2);
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(4400, vec![vitest()], &[], &[])]));
        assert_eq!(
            lab.here.chans(),
            vec![
                chan(false, list_frames()),
                chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID)), kill_frame(4400, STARTED + 3_000, Some("88002"))])
            ]
        );
        assert_eq!(lab.here.first_answer_saw.get(), Some(2), "both kills were sent before the first was answered");
        assert_eq!(signalled(&lab), vec![4200, 4300, 4400]);
        assert_eq!(table_pids(&lab.here), vec![1, 4100, 4150, 5000]);
    });
}

// AC29, AC31: activity.test.ts:240
#[test]
fn settle_ask_refuses_naming_what_outlives_the_agent() {
    run_local(async {
        let lab = Lab::main();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())], "one list, answered at once, never a watch waiting on the sampler");
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC28, AC31: activity.test.ts:264
#[test]
fn delete_ask_refuses_and_keep_reads_nothing() {
    run_local(async {
        let lab = Lab::main();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Delete, None).await.expect_err("the delete is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Delete, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]));
        let calls = lab.host.hooks.calls();
        let chans = lab.here.chans();
        let kept = all_ended(lab.meet(&[T1], ProcessesAsk::Keep, MeetVerb::Delete, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.host.hooks.calls(), calls, "keep asks the host nothing");
        assert_eq!(lab.here.chans(), chans);
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert!(table_pids(&lab.here).contains(&4200));
    });
}

// AC28
#[test]
fn keep_asks_nothing_of_the_host_and_opens_no_channel() {
    run_local(async {
        let lab = Lab::main();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let kept = all_ended(lab.meet(&[T1, T2], ProcessesAsk::Keep, MeetVerb::Settle, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.host.hooks.calls(), Vec::<String>::new());
        assert_eq!(lab.here.chans(), vec![]);
    });
}

fn with_shell() -> Lab {
    let lab = Lab::main();
    lab.here.procs.borrow_mut().extend(shell_procs(T1));
    lab
}

fn shell_left() -> LeftThread {
    t1_left(vec![vite(), esbuild(), shell(), shell_child()], 472_000_000)
}

// AC29, AC30, AC32: activity.test.ts:287
#[test]
fn a_shell_the_threads_pane_opened_is_asked_about_and_stopped() {
    run_local(async {
        let lab = with_shell();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[shell_left()], &[]));
        let me = Some(scoped_by(T1, "w1"));
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, me).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(5100, vec![shell(), shell_child()], &[], &[])]));
        assert_eq!(lab.here.chans().len(), 3);
        assert_eq!(
            lab.here.chans()[2].frames,
            vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID)), kill_frame(5100, STARTED + 5_000, Some("88100"))]
        );
        assert_eq!(table_pids(&lab.here), vec![1, 4100, 4150, 5000]);
        assert_eq!(lab.here.direct.borrow().clone(), Vec::<Value>::new(), "a shell here is ended by its tree, no pty.kill");
    });
}

// AC28, AC31: activity.test.ts:301
#[test]
fn delete_asks_about_a_shell_the_pane_opened_and_keep_leaves_it_running() {
    run_local(async {
        let lab = with_shell();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Delete, None).await.expect_err("the delete is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Delete, &[shell_left()], &[]));
        let kept = all_ended(lab.meet(&[T1], ProcessesAsk::Keep, MeetVerb::Delete, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.direct.borrow().clone(), Vec::<Value>::new());
        assert!(table_pids(&lab.here).contains(&5100) && table_pids(&lab.here).contains(&5110));
    });
}

// AC32: activity.test.ts:315
#[test]
fn delete_with_stop_ends_a_shell_the_pane_opened_with_the_rest() {
    run_local(async {
        let lab = with_shell();
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Delete, None).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(5100, vec![shell(), shell_child()], &[], &[])]));
        assert!(!table_pids(&lab.here).contains(&5100));
    });
}

// AC10, AC29: activity.test.ts:324
#[test]
fn a_thread_beside_it_keeps_its_own_processes() {
    run_local(async {
        let lab = Lab::main();
        lab.local_thread(T2, "w1", "start the docs server", STARTED + 10, "completed");
        let theirs = owned("fork", T2);
        lab.here.procs.borrow_mut().extend([
            proc_row(4600, json!({ "cmdline": "node /lab/node_modules/.bin/vite --port 5174", "rss": 90_000_000, "kind": "dev", "owner": theirs, "startedAt": STARTED + 3_000 })),
            proc_row(4650, json!({ "ppid": 4600, "comm": "esbuild", "cmdline": "/lab/node_modules/esbuild/bin/esbuild --service", "rss": 20_000_000, "owner": theirs, "startedAt": STARTED + 4_000 })),
        ]);
        lab.here.set_ports(&[(5173, 4200), (5174, 4600)]);
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(
            refusal,
            refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]),
            "the refusal carries this thread's tree alone"
        );
        let stopped = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(stopped, json!([ended(4200, vec![vite(), esbuild()], &[], &[])]));
        assert_eq!(signalled(&lab), vec![4200, 4300]);
        assert!(table_pids(&lab.here).contains(&4600) && table_pids(&lab.here).contains(&4650));
        let kept = all_ended(lab.meet(&[T2], ProcessesAsk::Keep, MeetVerb::Settle, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(signalled(&lab), vec![4200, 4300]);
    });
}

// AC30, AC31: activity.test.ts:197
#[test]
fn a_thread_settling_itself_hears_which_processes_only_the_person_ends() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(proc_row(
            4800,
            json!({ "comm": "python3", "cmdline": "python3 -m http.server", "owner": owned("env", T1), "startedAt": STARTED + 8_000 }),
        ));
        let me = || Some(scoped_by(T1, "w1"));
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, me()).await.expect_err("the ask is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild(), python()], 471_000_000)], &[4800]));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, me()).await);
        assert_eq!(
            said,
            (joined(&not_threads_line(4800), &NOT_THREADS_FIX.to_string()), Some(NOT_THREADS_FIX.to_string()), Some(Kind::Usage))
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames()), chan(false, list_frames())], "no kill channel before the refusal");
    });
}

// AC30
#[test]
fn nothing_left_answers_nothing_and_signals_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().retain(|p| p["pid"] != 4200 && p["pid"] != 4300);
        for processes in [ProcessesAsk::Ask, ProcessesAsk::Stop] {
            let answer = all_ended(lab.meet(&[T1], processes, MeetVerb::Settle, None).await);
            assert_eq!(answer, json!([]));
        }
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC29
#[test]
fn a_box_thread_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let lab = box_lab();
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|_t: &str, _c: Option<&Caller>| false));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(lab.boxd.chans(), vec![]);
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// AC29: this computer once, each box once for all its threads, a box's threads named by the box
#[test]
fn meet_reads_this_computer_once_and_each_box_once_and_names_the_box() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "second", STARTED + 10);
        lab.box_thread(T3, "w3", "third", STARTED + 20);
        lab.boxd.set_procs(vec![
            box_row(7300, 1, "node", "node /root/two/node_modules/.bin/vite", &thread_cgroup(T2)),
            box_row(7400, 1, "node", "node /root/three/node_modules/.bin/vite", &thread_cgroup(T3)),
        ]);
        let refusal = lab.meet(&[T1, T2, T3], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        let on_box = |id: &str, title: &str, ws: &str, pid: u32, path: &str| {
            left_thread(
                id,
                title,
                ws,
                50_000_000,
                1,
                vec![box_shown(pid, 1, 0, "node", &format!("node /root/{path}/node_modules/.bin/vite"), &[])],
                Some("hetzner"),
            )
        };
        let left = [
            t1_left(vec![vite(), esbuild()], 470_000_000),
            on_box(T2, "second", "w2", 7300, "two"),
            on_box(T3, "third", "w3", 7400, "three"),
        ];
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &left, &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())]);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
        assert_eq!(lab.host.hooks.called("box_channel"), vec!["box_channel(box)"]);
    });
}

// ===============================================================================================================
// A box thread's end
// ===============================================================================================================

fn pty_row(pid: u32, pty: &str, cgroup: &str) -> Value {
    let mut r = box_row(pid, 1, "zsh", "-zsh", cgroup);
    r["pty"] = json!(pty);
    r
}

fn pty_shown(pid: u32) -> Value {
    shown(pid, 1, 0, STARTED + pid as i64, "terminal", "zsh", "-zsh", 50_000_000, 1, &[])
}

/// A box thread whose cgroup holds vite and two panes' shells, and a shell of another cgroup beside them.
fn box_end_lab() -> Lab {
    let lab = Lab::new();
    lab.join_box();
    lab.box_thread(T1, "w1", "start the dev server", STARTED);
    let mine = thread_cgroup(T1);
    lab.boxd.set_procs(vec![
        box_row(7100, 1, "node", "node /root/lab/node_modules/.bin/vite", &mine),
        pty_row(7300, "pty_1", &mine),
        pty_row(7400, "pty_2", &mine),
        pty_row(7500, "pty_9", &thread_cgroup("other-thread")),
    ]);
    lab.boxd.set_ports(&[(5173, 7100)]);
    lab
}

fn box_ended_rows() -> Vec<Value> {
    vec![box_shown(7100, 1, 0, "node", "node /root/lab/node_modules/.bin/vite", &[5173]), pty_shown(7300), pty_shown(7400)]
}

fn arm_end_thread(lab: &Lab, on_end: impl Fn() + 'static) {
    lab.host.hooks.set::<dyn Fn(&Entry, &str, EndThread) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>>>(
        "ActivityCtx::end_thread",
        Rc::new(move |_e: &Entry, _t: &str, _o: EndThread| {
            on_end();
            Some(async { Ok(None) }.boxed_local())
        }),
    );
}

fn end_calls(lab: &Lab) -> Vec<String> {
    lab.host
        .hooks
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("end_thread") || c.starts_with("with_daemon") || c.starts_with("close_servers"))
        .collect()
}

fn pty_frames() -> Vec<Value> {
    vec![json!({ "op": "pty.kill", "ptyId": "pty_1" }), json!({ "op": "pty.kill", "ptyId": "pty_2" })]
}

// AC21
#[test]
fn a_box_end_kills_each_listed_shell_and_waits_out_the_grace() {
    run_local(async {
        let lab = box_end_lab();
        arm_end_thread(&lab, || {});
        *lab.boxd.pty_kill.borrow_mut() =
            Rc::new(|_d: &Daemon, pty: &str| if pty == "pty_1" { Err(refused(None, "no such shell")) } else { Ok(reply(json!({}))) });
        let warned = wsp_core::console::capture();
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.meet(&ids(&[T1]), ProcessesAsk::Stop, MeetVerb::Settle, None).await });
        for i in 1..=10 {
            settle().await;
            assert!(!task.is_finished(), "still inside the grace at poll {i}");
            lab.host.clock.advance(200);
        }
        settle().await;
        assert!(task.is_finished(), "the grace is {END_GRACE_MS} ms of the clock");
        let answer = all_ended(task.await.expect("the task ran"));
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[7100, 7300, 7400])]));
        assert_eq!(lab.boxd.direct.borrow().clone(), pty_frames(), "each listed shell, one at a time, never the other cgroup's");
        assert_eq!(warned.lines(), vec![format!("shell pty_1 of thread {} was not ended: no such shell", thread_word(T1))]);
        assert_eq!(
            end_calls(&lab),
            vec![format!("end_thread({T1})"), "with_daemon".to_string(), "with_daemon".to_string(), format!("close_servers({T1})")]
        );
        let chans = lab.boxd.chans();
        assert_eq!(chans.len(), 12, "the meet's read, one look after the end, one after each 200 ms");
        assert!(chans.iter().all(|c| *c == chan(false, list_frames())));
        let seq = lab.boxd.seq.borrow().clone();
        let at = seq.iter().position(|s| s == "direct:pty.kill").expect("a pty.kill");
        assert_eq!(&seq[at..at + 2], ["direct:pty.kill", "direct:pty.kill"]);
        assert_eq!(seq[..at].last().map(String::as_str), Some("c0:close"), "the shells are ended after the meet's own read");
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC21: nothing left at the first look ends the wait
#[test]
fn a_box_end_that_took_hold_before_the_first_look_does_not_wait() {
    run_local(async {
        let lab = box_end_lab();
        let d = lab.boxd.clone();
        let mine = thread_cgroup(T1);
        arm_end_thread(&lab, move || d.procs.borrow_mut().retain(|p| p["cgroup"] != json!(mine)));
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[])]));
        assert_eq!(lab.boxd.direct.borrow().clone(), pty_frames(), "the shells are the first read's, not the next one's");
        assert_eq!(lab.boxd.chans().len(), 2);
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC19, AC21: a look that fails counts every process standing
#[test]
fn a_failed_look_during_the_grace_counts_every_process_standing() {
    run_local(async {
        let lab = box_end_lab();
        let d = lab.boxd.clone();
        arm_end_thread(&lab, move || *d.list_error.borrow_mut() = Some(refused(None, "disk full")));
        let warned = wsp_core::console::capture();
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.meet(&ids(&[T1]), ProcessesAsk::Stop, MeetVerb::Settle, None).await });
        for _ in 1..=10 {
            settle().await;
            lab.host.clock.advance(200);
        }
        settle().await;
        assert!(task.is_finished());
        let answer = all_ended(task.await.expect("the task ran"));
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[7100, 7300, 7400])]));
        let line = "hetzner's processes were not read for a settle or a delete: disk full";
        assert_eq!(warned.lines(), vec![line; 11], "each failed look is warned");
    });
}

// AC19
#[test]
fn a_quiet_box_read_that_fails_is_warned_and_holds_nothing() {
    run_local(async {
        let lab = box_end_lab();
        *lab.boxd.list_error.borrow_mut() = Some(refused(None, "disk full"));
        let warned = wsp_core::console::capture();
        for processes in [ProcessesAsk::Ask, ProcessesAsk::Stop] {
            let answer = all_ended(lab.meet(&[T1], processes, MeetVerb::Settle, None).await);
            assert_eq!(answer, json!([]));
        }
        let line = "hetzner's processes were not read for a settle or a delete: disk full";
        assert_eq!(warned.lines(), vec![line; 2]);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
        assert_eq!(lab.boxd.direct.borrow().clone(), Vec::<Value>::new());
        assert_eq!(lab.host.hooks.called("end_thread"), Vec::<String>::new());
    });
}

// AC19
#[test]
fn a_box_that_is_away_holds_nothing_for_a_settle() {
    run_local(async {
        let lab = box_end_lab();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| false));
        let warned = wsp_core::console::capture();
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([]));
        assert_eq!(warned.lines(), vec!["hetzner's processes were not read for a settle or a delete: hetzner is away"]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}
