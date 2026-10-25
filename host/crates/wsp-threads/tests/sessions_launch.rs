// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_launch.md. TS cases: session-held.test.ts:15, session-opening.test.ts:50, owed-box.test.ts:56,
//! runtime-steer.test.ts:512-643, thread-cap.test.ts:111, 566-645, 1060, session-images.test.ts:88-381,
//! folder-threads.test.ts:637, 1038, thread-keeps-process.test.ts:92, session-mcp.test.ts:44, lost-session-line.test.ts:102,
//! replaces.test.ts:42, first-run.test.ts:128 (host package). Each test builds a `Prepared` by hand (the state
//! sessions_start leaves) and calls `launch_prepared`.
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use futures::future::{BoxFuture, Shared};
use support::*;
use wsp_core::console;
use wsp_core::types::wiring::{AccessMove, SteerTaken};
use wsp_threads::threads::sessions::start::Prepared;
use wsp_wire::types::attachments::{Attachment, AttachmentRecord};
use wsp_wire::views::session_events_more::SessionEarlierEvent;
use wsp_wire::views::session::{thread_ran, ForkedFrom, SessionOrigin, TitleSource};
use wsp_wire::views::session_events::AGENT_STARTING_MS;
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionInterruptOutcome, SessionInterruptResult, SessionStartOutcome};
use wsp_words::attachments::{attached_files_prompt, send_files_dir, turn_images_dir};
use wsp_words::place_state::deleted_before_start_line;
use wsp_words::projects::{home_shortened, no_cwd_line};
use wsp_words::thread_read::{thread_messages, thread_seed};
use wsp_words::words::computer::{send_given_up_line, thread_word};
use wsp_words::words::turn::{kept_opening, listed_failure, title_line, OPENING_KEPT};

type Launched = Result<Rc<dyn SessionHandle>, HostError>;
type Gate = (oneshot::Sender<()>, Joined);
type Landings = Rc<RefCell<Vec<(String, String, Vec<Attachment>)>>>;

fn gate() -> Gate {
    let (tx, rx) = oneshot::channel::<()>();
    (tx, futures::FutureExt::map(rx, |_| ()).boxed_local().shared())
}

fn ok(r: Launched) -> Rc<dyn SessionHandle> {
    match r {
        Ok(h) => h,
        Err(e) => panic!("the launch was to succeed, and said: {}", e.message),
    }
}

fn err(r: Launched) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("the launch was to fail"),
    }
}

fn merged(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().expect("extra is an object") {
        base[k] = v.clone();
    }
    base
}

/// The index of the first call of `name` in the log.
fn at(calls: &[String], name: &str) -> usize {
    calls.iter().position(|c| c == name || c.starts_with(&format!("{name}("))).unwrap_or_else(|| panic!("{name} was never called: {calls:?}"))
}

// ---------------------------------------------------------------------------------------------------------------
// A started turn the test holds: its view, how it finishes and what was steered into it.

struct Handle {
    id: String,
    workspace_id: String,
    turn_id: String,
    outcome: SessionStartOutcome,
    view: SharedView,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    finish: RefCell<Option<oneshot::Sender<TurnResult>>>,
    steers: bool,
    steer_answer: RefCell<Result<SteerTaken, String>>,
    unread: Option<bool>,
    steered: RefCell<Vec<(String, Option<String>)>>,
}

impl Handle {
    fn new(turn_id: &str, outcome: SessionStartOutcome, view: SharedView) -> Handle {
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let finished = futures::FutureExt::map(rx, |r| r.unwrap_or_else(|_| result(json!({ "status": "failed" })))).boxed_local().shared();
        let (id, workspace_id) = {
            let v = view.borrow();
            (v.id.clone(), v.workspace_id.clone())
        };
        Handle {
            id,
            workspace_id,
            turn_id: turn_id.to_string(),
            outcome,
            view,
            finished,
            finish: RefCell::new(Some(tx)),
            steers: false,
            steer_answer: RefCell::new(Ok(SteerTaken::Accepted)),
            unread: None,
            steered: RefCell::new(Vec::new()),
        }
    }

    fn finish(&self, doc: Value) {
        if let Some(tx) = self.finish.borrow_mut().take() {
            let _ = tx.send(result(doc));
        }
    }
}

impl SessionHandle for Handle {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.finished.clone()
    }
    fn turn_id(&self) -> String {
        self.turn_id.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        self.view.borrow().clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(&self, prompt: String, id: Option<String>, _images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        if !self.steers {
            return None;
        }
        self.steered.borrow_mut().push((prompt, id));
        let answer = self.steer_answer.borrow().clone().map_err(HostError::new);
        Some(futures::future::ready(answer).boxed_local())
    }
    fn tells_unread(&self) -> Option<bool> {
        self.unread
    }
    fn answer(&self, _ask_id: String, _option_id: String, _reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

/// A turn of `thread` the host holds as running, with its handle, under the key `key`.
fn add_running(host: &FakeHost, key: &str, turn_id: &str, thread: &str, extra: Value, set: impl FnOnce(&mut Handle)) -> Rc<Handle> {
    let doc = merged(json!({ "threadId": thread, "startedAt": T0 - 1000, "prompt": "one", "claudeSessionId": "sess-old" }), extra);
    let v = shared(view(key, "w1", doc));
    let mut h = Handle::new(turn_id, SessionStartOutcome::Started, v.clone());
    set(&mut h);
    let h = Rc::new(h);
    let mut r = row(v.borrow().clone(), turn_id);
    r.view = v;
    r.handle = Some(h.clone() as Rc<dyn SessionHandle>);
    host.add_row(key, r);
    h
}

/// A turn another send is still carrying to the machine: running, no handle, a launch not yet settled.
fn add_launching(host: &FakeHost, key: &str, thread: &str, launch: Joined) {
    let v = shared(view(key, "w1", json!({ "threadId": thread, "startedAt": T0 - 1000, "prompt": "one" })));
    let mut r = row(v.borrow().clone(), key);
    r.view = v;
    r.launch = Some(launch);
    host.add_row(key, r);
}

/// The turn is over: its row reads completed and its finish settles.
fn end_turn(host: &FakeHost, key: &str, h: &Handle) {
    host.rows.borrow()[key].view.borrow_mut().status = serde_json::from_value(json!("completed")).expect("a status");
    h.finish(json!({ "status": "completed", "text": "done" }));
}

// ---------------------------------------------------------------------------------------------------------------
// An adapter that records each start, and a kept process that records each next turn.

struct Started {
    prompt: String,
    resume: Option<String>,
    copy: bool,
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
    context_window: Option<String>,
    fast: Option<bool>,
    title: Option<String>,
    images: Vec<TurnImage>,
    fork: Option<(String, String)>,
    mcp_servers: Option<IndexMap<String, wsp_adapters::port::McpServerSpec>>,
    server_values: Option<ServerValues>,
    off_servers: Option<Vec<String>>,
    seed: Option<Arc<dyn Fn() -> BoxFuture<'static, Result<String, String>> + Send + Sync>>,
    limit_details: bool,
    prompt_after: bool,
    version: Option<String>,
    keep: bool,
}

#[derive(Default)]
struct FakeAdapter {
    steers: bool,
    steers_images: bool,
    attachments: Option<AttachmentRoad>,
    mcp: bool,
    denies: bool,
    reports_edits: bool,
    waits_prompt: bool,
    starts: Mutex<Vec<Started>>,
}

impl HarnessAdapter for FakeAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.starts.lock().unwrap().push(Started {
            prompt: o.prompt,
            resume: o.resume,
            copy: o.copy,
            cwd: o.cwd,
            model: o.model,
            effort: o.effort,
            permission_mode: o.permission_mode,
            context_window: o.context_window,
            fast: o.fast,
            title: o.title,
            images: o.images,
            fork: o.fork,
            mcp_servers: o.mcp_servers,
            server_values: o.server_values,
            off_servers: o.off_servers,
            seed: o.seed,
            limit_details: o.limit_details,
            prompt_after: o.prompt_after.is_some(),
            version: o.version,
            keep: o.keep,
        });
        Ok(FakeSession::new("sess-started"))
    }
    fn steers(&self) -> bool {
        self.steers
    }
    fn reports_edits(&self) -> bool {
        self.reports_edits
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.attachments
    }
    fn mcp_servers(&self) -> bool {
        self.mcp
    }
    fn denies_servers(&self) -> bool {
        self.denies
    }
    fn steers_images(&self) -> bool {
        self.steers_images
    }
    fn waits_for_prompt(&self) -> bool {
        self.waits_prompt
    }
}

fn adapter(set: impl FnOnce(&mut FakeAdapter)) -> Arc<FakeAdapter> {
    let mut a = FakeAdapter::default();
    set(&mut a);
    Arc::new(a)
}

/// What a kept process was handed for a next turn.
struct Next {
    prompt: String,
    images: Vec<TurnImage>,
    after: bool,
}

#[derive(Default)]
struct FakeKept {
    nexts: Mutex<Vec<Next>>,
}

impl KeptAgent for FakeKept {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.nexts.lock().unwrap().push(Next { prompt: turn.prompt, images: turn.images, after: turn.after.is_some() });
        Ok(FakeSession::new("sess-kept"))
    }
    fn close(&self, _now: bool) -> BoxFuture<'static, ()> {
        futures::FutureExt::boxed(async {})
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::FutureExt::shared(futures::FutureExt::boxed(futures::future::pending::<()>()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The rig: one host, one workspace `w1`, the launch area, and the hooks every launch needs.

/// What the host held at one call: the rows, what was emitted and the calls so far.
struct Seen {
    rows: Vec<(String, SessionEntry, SessionView)>,
    emitted: Vec<Value>,
    calls: Vec<String>,
}

fn snapshot(host: &FakeHost) -> Seen {
    Seen {
        rows: host.rows.borrow().iter().map(|(k, r)| (k.clone(), r.clone(), r.view.borrow().clone())).collect(),
        emitted: host.emitted.borrow().clone(),
        calls: host.hooks.calls(),
    }
}

struct Rig {
    host: Rc<FakeHost>,
    area: SessionsArea,
    entry: Entry,
    adapter: Arc<FakeAdapter>,
    /// Every `TurnStart` `run_turn` was handed.
    turns: Rc<RefCell<Vec<TurnStart>>>,
    /// The handles `run_turn` answered.
    handles: Rc<RefCell<Vec<Rc<Handle>>>>,
    /// The host's state at each ask for the agent's lists, the first thing a launch asks of the machine.
    seen: Rc<RefCell<Vec<Seen>>>,
    catalog: Rc<RefCell<HarnessCatalog>>,
}

fn catalog(source: &str, version: Option<&str>, models: Value) -> HarnessCatalog {
    serde_json::from_value(json!({
        "harness": "claude", "label": "Claude Code", "source": source, "version": version, "models": models, "efforts": [],
        "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false,
    }))
    .expect("a catalog the fixture builds")
}

fn rig() -> Rig {
    rig_with(adapter(|_| {}))
}

fn rig_with(adapter: Arc<FakeAdapter>) -> Rig {
    let host = FakeHost::new();
    let entry = host.add_entry(entry("w1", json!({})));
    *host.ids.uuids.borrow_mut() = vec!["turn-1".to_string(), "minted-1".to_string()];
    let turns: Rc<RefCell<Vec<TurnStart>>> = Rc::new(RefCell::new(Vec::new()));
    let handles: Rc<RefCell<Vec<Rc<Handle>>>> = Rc::new(RefCell::new(Vec::new()));
    let seen: Rc<RefCell<Vec<Seen>>> = Rc::new(RefCell::new(Vec::new()));
    let catalog = Rc::new(RefCell::new(catalog("table", None, json!([]))));

    let h = host.clone();
    host.hooks.set::<dyn Fn() -> LaunchArea>("SessionsCtx::launch_area", Rc::new(move || LaunchArea::new(h.clone())));

    let (t, hs, h) = (turns.clone(), handles.clone(), host.clone());
    host.hooks.set::<dyn Fn(TurnStart) -> Result<Rc<dyn SessionHandle>, HostError>>(
        "SessionsCtx::run_turn",
        Rc::new(move |turn: TurnStart| {
            let handle = Rc::new(Handle::new(&turn.turn_id, turn.outcome, turn.view.clone()));
            hs.borrow_mut().push(handle.clone());
            if let Some(r) = h.rows.borrow_mut().get_mut(&turn.turn_id) {
                r.handle = Some(handle.clone() as Rc<dyn SessionHandle>);
            }
            t.borrow_mut().push(turn);
            Ok(handle as Rc<dyn SessionHandle>)
        }),
    );

    let (s, c, h) = (seen.clone(), catalog.clone(), host.clone());
    host.hooks.set::<dyn Fn(&str, &Entry, &dyn HarnessAdapter) -> LocalBoxFuture<'static, Result<Option<HarnessCatalog>, HostError>>>(
        "LaunchCtx::catalog_on",
        Rc::new(move |_harness: &str, _entry: &Entry, _adapter: &dyn HarnessAdapter| {
            s.borrow_mut().push(snapshot(&h));
            let c = c.borrow().clone();
            async move { Ok(Some(c)) }.boxed_local()
        }),
    );

    host.hooks.set::<dyn Fn(&dyn HarnessAdapter, &str, &str, &str) -> LocalBoxFuture<'static, Result<String, HostError>>>(
        "SessionsCtx::pay_cut_owed",
        Rc::new(|_a: &dyn HarnessAdapter, _w: &str, _t: &str, resume: &str| {
            let resume = resume.to_string();
            async move { Ok(resume) }.boxed_local()
        }),
    );

    host.hooks.set::<dyn Fn(&Entry, &dyn HarnessAdapter, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<Landed, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(|_e: &Entry, _a: &dyn HarnessAdapter, _landing: &str, _dir: &str, _x: Vec<Attachment>| async { Ok(Landed::default()) }.boxed_local()),
    );

    let area = SessionsArea::new(host.clone());
    Rig { host, area, entry, adapter, turns, handles, seen, catalog }
}

