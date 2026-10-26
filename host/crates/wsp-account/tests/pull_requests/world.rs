
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::sync::Notify;
use common::{error_json, in_flight, live_with, oracle, plain, project, record, run, settle, FakeMachine};
use wsp_account::account::core_types::{AccountError, AccountResult, Ask, Entry, Frame};
use wsp_account::account::pull_requests::{CacheEntry, FrameFor, PullRequestsArea, PullRequestsCtx, ReadOn};
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_core::types::internal::DaemonRefusal;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::{GitPrReadReply, GitPrViewReply, GitRepoReadReply, GitUpdateReply, PullRequest, PullRequestSeen};
use wsp_wire::types::tree::TreeRecord;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::HostError;


























const REMOTE: &str = "https://github.com/wsp/pr-lab.git";

const HOME: &str = "/root";

const COPY: &str = "/root/stub-1";

const HEAD: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";

const SEEN: &str = "2026-10-05T17:40:01Z ec5c10de663bd1860925ad42e9580bab4eb1d377 open";

const MOVED: &str = "2026-10-05T18:02:11Z c0ffee open";

const NO_GH_TEXT: &str =
    "no signed-in command line for github.com is on this computer; the branch is pushed and the pull request waits for one";

const RL_TEXT: &str = "gh said: GraphQL: API rate limit already exceeded for user ID 1.";

const GH_DOWN: &str = "gh said: error connecting to api.github.com";

const T0: i64 = 1_790_000_000_000;

const POLL: i64 = 180_000;

const IDLE: i64 = 900_000;

const CHECKOUT_TTL: i64 = 10_000;

const PAGE_HOLD: i64 = 60_000;

const HOLD: i64 = 60_000;

const SLATE_POLL: i64 = 30_000;

const SETTINGS_MS: i64 = 3_600_000;


// ---- the TS's plain objects

fn with(mut base: Value, over: Value) -> Value {
    for (k, v) in over.as_object().expect("an object").iter() {
        base[k] = v.clone();
    }
    base
}


/// `open()` of pull-request.test.ts.
fn open(over: Value) -> Value {
    with(
        json!({
            "number": 12,
            "url": "https://github.com/wsp/pr-lab/pull/12",
            "state": "open",
            "host": "github.com",
            "draft": false,
            "base": "main",
            "branch": "fix/ci",
            "headOid": HEAD,
            "headSubject": "Set .ci-status to 1",
            "mergeable": "mergeable",
            "mergeState": "clean",
            "review": "none",
            "checks": [{ "name": "ci", "workflow": "ci", "state": "pass", "run": { "runId": 36, "jobId": 109 }, "link": "https://github.com/wsp/pr-lab/actions/runs/36/job/109" }],
            "additions": 2,
            "deletions": 1,
            "changedFiles": 1,
            "commits": 1,
            "behindBase": 0
        }),
        over,
    )
}


fn fact(over: Value, at: i64) -> Value {
    with(open(over), json!({ "readAt": at }))
}


fn why(line: &str, at: i64) -> Value {
    json!({ "why": line, "readAt": at })
}


/// `PAGE` of pull-request.test.ts.
fn page_json() -> Value {
    json!({
        "title": "t", "body": "b", "author": "octocat", "createdAt": "2026-09-30T10:00:00Z", "updatedAt": "2026-09-30T12:00:00Z",
        "labels": [], "reviewRequests": [], "latestReviews": [], "assignees": [],
        "commits": [{ "oid": HEAD, "subject": "Set .ci-status to 1", "body": "", "at": "2026-09-30T11:00:00Z", "author": "octocat" }],
        "reviews": [{ "id": 5324159317_i64, "author": "ana", "state": "changes_requested", "body": "Two things before this lands.", "at": "t" }],
        "comments": [{ "id": 5841958969_i64, "author": "vercel[bot]", "association": "none", "bot": true, "body": "Deployment failed for project wsp-www.", "url": "u", "at": "t" }],
        "reviewComments": [{ "id": 4109844888_i64, "path": "packages/engine/src/golden.ts", "line": 111, "side": "RIGHT", "author": "Zingzy", "association": "owner", "bot": false, "body": "Read it twice.", "url": "u", "at": "t", "hunk": "@@ -1 +1 @@\n-a\n+b", "resolved": false }],
        "files": []
    })
}


fn parsed<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}


fn page() -> GitPrViewReply {
    parsed(page_json())
}


fn merge_json() -> Value {
    json!({ "methods": ["squash", "merge"], "defaultMethod": "squash", "autoMerge": true })
}


fn frame(v: Value) -> Frame {
    v.as_object().expect("a frame is an object").clone()
}


/// One git.prRead frame as the area builds it: op, cwd, remote, then what the read names.
fn pr_frame(cwd: &str, tail: Value) -> String {
    with(json!({ "op": "git.prRead", "cwd": cwd, "remote": REMOTE }), tail).to_string()
}


fn ok(v: Value) -> Value {
    with(json!({ "id": 1, "ok": true }), v)
}


fn refused(code: Option<&str>, error: &str) -> Value {
    match code {
        Some(c) => json!({ "id": 1, "ok": false, "code": c, "error": error }),
        None => json!({ "id": 1, "ok": false, "error": error }),
    }
}


fn no_gh() -> Value {
    refused(Some("no-host-cli"), NO_GH_TEXT)
}


