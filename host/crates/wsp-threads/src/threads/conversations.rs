// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/conversations.ts` at 65f6b89c7: the agents' own conversations outside
//! wsp. Spec: `spec/conversations.md`.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{join, ready, try_join_all, FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_adapters::port::{
    ConversationLine, ConversationRoad, ConversationStore, EarlierAsk, EarlierRead, ExecFailure, ExecFailureKind, StoreError,
    StoredConversation,
};
use wsp_catalog::catalog::catalog_agents;
use wsp_core::clock::Clock;
use wsp_core::console;
use wsp_core::context::StateCell;
use wsp_core::types::api::{ConversationAsk, OutsideOpening, Placed, PlacedStart};
use wsp_core::types::events::EventBus;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::conversations::{
    ConversationOrigin, ConversationRead, ConversationReadRowsItem, ConversationReadRowsItemWho, ConversationsAnswer, ConversationsHeld,
    OutsideConversation, ResumeAsk, CONVERSATIONS_AGAIN_FIX, CONVERSATION_GONE_FIX, CONVERSATION_OPEN_KIND, NO_CONVERSATIONS_FIX,
    RESUME_WHERE_FIX, RESUME_WHERE_LINE,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::{SessionEarlierEvent, SessionEarlierEventWho};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::conversations::{
    conversation_gone_line, conversation_is_thread_fix, conversation_is_thread_line, conversation_open_fix, conversation_open_line,
    conversations_failed_line, conversations_unread_line, conversations_update_fix, earlier_line, lasting_store_error,
    no_conversations_line,
};
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::js::collate;
use wsp_words::words::tools::{tool_activity_line, tool_done_line};

use super::threads::{Entry, Sessions};
use super::transcripts::TranscriptState;
use wsp_account::account::core_types::AccountError;
use wsp_account::projects::folders::place::FolderPlaced;
use wsp_account::projects::folders::tree::ListedWorktree;
use wsp_wire::views::workspace::ProjectView;

/// How long one read of an agent's store on the project's computer may take.
pub const STORE_EXEC_MS: u64 = 30_000;
/// How long a project's kept list answers a visit while its stores' stamp stays the same and no thread of it ends.
pub const KEPT_LIST_MS: i64 = 60_000;

pub use wsp_wire::types::conversations::{EARLIER_MESSAGES, PREVIEW_MESSAGES, PREVIEW_TAIL_BYTES};
/// conversations.ts:53 `StoreRead`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreRead {
    pub listed: Vec<StoredConversation>,
    pub live: Option<IndexSet<String>>,
    pub held: Option<ConversationsHeld>,
    pub passing: bool,
}

/// conversations.ts:61 `Kept`.
#[derive(Clone)]
pub struct Kept {
    pub at: i64,
    pub cwds: String,
    pub stamp: String,
    pub read: Shared<LocalBoxFuture<'static, Result<StoreRead, HostError>>>,
}

/// Every `ctx.<x>` the conversations area calls (README R2).
pub trait ConversationsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn bus(&self) -> Rc<dyn EventBus>;
    fn sessions(&self) -> Sessions;
    fn transcript_state(&self) -> StateCell<TranscriptState>;
    /// `ctx.live.get(ws)?.record.project`.
    fn live_project(&self, workspace_id: &str) -> Option<String>;
    fn with_daemon(&self, entry: &Entry, frame: Value) -> LocalBoxFuture<'static, Result<Value, AccountError>>;
    /// `harnessExec(machine, ms)` as a road's exec.
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn wsp_adapters::port::HarnessExec>;
    fn project_held(&self, project: &str) -> Result<ProjectView, HostError>;
    fn worktrees_of(&self, top: &str) -> LocalBoxFuture<'static, Result<Vec<ListedWorktree>, HostError>>;
    /// `ctx.foldersOf(project)`: every folder record of the project.
    fn folders_of(&self, project: &str) -> Vec<WorkspaceRecord>;
    fn has_adapter(&self, harness: &str) -> bool;
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool;
    /// `ctx.adapterFor(entry, harness).adapter.conversations`.
    fn conversation_store(&self, entry: &Entry, harness: &str) -> Option<Arc<dyn ConversationStore>>;
    fn agent_label(&self, agent: &str) -> String;
    fn computer_of(&self, entry: &Entry) -> String;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn reaches_row(&self, row: &SessionView, origin: Option<&Caller>) -> Result<bool, HostError>;
    fn folder_for(
        &self,
        project: Option<String>,
        cwd: Option<String>,
        origin: Option<Caller>,
    ) -> LocalBoxFuture<'static, Result<FolderPlaced, HostError>>;
    fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    /// `ctx.state.preferencesHeld ?? await ctx.preferences.get()`.
    fn preferences(&self) -> LocalBoxFuture<'static, Result<Preferences, HostError>>;
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String;
}

