// SPDX-License-Identifier: AGPL-3.0-only
//! The conversations area's fake context and fake conversation store. The context keeps the runtime's rows and
//! transcript state in `StateCell`s, answers each call out from what a test configured, and logs every call as
//! `name args` in order. The store answers each of its reads from a script, repeats its last answer once the script
//! runs out, and logs every call it gets.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_adapters::port::{
    ConversationEarlier, ConversationLine, ConversationOrigin, ConversationStore, EarlierAsk, EarlierRead, Env, ExecFailure, HarnessExec, StoreError,
    StoredConversation,
};
use wsp_core::clock::Clock;
use wsp_core::context::StateCell;
use wsp_account::account::core_types::AccountError;
use wsp_account::projects::folders::place::FolderPlaced;
use wsp_account::projects::folders::tree::ListedWorktree;
use wsp_core::types::events::EventBus;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::workspace::ProjectView;
use wsp_threads::threads::conversations::{ConversationsArea, ConversationsCtx};
use wsp_threads::threads::threads::{Entry, Sessions};
use wsp_threads::threads::transcripts::TranscriptState;
use wsp_wire::views::preferences::{default_preferences, Preferences};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{err, BusHandle, TestBus, TestClock};

// ---- the store ---------------------------------------------------------------------------------------------------

/// Answers in the order given, then the last one again; `None` before any was given.
struct Script<T: Clone> {
    queue: VecDeque<T>,
    last: Option<T>,
}

impl<T: Clone> Script<T> {
    fn new() -> Script<T> {
        Script { queue: VecDeque::new(), last: None }
    }
    fn set(&mut self, answers: Vec<T>) {
        self.queue = answers.into();
        self.last = None;
    }
    fn next(&mut self) -> Option<T> {
        if let Some(x) = self.queue.pop_front() {
            self.last = Some(x.clone());
            return Some(x);
        }
        self.last.clone()
    }
}

struct StoreInner {
    lists: Script<Result<Vec<StoredConversation>, StoreError>>,
    lives: Script<Option<IndexSet<String>>>,
    stamps: Script<Result<String, StoreError>>,
    earliers: Script<Result<EarlierRead, StoreError>>,
    earlier_probe: Option<Result<EarlierRead, StoreError>>,
    has_live: bool,
    has_stamp: bool,
    lets_go: Option<String>,
    log: Vec<String>,
    list_cwds: Vec<Vec<String>>,
    stamp_cwds: Vec<Vec<String>>,
    earlier_asks: Vec<(String, EarlierAsk)>,
}

/// A conversation store whose list, live ids, stamp and earlier answers a test scripts per call. `live` and `stamp`
/// exist only once a test gave them answers, as an adapter's optional members do.
pub struct FakeConversationStore {
    inner: Mutex<StoreInner>,
}