impl Rig {
    async fn run(&self, p: Prepared) -> Launched {
        self.area.launch_prepared(p).await
    }

    fn spawn(&self, p: Prepared) -> tokio::task::JoinHandle<Launched> {
        let area = self.area.clone();
        tokio::task::spawn_local(async move { area.launch_prepared(p).await })
    }

    fn emitted(&self) -> Vec<Value> {
        self.host.emitted.borrow().clone()
    }

    fn emitted_of(&self, kind: &str) -> Vec<Value> {
        self.emitted().into_iter().filter(|e| e["type"] == kind).collect()
    }

    fn row_keys(&self) -> Vec<String> {
        self.host.rows.borrow().keys().cloned().collect()
    }

    /// The row of the turn this send holds, as the host holds it.
    fn row(&self, key: &str) -> SessionEntry {
        self.host.rows.borrow()[key].clone()
    }

    fn calls(&self) -> Vec<String> {
        self.host.hooks.calls()
    }

    /// Takes the nth `TurnStart` `run_turn` was handed.
    fn take_turn(&self, n: usize) -> TurnStart {
        self.turns.borrow_mut().remove(n)
    }
}

fn prefs() -> Preferences {
    serde_json::from_value(json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false }))
        .expect("preferences the fixture builds")
}

/// The state sessions_start leaves for a first send on thread `t1` of workspace `w1`.
fn prepared(r: &Rig) -> Prepared {
    Prepared {
        workspace_id: "w1".to_string(),
        origin: None,
        opts: StartOptions { prompt: "fix it".to_string(), harness: Some("claude".to_string()), request_id: Some("req_1".to_string()), ..Default::default() },
        prefs: prefs(),
        plan: None,
        thread_id: "t1".to_string(),
        opens: true,
        entry: r.entry.clone(),
        replaces: None,
        resume: None,
        gone_now: None,
        title: None,
        tree: Tree::default(),
        asked: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        notice: None,
        clouds: None,
        cleared: true,
        turn_token: "tok-1".to_string(),
        launch: ThreadLaunch::default(),
        built: BuiltAdapter { harness: "claude".to_string(), adapter: r.adapter.clone() },
        fork_turn: None,
        records: Vec::new(),
        mcp_servers: None,
        waiting: Rc::new(Cell::new(false)),
    }
}

/// A send into a thread that has run: not an opening, and the thread's turn is running.
fn send_into_running(_r: &Rig, p: &mut Prepared) {
    p.opens = false;
    p.cleared = false;
    p.opts.thread = Some("t1".to_string());
}

fn held_view(extra: Value) -> SessionView {
    view("turn-1", "w1", merged(json!({ "threadId": "t1", "startedBy": "person", "prompt": "fix it", "startedAt": T0 }), extra))
}

fn held_event() -> Value {
    json!({ "type": "session.held", "workspaceId": "w1", "threadId": "t1", "requestId": "req_1" })
}

fn queued_event(prompt: &str, request_id: &str) -> Value {
    json!({ "type": "session.queued", "workspaceId": "w1", "threadId": "t1", "harness": "claude", "prompt": prompt, "requestId": request_id })
}

fn starting_event() -> Value {
    json!({ "type": "session.starting", "workspaceId": "w1", "threadId": "t1", "harness": "claude", "requestId": "req_1" })
}

fn end_event(reason: &str) -> Value {
    json!({ "type": "session.end", "workspaceId": "w1", "sessionId": "turn-1", "turnId": "turn-1", "threadId": "t1", "exitCode": null, "sawResult": false, "reason": reason, "at": T0 })
}

fn scope(thread: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": thread })).expect("a thread scope")
}

fn caller_in(thread: &str) -> Caller {
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by: scope(thread) }
}

fn png() -> Attachment {
    Attachment { media_type: "image/png".to_string(), bytes: "iVBORw0=".to_string(), name: None }
}

fn pdf() -> Attachment {
    Attachment { media_type: "application/pdf".to_string(), bytes: "JVBERi0=".to_string(), name: Some("report.pdf".to_string()) }
}

fn record_of(media_type: &str, bytes: u32) -> AttachmentRecord {
    serde_json::from_value(json!({ "mediaType": media_type, "bytes": bytes })).expect("an attachment record")
}

fn forward() -> Forward {
    Rc::new(|_e: AdapterEvent| {})
}

/// Opens the turn the way `run_turn` does, on the forward that drops every event.
fn open(t: TurnStart) {
    match (t.open)(forward()) {
        Ok(_) => {}
        Err(e) => panic!("the turn was to open, and said: {}", e.message),
    }
}

/// Holds the landing of images until the sender fires, answering nothing landed.
fn stall_landing(r: &Rig) -> oneshot::Sender<()> {
    let (tx, rx) = oneshot::channel::<()>();
    let rx = Rc::new(RefCell::new(Some(rx)));
    r.host.hooks.set::<dyn Fn(&Entry, &dyn HarnessAdapter, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<Landed, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(move |_e: &Entry, _a: &dyn HarnessAdapter, _landing: &str, _dir: &str, _x: Vec<Attachment>| {
            let rx = rx.borrow_mut().take();
            async move {
                if let Some(rx) = rx {
                    let _ = rx.await;
                }
                Ok(Landed::default())
            }
            .boxed_local()
        }),
    );
    tx
}

/// Answers each landing with these and records what each was asked: the images' and the files'.
fn record_landings(r: &Rig, images: Vec<TurnImage>, files: Vec<&str>) -> (Landings, Landings) {
    let (imgs, fls): (Landings, Landings) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let i = imgs.clone();
    r.host.hooks.set::<dyn Fn(&Entry, &dyn HarnessAdapter, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<Landed, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(move |_e: &Entry, _a: &dyn HarnessAdapter, landing: &str, dir: &str, x: Vec<Attachment>| {
            i.borrow_mut().push((landing.to_string(), dir.to_string(), x));
            let landed = if images.is_empty() { Landed::default() } else { Landed { images: images.clone(), dir: Some(dir.to_string()) } };
            async move { Ok(landed) }.boxed_local()
        }),
    );
    let f = fls.clone();
    let files: Vec<String> = files.iter().map(|s| s.to_string()).collect();
    r.host.hooks.set::<dyn Fn(&Entry, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<Vec<String>, HostError>>>(
        "SessionsCtx::land_files",
        Rc::new(move |_e: &Entry, landing: &str, dir: &str, x: Vec<Attachment>| {
            f.borrow_mut().push((landing.to_string(), dir.to_string(), x));
            let files = files.clone();
            async move { Ok(files) }.boxed_local()
        }),
    );
    (imgs, fls)
}

/// `run_turn` answers a refusal in these words.
fn run_turn_fails(r: &Rig, message: &str) {
    let message = message.to_string();
    r.host.hooks.set::<dyn Fn(TurnStart) -> Result<Rc<dyn SessionHandle>, HostError>>(
        "SessionsCtx::run_turn",
        Rc::new(move |_t: TurnStart| Err(HostError::new(message.clone()))),
    );
}

fn thread_record_of(r: &Rig, thread: &str) -> HeldThread {
    r.host.records.borrow()[thread].clone()
}

// ===============================================================================================================
// L1, L3: the row, held before the machine is asked

// L1, L3: session-held.test.ts:15
#[test]
fn the_row_is_held_before_the_machine_is_asked() {
    run_local(async {
        let r = rig();
        let handle = ok(r.run(prepared(&r)).await);
        let seen = r.seen.borrow();
        assert_eq!(seen.len(), 1, "the agent's lists were asked once");
        let first = &seen[0];
        assert_eq!(first.rows.len(), 1);
        let (key, row, v) = &first.rows[0];
        assert_eq!(key, "turn-1");
        assert_eq!(row.turn_id, "turn-1");
        assert_eq!(*v, held_view(json!({})));
        assert!(row.launch.is_some(), "the row carries the launch others wait on");
        assert!(row.handle.is_none());
        assert_eq!((row.replaces.clone(), row.notify.clone(), row.notify_by.clone(), row.notify_road), (None, None, None, None));
        assert_eq!(first.emitted, vec![held_event()], "every subscriber heard the thread held, with the request it answers");
        assert!(at(&first.calls, "cap_hold") < at(&first.calls, "catalog_on"), "the slot is taken before the machine is asked");
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, None)".to_string()]);
        // The row's view is the turn's view: one object the turn writes through.
        assert!(Rc::ptr_eq(&r.row("turn-1").view, &r.turns.borrow()[0].view));
        assert_eq!(handle.id(), "turn-1");
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
    });
}

// L3: no TS test (the TS spreads replaces, notify, notifyBy and notifyRoad onto the held row)
#[test]
fn the_held_row_carries_who_it_replaces_and_who_it_tells() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        p.notify = Some(vec!["lead".to_string()]);
        p.notify_by = Some(scope("lead"));
        p.notify_road = Some(WorkspaceOrigin::Relayed);
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>("LaunchCtx::preferences", Rc::new(|| async { Ok(prefs()) }.boxed_local()));
        ok(r.run(p).await);
        let seen = r.seen.borrow();
        let (_, row, _) = &seen[0].rows[0];
        assert_eq!(row.replaces, Some("thr_old".to_string()));
        assert_eq!(row.notify, Some(vec!["lead".to_string()]));
        assert_eq!(row.notify_by, Some(scope("lead")));
        assert_eq!(row.notify_road, Some(WorkspaceOrigin::Relayed));
    });
}

// L1: session-opening.test.ts:42 (a row keeps the head of the opening, the transcript keeps it whole)
#[test]
fn the_row_keeps_the_head_of_a_long_opening_and_the_turn_the_whole() {
    run_local(async {
        let r = rig();
        let opening = format!("Build the ticket, and say what you find. {}", "Read every comment first, then the code it names. ".repeat(700));
        let mut p = prepared(&r);
        p.opts.prompt = opening.clone();
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        let kept = held.prompt.clone().expect("the row has a prompt");
        assert_eq!(kept, kept_opening(&opening));
        assert!(kept.len() <= OPENING_KEPT && opening.starts_with(&kept));
        let t = r.take_turn(0);
        assert_eq!(t.opening.prompt, opening, "the transcript's start keeps the message whole");
        assert_eq!(t.asked.as_ref().unwrap().borrow().prompt, opening);
    });
}

// L1: no TS test (the TS view literal)
#[test]
fn the_held_view_carries_what_the_start_knew() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.started_by = Some(SessionOrigin::Cli);
        p.opts.attempt = Some("2".to_string());
        p.opts.context_window = Some("1m".to_string());
        p.tree = Tree { parent_thread_id: Some("lead".to_string()), root_thread_id: Some("lead".to_string()) };
        p.opens = false;
        p.resume = Some("sess-0".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        let expected = held_view(json!({ "startedBy": "cli", "attempt": "2", "contextWindow": "1m", "parentThreadId": "lead", "rootThreadId": "lead", "claudeSessionId": "sess-0" }));
        assert_eq!(held, expected);
    });
}

// L1: a person's title stands as the person's
#[test]
fn a_persons_title_is_the_rows_title() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.title = Some("Landing fix".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(held, held_view(json!({ "harnessTitle": "Landing fix", "titleSource": "person" })));
        assert_eq!(r.take_turn(0).opening.title, Some("Landing fix".to_string()));
    });
}

// L1, L17: an outside conversation's own name stands as a generated one
#[test]
fn an_outside_conversations_name_stands_as_an_auto_title() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening { harness: "claude".to_string(), id: "out-1".to_string(), copy: false, project: "p1".to_string(), cwd: None, title: "  An outside talk\n".to_string(), earlier: Vec::new() });
        p.resume = Some("out-1".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(held, held_view(json!({ "harnessTitle": title_line("  An outside talk\n"), "titleSource": "auto", "claudeSessionId": "out-1" })));
    });
}

// L1: no title of its own, the one the thread carries
#[test]
fn a_thread_that_has_a_title_keeps_it_on_the_new_row() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> (Option<String>, Option<TitleSource>)>(
            "SessionsCtx::carried_title",
            Rc::new(|_t: &str| (Some("Earlier name".to_string()), Some(TitleSource::Person))),
        );
        ok(r.run(prepared(&r)).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(held, held_view(json!({ "harnessTitle": "Earlier name", "titleSource": "person" })));
        assert_eq!(r.host.hooks.called("carried_title"), vec!["carried_title(t1)".to_string()]);
    });
}

// L3: runtime-steer.test.ts:539 (a send into a thread whose turn runs holds no row of its own until the thread is free)
#[test]
fn a_send_into_a_running_thread_holds_no_second_row() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.opts.prompt = "two".to_string();
        p.opts.request_id = Some("req_2".to_string());
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()], "nothing held while the thread runs");
        assert_eq!(r.host.hooks.called("cap_hold"), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![queued_event("two", "req_2")], "the wait is said at once, and no held is");
        assert!(r.turns.borrow().is_empty());
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(r.row_keys(), vec!["sess-0".to_string(), "turn-1".to_string()]);
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, None)".to_string()]);
        // The thread's own session is what the turn resumes: read off its latest row once the thread was this send's.
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-old".to_string()));
        assert_eq!(t.outcome, SessionStartOutcome::Queued);
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.emitted(), vec![queued_event("two", "req_2"), held_event()], "held after the wait, queued said once");
        assert_eq!(t.view.borrow().claude_session_id, Some("sess-old".to_string()));
    });
}

