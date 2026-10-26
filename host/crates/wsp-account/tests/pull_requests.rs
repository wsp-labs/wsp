// SPDX-License-Identifier: AGPL-3.0-only
//! The pull requests area (spec/pull_requests.md): the git host read for a workspace and the pull request kept on it.
//! Holds runtime invariant 4. Every sentence is the TS's, written by oracle/pull_requests.oracle.ts into
//! tests/fixtures/oracle/pull_requests.json; every frame, record and status is the TS's own literal or what the TS ran
//! to. `World` below is the context: a clock of its own, two daemons that answer by op, and a log of everything the
//! area asked of it, in order (a road is logged when it is asked, a frame when it is sent).
//!
//! Cases: q1 to q16 and the two invariant_4 tests are the spec's rules; each `ts_` test ports a case of
//! test/pull-request.test.ts (or tree.test.ts, worktree-sweep.test.ts) at the area's seam. See spec ## Tests.
mod common;

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

use common::{error_json, FakeMachine, in_flight, live_with, oracle, plain, project, record, run, settle};
use wsp_account::account::core_types::{AccountError, AccountResult, Ask, Frame, Entry};
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_account::account::pull_requests::{CacheEntry, FrameFor, PullRequestsArea, PullRequestsCtx, ReadOn};
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
const NO_GH_TEXT: &str = "no signed-in command line for github.com is on this computer; the branch is pushed and the pull request waits for one";
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
    async fn on_this_computer(&self, work: Box<dyn FnOnce(Ask, String) -> LocalBoxFuture<'static, AccountResult<Value>>>) -> AccountResult<Value> {
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

// ---- Q1

// Q1: this computer answers first; only a no-host-cli refusal falls to a running copy that reads the host itself.
#[test]
fn q1_read_host_falls_back_to_a_running_copy() {
    run(async {
        let read = || builder(|cwd| json!({ "op": "git.prRead", "cwd": cwd, "remote": REMOTE, "branch": "fix/ci" }));
        let here_frame = pr_frame(HOME, json!({ "branch": "fix/ci" }));
        let copy_frame = pr_frame(COPY, json!({ "branch": "fix/ci" }));
        let want = |n: i64| parsed::<GitPrReadReply>(ok(json!({ "pr": open(json!({ "number": n })) })));

        // this computer answers: here is true and the copy is never asked
        let (w, area, e) = setup();
        let got = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(got.here);
        assert_eq!(got.read, want(12));
        assert_eq!(w.log.borrow().clone(), vec!["road here".to_string(), format!("frame here {here_frame}")]);

        // readHost answers the read alone
        let (_w, area, e) = setup();
        assert_eq!(area.read_host(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap(), want(12));

        // no command line here and a running copy: the copy's daemon with the frame built for its checkout
        let (w, area, e) = setup();
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 3 })) })));
        let got = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(!got.here);
        assert_eq!(got.read, want(3));
        assert_eq!(
            w.log.borrow().clone(),
            vec!["road here".to_string(), format!("frame here {here_frame}"), "road copy".to_string(), format!("frame copy {copy_frame}")]
        );

        // a joined computer's folder is not local: it reads for itself while it runs
        let (w, area, _e) = setup();
        let place = w.entry(json!({ "id": "w2", "kind": "place", "machineId": "", "place": "box1" }));
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 4 })) })));
        let got = area.read_host_on(&place, read(), parse_as::<GitPrReadReply>()).await.unwrap();
        assert!(!got.here);
        assert_eq!(got.read, want(4));

        // a stopped copy is never woken, a local one has no gh of its own: the refusal stands, the copy is not asked
        for over in [json!({ "phase": "napping" }), json!({ "phase": "pausing" }), json!({ "kind": "local", "machineId": "" })] {
            let (w, area, _e) = setup();
            let entry = w.entry(with(json!({ "id": "w3" }), over.clone()));
            w.here_says("git.prRead", |_| no_gh());
            w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({})) })));
            let e = area.read_host_on(&entry, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
            assert_eq!(refusal(&e), (Some("no-host-cli".into()), NO_GH_TEXT.into()), "{over}");
            assert_eq!(w.roads("copy"), 0, "{over}");
            assert!(w.frames("copy").is_empty(), "{over}");
            assert_eq!(w.frames("here"), vec![here_frame.clone()], "{over}");
        }

        // any other failure rethrows whole, a rate limit and a plain gh failure alike
        for (code, text) in [(Some("rate-limited"), RL_TEXT), (None, GH_DOWN), (Some("no-git-credential"), "no credential")] {
            let (w, area, e) = setup();
            w.here_says("git.prRead", move |_| refused(code, text));
            w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({})) })));
            let err = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
            assert_eq!(refusal(&err), (code.map(String::from), text.to_string()));
            assert_eq!(w.roads("copy"), 0);
        }

        // the copy's own failure is what comes back, after both roads were asked
        let (w, area, e) = setup();
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| refused(None, GH_DOWN));
        let err = area.read_host_on(&e, read(), parse_as::<GitPrReadReply>()).await.unwrap_err();
        assert_eq!(refusal(&err), (None, GH_DOWN.to_string()));
        assert_eq!(w.frames("here"), vec![here_frame.clone()]);
        assert_eq!(w.frames("copy"), vec![copy_frame.clone()]);
    });
}

// ---- Q2

async fn try_road(area: &Rc<PullRequestsArea>, runs: &Rc<Cell<u32>>, road: &str, answer: AccountResult<i32>) -> AccountResult<i32> {
    let r = runs.clone();
    area.held_off(road, Box::pin(async move {
        r.set(r.get() + 1);
        answer
    }))
    .await
}

// Q2: a road that met a rate-limited refusal answers every read with it for a minute and runs nothing.
#[test]
fn q2_rate_limit_holds_the_road() {
    run(async {
        let (_w, area, _e) = setup();
        let runs = Rc::new(Cell::new(0));
        let limit = || refusal_error(Some("rate-limited"), RL_TEXT);
        let first = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        assert_eq!(refusal(&first), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(runs.get(), 1);
        for _ in 0..3 {
            let held = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
            assert_eq!(refusal(&held), (Some("rate-limited".into()), RL_TEXT.into()));
        }
        // a read that would have succeeded is held too
        let held = try_road(&area, &runs, "here", Ok(5)).await.unwrap_err();
        assert_eq!(refusal(&held), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(runs.get(), 1);
        // another road is its own
        assert_eq!(try_road(&area, &runs, "w1", Ok(5)).await.unwrap(), 5);
        assert_eq!(runs.get(), 2);

        // no other refusal holds anything: each read runs
        let others = [
            refusal_error(Some("no-host-cli"), NO_GH_TEXT),
            refusal_error(None, GH_DOWN),
            refusal_error(Some("rate-limit"), RL_TEXT),
            AccountError::Host(plain(RL_TEXT)),
        ];
        for (n, other) in others.into_iter().enumerate() {
            let road = format!("other{n}");
            let before = runs.get();
            let _ = try_road(&area, &runs, &road, Err(other.clone())).await.unwrap_err();
            let _ = try_road(&area, &runs, &road, Err(other)).await.unwrap_err();
            assert_eq!(runs.get(), before + 2, "{road}");
            assert_eq!(try_road(&area, &runs, &road, Ok(1)).await.unwrap(), 1);
        }
    });
}

// Q2: the hold ends at now >= until, a minute from the refusal, and a read inside it does not move the end.
#[test]
fn q2_the_hold_ends_on_time() {
    run(async {
        let (w, area, _e) = setup();
        let runs = Rc::new(Cell::new(0));
        let limit = || refusal_error(Some("rate-limited"), RL_TEXT);
        let _ = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        w.advance(HOLD / 2);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        w.advance(HOLD / 2 - 1);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        assert_eq!(runs.get(), 1);
        w.advance(1);
        assert_eq!(try_road(&area, &runs, "here", Ok(7)).await.unwrap(), 7);
        assert_eq!(try_road(&area, &runs, "here", Ok(8)).await.unwrap(), 8);
        assert_eq!(runs.get(), 3);
        // a second refusal holds from its own time
        let _ = try_road(&area, &runs, "here", Err(limit())).await.unwrap_err();
        w.advance(HOLD - 1);
        let _ = try_road(&area, &runs, "here", Ok(1)).await.unwrap_err();
        assert_eq!(runs.get(), 4);
        w.advance(1);
        assert_eq!(try_road(&area, &runs, "here", Ok(9)).await.unwrap(), 9);
    });
}

// ---- Q3

// Q3: the entry by entryOf, the pull request's number off the record, the remote off the project; the keys.
#[test]
fn q3_pull_request_on_and_the_keys() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        let (got, remote, number) = area.pull_request_on("w1", None).await.unwrap();
        assert!(Rc::ptr_eq(&got, &e));
        assert_eq!((remote.as_str(), number), (REMOTE, 12));
        let caller = Caller::Road(parsed::<WorkspaceOrigin>(json!("here")));
        area.pull_request_on("w1", Some(&caller)).await.unwrap();
        assert_eq!(w.log_of("entry_of "), vec!["w1 none".to_string(), "w1 caller".to_string()]);

        // number 0 is a number
        keep(&e, json!({ "number": 0 }));
        assert_eq!(area.pull_request_on("w1", None).await.unwrap().2, 0);

        // no pull request: the plain refusal, by name, before the project is read
        let (w, area, e) = setup();
        set_record(&e, "project", json!("p9"));
        let err = area.pull_request_on("w1", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), oracle("pull_requests")["noPullRequest"]);
        assert!(w.log_of("entry_of ").len() == 1);

        // a pull request on a project nothing holds: the project's own refusal
        keep(&e, json!({}));
        let err = area.pull_request_on("w1", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), json!({ "message": "no project p9" }));

        // an entry nothing holds: entryOf's refusal
        let err = area.pull_request_on("w7", None).await.map(|_| ()).unwrap_err();
        assert_eq!(error_json(&err), json!({ "message": "no workspace w7" }));

        assert_eq!(PullRequestsArea::page_key(REMOTE, 12), "page:https://github.com/wsp/pr-lab.git#12");
        assert_eq!(PullRequestsArea::pr_key(REMOTE, 12), "pr:https://github.com/wsp/pr-lab.git#12");
    });
}

