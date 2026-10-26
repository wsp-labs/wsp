// SPDX-License-Identifier: AGPL-3.0-only
//! `FakeGit`: the workspaces_git unit's context over a `FakeBase`. Every `GitCtx` method logs `self.base.call(<name>,
//! args)` and answers from the script under the same name, with a default that lets the happy path run. `queued`
//! logs `queued` with the id and then runs the work, so the call order says what ran inside it.
#![allow(dead_code, unused_imports)]
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::{self, LocalBoxStream, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::account::pull_requests::FrameFor;
use wsp_account::projects::folders::tree::{ListedWorktree, RemoveAsk};
use wsp_account::projects::workspaces::git::{GitCtx, PullRequestOn, SentTo};
use wsp_account::projects::workspaces::seam::{
    Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld, Queued,
};
use wsp_adapters::port::{ExecFailure, ExecStream, SharedFuture, WriteOutcome};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::api::FolderForAsk;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{RunningExec, WorkspaceRecord};
use wsp_wire::types::changes::{Checkout, ViewedMarks};
use wsp_wire::types::pull_request::{GitUpdateReply, PullRequestPage};
use wsp_wire::types::tree::TreeRecord;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::base::{plain, rec, BaseOf, FakeBase};

pub const DEFAULT_REMOTE: &str = "git@github.com:acme/cart.git";
pub const DEFAULT_NUMBER: i64 = 12;

/// A command stream that hands out `lines`, ends with `exit`, and counts the kills and teardowns asked of it.
pub struct FakeStream {
    pub lines: Vec<String>,
    pub exit: Option<i32>,
    pub kills: Arc<AtomicUsize>,
}

impl ExecStream for FakeStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        stream::iter(self.lines.clone().into_iter().map(Ok)).boxed_local()
    }
    fn teardown(&self) {}
    fn kill(&self) {
        self.kills.fetch_add(1, Ordering::SeqCst);
    }
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        let code = self.exit;
        async move { code }.boxed_local().shared()
    }
}

pub struct FakeGit {
    pub base: Rc<FakeBase>,
    inner: BaseOf,
    pub viewed: Rc<RefCell<IndexMap<String, ViewedMarks>>>,
    pub kills: Arc<AtomicUsize>,
    /// The shell lines each `put_files` was told to run before it wrote.
    pub put_before: Rc<RefCell<Vec<Vec<String>>>>,
}

impl FakeGit {
    pub fn new(base: Rc<FakeBase>) -> Rc<FakeGit> {
        Rc::new(FakeGit {
            inner: BaseOf(base.clone()),
            base,
            viewed: Rc::new(RefCell::new(IndexMap::new())),
            kills: Arc::new(AtomicUsize::new(0)),
            put_before: Rc::new(RefCell::new(Vec::new())),
        })
    }

    fn live_entry(&self, id: &str) -> Result<Entry, HostError> {
        self.base.live.borrow().get(id).cloned().ok_or_else(|| plain(&format!("no workspace {id}")))
    }

    fn entry_named(&self, word: &str) -> Result<Entry, HostError> {
        let live = self.base.live.borrow();
        live.values()
            .find(|e| {
                let r = rec(e);
                r.id == word || r.name == word
            })
            .cloned()
            .ok_or_else(|| plain(&format!("no workspace {word}")))
    }

    fn entry_scripted(&self, name: &str) -> Result<Option<Entry>, HostError> {
        match self.base.answer_as::<Option<String>>(name, || None)? {
            Some(id) => Ok(Some(self.live_entry(&id)?)),
            None => Ok(None),
        }
    }
}

fn id_of(entry: &Entry) -> String {
    rec(entry).id
}

fn exec_ok() -> ExecResult {
    ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() }
}