/// conversations.ts:203 `earlierRows`: V10.
pub fn earlier_rows(lines: &[ConversationLine], left: u32, label: &str) -> Vec<SessionEarlierEvent> {
    let at = |line: Option<&ConversationLine>| line.and_then(|l| l.at).map(JsNumber);
    let row = |who: SessionEarlierEventWho, text: String, at: Option<JsNumber>| SessionEarlierEvent {
        workspace_id: String::new(),
        session_id: String::new(),
        at,
        turn_id: None,
        thread_id: None,
        pos: None,
        copied_from: None,
        who,
        text,
    };
    let mut out = Vec::new();
    if left > 0 {
        out.push(row(SessionEarlierEventWho::Note, earlier_line(f64::from(left), label), at(lines.first())));
    }
    for line in lines {
        let (who, text) = match line.who.as_str() {
            "tool" => (
                SessionEarlierEventWho::Tool,
                tool_done_line(line.tool.as_deref(), &line.text).unwrap_or_else(|| tool_activity_line(line.tool.as_deref(), &line.text)),
            ),
            "person" => (SessionEarlierEventWho::Person, line.text.clone()),
            _ => (SessionEarlierEventWho::Agent, line.text.clone()),
        };
        out.push(row(who, text, at(Some(line))));
    }
    out
}

/// The `kept` key of a project's read by one agent: conversations.ts:74.
fn kept_key(project: &str, harness: &str) -> String {
    format!("{project}\0{harness}")
}

/// conversations.ts:114 `unreadOf`: a daemon from before the op answers the update its computer needs.
fn unread_held(ctx: &dyn ConversationsCtx, said: &str, entry: &Entry, harness: &str) -> Option<ConversationsHeld> {
    if !said.starts_with(&unknown_op_line("")) {
        return None;
    }
    let computer = ctx.computer_of(entry);
    Some(ConversationsHeld {
        agent: harness.to_string(),
        said: conversations_unread_line(&ctx.agent_label(harness), &computer),
        fix: conversations_update_fix(&computer),
        why: None,
    })
}

/// The refusal a failed store read becomes where the caller gets it (conversations.ts:220, :242): the update line for a
/// daemon too old, else the store's own error.
fn store_failure(ctx: &dyn ConversationsCtx, e: StoreError, entry: &Entry, harness: &str) -> HostError {
    match unread_held(ctx, &e.message, entry, harness) {
        Some(held) => refusal(&held.said, &held.fix, Some(Kind::Usage)),
        None if e.lasting => lasting_store_error(&e.message),
        None => HostError::new(e.message),
    }
}

fn read_row(row: SessionEarlierEvent) -> ConversationReadRowsItem {
    let who = match row.who {
        SessionEarlierEventWho::Person => ConversationReadRowsItemWho::Person,
        SessionEarlierEventWho::Agent => ConversationReadRowsItemWho::Agent,
        SessionEarlierEventWho::Tool => ConversationReadRowsItemWho::Tool,
        SessionEarlierEventWho::Note => ConversationReadRowsItemWho::Note,
    };
    ConversationReadRowsItem { who, text: row.text, at: row.at }
}