// ---- Q4

const PAGE_KEY: &str = "page:https://github.com/wsp/pr-lab.git#12";

fn page_frame(cwd: &str) -> String {
    json!({ "op": "git.prView", "cwd": cwd, "remote": REMOTE, "number": 12 }).to_string()
}

fn page_body(here: bool) -> String {
    format!("{{\"page\":{},\"here\":{here}}}", serde_json::to_string(&page()).unwrap())
}

// Q4: a page held PR_PAGE_HOLD_MS per remote and number, dropped by a timer only if still the entry that set it.
#[test]
fn q4_page_held_then_dropped() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        let got = area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(got, ReadOn { read: page(), here: true });
        assert_eq!(w.frames("here"), vec![page_frame(HOME)]);
        let held = w.cached(PAGE_KEY).unwrap();
        assert_eq!(serde_json::to_string(&held.body).unwrap(), page_body(true));
        assert_eq!((held.tag, held.fetched_at), (None, T0));
        w.assert_due(&[T0 + PAGE_HOLD]);

        // within the hold nothing is read
        w.advance(PAGE_HOLD - 1);
        assert_eq!(area.read_page(&e, REMOTE, 12, false).await.unwrap(), ReadOn { read: page(), here: true });
        assert_eq!(w.frames("here").len(), 1);
        // at the hold the timer drops it and the next read asks again
        w.advance(1);
        assert!(w.cached(PAGE_KEY).is_none());
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.cached(PAGE_KEY).unwrap().fetched_at, T0 + PAGE_HOLD);
        // a fresh read asks even while held
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 3);
        // another number is its own page
        area.read_page(&e, REMOTE, 13, false).await.unwrap();
        assert_eq!(w.frames("here").last().unwrap(), &json!({ "op": "git.prView", "cwd": HOME, "remote": REMOTE, "number": 13 }).to_string());
    });
}

// Q4: the timer of an older read leaves an entry a fresh read set after it.
#[test]
fn q4_an_old_timer_leaves_a_newer_entry() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        w.advance(PAGE_HOLD / 2);
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(PAGE_HOLD / 2);
        assert_eq!(w.cached(PAGE_KEY).unwrap().fetched_at, T0 + PAGE_HOLD / 2);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(PAGE_HOLD / 2);
        assert!(w.cached(PAGE_KEY).is_none());
    });
}

// Q4: a page read by the copy says so, and is held as read by it.
#[test]
fn q4_a_page_the_copy_read_says_here_false() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| no_gh());
        w.copy_says("git.prView", |_| ok(page_json()));
        let got = area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(got, ReadOn { read: page(), here: false });
        assert_eq!(w.frames("here"), vec![page_frame(HOME)]);
        assert_eq!(w.frames("copy"), vec![page_frame(COPY)]);
        assert_eq!(serde_json::to_string(&w.cached(PAGE_KEY).unwrap().body).unwrap(), page_body(false));
        assert_eq!(area.read_page(&e, REMOTE, 12, false).await.unwrap(), ReadOn { read: page(), here: false });
        assert_eq!(w.frames("copy").len(), 1);
        // a read that failed holds nothing
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| refused(None, GH_DOWN));
        let err = area.read_page(&e, REMOTE, 12, false).await.map(|_| ()).unwrap_err();
        assert_eq!(refusal(&err), (None, GH_DOWN.to_string()));
        assert!(w.cached(PAGE_KEY).is_none());
        w.assert_due(&[]);
    });
}

// ---- Q5, invariant 4

fn writes() -> Vec<(&'static str, FrameFor)> {
    vec![
        ("git.prReply", builder(|cwd| json!({ "op": "git.prReply", "cwd": cwd, "remote": REMOTE, "number": 12, "replyTo": 7, "body": "Fixed." }))),
        ("git.prResolve", builder(|cwd| json!({ "op": "git.prResolve", "cwd": cwd, "remote": REMOTE, "number": 12, "threadId": "PRRT_kwDOx", "resolved": true }))),
        ("git.prReact", builder(|cwd| json!({ "op": "git.prReact", "cwd": cwd, "remote": REMOTE, "number": 12, "subject": "IC_kwDOx", "content": "eyes", "on": true }))),
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

// Invariant 4: nothing posts as the person through a copy's gh. This computer has no command line for the host, the
// only live copy runs and would answer every write, and the copy's road is never asked: not a frame, not the road.
#[test]
fn invariant_4_post_as_person_never_takes_the_copy_road() {
    run(async {
        let (w, area, _e) = setup();
        let line = oracle("pull_requests")["posts"][0]["line"].clone();
        for (op, _) in writes() {
            w.here_says(op, |_| no_gh());
            w.copy_says(op, |f| ok(json!({ "op": f["op"].clone(), "posted": "through the copy" })));
        }
        for (op, frame_for) in writes() {
            let err = area.post_as_person(REMOTE, 12, frame_for, whole()).await.unwrap_err();
            assert_eq!(host_json(&err), json!({ "message": line }), "{op}");
        }
        let key = PAGE_KEY.to_string();
        let mut expected = vec![];
        for op in ["git.prReply", "git.prResolve", "git.prReact"] {
            expected.push(format!("cache.delete {key}"));
            expected.push("road here".to_string());
            expected.push(format!("frame here {}", write_frame(op)));
        }
        assert_eq!(w.log.borrow().clone(), expected);
        assert_eq!(w.roads("copy"), 0);
        assert!(w.frames("copy").is_empty());
    });
}

// Invariant 4: the held page goes first, before this computer is asked and whether or not the write lands.
#[test]
fn invariant_4_post_as_person_drops_the_held_page() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [] })));
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        w.cache.borrow_mut().insert("pr:https://github.com/wsp/pr-lab.git#12".into(), CacheEntry { body: json!({}), tag: Some(SEEN.into()), fetched_at: T0 });
        w.cache.borrow_mut().insert("page:https://github.com/wsp/other.git#12".into(), CacheEntry { body: json!({}), tag: None, fetched_at: T0 });
        w.log.borrow_mut().clear();
        let reply = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        assert_eq!(Value::Object(reply), ok(json!({ "subject": "IC_kwDOx", "reactions": [] })));
        assert_eq!(
            w.log.borrow().clone(),
            vec![format!("cache.delete {PAGE_KEY}"), "road here".to_string(), format!("frame here {}", write_frame("git.prReact"))]
        );
        let mut left: Vec<String> = w.cache.borrow().keys().cloned().collect();
        left.sort();
        assert_eq!(left, vec!["page:https://github.com/wsp/other.git#12".to_string(), "pr:https://github.com/wsp/pr-lab.git#12".to_string()]);
        // the next view reads again
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);

        // a write this computer refuses drops the page all the same
        w.here_says("git.prReact", |_| no_gh());
        w.log.borrow_mut().clear();
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
        assert_eq!(host_json(&err), json!({ "message": oracle("pull_requests")["posts"][0]["line"].clone() }));
        assert!(w.cached(PAGE_KEY).is_none());
        assert_eq!(w.log_of("cache.delete "), vec![PAGE_KEY.to_string()]);
        assert_eq!(w.log.borrow()[0], format!("cache.delete {PAGE_KEY}"));
    });
}

// Q5: any other failure rethrows whole; the rate-limit hold of the read road does not stop a write.
#[test]
fn q5_post_as_person_rethrows_other_failures_and_ignores_the_hold() {
    run(async {
        let (w, area, _e) = setup();
        w.here_says("git.prReact", |_| refused(Some("forbidden"), "gh said: Resource not accessible"));
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("forbidden".into()), "gh said: Resource not accessible".into()));
        assert_eq!(w.roads("copy"), 0);

        let runs = Rc::new(Cell::new(0));
        let _ = try_road(&area, &runs, "here", Err(refusal_error(Some("rate-limited"), RL_TEXT))).await.unwrap_err();
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [{ "content": "eyes", "count": 1, "mine": true }] })));
        w.log.borrow_mut().clear();
        let reply = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        assert_eq!(Value::Object(reply), ok(json!({ "subject": "IC_kwDOx", "reactions": [{ "content": "eyes", "count": 1, "mine": true }] })));
        assert_eq!(w.frames("here"), vec![write_frame("git.prReact")]);

        // a reply that does not parse is the parse's own failure
        let bad: Rc<dyn Fn(Frame) -> AccountResult<Frame>> = Rc::new(|_| Err(plain("not a reply").into()));
        let err = area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, bad).await.unwrap_err();
        assert_eq!(host_json(&err), json!({ "message": "not a reply" }));
    });
}