fn rate_limited() -> Value {
    refused(Some("rate-limited"), RL_TEXT)
}


fn refusal_error(code: Option<&str>, message: &str) -> AccountError {
    AccountError::Refusal(DaemonRefusal { code: code.map(String::from), message: message.to_string() })
}


/// The code and sentence of a daemon's refusal.
fn refusal(e: &AccountError) -> (Option<String>, String) {
    match e {
        AccountError::Refusal(r) => (r.code.clone(), r.message.clone()),
        AccountError::Host(h) => panic!("a host error where a refusal was expected: {}", h.message),
    }
}


fn host_json(e: &AccountError) -> Value {
    match e {
        AccountError::Host(h) => error_json(h),
        AccountError::Refusal(r) => panic!("a refusal where a host error was expected: {:?} {}", r.code, r.message),
    }
}


fn builder(f: impl Fn(&str) -> Value + 'static) -> FrameFor {
    Rc::new(move |cwd: &str| frame(f(cwd)))
}


fn parse_as<T: DeserializeOwned + 'static>() -> Rc<dyn Fn(Frame) -> AccountResult<T>> {
    Rc::new(|f: Frame| serde_json::from_value(Value::Object(f)).map_err(|e| AccountError::from(plain(&e.to_string()))))
}


fn whole() -> Rc<dyn Fn(Frame) -> AccountResult<Frame>> {
    Rc::new(Ok)
}


fn seen_of(v: Value) -> PullRequestSeen {
    parsed(v)
}


fn sv(id: &str, workspace: &str, thread: Option<&str>, root: Option<&str>, status: &str) -> SessionView {
    let mut v = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": status });
    if let Some(t) = thread {
        v["threadId"] = json!(t);
    }
    if let Some(r) = root {
        v["rootThreadId"] = json!(r);
    }
    parsed(v)
}


fn checkout_json(branch: &str, head: &str) -> Checkout {
    parsed(json!({ "branch": branch, "ahead": 0, "behind": 0, "changed": 0, "head": head, "readAt": T0 }))
}


/// A timer's undo as an entry holds it: called once, the way the area takes it.
fn held(cancel: Cancel) -> Option<Box<dyn FnOnce()>> {
    Some(Box::new(move || cancel()))
}


fn counted() -> (Rc<Cell<u32>>, Cancel) {
    let c = Rc::new(Cell::new(0));
    let c2 = c.clone();
    (c, Rc::new(move || c2.set(c2.get() + 1)))
}


// ---- the context

type Answer = Rc<dyn Fn(&Frame) -> Value>;


struct Daemon {
    answers: RefCell<HashMap<String, Answer>>,
    hold: Cell<bool>,
    released: Notify,
}


impl Daemon {
    fn new() -> Daemon {
        Daemon { answers: RefCell::new(HashMap::new()), hold: Cell::new(false), released: Notify::new() }
    }
}


struct World {
    me: Weak<World>,
    clock: FakeClock,
    cache: RefCell<IndexMap<String, CacheEntry>>,
    live: RefCell<IndexMap<String, Entry>>,
    views: RefCell<Vec<SessionView>>,
    projects: RefCell<IndexMap<String, ProjectView>>,
    here: Daemon,
    copy: Daemon,
    log: RefCell<Vec<String>>,
    persisted: RefCell<Vec<Value>>,
    statuses: RefCell<Vec<Value>>,
    watched: Cell<bool>,
    slate_watches: Cell<bool>,
    pauses: Cell<bool>,
    checkout_answer: RefCell<Option<Checkout>>,
    mark_error: RefCell<Option<String>>,
    nap_error: RefCell<Option<String>>,
}


