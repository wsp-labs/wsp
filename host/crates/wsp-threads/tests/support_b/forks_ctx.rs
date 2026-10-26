// SPDX-License-Identifier: AGPL-3.0-only
//! The fork functions' fake context, a fake adapter whose `fork_session` answers what a test queued, and a fake
//! start. The context keeps the runtime's rows and records in `StateCell`s, each thread's events by workspace and
//! thread, answers each call out from what a test configured, and logs every call as `name args` in order.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::json;
use wsp_adapters::port::{AdapterError, ForkTurn, ForkedSession, HarnessSession, HarnessStartOptions};
use wsp_core::context::Joined;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapter;
use wsp_core::types::wiring::{AccessMove, Local, SessionHandle, SteerTaken, WorkspaceRecord};
use wsp_machines::machines::boot::entries::RowAt;
use wsp_store::store::{BlobMark, Store};
use wsp_threads::threads::forks::{ForksCtx, StartFn};
use wsp_threads::threads::threads::{Entry, Sessions, ThreadRecords};
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;

use super::conversations_ctx::project_view;
use super::{err, exec_result, view, FakeStore};

// ---- a store that logs its reads ---------------------------------------------------------------------------------

/// The fake store with each read logged as `store.get c/id` or `store.getBlob c/id` into the context's calls.
struct LoggedStore {
    inner: Rc<FakeStore>,
    calls: Rc<RefCell<Vec<String>>>,
}

#[async_trait::async_trait(?Send)]
impl Store for LoggedStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>, HostError> {
        self.calls.borrow_mut().push(format!("store.get {collection}/{id}"));
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<(), HostError> {
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<serde_json::Value>, HostError> {
        self.inner.list(collection).await
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(collection).await
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(collection, id).await
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.calls.borrow_mut().push(format!("store.getBlob {collection}/{id}"));
        self.inner.get_blob(collection, id).await
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
}

// ---- a handle a start answers ------------------------------------------------------------------------------------

struct FakeHandle {
    id: String,
    workspace_id: String,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
}

impl SessionHandle for FakeHandle {
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
        format!("turn-of-{}", self.id)
    }
    fn outcome(&self) -> SessionStartOutcome {
        SessionStartOutcome::Started
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        view(json!({ "id": self.id, "workspaceId": self.workspace_id }))
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<wsp_adapters::port::TurnImage>,
    ) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(
        &self,
        _ask: String,
        _option: String,
        _reason: Option<String>,
    ) -> Option<Local<'static, wsp_wire::wire::runtime::SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, wsp_adapters::port::TaskStop>> {
        None
    }
}

/// A handle for the session `id` on `workspace_id`, finished at once.
pub fn handle(id: &str, workspace_id: &str) -> Rc<dyn SessionHandle> {
    let result: TurnResult = serde_json::from_value(json!({ "status": "completed" })).expect("a TurnResult");
    Rc::new(FakeHandle { id: id.into(), workspace_id: workspace_id.into(), finished: async move { result }.boxed_local().shared() })
}

// ---- the adapter -------------------------------------------------------------------------------------------------

/// What the fake adapter's `fork_session` answers.
#[derive(Clone, Debug)]
pub enum ForkAnswer {
    /// `{ resume, drop }`: a copy the start resumes.
    Resume(String),
    /// `{ fork: { session, turn } }`: the start forks itself.
    Fork { session: String, turn: String },
    /// The adapter throws this message.
    Refuse(String),
}

/// An adapter that forks sessions only where a test says it does, answering what a test queued (a refusal where none
/// is) and logging each fork it was asked for.
pub struct FakeForkAdapter {
    pub forks: bool,
    pub by_count: bool,
    answers: Mutex<VecDeque<ForkAnswer>>,
    asked: Mutex<Vec<(String, ForkTurn)>>,
    dropped: Arc<Mutex<Vec<String>>>,
}