// L3, L20: the launch settles once the handover is done
#[test]
fn the_launch_others_wait_on_settles_at_the_handover() {
    run_local(async {
        let r = rig();
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        let launch = r.row("turn-1").launch.expect("a launch");
        assert!(launch.clone().now_or_never().is_none(), "the launch is open while the machine is asked");
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert!(launch.now_or_never().is_some(), "the launch settled");
    });
}

// ===============================================================================================================
// L4, L19: saying the agent is starting

// L4: first-run.test.ts:128 (host package)
#[test]
fn starting_is_said_once_after_four_seconds_with_what_the_first_run_installs() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<bool, HostError>>>("SessionsCtx::first_run_here", Rc::new(|_e: &Entry, _h: &str| async { Ok(true) }.boxed_local()));
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64 - 1);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event()], "nothing before the time");
        r.host.clock.advance(1);
        settle().await;
        let mut starting = starting_event();
        starting["installs"] = json!(true);
        assert_eq!(r.emitted(), vec![held_event(), starting.clone()]);
        assert_eq!(r.host.hooks.called("first_run_here"), vec!["first_run_here(claude)".to_string()]);
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.emitted(), vec![held_event(), starting], "the handover says nothing a second time");
    });
}

// L4: first-run.test.ts:173 (an agent that is not a wrapper says no installs)
#[test]
fn the_starting_line_names_no_install_where_there_is_none() {
    run_local(async {
        let r = rig();
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), starting_event()]);
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
    });
}

// L4: the turn's own session.start hushes it
#[test]
fn a_session_start_of_this_turn_hushes_the_starting_line() {
    run_local(async {
        let r = rig();
        let listeners: Rc<RefCell<Vec<(String, Rc<dyn Fn()>)>>> = Rc::new(RefCell::new(Vec::new()));
        let offs = Rc::new(Cell::new(0u32));
        let (l, o) = (listeners.clone(), offs.clone());
        r.host.hooks.set::<dyn Fn(&str, Rc<dyn Fn()>) -> Rc<dyn Fn()>>(
            "SessionsCtx::on_turn_settled",
            Rc::new(move |turn_id: &str, f: Rc<dyn Fn()>| {
                l.borrow_mut().push((turn_id.to_string(), f));
                let o = o.clone();
                Rc::new(move || o.set(o.get() + 1)) as Rc<dyn Fn()>
            }),
        );
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        assert_eq!(listeners.borrow().len(), 1);
        assert_eq!(listeners.borrow()[0].0, "turn-1");
        assert_eq!(r.host.clock.pending(), 1, "the four-second timer");
        let f = listeners.borrow()[0].1.clone();
        f();
        assert_eq!(offs.get(), 1, "the listener is taken off");
        assert_eq!(r.host.clock.pending(), 0, "the timer is cancelled");
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.emitted(), vec![held_event()]);
    });
}

// L19: the handover says the agent is starting at once, off a new process
#[test]
fn a_new_process_says_starting_at_the_handover() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), starting_event()]);
        assert_eq!(r.host.clock.pending(), 0, "the timer is cancelled once it is said");
    });
}

// L4, L7: a send that queues says nothing of starting
#[test]
fn a_queued_send_never_says_starting() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        ok(join.await.expect("the launch ran"));
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1"), held_event()]);
    });
}

// ===============================================================================================================
// L5: before the loop

// L5, Z1-Z5: no TS test (a local workspace whose named folder is gone)
#[test]
fn a_start_that_fails_before_its_launch_leaves_no_row_and_ends_the_wait() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut p = prepared(&r);
        p.opts.cwd = Some("/gone/dir".to_string());
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let join = r.spawn(p);
        settle().await;
        assert!(r.seen.borrow().is_empty(), "the agent's lists were never asked: the folder fails first");
        let e = err(join.await.expect("the launch ran"));
        let line = no_cwd_line(&home_shortened("/gone/dir", None));
        assert_eq!(said(&e), (line.clone(), None, Some(Kind::Usage)));
        assert_eq!(r.row_keys(), Vec::<String>::new(), "no row is left");
        assert_eq!(r.emitted(), vec![held_event(), end_event(&line)], "the wait ends on the bus");
        assert_eq!(r.host.recorded_json(), Vec::<Value>::new(), "no end with no start in the transcript");
        assert_eq!(r.host.clock.pending(), 0);
        assert!(r.host.hooks.called("cap_left").iter().all(|c| c == "cap_left(turn-1)") && !r.host.hooks.called("cap_left").is_empty());
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.host.hooks.called("drop_images"), Vec::<String>::new());
        assert!(r.turns.borrow().is_empty());
    });
}

// Z3, Z4: no TS test (a send waiting on a launch finds the thread as it now is)
#[test]
fn a_send_waiting_on_a_start_that_fails_goes_on_after_the_row_is_gone() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let (tx, rx) = oneshot::channel::<()>();
        let rx = Rc::new(RefCell::new(Some(rx)));
        r.host.hooks.set::<dyn Fn(&Entry, Option<&str>) -> LocalBoxFuture<'static, Result<String, HostError>>>(
            "LaunchCtx::thread_folder",
            Rc::new(move |_e: &Entry, cwd: Option<&str>| {
                let rx = rx.borrow_mut().take();
                let folder = cwd.map(String::from).unwrap_or_else(|| "/work/w1".to_string());
                async move {
                    if let Some(rx) = rx {
                        let _ = rx.await;
                    }
                    Ok(folder)
                }
                .boxed_local()
            }),
        );
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut a = prepared(&r);
        a.opts.cwd = Some("/gone/dir".to_string());
        let first = r.spawn(a);
        settle().await;
        let mut b = prepared(&r);
        b.opens = false;
        b.cleared = false;
        b.opts.thread = Some("t1".to_string());
        b.opts.prompt = "second".to_string();
        b.opts.request_id = Some("req_2".to_string());
        let second = r.spawn(b);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), queued_event("second", "req_2")], "the second waits on the first's launch");
        tx.send(()).unwrap();
        settle().await;
        let line = no_cwd_line(&home_shortened("/gone/dir", None));
        assert_eq!(err(first.await.expect("the launch ran")).message, line);
        let handle = ok(second.await.expect("the launch ran"));
        assert_eq!(handle.turn_id(), "turn-2");
        let events = r.emitted();
        assert_eq!(events[2], end_event(&line), "the first's end is out before the second holds its row");
        assert_eq!(events[3], json!({ "type": "session.held", "workspaceId": "w1", "threadId": "t1", "requestId": "req_2" }));
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Queued);
    });
}

// L5: the cwd check reads this computer's disk only for a local workspace
#[test]
fn a_cloud_workspace_does_not_look_for_the_folder_on_this_disk() {
    run_local(async {
        let r = rig();
        let cloud = r.host.add_entry(entry("w2", json!({ "kind": "cloud" })));
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut p = prepared(&r);
        p.workspace_id = "w2".to_string();
        p.entry = cloud;
        p.opts.cwd = Some("/gone/dir".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("folder_exists"), Vec::<String>::new());
        assert_eq!(r.take_turn(0).turn_id, "turn-1");
    });
}

// L5: a pick the lists do not carry is refused before this send waits on anything
#[test]
fn a_refused_pick_fails_the_start_before_anything_lands() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([{ "value": "haiku", "label": "Haiku" }]));
        r.host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, Option<&str>) -> Option<HostError>>(
            "LaunchCtx::not_run_refused",
            Rc::new(|_c: Option<&HarnessCatalog>, _m: Option<&str>| Some(HostError::with_kind("no-such has not run on this computer", Kind::Usage))),
        );
        let mut p = prepared(&r);
        p.opts.model = Some("no-such".to_string());
        let e = err(r.run(p).await);
        assert_eq!(said(&e), ("no-such has not run on this computer".to_string(), None, Some(Kind::Usage)));
        assert_eq!(r.host.hooks.called("thread_folder"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event("no-such has not run on this computer")]);
        assert!(r.adapter.starts.lock().unwrap().is_empty());
    });
}

// ===============================================================================================================
// L6: a stop's end owed to a computer that is away

fn owed_hooks(r: &Rig) -> oneshot::Sender<()> {
    let (tx, paid) = gate();
    r.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<Option<Option<Joined>>, HostError>>>(
        "SessionsCtx::end_owed",
        Rc::new(move |_e: &Entry, _t: &str| {
            let paid = paid.clone();
            async move { Ok(Some(Some(paid))) }.boxed_local()
        }),
    );
    r.host.hooks.set::<dyn Fn(&WorkspaceRecord) -> Option<String>>("SessionsCtx::workspace_place", Rc::new(|_r: &WorkspaceRecord| Some("place-1".to_string())));
    r.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::place_name_of", Rc::new(|_p: &str| "acme's laptop".to_string()));
    tx
}

// L6: owed-box.test.ts:41
#[test]
fn a_send_behind_a_stops_owed_end_waits_and_says_the_computer() {
    run_local(async {
        let r = rig();
        let paid = owed_hooks(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        assert!(r.area.stop_holds("t1"), "the owed end holds the thread's next turn");
        let mut queued = queued_event("fix it", "req_1");
        queued["waitsFor"] = json!("acme's laptop");
        assert_eq!(r.emitted(), vec![held_event(), queued], "held first, then the wait said once with the computer");
        assert_eq!(r.row("turn-1").view.borrow().waits_for, Some("acme's laptop".to_string()), "the row says what it waits for");
        assert_eq!(r.host.hooks.called("cap_hold").len(), 1);
        assert!(r.turns.borrow().is_empty());
        paid.send(()).unwrap();
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        let t = r.take_turn(0);
        assert_eq!(t.outcome, SessionStartOutcome::Queued);
        assert_eq!(t.view.borrow().waits_for, None, "the row's wait is taken off");
        assert!(!r.area.stop_holds("t1"), "the hold is forgotten once its gate settles");
        assert_eq!(r.emitted_of("session.queued").len(), 1);
    });
}

// L6, K5: owed-box.test.ts:41 (a stop on the waiting send gives it up)
#[test]
fn a_stop_gives_up_a_send_waiting_on_an_owed_end() {
    run_local(async {
        let r = rig();
        let _paid = owed_hooks(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        let stopped = r.area.interrupt_row("turn-1", None, None).await.expect("the stop answers");
        let line = send_given_up_line("acme's laptop");
        assert_eq!(stopped, SessionInterruptResult { outcome: SessionInterruptOutcome::Accepted, under: None, error: None, left: Some(line.clone()) });
        settle().await;
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(said(&e), (line.clone(), None, None));
        assert_eq!(r.row_keys(), Vec::<String>::new());
        let events = r.emitted();
        assert_eq!(events.last().cloned(), Some(end_event(&line)));
        assert!(r.turns.borrow().is_empty());
    });
}

// ===============================================================================================================
// L7: waiting for another send's launch

// L7: runtime-steer.test.ts:570
#[test]
fn two_sends_queued_behind_one_turn_run_in_order_and_each_says_so_once() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut a = prepared(&r);
        send_into_running(&r, &mut a);
        a.opts.prompt = "two".to_string();
        a.opts.request_id = Some("req_2".to_string());
        let mut b = prepared(&r);
        send_into_running(&r, &mut b);
        b.opts.prompt = "three".to_string();
        b.opts.request_id = Some("req_3".to_string());
        let (ja, jb) = (r.spawn(a), r.spawn(b));
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("two", "req_2"), queued_event("three", "req_3")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let first = ok(ja.await.expect("the launch ran"));
        assert_eq!(first.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.turns.borrow().len(), 1, "the second send waits for the first's turn");
        assert_eq!(r.turns.borrow()[0].opening.prompt, "two");
        let h1 = r.handles.borrow()[0].clone();
        end_turn(&r.host, "turn-1", &h1);
        settle().await;
        let second = ok(jb.await.expect("the launch ran"));
        assert_eq!(second.outcome(), SessionStartOutcome::Queued);
        let prompts: Vec<String> = r.turns.borrow().iter().map(|t| t.opening.prompt.clone()).collect();
        assert_eq!(prompts, vec!["two".to_string(), "three".to_string()]);
        let held: Vec<Value> = r.emitted_of("session.held");
        assert_eq!(held.iter().map(|e| e["requestId"].clone()).collect::<Vec<_>>(), vec![json!("req_2"), json!("req_3")]);
        assert_eq!(r.emitted_of("session.queued").len(), 2, "the second wait of the second send is not said again");
    });
}

// L7: another send's turn still launching is waited for, and said once
#[test]
fn a_send_waits_for_the_launch_of_a_turn_ahead_of_it() {
    run_local(async {
        let r = rig();
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        assert_eq!(r.row_keys(), vec!["turn-0".to_string()]);
        // The turn ahead is up: its row has its handle. This send now waits behind a running turn and says nothing more.
        let v = r.host.rows.borrow()["turn-0"].view.clone();
        let h0 = Rc::new(Handle::new("turn-0", SessionStartOutcome::Started, v));
        r.host.rows.borrow_mut().get_mut("turn-0").unwrap().handle = Some(h0.clone() as Rc<dyn SessionHandle>);
        open_launch.send(()).unwrap();
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        assert!(r.turns.borrow().is_empty());
        end_turn(&r.host, "turn-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1"), held_event()]);
    });
}