impl FakeConversationStore {
    pub fn new() -> Arc<FakeConversationStore> {
        Arc::new(FakeConversationStore {
            inner: Mutex::new(StoreInner {
                lists: Script::new(),
                lives: Script::new(),
                stamps: Script::new(),
                earliers: Script::new(),
                earlier_probe: None,
                has_live: false,
                has_stamp: false,
                lets_go: None,
                log: vec![],
                list_cwds: vec![],
                stamp_cwds: vec![],
                earlier_asks: vec![],
            }),
        })
    }
    fn with<R>(&self, f: impl FnOnce(&mut StoreInner) -> R) -> R {
        f(&mut self.inner.lock().expect("store"))
    }
    /// Every list answers these rows.
    pub fn rows(&self, rows: Vec<StoredConversation>) -> &Self {
        self.with(|s| s.lists.set(vec![Ok(rows)]));
        self
    }
    /// The lists answer these, one per call, the last again after.
    pub fn list_answers(&self, answers: Vec<Result<Vec<StoredConversation>, StoreError>>) -> &Self {
        self.with(|s| s.lists.set(answers));
        self
    }
    /// The store reads live ids; each call answers one of these (`None` is the TS `null`).
    pub fn live_answers(&self, answers: Vec<Option<IndexSet<String>>>) -> &Self {
        self.with(|s| {
            s.has_live = true;
            s.lives.set(answers);
        });
        self
    }
    /// The store has a stamp; each call answers one of these.
    pub fn stamp_answers(&self, answers: Vec<Result<String, StoreError>>) -> &Self {
        self.with(|s| {
            s.has_stamp = true;
            s.stamps.set(answers);
        });
        self
    }
    /// The `earlier` reads answer these, one per call, the last again after.
    pub fn earlier_answers(&self, answers: Vec<Result<EarlierRead, StoreError>>) -> &Self {
        self.with(|s| s.earliers.set(answers));
        self
    }
    /// A read asked with `probe: true` answers this whatever the script says.
    pub fn earlier_on_probe(&self, answer: Result<EarlierRead, StoreError>) -> &Self {
        self.with(|s| s.earlier_probe = Some(answer));
        self
    }
    pub fn lets_go_says(&self, line: &str) -> &Self {
        self.with(|s| s.lets_go = Some(line.to_string()));
        self
    }
    /// Calls of each kind, in order, as `list`, `live`, `stamp`, `earlier <id>`.
    pub fn log(&self) -> Vec<String> {
        self.with(|s| s.log.clone())
    }
    pub fn list_calls(&self) -> usize {
        self.with(|s| s.list_cwds.len())
    }
    pub fn live_calls(&self) -> usize {
        self.with(|s| s.log.iter().filter(|l| *l == "live").count())
    }
    pub fn stamp_calls(&self) -> usize {
        self.with(|s| s.stamp_cwds.len())
    }
    /// The folders each list was given.
    pub fn list_cwds(&self) -> Vec<Vec<String>> {
        self.with(|s| s.list_cwds.clone())
    }
    pub fn stamp_cwds(&self) -> Vec<Vec<String>> {
        self.with(|s| s.stamp_cwds.clone())
    }
    /// Every `earlier` ask with its id.
    pub fn earlier_asks(&self) -> Vec<(String, EarlierAsk)> {
        self.with(|s| s.earlier_asks.clone())
    }
}

impl ConversationStore for FakeConversationStore {
    fn list(&self, cwds: Vec<String>, _road: wsp_adapters::port::ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        let answer = self.with(|s| {
            s.log.push("list".into());
            s.list_cwds.push(cwds);
            s.lists.next()
        });
        async move { answer.unwrap_or_else(|| Ok(vec![])) }.boxed()
    }
    fn live(&self, _road: wsp_adapters::port::ConversationRoad) -> Option<LocalBoxFuture<'static, Option<IndexSet<String>>>> {
        let answer = self.with(|s| {
            if !s.has_live {
                return None;
            }
            s.log.push("live".into());
            Some(s.lives.next().unwrap_or_else(|| Some(IndexSet::new())))
        })?;
        Some(async move { answer }.boxed_local())
    }
    fn stamp(&self, cwds: Vec<String>, _road: wsp_adapters::port::ConversationRoad) -> Option<LocalBoxFuture<'static, Result<String, StoreError>>> {
        let answer = self.with(|s| {
            if !s.has_stamp {
                return None;
            }
            s.log.push("stamp".into());
            s.stamp_cwds.push(cwds);
            Some(s.stamps.next().unwrap_or_else(|| Ok(String::new())))
        })?;
        Some(async move { answer }.boxed_local())
    }
    fn earlier(&self, id: String, ask: EarlierAsk, _road: wsp_adapters::port::ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        let answer = self.with(|s| {
            s.log.push(format!("earlier {id}"));
            s.earlier_asks.push((id, ask.clone()));
            let scripted = s.earliers.next();
            match (&s.earlier_probe, ask.probe) {
                (Some(p), true) => p.clone(),
                _ => scripted.unwrap_or(Ok(EarlierRead::Gone)),
            }
        });
        async move { answer }.boxed()
    }
    fn lets_go(&self) -> Option<String> {
        self.with(|s| s.lets_go.clone())
    }
}

/// A stored conversation: title `t-<first four of the id>`, a terminal's.
pub fn stored(id: &str, cwd: &str, last_at: f64) -> StoredConversation {
    StoredConversation {
        id: id.to_string(),
        title: format!("t-{}", id.chars().take(4).collect::<String>()),
        first_prompt: None,
        branch: None,
        cwd: cwd.to_string(),
        last_at,
        bytes: None,
        origin: ConversationOrigin::Terminal,
    }
}