// Q5: hostOfRemote: the git host's first ssh host, else the remote's own host in lower case, else the remote, as the
// post line each shape of remote gets (the TS lower-cases only after its scheme strip, so an upper-case scheme is a host).
#[test]
fn q5_host_of_remote_and_the_post_line() {
    run(async {
        let posts = oracle("pull_requests")["posts"].clone();
        for p in posts.as_array().unwrap() {
            let remote = p["remote"].as_str().unwrap();
            assert_eq!(PullRequestsArea::host_of_remote(remote), p["host"].as_str().unwrap(), "{remote}");
            let (w, area, _e) = setup();
            w.here_says("git.prReact", |_| no_gh());
            let err = area.post_as_person(remote, 12, writes().pop().unwrap().1, whole()).await.unwrap_err();
            assert_eq!(host_json(&err), json!({ "message": p["line"].clone() }), "{remote}");
        }
    });
}

// ---- Q6

// Q6: PR_POLL_MS while a window watches, PR_POLL_IDLE_MS otherwise.
#[test]
fn q6_pr_poll_ms_follows_the_window() {
    let (w, area, _e) = setup();
    assert_eq!(area.pr_poll_ms(), IDLE);
    w.watched.set(true);
    assert_eq!(area.pr_poll_ms(), POLL);
    w.watched.set(false);
    assert_eq!(area.pr_poll_ms(), IDLE);
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
        vec!["persist".to_string(), "status".to_string(), "road here".to_string(), format!("frame here {}", pr_frame(HOME, json!({ "branch": "other" }))), "status".to_string()]
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
        (why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0), true, CHECKOUT_TTL - 1, true),
        (why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0), true, CHECKOUT_TTL, false),
        (why("no signed-in command line for github.com is on this computer, so the pull request is not read", 0), false, CHECKOUT_TTL, false),
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
        ("checks that finished", Some(cached_pr(Some(SEEN), open(json!({ "checks": [{ "name": "a", "state": "pass" }, { "name": "b", "state": "fail" }] })))), false, true),
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
    assert_eq!(w.log.borrow().clone(), vec!["road here".to_string(), format!("frame here {}", pr_frame(HOME, json!({ "number": 12, "seen": SEEN }))), "status".to_string()]);
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
        vec!["road here".to_string(), format!("frame here {here}"), "road copy".to_string(), format!("frame copy {copy}"), "status".to_string()]
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

// Q7: the table, one case per step of the spec.
#[test]
fn q7_read_pull_request_table() {
    run(async {
        q7_left_branch_drops_and_reads_again().await;
        q7_a_settled_record_is_never_read().await;
        q7_a_held_read_answers_held().await;
        q7_a_branch_with_no_pull_request_waits_for_its_head().await;
        q7_the_in_flight_mark().await;
        q7_the_read_frame_and_when_none_is_sent().await;
        q7_what_is_taken_from_the_answer().await;
        q7_a_failed_read().await;
    });
}

// Q7 step 5: a second reader joins the read in flight: one frame, one take, the same answer.
#[test]
fn q7_a_read_in_flight_is_joined() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here.hold.set(true);
        let (a1, e1) = (area.clone(), e.clone());
        let first = tokio::task::spawn_local(async move { a1.read_pull_request(&e1, true, false).await });
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        let (a2, e2) = (area.clone(), e.clone());
        let second = tokio::task::spawn_local(async move { a2.read_pull_request(&e2, true, false).await });
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        assert!(e.borrow().pr_reading.is_some());
        w.release("here");
        settle().await;
        let (one, two) = (first.await.unwrap(), second.await.unwrap());
        assert_eq!(pr_value(&one), fact(json!({}), T0));
        assert_eq!(pr_value(&two), fact(json!({}), T0));
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "branch": "fix/ci" }))]);
        assert_eq!(w.statuses.borrow().len(), 1);
        assert!(e.borrow().pr_reading.is_none());
    });
}

// Q7 step 5: a whole read does not join one in flight, and never sends what the last read saw.
#[test]
fn q7_whole_reads_alone() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        w.on_branch(&e, "fix/ci", "h1");
        w.cache.borrow_mut().insert(PullRequestsArea::pr_key(REMOTE, 12), CacheEntry { body: open(json!({})), tag: Some(SEEN.into()), fetched_at: T0 - 5 });
        w.here_says("git.prRead", |f| if f.contains_key("seen") { ok(json!({ "unchanged": true, "seen": SEEN })) } else { ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "fail" }] })), "seen": SEEN })) });
        w.here.hold.set(true);
        let (a1, e1) = (area.clone(), e.clone());
        let first = tokio::task::spawn_local(async move { a1.read_pull_request(&e1, true, false).await });
        settle().await;
        let (a2, e2) = (area.clone(), e.clone());
        let second = tokio::task::spawn_local(async move { a2.read_pull_request(&e2, true, true).await });
        settle().await;
        assert_eq!(
            w.frames("here"),
            vec![pr_frame(HOME, json!({ "number": 12, "seen": SEEN })), pr_frame(HOME, json!({ "number": 12 }))]
        );
        w.release("here");
        settle().await;
        let (one, two) = (first.await.unwrap(), second.await.unwrap());
        assert_eq!(pr_value(&one)["checks"][0]["state"], json!("pass"));
        assert_eq!(pr_value(&two)["checks"][0]["state"], json!("fail"));
        assert!(e.borrow().pr_reading.is_none());
    });
}

// ---- Q8

fn record_pr(e: &Entry) -> String {
    rec_of(e).get("pr").map(|v| v.to_string()).unwrap_or_else(|| "none".into())
}

// Q8: the record is written only where state, number or branch moved, in the TS's key order; checks only once settled.
#[test]
fn q8_take_writes_the_record_only_when_it_moved() {
    run(async {
        let (w, area, e) = setup();
        let written = |state_keys: Value| kept_pr(state_keys).to_string();

        // the first read of an open one writes it, without checks
        area.take_pull_request(&e, Some(seen_of(fact(json!({}), T0)))).await.unwrap();
        assert_eq!(record_pr(&e), written(json!({})));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"pr\":{}}}", written(json!({})))]);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "status".to_string()]);
        w.assert_due(&[T0 + IDLE]);

        // the same state, number and branch again: no write, the status pushed, the timer moved
        w.advance(1000);
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "headOid": "c0ffee" }), T0 + 1000)))).await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 1);
        assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0 + 1000));
        assert_eq!(w.statuses.borrow().len(), 2);
        w.assert_due(&[T0 + 1000 + IDLE]);

        // a move of branch alone writes
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "branch": "fix/two" }), T0 + 1000)))).await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 2);
        assert_eq!(record_pr(&e), kept_pr(json!({ "branch": "fix/two" })).to_string());

        // merged: mergedAt is the read's time, checks come with it, in that order; the timer goes; the tree settles
        let checks = json!([{ "name": "ci", "workflow": "ci", "state": "pass", "startedAt": "2026-10-02T10:00:00Z", "completedAt": "2026-10-02T10:16:00Z" }]);
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged", "checks": checks.clone() }), T0 + 2000)))).await.unwrap();
        let merged = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0 + 2000, "checks": checks.clone() });
        assert_eq!(record_pr(&e), merged.to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        w.assert_due(&[]);
        assert!(e.borrow().pr_poll.is_none());

        // closed: closedAt, not mergedAt
        let (w, area, e) = setup();
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "closed" }), T0)))).await.unwrap();
        let closed = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "closed", "base": "main", "branch": "fix/ci", "closedAt": T0, "checks": open(json!({}))["checks"].clone() });
        assert_eq!(record_pr(&e), closed.to_string());
        w.assert_due(&[]);

        // nothing, a why and a kept one set the entry and the status and touch no record
        for seen in [None, Some(seen_of(why("a sentence", T0))), Some(seen_of(json!({ "number": 12, "url": "u", "state": "open", "base": "main", "readAt": T0 })))] {
            let (w, area, e) = setup();
            keep(&e, json!({}));
            let want = pr_value(&seen);
            area.take_pull_request(&e, seen).await.unwrap();
            assert_eq!(pr_of(&e), want);
            assert_eq!(w.statuses.borrow().clone(), vec![want]);
            assert!(w.persisted.borrow().is_empty());
            assert_eq!(record_pr(&e), kept_pr(json!({})).to_string());
            w.assert_due(&[]);
        }

        // a settled record settles the tree even when this take was not the one that settled it
        let (w, area, e) = setup();
        keep(&e, json!({ "state": "merged", "mergedAt": T0 - 1 }));
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        area.take_pull_request(&e, None).await.unwrap();
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
    });
}