// L7: a steering harness waiting on a launching turn stays started and says nothing
#[test]
fn a_steering_harness_waiting_on_a_launching_turn_stays_started() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.emitted(), Vec::<Value>::new());
        r.host.rows.borrow()["turn-0"].view.borrow_mut().status = serde_json::from_value(json!("completed")).unwrap();
        open_launch.send(()).unwrap();
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Started);
        assert_eq!(r.emitted_of("session.queued"), Vec::<Value>::new());
    });
}

// L3, L7: thread-cap.test.ts:78 (a turn a running thread waits on runs in that thread's slot)
#[test]
fn the_slot_of_a_thread_waiting_on_this_send_is_lent_to_it() {
    run_local(async {
        async fn lender(origin: Option<Caller>, on_held: bool, followed: Option<bool>) -> String {
            let r = rig();
            add_running(&r.host, "lead-sess", "lead-turn", "lead", json!({}), |_| {});
            let mut p = prepared(&r);
            p.origin = origin;
            if on_held {
                p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
            }
            p.opts.followed = followed;
            ok(r.run(p).await);
            r.host.hooks.called("cap_hold").remove(0)
        }
        assert_eq!(lender(Some(caller_in("lead")), false, None).await, "cap_hold(turn-1, Some(\"lead-turn\"))", "a start the caller waits on");
        assert_eq!(lender(Some(caller_in("lead")), true, Some(true)).await, "cap_hold(turn-1, Some(\"lead-turn\"))", "a follow");
        assert_eq!(lender(Some(caller_in("lead")), true, None).await, "cap_hold(turn-1, None)", "answered held: nobody waits on it");
        assert_eq!(lender(Some(caller_in("t1")), false, None).await, "cap_hold(turn-1, None)", "a thread never lends itself");
        assert_eq!(lender(None, false, None).await, "cap_hold(turn-1, None)", "a person lends nothing");
    });
}

// L7: the lender's slot goes to the turn ahead too
#[test]
fn a_send_waiting_on_a_launching_turn_lends_its_slot_to_that_turn() {
    run_local(async {
        let r = rig();
        add_running(&r.host, "lead-sess", "lead-turn", "lead", json!({}), |_| {});
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.origin = Some(caller_in("lead"));
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.host.hooks.called("cap_lend"), vec!["cap_lend(turn-0, lead-turn)".to_string()]);
        r.host.rows.borrow()["turn-0"].view.borrow_mut().status = serde_json::from_value(json!("completed")).unwrap();
        open_launch.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, Some(\"lead-turn\"))".to_string()]);
    });
}

// L8: no TS test (a workspace deleted during the wait fails the start in its words)
#[test]
fn a_workspace_deleted_during_a_wait_fails_the_start_in_its_words() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let join = r.spawn(p);
        settle().await;
        r.host.entries.borrow_mut().shift_remove("w1");
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(e.message, deleted_before_start_line("w1"));
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()], "a send that never held its row leaves none");
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")], "and says no end of a turn that never was");
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert!(r.turns.borrow().is_empty());
    });
}

// L8: the checks run again after the wait: the copy's block, the mounts and the kept process
#[test]
fn the_checks_run_once_for_a_free_thread_and_again_after_a_wait() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("copy_blocked"), Vec::<String>::new(), "cleared: the start made them already");
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        ok(join.await.expect("the launch ran"));
        let calls = r.calls();
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1);
        assert_eq!(r.host.hooks.called("worktree_mounted").len(), 1);
        assert_eq!(r.host.hooks.called("kept_check"), vec!["kept_check(t1)".to_string()]);
        assert!(at(&calls, "copy_blocked") < at(&calls, "worktree_mounted") && at(&calls, "worktree_mounted") < at(&calls, "kept_check"));
    });
}

// ===============================================================================================================
// L9: a host write in flight on the resumed session

// L9
#[test]
fn a_host_write_in_flight_on_the_resumed_session_is_waited_out() {
    run_local(async {
        let r = rig();
        let (done_writing, writing) = gate();
        let asked = Rc::new(Cell::new(0u32));
        let a = asked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<Joined>>(
            "SessionsCtx::host_writes",
            Rc::new(move |_s: &str| {
                a.set(a.get() + 1);
                if a.get() == 1 { Some(writing.clone()) } else { None }
            }),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        let join = r.spawn(p);
        settle().await;
        assert!(r.turns.borrow().is_empty(), "nothing launches over the write");
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
        done_writing.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("host_writes"), vec!["host_writes(sess-1)".to_string(), "host_writes(sess-1)".to_string()]);
        assert_eq!(r.take_turn(0).resume, Some("sess-1".to_string()));
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1, "the checks run again after a wait");
    });
}

// ===============================================================================================================
// L10: a full computer

fn ask_of(resume: Option<&str>) -> CapWaitAsk {
    CapWaitAsk {
        workspace_id: "w1".to_string(),
        thread_id: "t1".to_string(),
        turn_id: "turn-1".to_string(),
        session_id: resume.unwrap_or("turn-1").to_string(),
        prompt: "fix it".to_string(),
        request_id: Some("req_1".to_string()),
    }
}

fn full_wait() -> ThreadCapWait {
    serde_json::from_value(json!({ "placeId": "here", "place": "mac", "running": 1, "atOnce": 1 })).expect("a cap wait")
}

// L10: thread-cap.test.ts:111
#[test]
fn a_full_computer_holds_the_turn_and_tells_the_caller_once() {
    run_local(async {
        let r = rig();
        let (open_one, one) = gate();
        let (open_two, two) = gate();
        let gates = Rc::new(RefCell::new(vec![one, two]));
        let asks: Rc<RefCell<Vec<(CapWaitAsk, ThreadCapWait)>>> = Rc::new(RefCell::new(Vec::new()));
        let looks = Rc::new(Cell::new(0u32));
        let (l, w) = (looks.clone(), full_wait());
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
            "SessionsCtx::cap_full",
            Rc::new(move |_r: &WorkspaceRecord, _t: &str| {
                l.set(l.get() + 1);
                if l.get() <= 2 { Some(w.clone()) } else { None }
            }),
        );
        let (g, a) = (gates.clone(), asks.clone());
        r.host.hooks.set::<dyn Fn(CapWaitAsk, ThreadCapWait) -> Joined>(
            "SessionsCtx::cap_wait",
            Rc::new(move |ask: CapWaitAsk, full: ThreadCapWait| {
                a.borrow_mut().push((ask, full));
                g.borrow_mut().remove(0)
            }),
        );
        let told: Rc<RefCell<Vec<HeldStart>>> = Rc::new(RefCell::new(Vec::new()));
        let t = told.clone();
        let mut p = prepared(&r);
        p.opts.on_held = Some(Rc::new(move |h: HeldStart| t.borrow_mut().push(h)));
        let join = r.spawn(p);
        settle().await;
        assert_eq!(*asks.borrow(), vec![(ask_of(None), full_wait())]);
        assert_eq!(*told.borrow(), vec![HeldStart { view: held_view(json!({ "capped": serde_json::to_value(full_wait()).unwrap() })), turn_id: "turn-1".to_string(), wait: full_wait() }]);
        assert_eq!(r.host.clock.pending(), 0, "a held turn is hushed: no starting line comes out of it");
        assert_eq!(r.host.hooks.called("cap_left"), Vec::<String>::new());
        assert!(r.turns.borrow().is_empty());
        assert_eq!(r.emitted(), vec![held_event()]);
        open_one.send(()).unwrap();
        settle().await;
        assert_eq!(asks.borrow().len(), 2, "it looks again and waits again");
        assert_eq!(told.borrow().len(), 1, "the caller is told once");
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1, "the checks run again after a wait");
        open_two.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()], "let through once");
        assert_eq!(r.host.hooks.called("cap_full").len(), 3);
        assert!(at(&r.calls(), "cap_left") < at(&r.calls(), "land_images"), "the slot is taken before anything lands");
        assert_eq!(r.emitted(), vec![held_event()], "a hushed start says nothing of starting");
    });
}

// L10: a send that wakes its lead is let through a full computer
#[test]
fn a_send_that_wakes_its_lead_is_never_held_by_a_full_computer() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>("SessionsCtx::cap_full", Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())));
        let mut p = prepared(&r);
        p.opts.wakes_lead = true;
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("cap_full"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()]);
        assert_eq!(r.take_turn(0).turn_id, "turn-1");
    });
}

// L10: stop marks pending are waited for, then the look is made again
#[test]
fn stops_pending_on_the_turn_are_waited_for_before_anything_else() {
    run_local(async {
        let r = rig();
        let (done, stop) = gate();
        let looked = Rc::new(Cell::new(0u32));
        let l = looked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
            "SessionsCtx::cap_held",
            Rc::new(move |_t: &str| {
                l.set(l.get() + 1);
                if l.get() == 1 { Some(CapHeld { stops: vec![stop.clone()], ..Default::default() }) } else { None }
            }),
        );
        let join = r.spawn(prepared(&r));
        settle().await;
        assert_eq!(r.host.hooks.called("cap_full"), Vec::<String>::new(), "a mark is read before the computer is");
        assert!(r.turns.borrow().is_empty());
        done.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1, "every check again");
        assert_eq!(r.host.hooks.called("cap_full").len(), 1);
    });
}

// L10, Z1-Z3: thread-cap.test.ts:583 (a held start that is stopped fails with the stop's words)
#[test]
fn a_stop_mark_fails_the_start_with_its_words_and_it_never_launches() {
    run_local(async {
        let r = rig();
        let stopped = "stopped before it started, while mac was full";
        let looked = Rc::new(Cell::new(0u32));
        let l = looked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
            "SessionsCtx::cap_held",
            Rc::new(move |_t: &str| {
                l.set(l.get() + 1);
                if l.get() >= 2 { Some(CapHeld { stopped: Some(stopped.to_string()), ..Default::default() }) } else { None }
            }),
        );
        let mut p = prepared(&r);
        p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>("SessionsCtx::cap_full", Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())));
        let e = err(r.run(p).await);
        assert_eq!(said(&e), (stopped.to_string(), None, None));
        assert!(r.turns.borrow().is_empty());
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event(stopped)]);
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
    });
}

// ===============================================================================================================
// L11: the landing

// L11, L18: session-images.test.ts:289 (an image on the file road lands in this send's own folder)
#[test]
fn images_land_in_a_folder_of_their_own_per_send() {
    run_local(async {
        let r = rig_with(adapter(|a| a.attachments = Some(AttachmentRoad::File)));
        let folder = "/work/w1";
        let dir = turn_images_dir(folder, "t1", Some("req_1"), "minted-1");
        let sent = format!("{dir}/1.png");
        let landed_image = TurnImage { media_type: "image/png".to_string(), bytes: "iVBORw0=".to_string(), path: Some(sent.clone()) };
        let files_dir = send_files_dir(folder, "t1", Some("req_1"), "minted-1");
        let file_path = format!("{files_dir}/report.pdf");
        let (images, files) = record_landings(&r, vec![landed_image.clone()], vec![file_path.as_str()]);
        let mut p = prepared(&r);
        p.opts.prompt = "what is the first heading?".to_string();
        p.opts.attachments = Some(vec![png(), pdf()]);
        p.records = vec![record_of("image/png", 5), record_of("application/pdf", 5)];
        ok(r.run(p).await);
        assert_eq!(*images.borrow(), vec![(folder.to_string(), dir.clone(), vec![png()])], "the images only, in a folder keyed by thread, request and the send's own id");
        assert_eq!(*files.borrow(), vec![(folder.to_string(), files_dir, vec![pdf()])], "the other files only");
        let t = r.take_turn(0);
        assert_eq!(t.images_dir, Some(dir));
        let handed = attached_files_prompt("what is the first heading?", &[file_path.as_str()]);
        assert_ne!(handed, "what is the first heading?");
        let asked = t.asked.as_ref().unwrap().borrow().clone();
        assert_eq!(asked, TurnAsked { prompt: handed.clone(), effort: None, typed: Some("what is the first heading?".to_string()), request_id: Some("req_1".to_string()), owed: None, awaits_prompt: None });
        assert_eq!(t.opening.prompt, "what is the first heading?", "the transcript keeps the words the person typed");
        assert_eq!(t.opening.attachments, Some(vec![record_of("image/png", 5), record_of("application/pdf", 5)]));
        assert_eq!(r.host.hooks.called("cap_left").len(), 1);
        assert!(at(&r.calls(), "land_images") < at(&r.calls(), "land_files"));
        assert_eq!(r.host.records.borrow()["t1"].record.files_in, Some(vec![folder.to_string()]), "the files folder is kept on the thread");
        assert_eq!(r.row("turn-1").view.borrow().prompt, Some("what is the first heading?".to_string()));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!(starts[0].prompt, handed);
        assert_eq!(starts[0].images, vec![landed_image]);
    });
}

// L11: session-images.test.ts:373 (a message with no image asks the machine for nothing)
#[test]
fn a_message_with_no_attachment_lands_nothing_and_hands_the_adapter_no_image() {
    run_local(async {
        let r = rig();
        let (images, files) = record_landings(&r, Vec::new(), Vec::new());
        ok(r.run(prepared(&r)).await);
        assert_eq!(images.borrow().len(), 1);
        assert_eq!(images.borrow()[0].2, Vec::<Attachment>::new());
        assert_eq!(files.borrow()[0].2, Vec::<Attachment>::new());
        let t = r.take_turn(0);
        assert_eq!(t.images_dir, None);
        let asked = t.asked.as_ref().unwrap().borrow().clone();
        assert_eq!(asked, TurnAsked { prompt: "fix it".to_string(), effort: None, typed: None, request_id: Some("req_1".to_string()), owed: None, awaits_prompt: None });
        assert_eq!(t.opening.attachments, None);
        assert_eq!(r.host.records.borrow()["t1"].record.files_in, None);
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].images, Vec::<TurnImage>::new());
    });
}

