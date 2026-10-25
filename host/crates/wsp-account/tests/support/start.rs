// SPDX-License-Identifier: AGPL-3.0-only
//! `FakeStart`: the context start_from reads, over a `FakeBase`. Each method logs `base.call("<method>", args)` and
//! answers from the script under its own name, with the default noted at the method. A scripted
//! `err_answer(..)` is the method's failure. Entries are held in `base.live`: a test pre-holds the entry it wants
//! (`ws_folder` for the project folder, `ws_wt` for the worktree) and the fake otherwise makes a plain one.
#![allow(dead_code)]
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use wsp_account::projects::folders::tree::ListedWorktree;
use wsp_account::projects::projects::add::LandingKind;
use wsp_account::projects::start_from::{ResolvedPicks, StartFromCtx};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld,
};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, WorkspaceRecord};
use wsp_wire::types::pull_request::PullRequest;
use wsp_wire::types::start::IssueRead;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{BranchOfRecord, Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::base::{record, view_of, BaseOf, FakeBase};

pub const HEAD: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";

/// A JSON object from pairs, keys in the order given.
pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_owned(), v);
    }
    Value::Object(m)
}

/// A pull request as the daemon reads it (the TS test's `pullRequest(repo)`), `over` written on top.
pub fn pr_json(repo: &str, number: i64, over: Value) -> Value {
    let mut v = json!({
        "number": number, "url": format!("https://github.com/{repo}/pull/{number}"), "state": "open", "host": "github.com",
        "draft": false, "base": "main", "branch": "lab/review-me", "headOid": HEAD, "headSubject": "Rename the status word",
        "mergeable": "mergeable", "mergeState": "clean", "review": "none", "checks": [], "additions": 2, "deletions": 1,
        "changedFiles": 2, "commits": 1, "author": "maya",
    });
    crate::base::merge(&mut v, over);
    v
}

pub fn pr(repo: &str, number: i64, over: Value) -> PullRequest {
    serde_json::from_value(pr_json(repo, number, over)).expect("a test pull request reads")
}

pub fn issue_json(repo: &str, number: i64, title: &str) -> Value {
    json!({ "number": number, "url": format!("https://github.com/{repo}/issues/{number}"), "title": title,
        "body": "The first line should say hello.", "state": "OPEN",
        "comments": [{ "author": "maya", "body": "Keep it one line.", "at": "2026-09-29T10:00:00Z" }] })
}

pub fn issue(repo: &str, number: i64, title: &str) -> IssueRead {
    serde_json::from_value(issue_json(repo, number, title)).expect("a test issue reads")
}

