use std::cell::{Cell, RefCell};

use std::collections::BTreeSet;

use std::rc::Rc;

use std::sync::atomic::Ordering;

use futures::future::{FutureExt, LocalBoxFuture, Shared};

use indexmap::IndexMap;

use serde_json::{json, Value};

use support::threads_ctx::{entry, held_project, FakeThreadsCtx};

use support::Lend;

use support::{local, record, row, rows_of, settle, silent, view, FakeKeptAgent};

use wsp_adapters::port::{SessionFile, SessionRenameWrite};

use wsp_core::context::Joined;

use wsp_core::types::internal::{CallSeen, KeptLaunch, Steered, Taken, TakenOutcome, TurnAsked, TurnLive, WarmKeep};

use wsp_core::types::wiring::{SessionHandle, SteerTaken};

use wsp_threads::threads::threads::cap::CapAt;

use wsp_threads::threads::threads::kept::{AGENTS_KEPT, AGENT_KEEP_MS, AGENT_WARM_MS};

use wsp_threads::threads::threads::tree::{CallsOf, TellAs};

use wsp_threads::threads::threads::turn_end::{SteerSent, TopRead, TurnChangesAsk};

use wsp_threads::threads::threads::{Entry, KeptHand, Owed, PersonRow, Row, ThreadsArea};

use wsp_wire::views::session::{SessionView, ThreadCapWait, ThreadPrompt};

use wsp_wire::views::session_events::TurnResult;

use wsp_wire::views::session_events_more::SessionUncappedEventWhy;

use wsp_wire::views::workspace::Caller;

use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};

use wsp_wire::wire::runtime::SessionStartOutcome;

use wsp_words::words::turn::{notify_line, unread_line, NotifyLength};

const FROM: &str = "child0001";

const PARENT: &str = "parent0001";

const HOUR: i64 = 3_600_000;

const NO_SUCH_TURN: &str =
    "no turn on this host carries that token; only wsp running inside a turn has one, and a turn that ended has none";

// ---- shared builders ------------------------------------------------------------------------------------------------

/// A handle a start answered, its finish settled through the sender (copied from threads_owed.rs).
struct FakeHandle {
    id: String,
    turn: String,
    outcome: SessionStartOutcome,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    tells_unread: Option<bool>,
}

type Finish = Rc<RefCell<Option<futures::channel::oneshot::Sender<TurnResult>>>>;

fn handle(id: &str, turn: &str, outcome: SessionStartOutcome, tells_unread: Option<bool>) -> (Rc<dyn SessionHandle>, Finish) {
    let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
    let finished = rx.map(|r| r.unwrap_or_else(|_| result("failed", None))).boxed_local().shared();
    (Rc::new(FakeHandle { id: id.into(), turn: turn.into(), outcome, finished, tells_unread }), Rc::new(RefCell::new(Some(tx))))
}

impl SessionHandle for FakeHandle {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn workspace_id(&self) -> String {
        "ws1".into()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.finished.clone()
    }
    fn turn_id(&self) -> String {
        self.turn.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        view(json!({ "id": self.id }))
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<wsp_adapters::port::TurnImage>,
    ) -> Option<wsp_core::types::wiring::Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        self.tells_unread
    }
    fn answer(
        &self,
        _ask: String,
        _option: String,
        _reason: Option<String>,
    ) -> Option<wsp_core::types::wiring::Local<'static, wsp_wire::wire::runtime::SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_core::types::wiring::AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_adapters::port::TaskStop>> {
        None
    }
}

fn result(status: &str, error: Option<&str>) -> TurnResult {
    let mut v = json!({ "status": status });
    if let Some(e) = error {
        v["error"] = json!(e);
    }
    serde_json::from_value(v).expect("a TurnResult")
}

fn result_of(v: Value) -> TurnResult {
    serde_json::from_value(v).expect("a TurnResult")
}

fn finish(tx: &Finish, r: TurnResult) {
    let _ = tx.borrow_mut().take().expect("finished once").send(r);
}

fn scope_of(thread: &str, workspace: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root }))
        .expect("a scope")
}

fn origin(word: &str) -> WorkspaceOrigin {
    serde_json::from_value(json!(word)).expect("an origin")
}