impl World {
    /// A world at T0 with one project, `p1` on REMOTE with base main, and this computer answering git.prRead with the
    /// open pull request.
    fn new() -> Rc<World> {
        let w = Rc::new_cyclic(|me| World {
            me: me.clone(),
            clock: FakeClock::new(T0),
            cache: RefCell::new(IndexMap::new()),
            live: RefCell::new(IndexMap::new()),
            views: RefCell::new(vec![]),
            projects: RefCell::new(IndexMap::new()),
            here: Daemon::new(),
            copy: Daemon::new(),
            log: RefCell::new(vec![]),
            persisted: RefCell::new(vec![]),
            statuses: RefCell::new(vec![]),
            watched: Cell::new(false),
            slate_watches: Cell::new(false),
            pauses: Cell::new(false),
            checkout_answer: RefCell::new(None),
            mark_error: RefCell::new(None),
            nap_error: RefCell::new(None),
        });
        w.set_project(Some("main"), "main", REMOTE);
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({})) })));
        w
    }

    fn set_project(&self, base: Option<&str>, default_branch: &str, remote: &str) {
        let mut p = json!({ "id": "p1", "name": "pr-lab", "computer": "here", "source": { "kind": "git" }, "path": "/home/u/pr-lab", "remote": remote, "defaultBranch": default_branch });
        if let Some(b) = base {
            p["base"] = json!(b);
        }
        self.projects.borrow_mut().insert("p1".into(), project(&p));
    }

    fn here_says(&self, op: &str, f: impl Fn(&Frame) -> Value + 'static) {
        self.here.answers.borrow_mut().insert(op.into(), Rc::new(f));
    }

    fn copy_says(&self, op: &str, f: impl Fn(&Frame) -> Value + 'static) {
        self.copy.answers.borrow_mut().insert(op.into(), Rc::new(f));
    }

    /// An entry on a record built from `over` laid on a running cloud copy of p1, held live under its id.
    fn entry(&self, over: Value) -> Entry {
        let base = json!({ "id": "w1", "name": "pr-lab", "kind": "cloud", "project": "p1", "machineId": "m1", "phase": "running" });
        let rec = record(&with(base, over));
        let machine = FakeMachine::new(&rec.machine_id);
        let id = rec.id.clone();
        let e = live_with(rec, machine);
        self.live.borrow_mut().insert(id, e.clone());
        e
    }

    fn cloud(&self) -> Entry {
        self.entry(json!({}))
    }

    /// A folder of this computer's project: a local workspace, no worktree.
    fn folder(&self) -> Entry {
        self.entry(json!({ "kind": "local", "machineId": "" }))
    }

    fn on_branch(&self, e: &Entry, branch: &str, head: &str) {
        let c = checkout_json(branch, head);
        e.borrow_mut().checkout = Some(c.clone());
        *self.checkout_answer.borrow_mut() = Some(c);
    }

    fn put(&self, line: String) {
        self.log.borrow_mut().push(line);
    }

    fn log_of(&self, prefix: &str) -> Vec<String> {
        self.log.borrow().iter().filter_map(|l| l.strip_prefix(prefix).map(String::from)).collect()
    }

    fn frames(&self, road: &str) -> Vec<String> {
        self.log_of(&format!("frame {road} "))
    }

    fn roads(&self, road: &str) -> usize {
        self.log.borrow().iter().filter(|l| **l == format!("road {road}")).count()
    }

    /// The timers armed are exactly those due at `ats`, each moment named once.
    fn assert_due(&self, ats: &[i64]) {
        assert_eq!(self.clock.pending(), ats.len(), "timers armed, wanted at {ats:?}");
        for at in ats {
            assert!(self.clock.due_for(*at), "no timer due at {at}");
        }
    }

    fn advance(&self, ms: i64) {
        self.clock.advance(ms);
    }

    fn cached(&self, key: &str) -> Option<CacheEntry> {
        self.cache.borrow().get(key).cloned()
    }

    fn fields_persisted(&self) -> Vec<String> {
        self.persisted.borrow().iter().map(fields).collect()
    }

    fn release(&self, road: &str) {
        let d = if road == "here" { &self.here } else { &self.copy };
        d.hold.set(false);
        d.released.notify_waiters();
    }

    async fn serve(&self, road: &'static str, f: Frame) -> AccountResult<Frame> {
        self.put(format!("frame {road} {}", Value::Object(f.clone())));
        let d = if road == "here" { &self.here } else { &self.copy };
        while d.hold.get() {
            d.released.notified().await;
        }
        let op = f.get("op").and_then(|o| o.as_str()).unwrap_or("").to_string();
        let answer = d.answers.borrow().get(&op).cloned();
        let reply = match answer {
            Some(a) => a(&f),
            None => refused(None, &format!("{op} was not answered here")),
        };
        let map = reply.as_object().expect("a reply is an object").clone();
        if map.get("ok") == Some(&Value::Bool(true)) {
            return Ok(map);
        }
        let code = map.get("code").and_then(|c| c.as_str()).map(String::from);
        let message = map.get("error").and_then(|e| e.as_str()).map(String::from).unwrap_or_else(|| format!("{op} was refused"));
        Err(AccountError::Refusal(DaemonRefusal { code, message }))
    }

    fn ask(&self, road: &'static str) -> Ask {
        let w = self.me.upgrade().expect("the world");
        Rc::new(move |f: Frame| {
            let w = w.clone();
            Box::pin(async move { w.serve(road, f).await })
        })
    }
}


/// The record's four fields this area writes, as the TS's store held them, in the record's key order.
fn fields(rec: &Value) -> String {
    let mut out = serde_json::Map::new();
    for (k, v) in rec.as_object().unwrap() {
        if ["pr", "prSent", "branchThreads", "tree"].contains(&k.as_str()) {
            out.insert(k.clone(), v.clone());
        }
    }
    Value::Object(out).to_string()
}


