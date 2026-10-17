// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar as today's app draws it (`apps/web/src/sidebar/`): the header, the four rows over the list, the list
//! of thread tiles in its sections with their trees and Settled, and the foot with the cloud card and the corner. It
//! reads and writes through `wsp-host-client` and follows the host's pushes; the list is virtual, so only the rows
//! on screen are built, whatever the number of threads.

pub mod alone;
pub mod chrome;
pub mod host;
pub mod model;
mod tile;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{v_flex, WindowExt as _};
use gpui_kit::{
    div, list, prelude::*, px, App, Context, Entity, EventEmitter, ListAlignment, ListOffset, ListState, Pixels, SharedString,
    Subscription, Task, Window,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{
    PreferencesSetRequest, SessionView, SessionsDeleteRequest, SessionsForgetRequest, SessionsInterruptRequest, SessionsMarkRequest,
    SessionsRenameRequest, SessionsRestoreRequest, SessionsSettleRequest,
};
use wsp_host_client::{HostClient, Op};

use crate::ui::status::hold_crabs;
use crate::ui::theme::{self, scoped};
use crate::ui::{ActivePalette, Glyph};
use host::{Change, Snapshot};
use model::{Folds, Item, Line, Prefs, Row, Section, Shape, Thread};

/// `--sidebar-width`'s default, its right edge included: the column the shell gives the sidebar.
pub const WIDTH: Pixels = px(256.);
/// How long Stop stands armed after its first press (`components/arm.ts`).
const ARMED: Duration = Duration::from_millis(2_000);

/// A thread as the shell names it to open it.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadItem {
    pub workspace_id: String,
    pub thread_id: String,
    pub project: String,
    pub title: String,
}

impl ThreadItem {
    /// A row of the host's thread list; a sub-thread lives under its lead and is not listed here.
    pub fn of(view: &SessionView) -> Option<ThreadItem> {
        if view.parent_thread_id.is_some() {
            return None;
        }
        Some(ThreadItem {
            workspace_id: view.workspace_id.clone(),
            thread_id: view.thread_id.clone().unwrap_or_else(|| view.id.clone()),
            project: view.project.as_ref().map_or_else(String::new, |p| p.name.clone()),
            title: view.harness_title.clone().or_else(|| view.prompt.clone()).unwrap_or_else(|| "New thread".to_owned()),
        })
    }

    fn of_thread(t: &Thread) -> ThreadItem {
        ThreadItem {
            workspace_id: t.workspace_id.clone(),
            thread_id: t.key.clone(),
            project: t.project_name.clone().unwrap_or_default(),
            title: t.title.clone(),
        }
    }
}

/// A thread picked in the list, for the shell to open.
pub struct Selected(pub ThreadItem);

/// What the sidebar asks of the window around it: screens it does not own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    NewThread,
    Search,
    Settings,
    Usage,
    Activity,
    AddCloud,
    AddProject,
}

/// What a settle or a delete does with processes a thread still runs beside its agent: ask first, stop them, or keep
/// them running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Processes {
    Ask,
    Stop,
    Keep,
}

impl Processes {
    fn word(self) -> &'static str {
        match self {
            Processes::Ask => "ask",
            Processes::Stop => "stop",
            Processes::Keep => "keep",
        }
    }
}

/// Where a dragged root is let go.
#[derive(Clone, Debug, PartialEq)]
enum DropTarget {
    Head(Section),
    Root(String),
}

/// A scroll driven by the clock, as the bench runs it: the offset is set from elapsed time on every frame.
struct Drive {
    px_per_s: f32,
    start: Instant,
    frames: Rc<RefCell<Vec<Instant>>>,
    done: bool,
}

/// The host and the runtime its calls run on.
#[derive(Clone)]
pub struct Link {
    pub host: Arc<HostClient>,
    pub runtime: Arc<Runtime>,
}

struct Following {
    task: tokio::task::AbortHandle,
    _receive: Task<()>,
}

