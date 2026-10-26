// SPDX-License-Identifier: AGPL-3.0-only
//! `FakeReview`: the context the start and review doors read, over a `FakeBase`. Each method logs
//! `base.call("<method>", args)` and answers from the script under its own name, with the default noted at the
//! method; a scripted `err_answer(..)` is the method's failure. Channels answer a `send` with `{"channel": <name>}`
//! so a test reads which road a channel came from, and `heard` keeps the listener the shared channel was given.
#![allow(dead_code, unused_imports)]
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::projects::start_from::{OpenAsk, Placed, StartKind};
use wsp_account::projects::workspaces::review::ReviewCtx;
use wsp_account::projects::workspaces::seam::{Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld};
use wsp_core::clock::Clock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_core::ids::Ids;
use wsp_core::types::api::DaemonEventListener;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{Detach, SysListener, WorkspaceRecord};
use wsp_wire::types::pull_request::PullRequest;
use wsp_wire::types::start::{IssueRead, WorkspaceFrom, WorkspaceFromKind};
use wsp_wire::views::harness::StartResult;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, DaemonReachView, PortProbeView, PortReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::base::{merge, record, view_of, BaseOf, FakeBase};

pub const HEAD: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";

pub fn pr_json(repo: &str, number: i64, over: Value) -> Value {
    let mut v = json!({
        "number": number, "url": format!("https://github.com/{repo}/pull/{number}"), "state": "open", "host": "github.com",
        "draft": false, "base": "main", "branch": "lab/review-me", "headOid": HEAD, "headSubject": "Rename the status word",
        "mergeable": "mergeable", "mergeState": "clean", "review": "none", "checks": [], "additions": 2, "deletions": 1,
        "changedFiles": 2, "commits": 1, "author": "maya",
    });
    merge(&mut v, over);
    v
}

pub fn issue_json(repo: &str, number: i64, title: &str) -> Value {
    json!({ "number": number, "url": format!("https://github.com/{repo}/issues/{number}"), "title": title,
        "body": "The first line should say hello.", "state": "OPEN", "comments": [] })
}

/// A review draft that reads, as the TS tests' `REVIEW` kept it: two comments, the second off the diff.
pub fn draft_json(over: Value) -> Value {
    let mut v = json!({
        "verdict": "request_changes", "summary": "One unused variable.",
        "comments": [
            { "id": "c1", "path": "check.sh", "line": 2, "side": "RIGHT", "body": "unused is never read.", "on": true },
            { "id": "c2", "path": "check.sh", "line": 40, "side": "RIGHT", "body": "and this line is not in the diff.", "on": true, "inSummary": true },
        ],
        "headOid": HEAD, "threadId": "th_review", "at": 1,
    });
    merge(&mut v, over);
    v
}

pub fn picks_json(p: &StartPicksAsked) -> Value {
    let mut m = Map::new();
    if let Some(h) = &p.harness {
        m.insert("harness".into(), json!(h));
    }
    if let Some(h) = &p.model {
        m.insert("model".into(), json!(h));
    }
    if let Some(h) = &p.effort {
        m.insert("effort".into(), json!(h));
    }
    if let Some(a) = &p.access {
        m.insert("access".into(), serde_json::to_value(a).unwrap());
    }
    if let Some(h) = &p.permission_mode {
        m.insert("permissionMode".into(), json!(h));
    }
    if let Some(f) = p.fast {
        m.insert("fast".into(), json!(f));
    }
    Value::Object(m)
}

pub fn origin_text(origin: &Option<Caller>) -> Value {
    json!(format!("{origin:?}"))
}