/// conversations.ts:125 `readStore`: one store read fresh, the list and the live ids together.
async fn read_store_of(
    ctx: Rc<dyn ConversationsCtx>,
    entry: Entry,
    cwds: Vec<String>,
    road: ConversationRoad,
    harness: String,
    store: Arc<dyn ConversationStore>,
) -> Result<StoreRead, HostError> {
    let list = store.list(cwds, road.clone());
    let live_call = store.live(road);
    let live = async {
        match live_call {
            Some(fut) => fut.await,
            None => None,
        }
    };
    let (listed, live) = join(list, live).await;
    let mut held = None;
    let mut passing = false;
    let listed = match listed {
        Ok(rows) => rows,
        Err(e) => {
            held = unread_held(ctx.as_ref(), &e.message, &entry, &harness);
            if held.is_some() {
                Vec::new()
            } else {
                let label = ctx.agent_label(&harness);
                let computer = ctx.computer_of(&entry);
                let why = e.message.clone();
                console::warn(&format!("the {label} conversations on {computer} were not read: {why}"));
                if e.lasting {
                    Vec::new()
                } else {
                    passing = true;
                    held = Some(ConversationsHeld {
                        agent: harness.clone(),
                        said: conversations_failed_line(&label, &why),
                        fix: CONVERSATIONS_AGAIN_FIX.to_string(),
                        why: Some(why),
                    });
                    Vec::new()
                }
            }
        }
    };
    Ok(StoreRead { listed, live, held, passing })
}

/// The conversations area (`conversationsArea`, conversations.ts:68).
pub struct ConversationsArea {
    pub(crate) ctx: Rc<dyn ConversationsCtx>,
    pub(crate) me: Weak<ConversationsArea>,
    /// conversations.ts:73
    pub kept: RefCell<IndexMap<String, Kept>>,
}