/// A plain store failure that passes; `lasting_error` is the one that lasts.
pub fn store_error(message: &str) -> StoreError {
    StoreError { message: message.to_string(), lasting: false }
}

pub fn lasting_error(message: &str) -> StoreError {
    StoreError { message: message.to_string(), lasting: true }
}

/// An earlier read of a conversation: cwd, title, lines as `(who, text)` and the count left out.
pub fn earlier(cwd: Option<&str>, title: Option<&str>, lines: Vec<ConversationLine>, left: u32) -> ConversationEarlier {
    ConversationEarlier { cwd: cwd.map(String::from), title: title.map(String::from), lines, earlier: f64::from(left) }
}

pub fn line(who: &str, text: &str) -> ConversationLine {
    ConversationLine { who: who.to_string(), text: text.to_string(), tool: None, at: None }
}

pub fn tool_line(tool: &str, text: &str) -> ConversationLine {
    ConversationLine { who: "tool".to_string(), text: text.to_string(), tool: Some(tool.to_string()), at: None }
}

/// The worktrees git lists at a top, by path alone.
pub fn wts(paths: &[&str]) -> Vec<ListedWorktree> {
    paths.iter().map(|p| ListedWorktree { path: p.to_string(), branch: None }).collect()
}

pub fn set_of(ids: &[&str]) -> IndexSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

// ---- the exec a road runs commands through -----------------------------------------------------------------------

struct FakeExec {
    commands: Arc<Mutex<Vec<String>>>,
}

impl HarnessExec for FakeExec {
    fn exec(&self, command: &str, _env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        self.commands.lock().expect("commands").push(command.to_string());
        async { Ok("exec-out".to_string()) }.boxed_local()
    }
}

// ---- the context -------------------------------------------------------------------------------------------------

pub struct FakeConversationsCtx {
    pub clock: Rc<TestClock>,
    pub bus: Rc<TestBus>,
    pub sessions: Sessions,
    pub transcript_state: StateCell<TranscriptState>,
    /// Workspace id to the project it is live on.
    pub live_projects: RefCell<IndexMap<String, String>>,
    pub projects: RefCell<IndexMap<String, ProjectView>>,
    pub worktrees: RefCell<IndexMap<String, Result<Vec<ListedWorktree>, HostError>>>,
    pub held_folders: RefCell<IndexMap<String, Vec<WorkspaceRecord>>>,
    pub adapters: RefCell<IndexSet<String>>,
    pub off: RefCell<IndexSet<String>>,
    pub stores: RefCell<IndexMap<String, Arc<dyn ConversationStore>>>,
    /// Rows the caller does not reach, by row id.
    pub unreached: RefCell<IndexSet<String>>,
    /// `(project, cwd)` to the folder answered; the first match wins.
    pub folder_answers: RefCell<Vec<(Option<String>, Option<String>, Result<FolderPlaced, HostError>)>>,
    pub default_folder: RefCell<Option<FolderPlaced>>,
    pub entries: RefCell<IndexMap<String, Entry>>,
    pub preferences_read: RefCell<Result<Preferences, HostError>>,
    pub default_agent: RefCell<String>,
    pub daemon: RefCell<Vec<(String, Result<Value, AccountError>)>>,
    pub frames: RefCell<Vec<Value>>,
    pub exec_timeouts: RefCell<Vec<u64>>,
    pub exec_commands: Arc<Mutex<Vec<String>>>,
    /// Every call out, as `name args`, in order.
    pub calls: RefCell<Vec<String>>,
}

