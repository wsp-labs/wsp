use std::cell::{Cell, RefCell};

use std::rc::Rc;

use std::sync::{Arc, Mutex};

use futures::channel::oneshot;

use futures::future::Shared;

use support::*;

use wsp_account::account::rules::TreeOf;

use wsp_core::console;

use wsp_core::types::wiring::{AccessMove, SteerTaken};

use wsp_threads::threads::forks::{ForkPicks, ForkPlanSource, ForkPlanTurn};

use wsp_threads::threads::sessions::start::Prepared;

use wsp_wire::types::attachments::{Attachment, AttachmentRecord};

use wsp_wire::types::js_number::JsNumber;

use wsp_wire::views::session::{thread_ran, ForkedFrom, SessionOrigin, TitleSource};

use wsp_wire::views::session_events::AGENT_STARTING_MS;

use wsp_wire::views::session_events_more::SessionEarlierEvent;

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

fn id_of(entry: &Entry) -> String {
    entry.borrow().record.borrow().id.clone()
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
    calls
        .iter()
        .position(|c| c == name || c.starts_with(&format!("{name}(")))
        .unwrap_or_else(|| panic!("{name} was never called: {calls:?}"))
}

// ---------------------------------------------------------------------------------------------------------------
// A started turn the test holds: its view, how it finishes and what was steered into it.

struct Handle {
    id: String,
    workspace_id: String,
    turn_id: String,
    outcome: SessionStartOutcome,
    row: Row,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    finish: RefCell<Option<oneshot::Sender<TurnResult>>>,
    steers: bool,
    steer_answer: RefCell<Result<SteerTaken, String>>,
    unread: Option<bool>,
    steered: RefCell<Vec<(String, Option<String>)>>,
}