#[async_trait(?Send)]
impl PullRequestsCtx for World {
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    fn cache_get(&self, key: &str) -> Option<CacheEntry> {
        self.cache.borrow().get(key).cloned()
    }
    fn cache_set(&self, key: &str, entry: CacheEntry) {
        self.put(format!("cache.set {key}"));
        self.cache.borrow_mut().insert(key.into(), entry);
    }
    fn cache_delete(&self, key: &str) {
        self.put(format!("cache.delete {key}"));
        self.cache.borrow_mut().shift_remove(key);
    }
    fn live(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.views.borrow().clone()
    }
    async fn on_this_computer(
        &self,
        work: Box<dyn FnOnce(Ask, String) -> LocalBoxFuture<'static, AccountResult<Value>>>,
    ) -> AccountResult<Value> {
        self.put("road here".into());
        work(self.ask("here"), HOME.to_string()).await
    }
    async fn with_daemon(&self, entry: &Entry, work: wsp_account::account::core_types::Work<Value>) -> AccountResult<Value> {
        self.put("road copy".into());
        let _ = entry;
        work(self.ask("copy")).await
    }
    fn checkout_of(&self, _record: &WorkspaceRecord) -> Result<String, HostError> {
        Ok(COPY.to_string())
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<Entry, HostError> {
        self.put(format!("entry_of {workspace_id} {}", if origin.is_some() { "caller" } else { "none" }));
        self.live.borrow().get(workspace_id).cloned().ok_or_else(|| plain(&format!("no workspace {workspace_id}")))
    }
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        self.projects.borrow().get(id).cloned().ok_or_else(|| plain(&format!("no project {id}")))
    }
    fn status_watched(&self) -> bool {
        self.watched.get()
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        self.put("persist".into());
        self.persisted.borrow_mut().push(serde_json::to_value(record).unwrap());
        Ok(())
    }
    async fn status_now(&self, entry: &Entry) {
        self.put("status".into());
        let pr = serde_json::to_value(&entry.borrow().pr).unwrap();
        self.statuses.borrow_mut().push(pr);
    }
    async fn read_checkout(&self, entry: &Entry, force: bool) -> AccountResult<Option<Checkout>> {
        self.put(format!("checkout {force}"));
        let c = self.checkout_answer.borrow().clone();
        entry.borrow_mut().checkout = c.clone();
        Ok(c)
    }
    fn slates_watch_pr(&self, _workspace_id: &str) -> bool {
        self.slate_watches.get()
    }
    async fn mark_settled(&self, threads: Vec<String>, at: i64) -> AccountResult<()> {
        self.put(format!("mark {} {at}", json!(threads)));
        match self.mark_error.borrow().clone() {
            Some(m) => Err(plain(&m).into()),
            None => Ok(()),
        }
    }
    fn pauses(&self, _record: &WorkspaceRecord) -> bool {
        self.pauses.get()
    }
    async fn nap_with(&self, workspace_id: &str) -> AccountResult<()> {
        self.put(format!("nap {workspace_id}"));
        match self.nap_error.borrow().clone() {
            Some(m) => Err(plain(&m).into()),
            None => Ok(()),
        }
    }
    fn read_lead_of(&self, child: &Entry) {
        self.put(format!("lead {}", child.borrow().record.borrow().id));
    }
    async fn queued(&self, workspace_id: &str, work: LocalBoxFuture<'static, AccountResult<Value>>) -> AccountResult<Value> {
        self.put(format!("queued {workspace_id}"));
        work.await
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.put(format!("warn {line}"));
    }
}


fn setup() -> (Rc<World>, Rc<PullRequestsArea>, Entry) {
    let w = World::new();
    let area = PullRequestsArea::new(w.clone());
    let e = w.cloud();
    (w, area, e)
}


fn pr_of(e: &Entry) -> Value {
    serde_json::to_value(&e.borrow().pr).unwrap()
}


fn rec_of(e: &Entry) -> Value {
    serde_json::to_value(&*e.borrow().record.borrow()).unwrap()
}


fn pr_value(r: &Option<PullRequestSeen>) -> Value {
    serde_json::to_value(r).unwrap()
}


fn kept_pr(over: Value) -> Value {
    with(json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "open", "base": "main", "branch": "fix/ci" }), over)
}


/// A kept pull request on the record, as a past read wrote it.
fn keep(e: &Entry, over: Value) {
    let mut v = rec_of(e);
    v["pr"] = kept_pr(over);
    *e.borrow().record.borrow_mut() = parsed(v);
}


fn set_record(e: &Entry, key: &str, value: Value) {
    let mut v = rec_of(e);
    v[key] = value;
    *e.borrow().record.borrow_mut() = parsed(v);
}


// ---- Q2

async fn try_road(area: &Rc<PullRequestsArea>, runs: &Rc<Cell<u32>>, road: &str, answer: AccountResult<i32>) -> AccountResult<i32> {
    let r = runs.clone();
    area.held_off(
        road,
        Box::pin(async move {
            r.set(r.get() + 1);
            answer
        }),
    )
    .await
}


// ---- Q4

const PAGE_KEY: &str = "page:https://github.com/wsp/pr-lab.git#12";


fn page_frame(cwd: &str) -> String {
    json!({ "op": "git.prView", "cwd": cwd, "remote": REMOTE, "number": 12 }).to_string()
}


fn page_body(here: bool) -> String {
    format!("{{\"page\":{},\"here\":{here}}}", serde_json::to_string(&page()).unwrap())
}


// ---- Q5, invariant 4

fn writes() -> Vec<(&'static str, FrameFor)> {
    vec![
        (
            "git.prReply",
            builder(|cwd| json!({ "op": "git.prReply", "cwd": cwd, "remote": REMOTE, "number": 12, "replyTo": 7, "body": "Fixed." })),
        ),
        (
            "git.prResolve",
            builder(
                |cwd| json!({ "op": "git.prResolve", "cwd": cwd, "remote": REMOTE, "number": 12, "threadId": "PRRT_kwDOx", "resolved": true }),
            ),
        ),
        (
            "git.prReact",
            builder(
                |cwd| json!({ "op": "git.prReact", "cwd": cwd, "remote": REMOTE, "number": 12, "subject": "IC_kwDOx", "content": "eyes", "on": true }),
            ),
        ),
    ]
}