impl FakeConversationsCtx {
    pub fn new() -> Rc<FakeConversationsCtx> {
        Rc::new(FakeConversationsCtx {
            clock: TestClock::new(1_000_000),
            bus: TestBus::new(),
            sessions: Sessions::default(),
            transcript_state: StateCell::new(TranscriptState::default()),
            live_projects: RefCell::default(),
            projects: RefCell::default(),
            worktrees: RefCell::default(),
            held_folders: RefCell::default(),
            adapters: RefCell::default(),
            off: RefCell::default(),
            stores: RefCell::default(),
            unreached: RefCell::default(),
            folder_answers: RefCell::default(),
            default_folder: RefCell::new(None),
            entries: RefCell::default(),
            preferences_read: RefCell::new(Ok(default_preferences())),
            default_agent: RefCell::new("claude".into()),
            daemon: RefCell::default(),
            frames: RefCell::default(),
            exec_timeouts: RefCell::default(),
            exec_commands: Arc::default(),
            calls: RefCell::default(),
        })
    }
    /// The area over this context.
    pub fn area(self: &Rc<Self>) -> Rc<ConversationsArea> {
        ConversationsArea::new(self.clone())
    }
    pub fn call(&self, line: String) {
        self.calls.borrow_mut().push(line);
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    /// The agent has an adapter whose store is this one.
    pub fn install(&self, harness: &str, store: Arc<FakeConversationStore>) {
        self.adapters.borrow_mut().insert(harness.to_string());
        self.stores.borrow_mut().insert(harness.to_string(), store);
    }
    /// The agent has an adapter that reads no conversations.
    pub fn install_without_store(&self, harness: &str) {
        self.adapters.borrow_mut().insert(harness.to_string());
    }
    pub fn add_entry(&self, entry: Entry) {
        let id = entry.borrow().record.borrow().id.clone();
        self.entries.borrow_mut().insert(id, entry);
    }
    /// Every `folderFor` with no more specific answer answers this folder.
    pub fn folder_is(&self, entry: Entry, cwd: Option<&str>) {
        *self.default_folder.borrow_mut() = Some(FolderPlaced { entry, cwd: cwd.map(String::from), setup: None });
    }
    pub fn project(&self, id: &str, name: &str, path: &str, top: Option<&str>) {
        self.projects.borrow_mut().insert(
            id.to_string(),
            project_view(id, name, path, top),
        );
    }
    pub fn daemon_answers(&self, op: &str, answer: Result<Value, AccountError>) {
        self.daemon.borrow_mut().push((op.to_string(), answer));
    }
}

impl ConversationsCtx for FakeConversationsCtx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        Rc::new(BusHandle(self.bus.clone()))
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn transcript_state(&self) -> StateCell<TranscriptState> {
        self.transcript_state.clone()
    }
    fn live_project(&self, workspace_id: &str) -> Option<String> {
        self.live_projects.borrow().get(workspace_id).cloned()
    }
    fn with_daemon(&self, _entry: &Entry, frame: Map<String, Value>) -> LocalBoxFuture<'static, Result<Value, AccountError>> {
        let frame = Value::Object(frame);
        let op = frame["op"].as_str().unwrap_or("").to_string();
        self.frames.borrow_mut().push(frame);
        let answer = self.daemon.borrow().iter().find(|(o, _)| *o == op).map(|(_, a)| a.clone());
        let answer = answer.unwrap_or_else(|| Err(AccountError::Host(err(&format!("no answer for {op}")))));
        async move { answer }.boxed_local()
    }
    fn harness_exec(&self, _entry: &Entry, timeout_ms: u64) -> Arc<dyn HarnessExec> {
        self.exec_timeouts.borrow_mut().push(timeout_ms);
        Arc::new(FakeExec { commands: self.exec_commands.clone() })
    }
    fn project_held(&self, project: &str) -> ProjectView {
        self.call(format!("projectHeld {project}"));
        self.projects.borrow().get(project).cloned().unwrap_or_else(|| project_view(project, project, &format!("/repo/{project}"), Some(&format!("/repo/{project}"))))
    }
    fn worktrees_of(&self, top: &str) -> LocalBoxFuture<'static, Result<Vec<ListedWorktree>, HostError>> {
        self.call(format!("worktreesOf {top}"));
        let answer = self.worktrees.borrow().get(top).cloned().unwrap_or(Ok(vec![]));
        async move { answer }.boxed_local()
    }
    fn folders_of(&self, project: &str) -> Vec<WorkspaceRecord> {
        self.held_folders.borrow().get(project).cloned().unwrap_or_default()
    }
    fn has_adapter(&self, harness: &str) -> bool {
        self.adapters.borrow().contains(harness)
    }
    fn agent_off(&self, _entry: &Entry, agent: &str) -> bool {
        self.off.borrow().contains(agent)
    }
    fn conversation_store(&self, _entry: &Entry, harness: &str) -> Option<Arc<dyn ConversationStore>> {
        self.stores.borrow().get(harness).cloned()
    }
    fn agent_label(&self, agent: &str) -> String {
        match agent {
            "claude" => "Claude Code".into(),
            "codex" => "Codex".into(),
            other => other.to_string(),
        }
    }
    fn computer_of(&self, entry: &Entry) -> String {
        format!("{}-computer", entry.borrow().record.borrow().id)
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
    fn reaches_row(&self, row: &SessionView, _origin: Option<&Caller>) -> bool {
        self.call(format!("reachesRow {}", row.id));
        !self.unreached.borrow().contains(&row.id)
    }
    fn folder_for(&self, project: Option<String>, cwd: Option<String>, _origin: Option<Caller>) -> LocalBoxFuture<'static, Result<FolderPlaced, HostError>> {
        self.call(format!("folderFor {} {}", project.clone().unwrap_or_default(), cwd.clone().unwrap_or_default()));
        let named = self.folder_answers.borrow().iter().find(|(p, c, _)| *p == project && *c == cwd).map(|(_, _, a)| a.clone());
        let answer = named.unwrap_or_else(|| self.default_folder.borrow().clone().ok_or_else(|| err("no folder answer")));
        async move { answer }.boxed_local()
    }
    fn entry_of(&self, workspace_id: &str, _origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        self.call(format!("entryOf {workspace_id}"));
        let answer = self.entries.borrow().get(workspace_id).cloned().ok_or_else(|| err(&format!("no workspace {workspace_id}")));
        async move { answer }.boxed_local()
    }
    fn preferences(&self) -> LocalBoxFuture<'static, Result<Preferences, HostError>> {
        self.call("preferences".into());
        let answer = self.preferences_read.borrow().clone();
        async move { answer }.boxed_local()
    }
    fn default_agent_of(&self, _prefs: &Preferences, _entry: &Entry) -> String {
        self.call("defaultAgentOf".into());
        self.default_agent.borrow().clone()
    }
}

