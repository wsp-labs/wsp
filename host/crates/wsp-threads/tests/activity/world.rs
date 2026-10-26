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
        h.set::<dyn Fn(&Entry, Value) -> LocalBoxFuture<'static, Result<Value, AccountError>>>(
            "ActivityCtx::with_daemon",
            Rc::new(move |_e: &Entry, frame: Value| {
                let asked = d.ask_direct(frame);
                async move { asked.await.map(Value::Object) }.boxed_local()
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

fn with_shell() -> Lab {
    let lab = Lab::main();
    lab.here.procs.borrow_mut().extend(shell_procs(T1));
    lab
}

fn shell_left() -> LeftThread {
    t1_left(vec![vite(), esbuild(), shell(), shell_child()], 472_000_000)
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