fn write_frame(op: &str) -> String {
    match op {
        "git.prReply" => json!({ "op": op, "cwd": HOME, "remote": REMOTE, "number": 12, "replyTo": 7, "body": "Fixed." }),
        "git.prResolve" => json!({ "op": op, "cwd": HOME, "remote": REMOTE, "number": 12, "threadId": "PRRT_kwDOx", "resolved": true }),
        _ => json!({ "op": op, "cwd": HOME, "remote": REMOTE, "number": 12, "subject": "IC_kwDOx", "content": "eyes", "on": true }),
    }
    .to_string()
}


// ---- Q7

fn answer_pr(w: &World, pr: Value) {
    w.here_says("git.prRead", move |_| ok(json!({ "pr": pr.clone() })));
}


fn answer_none(w: &World) {
    w.here_says("git.prRead", |_| ok(json!({})));
}


// Q7 step 1: a folder that left its branch drops the pull request and reads again.
async fn q7_left_branch_drops_and_reads_again() {
    let (w, area, _e) = setup();
    let e = w.folder();
    keep(&e, json!({}));
    set_record(&e, "prSent", json!([{ "kind": "comment", "id": 5841958969_i64, "at": T0 }]));
    e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 1000)));
    let (cancelled, cancel) = counted();
    e.borrow_mut().pr_poll = held(cancel);
    w.on_branch(&e, "other", "h1");
    answer_none(&w);
    let got = area.read_pull_request(&e, false, false).await;
    assert_eq!(pr_value(&got), Value::Null);
    assert_eq!(
        w.log.borrow().clone(),
        vec![
            "persist".to_string(),
            "status".to_string(),
            "road here".to_string(),
            format!("frame here {}", pr_frame(HOME, json!({ "branch": "other" }))),
            "status".to_string()
        ]
    );
    assert_eq!(w.fields_persisted(), vec!["{}".to_string()]);
    assert_eq!(cancelled.get(), 1);
    assert_eq!(pr_of(&e), Value::Null);
    assert_eq!(e.borrow().pr_none_at.clone(), Some("h1".to_string()));
    assert!(e.borrow().pr_poll.is_none());
}


// Q7 step 2: a kept record pr that is not open answers the entry's pr, nothing read.
async fn q7_a_settled_record_is_never_read() {
    for state in ["merged", "closed"] {
        let (w, area, _e) = setup();
        let e = w.folder();
        keep(&e, json!({ "state": state }));
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().pr = Some(seen_of(fact(json!({ "state": state }), T0 - 5)));
        let got = area.read_pull_request(&e, true, true).await;
        assert_eq!(pr_value(&got), fact(json!({ "state": state }), T0 - 5), "{state}");
        assert!(w.log.borrow().is_empty(), "{state}");
        e.borrow_mut().pr = None;
        assert_eq!(pr_value(&area.read_pull_request(&e, true, false).await), Value::Null);
        assert!(w.log.borrow().is_empty(), "{state}");
    }
}


// Q7 step 3: an unforced read inside its hold answers held; the hold is the poll interval for a fact, the checkout's
// ttl for a why; an armed timer of another interval re-arms from the read.
async fn q7_a_held_read_answers_held() {
    let read_frame = pr_frame(HOME, json!({ "branch": "fix/ci" }));
    // (what is held, window, age, held)
    let rows: Vec<(Value, bool, i64, bool)> = vec![
        (fact(json!({}), 0), false, IDLE - 1, true),
        (fact(json!({}), 0), false, IDLE, false),
        (fact(json!({}), 0), true, POLL - 1, true),
        (fact(json!({}), 0), true, POLL, false),
        (
            why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0),
            true,
            CHECKOUT_TTL - 1,
            true,
        ),
        (
            why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0),
            true,
            CHECKOUT_TTL,
            false,
        ),
        (
            why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0),
            false,
            CHECKOUT_TTL,
            false,
        ),
    ];
    for (n, (held, watched, age, stands)) in rows.into_iter().enumerate() {
        let (w, area, e) = setup();
        w.watched.set(watched);
        w.on_branch(&e, "fix/ci", "h1");
        let mut held = held;
        held["readAt"] = json!(T0 - age);
        e.borrow_mut().pr = Some(seen_of(held.clone()));
        let got = area.read_pull_request(&e, false, false).await;
        if stands {
            assert_eq!(pr_value(&got), held, "row {n}");
            assert!(w.frames("here").is_empty(), "row {n}");
        } else {
            assert_eq!(w.frames("here"), vec![read_frame.clone()], "row {n}");
            assert_eq!(pr_value(&got), fact(json!({}), T0), "row {n}");
        }
    }
    // a force reads whatever the age
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 1)));
    area.read_pull_request(&e, true, false).await;
    assert_eq!(w.frames("here"), vec![read_frame.clone()]);

    // a window opened since the timer was armed: held, and re-armed at the interval now in force from the last read
    let (w, area, e) = setup();
    w.watched.set(true);
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 100_000)));
    let (cancelled, cancel) = counted();
    e.borrow_mut().pr_poll = held(cancel);
    e.borrow_mut().pr_poll_ms = Some(IDLE as u64);
    let got = area.read_pull_request(&e, false, false).await;
    assert_eq!(pr_value(&got), fact(json!({}), T0 - 100_000));
    assert!(w.log.borrow().is_empty());
    assert_eq!(cancelled.get(), 1);
    w.assert_due(&[T0 - 100_000 + POLL]);
    assert_eq!(e.borrow().pr_poll_ms, Some(POLL as u64));
    // an armed timer already at the interval in force is left alone, and no timer armed is none armed
    let (cancelled, cancel) = counted();
    e.borrow_mut().pr_poll = held(cancel);
    area.read_pull_request(&e, false, false).await;
    assert_eq!(cancelled.get(), 0);
    e.borrow_mut().pr_poll = None;
    e.borrow_mut().pr_poll_ms = None;
    let before = w.clock.pending();
    area.read_pull_request(&e, false, false).await;
    assert_eq!((w.clock.pending(), e.borrow().pr_poll_ms), (before, None));
    // a why holds no timer of its own to re-arm
    e.borrow_mut().pr = Some(seen_of(why("w", T0 - 1000)));
    let (cancelled, cancel) = counted();
    e.borrow_mut().pr_poll = held(cancel);
    e.borrow_mut().pr_poll_ms = Some(IDLE as u64);
    area.read_pull_request(&e, false, false).await;
    assert_eq!(cancelled.get(), 0);
}