impl FakeForkAdapter {
    /// An adapter that forks sessions.
    pub fn new() -> Arc<FakeForkAdapter> {
        Arc::new(FakeForkAdapter {
            forks: true,
            by_count: false,
            answers: Mutex::default(),
            asked: Mutex::default(),
            dropped: Arc::default(),
        })
    }
    /// An adapter with no `forkSession`.
    pub fn without_forks() -> Arc<FakeForkAdapter> {
        Arc::new(FakeForkAdapter {
            forks: false,
            by_count: false,
            answers: Mutex::default(),
            asked: Mutex::default(),
            dropped: Arc::default(),
        })
    }
    pub fn answers(&self, answers: Vec<ForkAnswer>) {
        *self.answers.lock().expect("answers") = answers.into();
    }
    /// Every fork asked, as the session and the turn.
    pub fn asked(&self) -> Vec<(String, ForkTurn)> {
        self.asked.lock().expect("asked").clone()
    }
    /// The copies a start never ran, which were dropped.
    pub fn dropped(&self) -> Vec<String> {
        self.dropped.lock().expect("dropped").clone()
    }
}

impl HarnessAdapter for FakeForkAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the fake fork adapter starts nothing".to_string()))
    }
    fn steers(&self) -> bool {
        false
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        if !self.forks {
            return None;
        }
        self.asked.lock().expect("asked").push((session, turn));
        let answer =
            self.answers.lock().expect("answers").pop_front().unwrap_or_else(|| ForkAnswer::Refuse("no fork answer queued".to_string()));
        let dropped = self.dropped.clone();
        Some(
            async move {
                match answer {
                    ForkAnswer::Resume(copy) => {
                        let gone = copy.clone();
                        Ok(ForkedSession::Resume {
                            resume: copy,
                            drop: Box::new(move || {
                                dropped.lock().expect("dropped").push(gone);
                                async { Ok(()) }.boxed_local()
                            }),
                        })
                    }
                    ForkAnswer::Fork { session, turn } => Ok(ForkedSession::Fork { session, turn }),
                    ForkAnswer::Refuse(why) => Err(why),
                }
            }
            .boxed_local(),
        )
    }
    fn forks_by_count(&self) -> bool {
        self.by_count
    }
}

// ---- the context -------------------------------------------------------------------------------------------------

pub struct FakeForksCtx {
    pub store: Rc<FakeStore>,
    logged: Rc<LoggedStore>,
    pub sessions: Sessions,
    pub thread_records: ThreadRecords,
    /// Each thread's events as `openThread` reads them, by workspace and thread.
    pub thread_events: RefCell<IndexMap<(String, String), Vec<SessionEvent>>>,
    pub running: RefCell<IndexMap<String, String>>,
    pub launching: RefCell<IndexMap<String, String>>,
    /// Live entries by workspace id: what `entry_of_row` answers.
    pub entries: RefCell<IndexMap<String, Entry>>,
    /// Threads the caller does not reach: `entry_of_row` refuses them.
    pub unreached: RefCell<IndexSet<String>>,
    pub started_as: RefCell<IndexMap<(String, String), String>>,
    pub folders: RefCell<IndexMap<(String, String), String>>,
    pub kept_writes: RefCell<IndexMap<String, Joined>>,
    /// Every `keepSentImages` call: workspace, thread, request and the images.
    pub kept_images: RefCell<Vec<(String, String, Option<String>, Vec<Attachment>)>>,
    pub projects: RefCell<IndexMap<String, ProjectView>>,
    /// A git command containing the needle (its args joined by a space) answers; the first match wins, else exit 0.
    pub git: RefCell<Vec<(String, Result<ExecResult, HostError>)>>,
    pub live: RefCell<Vec<String>>,
    /// What each `worktreeFolder` answers, in order of calls.
    pub worktrees: RefCell<VecDeque<Result<Entry, HostError>>>,
    pub checkouts: RefCell<IndexMap<String, String>>,
    pub setup_lines: RefCell<IndexMap<String, String>>,
    /// The message `removeWorktree` fails with.
    pub remove_fails: RefCell<Option<String>>,
    pub starts: Rc<RefCell<VecDeque<Result<Rc<dyn SessionHandle>, HostError>>>>,
    pub started: Rc<RefCell<Vec<(String, StartOptions)>>>,
    /// Every call out, as `name args`, in order.
    pub calls: Rc<RefCell<Vec<String>>>,
}