fn scoped(thread: &str, workspace: &str, root: &str) -> Caller {
    Caller::Scoped { origin: origin("relayed"), by: scope_of(thread, workspace, root) }
}

/// A host whose parent thread has a settled row with an agent session on a running workspace `ws1`.
fn owed_host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "phase": "running" }));
    ctx.add_entry(e);
    ctx.sessions.with(|s| {
        s.insert(
            "r-p".into(),
            row(view(json!({ "id": "r-p", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "completed", "startedAt": 1 })), "u-p"),
        );
    });
    let area = ctx.area();
    (ctx, area)
}

fn person_row() -> PersonRow {
    PersonRow {
        workspace_id: "ws-c".into(),
        session_id: "cs-c".into(),
        turn_id: "u-c".into(),
        text: "thread child000 finished (completed)".into(),
    }
}

fn line(id: &str) -> Owed {
    Owed {
        id: id.into(),
        from: FROM.into(),
        notify: PARENT.into(),
        text: "thread child000 finished (completed): the report".into(),
        by: None,
        road: None,
        started_by: None,
        to_person: Some(person_row()),
        since: None,
        request_id: None,
        tries: None,
        next: None,
        images: None,
        awaits_result: None,
    }
}

fn told_person(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.notify" && e["notify"] == "me").cloned().collect()
}

fn set_phase(ctx: &FakeThreadsCtx, ws: &str, phase: &str) {
    let entries = ctx.entries.borrow();
    let e = entries.get(ws).expect("live").borrow();
    let moved = serde_json::from_value({
        let mut v = serde_json::to_value(&*e.record.borrow()).expect("a record");
        v["phase"] = json!(phase);
        v
    })
    .expect("a record");
    *e.record.borrow_mut() = moved;
}

fn child_row(claude: Option<&str>, told: Option<u64>) -> Row {
    let mut v = json!({ "id": "r-c", "workspaceId": "ws-c", "threadId": FROM });
    if let Some(c) = claude {
        v["claudeSessionId"] = json!(c);
    }
    let r = row(view(v), "u-c");
    r.borrow_mut().turn_live = told.map(|n| TurnLive { told: Some(n), ..Default::default() });
    r
}

fn taken_by(ctx: &FakeThreadsCtx, request: &str, row: &str, turn: &str) {
    ctx.taken.borrow_mut().insert(
        ("ws1".into(), request.into()),
        Taken { session_id: row.into(), thread_id: PARENT.into(), turn_id: turn.into(), outcome: TakenOutcome::Started },
    );
}

fn woken(ws: &str) -> Value {
    json!({ "type": "workspace.woken", "workspaceId": ws, "machineId": "m1" })
}

fn turn_row(ctx: &FakeThreadsCtx, id: &str) -> Row {
    ctx.sessions.with(|s| {
        let r = s.get(id).expect("a row").borrow();
        let copy = row(r.view.clone(), &r.turn_id);
        copy.borrow_mut().turn_live = r.turn_live.clone();
        copy
    })
}

fn steer_sent(prompt: &str) -> SteerSent {
    SteerSent { prompt: prompt.into(), request_id: Some("req-s".into()), via_slate: false, started_by: None, attachments: vec![] }
}

fn steered_on(ctx: &FakeThreadsCtx) -> Vec<String> {
    ctx.sessions.with(|s| {
        let steered: Vec<String> = s
            .get("r1")
            .map(|r| {
                r.borrow().turn_live.as_ref().and_then(|l| l.steered.as_ref()).map(|m| m.keys().cloned().collect()).unwrap_or_default()
            })
            .unwrap_or_default();
        steered
    })
}

fn unread_row(steered: Value) -> Row {
    let steered: Steered = serde_json::from_value(steered).expect("a steer");
    let r = row(view(json!({ "id": "r-t", "workspaceId": "ws1", "threadId": PARENT, "claudeSessionId": "cs-p" })), "u-t");
    r.borrow_mut().turn_live = Some(TurnLive { steered: Some([("st1".to_string(), steered)].into_iter().collect()), ..Default::default() });
    r
}

fn running_row(ctx: &FakeThreadsCtx, thread: &str, notify: Option<Vec<&str>>) {
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r1", "workspaceId": "ws1", "threadId": thread, "status": "running" })), "u1");
        r.borrow_mut().notify = notify.map(|n| n.into_iter().map(String::from).collect());
        r.borrow_mut().turn_live = None;
        s.insert("r1".into(), r);
    });
}