// Q7 step 4: no kept pr and the checkout's head the one a read found none on: held, no read, unless forced.
async fn q7_a_branch_with_no_pull_request_waits_for_its_head() {
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().pr_none_at = Some("h1".into());
    answer_none(&w);
    assert_eq!(pr_value(&area.read_pull_request(&e, false, false).await), Value::Null);
    assert!(w.log.borrow().is_empty());
    // the held answer is the entry's pr, a why included
    e.borrow_mut().pr = Some(seen_of(why("a sentence", T0 - 50_000)));
    assert_eq!(pr_value(&area.read_pull_request(&e, false, false).await), why("a sentence", T0 - 50_000));
    assert!(w.log.borrow().is_empty());
    // forced, or the head moved, reads
    area.read_pull_request(&e, true, false).await;
    assert_eq!(w.frames("here").len(), 1);
    w.on_branch(&e, "fix/ci", "h2");
    e.borrow_mut().pr_none_at = Some("h1".into());
    area.read_pull_request(&e, false, false).await;
    assert_eq!(w.frames("here").len(), 2);
    assert_eq!(e.borrow().pr_none_at.clone(), Some("h2".to_string()));
    // no head on the checkout is not a head to match
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().checkout.as_mut().unwrap().head = None;
    answer_none(&w);
    area.read_pull_request(&e, false, false).await;
    assert_eq!(w.frames("here").len(), 1);
    assert_eq!(e.borrow().pr_none_at.clone(), None);
    area.read_pull_request(&e, false, false).await;
    assert_eq!(w.frames("here").len(), 2);
}


// Q7 step 5 and 9: a read in flight is joined unless whole, and the mark clears however a read ends.
async fn q7_the_in_flight_mark() {
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    let marker = Some(seen_of(why("the read already in flight", T0)));
    let m = marker.clone();
    let flying: LocalBoxFuture<'static, Result<Option<PullRequestSeen>, HostError>> = Box::pin(async move { Ok(m) });
    e.borrow_mut().pr_reading = Some(flying.shared());
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), pr_value(&marker));
    assert!(w.log.borrow().is_empty());
    assert!(e.borrow().pr_reading.is_some());
    // whole reads on its own, and clears the mark when it ends
    let got = area.read_pull_request(&e, true, true).await;
    assert_eq!(pr_value(&got), fact(json!({}), T0));
    assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "fix/ci" }))]);
    assert!(e.borrow().pr_reading.is_none());
    // a failed read clears it too
    w.here_says("git.prRead", |_| refused(None, GH_DOWN));
    area.read_pull_request(&e, true, false).await;
    assert!(e.borrow().pr_reading.is_none());
    // and so does one that took a no-host-cli's why
    w.here_says("git.prRead", |_| no_gh());
    w.copy_says("git.prRead", |_| no_gh());
    area.read_pull_request(&e, true, false).await;
    assert!(e.borrow().pr_reading.is_none());
}