// L11: session-images.test.ts:406 (two sends that wake together land in folders of their own)
#[test]
fn two_sends_never_share_a_folder_whatever_their_request_id() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let (images, _files) = record_landings(&r, Vec::new(), Vec::new());
        let mut a = prepared(&r);
        a.opts.request_id = None;
        a.opts.attachments = Some(vec![png()]);
        ok(r.run(a).await);
        let mut b = prepared(&r);
        b.thread_id = "t2".to_string();
        b.opts.request_id = Some("../../etc".to_string());
        b.opts.attachments = Some(vec![png()]);
        ok(r.run(b).await);
        let dirs: Vec<String> = images.borrow().iter().map(|i| i.1.clone()).collect();
        assert_eq!(dirs, vec![turn_images_dir("/work/w1", "t1", None, "minted-1"), turn_images_dir("/work/w1", "t2", Some("../../etc"), "minted-2")]);
        assert_ne!(dirs[0], dirs[1]);
        assert!(!dirs[1].contains(".."), "a request id shaped like a path is not one");
    });
}

// L11, Z5: session-images.test.ts:452 (a send that landed its images and is then refused takes them off the machine)
#[test]
fn images_landed_for_a_start_that_fails_are_taken_off_the_machine() {
    run_local(async {
        let r = rig_with(adapter(|a| a.attachments = Some(AttachmentRoad::File)));
        let dir = turn_images_dir("/work/w1", "t1", Some("req_1"), "minted-1");
        let image = TurnImage { media_type: "image/png".to_string(), bytes: "iVBORw0=".to_string(), path: Some(format!("{dir}/1.png")) };
        record_landings(&r, vec![image], Vec::new());
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![png()]);
        let e = err(r.run(p).await);
        assert_eq!(e.message, "the agent would not start");
        assert_eq!(r.host.hooks.called("drop_images"), vec![format!("drop_images({dir})")]);
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event("the agent would not start")]);
        assert!(r.host.hooks.called("cap_left").iter().all(|c| c == "cap_left(turn-1)"));
        assert_eq!(r.host.hooks.called("keep_sent_images"), Vec::<String>::new(), "nothing is kept for a turn that never ran");
        assert_eq!(r.host.records.borrow().get("t1").cloned(), None, "a launch that never opened leaves no record");
    });
}

// L11: the folder's snapshot is awaited unless the agent takes its prompt late
#[test]
fn the_snapshot_is_awaited_for_an_agent_that_takes_its_prompt_at_once() {
    run_local(async {
        let r = rig();
        let (done, taking) = {
            let (tx, rx) = oneshot::channel::<Option<String>>();
            let taking: Shared<LocalBoxFuture<'static, Option<String>>> = futures::FutureExt::map(rx, |r| r.unwrap_or(None)).boxed_local().shared();
            (tx, taking)
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> Shared<LocalBoxFuture<'static, Option<String>>>>("SessionsCtx::snapshot_of", Rc::new(move |_e: &Entry, _cwd: &str| taking.clone()));
        let join = r.spawn(prepared(&r));
        settle().await;
        assert!(r.turns.borrow().is_empty(), "the launch waits for the checkpoint the turn starts from");
        assert_eq!(r.host.hooks.called("snapshot_of"), vec!["snapshot_of(/work/w1)".to_string()]);
        done.send(Some("abc123".to_string())).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        let t = r.take_turn(0);
        let snap = t.snapshot.clone().expect("a snapshot");
        assert_eq!(snap.cwd, "/work/w1");
        match snap.from {
            SnapshotFrom::Known(from) => assert_eq!(from, Some("abc123".to_string())),
            SnapshotFrom::Taking(_) => panic!("the snapshot was awaited, so it is known"),
        }
        open(t);
        assert!(!r.adapter.starts.lock().unwrap()[0].prompt_after);
    });
}

// L11: no TS test (sessions.ts:699 leaves the snapshot out where none was taken)
#[test]
fn a_snapshot_that_found_nothing_leaves_the_turn_none() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert!(r.take_turn(0).snapshot.is_none());
    });
}

// L11, L18: an agent that takes its prompt late does not wait for the snapshot, and its start waits on it
#[test]
fn a_late_prompt_agent_launches_without_the_snapshot_and_is_handed_it_as_the_wait() {
    run_local(async {
        let r = rig_with(adapter(|a| a.waits_prompt = true));
        let (done, taking) = {
            let (tx, rx) = oneshot::channel::<Option<String>>();
            let taking: Shared<LocalBoxFuture<'static, Option<String>>> = futures::FutureExt::map(rx, |r| r.unwrap_or(None)).boxed_local().shared();
            (tx, taking)
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> Shared<LocalBoxFuture<'static, Option<String>>>>("SessionsCtx::snapshot_of", Rc::new(move |_e: &Entry, _cwd: &str| taking.clone()));
        ok(r.run(prepared(&r)).await);
        let t = r.take_turn(0);
        let snap = t.snapshot.clone().expect("a snapshot");
        assert_eq!(snap.cwd, "/work/w1");
        let taking = match snap.from {
            SnapshotFrom::Taking(s) => s,
            SnapshotFrom::Known(_) => panic!("the snapshot is still being taken"),
        };
        done.send(Some("abc123".to_string())).unwrap();
        assert_eq!(taking.await, Some("abc123".to_string()));
        open(t);
        assert!(r.adapter.starts.lock().unwrap()[0].prompt_after, "the prompt goes after the snapshot");
    });
}

// L11: launch servers are read where the agent takes servers or denies them
#[test]
fn launch_servers_are_read_for_an_agent_that_takes_or_denies_servers() {
    run_local(async {
        let off = vec!["codex-apps".to_string()];
        let values = ServerValues { entries: None, config: Some(IndexMap::from([("mcp_servers.x.url".to_string(), "http://x".to_string())])) };
        for takes in [(true, false), (false, true)] {
            let r = rig_with(adapter(|a| {
                a.mcp = takes.0;
                a.denies = takes.1;
            }));
            let (o, v) = (off.clone(), values.clone());
            r.host.hooks.set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
                "LaunchCtx::launch_servers_for",
                Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _cwd: &str| {
                    let ls = LaunchServers { server_values: Some(v.clone()), off_servers: Some(o.clone()) };
                    async move { Ok(ls) }.boxed_local()
                }),
            );
            ok(r.run(prepared(&r)).await);
            assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
            open(r.take_turn(0));
            let starts = r.adapter.starts.lock().unwrap();
            assert_eq!(starts[0].off_servers, Some(off.clone()));
            assert_eq!(starts[0].server_values, Some(values.clone()));
        }
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("launch_servers_for"), Vec::<String>::new(), "an agent with no servers asks for none");
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].off_servers.clone(), starts[0].server_values.clone()), (None, None));
    });
}

// L11: a send resuming pays a cut owed, and the opening says it came after one
#[test]
fn a_send_resuming_pays_the_cut_owed_and_resumes_the_session_it_made() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&dyn HarnessAdapter, &str, &str, &str) -> LocalBoxFuture<'static, Result<String, HostError>>>(
            "SessionsCtx::pay_cut_owed",
            Rc::new(|_a: &dyn HarnessAdapter, _w: &str, _t: &str, _resume: &str| async { Ok("sess-1b".to_string()) }.boxed_local()),
        );
        r.host.hooks.set::<dyn Fn(&str, &str) -> bool>("SessionsCtx::cut_before", Rc::new(|_w: &str, _t: &str| true));
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("pay_cut_owed"), vec!["pay_cut_owed(w1, t1, sess-1)".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-1b".to_string()));
        assert!(t.opening.after_cut);
        assert_eq!(t.view.borrow().claude_session_id, Some("sess-1b".to_string()));
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].resume, Some("sess-1b".to_string()));
    });
}

// L11: a thread's opening pays no cut, and no cut is said
#[test]
fn an_opening_pays_no_cut_and_a_resume_with_none_owed_says_none() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening { harness: "claude".to_string(), id: "out-1".to_string(), copy: true, project: "p1".to_string(), cwd: None, title: "talk".to_string(), earlier: Vec::new() });
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("pay_cut_owed"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("cut_before"), Vec::<String>::new(), "no session to resume, no cut to read");
        let t = r.take_turn(0);
        assert!(!t.opening.after_cut);
        assert_eq!(t.resume, None);
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].resume.clone(), starts[0].copy), (Some("out-1".to_string()), true), "a copy opens on the outside conversation's own id");
    });
}

// ===============================================================================================================
// L11, L17, Z5: a fork

fn plan() -> ForkPlan {
    ForkPlan {
        has_turn: true,
        source_thread_id: "src".to_string(),
        source_session: Some("src-sess".to_string()),
        forked_from: ForkedFrom { thread_id: "src".to_string(), turn_id: Some("src-turn".to_string()), title: "Source".to_string() },
        raw: json!({ "fork": "plan" }),
    }
}

fn readies_a_copy(r: &Rig, dropped: &Rc<Cell<bool>>, drop_fails: bool) {
    let d = dropped.clone();
    r.host.hooks.set::<dyn Fn(&dyn HarnessAdapter, &str, Value, &str) -> LocalBoxFuture<'static, Result<Readied, HostError>>>(
        "SessionsCtx::ready_fork",
        Rc::new(move |_a: &dyn HarnessAdapter, _s: &str, _turn: Value, _h: &str| {
            let d = d.clone();
            async move {
                let drop: Box<dyn FnOnce() -> LocalBoxFuture<'static, Result<(), HostError>>> = Box::new(move || {
                    d.set(true);
                    async move { if drop_fails { Err(HostError::new("the machine would not answer")) } else { Ok(()) } }.boxed_local()
                });
                Ok(Readied::Resume { resume: "copy-1".to_string(), drop: Some(drop) })
            }
            .boxed_local()
        }),
    );
}

// L11, L17
#[test]
fn a_fork_readies_its_session_on_the_sources_computer_and_records_its_history_first() {
    run_local(async {
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        let readied: Rc<RefCell<Vec<(String, Value, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let rd = readied.clone();
        let d = dropped.clone();
        r.host.hooks.set::<dyn Fn(&dyn HarnessAdapter, &str, Value, &str) -> LocalBoxFuture<'static, Result<Readied, HostError>>>(
            "SessionsCtx::ready_fork",
            Rc::new(move |_a: &dyn HarnessAdapter, s: &str, turn: Value, h: &str| {
                rd.borrow_mut().push((s.to_string(), turn, h.to_string()));
                let d = d.clone();
                async move {
                    let drop: Box<dyn FnOnce() -> LocalBoxFuture<'static, Result<(), HostError>>> = Box::new(move || {
                        d.set(true);
                        async { Ok(()) }.boxed_local()
                    });
                    Ok(Readied::Resume { resume: "copy-1".to_string(), drop: Some(drop) })
                }
                .boxed_local()
            }),
        );
        let copied = Rc::new(RefCell::new(Vec::<String>::new()));
        let c = copied.clone();
        r.host.hooks.set::<dyn Fn(&ForkPlan, &str, &str) -> ()>("SessionsCtx::record_copies", Rc::new(move |p: &ForkPlan, w: &str, t: &str| c.borrow_mut().push(format!("{}:{w}:{t}", p.source_thread_id))));
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(json!({ "anchor": "a1" }));
        ok(r.run(p).await);
        assert_eq!(*readied.borrow(), vec![("src-sess".to_string(), json!({ "anchor": "a1" }), "claude".to_string())]);
        let calls = r.calls();
        assert!(at(&calls, "land_images") < at(&calls, "ready_fork"));
        assert!(at(&calls, "ready_fork") < at(&calls, "copy_images"));
        assert_eq!(r.host.hooks.called("copy_images"), vec!["copy_images(w1, t1)".to_string()]);
        assert!(at(&calls, "copy_images") < at(&calls, "record_copies") && at(&calls, "record_copies") < at(&calls, "run_turn"), "the copied history opens the transcript");
        assert_eq!(*copied.borrow(), vec!["src:w1:t1".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("copy-1".to_string()), "the turn resumes the copy");
        assert_eq!(t.view.borrow().claude_session_id, Some("copy-1".to_string()));
        assert_eq!(r.host.records.borrow()["t1"].record.forked_from, Some(plan().forked_from));
        assert!(!dropped.get(), "a fork that ran keeps its copy");
        assert_eq!(r.host.hooks.called("drop_sent_images"), Vec::<String>::new());
    });
}

// L11, L18: a fork the start makes itself opens with the fork, resumes nothing and keeps no process
#[test]
fn a_fork_the_start_makes_itself_opens_with_the_fork_and_keeps_no_process() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&dyn HarnessAdapter, &str, Value, &str) -> LocalBoxFuture<'static, Result<Readied, HostError>>>(
            "SessionsCtx::ready_fork",
            Rc::new(|_a: &dyn HarnessAdapter, _s: &str, _t: Value, _h: &str| async { Ok(Readied::Fork { fork: json!({ "session": "src-sess", "turn": "anchor-1" }) }) }.boxed_local()),
        );
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(json!({ "anchor": "a1" }));
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("take_kept"), vec!["take_kept(t1, None)".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, None);
        assert!(t.keep.is_none(), "a fork is a launch of its own");
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!(starts[0].fork, Some(("src-sess".to_string(), "anchor-1".to_string())));
        assert_eq!(starts[0].resume, None);
        assert!(!starts[0].keep);
    });
}