// =====================================================================================================================
// thread-cap.test.ts: a computer's threads at once
// =====================================================================================================================

fn cap_host(at_once: i64, running: usize) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, wsp_core::types::wiring::WorkspaceRecord) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    let record = e.borrow().record.borrow().clone();
    ctx.add_entry(e);
    ctx.place_ids.borrow_mut().insert("ws1".into(), "here".into());
    ctx.threads_at.borrow_mut().insert("here".into(), at_once);
    ctx.sessions.with(|s| {
        *s = rows_of(
            (0..running)
                .map(|i| row(view(json!({ "id": format!("u{i}"), "threadId": format!("t{i}"), "status": "running" })), &format!("u{i}")))
                .collect(),
        )
    });
    let area = ctx.area();
    (ctx, area, record)
}

fn start_row(ctx: &FakeThreadsCtx, turn: &str, thread: &str) {
    ctx.sessions.with(|s| {
        s.insert(turn.to_string(), row(view(json!({ "id": turn, "threadId": thread, "status": "running", "harness": "codex" })), turn));
    });
}

fn cap_wait_of(running: i64, at_once: i64) -> ThreadCapWait {
    serde_json::from_value(json!({ "placeId": "here", "place": "here-name", "running": running, "atOnce": at_once })).expect("a wait")
}

fn at(turn: &str, thread: &str) -> CapAt {
    CapAt {
        workspace_id: "ws1".into(),
        thread_id: thread.into(),
        turn_id: turn.into(),
        session_id: format!("s-{turn}"),
        request_id: Some("req1".into()),
        prompt: "build the thing\nwhole".into(),
    }
}

fn end_of(turn: &str) -> Value {
    json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "x", "turnId": turn, "exitCode": null, "sawResult": false })
}

/// A turn's end: its row goes and the end is announced, as a turn that ended does.
fn turn_ends(ctx: &FakeThreadsCtx, turn: &str) {
    ctx.sessions.with(|s| s.shift_remove(turn));
    ctx.bus.emit_json(end_of(turn));
}

fn borrowed_of(area: &ThreadsArea, turn: &str) -> Option<String> {
    area.borrowed.borrow().get(turn).cloned()
}

// =====================================================================================================================
// thread-keeps-process, thread-warm-agent and box-keep: agent processes kept between turns
// =====================================================================================================================

struct Agent {
    closed: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    working: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Agent {
    fn closes(&self) -> Vec<bool> {
        self.closed.lock().expect("closed").clone()
    }
}

fn launch(fixed: &str, picks: Value) -> KeptLaunch {
    KeptLaunch { fixed: fixed.into(), picks: serde_json::from_value(picks).expect("picks") }
}

fn kept_hand(workspace: &str, file: Option<SessionFile>, token: &str) -> (KeptHand, Agent) {
    let agent = FakeKeptAgent::new(file);
    let seen = Agent { closed: agent.closed.clone(), working: agent.working.clone() };
    (
        KeptHand {
            workspace_id: workspace.into(),
            agent: Box::new(agent),
            pid: None,
            launch: launch("claude|/work|v1", json!({ "model": "opus" })),
            session: Some("s1".into()),
            warm: None,
            turn_token: token.into(),
            scope_device_id: None,
            waiting: Default::default(),
        },
        seen,
    )
}

fn warm_kept_hand(workspace: &str) -> (KeptHand, Agent) {
    let (mut h, a) = kept_hand(workspace, None, "tok-warm");
    h.session = None;
    h.warm = Some(WarmKeep { harness: "claude".into(), claimed: false });
    (h, a)
}

/// A local workspace `ws1`, a workspace `wsb` on the joined computer `box1` (4 GB, linked), and records for `threads`.
fn kept_host(threads: &[&str]) -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, std::sync::Arc<support::FakeMachine>) {
    let ctx = FakeThreadsCtx::new();
    let (local_ws, _) = entry(json!({ "id": "ws1", "kind": "local" }));
    ctx.add_entry(local_ws);
    let (boxed, machine) = entry(json!({ "id": "wsb", "kind": "local", "place": "box1" }));
    ctx.add_entry(boxed);
    ctx.mem_mb.borrow_mut().insert("box1".into(), 4096.0);
    ctx.linked.borrow_mut().push("box1".into());
    ctx.thread_records.with(|r| {
        for t in threads {
            r.insert(t.to_string(), record("ws1", json!({})));
        }
    });
    let area = ctx.area();
    (ctx, area, machine)
}