// Q7 step 6: the read itself: what is asked, and when nothing is.
async fn q7_the_read_frame_and_when_none_is_sent() {
    // a project with no remote answers the entry's pr, nothing read, nothing written
    let (w, area, e) = setup();
    w.set_project(Some("main"), "main", "");
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 1)));
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), fact(json!({}), T0 - 1));
    assert!(w.log.borrow().is_empty());

    // no kept pr: the branch comes off the checkout, read only when the entry holds none
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    area.read_pull_request(&e, true, false).await;
    assert_eq!(w.log_of("checkout "), Vec::<String>::new());
    assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "fix/ci" }))]);
    let (w, area, e) = setup();
    *w.checkout_answer.borrow_mut() = Some(checkout_json("feat/read", "h1"));
    area.read_pull_request(&e, true, false).await;
    assert_eq!(w.log_of("checkout "), vec!["false".to_string()]);
    assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "feat/read" }))]);
    // a checkout read that finds none, and no branch at all, answer the entry's pr
    let (w, area, e) = setup();
    e.borrow_mut().pr = Some(seen_of(why("held", T0 - 1)));
    assert_eq!(pr_value(&area.read_pull_request(&e, true, false).await), why("held", T0 - 1));
    assert_eq!(w.log_of("checkout "), vec!["false".to_string()]);
    assert!(w.frames("here").is_empty());

    // the base: the record's, else the project's, never the project's default branch; a (-branch is not a branch
    for (record_base, project_base, branch, reads) in [
        (Some("develop"), Some("main"), "develop", false),
        (Some("develop"), Some("main"), "main", true),
        (None, Some("main"), "main", false),
        (None, Some("trunk"), "trunk", false),
        (None, None, "main", true),
        (None, Some("main"), "(HEAD detached at abc1234)", false),
        (None, Some("main"), "(no branch, rebasing fix/ci)", false),
    ] {
        let (w, area, e) = setup();
        w.set_project(project_base, "main", REMOTE);
        if let Some(b) = record_base {
            set_record(&e, "base", json!(b));
        }
        w.on_branch(&e, branch, "h1");
        area.read_pull_request(&e, true, false).await;
        assert_eq!(w.frames("here").len(), usize::from(reads), "{record_base:?} {project_base:?} {branch}");
        if reads {
            assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": branch }))]);
        }
    }

    // a kept pr is read by its number, and carries what the last read saw only while that stands
    let cached_pr = |tag: Option<&str>, body: Value| CacheEntry { body, tag: tag.map(String::from), fetched_at: T0 - 5 };
    let key = PullRequestsArea::pr_key(REMOTE, 12);
    let rows: Vec<(&str, Option<CacheEntry>, bool, bool)> = vec![
        ("nothing cached", None, false, false),
        ("a tag", Some(cached_pr(Some(SEEN), open(json!({})))), false, true),
        ("whole", Some(cached_pr(Some(SEEN), open(json!({})))), true, false),
        ("a body that does not parse", Some(cached_pr(Some(SEEN), json!({ "x": 1 }))), false, false),
        ("a check pending", Some(cached_pr(Some(SEEN), open(json!({ "checks": [{ "name": "ci", "state": "pending" }] })))), false, false),
        ("no tag", Some(cached_pr(None, open(json!({})))), false, false),
        (
            "checks that finished",
            Some(cached_pr(Some(SEEN), open(json!({ "checks": [{ "name": "a", "state": "pass" }, { "name": "b", "state": "fail" }] })))),
            false,
            true,
        ),
    ];
    for (label, cached, whole_read, seen) in rows {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        w.on_branch(&e, "fix/ci", "h1");
        if let Some(c) = cached {
            w.cache.borrow_mut().insert(key.clone(), c);
        }
        answer_none(&w);
        area.read_pull_request(&e, true, whole_read).await;
        let tail = if seen { json!({ "number": 12, "seen": SEEN }) } else { json!({ "number": 12 }) };
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, tail)], "{label}");
    }

    // a worktree no thread ran in reads the pull request of its own branch
    let (w, area, _e) = setup();
    let tree = w.entry(json!({ "id": "w5", "kind": "local", "machineId": "", "worktree": { "path": "/home/u/.wsp/trees/z", "branch": "feat/z", "made": true } }));
    w.on_branch(&tree, "feat/z", "h1");
    answer_none(&w);
    area.read_pull_request(&tree, false, false).await;
    assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "feat/z" }))]);
}


// Q7 step 7: unchanged keeps the cached body at this read's time; a pr that came with a tag is cached; the head a
// branch had none on is remembered.
async fn q7_what_is_taken_from_the_answer() {
    let key = PullRequestsArea::pr_key(REMOTE, 12);
    let cached = |body: Value| CacheEntry { body, tag: Some(SEEN.into()), fetched_at: T0 - 5 };

    // unchanged with a body that parses: the cached body, readAt this read; the record stands, so nothing is written
    let (w, area, e) = setup();
    keep(&e, json!({}));
    w.on_branch(&e, "fix/ci", "h1");
    w.cache.borrow_mut().insert(key.clone(), cached(open(json!({ "headOid": "c0ffee" }))));
    w.here_says("git.prRead", |_| ok(json!({ "unchanged": true, "seen": SEEN })));
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), fact(json!({ "headOid": "c0ffee" }), T0));
    assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0));
    assert_eq!(
        w.log.borrow().clone(),
        vec![
            "road here".to_string(),
            format!("frame here {}", pr_frame(HOME, json!({ "number": 12, "seen": SEEN }))),
            "status".to_string()
        ]
    );
    assert_eq!(w.cached(&key).unwrap(), cached(open(json!({ "headOid": "c0ffee" }))));

    // unchanged where the cache no longer holds a body that parses: nothing comes of it, and the fact is let go
    let (w, area, e) = setup();
    keep(&e, json!({}));
    w.on_branch(&e, "fix/ci", "h1");
    w.cache.borrow_mut().insert(key.clone(), cached(json!({ "x": 1 })));
    w.here_says("git.prRead", |_| ok(json!({ "unchanged": true, "seen": SEEN })));
    e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 1)));
    assert_eq!(pr_value(&area.read_pull_request(&e, true, false).await), Value::Null);
    assert_eq!(w.statuses.borrow().clone(), vec![Value::Null]);

    // a pr that came with a tag is cached with it, at this read's time
    let (w, area, e) = setup();
    keep(&e, json!({}));
    w.on_branch(&e, "fix/ci", "h1");
    w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "headOid": "c0ffee" })), "seen": MOVED })));
    area.read_pull_request(&e, true, false).await;
    let held = w.cached(&key).unwrap();
    assert_eq!(parsed::<PullRequest>(held.body.clone()), parsed::<PullRequest>(open(json!({ "headOid": "c0ffee" }))));
    assert_eq!((held.tag, held.fetched_at), (Some(MOVED.to_string()), T0));
    assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0));
    // one that came with none is not
    let (w, area, e) = setup();
    keep(&e, json!({}));
    w.on_branch(&e, "fix/ci", "h1");
    area.read_pull_request(&e, true, false).await;
    assert!(w.cached(&key).is_none());

    // no pr and none kept: the head is remembered; a pr, or a kept one, clears it
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    answer_none(&w);
    area.read_pull_request(&e, true, false).await;
    assert_eq!(e.borrow().pr_none_at.clone(), Some("h1".to_string()));
    assert_eq!(w.statuses.borrow().clone(), vec![Value::Null]);
    answer_pr(&w, open(json!({})));
    area.read_pull_request(&e, true, false).await;
    assert_eq!(e.borrow().pr_none_at.clone(), None);
    let (w, area, e) = setup();
    keep(&e, json!({}));
    w.on_branch(&e, "fix/ci", "h1");
    e.borrow_mut().pr_none_at = Some("h1".into());
    answer_none(&w);
    area.read_pull_request(&e, true, false).await;
    assert_eq!(e.borrow().pr_none_at.clone(), None);
}