// Z5: a fork that never ran leaves no copy and no image of its history
#[test]
fn a_fork_that_never_ran_drops_its_copy_and_its_sent_images() {
    run_local(async {
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        readies_a_copy(&r, &dropped, false);
        let sent: Rc<RefCell<Vec<(String, Vec<String>)>>> = Rc::new(RefCell::new(Vec::new()));
        let s = sent.clone();
        r.host.hooks.set::<dyn Fn(&str, &[String]) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_sent_images",
            Rc::new(move |w: &str, ids: &[String]| {
                s.borrow_mut().push((w.to_string(), ids.to_vec()));
                async { Ok(()) }.boxed_local()
            }),
        );
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(json!({ "anchor": "a1" }));
        err(r.run(p).await);
        settle().await;
        assert!(dropped.get(), "the copy is removed");
        assert_eq!(*sent.borrow(), vec![("w1".to_string(), vec!["t1".to_string()])]);
    });
}

// Z5: the copy that would not go is warned, in the source's words
#[test]
fn a_copy_that_would_not_be_removed_is_warned() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        readies_a_copy(&r, &dropped, true);
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(json!({ "anchor": "a1" }));
        err(r.run(p).await);
        settle().await;
        assert_eq!(warned.lines(), vec![format!("the copy a fork of thread {} made was not removed: the machine would not answer", thread_word("src"))]);
    });
}

// ===============================================================================================================
// L12: a running turn

fn steering(r: &Rig, set: impl FnOnce(&mut Handle)) -> Rc<Handle> {
    add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |h| {
        h.steers = true;
        set(h);
    })
}

fn steer_answers(r: &Rig, ends: bool) {
    r.host.hooks.set::<dyn Fn(&SessionEntry, &str, &StartOptions, Option<&Caller>, &str, bool) -> bool>(
        "SessionsCtx::steer_answered",
        Rc::new(move |_r: &SessionEntry, _id: &str, _o: &StartOptions, _origin: Option<&Caller>, _steer: &str, accepted: bool| accepted && ends),
    );
}

fn steering_send(r: &Rig) -> Prepared {
    let mut p = prepared(r);
    send_into_running(r, &mut p);
    p.opts.prompt = "end with STEERED".to_string();
    p.opts.request_id = Some("req_2".to_string());
    p
}

// L12: runtime-steer.test.ts:512
#[test]
fn a_steering_harness_steers_into_the_running_turn() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        let h0 = steering(&r, |_| {});
        steer_answers(&r, true);
        let mut p = steering_send(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Steered);
        assert_eq!(handle.id(), "sess-0", "the caller holds the running turn");
        assert_eq!(handle.turn_id(), "turn-0");
        assert_eq!(*h0.steered.borrow(), vec![("end with STEERED".to_string(), Some("steer-1".to_string()))]);
        let calls = r.calls();
        assert!(at(&calls, "keep_steer") < at(&calls, "steer_answered"), "the turn keeps the message before it is written");
        assert_eq!(r.host.hooks.called("keep_steer"), vec!["keep_steer(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_answered"), vec!["steer_answered(sess-0, steer-1, true)".to_string()]);
        assert!(r.turns.borrow().is_empty(), "no second turn");
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()]);
        assert_eq!(r.emitted(), Vec::<Value>::new());
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()], "a steer holds no slot");
        settle().await;
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()], "and keeps no token");
        assert_eq!(r.host.clock.pending(), 0);
    });
}

// L12: runtime-steer.test.ts:624 (a turn that has answered takes no message)
#[test]
fn a_turn_that_has_answered_takes_no_steer_and_the_send_queues_for_the_next() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let h0 = steering(&r, |_| {});
        r.host.rows.borrow_mut().get_mut("sess-0").unwrap().turn_live = Some(shared_live(TurnLive { reply: Some(TurnStatus::Completed), ..Default::default() }));
        steer_answers(&r, true);
        let join = r.spawn(steering_send(&r));
        settle().await;
        assert_eq!(*h0.steered.borrow(), Vec::<(String, Option<String>)>::new(), "steering it would put the words behind a reply already read");
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.host.hooks.called("keep_steer"), Vec::<String>::new());
    });
}

fn shared_live(l: TurnLive) -> SharedLive {
    Rc::new(RefCell::new(l))
}

// L12: files an image-less steer cannot carry wait for the turn to end
#[test]
fn files_the_running_turn_cannot_take_queue_the_send() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let h0 = steering(&r, |_| {});
        steer_answers(&r, true);
        let mut p = steering_send(&r);
        p.records = vec![record_of("image/png", 5)];
        p.opts.attachments = Some(vec![png()]);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(*h0.steered.borrow(), Vec::<(String, Option<String>)>::new(), "this agent takes no image into a running turn");
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        assert_eq!(ok(join.await.expect("the launch ran")).outcome(), SessionStartOutcome::Queued);
    });
}

// L12: runtime-steer.test.ts:286 (the adapter says the line missed the turn: not running)
#[test]
fn a_steer_the_turn_never_took_queues_behind_it() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        let h0 = steering(&r, |h| *h.steer_answer.borrow_mut() = Ok(SteerTaken::NotRunning));
        steer_answers(&r, true);
        let join = r.spawn(steering_send(&r));
        settle().await;
        assert_eq!(r.host.hooks.called("steer_answered"), vec!["steer_answered(sess-0, steer-1, false)".to_string()], "the answer says the line was not taken");
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        assert_eq!(ok(join.await.expect("the launch ran")).outcome(), SessionStartOutcome::Queued);
    });
}

// L12: no TS test (a write that threw on an agent that tells no unread messages falls to the person)
#[test]
fn a_lost_steer_falls_to_the_person() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        steering(&r, |h| *h.steer_answer.borrow_mut() = Err("the computer did not answer".to_string()));
        steer_answers(&r, true);
        let e = err(r.run(steering_send(&r)).await);
        assert_eq!(said(&e), ("the computer did not answer".to_string(), None, None), "no kind: no retry reads it as a computer that did not answer");
        assert_eq!(r.host.hooks.called("keep_steer"), vec!["keep_steer(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_lost"), vec!["steer_lost(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_answered"), Vec::<String>::new());
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()]);
        assert_eq!(r.emitted(), Vec::<Value>::new(), "a send that never held its row says no end");
        assert!(r.turns.borrow().is_empty());
    });
}

// L12: an agent that tells unread messages keeps the line the failed write may have landed
#[test]
fn a_write_that_threw_on_an_agent_that_tells_unread_is_left_to_its_unread_list() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        steering(&r, |h| {
            *h.steer_answer.borrow_mut() = Err("the computer did not answer".to_string());
            h.unread = Some(true);
        });
        steer_answers(&r, true);
        let handle = ok(r.run(steering_send(&r)).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Steered);
        assert_eq!(r.host.hooks.called("steer_lost"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("steer_answered"), vec!["steer_answered(sess-0, steer-1, true)".to_string()]);
    });
}

// L12: runtime-steer.test.ts:643 (another thread's turn is no reason to wait)
#[test]
fn a_turn_on_another_thread_holds_nothing_back() {
    run_local(async {
        let r = rig();
        add_running(&r.host, "sess-9", "turn-9", "other", json!({}), |_| {});
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.emitted_of("session.queued"), Vec::<Value>::new());
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Started);
    });
}

// ===============================================================================================================
// L13-L15: the handover's folder

fn gone_worktree() -> Value {
    json!({ "worktree": { "path": "/wt/feat-gone", "branch": "feat/gone", "made": true, "gone": true } })
}

fn project_folder(r: &Rig) {
    r.host.hooks.set::<dyn Fn(&Entry, Option<&str>) -> LocalBoxFuture<'static, Result<String, HostError>>>(
        "LaunchCtx::thread_folder",
        Rc::new(|_e: &Entry, _cwd: Option<&str>| async { Ok("/proj".to_string()) }.boxed_local()),
    );
}

fn ran_in_the_worktree(r: &Rig) {
    r.host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>("TrimmedCtx::folder_of", Rc::new(|_w: &str, _s: &str| Some("/wt/feat-gone".to_string())));
}

// L13: no TS test (behind and setup each record a row)
#[test]
fn behind_and_setup_each_record_a_session_behind_row() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.behind = Some("the branch is two commits behind".to_string());
        p.opts.setup = Some("ran pnpm install".to_string());
        ok(r.run(p).await);
        let row = |text: &str| json!({ "type": "session.behind", "workspaceId": "w1", "sessionId": "turn-1", "turnId": "turn-1", "threadId": "t1", "text": text });
        assert_eq!(r.host.recorded_json(), vec![row("the branch is two commits behind"), row("ran pnpm install")]);
    });
}

// L14: folder-threads.test.ts:637 (Claude Code keeps its session)
#[test]
fn a_gone_worktree_moves_the_turn_and_says_so() {
    run_local(async {
        let r = rig();
        let e = r.host.add_entry(entry("w3", gone_worktree()));
        project_folder(&r);
        ran_in_the_worktree(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.opens = false;
        p.resume = Some("sess-claude".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "sess-claude", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-gone", "to": "/proj", "branch": "feat/gone" })],
            "claude keys its sessions to the project: no fresh"
        );
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-claude".to_string()));
        assert_eq!(t.view.borrow().claude_session_id, Some("sess-claude".to_string()));
        assert_eq!(t.view.borrow().cwd, Some("/proj".to_string()));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].cwd.clone(), starts[0].resume.clone()), (Some("/proj".to_string()), Some("sess-claude".to_string())));
    });
}

// L14: folder-threads.test.ts:637 (another agent opens a fresh one)
#[test]
fn an_agent_that_keys_sessions_to_the_folder_opens_a_fresh_one_in_the_project() {
    run_local(async {
        let codex = adapter(|_| {});
        let r = rig_with(codex.clone());
        let e = r.host.add_entry(entry("w3", gone_worktree()));
        project_folder(&r);
        ran_in_the_worktree(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.opens = false;
        p.opts.harness = Some("codex".to_string());
        p.built = BuiltAdapter { harness: "codex".to_string(), adapter: codex };
        p.resume = Some("sess-codex".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "sess-codex", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-gone", "to": "/proj", "branch": "feat/gone", "fresh": true })]
        );
        let t = r.take_turn(0);
        assert_eq!(t.resume, None, "the session is dropped");
        assert_eq!(t.view.borrow().claude_session_id, None, "and the view's with it");
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].resume, None);
    });
}

// L14: folder-threads.test.ts:1038 (a worktree removed before any thread ran in it)
#[test]
fn a_worktree_removed_before_its_first_thread_ran_moves_that_thread_with_no_session() {
    run_local(async {
        let r = rig();
        let e = r.host.add_entry(entry("w3", json!({ "worktree": { "path": "/wt/feat-bare", "branch": "feat/bare", "made": true, "gone": true } })));
        project_folder(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.gone_now = Some("/wt/feat-bare".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "turn-1", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-bare", "to": "/proj", "branch": "feat/bare" })]
        );
        open(r.take_turn(0));
        assert_eq!(r.adapter.starts.lock().unwrap()[0].cwd, Some("/proj".to_string()));
    });
}

// L14: a worktree that is there is no move
#[test]
fn a_folder_that_did_not_change_says_no_move() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        r.host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>("TrimmedCtx::folder_of", Rc::new(|_w: &str, _s: &str| Some("/work/w1".to_string())));
        ok(r.run(p).await);
        assert_eq!(r.host.recorded_of("session.moved"), Vec::<Value>::new());
    });
}

// L13, L15: the picks, the folder and the session land on the row the start wrote
#[test]
fn the_picks_the_folder_and_the_session_land_on_the_row() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.effort = Some("low".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        let t = r.take_turn(0);
        let v = t.view.borrow().clone();
        assert_eq!(v, held_view(json!({ "model": "claude-haiku-5-5", "effort": "low", "permissionMode": "default", "cwd": "/work/w1", "claudeSessionId": "sess-1" })));
        assert!(Rc::ptr_eq(&t.view, &r.row("turn-1").view));
    });
}

// ===============================================================================================================
// L16: the kept process

fn kept_launch(fixed: &str, picks: Value) -> KeptLaunch {
    KeptLaunch { fixed: fixed.to_string(), picks: serde_json::from_value(picks).expect("picks") }
}

type KeptAsks = Rc<RefCell<Vec<(String, Option<KeptLaunch>, Option<String>, Option<KeptLaunch>)>>>;

fn takes_kept(r: &Rig, taken: Option<TakenKept>) -> KeptAsks {
    let asks: KeptAsks = Rc::new(RefCell::new(Vec::new()));
    let a = asks.clone();
    r.host.hooks.set::<dyn Fn(&str, Option<KeptLaunch>, Option<&str>, Option<&KeptLaunch>) -> Option<TakenKept>>(
        "SessionsCtx::take_kept",
        Rc::new(move |t: &str, ask: Option<KeptLaunch>, resume: Option<&str>, key: Option<&KeptLaunch>| {
            a.borrow_mut().push((t.to_string(), ask, resume.map(String::from), key.cloned()));
            taken.clone()
        }),
    );
    asks
}

fn expected_fixed(r: &Rig, version: Option<Option<&str>>, mcp: Option<&Value>) -> String {
    let area = LaunchArea::new(r.host.clone());
    area.launch_fixed(&r.entry, "claude", "/work/w1", mcp, version)
}