/// A whole workspace record from JSON: `id`, `name`, `kind`, `project`, `phase` and the fields a record must carry
/// default, the fields given over them.
pub fn record_json(fields: Value) -> Value {
    let mut v = json!({
        "id": "ws1", "name": "ws1", "kind": "local", "golden": "snap_g", "createdAt": "2026-10-01T12:00:00.000Z",
        "project": "p1", "phase": "running", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    if fields.get("machineId").is_none() {
        v["machineId"] = json!(format!("m-{}", v["id"].as_str().unwrap_or("ws1")));
    }
    v
}

/// A live entry over a fake machine.
pub fn ws_entry(fields: Value) -> Entry {
    let v = record_json(fields);
    let machine = super::FakeMachine::new(v["machineId"].as_str().expect("a machine id"));
    let record: WorkspaceRecord = serde_json::from_value(v).expect("a WorkspaceRecord");
    super::entry::entry_over(record, machine)
}

/// A project view on this computer: a folder at `path`, with a git top where one is given.
pub fn project_view(id: &str, name: &str, path: &str, top: Option<&str>) -> ProjectView {
    let mut v = json!({
        "id": id, "name": name, "computer": "this", "source": { "kind": "folder", "path": path }, "path": path, "remote": "",
        "defaultBranch": "main", "memoryKey": id, "memoryDir": format!("/mem/{id}"), "createdAt": "2026-10-01T12:00:00.000Z",
    });
    if let Some(top) = top {
        v["git"] = json!({ "top": top });
    }
    serde_json::from_value(v).expect("a ProjectView")
}

/// A folder record of a project with a worktree under it, `gone` where the worktree is.
pub fn folder_record(id: &str, project: &str, worktree: Option<(&str, bool)>) -> WorkspaceRecord {
    let mut v = record_json(json!({ "id": id, "name": id, "project": project }));
    if let Some((path, gone)) = worktree {
        v["worktree"] = json!({ "path": path, "branch": "feat/x", "made": true });
        if gone {
            v["worktree"]["gone"] = json!(true);
        }
    }
    serde_json::from_value(v).expect("a WorkspaceRecord")
}