// Q7 step 8: a refusal for want of a command line says why; any other failure keeps the last fact and re-arms the poll.
async fn q7_a_failed_read() {
    let at = T0;
    let unread = oracle("pull_requests")["unread"].as_str().unwrap().to_string();
    let stopped = oracle("pull_requests")["stopped"].as_str().unwrap().to_string();
    let here = pr_frame(HOME, json!({ "branch": "fix/ci" }));
    let copy = pr_frame(COPY, json!({ "branch": "fix/ci" }));

    // neither road has one: the unread sentence, off the remote's host
    let (w, area, e) = setup();
    w.on_branch(&e, "fix/ci", "h1");
    w.here_says("git.prRead", |_| no_gh());
    w.copy_says("git.prRead", |_| no_gh());
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), why(&unread, at));
    assert_eq!(
        w.log.borrow().clone(),
        vec![
            "road here".to_string(),
            format!("frame here {here}"),
            "road copy".to_string(),
            format!("frame copy {copy}"),
            "status".to_string()
        ]
    );
    assert!(w.persisted.borrow().is_empty());
    w.assert_due(&[]);

    // a stopped copy: the stopped sentence names the workspace, and the copy is not asked
    let (w, area, _e) = setup();
    let e = w.entry(json!({ "id": "w2", "phase": "napping" }));
    w.on_branch(&e, "fix/ci", "h1");
    w.here_says("git.prRead", |_| no_gh());
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), why(&stopped, at));
    assert_eq!(w.roads("copy"), 0);

    // a local folder is never "stopped"
    let (w, area, _e) = setup();
    let e = w.entry(json!({ "id": "w2", "kind": "local", "machineId": "", "phase": "napping" }));
    w.on_branch(&e, "fix/ci", "h1");
    w.here_says("git.prRead", |_| no_gh());
    let got = area.read_pull_request(&e, true, false).await;
    assert_eq!(pr_value(&got), why(&unread, at));

    // any other failure keeps what was held and reads again on the timer, saying nothing
    for failure in [refused(None, GH_DOWN), rate_limited(), refused(Some("no-git-credential"), "x")] {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0 - 1)));
        w.here_says("git.prRead", move |_| failure.clone());
        let got = area.read_pull_request(&e, true, false).await;
        assert_eq!(pr_value(&got), fact(json!({}), T0 - 1));
        assert_eq!(w.log.borrow().clone(), vec!["road here".to_string(), format!("frame here {here}")]);
        w.assert_due(&[T0 + IDLE]);
        assert_eq!(e.borrow().pr_poll_ms, Some(IDLE as u64));
    }
}


// ---- Q8

fn record_pr(e: &Entry) -> String {
    rec_of(e).get("pr").map(|v| v.to_string()).unwrap_or_else(|| "none".into())
}


// ---- Q13

fn tree_views(w: &World) {
    let mut v = w.views.borrow_mut();
    v.push(sv("s1", "w1", Some("tA"), None, "completed"));
    v.push(sv("s2", "w1", Some("tA"), None, "completed"));
    v.push(sv("s3", "w2", Some("tB"), Some("tA"), "completed"));
    v.push(sv("s4", "w1", None, None, "failed"));
    v.push(sv("s5", "w3", Some("tZ"), None, "completed"));
    v.push(sv("s6", "w1", Some("tC"), Some("tOther"), "completed"));
    v.push(sv("s7", "w3", Some("tY"), Some("tZ"), "running"));
}


fn settled_pr() -> Value {
    json!({ "state": "merged", "mergedAt": T0 - 1 })
}


// ---- Q15

fn update_json(merged: bool, commits: i64, conflicts: Value) -> Value {
    json!({ "base": "main", "merged": merged, "commits": commits, "conflicts": conflicts })
}