impl Drop for Following {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Renaming {
    key: String,
    input: Entity<InputState>,
    _events: Subscription,
}

/// What the foot and the switchers read besides the threads.
#[derive(Default)]
struct Around {
    places: Vec<Value>,
    preferences: Option<Value>,
    job: Option<Value>,
    /// Workspace id to its kind: a folder's thread is deleted, a cloud's forgotten.
    kinds: HashMap<String, String>,
    projects: Vec<(String, String)>,
}

pub struct Sidebar {
    /// The shell's own list, kept so it can mark a thread before the host's read lands.
    items: Vec<ThreadItem>,
    selected: Option<String>,
    /// The open thread, where it was opened from the live list: the time fold leaves it out of Settled.
    held: Option<String>,
    /// The roots of every tree a thread of which works, which take Stop in place of Settle.
    working: HashSet<String>,
    note: Option<String>,
    refusal: Option<String>,
    rows: Vec<Row>,
    threads: Vec<Thread>,
    lines: Vec<Line>,
    /// What each drawn row stands for, read off the threads it was drawn from.
    line_ids: Vec<String>,
    folds: Folds,
    prefs: Prefs,
    around: Around,
    list: ListState,
    link: Option<Link>,
    following: Option<Following>,
    renaming: Option<Renaming>,
    armed: Option<(String, Instant)>,
    switcher_open: bool,
    drive: Option<Drive>,
    /// The clock the list was last read at, in ms since the epoch.
    now: f64,
    _clock: Task<()>,
}

impl EventEmitter<Selected> for Sidebar {}
impl EventEmitter<Asked> for Sidebar {}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| d.as_secs_f64() * 1000.)
}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, px(240.));
        let weak = cx.weak_entity();
        list.set_scroll_handler(move |_, _, cx| {
            hold_crabs(cx);
            _ = weak.update(cx, |_, cx| cx.notify());
        });
        // A working thread's time ticks each second; an age or a settle by time moves by the minute.
        let clock = cx.spawn(async move |this, cx| loop {
            let working = this.update(cx, |this, _| this.threads.iter().any(|t| t.status == model::Status::Running)).unwrap_or(false);
            cx.background_executor().timer(if working { Duration::from_secs(1) } else { Duration::from_secs(30) }).await;
            if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                break;
            }
        });
        Sidebar {
            items: Vec::new(),
            selected: None,
            held: None,
            working: HashSet::new(),
            note: Some("Connecting".to_owned()),
            refusal: None,
            rows: Vec::new(),
            threads: Vec::new(),
            lines: Vec::new(),
            line_ids: Vec::new(),
            folds: Folds::default(),
            prefs: Prefs::default(),
            around: Around::default(),
            list,
            link: None,
            following: None,
            renaming: None,
            armed: None,
            switcher_open: false,
            drive: None,
            now: now_ms(),
            _clock: clock,
        }
    }

    /// The shell's list, before the sidebar's own read lands.
    pub fn set_items(&mut self, items: Vec<ThreadItem>, cx: &mut Context<Self>) {
        self.items = items;
        if self.link.is_none() {
            self.note = self.items.is_empty().then(|| "No threads".to_owned());
        }
        cx.notify();
    }

    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.note = Some(note);
        cx.notify();
    }

    /// What the sidebar says over its list: the host's refusal of the last write, else connecting or the road to the
    /// host down.
    pub fn note(&self) -> Option<&str> {
        self.refusal.as_deref().or(self.note.as_deref())
    }

    /// The host refused a write: said over the list until the next write.
    fn refuse(&mut self, said: String, cx: &mut Context<Self>) {
        self.refusal = Some(said);
        cx.notify();
    }

    pub fn items(&self) -> &[ThreadItem] {
        &self.items
    }

    /// The rows the list draws, top to bottom.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn threads(&self) -> &[Thread] {
        &self.threads
    }

    pub fn list_state(&self) -> &ListState {
        &self.list
    }

    /// Scrolls the list from its top to its end at `px_per_s`, noting when each frame is presented.
    pub fn drive(&mut self, px_per_s: f32, start: Instant, frames: Rc<RefCell<Vec<Instant>>>, cx: &mut Context<Self>) {
        self.list.scroll_to(ListOffset::default());
        self.drive = Some(Drive { px_per_s, start, frames, done: false });
        cx.notify();
    }

    pub fn driven_to_end(&self) -> bool {
        self.drive.as_ref().is_some_and(|d| d.done)
    }

    /// Whether the cloud card stands in the foot.
    pub fn cloud_card_shown(&self) -> bool {
        self.offers_cloud()
    }

    /// Marks a thread as the open one, without asking the shell to open it.
    pub fn mark(&mut self, thread_id: &str, cx: &mut Context<Self>) -> Option<ThreadItem> {
        let item = self
            .threads
            .iter()
            .find(|t| t.key == thread_id)
            .map(ThreadItem::of_thread)
            .or_else(|| self.items.iter().find(|i| i.thread_id == thread_id).cloned())?;
        // A thread opened from Settled stays there; one opened from the live list is held out of the time fold.
        let settled = self
            .lines
            .iter()
            .any(|l| matches!(&l.item, Item::Tile(t) if t.shape == Shape::Settled && self.threads[t.thread].key == item.thread_id));
        self.held = (!settled).then(|| item.thread_id.clone());
        self.selected = Some(item.thread_id.clone());
        self.folds.open = self.selected.clone();
        self.rebuild(cx);
        Some(item)
    }

    fn select(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(item) = self.mark(key, cx) {
            cx.emit(Selected(item));
        }
    }

    /// The sections and trees as they stand now.
    fn view(&self) -> model::View<'_> {
        model::sidebar(&self.threads, &self.prefs, self.now, self.held.as_deref())
    }

    /// Where a root stands among the roots that move in its section, for the menu's moves.
    pub fn root_place(&self, key: &str) -> Option<model::RootPlace> {
        self.view().root_place(key)
    }

    /// Reads everything the sidebar draws from the host, then follows its pushes until the sidebar is dropped.
    pub fn attach(&mut self, link: Link, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded();
        let task = host::follow(link.host.clone(), &link.runtime, tx);
        let receive = cx.spawn(async move |this, cx| {
            while let Some(change) = rx.next().await {
                if this.update(cx, |this, cx| this.apply(change, cx)).is_err() {
                    break;
                }
            }
        });
        self.link = Some(link);
        self.following = Some(Following { task, _receive: receive });
    }

    /// One change the host pushed, or its first read.
    pub fn apply(&mut self, change: Change, cx: &mut Context<Self>) {
        match change {
            Change::Whole(snapshot) => self.take(*snapshot),
            Change::Row { id, row } => {
                let at = self.rows.iter().position(|r| r.id == id);
                match (at, row.as_ref().and_then(Row::of)) {
                    (Some(at), Some(row)) => {
                        // A pushed row may leave out the prompt the row it replaces carried.
                        let prompt = self.rows[at].prompt.take();
                        self.rows[at] = Row { prompt: row.prompt.clone().or(prompt), ..row };
                    }
                    (None, Some(row)) => self.rows.push(row),
                    (Some(at), None) => {
                        self.rows.remove(at);
                    }
                    (None, None) => {}
                }
            }
            Change::Preferences(p) => self.take_preferences(p),
            Change::Places(places) => self.around.places = places,
            Change::Job(job) => self.around.job = job,
            Change::Workspaces(w) => self.take_workspaces(&w),
            Change::Projects(p) => self.take_projects(&p),
            Change::Lost(why) => {
                self.note = Some(why);
                cx.notify();
                return;
            }
        }
        self.note = None;
        self.threads = model::fold_threads(&self.rows);
        self.rebuild(cx);
    }

    fn take(&mut self, s: Snapshot) {
        self.rows = s.sessions.iter().filter_map(Row::of).collect();
        self.around.places = s.places;
        self.around.job = s.job;
        self.take_workspaces(&s.workspaces);
        self.take_projects(&s.projects);
        self.take_preferences(s.preferences);
    }

    fn take_preferences(&mut self, p: Value) {
        self.prefs.settle_ms = match p.get("settleAfter").and_then(Value::as_str) {
            Some("15m") => Some(15. * 60_000.),
            Some("1h") => Some(3_600_000.),
            Some("1d") => Some(86_400_000.),
            Some("never") => None,
            _ => Some(model::SETTLE_MS_DEFAULT),
        };
        self.around.preferences = Some(p);
    }

    fn take_workspaces(&mut self, workspaces: &[Value]) {
        self.around.kinds =
            workspaces.iter().filter_map(|w| Some((w.get("id")?.as_str()?.to_owned(), w.get("kind")?.as_str()?.to_owned()))).collect();
    }

    fn take_projects(&mut self, projects: &[Value]) {
        self.around.projects =
            projects.iter().filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("name")?.as_str()?.to_owned()))).collect();
    }

    /// Reads the clock again: a working time, an age, or a thread settling by time.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.armed.as_ref().is_some_and(|(_, at)| at.elapsed() >= ARMED) {
            self.armed = None;
        }
        self.rebuild(cx);
    }

    /// Draws the list again from the threads, keeping the scroll where it was.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.now = now_ms();
        let lines = self.view().lines(&self.folds);
        let ids: Vec<String> = lines.iter().map(|l| self.identity(l)).collect();
        self.working = self.working_roots();
        if lines != self.lines {
            let shape_changed = lines.len() != self.lines.len()
                || lines
                    .iter()
                    .zip(&self.lines)
                    .any(|(a, b)| std::mem::discriminant(&a.item) != std::mem::discriminant(&b.item) || a.gap != b.gap);
            if shape_changed {
                // The row at the top stays at the top wherever it moved to, as the browser's scroll anchoring keeps it.
                let top = self.list.logical_scroll_top();
                let at = self.line_ids.get(top.item_ix).and_then(|a| ids.iter().position(|id| id == a)).unwrap_or(top.item_ix);
                self.list.reset(lines.len());
                self.list.scroll_to(ListOffset { item_ix: at.min(lines.len().saturating_sub(1)), offset_in_item: top.offset_in_item });
            }
            self.lines = lines;
            self.line_ids = ids;
        }
        cx.notify();
    }

    /// What a row stands for, kept across a rebuild: a thread's key, a section, a fold, or its place.
    fn identity(&self, line: &Line) -> String {
        match &line.item {
            Item::Tile(t) => format!("t:{}:{}", self.threads[t.thread].key, t.inbox),
            Item::Head { section, .. } => format!("h:{}", section.id()),
            Item::Group { attempt, .. } => format!("g:{attempt}"),
            Item::Finished { lead, .. } => format!("f:{}", self.threads[*lead].key),
            Item::Subagent { lead, index, .. } => format!("s:{}:{index}", self.threads[*lead].key),
            Item::More { lead, .. } => format!("m:{}", lead.map_or("", |l| self.threads[l].key.as_str())),
        }
    }

    /// The root of each tree a thread of which is running, read once per rebuild.
    fn working_roots(&self) -> HashSet<String> {
        let parent: HashMap<&str, &str> = self.threads.iter().filter_map(|t| Some((t.key.as_str(), t.parent.as_deref()?))).collect();
        let mut out = HashSet::new();
        for t in self.threads.iter().filter(|t| t.status == model::Status::Running) {
            let mut at = t.key.as_str();
            out.insert(at.to_owned());
            let mut steps = 0;
            while let Some(&up) = parent.get(at) {
                out.insert(up.to_owned());
                at = up;
                steps += 1;
                if steps > self.threads.len() {
                    break;
                }
            }
        }
        out
    }

    /// Whether a thread's workspace runs in a folder, where a thread is deleted; a cloud's thread is forgotten.
    pub(crate) fn in_folder(&self, t: &Thread) -> bool {
        matches!(self.around.kinds.get(&t.workspace_id).map(String::as_str), Some("local" | "place") | None)
    }

    fn offers_cloud(&self) -> bool {
        let Some(prefs) = &self.around.preferences else { return false };
        if prefs.get("cloudCardDismissed").and_then(Value::as_bool) == Some(true) {
            return false;
        }
        let places = &self.around.places;
        let kind = |p: &Value| p.get("kind").and_then(Value::as_str).unwrap_or_default().to_owned();
        let setting_up = places.iter().any(|p| p.get("setup").is_some_and(|s| !s.is_null()));
        let job_running = self
            .around
            .job
            .as_ref()
            .and_then(|j| j.get("phase").and_then(Value::as_str))
            .is_some_and(|phase| !matches!(phase, "done" | "cancelled"));
        !places.is_empty() && !places.iter().any(|p| kind(p) == "provider") && !setting_up && !job_running
    }

    fn thread(&self, key: &str) -> Option<&Thread> {
        self.threads.iter().find(|t| t.key == key)
    }

    /// The keys of a thread and every thread under it, which settle and restore take whole.
    fn tree_keys(&self, key: &str) -> Vec<String> {
        let mut out = vec![key.to_owned()];
        let mut at = 0;
        while at < out.len() {
            let parent = out[at].clone();
            out.extend(self.threads.iter().filter(|t| t.parent.as_deref() == Some(parent.as_str())).map(|t| t.key.clone()));
            at += 1;
        }
        out
    }

    fn tree_works(&self, key: &str) -> bool {
        self.working.contains(key)
    }

    /// One write; a refusal is said where the note stands, and the host's push redraws what changed.
    fn write<O: Op + serde::de::DeserializeOwned + Send + 'static>(&mut self, params: Value, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let task = link.runtime.spawn(host::write::<O>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let done = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string())));
            if let Err(failure) = done {
                _ = this.update(cx, |this, cx| this.refuse(failure.message, cx));
            }
        })
        .detach();
    }

    pub fn settle(&mut self, key: &str, cx: &mut Context<Self>) {
        self.settle_with(key, Processes::Ask, cx);
    }

    /// Settles a thread's tree; a thread the host leaves standing is said in the note, with the host's reason.
    pub fn settle_with(&mut self, key: &str, processes: Processes, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let ids = self.tree_keys(key);
        let params = json!({ "threadIds": ids, "processes": processes.word() });
        let task = link.runtime.spawn(host::write_read::<SessionsSettleRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let said = match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                Err(failure) => Some(format!("Not settled: {}", failure.message)),
                Ok(reply) => reply["left"]
                    .as_array()
                    .and_then(|left| left.iter().find(|l| l["why"] != "already settled"))
                    .map(|l| format!("Not settled: {}", l["why"].as_str().unwrap_or_default())),
            };
            if let Some(said) = said {
                _ = this.update(cx, |this, cx| this.refuse(said, cx));
            }
        })
        .detach();
    }

    pub fn restore(&mut self, key: &str, cx: &mut Context<Self>) {
        let ids = self.tree_keys(key);
        self.write::<SessionsRestoreRequest>(json!({ "threadIds": ids }), cx);
    }

    pub fn stop(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key) else { return };
        let session = t.session_id.clone();
        self.armed = None;
        self.write::<SessionsInterruptRequest>(json!({ "sessionId": session }), cx);
    }

    /// The hover act's Stop: the first press arms it, a second inside two seconds stops.
    fn press_stop(&mut self, key: &str, cx: &mut Context<Self>) {
        match &self.armed {
            Some((armed, at)) if armed == key && at.elapsed() < ARMED => self.stop(key, cx),
            _ => {
                self.armed = Some((key.to_owned(), Instant::now()));
                cx.notify();
            }
        }
    }

    /// Renames a thread through its agent; an outcome other than renamed is said over the list with the host's words.
    pub fn rename(&mut self, key: &str, title: &str, cx: &mut Context<Self>) {
        let Some(session) = self.thread(key).map(|t| t.session_id.clone()) else { return };
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let params = json!({ "sessionId": session, "title": title });
        let task = link.runtime.spawn(host::write_read::<SessionsRenameRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let said = match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                Err(failure) => Some(failure.message),
                Ok(reply) if reply["outcome"] == "renamed" => None,
                Ok(reply) => Some(reply["error"].as_str().or(reply["outcome"].as_str()).unwrap_or("refused").to_owned()),
            };
            if let Some(said) = said {
                _ = this.update(cx, |this, cx| this.refuse(format!("Not renamed: {said}"), cx));
            }
        })
        .detach();
    }

    pub fn delete(&mut self, key: &str, cx: &mut Context<Self>) {
        self.delete_with(key, Processes::Ask, cx);
    }

    pub fn delete_with(&mut self, key: &str, processes: Processes, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key) else { return };
        if self.in_folder(t) {
            self.write::<SessionsDeleteRequest>(json!({ "threadId": key, "processes": processes.word() }), cx);
        } else {
            self.write::<SessionsForgetRequest>(json!({ "threadId": key }), cx);
        }
    }

    pub fn mark_threads(&mut self, ids: Vec<String>, marks: Value, cx: &mut Context<Self>) {
        self.write::<SessionsMarkRequest>(json!({ "threadIds": ids, "marks": marks }), cx);
    }

    fn fold_tree(&mut self, key: &str, cx: &mut Context<Self>) {
        let folded = self.thread(key).is_some_and(|t| t.folded_at.is_some());
        self.mark_threads(vec![key.to_owned()], json!({ "folded": !folded }), cx);
    }

    fn toggle_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if !self.folds.sections.remove(&section) {
            self.folds.sections.insert(section);
        }
        if section == Section::Settled {
            self.folds.settled_shown = model::SETTLED_PAGE;
        }
        self.rebuild(cx);
    }

    fn toggle_finished(&mut self, lead: &str, cx: &mut Context<Self>) {
        if self.folds.finished_open.remove(lead).is_none() {
            self.folds.finished_open.insert(lead.to_owned(), model::FINISHED_PAGE);
        }
        self.rebuild(cx);
    }

    fn more(&mut self, lead: Option<&str>, cx: &mut Context<Self>) {
        match lead {
            None => self.folds.settled_shown += model::SETTLED_PAGE,
            Some(lead) => *self.folds.finished_open.entry(lead.to_owned()).or_insert(model::FINISHED_PAGE) += model::FINISHED_PAGE,
        }
        self.rebuild(cx);
    }

    fn toggle_group(&mut self, attempt: &str, cx: &mut Context<Self>) {
        if !self.folds.groups_shut.remove(attempt) {
            self.folds.groups_shut.insert(attempt.to_owned());
        }
        self.rebuild(cx);
    }

    /// Moves a root up, down or to the top of its section, as the menu's moves and a drop do.
    pub fn move_root(&mut self, key: &str, to: model::Move, cx: &mut Context<Self>) {
        let marks = self.view().move_root(key, to);
        for m in marks.unwrap_or_default() {
            self.mark_threads(m.thread_ids, m.marks, cx);
        }
    }

    /// A dragged root dropped on a section's head, or on a root, before which it then stands in that root's section.
    fn drop_on(&mut self, dragged: &str, target: &DropTarget, cx: &mut Context<Self>) {
        if *target == DropTarget::Head(Section::Settled) {
            if self.tree_works(dragged) {
                return self.refuse("A thread in it is still working".to_owned(), cx);
            }
            return self.settle(dragged, cx);
        }
        let view = self.view();
        let marks = match target {
            DropTarget::Head(section) => view.drop_marks(dragged, *section).map(|m| vec![m]),
            DropTarget::Root(target) => view.root_place(target).and_then(|place| {
                let from = view.root_place(dragged);
                let at = match from {
                    Some(f) if f.section == place.section && f.index < place.index => place.index - 1,
                    _ => place.index,
                };
                view.move_root(dragged, model::Move::To { section: place.section, at })
            }),
        };
        for m in marks.unwrap_or_default() {
            self.mark_threads(m.thread_ids, m.marks, cx);
        }
    }

    pub fn dismiss_cloud_card(&mut self, cx: &mut Context<Self>) {
        self.write::<PreferencesSetRequest>(json!({ "patch": { "cloudCardDismissed": true } }), cx);
    }

    /// Opens the name box on a tile, as the menu's Rename and a double-click on the title do.
    pub fn open_rename(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(title) = self.thread(key).map(|t| t.title.clone()) else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
        let events = cx.subscribe_in(&input, window, |this, input, event: &InputEvent, _, cx| match event {
            InputEvent::PressEnter { .. } => {
                let title = input.read(cx).value().trim().to_owned();
                let Some(r) = this.renaming.take_if(|_| !title.is_empty()) else { return };
                this.rename(&r.key, &title, cx);
            }
            InputEvent::Blur => {
                this.renaming = None;
                cx.notify();
            }
            _ => {}
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        self.renaming = Some(Renaming { key: key.to_owned(), input, _events: events });
        cx.notify();
    }

    /// Asks before a delete, as `DeleteThreadDialog.tsx` does.
    fn confirm_delete(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key) else { return };
        let title = SharedString::from(format!("Delete {}?", t.title));
        let (key, me) = (key.to_owned(), cx.weak_entity());
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (key, me) = (key.clone(), me.clone());
            alert
                .title(title.clone())
                // threadDeleteQuestion in packages/protocol/src/projects.ts, for a thread with no worktree of its own.
                .description("Its turns and checkpoints leave this computer; the folder it worked in is left as it is.")
                .ok_text("Delete")
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    _ = me.update(cx, |this, cx| this.delete(&key, cx));
                    true
                })
        });
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(drive) = &mut self.drive {
            if !drive.done {
                if self.list.is_scrolled_to_end() == Some(true) {
                    drive.done = true;
                } else {
                    let target = px(drive.px_per_s * drive.start.elapsed().as_secs_f32());
                    let at = -self.list.scroll_px_offset_for_scrollbar().y;
                    self.list.scroll_by(target - at);
                    window.request_animation_frame();
                }
            }
        }
        let frames = self.drive.as_ref().filter(|d| !d.done).map(|d| d.frames.clone());
        let scope = cx.palette().sidebar_scope();
        let column = theme::within(scope.clone(), cx, |cx| self.column(frames, cx));
        scoped(scope, column)
    }
}