// L16: thread-keeps-process.test.ts:92 (the second send runs on the process the first left up, under its token)
#[test]
fn a_turn_runs_on_the_threads_kept_process_at_the_same_launch() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        let agent = Arc::new(FakeKept::default());
        let waiting: Waiting = Rc::new(Cell::new(false));
        let kept_launch_of = kept_launch("kept-fixed", json!({ "model": "claude-haiku-5-5" }));
        let taken = TakenKept { agent: agent.clone(), turn_token: "kept-token".to_string(), scope_device_id: Some("kept-dev".to_string()), launch: kept_launch_of.clone(), waiting: waiting.clone() };
        let asks = takes_kept(&r, Some(taken));
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.launch.scoped_device_id = Some("dev-1".to_string());
        p.waiting = Rc::new(Cell::new(false));
        ok(r.run(p).await);
        let fixed = expected_fixed(&r, Some(Some("2.1.300")), None);
        let picks = json!({ "model": "claude-haiku-5-5" });
        assert_eq!(*asks.borrow(), vec![("t1".to_string(), Some(kept_launch(&fixed, picks.clone())), Some("sess-1".to_string()), Some(kept_launch(&fixed, picks)))]);
        let t = r.take_turn(0);
        assert_eq!(t.turn_token, Some("kept-token".to_string()), "the token its first turn was launched with");
        assert_eq!(t.scope_device_id, Some("kept-dev".to_string()));
        assert_eq!(t.keep, Some(kept_launch_of));
        assert!(Rc::ptr_eq(t.waiting.as_ref().unwrap(), &waiting), "the kept process's own waiting box");
        settle().await;
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()], "this send's own device goes unused and is taken away");
        assert_eq!(r.emitted(), vec![held_event()], "a kept process says no starting");
        assert_eq!(r.host.hooks.called("end_kept"), Vec::<String>::new());
        open(t);
        assert!(r.adapter.starts.lock().unwrap().is_empty(), "the adapter is not asked to start");
        let nexts = agent.nexts.lock().unwrap();
        assert_eq!((nexts.len(), nexts[0].prompt.as_str(), nexts[0].images.len(), nexts[0].after), (1, "fix it", 0, false));
    });
}

// L16: a cold launch keeps its launch key for the next turn, matched on what this send named
#[test]
fn a_cold_turn_carries_its_launch_key_and_asks_for_what_it_named() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        r.host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> Resolved>(
            "LaunchCtx::defaults_on",
            Rc::new(|c: HarnessCatalog, _p: &Preferences, _project: &str| Resolved { catalog: Some(c), open_model: None, open_effort: Some("high".to_string()) }),
        );
        let asks = takes_kept(&r, None);
        let mut p = prepared(&r);
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opts.mcp_servers = Some(IndexMap::from([("extra".to_string(), McpServerSpec { command: "/bin/x".to_string(), args: Vec::new(), no_slate: None })]));
        p.launch.scoped_device_id = Some("dev-1".to_string());
        ok(r.run(p).await);
        let named = json!({ "extra": { "command": "/bin/x", "args": [] } });
        let fixed = expected_fixed(&r, Some(Some("2.1.300")), Some(&named));
        let picks = json!({ "model": "claude-haiku-5-5", "effort": "high", "permissionMode": "default" });
        let asked = json!({ "model": "claude-haiku-5-5", "permissionMode": "default" });
        assert_eq!(*asks.borrow(), vec![("t1".to_string(), Some(kept_launch(&fixed, asked)), None, Some(kept_launch(&fixed, picks.clone())))], "an effort the send left out is the thread's own");
        let t = r.take_turn(0);
        assert_eq!(t.turn_token, Some("tok-1".to_string()));
        assert_eq!(t.scope_device_id, Some("dev-1".to_string()));
        assert_eq!(t.keep, Some(kept_launch(&fixed, picks)));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert!(starts[0].keep, "the agent is asked to keep its process");
        assert_eq!(starts[0].version, Some("2.1.300".to_string()));
        assert_eq!((starts[0].model.clone(), starts[0].effort.clone(), starts[0].permission_mode.clone()), (Some("claude-haiku-5-5".to_string()), Some("high".to_string()), Some("default".to_string())));
    });
}

// L16: a kind that keeps no agents carries no key
#[test]
fn a_kind_that_keeps_no_agents_carries_no_launch_key() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry) -> bool>("LaunchCtx::keeps_agents", Rc::new(|_e: &Entry| false));
        let asks = takes_kept(&r, None);
        ok(r.run(prepared(&r)).await);
        assert_eq!(*asks.borrow(), vec![("t1".to_string(), None, None, None)]);
        let t = r.take_turn(0);
        assert!(t.keep.is_none());
        open(t);
        assert!(!r.adapter.starts.lock().unwrap()[0].keep);
    });
}

// Z6, Z1: a kept process taken for a start that never opened is ended
#[test]
fn a_kept_process_taken_for_a_start_that_fails_is_ended() {
    run_local(async {
        let r = rig();
        let taken = TakenKept { agent: Arc::new(FakeKept::default()), turn_token: "kept-token".to_string(), scope_device_id: None, launch: kept_launch("kept-fixed", json!({})), waiting: Rc::new(Cell::new(false)) };
        takes_kept(&r, Some(taken));
        run_turn_fails(&r, "the agent would not start");
        let e = err(r.run(prepared(&r)).await);
        assert_eq!(e.message, "the agent would not start");
        assert_eq!(r.host.hooks.called("end_kept"), vec!["end_kept(t1)".to_string()]);
    });
}

// ===============================================================================================================
// L17, L18: the transcript's opening and what the agent is started with

// L17: an outside conversation's earlier rows go in once, ahead of the prompt
#[test]
fn an_outside_conversations_earlier_rows_are_recorded_before_the_turn() {
    run_local(async {
        let r = rig();
        let earlier = |who: &str, text: &str, at: i64| -> SessionEarlierEvent {
            serde_json::from_value(json!({ "workspaceId": "", "sessionId": "", "who": who, "text": text, "at": at })).expect("an earlier row")
        };
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening {
            harness: "claude".to_string(),
            id: "out-1".to_string(),
            copy: false,
            project: "p1".to_string(),
            cwd: None,
            title: "talk".to_string(),
            earlier: vec![earlier("person", "hello", 1700), earlier("agent", "hi there", 1800)],
        });
        p.resume = Some("out-1".to_string());
        ok(r.run(p).await);
        let row = |who: &str, text: &str, at: i64| json!({ "type": "session.earlier", "workspaceId": "w1", "sessionId": "out-1", "threadId": "t1", "who": who, "text": text, "at": at });
        assert_eq!(r.host.recorded_json(), vec![row("person", "hello", 1700), row("agent", "hi there", 1800)]);
        assert!(at(&r.calls(), "run_turn") > 0);
    });
}

// L18: session-mcp.test.ts:49 (servers named on a start reach an adapter that renders them)
#[test]
fn the_launch_carries_picks_servers_and_the_rest_of_what_the_start_named() {
    run_local(async {
        let r = rig_with(adapter(|a| a.mcp = true));
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<bool, HostError>>>("SessionsCtx::limit_details_due", Rc::new(|_e: &Entry, _h: &str| async { Ok(true) }.boxed_local()));
        let wsp = McpServerSpec { command: "/usr/local/bin/node".to_string(), args: vec!["/opt/wsp/bin.js".to_string(), "mcp".to_string()], no_slate: None };
        let extra = McpServerSpec { command: "/bin/x".to_string(), args: Vec::new(), no_slate: None };
        let mut p = prepared(&r);
        p.title = Some("Landing fix".to_string());
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.effort = Some("low".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opts.context_window = Some("1m".to_string());
        p.mcp_servers = Some(IndexMap::from([("wsp".to_string(), wsp), ("extra".to_string(), extra)]));
        ok(r.run(p).await);
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        let s = &starts[0];
        assert_eq!(s.prompt, "fix it");
        assert_eq!((s.resume.clone(), s.copy, s.fork.clone()), (None, false, None));
        assert_eq!(s.cwd, Some("/work/w1".to_string()));
        assert_eq!((s.model.clone(), s.effort.clone(), s.permission_mode.clone(), s.context_window.clone(), s.fast), (Some("claude-haiku-5-5".to_string()), Some("low".to_string()), Some("default".to_string()), Some("1m".to_string()), None));
        assert_eq!(s.title, Some("Landing fix".to_string()));
        assert!(s.limit_details);
        assert_eq!(s.version, Some("2.1.300".to_string()));
        assert!(s.keep);
        assert!(s.seed.is_none(), "a new session has nothing to seed");
        let servers = s.mcp_servers.clone().expect("servers");
        let adapter_spec = |command: &str, args: &[&str]| wsp_adapters::port::McpServerSpec { command: command.to_string(), args: args.iter().map(|a| a.to_string()).collect(), no_slate: None };
        assert_eq!(servers.keys().cloned().collect::<Vec<_>>(), vec!["wsp".to_string(), "extra".to_string()]);
        assert_eq!(servers["wsp"], adapter_spec("/usr/local/bin/node", &["/opt/wsp/bin.js", "mcp"]));
        assert_eq!(servers["extra"], adapter_spec("/bin/x", &[]));
    });
}

// L18: the table's version is not the agent's: only a catalog the harness answered names one
#[test]
fn a_catalog_read_off_the_table_names_no_version() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].version.clone(), starts[0].limit_details, starts[0].mcp_servers.is_none()), (None, false, true));
    });
}

// L18: lost-session-line.test.ts:102 (a resume seeds the new session with the thread so far, this turn left out)
#[test]
fn the_seed_is_the_threads_earlier_turns_without_this_one() {
    run_local(async {
        let r = rig();
        let base = |kind: &str, turn: &str| json!({ "type": kind, "workspaceId": "w1", "sessionId": "sess-1", "turnId": turn, "threadId": "t1" });
        let events = vec![
            event(merged(base("session.start", "turn-0"), json!({ "prompt": "Remember the word lantern.", "agent": "claude" }))),
            event(merged(base("session.start", "turn-1"), json!({ "prompt": "what was the word?", "agent": "claude" }))),
        ];
        let served = events.clone();
        r.host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "SessionsCtx::open_thread",
            Rc::new(move |_w: &str, _t: &str| {
                let served = served.clone();
                async move { Ok(served) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        open(r.take_turn(0));
        let seed = r.adapter.starts.lock().unwrap()[0].seed.clone().expect("a resume carries its seed");
        let got = seed().await.expect("the seed reads");
        let earlier: Vec<SessionEvent> = events.iter().filter(|e| serde_json::to_value(e).unwrap()["turnId"] != "turn-1").cloned().collect();
        let expected = thread_seed(&thread_messages(&earlier, "t1"));
        assert_eq!(got, expected);
        assert_ne!(got, thread_seed(&thread_messages(&events, "t1")), "this turn's own message follows, so it is left out");
        assert_eq!(r.host.hooks.called("open_thread"), vec!["open_thread(w1, t1)".to_string()]);
    });
}

// L18, L20: the whole handover
#[test]
fn run_turn_is_handed_the_whole_turn() {
    run_local(async {
        let r = rig_with(adapter(|a| a.reports_edits = true));
        let waiting: Waiting = Rc::new(Cell::new(false));
        let mut p = prepared(&r);
        p.waiting = waiting.clone();
        p.title = Some("Landing".to_string());
        p.notify = Some(vec!["lead".to_string()]);
        p.notify_by = Some(scope("lead"));
        p.notify_road = Some(WorkspaceOrigin::Relayed);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        p.records = vec![record_of("image/png", 5)];
        p.opts.attachments = Some(vec![png()]);
        p.opts.via_slate = true;
        p.opts.after_limit = Some(1234.0);
        p.opts.effort = Some("low".to_string());
        p.opts.owed = true;
        ok(r.run(p).await);
        let t = r.take_turn(0);
        assert_eq!((t.entry.id(), t.thread_id.as_str(), t.turn_id.as_str()), ("w1".to_string(), "t1", "turn-1"));
        assert_eq!((t.notify.clone(), t.notify_by.clone(), t.notify_road), (Some(vec!["lead".to_string()]), Some(scope("lead")), Some(WorkspaceOrigin::Relayed)));
        assert_eq!((t.turn_token.clone(), t.scope_device_id.clone(), t.outcome), (Some("tok-1".to_string()), Some("dev-1".to_string()), SessionStartOutcome::Started));
        assert_eq!(
            t.opening,
            Opening {
                prompt: "fix it".to_string(),
                request_id: Some("req_1".to_string()),
                via_slate: true,
                after_cut: false,
                after_limit: Some(1234.0),
                opens_thread: true,
                title: Some("Landing".to_string()),
                attachments: Some(vec![record_of("image/png", 5)]),
            }
        );
        let asked = t.asked.as_ref().expect("asked").borrow().clone();
        assert_eq!(asked, TurnAsked { prompt: "fix it".to_string(), effort: Some("low".to_string()), typed: None, request_id: Some("req_1".to_string()), owed: Some(true), awaits_prompt: None });
        assert_eq!((t.resume.clone(), t.images_dir.clone(), t.written.is_none(), t.turn_live.is_none()), (None, None, true, true));
        assert!(t.reports_edits, "the adapter says it reports edits");
        assert!(Rc::ptr_eq(t.waiting.as_ref().unwrap(), &waiting));
    });
}

// L18: a send that is not an opening says so
#[test]
fn the_opening_names_a_thread_only_for_an_opening() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        ok(r.run(p).await);
        assert!(!r.take_turn(0).opening.opens_thread);
    });
}

// ===============================================================================================================
// L20: the thread's record

// L20
#[test]
fn the_record_is_made_at_the_first_turn_only() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.tree = Tree { parent_thread_id: Some("lead".to_string()), root_thread_id: Some("lead".to_string()) };
        p.opts.permission_mode = Some("default".to_string());
        p.clouds = Some(vec!["acme".to_string()]);
        ok(r.run(p).await);
        let expected = ThreadRecord {
            harness: "claude".to_string(),
            permission_mode: Some("default".to_string()),
            parent_thread_id: Some("lead".to_string()),
            root_thread_id: Some("lead".to_string()),
            worked: Some(thread_ran(&[])),
            clouds: Some(vec!["acme".to_string()]),
            ..Default::default()
        };
        assert_eq!(thread_record_of(&r, "t1"), HeldThread { workspace_id: "w1".to_string(), record: expected });
    });
}