// Q8: what was sent belongs to the pull request it was read off.
#[test]
fn q8_a_new_number_drops_what_was_sent() {
    run(async {
        let (w, area, e) = setup();
        let sent = json!([{ "kind": "comment", "id": 5841958969_i64, "at": T0 }]);
        keep(&e, json!({}));
        set_record(&e, "prSent", sent.clone());
        // the same pull request read again keeps it
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        assert_eq!(rec_of(&e)["prSent"], sent);
        // another number starts with nothing sent
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" }), T0)))).await.unwrap();
        assert!(rec_of(&e).get("prSent").is_none());
        assert_eq!(record_pr(&e), kept_pr(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" })).to_string());
        let last = w.fields_persisted().pop().unwrap();
        assert_eq!(last, format!("{{\"pr\":{}}}", kept_pr(json!({ "number": 13, "url": "https://github.com/wsp/pr-lab/pull/13" }))));

        // with no pull request on the record yet there is nothing it belonged to: what was sent stays
        let (_w, area, e) = setup();
        set_record(&e, "prSent", sent.clone());
        area.take_pull_request(&e, Some(seen_of(fact(json!({}), T0)))).await.unwrap();
        assert_eq!(rec_of(&e)["prSent"], sent);
    });
}

// ---- Q9

// Q9: left_branch: only a shared folder with a kept pr and a checkout on a real branch.
#[test]
fn q9_left_branch_table() {
    // (label, record over, project base, project default branch, kept pr, checkout branch, left)
    let folder = json!({ "kind": "local", "machineId": "" });
    let place = json!({ "kind": "place", "machineId": "", "place": "box1" });
    let cloud = json!({});
    let worktree = json!({ "kind": "local", "machineId": "", "worktree": { "path": "/home/u/.wsp/trees/x", "branch": "fix/ci", "made": true } });
    let on_branch = Some(kept_pr(json!({})));
    let no_branch = Some(json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "open", "base": "main" }));
    let rows: Vec<(&str, Value, Option<&str>, &str, Option<Value>, Option<&str>, bool)> = vec![
        ("on its branch", folder.clone(), Some("main"), "main", on_branch.clone(), Some("fix/ci"), false),
        ("back on the base", folder.clone(), Some("main"), "main", on_branch.clone(), Some("main"), true),
        ("on another branch", folder.clone(), Some("main"), "main", on_branch.clone(), Some("other"), true),
        ("a joined computer's folder", place.clone(), Some("main"), "main", on_branch.clone(), Some("main"), true),
        ("a machine keeps its pr", cloud.clone(), Some("main"), "main", on_branch.clone(), Some("main"), false),
        ("a worktree keeps its pr", worktree.clone(), Some("main"), "main", on_branch.clone(), Some("main"), false),
        ("no pull request", folder.clone(), Some("main"), "main", None, Some("main"), false),
        ("no checkout read", folder.clone(), Some("main"), "main", on_branch.clone(), None, false),
        ("detached", folder.clone(), Some("main"), "main", on_branch.clone(), Some("(HEAD detached at abc1234)"), false),
        ("rebasing", folder.clone(), Some("main"), "main", on_branch.clone(), Some("(no branch, rebasing fix/ci)"), false),
        ("kept before branches, on the base", folder.clone(), Some("main"), "main", no_branch.clone(), Some("main"), true),
        ("kept before branches, on a branch", folder.clone(), Some("main"), "main", no_branch.clone(), Some("feature/x"), false),
        ("the record's base outranks", with(folder.clone(), json!({ "base": "develop" })), Some("main"), "main", no_branch.clone(), Some("develop"), true),
        ("the record's base, project's is not it", with(folder.clone(), json!({ "base": "develop" })), Some("main"), "main", no_branch.clone(), Some("main"), false),
        ("the project's base", folder.clone(), Some("trunk"), "main", no_branch.clone(), Some("trunk"), true),
        ("the project's base, default is not it", folder.clone(), Some("trunk"), "main", no_branch.clone(), Some("main"), false),
        ("the default branch when none is named", folder.clone(), None, "dev", no_branch.clone(), Some("dev"), true),
        ("the default branch, main is not it", folder.clone(), None, "dev", no_branch.clone(), Some("main"), false),
    ];
    for (label, over, base, default_branch, kept, branch, left) in rows {
        let (w, area, _e) = setup();
        w.set_project(base, default_branch, REMOTE);
        let e = w.entry(with(json!({ "id": "w9" }), over));
        if let Some(k) = kept {
            let mut v = rec_of(&e);
            v["pr"] = k;
            *e.borrow().record.borrow_mut() = parsed(v);
        }
        if let Some(b) = branch {
            e.borrow_mut().checkout = Some(checkout_json(b, "h1"));
        }
        assert_eq!(area.left_branch(&e), left, "{label}");
    }
}

// ---- Q10

// Q10: the poll, the entry's pr, the head mark, the record's pr and what was sent all go, then the record is kept and
// the status pushed.
#[test]
fn q10_drop_pull_request_clears_everything() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        set_record(&e, "prSent", json!([{ "kind": "comment", "id": 1, "at": T0 }]));
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tA"] }));
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        e.borrow_mut().pr_none_at = Some("h1".into());
        let (cancelled, cancel) = counted();
        e.borrow_mut().pr_poll = held(cancel);
        area.drop_pull_request(&e).await.unwrap();
        assert_eq!(cancelled.get(), 1);
        assert!(e.borrow().pr_poll.is_none());
        assert_eq!(pr_of(&e), Value::Null);
        assert_eq!(e.borrow().pr_none_at.clone(), None);
        assert_eq!(w.fields_persisted(), vec!["{\"branchThreads\":{\"branch\":\"fix/ci\",\"threads\":[\"tA\"]}}".to_string()]);
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "status".to_string()]);
        assert_eq!(w.statuses.borrow().clone(), vec![Value::Null]);
        // with nothing kept it still persists and pushes
        area.drop_pull_request(&e).await.unwrap();
        assert_eq!(w.log.borrow().len(), 4);
    });
}

// ---- Q11

// Q11: only a shared folder on a real branch; its base keeps nothing; another branch starts a new list.
#[test]
fn q11_ran_on_branch_table() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.folder();
        let kept = |e: &Entry| rec_of(e).get("branchThreads").map(|v| v.to_string()).unwrap_or_else(|| "none".into());

        // on its base, with nothing kept: nothing is written
        w.on_branch(&e, "main", "h1");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert!(w.persisted.borrow().is_empty());
        assert_eq!(kept(&e), "none");

        w.on_branch(&e, "fix/ci", "h1");
        area.ran_on_branch(&e, "tA").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/ci", "threads": ["tA"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 1);
        // the same thread again moves nothing
        area.ran_on_branch(&e, "tA").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 1);
        area.ran_on_branch(&e, "tB").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/ci", "threads": ["tA", "tB"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 2);
        assert_eq!(w.fields_persisted().last().unwrap(), &format!("{{\"branchThreads\":{}}}", json!({ "branch": "fix/ci", "threads": ["tA", "tB"] })));
        // a branch changed starts a new list
        w.on_branch(&e, "fix/two", "h2");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/two", "threads": ["tC"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        // a (-branch says nothing
        w.on_branch(&e, "(HEAD detached at abc1234)", "h3");
        area.ran_on_branch(&e, "tD").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "fix/two", "threads": ["tC"] }).to_string());
        assert_eq!(w.persisted.borrow().len(), 3);
        // back on its base removes what was kept, and persists once
        w.on_branch(&e, "main", "h4");
        area.ran_on_branch(&e, "tE").await.unwrap();
        assert_eq!(kept(&e), "none");
        assert_eq!(w.persisted.borrow().len(), 4);
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
        area.ran_on_branch(&e, "tE").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), 4);

        // a record's own base is the base
        set_record(&e, "base", json!("develop"));
        w.on_branch(&e, "main", "h5");
        area.ran_on_branch(&e, "tF").await.unwrap();
        assert_eq!(kept(&e), json!({ "branch": "main", "threads": ["tF"] }).to_string());

        // a worktree, a machine and a checkout never read keep nothing
        let tree = w.entry(json!({ "id": "w5", "kind": "local", "machineId": "", "worktree": { "path": "/p", "branch": "fix/ci", "made": true } }));
        let cloud = w.entry(json!({ "id": "w6" }));
        let bare = w.folder();
        w.on_branch(&tree, "fix/ci", "h1");
        w.on_branch(&cloud, "fix/ci", "h1");
        let before = w.persisted.borrow().len();
        area.ran_on_branch(&tree, "tA").await.unwrap();
        area.ran_on_branch(&cloud, "tA").await.unwrap();
        area.ran_on_branch(&bare, "tA").await.unwrap();
        assert_eq!(w.persisted.borrow().len(), before);
        assert_eq!((kept(&tree), kept(&cloud)), ("none".to_string(), "none".to_string()));
    });
}

// ---- Q12