impl FakeForksCtx {
    pub fn new() -> Rc<FakeForksCtx> {
        let store = FakeStore::new();
        let calls: Rc<RefCell<Vec<String>>> = Rc::default();
        Rc::new(FakeForksCtx {
            logged: Rc::new(LoggedStore { inner: store.clone(), calls: calls.clone() }),
            store,
            sessions: Sessions::default(),
            thread_records: ThreadRecords::default(),
            thread_events: RefCell::default(),
            running: RefCell::default(),
            launching: RefCell::default(),
            entries: RefCell::default(),
            unreached: RefCell::default(),
            started_as: RefCell::default(),
            folders: RefCell::default(),
            kept_writes: RefCell::default(),
            kept_images: RefCell::default(),
            projects: RefCell::default(),
            git: RefCell::default(),
            live: RefCell::default(),
            worktrees: RefCell::default(),
            checkouts: RefCell::default(),
            setup_lines: RefCell::default(),
            remove_fails: RefCell::new(None),
            starts: Rc::default(),
            started: Rc::default(),
            calls,
        })
    }
    /// The context as the fork functions take it.
    pub fn ctx(self: &Rc<Self>) -> Rc<dyn ForksCtx> {
        self.clone()
    }
    pub fn call(&self, line: String) {
        self.calls.borrow_mut().push(line);
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    pub fn add_entry(&self, entry: Entry) {
        let id = entry.borrow().record.borrow().id.clone();
        self.entries.borrow_mut().insert(id, entry);
    }
    pub fn events_of(&self, workspace_id: &str, thread_id: &str, events: Vec<SessionEvent>) {
        self.thread_events.borrow_mut().insert((workspace_id.to_string(), thread_id.to_string()), events);
    }
    /// A git command whose args joined by a space contain `needle` answers this.
    pub fn git_answers(&self, needle: &str, exit_code: i64) {
        self.git.borrow_mut().push((needle.to_string(), Ok(exec_result(exit_code, "", ""))));
    }
    /// What the fork's start answers, in order of calls; with none queued a start fails `no start answer queued`.
    pub fn start_answers(&self, answer: Result<Rc<dyn SessionHandle>, HostError>) {
        self.starts.borrow_mut().push_back(answer);
    }
    /// The start a fork runs: it logs `start <workspace>` into the calls and keeps the options it was handed.
    pub fn start_fn(self: &Rc<Self>) -> StartFn {
        let me = self.clone();
        Rc::new(move |workspace_id: String, options: StartOptions| {
            me.call(format!("start {workspace_id}"));
            me.started.borrow_mut().push((workspace_id, options));
            let answer = me.starts.borrow_mut().pop_front().unwrap_or_else(|| Err(err("no start answer queued")));
            async move { answer }.boxed_local()
        })
    }
}

impl ForksCtx for FakeForksCtx {
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call("ready".into());
        async { Ok(()) }.boxed_local()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.logged.clone()
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        let mut best: Option<SessionView> = None;
        for r in self.sessions.borrow().values().filter(|r| r.borrow().view.thread_id.as_deref() == Some(thread_id)) {
            let r = r.borrow();
            let at = r.view.started_at.map(|n| n.0).unwrap_or(0.0);
            if best.as_ref().is_none_or(|b| at >= b.started_at.map(|n| n.0).unwrap_or(0.0)) {
                best = Some(r.view.clone());
            }
        }
        best
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.sessions
            .borrow()
            .values()
            .filter(|r| r.borrow().view.thread_id.as_deref() == Some(thread_id))
            .map(|r| r.borrow().view.clone())
            .collect()
    }
    fn running_turn(&self, thread_id: &str) -> Option<String> {
        self.running.borrow().get(thread_id).cloned()
    }
    fn launching_turn(&self, thread_id: &str) -> Option<String> {
        self.launching.borrow().get(thread_id).cloned()
    }
    fn entry_of_row(&self, at: &RowAt, _origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let thread_id = at.thread_id.clone().unwrap_or_default();
        let workspace_id = at.workspace_id.as_str();
        self.call(format!("entryOfRow {thread_id} {workspace_id}"));
        let answer = if self.unreached.borrow().contains(&thread_id) {
            Err(err("the caller does not reach that thread"))
        } else {
            self.entries.borrow().get(workspace_id).cloned().ok_or_else(|| err(&format!("no live workspace {workspace_id}")))
        };
        async move { answer }.boxed_local()
    }
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>> {
        self.call(format!("openThread {workspace_id} {thread_id}"));
        let events = self.thread_events.borrow().get(&(workspace_id.to_string(), thread_id.to_string())).cloned().unwrap_or_default();
        async move { Ok(events) }.boxed_local()
    }
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.started_as.borrow().get(&(workspace_id.to_string(), thread_id.to_string())).cloned()
    }
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String> {
        self.folders.borrow().get(&(workspace_id.to_string(), session.to_string())).cloned()
    }
    fn agent_label(&self, agent: &str) -> String {
        match agent {
            "claude" => "Claude Code".into(),
            "codex" => "Codex".into(),
            other => other.to_string(),
        }
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined> {
        self.call(format!("keptWrites {workspace_id}"));
        self.kept_writes.borrow().get(workspace_id).cloned()
    }
    fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<String>,
        attachments: Vec<Attachment>,
    ) -> LocalBoxFuture<'static, ()> {
        self.call(format!("keepSentImages {workspace_id} {thread_id} {} {}", request_id.clone().unwrap_or_default(), attachments.len()));
        self.kept_images.borrow_mut().push((workspace_id.to_string(), thread_id.to_string(), request_id, attachments));
        async {}.boxed_local()
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.call(format!("persist {workspace_id}"));
        async {}.boxed_local()
    }
    fn project_held(&self, project: &str) -> ProjectView {
        self.projects
            .borrow()
            .get(project)
            .cloned()
            .unwrap_or_else(|| project_view(project, project, &format!("/repo/{project}"), Some(&format!("/repo/{project}"))))
    }
    fn place_name(&self, computer: &str) -> String {
        format!("{computer}-name")
    }
    fn git_here(&self, top: &str, args: Vec<String>) -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
        let joined = args.join(" ");
        self.call(format!("git {top} {joined}"));
        let answer = self
            .git
            .borrow()
            .iter()
            .find(|(needle, _)| joined.contains(needle.as_str()))
            .map(|(_, a)| a.clone())
            .unwrap_or_else(|| Ok(exec_result(0, "", "")));
        async move { answer }.boxed_local()
    }
    fn live_ids(&self) -> Vec<String> {
        self.live.borrow().clone()
    }
    fn worktree_folder(
        &self,
        project: &ProjectView,
        top: &str,
        branch: &str,
        from: &str,
        root: Option<String>,
        at_ref: &str,
    ) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        self.call(format!("worktreeFolder {} {top} {branch} {from} {} {at_ref}", project.id, root.unwrap_or_else(|| "-".into())));
        let answer = self.worktrees.borrow_mut().pop_front().unwrap_or_else(|| Err(err("no worktree answer queued")));
        async move { answer }.boxed_local()
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        self.checkouts.borrow().get(&record.id).cloned().unwrap_or_else(|| format!("/work/{}", record.id))
    }
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        self.call(format!("takeSetupLine {workspace_id}"));
        self.setup_lines.borrow_mut().shift_remove(workspace_id)
    }
    fn remove_worktree(&self, entry: &Entry, force: bool) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call(format!("removeWorktree {} {force}", entry.borrow().record.borrow().id));
        let answer = match self.remove_fails.borrow().clone() {
            Some(message) => Err(err(&message)),
            None => Ok(()),
        };
        async move { answer }.boxed_local()
    }
}