impl ConversationsArea {
    /// Builds the area and registers the forgets on `session.done` and `session.end` (conversations.ts:79-80).
    pub fn new(ctx: Rc<dyn ConversationsCtx>) -> Rc<ConversationsArea> {
        let bus = ctx.bus();
        let area = Rc::new_cyclic(|me| ConversationsArea { ctx: ctx.clone(), me: me.clone(), kept: RefCell::default() });
        for ty in ["session.done", "session.end"] {
            let weak = area.me.clone();
            drop(bus.on(
                ty,
                Rc::new(move |e: &EventUnion| {
                    if let Some(area) = weak.upgrade() {
                        match e {
                            EventUnion::SessionDone(v) => area.forget(&v.event.workspace_id),
                            EventUnion::SessionEnd(v) => area.forget(&v.event.workspace_id),
                            _ => {}
                        }
                    }
                    Ok(())
                }),
            ));
        }
        area
    }
    /// conversations.ts:75: V1.
    pub fn forget(&self, workspace_id: &str) {
        let Some(project) = self.ctx.live_project(workspace_id) else { return };
        let prefix = kept_key(&project, "");
        self.kept.borrow_mut().retain(|key, _| !key.starts_with(&prefix));
    }
    /// conversations.ts:83: V2. Agent session to thread.
    pub fn wsp_sessions(&self) -> IndexMap<String, String> {
        let mut mine = IndexMap::new();
        self.ctx.transcript_state().with(|t| {
            for index in t.index.values() {
                for (thread, session) in &index.starts {
                    mine.insert(session.clone(), thread.clone());
                }
            }
        });
        let sessions = self.ctx.sessions();
        for row in sessions.borrow().values() {
            let row = row.borrow();
            if let (Some(session), Some(thread)) = (&row.view.claude_session_id, &row.view.thread_id) {
                mine.insert(session.clone(), thread.clone());
            }
        }
        mine
    }
    /// conversations.ts:90: V3.
    pub fn road_of(&self, entry: &Entry) -> ConversationRoad {
        let ctx = self.ctx.clone();
        let owned = entry.clone();
        ConversationRoad {
            exec: self.ctx.harness_exec(entry, STORE_EXEC_MS),
            ask: Arc::new(move |frame: Map<String, Value>| {
                let ctx = ctx.clone();
                let entry = owned.clone();
                async move {
                    match ctx.with_daemon(&entry, Value::Object(frame)).await {
                        Ok(Value::Object(reply)) => Ok(reply),
                        Ok(_) => Ok(Map::new()),
                        Err(e) => Err(ExecFailure { kind: ExecFailureKind::Other, message: e.message() }),
                    }
                }
                .boxed_local()
            }),
        }
    }
    /// conversations.ts:97: V4.
    pub async fn folders_of(&self, entry: &Entry) -> Result<Vec<String>, HostError> {
        let record = entry.borrow().record.borrow().clone();
        let project = self.ctx.project_held(&record.project)?;
        let top = project.git.as_ref().map(|git| git.top.clone());
        let trees = match top {
            Some(top) if wsp_words::projects::copies_folder(&record.kind) => {
                self.ctx.worktrees_of(&top).await?.into_iter().map(|w| w.path).collect()
            }
            _ => Vec::new(),
        };
        let held = self.ctx.folders_of(&project.id).into_iter().filter_map(|r| r.worktree.filter(|w| w.gone.is_none()).map(|w| w.path));
        let mut folders = IndexSet::new();
        folders.insert(project.path.clone());
        folders.extend(trees);
        folders.extend(held);
        Ok(folders.into_iter().collect())
    }
    /// conversations.ts:106: V5.
    pub fn stores_on(&self, entry: &Entry, agent: Option<&str>) -> Vec<(String, Arc<dyn ConversationStore>)> {
        catalog_agents()
            .into_iter()
            .filter(|a| agent.is_none_or(|named| a.id == named))
            .filter(|a| self.ctx.has_adapter(&a.id) && !self.ctx.agent_off(entry, &a.id))
            .filter_map(|a| self.ctx.conversation_store(entry, &a.id).map(|store| (a.id.clone(), store)))
            .collect()
    }
    /// conversations.ts:114: V6.
    pub fn unread_of(&self, said: &str, entry: &Entry, harness: &str) -> Option<ConversationsHeld> {
        unread_held(self.ctx.as_ref(), said, entry, harness)
    }
    /// conversations.ts:125: V7.
    pub async fn read_store(
        &self,
        entry: &Entry,
        cwds: &[String],
        road: ConversationRoad,
        harness: &str,
        store: Arc<dyn ConversationStore>,
    ) -> Result<StoreRead, HostError> {
        read_store_of(self.ctx.clone(), entry.clone(), cwds.to_vec(), road, harness.to_string(), store).await
    }
    /// Whether the kept entry under `key` is the one `was` was read as (the same folders, stamp and time).
    fn kept_is(&self, key: &str, was: &Kept) -> bool {
        self.kept.borrow().get(key).is_some_and(|k| k.at == was.at && k.cwds == was.cwds && k.stamp == was.stamp)
    }
    /// conversations.ts:147: V8.
    pub async fn kept_read(
        &self,
        entry: &Entry,
        cwds: &[String],
        road: ConversationRoad,
        harness: &str,
        store: Arc<dyn ConversationStore>,
        mine: &IndexMap<String, String>,
    ) -> Result<StoreRead, HostError> {
        let now = self.ctx.clock().now();
        self.kept.borrow_mut().retain(|_, k| now - k.at < KEPT_LIST_MS);
        let project = entry.borrow().record.borrow().project.clone();
        let key = kept_key(&project, harness);
        let folders = cwds.join("\0");
        let stamp = match store.stamp(cwds.to_vec(), road.clone()) {
            Some(fut) => fut.await.unwrap_or_default(),
            None => String::new(),
        };
        let was = self.kept.borrow().get(&key).cloned();
        if let Some(was) = was.filter(|k| k.cwds == folders && k.stamp == stamp) {
            let held = was.read.clone().await?;
            let shows_open =
                held.listed.iter().any(|row| !mine.contains_key(&row.id) && held.live.as_ref().is_some_and(|live| live.contains(&row.id)));
            if !shows_open {
                return Ok(held);
            }
            let live = match store.live(road) {
                Some(fut) => fut.await,
                None => None,
            };
            let fresh = StoreRead { live, ..held };
            if self.kept_is(&key, &was) {
                let read = ready(Ok(fresh.clone())).boxed_local().shared();
                self.kept.borrow_mut().insert(key, Kept { read, ..was });
            }
            return Ok(fresh);
        }
        let read = read_store_of(self.ctx.clone(), entry.clone(), cwds.to_vec(), road, harness.to_string(), store).boxed_local().shared();
        let mine_kept = Kept { at: self.ctx.clock().now(), cwds: folders, stamp, read: read.clone() };
        self.kept.borrow_mut().insert(key.clone(), mine_kept.clone());
        let result = read.await;
        let dropped = match &result {
            Ok(r) => r.passing,
            Err(_) => true,
        };
        if dropped && self.kept_is(&key, &mine_kept) {
            self.kept.borrow_mut().shift_remove(&key);
        }
        result
    }
    /// conversations.ts:168: V9.
    pub async fn list(
        &self,
        project: Option<String>,
        agent: Option<String>,
        origin: Option<Caller>,
    ) -> Result<ConversationsAnswer, HostError> {
        let entry = self.ctx.folder_for(project, None, origin.clone()).await?.entry;
        let stores = self.stores_on(&entry, agent.as_deref());
        if let Some(agent) = &agent {
            if stores.is_empty() {
                return Err(refusal(&no_conversations_line(&self.ctx.agent_label(agent)), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
            }
        }
        let cwds = self.folders_of(&entry).await?;
        let road = self.road_of(&entry);
        let mine = self.wsp_sessions();
        let held: RefCell<Vec<ConversationsHeld>> = RefCell::default();
        let rows: RefCell<Vec<OutsideConversation>> = RefCell::default();
        let (entry_r, cwds_r, road_r, mine_r, held_r, rows_r, origin_r) = (&entry, &cwds, &road, &mine, &held, &rows, &origin);
        let work = stores.into_iter().map(|(harness, store)| async move {
            let read = self.kept_read(entry_r, cwds_r, road_r.clone(), &harness, store.clone(), mine_r).await?;
            if let Some(unread) = read.held {
                held_r.borrow_mut().push(unread);
            }
            for row in read.listed {
                let thread = mine_r.get(&row.id).cloned();
                // A script's conversation is nobody's to pick up, unless it is a wsp thread's.
                if row.origin == ConversationOrigin::Script && thread.is_none() {
                    continue;
                }
                let reached = thread.as_ref().and_then(|t| self.ctx.latest_on(t));
                let reaches = match reached {
                    Some(r) => self.ctx.reaches_row(&r, origin_r.as_ref())?,
                    None => false,
                };
                let wsp = thread.is_some();
                let live = !wsp && read.live.as_ref().is_some_and(|live| live.contains(&row.id));
                rows_r.borrow_mut().push(OutsideConversation {
                    agent: harness.clone(),
                    id: row.id,
                    title: row.title,
                    first_prompt: row.first_prompt,
                    branch: row.branch,
                    cwd: row.cwd,
                    last_at: JsNumber(row.last_at),
                    bytes: row.bytes.map(JsNumber),
                    origin: if wsp { ConversationOrigin::Wsp } else { row.origin },
                    live,
                    thread: if reaches { thread } else { None },
                    lets_go: store.lets_go(),
                });
            }
            Ok::<(), HostError>(())
        });
        try_join_all(work).await?;
        let mut rows = rows.into_inner();
        rows.sort_by(|a, b| b.last_at.0.partial_cmp(&a.last_at.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| collate(&a.id, &b.id)));
        Ok(ConversationsAnswer { rows, held: held.into_inner() })
    }
    /// conversations.ts:213: V11.
    pub async fn read(&self, ask: ConversationAsk, origin: Option<Caller>) -> Result<ConversationRead, HostError> {
        let entry = self.ctx.folder_for(ask.project.clone(), None, origin).await?.entry;
        let label = self.ctx.agent_label(&ask.agent);
        let Some((_, store)) = self.stores_on(&entry, Some(&ask.agent)).into_iter().next() else {
            return Err(refusal(&no_conversations_line(&label), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        };
        let preview = ask.preview == Some(true);
        let cwds = self.folders_of(&entry).await?;
        let road = self.road_of(&entry);
        let want = EarlierAsk {
            cwds,
            last: if preview { PREVIEW_MESSAGES as u32 } else { EARLIER_MESSAGES as u32 },
            probe: false,
            tail: preview.then_some(PREVIEW_TAIL_BYTES),
            count: !preview,
        };
        let got = store.earlier(ask.id.clone(), want, road).await.map_err(|e| store_failure(self.ctx.as_ref(), e, &entry, &ask.agent))?;
        let EarlierRead::Read(got) = got else {
            let computer = self.ctx.computer_of(&entry);
            return Err(refusal(&conversation_gone_line(&label, &ask.id, &computer), CONVERSATION_GONE_FIX, Some(Kind::NotFound)));
        };
        let left = if preview { 0 } else { got.earlier as u32 };
        Ok(ConversationRead {
            cwd: got.cwd,
            title: got.title.unwrap_or(ask.id),
            rows: earlier_rows(&got.lines, left, &label).into_iter().map(read_row).collect(),
        })
    }
    /// conversations.ts:228: V12.
    pub async fn opening(
        &self,
        workspace_id: &str,
        harness: Option<String>,
        resume: ResumeAsk,
        origin: Option<Caller>,
    ) -> Result<OutsideOpening, HostError> {
        let entry = self.ctx.entry_of(workspace_id, origin).await?;
        let prefs = self.ctx.preferences().await?;
        let harness = match harness {
            Some(harness) => harness,
            None => self.ctx.default_agent_of(&prefs, &entry),
        };
        let label = self.ctx.agent_label(&harness);
        let store = if self.ctx.has_adapter(&harness) { self.ctx.conversation_store(&entry, &harness) } else { None };
        let Some(store) = store else {
            return Err(refusal(&no_conversations_line(&label), NO_CONVERSATIONS_FIX, Some(Kind::Usage)));
        };
        let id = resume.id.clone();
        let copy = resume.copy == Some(true);
        if let Some(thread) = self.wsp_sessions().get(&id) {
            return Err(refusal(&conversation_is_thread_line(&id, thread), &conversation_is_thread_fix(thread), Some(Kind::Usage)));
        }
        let cwds = self.folders_of(&entry).await?;
        let road = self.road_of(&entry);
        let computer = self.ctx.computer_of(&entry);
        let ask = |probe: bool, last: u32| EarlierAsk { cwds: cwds.clone(), last, probe, tail: None, count: true };
        let gone = || refusal(&conversation_gone_line(&label, &id, &computer), CONVERSATION_GONE_FIX, Some(Kind::NotFound));
        let read = store
            .earlier(id.clone(), ask(!copy, EARLIER_MESSAGES as u32), road.clone())
            .await
            .map_err(|e| store_failure(self.ctx.as_ref(), e, &entry, &harness))?;
        if let EarlierRead::Gone = read {
            return Err(gone());
        }
        let named = match &read {
            EarlierRead::Open => store
                .earlier(id.clone(), ask(false, 0), road.clone())
                .await
                .map_err(|e| store_failure(self.ctx.as_ref(), e, &entry, &harness))?,
            _ => read.clone(),
        };
        let title = match &named {
            EarlierRead::Read(c) => c.title.clone().unwrap_or_else(|| id.clone()),
            _ => id.clone(),
        };
        let open = match &read {
            EarlierRead::Open => true,
            _ if copy => false,
            _ => match store.live(road) {
                Some(fut) => fut.await.is_some_and(|live| live.contains(&id)),
                None => false,
            },
        };
        if open {
            return Err(refusal(
                &conversation_open_line(&title, &computer),
                &conversation_open_fix(store.lets_go().as_deref()),
                Some(Kind::parse(CONVERSATION_OPEN_KIND)),
            ));
        }
        let EarlierRead::Read(read) = read else { return Err(gone()) };
        let project = entry.borrow().record.borrow().project.clone();
        Ok(OutsideOpening {
            harness,
            id,
            copy,
            project,
            cwd: read.cwd.clone(),
            title,
            earlier: earlier_rows(&read.lines, read.earlier as u32, &label),
        })
    }
    /// conversations.ts:264: V13.
    pub async fn placed(&self, start: PlacedStart, resume: ResumeAsk, origin: Option<Caller>) -> Result<Placed, HostError> {
        if start.thread.is_some() || start.branch.is_some() || start.cwd.is_some() {
            return Err(refusal(RESUME_WHERE_LINE, RESUME_WHERE_FIX, Some(Kind::Usage)));
        }
        let base = match start.workspace_id.clone() {
            Some(id) => id,
            None => {
                let folder = self.ctx.folder_for(start.project.clone(), None, origin.clone()).await?;
                let id = folder.entry.borrow().record.borrow().id.clone();
                id
            }
        };
        let outside = self.opening(&base, start.harness.clone(), resume, origin.clone()).await?;
        let at = self.ctx.folder_for(Some(outside.project.clone()), outside.cwd.clone(), origin).await?;
        let workspace_id = at.entry.borrow().record.borrow().id.clone();
        Ok(Placed { workspace_id, cwd: at.cwd, outside })
    }
}