// Q12: the poll arms only for an open fact on an entry still live; its interval and its first moment.
#[test]
fn q12_poll_intervals() {
    run(async {
        let pending_fact = fact(json!({ "checks": [{ "name": "ci", "state": "pending" }] }), T0);
        // open, no window, no slate: the idle interval
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + IDLE]);
        assert_eq!(e.borrow().pr_poll_ms, Some(IDLE as u64));
        assert!(e.borrow().pr_poll.is_some());
        // arming again cancels the first
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + IDLE]);
        // a window: the watched interval
        w.watched.set(true);
        area.poll_pull_request(&e, None);
        w.assert_due(&[T0 + POLL]);
        assert_eq!(e.borrow().pr_poll_ms, Some(POLL as u64));

        // a pending check read by a slate: thirty seconds, window or none; a slate with nothing pending, or
        // pending with no slate, keeps the pace of the window
        for (watched, pending, slate, ms) in [
            (false, true, true, SLATE_POLL),
            (true, true, true, SLATE_POLL),
            (false, true, false, IDLE),
            (true, true, false, POLL),
            (false, false, true, IDLE),
            (true, false, true, POLL),
        ] {
            let (w, area, e) = setup();
            w.watched.set(watched);
            w.slate_watches.set(slate);
            let f = if pending { pending_fact.clone() } else { fact(json!({}), T0) };
            e.borrow_mut().pr = Some(seen_of(f));
            area.poll_pull_request(&e, None);
            w.assert_due(&[T0 + ms]);
            assert_eq!(e.borrow().pr_poll_ms, Some(ms as u64));
        }

        // from: the interval counts from the last read; one already past fires at once
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, Some(T0 - 100_000));
        w.assert_due(&[T0 + IDLE - 100_000]);
        assert_eq!(e.borrow().pr_poll_ms, Some(IDLE as u64));
        area.poll_pull_request(&e, Some(T0 - 2 * IDLE));
        w.assert_due(&[T0]);

        // nothing is armed for anything but an open fact, and what was armed is cancelled
        for held in [Some(seen_of(fact(json!({ "state": "merged" }), T0))), Some(seen_of(fact(json!({ "state": "closed" }), T0))), Some(seen_of(why("w", T0))), None] {
            let (w, area, e) = setup();
            e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
            area.poll_pull_request(&e, None);
            assert_eq!(w.clock.pending(), 1);
            e.borrow_mut().pr = held;
            area.poll_pull_request(&e, None);
            w.assert_due(&[]);
            assert!(e.borrow().pr_poll.is_none());
        }

        // nor for an entry the live map no longer holds under its id, or holds another under
        let (w, area, e) = setup();
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        w.live.borrow_mut().shift_remove("w1");
        area.poll_pull_request(&e, None);
        w.assert_due(&[]);
        let other = w.cloud();
        assert!(!Rc::ptr_eq(&other, &e));
        area.poll_pull_request(&e, None);
        w.assert_due(&[]);
        assert!(e.borrow().pr_poll.is_none());
    });
}

// Q12: the timer clears itself and forces a read, which re-arms it.
#[test]
fn q12_the_timer_forces_a_read() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        area.poll_pull_request(&e, None);
        w.advance(IDLE - 1);
        settle().await;
        assert!(w.frames("here").is_empty());
        w.advance(1);
        settle().await;
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "number": 12 }))]);
        assert_eq!(pr_of(&e), fact(json!({}), T0 + IDLE));
        w.assert_due(&[T0 + 2 * IDLE]);
    });
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

// Q13: the roots are the workspace's rows with no root thread; the tree is the roots and every row under one, across
// workspaces; its unique keys are marked read and settled now, and a machine that pauses is napped. Nothing is deleted.
#[test]
fn q13_settle_marks_the_tree_and_naps() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        tree_views(&w);
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\",\"s4\"] {T0}"), "nap w1".to_string()]);
        assert!(w.persisted.borrow().is_empty());
        assert!(w.statuses.borrow().is_empty());

        // a machine that does not pause, or is not running, is not napped
        for (pauses, phase) in [(false, "running"), (true, "napping"), (true, "pausing")] {
            let (w, area, _e) = setup();
            let e = w.entry(json!({ "id": "w1", "phase": phase }));
            keep(&e, settled_pr());
            tree_views(&w);
            w.pauses.set(pauses);
            area.settle_tree(&e).await;
            settle().await;
            assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\",\"s4\"] {T0}")], "{pauses} {phase}");
        }

        // no roots: nothing; a workspace a thread opened: nothing
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s9", "w1", Some("tK"), Some("tOther"), "completed"));
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        set_record(&e, "parentThreadId", json!("tL"));
        tree_views(&w);
        w.pauses.set(true);
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
    });
}

// Q13: any running turn in the tree, on any workspace it spans, leaves it whole; a running row outside it does not.
#[test]
fn q13_a_running_turn_waits() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w2", Some("tB"), Some("tA"), "running"));
        area.settle_tree(&e).await;
        settle().await;
        assert!(w.log.borrow().is_empty());
        // the child's turn ends and its end settles the tree
        w.views.borrow_mut()[1] = sv("s2", "w2", Some("tB"), Some("tA"), "completed");
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log.borrow().clone(), vec![format!("mark [\"tA\",\"tB\"] {T0}"), "nap w1".to_string()]);
        // a running root of the tree itself waits too
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "running"));
        area.settle_tree(&e).await;
        assert!(w.log.borrow().is_empty());
        // a running row of another tree does not
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w3", Some("tZ"), None, "running"));
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
    });
}

// Q13: in a shared folder only the threads kept for the pull request's branch settle.
#[test]
fn q13_a_folder_settles_only_its_branch_threads() {
    run(async {
        let rows = |w: &World| {
            let mut v = w.views.borrow_mut();
            v.push(sv("s1", "w1", Some("tA"), None, "completed"));
            v.push(sv("s2", "w1", Some("tB"), None, "completed"));
            v.push(sv("s3", "w1", Some("tC"), None, "completed"));
            v.push(sv("s4", "w1", Some("tD"), Some("tA"), "completed"));
        };
        // the thread that ran there, and what is under it
        let (w, area, _e) = setup();
        let e = w.folder();
        keep(&e, settled_pr());
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tA"] }));
        rows(&w);
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\",\"tD\"] {T0}")]);

        // several
        set_record(&e, "branchThreads", json!({ "branch": "fix/ci", "threads": ["tB", "tC"] }));
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark ").last().unwrap(), &format!("[\"tB\",\"tC\"] {T0}"));

        // kept for another branch, kept for none, or a pull request with no branch: nobody
        for (threads, pr) in [(Some(json!({ "branch": "fix/other", "threads": ["tA"] })), settled_pr()), (None, settled_pr()), (Some(json!({ "branch": "fix/ci", "threads": ["tA"] })), json!({ "state": "merged", "mergedAt": T0 - 1, "branch": null }))] {
            let (w, area, _e) = setup();
            let e = w.folder();
            let mut v = rec_of(&e);
            v["pr"] = with(kept_pr(json!({})), pr.clone());
            if pr["branch"].is_null() {
                v["pr"].as_object_mut().unwrap().remove("branch");
            }
            if let Some(t) = threads {
                v["branchThreads"] = t;
            }
            *e.borrow().record.borrow_mut() = parsed(v);
            rows(&w);
            area.settle_tree(&e).await;
            assert!(w.log.borrow().is_empty());
        }

        // a worktree is its own branch: every root settles
        let (w, area, _e) = setup();
        let e = w.entry(json!({ "kind": "local", "machineId": "", "worktree": { "path": "/p", "branch": "fix/ci", "made": true } }));
        keep(&e, settled_pr());
        rows(&w);
        area.settle_tree(&e).await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\",\"tB\",\"tC\",\"tD\"] {T0}")]);
    });
}

// Q13: a mark that fails and a nap that fails each warn once, in the TS's words; the nap is tried after a failed mark.
#[test]
fn q13_a_failed_mark_or_nap_warns_whole_lines() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, settled_pr());
        w.views.borrow_mut().push(sv("s1", "w1", Some("tA"), None, "completed"));
        w.pauses.set(true);
        *w.mark_error.borrow_mut() = Some("boom".into());
        *w.nap_error.borrow_mut() = Some("no machine".into());
        area.settle_tree(&e).await;
        settle().await;
        let warns = oracle("pull_requests")["warns"].clone();
        let got: Vec<String> = w.log_of("warn ");
        assert_eq!(json!(got), warns);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
    });
}

// ---- Q14