impl Sidebar {
    /// Everything the sidebar draws, built with the sidebar's palette active.
    fn column(&mut self, frames: Option<Rc<RefCell<Vec<Instant>>>>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
        let p = cx.palette().clone();
        let me = cx.weak_entity();
        let ask = |asked: Asked| {
            let me = me.clone();
            Box::new(move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                _ = me.update(cx, |_, cx| cx.emit(asked));
            }) as Box<dyn Fn(&gpui_kit::ClickEvent, &mut Window, &mut App)>
        };
        let picked = self.prefs.project.as_ref().and_then(|id| self.around.projects.iter().find(|(p, _)| p == id).cloned());
        let projects = self.around.projects.clone();
        let rows = v_flex()
            .flex_none()
            .px(chrome::INSET)
            .pb(px(12.))
            .child(chrome::top_row("new-thread", Glyph::SquarePen, "New thread", false, cx).on_click(ask(Asked::NewThread)))
            .child(chrome::top_row("search", Glyph::Search, "Search", false, cx).on_click(ask(Asked::Search)))
            .child(tile::project_switcher(picked, projects, self.switcher_open, me.clone(), cx))
            .child(chrome::top_row("computers", Glyph::Monitor, "All computers", true, cx));
        let body = list(self.list.clone(), cx.processor(|this, ix: usize, window, cx| this.render_line(ix, window, cx)))
            .flex_1()
            .min_h_0()
            .w_full();
        let foot = v_flex()
            .flex_none()
            .p(px(8.))
            .gap(px(8.))
            .when(self.offers_cloud(), |el| {
                let me2 = me.clone();
                el.child(chrome::cloud_card(
                    Box::new(move |_, _, cx| _ = me2.update(cx, |this, cx| this.dismiss_cloud_card(cx))),
                    ask(Asked::AddCloud),
                    cx,
                ))
            })
            .child(chrome::corner(ask(Asked::Settings), ask(Asked::Usage), ask(Asked::Activity)));
        let note = self.refusal.clone().or_else(|| self.note.clone().filter(|_| self.lines.is_empty() || self.link.is_some()));
        // The shell draws the column: its width, its surface (glass on macOS) and the frame row with the trigger.
        let column = v_flex()
            .id("sidebar")
            .debug_selector(|| "sidebar".into())
            .size_full()
            .text_color(p.sidebar_foreground)
            .child(rows)
            .children(note.map(|note| {
                div().flex_none().px(px(16.)).pb(px(4.)).text_size(px(12.)).line_height(px(16.)).text_color(p.sidebar_prose).child(note)
            }))
            .child(body)
            .child(foot)
            .children(frames.map(|frames| {
                // A deferred callback runs when the update that paints ends, right after GPUI presents the frame.
                gpui_kit::canvas(
                    |_, _, _| {},
                    move |_, _, _, cx| {
                        let frames = frames.clone();
                        cx.defer(move |_| frames.borrow_mut().push(Instant::now()));
                    },
                )
                .absolute()
                .size_0()
            }));
        column
    }
}

impl Sidebar {
    fn render_line(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(line) = self.lines.get(ix).cloned() else { return div().into_any_element() };
        tile::line(self, ix, &line, window, cx)
    }
}