/// The smallest catalog the wire reads, for an agent named `harness`.
pub fn catalog_json(harness: &str) -> Value {
    json!({ "harness": harness, "label": harness, "source": "table", "version": null, "models": [], "efforts": [],
        "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false })
}

pub fn catalog(harness: &str) -> HarnessCatalog {
    serde_json::from_value(catalog_json(harness)).expect("a test catalog reads")
}

/// A session row as `handle.view()` reads it.
pub fn session_json(id: &str, thread: Option<&str>) -> Value {
    let mut v = json!({ "id": id, "workspaceId": "ws_placed", "harness": "claude", "status": "running" });
    if let Some(t) = thread {
        v["threadId"] = json!(t);
    }
    v
}

/// A `Caller`'s debug text, what a fake logs for an origin it was handed.
pub fn origin_text(origin: &Option<Caller>) -> Value {
    json!(format!("{origin:?}"))
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

pub struct FakeStart {
    pub base: Rc<FakeBase>,
}

impl FakeStart {
    pub fn new() -> Rc<FakeStart> {
        Rc::new(FakeStart { base: FakeBase::new() })
    }

    fn b(&self) -> BaseOf {
        BaseOf(self.base.clone())
    }

    fn entry_named(&self, id: &str, project: &str, kind: &str, over: Value) -> Entry {
        if let Some(e) = self.base.live.borrow().get(id).cloned() {
            return e;
        }
        self.base.hold(record(id, kind, project, over))
    }
}

#[async_trait(?Send)]
impl Base for FakeStart {
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
    fn thread_workspaces(&self) -> Vec<(String, String)> {
        self.b().thread_workspaces()
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
impl StartFromCtx for FakeStart {
    /// Default `here`.
    fn places_wired(&self) -> String {
        self.base.call("places_wired", json!(null));
        self.base.answer_as("places_wired", || "here".to_owned()).unwrap()
    }

    /// Default none: the start names the wired place.
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.base.call("landing_place", json!({ "computer": computer }));
        self.base.answer_as::<Option<String>>("landing_place", || None)
    }

    /// Default `provider` for a cloud computer, else `mac`.
    fn landing_kind(&self, computer: &str) -> LandingKind {
        self.base.call("landing_kind", json!({ "computer": computer }));
        let default = if self.kind_of(computer) == WorkspaceKind::Cloud { "provider" } else { "mac" };
        match self.base.answer_as("landing_kind", || default.to_owned()).unwrap().as_str() {
            "provider" => LandingKind::Provider,
            "box" => LandingKind::Box,
            _ => LandingKind::Mac,
        }
    }

    /// Default false: not brokered.
    fn brokered_start(&self, provider: bool) -> bool {
        self.base.call("brokered_start", json!({ "provider": provider }));
        self.base.answer_as("brokered_start", || false).unwrap()
    }

    /// Default none.
    async fn pull_request_of_record(&self, remote: &str, number: i64, project: &str, computer: &str, start: bool) -> Result<Option<BranchOfRecord>, HostError> {
        self.base.call("pull_request_of_record", json!({ "remote": remote, "number": number, "project": project, "computer": computer, "start": start }));
        self.base.answer_as::<Option<BranchOfRecord>>("pull_request_of_record", || None)
    }

    /// Holds a cloud record named `opts.name` on `opts.project` under the id `ws_made` unless a script
    /// `{"id": .., "live": false}` says the entry is not live after.
    async fn create_workspace(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
        let mut args = Map::new();
        args.insert("project".into(), json!(opts.project));
        args.insert("name".into(), json!(opts.name));
        if let Some(b) = &opts.branch_of_record {
            args.insert("branchOfRecord".into(), serde_json::to_value(b).unwrap());
        }
        if let Some(t) = &opts.opens_thread {
            args.insert("opensThread".into(), json!(t));
        }
        args.insert("origin".into(), origin_text(&origin));
        self.base.call("create_workspace", Value::Object(args));
        let said = self.base.answer_as("create_workspace", || json!({ "id": "ws_made", "live": true }))?;
        let id = said["id"].as_str().unwrap_or("ws_made").to_owned();
        let rec = record(&id, "cloud", &opts.project, json!({ "name": opts.name }));
        if said["live"].as_bool().unwrap_or(true) {
            self.base.hold(rec.clone());
        }
        let view = view_of(&self.base, &rec);
        Ok(serde_json::from_value(serde_json::to_value(view).unwrap()).expect("a created workspace reads"))
    }

    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.base.call("delete_workspace", json!({ "id": id, "origin": origin_text(&origin) }));
        self.base.answer_as("delete_workspace", || ())
    }

    /// The held `ws_folder`, else a local record of the project with no worktree.
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        self.base.call("project_folder", json!({ "project": project.id }));
        Ok(self.entry_named("ws_folder", &project.id, "local", json!({})))
    }

    /// The held `ws_wt`, else a local record whose worktree is `<top>/../wt/<branch>`, made by wsp.
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str) -> Result<Entry, HostError> {
        self.base.call("worktree_folder", json!({ "project": project.id, "top": top, "branch": branch }));
        let path = format!("/home/me/wt/{}", branch.replace('/', "-"));
        Ok(self.entry_named("ws_wt", &project.id, "local", json!({ "worktree": { "path": path, "branch": branch, "made": true } })))
    }

    /// Default none; script `[{"path": .., "branch": ..}]`.
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError> {
        self.base.call("worktrees_of", json!({ "top": top }));
        let rows = self.base.answer_as::<Vec<Value>>("worktrees_of", Vec::new)?;
        Ok(rows
            .iter()
            .map(|r| ListedWorktree { path: r["path"].as_str().unwrap_or("").to_owned(), branch: r["branch"].as_str().map(str::to_owned) })
            .collect())
    }

    /// Default none.
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        self.base.call("take_setup_line", json!({ "id": workspace_id }));
        self.base.answer_as::<Option<String>>("take_setup_line", || None).unwrap()
    }

    fn preferences_held(&self) -> Option<Preferences> {
        self.base.call("preferences_held", json!(null));
        self.base.prefs.borrow().clone()
    }

    /// Default `claude`.
    fn default_agent_of(&self, _prefs: &Preferences, entry: &Entry) -> String {
        self.base.call("default_agent_of", json!({ "entry": entry.borrow().record.borrow().id }));
        self.base.answer_as("default_agent_of", || "claude".to_owned()).unwrap()
    }

    /// Default: a catalog for the harness with no defaults. A script `null` is no table for the harness.
    async fn resolved_picks(&self, home: &Entry, harness: &str, _prefs: &Preferences, project_id: &str) -> Result<Option<ResolvedPicks>, HostError> {
        self.base.call("resolved_picks", json!({ "home": home.borrow().record.borrow().id, "harness": harness, "projectId": project_id }));
        let said = self.base.answer_as::<Option<Value>>("resolved_picks", || Some(json!({ "catalog": catalog_json(harness) })))?;
        Ok(said.map(|v| ResolvedPicks {
            catalog: serde_json::from_value(v["catalog"].clone()).expect("a scripted catalog reads"),
            open_model: v["openModel"].as_str().map(str::to_owned),
            open_effort: v["openEffort"].as_str().map(str::to_owned),
        }))
    }

    /// Default none; a scripted error is the refusal.
    fn access_word_refusal(&self, catalog: &HarnessCatalog, access: &AccessChoice) -> Option<HostError> {
        self.base.call("access_word_refusal", json!({ "harness": catalog.harness, "access": serde_json::to_value(access).unwrap() }));
        self.base.answer_as::<()>("access_word_refusal", || ()).err()
    }

    /// Default none.
    fn named_mode(&self, catalog: &HarnessCatalog, harness: &str, access: &AccessChoice) -> Option<String> {
        self.base.call("named_mode", json!({ "catalog": catalog.harness, "harness": harness, "access": serde_json::to_value(access).unwrap() }));
        self.base.answer_as::<Option<String>>("named_mode", || None).unwrap()
    }

    /// Default none; a scripted error is the pick's refusal.
    fn start_picks_refusal(&self, catalog: &HarnessCatalog, picks: &StartPicksAsked) -> Option<HostError> {
        self.base.call("start_picks_refusal", json!({ "catalog": catalog.harness, "picks": picks_json(picks) }));
        self.base.answer_as::<()>("start_picks_refusal", || ()).err()
    }

    /// Default a running row `se_1` on thread `th_1`.
    async fn start_session(&self, workspace_id: &str, opts: Value, origin: Option<Caller>) -> Result<SessionView, HostError> {
        self.base.call("start_session", json!({ "workspaceId": workspace_id, "opts": opts, "origin": origin_text(&origin) }));
        self.base.answer_as("start_session", || serde_json::from_value(session_json("se_1", Some("th_1"))).unwrap())
    }

    /// Default claude `plan`, gemini none, codex `read-only`, opencode none.
    fn read_only_modes(&self) -> Vec<(String, Option<String>)> {
        self.base.call("read_only_modes", json!(null));
        let rows = self.base.answer_as("read_only_modes", || {
            vec![json!(["claude", "plan"]), json!(["gemini", null]), json!(["codex", "read-only"]), json!(["opencode", null])]
        });
        rows.unwrap().iter().map(|r| (r[0].as_str().unwrap().to_owned(), r[1].as_str().map(str::to_owned))).collect()
    }

    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str) -> Result<(), HostError> {
        self.base.call("send_detached", json!({ "id": workspace_id, "prompt": prompt, "thread": thread }));
        self.base.answer_as("send_detached", || ())
    }
}