// Q14: the repository settings of a remote, held an hour, read on this computer through the rate limit hold.
#[test]
fn q14_merge_settings_held_an_hour() {
    run(async {
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| ok(merge_json()));
        let frame_for = |remote: &str| json!({ "op": "git.repoRead", "cwd": HOME, "remote": remote }).to_string();
        let want = parsed::<GitRepoReadReply>(merge_json());
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here"), vec![frame_for(REMOTE)]);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        w.advance(SETTINGS_MS - 1);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 1);
        w.advance(1);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 2);
        // another remote is its own
        let other = "https://github.com/wsp/other.git";
        area.merge_settings(other).await.unwrap();
        assert_eq!(w.frames("here").last().unwrap(), &frame_for(other));
        area.merge_settings(REMOTE).await.unwrap();
        assert_eq!(w.frames("here").len(), 3);

        // a refusal, and a reply that does not parse, are not held
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| refused(None, "no"));
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (None, "no".to_string()));
        w.here_says("git.repoRead", |_| ok(json!({ "methods": "squash" })));
        assert!(area.merge_settings(REMOTE).await.is_err());
        w.here_says("git.repoRead", |_| ok(merge_json()));
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 3);

        // a rate limit holds the road: the next ask runs nothing for a minute
        let (w, area, _e) = setup();
        w.here_says("git.repoRead", |_| rate_limited());
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        w.here_says("git.repoRead", |_| ok(merge_json()));
        let err = area.merge_settings(REMOTE).await.unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        assert_eq!(w.frames("here").len(), 1);
        w.advance(HOLD);
        assert_eq!(area.merge_settings(REMOTE).await.unwrap(), want);
        assert_eq!(w.frames("here").len(), 2);
    });
}

// ---- Q15

fn update_json(merged: bool, commits: i64, conflicts: Value) -> Value {
    json!({ "base": "main", "merged": merged, "commits": commits, "conflicts": conflicts })
}

// Q15: git.update on the copy's checkout with the base, through the workspace's queue; then the checkout read, forced
// and awaited, and the pull request read, forced and not awaited.
#[test]
fn q15_update_copy_reads_after() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        e.borrow_mut().checkout.take();
        w.copy_says("git.update", |_| ok(update_json(false, 0, json!(["README.md"]))));
        // the pull request read waits on this computer; the update answers regardless
        w.here.hold.set(true);
        let got = area.update_copy(&e).await.unwrap();
        assert_eq!(got, parsed::<GitUpdateReply>(update_json(false, 0, json!(["README.md"]))));
        settle().await;
        let update = json!({ "op": "git.update", "cwd": COPY, "base": "main" }).to_string();
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "queued w1".to_string(),
                "road copy".to_string(),
                format!("frame copy {update}"),
                "checkout true".to_string(),
                "road here".to_string(),
                format!("frame here {}", pr_frame(HOME, json!({ "branch": "fix/ci" }))),
            ]
        );
        w.release("here");
        settle().await;
        assert_eq!(pr_of(&e)["number"], json!(12));

        // the record's base outranks the project's; none anywhere sends none
        let (w, area, e) = setup();
        set_record(&e, "base", json!("develop"));
        w.copy_says("git.update", |_| ok(update_json(true, 2, json!([]))));
        area.update_copy(&e).await.unwrap();
        assert_eq!(w.frames("copy"), vec![json!({ "op": "git.update", "cwd": COPY, "base": "develop" }).to_string()]);
        let (w, area, e) = setup();
        w.set_project(None, "main", REMOTE);
        w.copy_says("git.update", |_| ok(update_json(true, 2, json!([]))));
        area.update_copy(&e).await.unwrap();
        assert_eq!(w.frames("copy"), vec![json!({ "op": "git.update", "cwd": COPY }).to_string()]);

        // a failed update reads nothing after it
        let (w, area, e) = setup();
        w.copy_says("git.update", |_| refused(None, "git said: conflict"));
        let err = area.update_copy(&e).await.unwrap_err();
        assert_eq!(refusal(&err), (None, "git said: conflict".to_string()));
        settle().await;
        assert_eq!(w.log_of("checkout "), Vec::<String>::new());
        assert!(w.frames("here").is_empty());
    });
}

// ---- Q16

// Q16: the child's record keeps what it was given, or drops it when it is empty; then the lead's rows are read.
#[test]
fn q16_keep_tree_sets_or_removes_and_reads_the_lead() {
    run(async {
        let (w, area, child) = setup();
        let why = "no credential here for git.example.com; sign in and bring back again";
        area.keep_tree(&child, parsed::<TreeRecord>(json!({ "pushRefused": why }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "pushRefused": why }));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"tree\":{}}}", json!({ "pushRefused": why }))]);
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "lead w1".to_string()]);
        // empty removes it
        area.keep_tree(&child, parsed::<TreeRecord>(json!({}))).await.unwrap();
        assert!(rec_of(&child).get("tree").is_none());
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
        assert_eq!(w.log.borrow().len(), 4);
        // a merge and the files a merge stopped on
        let merged = json!({ "merged": { "oid": "d00d", "at": T0, "head": "c0de" } });
        area.keep_tree(&child, parsed::<TreeRecord>(merged.clone())).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], merged);
        let stopped = json!({ "conflicts": ["lead.txt"] });
        area.keep_tree(&child, parsed::<TreeRecord>(stopped.clone())).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], stopped);
        assert_eq!(w.persisted.borrow().len(), 4);
        assert_eq!(w.log_of("lead "), vec!["w1".to_string(); 4]);
    });
}

// ---- ported from the TS tests, at the area's seam

// TS: test/pull-request.test.ts:215
#[test]
fn ts_a_read_goes_over_this_computer_with_the_projects_remote_and_the_copys_branch() {
    run(async {
        let (w, area, e) = setup();
        *w.checkout_answer.borrow_mut() = Some(checkout_json("fix/ci", "abc"));
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), fact(json!({}), T0));
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "checkout false".to_string(),
                "road here".to_string(),
                format!("frame here {}", json!({ "op": "git.prRead", "cwd": HOME, "remote": REMOTE, "branch": "fix/ci" })),
                "persist".to_string(),
                "status".to_string(),
            ]
        );
        assert_eq!(w.roads("copy"), 0);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        assert_eq!(w.fields_persisted(), vec![format!("{{\"pr\":{}}}", kept_pr(json!({})))]);
        w.assert_due(&[T0 + IDLE]);
    });
}

// TS: test/pull-request.test.ts:231
#[test]
fn ts_the_copy_is_asked_only_where_it_runs_and_a_stopped_one_is_never_woken() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "number": 3 })) })));
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), fact(json!({ "number": 3 }), T0));
        assert_eq!(
            w.log.borrow().clone(),
            vec![
                "road here".to_string(),
                format!("frame here {}", pr_frame(HOME, json!({ "branch": "fix/ci" }))),
                "road copy".to_string(),
                format!("frame copy {}", pr_frame(COPY, json!({ "branch": "fix/ci" }))),
                "persist".to_string(),
                "status".to_string(),
            ]
        );
        // napped, a read after the poll interval says why it is not read, and the copy is not asked
        set_record(&e, "phase", json!("napping"));
        let armed = e.borrow_mut().pr_poll.take();
        armed.expect("the timer a read armed")();
        w.advance(IDLE);
        w.log.borrow_mut().clear();
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), why(oracle("pull_requests")["stopped"].as_str().unwrap(), T0 + IDLE));
        assert_eq!(w.log.borrow().clone(), vec!["road here".to_string(), format!("frame here {}", pr_frame(HOME, json!({ "number": 3 }))), "status".to_string()]);
    });
}

// TS: test/pull-request.test.ts:251
#[test]
fn ts_neither_road_has_a_command_line_so_the_pull_request_is_not_read() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| no_gh());
        w.copy_says("git.prRead", |_| no_gh());
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), why(oracle("pull_requests")["unread"].as_str().unwrap(), T0));
        assert_eq!(w.statuses.borrow().clone(), vec![why(oracle("pull_requests")["unread"].as_str().unwrap(), T0)]);
    });
}

// TS: test/pull-request.test.ts:259
#[test]
fn ts_a_failed_read_keeps_the_fact_and_reads_again_on_its_timer() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        answer_pr(&w, open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })));
        area.read_pull_request(&e, false, false).await;
        let held = fact(json!({ "checks": [{ "name": "ci", "state": "pass" }] }), T0);
        assert_eq!(pr_of(&e), held);
        w.here_says("git.prRead", |_| refused(None, GH_DOWN));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12 })));
        assert_eq!(pr_of(&e), held);
        assert_eq!(w.statuses.borrow().len(), 1);
        assert_eq!(w.persisted.borrow().len(), 1);
        w.assert_due(&[T0 + 2 * IDLE]);
        answer_pr(&w, open(json!({ "state": "merged" })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        assert_eq!(pr_of(&e), fact(json!({ "state": "merged" }), T0 + 2 * IDLE));
        assert_eq!(w.persisted.borrow().len(), 2);
        w.assert_due(&[]);
    });
}

// TS: test/pull-request.test.ts:277
#[test]
fn ts_merged_and_closed_are_recorded_as_the_host_says_them() {
    run(async {
        for (state, at_key) in [("merged", "mergedAt"), ("closed", "closedAt")] {
            let (w, area, e) = setup();
            w.on_branch(&e, "fix/ci", "h1");
            answer_pr(&w, open(json!({ "state": state })));
            area.read_pull_request(&e, false, false).await;
            assert_eq!(pr_of(&e), fact(json!({ "state": state }), T0));
            let mut kept = kept_pr(json!({ "state": state }));
            kept[at_key] = json!(T0);
            kept["checks"] = open(json!({}))["checks"].clone();
            assert_eq!(record_pr(&e), kept.to_string(), "{state}");
            w.assert_due(&[]);
        }
    });
}