fn session_file() -> SessionFile {
    SessionFile { folder: "/home/u/.claude/projects".into(), name: "s1.jsonl".into(), depth: 1 }
}

fn kept_ids(area: &ThreadsArea) -> Vec<String> {
    area.kept_agents.borrow().keys().cloned().collect()
}

fn same_launch() -> KeptLaunch {
    launch("claude|/work|v1", json!({}))
}

fn wc_reads(machine: &support::FakeMachine) -> usize {
    machine.commands().iter().filter(|c| c.contains("wc -c")).count()
}

// =====================================================================================================================
// turn-changes.test.ts: what a turn changed
// =====================================================================================================================

fn te_host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, Entry) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local", "name": "the-folder" }));
    ctx.add_entry(e.clone());
    let area = ctx.area();
    (ctx, area, e)
}

fn beside(ctx: &FakeThreadsCtx, rows: Vec<Value>) {
    ctx.sessions.with(|s| {
        for (i, v) in rows.into_iter().enumerate() {
            let id = format!("b{i}");
            let mut v = v;
            v["id"] = json!(id);
            s.insert(id.clone(), row(view(v), &format!("ub{i}")));
        }
    });
}

fn diff(files: Value, moved: Value) -> Value {
    json!({ "base": null, "files": files, "truncated": false, "moved": moved })
}

fn changed(path: &str) -> Value {
    json!({ "path": path, "kind": "added", "additions": 1, "deletions": 0, "patch": "@@" })
}

fn bare(path: &str) -> Value {
    json!({ "path": path, "kind": "added", "additions": 1, "deletions": 0 })
}

fn turn_ask(thread: &str, cwd: &str, wrote: Option<&[&str]>) -> TurnChangesAsk {
    TurnChangesAsk {
        session_id: "cs-1".into(),
        turn_id: format!("u-{thread}"),
        thread_id: thread.into(),
        cwd: cwd.into(),
        from: "base123".into(),
        started_at: 999_000,
        wrote: wrote.map(|w| w.iter().map(|p| p.to_string()).collect::<BTreeSet<_>>()),
    }
}

fn top_of_repo() -> (Value, TopRead) {
    let reads = Rc::new(Cell::new(0));
    let read: TopRead = Rc::new(move || {
        reads.set(reads.get() + 1);
        async { Some("/repo".to_string()) }.boxed_local()
    });
    (json!({ "worktrees": [{ "path": "/repo" }] }), read)
}

// =====================================================================================================================
// spawn-guard.test.ts, settle-tree.test.ts, waiting-behind.test.ts: the tree a thread's token reaches
// =====================================================================================================================

fn roots(ctx: &FakeThreadsCtx, pairs: &[(&str, &str)]) {
    for (thread, root) in pairs {
        ctx.roots.borrow_mut().insert(thread.to_string(), root.to_string());
    }
}

fn prompt_of(ask_id: &str) -> ThreadPrompt {
    serde_json::from_value(json!({
        "askId": ask_id,
        "toolName": "Read",
        "toolUseId": "toolu_read",
        "input": "{\"file_path\":\"/root/hello.txt\"}",
        "detail": "hello.txt",
        "options": [{ "id": "allow", "label": "Allow", "effect": "allow" }, { "id": "deny", "label": "Deny", "effect": "deny" }],
    }))
    .expect("a prompt")
}

fn calls_of(calls: &[(&str, &str, Value)]) -> IndexMap<String, CallSeen> {
    calls.iter().map(|(id, tool, input)| (id.to_string(), CallSeen { tool_name: tool.to_string(), input: input.to_string() })).collect()
}

fn waiting_pair() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-caller", "threadId": "caller01", "status": "running", "prompt": "ask the other one" })), "u1"),
            row(view(json!({ "id": "r-target", "threadId": "target01", "status": "running", "prompt": "read the file" })), "u2"),
        ])
    });
    let area = ctx.area();
    (ctx, area)
}