fn default_prefs() -> Preferences {
    serde_json::from_value(json!({"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}))
        .expect("preferences read")
}

/// The page a read answers, every list empty; `sent` is the area's to fill.
pub fn empty_page() -> Value {
    json!({ "title": "t", "body": "b", "author": "octocat", "createdAt": "2026-09-30T10:00:00Z", "updatedAt": "2026-09-30T12:00:00Z",
        "labels": [], "reviewRequests": [], "latestReviews": [], "assignees": [], "commits": [], "reviews": [], "comments": [],
        "reviewComments": [], "files": [], "sent": [] })
}

#[async_trait(?Send)]
impl Base for FakeGit {
    async fn ready(&self) -> Result<(), HostError> {
        self.inner.ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.inner.clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.inner.ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.inner.store()
    }
    fn live(&self) -> Live {
        self.inner.live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.inner.projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.inner.project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.inner.session_views()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.inner.thread_records()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.inner.persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.inner.view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.inner.emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.inner.entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.inner.spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.inner.kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        self.inner.place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.inner.copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        self.inner.with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        self.inner.on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.inner.git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        self.inner.checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.base.call("preferences", json!(null));
        Ok(self.base.prefs.borrow().clone().unwrap_or_else(default_prefs))
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        self.inner.write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        self.inner.home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        self.inner.machine_id(entry)
    }
}

#[async_trait(?Send)]
impl GitCtx for FakeGit {
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        self.base.call("worktree_mounted", json!({ "id": id_of(entry) }));
        self.base.answer_as("worktree_mounted", || ())
    }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult, HostError> {
        self.base.call("machine_exec", json!({ "id": id_of(entry), "cmd": cmd, "timeoutMs": timeout_ms }));
        self.base.answer_as("machine_exec", exec_ok)
    }
    async fn exec_stream_of(&self, entry: &Entry, argv: Vec<String>, cwd: Option<String>) -> Result<RunningExec, HostError> {
        self.base.call("exec_stream_of", json!({ "id": id_of(entry), "argv": argv, "cwd": cwd }));
        let said = self.base.answer_as::<Value>("exec_stream_of", || json!({}))?;
        let lines: Vec<String> = said
            .get("lines")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let exit = said.get("exit").and_then(Value::as_i64).map(|c| c as i32);
        let ran_in = said.get("ranIn").and_then(Value::as_str).map(str::to_owned);
        Ok(RunningExec { stream: Arc::new(FakeStream { lines, exit, kills: self.kills.clone() }), ran_in })
    }
    async fn queued(&self, id: &str, work: Queued<Value>) -> Result<Value, HostError> {
        self.base.call("queued", json!({ "id": id }));
        work.await
    }
    async fn keep_tree(&self, entry: &Entry, tree: TreeRecord) -> Result<(), HostError> {
        self.base.call("keep_tree", json!({ "id": id_of(entry), "tree": serde_json::to_value(&tree).unwrap() }));
        self.base.answer_as("keep_tree", || ())
    }
    async fn take_pull_request(&self, entry: &Entry, pr: Value) -> Result<(), HostError> {
        self.base.call("take_pull_request", json!({ "id": id_of(entry), "pr": pr }));
        self.base.answer_as("take_pull_request", || ())
    }
    fn read_lead_of(&self, entry: &Entry) {
        self.base.call("read_lead_of", json!({ "id": id_of(entry) }));
    }
    async fn folder_for(&self, ask: FolderForAsk, _origin: Option<Caller>) -> Result<(Entry, Option<String>, Option<String>), HostError> {
        self.base.call("folder_for", json!({ "project": ask.project, "branch": ask.branch, "cwd": ask.cwd, "opens": ask.opens }));
        let said = self.base.answer_as::<Value>("folder_for", || json!({}))?;
        let id = said.get("id").and_then(Value::as_str).expect("the test scripts folder_for with an id").to_owned();
        Ok((
            self.live_entry(&id)?,
            said.get("cwd").and_then(Value::as_str).map(str::to_owned),
            said.get("setup").and_then(Value::as_str).map(str::to_owned),
        ))
    }
    fn take_setup_line(&self, id: &str) -> Option<String> {
        self.base.call("take_setup_line", json!({ "id": id }));
        self.base.answer_as::<Option<String>>("take_setup_line", || None).unwrap()
    }
    async fn resolve_project(&self, word: &str, _origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.base.call("resolve_project", json!({ "word": word }));
        self.base.answer_as::<()>("resolve_project", || ())?;
        let held = self.base.projects.borrow();
        held.values().find(|p| p.id == word || p.name == word).cloned().ok_or_else(|| plain(&format!("no project {word}")))
    }
    fn refuse_recording(&self, word: &str, _origin: Option<&Caller>) -> Result<(), HostError> {
        self.base.call("refuse_recording", json!({ "word": word }));
        self.base.answer_as("refuse_recording", || ())
    }
    fn refuse_relayed(&self, record: &WorkspaceRecord, _origin: Option<&Caller>) -> Result<(), HostError> {
        self.base.call("refuse_relayed", json!({ "id": record.id }));
        self.base.answer_as("refuse_relayed", || ())
    }
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        self.base.call("project_folder", json!({ "project": project.id }));
        match self.entry_scripted("project_folder")? {
            Some(e) => Ok(e),
            None => {
                let live = self.base.live.borrow();
                live.values().find(|e| rec(e).project == project.id).cloned().ok_or_else(|| plain("no folder record"))
            }
        }
    }
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str, made_for: Option<String>) -> Result<Entry, HostError> {
        self.base.call("worktree_folder", json!({ "project": project.id, "top": top, "branch": branch, "madeFor": made_for }));
        self.entry_scripted("worktree_folder")?.ok_or_else(|| plain("the test scripts worktree_folder with a workspace id"))
    }
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError> {
        self.base.call("worktrees_of", json!({ "top": top }));
        let said = self.base.answer_as::<Vec<Value>>("worktrees_of", Vec::new)?;
        Ok(said
            .iter()
            .map(|w| ListedWorktree {
                path: w["path"].as_str().unwrap().to_owned(),
                branch: w.get("branch").and_then(Value::as_str).map(str::to_owned),
            })
            .collect())
    }
    fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.base.call("folders_of", json!({ "project": project_id }));
        self.base.live.borrow().values().filter(|e| rec(e).project == project_id).cloned().collect()
    }
    async fn remove_worktree(&self, entry: &Entry, force: bool, ask: RemoveAsk) -> Result<(), HostError> {
        self.base.call("remove_worktree", json!({ "id": id_of(entry), "force": force, "ending": ask.ending, "check": ask.check }));
        self.base.answer_as("remove_worktree", || ())
    }
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<Option<Checkout>, HostError> {
        self.base.call("read_checkout", json!({ "id": id_of(entry), "fresh": fresh }));
        let key = format!("read_checkout:{}", id_of(entry));
        if self.base.script.borrow().contains_key(&key) {
            return self.base.answer_as(&key, || None);
        }
        self.base.answer_as("read_checkout", || None)
    }
    async fn read_pull_request(&self, entry: &Entry, force: bool, whole: bool) -> Result<Option<Value>, HostError> {
        self.base.call("read_pull_request", json!({ "id": id_of(entry), "force": force, "whole": whole }));
        self.base.answer_as::<Option<Value>>("read_pull_request", || None)
    }
    async fn read_tree(&self, entry: &Entry) -> Result<(), HostError> {
        self.base.call("read_tree", json!({ "id": id_of(entry) }));
        self.base.answer_as("read_tree", || ())
    }
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String {
        match thread_id {
            None => rec(entry).name,
            Some(t) => format!("thread {}", t.chars().take(8).collect::<String>()),
        }
    }
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<Value>, HostError> {
        self.base.call("open_thread", json!({ "workspaceId": workspace_id, "threadId": thread_id }));
        self.base.answer_as("open_thread", Vec::new)
    }
    fn default_agent_of(&self, entry: &Entry) -> String {
        self.base.call("default_agent_of", json!({ "id": id_of(entry) }));
        self.base.answer_as::<String>("default_agent_of", || "claude".to_owned()).unwrap()
    }
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        self.base.call("agent_off", json!({ "id": id_of(entry), "agent": agent }));
        self.base.answer_as::<bool>("agent_off", || false).unwrap()
    }
    fn has_adapter(&self, agent: &str) -> bool {
        self.base.call("has_adapter", json!({ "agent": agent }));
        self.base.answer_as::<bool>("has_adapter", || true).unwrap()
    }
    async fn drafter(&self, entry: &Entry, agent: &str) -> Result<Option<Option<String>>, HostError> {
        self.base.call("drafter", json!({ "id": id_of(entry), "agent": agent }));
        let said = self.base.answer_as::<Value>("drafter", || json!({ "model": "small-model" }))?;
        if said.is_null() || said == json!(false) {
            return Ok(None);
        }
        Ok(Some(said.get("model").and_then(Value::as_str).map(str::to_owned)))
    }
    async fn put_files(&self, entry: &Entry, path: &str, text: &str, before: &[&str]) -> Result<ExecResult, HostError> {
        self.put_before.borrow_mut().push(before.iter().map(|line| (*line).to_owned()).collect());
        self.base.call("put_files", json!({ "id": id_of(entry), "path": path, "text": text }));
        self.base.answer_as("put_files", exec_ok)
    }
    async fn draft_for(&self, entry: &Entry, agent: &str, prompt_file: &str, model: Option<&str>) -> Result<Option<String>, HostError> {
        self.base.call("draft_for", json!({ "id": id_of(entry), "agent": agent, "promptFile": prompt_file, "model": model }));
        self.base.answer_as::<Option<String>>("draft_for", || Some("Round the cart total once\n\nThe total rounded per line.".to_owned()))
    }
    fn viewed_marks(&self) -> Rc<RefCell<IndexMap<String, ViewedMarks>>> {
        self.viewed.clone()
    }
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), HostError> {
        self.base.call("persist_sessions", json!({ "id": workspace_id }));
        self.base.answer_as("persist_sessions", || ())
    }
    async fn pull_request_on(&self, workspace_id: &str, _origin: Option<Caller>) -> Result<PullRequestOn, HostError> {
        self.base.call("pull_request_on", json!({ "id": workspace_id }));
        let said = self.base.answer_as::<Value>("pull_request_on", || json!({}))?;
        Ok(PullRequestOn {
            entry: self.live_entry(workspace_id)?,
            remote: said.get("remote").and_then(Value::as_str).unwrap_or(DEFAULT_REMOTE).to_owned(),
            number: said.get("number").and_then(Value::as_i64).unwrap_or(DEFAULT_NUMBER),
        })
    }
    async fn read_page(&self, entry: &Entry, remote: &str, number: i64, fresh: bool) -> Result<(PullRequestPage, bool), HostError> {
        self.base.call("read_page", json!({ "id": id_of(entry), "remote": remote, "number": number, "fresh": fresh }));
        let said = self.base.answer_as::<Value>("read_page", || json!({}))?;
        let page = said.get("page").cloned().unwrap_or_else(empty_page);
        let here = said.get("here").and_then(Value::as_bool).unwrap_or(true);
        Ok((serde_json::from_value(page).expect("the scripted page reads"), here))
    }
    async fn merge_settings(&self, remote: &str) -> Result<Value, HostError> {
        self.base.call("merge_settings", json!({ "remote": remote }));
        self.base.answer_as("merge_settings", || json!({ "methods": ["merge", "squash"], "defaultMethod": "squash", "autoMerge": true }))
    }
    async fn read_host(&self, entry: &Entry, frame: FrameFor) -> Result<Value, HostError> {
        let ask = Value::Object(frame(&self.home_dir()));
        self.base.call("read_host", json!({ "id": id_of(entry), "ask": ask }));
        self.base.answer_as("read_host", || json!({}))
    }
    async fn post_as_person(&self, remote: &str, number: i64, ask: Value) -> Result<Value, HostError> {
        self.base.call("post_as_person", json!({ "remote": remote, "number": number, "ask": ask }));
        self.base.answer_as("post_as_person", || json!({}))
    }
    async fn to_first_thread(&self, workspace_id: &str, prompt: &str, _origin: Option<Caller>) -> Result<SentTo, HostError> {
        self.base.call("to_first_thread", json!({ "workspaceId": workspace_id, "prompt": prompt }));
        sent_to(self.base.answer_as::<Value>("to_first_thread", || json!({}))?)
    }
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str, _origin: Option<Caller>) -> Result<SentTo, HostError> {
        self.base.call("send_detached", json!({ "workspaceId": workspace_id, "prompt": prompt, "thread": thread }));
        sent_to(self.base.answer_as::<Value>("send_detached", || json!({ "threadId": thread }))?)
    }
    async fn child_of(
        &self,
        lead: &Entry,
        child: &str,
        _origin: Option<Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<Entry, HostError> {
        self.base.call("child_of", json!({ "lead": id_of(lead), "child": child, "leadThread": lead_thread, "childThread": child_thread }));
        self.base.answer_as::<()>("child_of", || ())?;
        self.entry_named(child)
    }
    async fn update_copy(&self, entry: &Entry) -> Result<GitUpdateReply, HostError> {
        self.base.call("update_copy", json!({ "id": id_of(entry) }));
        self.base.answer_as("update_copy", || GitUpdateReply {
            base: "main".to_owned(),
            merged: true,
            commits: 0.into(),
            conflicts: vec![],
        })
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        self.base.call("drop_page_cache", json!({ "remote": remote, "number": number }));
    }
    fn is_local_workspace(&self, record: &WorkspaceRecord) -> bool {
        matches!(record.kind, WorkspaceKind::Local)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

fn sent_to(said: Value) -> Result<SentTo, HostError> {
    Ok(SentTo {
        outcome: said.get("outcome").and_then(Value::as_str).unwrap_or("started").to_owned(),
        thread_id: said.get("threadId").and_then(Value::as_str).unwrap_or("t1").to_owned(),
        harness: said.get("harness").and_then(Value::as_str).unwrap_or("claude").to_owned(),
        capped: said.get("capped").cloned(),
    })
}