// TS: test/pull-request.test.ts:285
#[test]
fn ts_an_open_one_is_read_on_its_timer_whatever_its_checks_say_and_never_again_once_merged() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        answer_pr(&w, open(json!({ "checks": [] })));
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        answer_pr(&w, open(json!({ "checks": [{ "name": "ci", "state": "pass" }] })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        answer_pr(&w, open(json!({ "state": "merged", "checks": [{ "name": "ci", "state": "pass" }] })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        assert_eq!(pr_of(&e)["state"], json!("merged"));
        assert_eq!(w.persisted.borrow().len(), 2);
        // never again, whatever asks
        w.advance(3 * IDLE);
        w.advance(20_000);
        settle().await;
        area.read_pull_request(&e, false, false).await;
        area.read_pull_request(&e, true, true).await;
        assert_eq!(w.frames("here").len(), 3);
        w.assert_due(&[]);
    });
}

// TS: test/pull-request.test.ts:317
#[test]
fn ts_a_read_asks_with_what_the_last_read_saw_and_an_unchanged_answer_keeps_the_fact() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({})), "seen": SEEN })));
        area.read_pull_request(&e, false, false).await;
        let key = PullRequestsArea::pr_key(REMOTE, 12);
        let held = w.cached(&key).unwrap();
        assert_eq!((held.tag.clone(), held.fetched_at), (Some(SEEN.to_string()), T0));
        w.here_says("git.prRead", |_| ok(json!({ "unchanged": true, "seen": SEEN })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e), fact(json!({}), T0 + IDLE));
        assert_eq!(w.persisted.borrow().len(), 1);
        // a changed one answers in full with what it saw now, which the next read carries
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "headOid": "c0ffee" })), "seen": MOVED })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[2], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e), fact(json!({ "headOid": "c0ffee" }), T0 + 2 * IDLE));
        assert_eq!(w.cached(&key).unwrap().tag, Some(MOVED.to_string()));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here")[3], pr_frame(HOME, json!({ "number": 12, "seen": MOVED })));
    });
}

// TS: test/pull-request.test.ts:340
#[test]
fn ts_a_check_still_running_is_asked_for_in_full() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prRead", |_| ok(json!({ "pr": open(json!({ "checks": [{ "name": "ci", "state": "pending" }] })), "seen": SEEN })));
        area.read_pull_request(&e, false, false).await;
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12 })));
    });
}

// TS: test/pull-request.test.ts:351
#[test]
fn ts_an_open_one_is_read_every_poll_ms_with_a_window_and_every_idle_ms_with_none() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 1);
        w.advance(IDLE - POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        // a window opens just after that read: the sidebar's ask reads nothing, and the timer moves to three minutes
        w.watched.set(true);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), 2);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 3);
        w.advance(POLL);
        settle().await;
        assert_eq!(w.frames("here").len(), 4);
        w.advance(CHECKOUT_TTL + 1);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), 4);
    });
}

// TS: test/pull-request.test.ts:386
#[test]
fn ts_a_branch_with_no_pull_request_is_read_again_on_view_only_once_its_head_moves() {
    run(async {
        let (w, area, e) = setup();
        answer_none(&w);
        w.on_branch(&e, "fix/ci", "abc1234");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 1);
        for _ in 0..3 {
            w.advance(CHECKOUT_TTL + 1);
            area.read_pull_request(&e, false, false).await;
        }
        assert_eq!(w.frames("here").len(), 1);
        w.advance(CHECKOUT_TTL + 1);
        w.on_branch(&e, "fix/ci", "def0123");
        area.read_pull_request(&e, false, false).await;
        assert_eq!(w.frames("here").len(), 2);
    });
}

// TS: test/pull-request.test.ts:412
#[test]
fn ts_a_rate_limit_holds_pages_and_status_reads_on_that_road_for_a_minute() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.here_says("git.prView", |_| rate_limited());
        area.read_pull_request(&e, false, false).await;
        let reads = w.frames("here").len();
        w.advance(IDLE - 30_000);
        let err = area.read_page(&e, REMOTE, 12, false).await.map(|_| ()).unwrap_err();
        assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        for _ in 0..3 {
            let err = area.read_page(&e, REMOTE, 12, true).await.map(|_| ()).unwrap_err();
            assert_eq!(refusal(&err), (Some("rate-limited".into()), RL_TEXT.into()));
        }
        let views = |w: &World| w.frames("here").iter().filter(|f| f.contains("git.prView")).count();
        assert_eq!(views(&w), 1);
        // the status timer falling inside it asks no more, and the fact it held stands
        w.advance(30_000);
        settle().await;
        assert_eq!(w.frames("here").len(), reads + 1);
        assert_eq!(pr_of(&e), fact(json!({}), T0));
        w.here_says("git.prView", |_| ok(page_json()));
        w.advance(HOLD - 30_000);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(views(&w), 2);
    });
}

// TS: test/pull-request.test.ts:439
#[test]
fn ts_a_merged_pull_request_stamps_a_quiet_tree_and_naps_its_machine() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tL"), None, "completed"));
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        answer_pr(&w, open(json!({ "state": "merged" })));
        w.advance(IDLE);
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tL\"] {}", T0 + IDLE)]);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
        let marked = w.log.borrow().iter().position(|l| l.starts_with("mark ")).unwrap();
        let napped = w.log.borrow().iter().position(|l| l.starts_with("nap ")).unwrap();
        assert!(marked < napped);
        // restored, nothing is read again: a merged one is never read
        let reads = w.frames("here").len();
        w.advance(CHECKOUT_TTL + 1);
        area.read_pull_request(&e, false, false).await;
        settle().await;
        assert_eq!(w.frames("here").len(), reads);
    });
}

// TS: test/pull-request.test.ts:467
#[test]
fn ts_a_project_folder_keeps_the_pull_request_with_its_branch_and_loses_it_when_it_leaves() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.folder();
        w.here_says("git.prRead", |f| {
            if f.get("branch") == Some(&json!("fix/ci")) || f.get("number") == Some(&json!(12)) {
                ok(json!({ "pr": open(json!({ "state": "merged" })) }))
            } else {
                ok(json!({}))
            }
        });
        for t in ["tC", "tA", "tB"] {
            w.views.borrow_mut().push(sv(&format!("s{t}"), "w1", Some(t), None, "completed"));
        }
        // C ran on main before the folder went to the branch: nothing is kept for it
        w.on_branch(&e, "main", "h0");
        area.ran_on_branch(&e, "tC").await.unwrap();
        assert!(w.persisted.borrow().is_empty());
        // A's turn ended on the branch
        w.on_branch(&e, "fix/ci", "h1");
        area.ran_on_branch(&e, "tA").await.unwrap();
        area.read_pull_request(&e, true, false).await;
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tA\"] {T0}")]);
        let kept = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0, "checks": open(json!({}))["checks"].clone() });
        assert_eq!(rec_of(&e)["pr"], kept);
        assert_eq!(rec_of(&e)["branchThreads"], json!({ "branch": "fix/ci", "threads": ["tA"] }));
        // B ends back on main: the folder keeps none of the base's threads, the merged pull request is not its any more
        w.on_branch(&e, "main", "h2");
        area.ran_on_branch(&e, "tB").await.unwrap();
        assert!(rec_of(&e).get("branchThreads").is_none());
        let got = area.read_pull_request(&e, false, false).await;
        assert_eq!(pr_value(&got), Value::Null);
        settle().await;
        assert!(rec_of(&e).get("pr").is_none());
        assert_eq!(w.log_of("mark ").len(), 1);
        assert_eq!(w.fields_persisted().last().unwrap(), "{}");
    });
}

// TS: test/pull-request.test.ts:513, test/worktree-sweep.test.ts:329
#[test]
fn ts_a_fork_pull_request_on_a_worktree_is_read_by_number_alone_and_kept() {
    run(async {
        let (w, area, _e) = setup();
        let e = w.entry(json!({ "kind": "local", "machineId": "", "worktree": { "path": "/home/u/.wsp/trees/feat", "branch": "ana/feat", "made": true } }));
        keep(&e, json!({ "branch": "feat" }));
        w.on_branch(&e, "ana/feat", "h1");
        w.here_says("git.prRead", |f| if f.get("number") == Some(&json!(12)) { ok(json!({ "pr": open(json!({ "branch": "feat", "fork": { "owner": "ana", "pushable": true } })) })) } else { ok(json!({})) });
        for _ in 0..3 {
            area.read_pull_request(&e, true, false).await;
        }
        assert!(!area.left_branch(&e));
        assert_eq!(w.frames("here"), vec![pr_frame(HOME, json!({ "number": 12 })); 3]);
        assert_eq!(rec_of(&e)["pr"]["number"], json!(12));
        assert!(w.persisted.borrow().is_empty());
    });
}