pub struct FakeChannel(pub &'static str);

#[async_trait(?Send)]
impl DaemonChannel for FakeChannel {
    async fn send(&self, _frame: Frame) -> Result<Frame, ChannelError> {
        let mut m = Frame::new();
        m.insert("channel".into(), json!(self.0));
        Ok(m)
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        ChannelEnd { code: 1000, reason: String::new() }
    }
}

pub struct FakeReview {
    pub base: Rc<FakeBase>,
    pub pids: RefCell<IndexMap<String, Rc<RefCell<IndexMap<String, i64>>>>>,
    /// The listener the shared channel was handed (`heard`).
    pub heard: RefCell<Option<DaemonEventListener>>,
}

impl FakeReview {
    pub fn new() -> Rc<FakeReview> {
        Rc::new(FakeReview { base: FakeBase::new(), pids: RefCell::new(IndexMap::new()), heard: RefCell::new(None) })
    }

    fn b(&self) -> BaseOf {
        BaseOf(self.base.clone())
    }

    fn id_of(entry: &Entry) -> String {
        entry.borrow().record.borrow().id.clone()
    }
}

#[async_trait(?Send)]
impl Base for FakeReview {
    async fn ready(&self) -> Result<(), HostError> {
        self.b().ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.b().clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.b().ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.b().store()
    }
    fn live(&self) -> Live {
        self.b().live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.b().projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.b().project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.b().session_views()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.b().thread_records()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.b().persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.b().view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.b().emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.b().entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.b().spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.b().kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        self.b().place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.b().copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        self.b().with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        self.b().on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.b().git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        self.b().checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.b().preferences().await
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        self.b().write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        self.b().home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        self.b().machine_id(entry)
    }
}

#[async_trait(?Send)]
impl ReviewCtx for FakeReview {
    /// Default: the first held project; a scripted error is the refusal.
    fn project_by_repo(&self, repo: &str, named: Option<&str>) -> Result<ProjectView, HostError> {
        self.base.call("project_by_repo", json!({ "repo": repo, "named": named }));
        self.base.answer_as::<()>("project_by_repo", || ())?;
        Ok(self.base.projects.borrow().values().next().cloned().expect("the test holds a project"))
    }

    /// Default: issue `<number>` of the repository, titled `Add a greeting line`.
    async fn issue_of(&self, remote: &str, number: i64) -> Result<IssueRead, HostError> {
        self.base.call("issue_of", json!({ "remote": remote, "number": number }));
        self.base.answer_as("issue_of", || serde_json::from_value(issue_json("acme/spoo", number, "Add a greeting line")).unwrap())
    }

    /// Default: `pr_json("acme/spoo", number, {})`.
    async fn pull_request_of(&self, remote: &str, number: i64) -> Result<PullRequest, HostError> {
        self.base.call("pull_request_of", json!({ "remote": remote, "number": number }));
        self.base.answer_as("pull_request_of", || serde_json::from_value(pr_json("acme/spoo", number, json!({}))).unwrap())
    }

    /// TS `fromOf`, kept here because the real one is the start_from unit's.
    fn from_of(&self, kind: WorkspaceFromKind, repo: &str, read: &IssueRead, fact: Option<&PullRequest>) -> WorkspaceFrom {
        let kind_word = serde_json::to_value(kind).unwrap();
        self.base.call("from_of", json!({ "kind": kind_word, "repo": repo, "number": read.number, "withFact": fact.is_some() }));
        let mut v = json!({ "kind": kind_word, "repo": repo, "number": read.number, "url": read.url, "title": read.title });
        if let Some(f) = fact {
            v["base"] = json!(f.base);
            let mut head = json!({ "branch": f.branch, "oid": f.head_oid });
            if let Some(fork) = &f.fork {
                head["fork"] = serde_json::to_value(fork).unwrap();
            }
            v["head"] = head;
        }
        serde_json::from_value(v).expect("a from reads")
    }

    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError> {
        self.base.call("picks_hold", json!({ "project": project.id, "picks": picks_json(picks) }));
        self.base.answer_as("picks_hold", || ())
    }

    /// Places on the held `ws_placed` (made on the project when none is held); `behind`, `setup` and `as` from a
    /// script `{"behind": .., "setup": .., "as": {"threadId": .., "turnId": ..}}`.
    async fn workspace_from(
        &self,
        project: &ProjectView,
        from: WorkspaceFrom,
        fact: Option<PullRequest>,
        kind: StartKind,
        origin: Option<Caller>,
    ) -> Result<Placed, HostError> {
        self.base.call(
            "workspace_from",
            json!({
                "project": project.id, "from": serde_json::to_value(&from).unwrap(),
                "fact": fact.as_ref().map(|f| serde_json::to_value(f).unwrap()),
                "kind": match kind { StartKind::Start => "start", StartKind::Review => "review" },
                "origin": origin_text(&origin),
            }),
        );
        let said = self.base.answer_as("workspace_from", || json!({}))?;
        let held = self.base.live.borrow().get("ws_placed").cloned();
        let entry = match held {
            Some(e) => e,
            None => self.base.hold(record("ws_placed", "cloud", &project.id, json!({ "name": "placed" }))),
        };
        let r#as = said.get("as").map(|a| wsp_core::types::api::StartAs {
            thread_id: a["threadId"].as_str().unwrap().to_owned(),
            turn_id: a["turnId"].as_str().unwrap().to_owned(),
        });
        Ok(Placed { entry, behind: said["behind"].as_str().map(str::to_owned), setup: said["setup"].as_str().map(str::to_owned), r#as })
    }

    /// Default: the placed workspace's view on thread `th_open`, session `se_open`.
    async fn open_with(&self, placed: &Placed, o: OpenAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
        let mut m = Map::new();
        m.insert("entry".into(), json!(Self::id_of(&placed.entry)));
        if let Some(b) = &placed.behind {
            m.insert("behind".into(), json!(b));
        }
        if let Some(s) = &placed.setup {
            m.insert("setup".into(), json!(s));
        }
        if let Some(a) = &placed.r#as {
            m.insert("as".into(), json!({ "threadId": a.thread_id, "turnId": a.turn_id }));
        }
        m.insert("prompt".into(), json!(o.prompt));
        if let Some(h) = &o.harness {
            m.insert("harness".into(), json!(h));
        }
        if let Some(h) = &o.model {
            m.insert("model".into(), json!(h));
        }
        if let Some(h) = &o.effort {
            m.insert("effort".into(), json!(h));
        }
        if let Some(h) = &o.permission_mode {
            m.insert("permissionMode".into(), json!(h));
        }
        if let Some(a) = &o.access {
            m.insert("access".into(), serde_json::to_value(a).unwrap());
        }
        m.insert("origin".into(), origin_text(&origin));
        self.base.call("open_with", Value::Object(m));
        let rec = placed.entry.borrow().record.borrow().clone();
        let view = serde_json::to_value(view_of(&self.base, &rec)).unwrap();
        self.base.answer_as("open_with", || {
            serde_json::from_value(json!({ "workspace": view, "threadId": "th_open", "sessionId": "se_open" }))
                .expect("a start result reads")
        })
    }

    /// Default `read-only` for every agent.
    fn read_only_of(&self, agent: &str) -> Option<String> {
        self.base.call("read_only_of", json!({ "agent": agent }));
        self.base.answer_as::<Option<String>>("read_only_of", || Some("read-only".to_owned())).unwrap()
    }

    /// Default `codex`, `claude`.
    fn reviewers(&self) -> Vec<String> {
        self.base.call("reviewers", json!(null));
        self.base.answer_as("reviewers", || vec!["codex".to_owned(), "claude".to_owned()]).unwrap()
    }

    fn drop_page_cache(&self, remote: &str, number: i64) {
        self.base.call("drop_page_cache", json!({ "remote": remote, "number": number }));
    }

    fn reread_pull_request(&self, entry: &Entry) {
        self.base.call("reread_pull_request", json!({ "id": Self::id_of(entry) }));
    }

    /// Default the workspace's name, or `thread <first 8 of the id>` for a thread.
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String {
        self.base.call("label_of", json!({ "id": Self::id_of(entry), "thread": thread_id }));
        let name = entry.borrow().record.borrow().name.clone();
        let default = match thread_id {
            Some(t) => format!("thread {}", &t[..t.len().min(8)]),
            None => name,
        };
        self.base.answer_as("label_of", || default).unwrap()
    }

    /// Default `{url: "https://m1-7070.preview.example/?pt_token=edge", expiresAt: 5000, daemonToken: "tok"}`.
    async fn daemon_road(&self, entry: &Entry) -> Result<DaemonReachView, HostError> {
        self.base.call("daemon_road", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("daemon_road", || {
            serde_json::from_value(
                json!({ "url": "https://m1-7070.preview.example/?pt_token=edge", "expiresAt": 5000, "daemonToken": "tok" }),
            )
            .unwrap()
        })
    }

    fn forget_daemon_token(&self, machine_id: &str) {
        self.base.call("forget_daemon_token", json!({ "machineId": machine_id }));
    }

    /// Default false.
    fn shared_daemon(&self, entry: &Entry) -> bool {
        self.base.call("shared_daemon", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("shared_daemon", || false).unwrap()
    }

    async fn copy_channel(&self, entry: &Entry, _on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.base.call("copy_channel", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("copy_channel", || ())?;
        Ok(Rc::new(FakeChannel("plain")))
    }

    async fn shared_channel(&self, entry: &Entry, heard: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.base.call("shared_channel", json!({ "id": Self::id_of(entry) }));
        *self.heard.borrow_mut() = Some(heard);
        self.base.answer_as("shared_channel", || ())?;
        Ok(Rc::new(FakeChannel("shared")))
    }

    /// One map per workspace id, made once.
    fn port_pids(&self, workspace_id: &str) -> Rc<RefCell<IndexMap<String, i64>>> {
        self.base.call("port_pids", json!({ "id": workspace_id }));
        self.pids.borrow_mut().entry(workspace_id.to_owned()).or_default().clone()
    }

    fn port_roots_moved(&self, workspace_id: &str) {
        self.base.call("port_roots_moved", json!({ "id": workspace_id }));
    }

    async fn guest_road_channel(&self, entry: &Entry, _on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
        self.base.call("guest_road_channel", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("guest_road_channel", || ())?;
        Ok(Rc::new(FakeChannel("guest")))
    }

    /// Default none.
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        self.base.call("served_by_its_computer", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as::<Option<String>>("served_by_its_computer", || None).unwrap()
    }

    /// Default true.
    fn metrics_on_host(&self, entry: &Entry) -> bool {
        self.base.call("metrics_on_host", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("metrics_on_host", || true).unwrap()
    }

    /// Default `a machine`.
    fn machine_word(&self, entry: &Entry) -> String {
        self.base.call("machine_word", json!({ "id": Self::id_of(entry) }));
        self.base.answer_as("machine_word", || "a machine".to_owned()).unwrap()
    }

    /// Default: samples are read, and the detach it answers logs `sys_detach`. Script `false` for a host that
    /// reads nothing of its own computer.
    fn sys_samples(&self, _listener: SysListener) -> Option<Detach> {
        self.base.call("sys_samples", json!(null));
        if !self.base.answer_as("sys_samples", || true).unwrap() {
            return None;
        }
        let base = self.base.clone();
        Some(Box::new(move || base.call("sys_detach", json!(null))))
    }

    /// Default none (the kind has no road of its own); script a `{url, expiresAt}` object or an error.
    async fn kind_port_reach(&self, entry: &Entry, port: u16, standing: bool) -> Option<Result<PortReachView, HostError>> {
        self.base.call("kind_port_reach", json!({ "id": Self::id_of(entry), "port": port, "standing": standing }));
        match self.base.answer_as::<Option<PortReachView>>("kind_port_reach", || None) {
            Ok(Some(v)) => Some(Ok(v)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }

    /// Default `{url: "https://m1-<port>.preview.example/?pt_token=t1", expiresAt: 9000}`.
    async fn ws_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError> {
        self.base.call("ws_port_reach", json!({ "id": Self::id_of(entry), "port": port }));
        self.base.answer_as("ws_port_reach", || {
            serde_json::from_value(json!({ "url": format!("https://m1-{port}.preview.example/?pt_token=t1"), "expiresAt": 9000 })).unwrap()
        })
    }

    /// Default `{url: "https://m1-<port>.preview.example/?pt_token=t2", expiresAt: 9000}`.
    async fn ws_remint_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError> {
        self.base.call("ws_remint_port_reach", json!({ "id": Self::id_of(entry), "port": port }));
        self.base.answer_as("ws_remint_port_reach", || {
            serde_json::from_value(json!({ "url": format!("https://m1-{port}.preview.example/?pt_token=t2"), "expiresAt": 9000 })).unwrap()
        })
    }

    /// Default `{status: 200, body: ""}`.
    async fn probe(&self, url: &str) -> Result<PortProbeView, HostError> {
        self.base.call("probe", json!({ "url": url }));
        self.base.answer_as("probe", || serde_json::from_value(json!({ "status": 200, "body": "" })).unwrap())
    }

    /// Default none. A script under `refusal_for:<workspace id>` answers for that record alone; a record of none
    /// logs a null id.
    fn refusal_for(&self, record: Option<&WorkspaceRecord>, origin: Option<&Caller>) -> Option<String> {
        let id = record.map(|r| r.id.clone());
        self.base.call("refusal_for", json!({ "id": id, "origin": format!("{origin:?}") }));
        let key = format!("refusal_for:{}", id.clone().unwrap_or_default());
        let said = self.base.answer(&key).or_else(|| self.base.answer("refusal_for"));
        said.and_then(|v| v.as_str().map(str::to_owned))
    }

    /// Default true. A script under `reaches_row:<thread id>` answers for that thread alone.
    fn reaches_row(&self, thread_id: &str, workspace_id: &str, origin: Option<&Caller>) -> bool {
        self.base.call("reaches_row", json!({ "thread": thread_id, "workspace": workspace_id, "origin": format!("{origin:?}") }));
        let key = format!("reaches_row:{thread_id}");
        self.base.answer(&key).or_else(|| self.base.answer("reaches_row")).and_then(|v| v.as_bool()).unwrap_or(true)
    }
}