// L20: a thread with a record keeps it; the clouds are told on it and a limit resume is taken off with its timer
#[test]
fn a_thread_that_has_a_record_keeps_it_and_takes_the_clouds_and_drops_a_limit_resume() {
    run_local(async {
        let r = rig();
        r.host.add_record(
            "t1",
            "w1",
            thread_record(json!({ "harness": "claude", "readAt": 5, "pinnedAt": 9, "limitResume": { "at": 99, "turnId": "turn-0" }, "worked": true })),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.clouds = Some(vec!["acme".to_string()]);
        ok(r.run(p).await);
        let expected = thread_record(json!({ "harness": "claude", "readAt": 5, "pinnedAt": 9, "worked": true, "clouds": ["acme"] }));
        assert_eq!(thread_record_of(&r, "t1"), HeldThread { workspace_id: "w1".to_string(), record: expected });
        assert_eq!(r.host.hooks.called("cancel_resume_timer"), vec!["cancel_resume_timer(t1)".to_string()]);
    });
}

// L20: a thread with no limit to resume leaves its timer alone
#[test]
fn a_thread_with_no_limit_resume_leaves_the_timer_alone() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("cancel_resume_timer"), Vec::<String>::new());
    });
}

// L20: no TS test (the files folder is kept once)
#[test]
fn files_folder_is_kept_once() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        record_landings(&r, Vec::new(), vec!["/work/w1/.wsp-files/t1/req_1/report.pdf"]);
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![pdf()]);
        ok(r.run(p).await);
        assert_eq!(r.host.records.borrow()["t1"].record.files_in, Some(vec!["/work/w1".to_string()]));
        let h1 = r.handles.borrow()[0].clone();
        end_turn(&r.host, "turn-1", &h1);
        let mut again = prepared(&r);
        again.opens = false;
        again.opts.thread = Some("t1".to_string());
        again.opts.attachments = Some(vec![pdf()]);
        ok(r.run(again).await);
        assert_eq!(r.host.records.borrow()["t1"].record.files_in, Some(vec!["/work/w1".to_string()]), "the same folder is not added twice");
    });
}

// L20: the thread says it worked off the rows that ended
#[test]
fn a_thread_from_before_its_record_gets_one_off_its_ended_rows() {
    run_local(async {
        let r = rig();
        r.host.add_row("sess-0", row(view("sess-0", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 5000, "claudeSessionId": "sess-0" })), "turn-0"));
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-0".to_string());
        ok(r.run(p).await);
        let ended = [view("sess-0", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 5000, "claudeSessionId": "sess-0" }))];
        assert_eq!(thread_record_of(&r, "t1").record, ThreadRecord { harness: "claude".to_string(), worked: Some(thread_ran(&ended)), ..Default::default() });
    });
}

// ===============================================================================================================
// L21: after the handover

// L21: replaces.test.ts:32
#[test]
fn a_restart_settles_the_thread_it_replaces_and_pushes_its_head() {
    run_local(async {
        let r = rig();
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>("LaunchCtx::preferences", Rc::new(|| async { Ok(prefs()) }.boxed_local()));
        let marked: Rc<RefCell<Vec<(Vec<String>, ThreadStamps)>>> = Rc::new(RefCell::new(Vec::new()));
        let m = marked.clone();
        r.host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |ids: &[String], stamps: StampsOf, _o: Option<&Caller>| {
                m.borrow_mut().push((ids.to_vec(), stamps("thr_old")));
                async { Ok(()) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        ok(r.run(p).await);
        let stamps = json!({ "readAt": T0, "settledAt": T0, "settleNamedAt": T0 });
        assert_eq!(*marked.borrow(), vec![(vec!["thr_old".to_string()], stamps.as_object().unwrap().clone())], "settled as a settle by the person would");
        assert_eq!(r.host.hooks.called("push_head"), vec!["push_head(thr_old)".to_string()]);
        assert!(at(&r.calls(), "run_turn") < at(&r.calls(), "mark") && at(&r.calls(), "mark") < at(&r.calls(), "push_head"));
        assert_eq!(r.host.records.borrow()["t1"].record.replaces, Some("thr_old".to_string()));
    });
}

// L21: a settle that failed is warned and the start still answers
#[test]
fn a_restart_whose_settle_fails_is_warned_and_still_answers() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>("LaunchCtx::preferences", Rc::new(|| async { Ok(prefs()) }.boxed_local()));
        r.host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(|_ids: &[String], _s: StampsOf, _o: Option<&Caller>| async { Err(HostError::new("the store would not write")) }.boxed_local()),
        );
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.turn_id(), "turn-1");
        assert_eq!(warned.lines(), vec![format!("thread {} was not settled after its restart: the store would not write", thread_word("thr_old"))]);
        assert_eq!(r.host.hooks.called("push_head"), vec!["push_head(thr_old)".to_string()], "the head is pushed all the same");
    });
}

// L21: a new session remembers where it ran
#[test]
fn a_new_session_remembers_the_target_and_a_resume_does_not() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("remember_target"), vec!["remember_target".to_string()]);
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("remember_target"), Vec::<String>::new());
    });
}

// L21: a target that was not remembered is no failed start
#[test]
fn a_target_that_was_not_remembered_is_warned_and_the_start_still_answers() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::remember_target",
            Rc::new(|_r: &WorkspaceRecord| async { Err(HostError::new("the store is full")) }.boxed_local()),
        );
        let handle = ok(r.run(prepared(&r)).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
        assert_eq!(warned.lines(), vec!["last target for w1 not remembered: the store is full".to_string()]);
    });
}

// L21: the handle answers with the start's notice
#[test]
fn the_handle_answers_with_the_starts_notice() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.notice = Some("the machine was started again".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.notice(), Some("the machine was started again".to_string()));
        assert_eq!((handle.id(), handle.turn_id(), handle.outcome()), ("turn-1".to_string(), "turn-1".to_string(), SessionStartOutcome::Started));
        let r = rig();
        assert_eq!(ok(r.run(prepared(&r)).await).notice(), None);
    });
}

// L21, L20: the keep of the images the send carried, once the turn is handed over
#[test]
fn the_images_a_send_carried_are_kept_once_it_is_handed_over() {
    run_local(async {
        let r = rig();
        let kept: Rc<RefCell<Vec<(String, String, Option<String>, Vec<Attachment>)>>> = Rc::new(RefCell::new(Vec::new()));
        let k = kept.clone();
        r.host.hooks.set::<dyn Fn(&str, &str, Option<&str>, Vec<Attachment>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::keep_sent_images",
            Rc::new(move |w: &str, t: &str, req: Option<&str>, a: Vec<Attachment>| {
                k.borrow_mut().push((w.to_string(), t.to_string(), req.map(String::from), a));
                async { Ok(()) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![png()]);
        ok(r.run(p).await);
        settle().await;
        assert_eq!(*kept.borrow(), vec![("w1".to_string(), "t1".to_string(), Some("req_1".to_string()), vec![png()])]);
        assert!(at(&r.calls(), "run_turn") < at(&r.calls(), "keep_sent_images"));
    });
}

// ===============================================================================================================
// Z1-Z3: a start that never reached its machine

// Z1: the scoped device goes, and a refusal to take it away is warned
#[test]
fn a_failed_start_hands_its_token_back() {
    run_local(async {
        let r = rig();
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        err(r.run(p).await);
        settle().await;
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.host.clock.pending(), 0, "hushed");
    });
}

// Z1
#[test]
fn a_token_that_would_not_be_taken_away_is_warned() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "LaunchCtx::revoke_device",
            Rc::new(|_d: &str| async { Err(HostError::new("the daemon is away")) }.boxed_local()),
        );
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        err(r.run(p).await);
        settle().await;
        assert_eq!(warned.lines(), vec![format!("the token of thread {} was not taken away: the daemon is away", thread_word("t1"))]);
    });
}

fn told_held_rig(owed: bool, on_held: bool) -> (Rig, Rc<RefCell<Vec<(bool, usize, Vec<String>, Option<ThreadScope>, Option<WorkspaceOrigin>, TurnResult, String)>>>, Prepared) {
    let r = rig();
    // A turn the thread ran before: the latest row once this send's own is gone.
    r.host.add_row("sess-prev", row(view("sess-prev", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 9000, "claudeSessionId": "sess-prev" })), "turn-0"));
    let looks = Rc::new(Cell::new(0u32));
    let l = looks.clone();
    r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>("SessionsCtx::cap_full", Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())));
    r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
        "SessionsCtx::cap_held",
        Rc::new(move |_t: &str| {
            l.set(l.get() + 1);
            if l.get() >= 2 { Some(CapHeld { stopped: Some("stopped before it started, while mac was full".to_string()), ..Default::default() }) } else { None }
        }),
    );
    let ended: Rc<RefCell<Vec<(bool, usize, Vec<String>, Option<ThreadScope>, Option<WorkspaceOrigin>, TurnResult, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let (e, h) = (ended.clone(), r.host.clone());
    r.host.hooks.set::<dyn Fn(SharedView, &str, &[String], Option<&ThreadScope>, Option<&WorkspaceOrigin>, TurnResult) -> ()>(
        "SessionsCtx::notify_end",
        Rc::new(move |view: SharedView, turn: &str, notify: &[String], by: Option<&ThreadScope>, road: Option<&WorkspaceOrigin>, res: TurnResult| {
            let row_there = h.rows.borrow().contains_key("turn-1");
            e.borrow_mut().push((row_there, h.emitted.borrow().len(), notify.to_vec(), by.cloned(), road.copied(), res, format!("{}:{turn}", view.borrow().id)));
        }),
    );
    let mut p = prepared(&r);
    p.opens = false;
    p.opts.thread = Some("t1".to_string());
    p.notify = Some(vec!["lead-thread".to_string()]);
    p.notify_by = Some(scope("lead"));
    p.notify_road = Some(WorkspaceOrigin::Relayed);
    p.opts.owed = owed;
    if on_held {
        p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
    }
    (r, ended, p)
}

// Z2: thread-cap.test.ts:583 (a held start that fails tells its targets)
#[test]
fn a_held_start_that_fails_tells_its_targets() {
    run_local(async {
        let (r, ended, p) = told_held_rig(false, true);
        let stopped = "stopped before it started, while mac was full";
        err(r.run(p).await);
        assert_eq!(
            *ended.borrow(),
            vec![(true, 1, vec!["lead-thread".to_string()], Some(scope("lead")), Some(WorkspaceOrigin::Relayed), result(json!({ "status": "failed", "error": stopped })), "turn-1:turn-1".to_string())],
            "told while the row still stands and before the end goes out"
        );
        assert_eq!(r.row_keys(), vec!["sess-prev".to_string()]);
        assert_eq!(r.emitted(), vec![held_event(), end_event(stopped)]);
    });
}

// Z2: an owed start tells nobody
#[test]
fn an_owed_start_that_fails_tells_nobody() {
    run_local(async {
        let (r, ended, p) = told_held_rig(true, true);
        err(r.run(p).await);
        assert_eq!(ended.borrow().len(), 0);
        assert_eq!(r.emitted_of("session.end").len(), 1);
    });
}

// Z2: a start that was never told held tells nobody either
#[test]
fn a_start_never_told_held_tells_nobody_when_it_fails() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let told = Rc::new(Cell::new(0u32));
        let t = told.clone();
        r.host.hooks.set::<dyn Fn(SharedView, &str, &[String], Option<&ThreadScope>, Option<&WorkspaceOrigin>, TurnResult) -> ()>(
            "SessionsCtx::notify_end",
            Rc::new(move |_v: SharedView, _t2: &str, _n: &[String], _b: Option<&ThreadScope>, _r: Option<&WorkspaceOrigin>, _res: TurnResult| t.set(t.get() + 1)),
        );
        let mut p = prepared(&r);
        p.opts.cwd = Some("/gone/dir".to_string());
        p.notify = Some(vec!["lead-thread".to_string()]);
        err(r.run(p).await);
        assert_eq!(told.get(), 0);
    });
}

// Z3: thread-cap.test.ts:627 (a send answered held whose agent then fails leaves its reason on the thread)
#[test]
fn a_sender_told_held_whose_start_failed_leaves_the_reason_on_the_threads_latest_row() {
    run_local(async {
        let (r, _ended, p) = told_held_rig(false, true);
        let stopped = "stopped before it started, while mac was full";
        err(r.run(p).await);
        assert_eq!(r.host.rows.borrow()["sess-prev"].view.borrow().failure, Some(listed_failure(Some(stopped))));
        assert_eq!(r.host.hooks.called("persist_sessions"), vec!["persist_sessions(w1)".to_string()]);
    });
}

// Z3: a sender that waited heard the refusal, and the thread did not fail
#[test]
fn a_sender_that_waited_leaves_the_threads_latest_row_alone() {
    run_local(async {
        let (r, ended, p) = told_held_rig(false, false);
        err(r.run(p).await);
        assert_eq!(r.host.rows.borrow()["sess-prev"].view.borrow().failure, None);
        assert_eq!(r.host.hooks.called("persist_sessions"), Vec::<String>::new());
        assert_eq!(ended.borrow().len(), 1, "its targets are told all the same");
        assert_eq!(r.emitted_of("session.end").len(), 1);
    });
}