impl Handle {
    fn new(turn_id: &str, outcome: SessionStartOutcome, row: Row) -> Handle {
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let finished = futures::FutureExt::map(rx, |r| r.unwrap_or_else(|_| result(json!({ "status": "failed" })))).boxed_local().shared();
        let (id, workspace_id) = {
            let v = row.borrow();
            (v.view.id.clone(), v.view.workspace_id.clone())
        };
        Handle {
            id,
            workspace_id,
            turn_id: turn_id.to_string(),
            outcome,
            row,
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
        self.row.borrow().view.clone()
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
    let r = row(view(key, "w1", doc), turn_id);
    let mut h = Handle::new(turn_id, SessionStartOutcome::Started, r.clone());
    set(&mut h);
    let h = Rc::new(h);
    r.borrow_mut().handle = Some(h.clone() as Rc<dyn SessionHandle>);
    host.add_row(key, r);
    h
}

/// A turn another send is still carrying to the machine: running, no handle, a launch not yet settled.
fn add_launching(host: &FakeHost, key: &str, thread: &str, launch: Joined) {
    let r = row(view(key, "w1", json!({ "threadId": thread, "startedAt": T0 - 1000, "prompt": "one" })), key);
    r.borrow_mut().launch = Some(launch);
    host.add_row(key, r);
}

/// The turn is over: its row reads completed and its finish settles.
fn end_turn(host: &FakeHost, key: &str, h: &Handle) {
    host.rows.borrow()[key].borrow_mut().view.status = serde_json::from_value(json!("completed")).expect("a status");
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
    seed: Option<Arc<dyn Fn() -> LocalBoxFuture<'static, Result<String, String>>>>,
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
    fn close(&self, _now: bool) -> LocalBoxFuture<'static, ()> {
        futures::FutureExt::boxed_local(async {})
    }
    fn exited(&self) -> SharedFuture<()> {
        futures::FutureExt::shared(futures::FutureExt::boxed_local(futures::future::pending::<()>()))
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

/// A row as it stood: `SessionEntry` is not `Clone`.
fn copy_of(r: &SessionEntry) -> SessionEntry {
    SessionEntry {
        view: r.view.clone(),
        turn_id: r.turn_id.clone(),
        replaces: r.replaces.clone(),
        notify: r.notify.clone(),
        notify_by: r.notify_by.clone(),
        notify_road: r.notify_road,
        turn_token: r.turn_token.clone(),
        scope_device_id: r.scope_device_id.clone(),
        handle: r.handle.clone(),
        end: r.end.clone(),
        turn_live: r.turn_live.clone(),
        run: r.run.clone(),
        from: r.from,
        asked: r.asked.clone(),
        snapshot: r.snapshot.clone(),
        pid: r.pid,
        launch: r.launch.clone(),
        calls: r.calls.clone(),
        unanswered: r.unanswered,
        queued: r.queued,
    }
}

fn snapshot(host: &FakeHost) -> Seen {
    Seen {
        rows: host.rows.borrow().iter().map(|(k, r)| (k.clone(), copy_of(&r.borrow()), r.borrow().view.clone())).collect(),
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

    let (t, hs) = (turns.clone(), handles.clone());
    host.hooks.set::<dyn Fn(TurnStart) -> Result<Rc<dyn SessionHandle>, HostError>>(
        "SessionsCtx::run_turn",
        Rc::new(move |turn: TurnStart| {
            let handle = Rc::new(Handle::new(&turn.turn_id, turn.outcome, turn.row.clone()));
            hs.borrow_mut().push(handle.clone());
            turn.row.borrow_mut().handle = Some(handle.clone() as Rc<dyn SessionHandle>);
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

    host.hooks.set::<dyn Fn(Arc<dyn HarnessAdapter>, &str, &str, &str) -> LocalBoxFuture<'static, String>>(
        "SessionsCtx::pay_cut_owed",
        Rc::new(|_a: Arc<dyn HarnessAdapter>, _w: &str, _t: &str, resume: &str| {
            let resume = resume.to_string();
            async move { resume }.boxed_local()
        }),
    );

    host.hooks.set::<dyn Fn(&Entry, &dyn HarnessAdapter, &str, &str, Vec<Attachment>) -> LocalBoxFuture<'static, Result<LandedImages, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(|_e: &Entry, _a: &dyn HarnessAdapter, _landing: &str, _dir: &str, _x: Vec<Attachment>| async { Ok(LandedImages::default()) }.boxed_local()),
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
    fn row(&self, key: &str) -> Row {
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
        opts: StartOptions {
            prompt: "fix it".to_string(),
            harness: Some("claude".to_string()),
            request_id: Some("req_1".to_string()),
            ..Default::default()
        },
        prefs: prefs(),
        plan: None,
        thread_id: "t1".to_string(),
        opens: true,
        entry: r.entry.clone(),
        replaces: None,
        resume: None,
        gone_now: None,
        title: None,
        tree: TreeOf::default(),
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
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": thread }))
        .expect("a thread scope")
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

fn forward() -> OnEvent {
    Arc::new(|_e: AdapterEvent| {})
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
    r.host.hooks.set::<dyn Fn(
        &Entry,
        &dyn HarnessAdapter,
        &str,
        &str,
        Vec<Attachment>,
    ) -> LocalBoxFuture<'static, Result<LandedImages, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(move |_e: &Entry, _a: &dyn HarnessAdapter, _landing: &str, _dir: &str, _x: Vec<Attachment>| {
            let rx = rx.borrow_mut().take();
            async move {
                if let Some(rx) = rx {
                    let _ = rx.await;
                }
                Ok(LandedImages::default())
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
    r.host.hooks.set::<dyn Fn(
        &Entry,
        &dyn HarnessAdapter,
        &str,
        &str,
        Vec<Attachment>,
    ) -> LocalBoxFuture<'static, Result<LandedImages, HostError>>>(
        "SessionsCtx::land_images",
        Rc::new(move |_e: &Entry, _a: &dyn HarnessAdapter, landing: &str, dir: &str, x: Vec<Attachment>| {
            i.borrow_mut().push((landing.to_string(), dir.to_string(), x));
            let landed = if images.is_empty() {
                LandedImages::default()
            } else {
                LandedImages { images: images.clone(), dir: Some(dir.to_string()) }
            };
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

fn thread_record_of(r: &Rig, thread: &str) -> ThreadRecord {
    r.host.records.borrow()[thread].clone()
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
    r.host.hooks.set::<dyn Fn(&WorkspaceRecord) -> Option<String>>(
        "SessionsCtx::workspace_place",
        Rc::new(|_r: &WorkspaceRecord| Some("place-1".to_string())),
    );
    r.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::place_name_of", Rc::new(|_p: &str| "acme's laptop".to_string()));
    tx
}

// ===============================================================================================================
// L10: a full computer

/// A cap hold nothing has looked at yet.
fn blank_hold() -> CapHeld {
    CapHeld { place_id: None, lenders: Vec::new(), order: 0, stopped: None, stops: Vec::new(), waiting: None, through: None }
}

fn ask_of(resume: Option<&str>) -> CapAt {
    CapAt {
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

// ===============================================================================================================
// L11, L17, Z5: a fork

fn plan() -> ForkPlan {
    ForkPlan {
        source: ForkPlanSource {
            thread_id: "src".to_string(),
            workspace_id: "w1".to_string(),
            entry: entry("w1", json!({})),
            harness: "claude".to_string(),
            title: "Source".to_string(),
            session: Some("src-sess".to_string()),
            cwd: None,
        },
        turn: Some(ForkPlanTurn { turn_id: "src-turn".to_string(), anchor: None, at_ref: None, after: Vec::new() }),
        events: Vec::new(),
        picks: ForkPicks::default(),
        forked_from: ForkedFrom { thread_id: "src".to_string(), turn_id: Some("src-turn".to_string()), title: "Source".to_string() },
    }
}

fn readies_a_copy(r: &Rig, dropped: &Rc<Cell<bool>>, drop_fails: bool) {
    let d = dropped.clone();
    r.host.hooks.set::<dyn Fn(Arc<dyn HarnessAdapter>, &str, ForkTurn, &str) -> LocalBoxFuture<'static, Result<ForkedSession, HostError>>>(
        "SessionsCtx::ready_fork",
        Rc::new(move |_a: Arc<dyn HarnessAdapter>, _s: &str, _turn: ForkTurn, _h: &str| {
            let d = d.clone();
            async move {
                let drop: Box<dyn FnOnce() -> LocalBoxFuture<'static, Result<(), String>>> = Box::new(move || {
                    d.set(true);
                    async move {
                        if drop_fails {
                            Err("the machine would not answer".to_string())
                        } else {
                            Ok(())
                        }
                    }
                    .boxed_local()
                });
                Ok(ForkedSession::Resume { resume: "copy-1".to_string(), drop })
            }
            .boxed_local()
        }),
    );
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
    r.host.hooks.set::<dyn Fn(&Row, &str, &StartOptions, Option<&Caller>, &str, bool) -> bool>(
        "SessionsCtx::steer_answered",
        Rc::new(move |_r: &Row, _id: &str, _o: &StartOptions, _origin: Option<&Caller>, _steer: &str, accepted: bool| accepted && ends),
    );
}

fn steering_send(r: &Rig) -> Prepared {
    let mut p = prepared(r);
    send_into_running(r, &mut p);
    p.opts.prompt = "end with STEERED".to_string();
    p.opts.request_id = Some("req_2".to_string());
    p
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
    r.host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
        "TrimmedCtx::folder_of",
        Rc::new(|_w: &str, _s: &str| Some("/wt/feat-gone".to_string())),
    );
}

// ===============================================================================================================
// L16: the kept process

fn kept_launch(fixed: &str, picks: Value) -> KeptLaunch {
    KeptLaunch { fixed: fixed.to_string(), picks: serde_json::from_value(picks).expect("picks") }
}

/// A kept process the test keeps a hand on.
struct SharedKept(Arc<FakeKept>);

impl KeptAgent for SharedKept {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.0.next(turn)
    }
    fn close(&self, now: bool) -> LocalBoxFuture<'static, ()> {
        self.0.close(now)
    }
    fn exited(&self) -> SharedFuture<()> {
        self.0.exited()
    }
}

fn kept_process(agent: Box<dyn KeptAgent>, device: Option<&str>, launch: KeptLaunch, waiting: Waiting) -> Rc<RefCell<KeptProcess>> {
    Rc::new(RefCell::new(KeptProcess {
        workspace_id: "w1".to_string(),
        agent,
        pid: None,
        launch,
        session: None,
        warm: None,
        turn_token: "kept-token".to_string(),
        scope_device_id: device.map(String::from),
        waiting,
        file: None,
        box_file: None,
        used_at: JsNumber(0.0),
        cancel: Box::new(|| {}),
    }))
}

type KeptAsks = Rc<RefCell<Vec<(String, Option<KeptLaunch>, Option<String>, Option<KeptLaunch>)>>>;

fn takes_kept(r: &Rig, taken: Option<Rc<RefCell<KeptProcess>>>) -> KeptAsks {
    let asks: KeptAsks = Rc::new(RefCell::new(Vec::new()));
    let a = asks.clone();
    r.host.hooks.set::<dyn Fn(&str, Option<KeptLaunch>, Option<&str>, Option<&KeptLaunch>) -> Option<Rc<RefCell<KeptProcess>>>>(
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

fn told_held_rig(
    owed: bool,
    on_held: bool,
) -> (Rig, Rc<RefCell<Vec<(bool, usize, Vec<String>, Option<ThreadScope>, Option<WorkspaceOrigin>, TurnResult, String)>>>, Prepared) {
    let r = rig();
    // A turn the thread ran before: the latest row once this send's own is gone.
    r.host.add_row(
        "sess-prev",
        row(
            view(
                "sess-prev",
                "w1",
                json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 9000, "claudeSessionId": "sess-prev" }),
            ),
            "turn-0",
        ),
    );
    let looks = Rc::new(Cell::new(0u32));
    let l = looks.clone();
    r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
        "SessionsCtx::cap_full",
        Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())),
    );
    r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
        "SessionsCtx::cap_held",
        Rc::new(move |_t: &str| {
            l.set(l.get() + 1);
            if l.get() >= 2 {
                Some(CapHeld { stopped: Some("stopped before it started, while mac was full".to_string()), ..blank_hold() })
            } else {
                None
            }
        }),
    );
    let ended: Rc<RefCell<Vec<(bool, usize, Vec<String>, Option<ThreadScope>, Option<WorkspaceOrigin>, TurnResult, String)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let (e, h) = (ended.clone(), r.host.clone());
    r.host.hooks.set::<dyn Fn(&Row, &[String], Option<&ThreadScope>, Option<&WorkspaceOrigin>, TurnResult)>(
        "SessionsCtx::notify_end",
        Rc::new(move |row: &Row, notify: &[String], by: Option<&ThreadScope>, road: Option<&WorkspaceOrigin>, res: TurnResult| {
            let row_there = h.rows.borrow().contains_key("turn-1");
            e.borrow_mut().push((
                row_there,
                h.emitted.borrow().len(),
                notify.to_vec(),
                by.cloned(),
                road.copied(),
                res,
                format!("{}:{}", row.borrow().view.id, row.borrow().turn_id),
            ));
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