// TS: test/pull-request.test.ts:544
#[test]
fn ts_a_merged_tree_stays_live_while_a_child_works_and_is_stamped_when_it_ends() {
    run(async {
        let (w, area, e) = setup();
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w1", Some("tL"), None, "completed"));
        w.views.borrow_mut().push(sv("s2", "w1", Some("tC"), Some("tL"), "running"));
        area.take_pull_request(&e, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        assert!(w.log_of("nap ").is_empty());
        w.views.borrow_mut()[1] = sv("s2", "w1", Some("tC"), Some("tL"), "completed");
        area.settle_tree(&e).await;
        settle().await;
        assert_eq!(w.log_of("mark "), vec![format!("[\"tL\",\"tC\"] {T0}")]);
        assert_eq!(w.log_of("nap "), vec!["w1".to_string()]);
    });
}

// TS: test/pull-request.test.ts:564
#[test]
fn ts_a_workspace_a_thread_opened_settles_nothing_off_its_own_pull_request() {
    run(async {
        let (w, area, _e) = setup();
        let child = w.entry(json!({ "id": "w2", "parentThreadId": "tL", "rootThreadId": "tL" }));
        w.pauses.set(true);
        w.views.borrow_mut().push(sv("s1", "w2", Some("tK"), None, "completed"));
        area.take_pull_request(&child, Some(seen_of(fact(json!({ "state": "merged" }), T0)))).await.unwrap();
        area.settle_tree(&child).await;
        settle().await;
        assert!(w.log_of("mark ").is_empty());
        assert!(w.log_of("nap ").is_empty());
        assert_eq!(rec_of(&child)["pr"]["state"], json!("merged"));
    });
}

// TS: test/pull-request.test.ts:610
#[test]
fn ts_a_page_write_and_the_settings_cost_one_read_each_within_their_holds() {
    run(async {
        let (w, area, e) = setup();
        w.here_says("git.prView", |_| ok(page_json()));
        w.here_says("git.repoRead", |_| ok(merge_json()));
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [] })));
        area.merge_settings(REMOTE).await.unwrap();
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here"), vec![json!({ "op": "git.repoRead", "cwd": HOME, "remote": REMOTE }).to_string(), page_frame(HOME)]);
        w.advance(PAGE_HOLD - 1);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        assert_eq!(w.frames("here").len(), 2);
        w.advance(1);
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        area.read_page(&e, REMOTE, 12, true).await.unwrap();
        assert_eq!(w.frames("here").len(), 4);
        area.post_as_person(REMOTE, 12, writes().pop().unwrap().1, whole()).await.unwrap();
        area.read_page(&e, REMOTE, 12, false).await.unwrap();
        area.merge_settings(REMOTE).await.unwrap();
        let frames = w.frames("here");
        assert_eq!(frames.iter().filter(|f| f.contains("git.prView")).count(), 4);
        assert_eq!(frames.iter().filter(|f| f.contains("git.repoRead")).count(), 1);
    });
}

// TS: test/pull-request.test.ts:751
#[test]
fn ts_a_reply_a_resolve_and_a_reaction_post_as_the_person_over_this_computers_daemon() {
    run(async {
        let (w, area, _e) = setup();
        let comment = json!({ "id": 9, "nodeId": "IC_kwDOx", "author": "Zingzy", "association": "owner", "bot": false, "body": "Fixed.", "url": "u", "at": "t" });
        let c = comment.clone();
        w.here_says("git.prReply", move |_| ok(json!({ "comment": c.clone() })));
        w.here_says("git.prResolve", |f| ok(json!({ "threadId": f["threadId"].clone(), "resolved": f["resolved"].clone() })));
        w.here_says("git.prReact", |f| ok(json!({ "subject": f["subject"].clone(), "reactions": [{ "content": "eyes", "count": 1, "mine": true }] })));
        let mut got = vec![];
        for (_, frame_for) in writes() {
            got.push(Value::Object(area.post_as_person(REMOTE, 12, frame_for, whole()).await.unwrap()));
        }
        assert_eq!(got[0]["comment"], comment);
        assert_eq!((got[1]["threadId"].clone(), got[1]["resolved"].clone()), (json!("PRRT_kwDOx"), json!(true)));
        assert_eq!(got[2]["reactions"], json!([{ "content": "eyes", "count": 1, "mine": true }]));
        assert_eq!(w.frames("here"), vec![write_frame("git.prReply"), write_frame("git.prResolve"), write_frame("git.prReact")]);
        assert_eq!(w.roads("copy"), 0);
    });
}

// TS: test/pull-request.test.ts:804
#[test]
fn ts_a_settled_record_keeps_the_checks_the_read_that_saw_it_settle_found() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        area.read_pull_request(&e, false, false).await;
        assert!(rec_of(&e)["pr"].get("checks").is_none());
        let checks = json!([{ "name": "ci", "workflow": "ci", "state": "pass", "startedAt": "2026-10-02T10:00:00Z", "completedAt": "2026-10-02T10:16:00Z" }]);
        answer_pr(&w, open(json!({ "state": "merged", "checks": checks.clone() })));
        w.advance(IDLE);
        settle().await;
        let merged = json!({ "number": 12, "url": "https://github.com/wsp/pr-lab/pull/12", "state": "merged", "base": "main", "branch": "fix/ci", "mergedAt": T0 + IDLE, "checks": checks });
        assert_eq!(rec_of(&e)["pr"], merged);
        assert_eq!(record_pr(&e), merged.to_string());
    });
}

// TS: test/pull-request.test.ts:846
#[test]
fn ts_a_whole_read_never_asks_with_what_the_last_read_saw() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        let checks = Rc::new(RefCell::new(json!([{ "name": "ci", "workflow": "ci", "state": "pass", "run": { "runId": 36, "jobId": 109 } }])));
        let c = checks.clone();
        w.here_says("git.prRead", move |f| if f.get("seen") == Some(&json!(SEEN)) { ok(json!({ "unchanged": true, "seen": SEEN })) } else { ok(json!({ "pr": open(json!({ "checks": c.borrow().clone() })), "seen": SEEN })) });
        area.read_pull_request(&e, false, false).await;
        *checks.borrow_mut() = json!([{ "name": "ci", "workflow": "ci", "state": "fail", "run": { "runId": 36, "jobId": 109 } }]);
        w.advance(5_000);
        // a forced read asks with what it saw and is told nothing moved
        area.read_pull_request(&e, true, false).await;
        assert_eq!(w.frames("here")[1], pr_frame(HOME, json!({ "number": 12, "seen": SEEN })));
        assert_eq!(pr_of(&e)["checks"][0]["state"], json!("pass"));
        // a whole one does not
        let got = area.read_pull_request(&e, true, true).await;
        assert_eq!(w.frames("here")[2], pr_frame(HOME, json!({ "number": 12 })));
        assert_eq!(pr_value(&got)["checks"][0]["state"], json!("fail"));
        assert_eq!(pr_of(&e)["readAt"], json!(T0 + 5_000));
    });
}

// TS: test/pull-request.test.ts:867
#[test]
fn ts_update_answers_a_clean_merge_and_the_conflicts_through_the_copys_daemon() {
    run(async {
        let (w, area, e) = setup();
        w.on_branch(&e, "fix/ci", "h1");
        let answer = Rc::new(RefCell::new(update_json(true, 2, json!([]))));
        let a = answer.clone();
        w.copy_says("git.update", move |_| ok(a.borrow().clone()));
        assert_eq!(area.update_copy(&e).await.unwrap(), parsed::<GitUpdateReply>(update_json(true, 2, json!([]))));
        *answer.borrow_mut() = update_json(false, 0, json!(["README.md"]));
        assert_eq!(area.update_copy(&e).await.unwrap(), parsed::<GitUpdateReply>(update_json(false, 0, json!(["README.md"]))));
        let update = json!({ "op": "git.update", "cwd": COPY, "base": "main" }).to_string();
        assert_eq!(w.frames("copy"), vec![update.clone(), update]);
        assert_eq!(w.log_of("queued "), vec!["w1".to_string(), "w1".to_string()]);
    });
}

// TS: test/tree.test.ts:351, :418, :432
#[test]
fn ts_the_tree_a_child_keeps_is_what_its_lead_reads() {
    run(async {
        let (w, area, child) = setup();
        let refused = "no credential here for git.example.com; sign in and bring back again";
        area.keep_tree(&child, parsed(json!({ "pushRefused": refused }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"]["pushRefused"], json!(refused));
        area.keep_tree(&child, parsed(json!({}))).await.unwrap();
        assert!(rec_of(&child).get("tree").is_none());
        area.keep_tree(&child, parsed(json!({ "conflicts": ["lead.txt"] }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "conflicts": ["lead.txt"] }));
        area.keep_tree(&child, parsed(json!({ "merged": { "oid": "d00d", "at": T0 } }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"], json!({ "merged": { "oid": "d00d", "at": T0 } }));
        assert!(rec_of(&child)["tree"].get("conflicts").is_none());
        area.keep_tree(&child, parsed(json!({ "merged": { "oid": "d00d", "at": T0, "head": "c0de" } }))).await.unwrap();
        assert_eq!(rec_of(&child)["tree"]["merged"]["head"], json!("c0de"));
        assert_eq!(w.log_of("lead ").len(), 5);
    });
}
