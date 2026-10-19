// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar as today's app draws it (`apps/web/src/sidebar/`): the header, the four rows over the list, the list
//! of thread tiles in its sections with their trees and Settled, and the foot with the cloud card and the corner. It
//! reads and writes through `wsp-host-client` and follows the host's pushes; the list is virtual, so only the rows
//! on screen are built, whatever the number of threads.

pub mod alone;
mod card;
pub mod chrome;
mod dialogs;
mod foot;
pub mod host;
mod keys;
pub mod looks;
pub(crate) mod markdown;
pub(crate) mod menu;
pub mod model;
pub mod rows;
mod switcher;
pub(crate) mod tile;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::{
    div, list, prelude::*, px, App, Bounds, Context, Entity, EventEmitter, KeyDownEvent, ListAlignment, ListOffset, ListState, Pixels,
    Subscription, Task, Window,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{
    HarnessesListRequest, PreferencesSetRequest, ProjectsIconsRequest, SessionView, SessionsDeleteRequest, SessionsForgetRequest,
    SessionsInterruptRequest, SessionsMarkRequest, SessionsRenameRequest, SessionsRestoreRequest, SessionsSettleRequest,
    WorkspacesBringBackRequest, WorkspacesRenameRequest, WorkspacesWakeRequest,
};
use wsp_host_client::{lock, HostClient, Op};

use crate::screens::app_shell::title::thread_title;
use crate::ui::catalog;
use crate::ui::context_menu::ContextMenu;
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
            title: thread_title(view.harness_title.as_deref(), view.prompt.as_deref()).unwrap_or_else(|| "New thread".to_owned()),
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

/// What the sidebar asks of the window around it: screens it does not own, each named by what it opens on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asked {
    NewThread,
    Search,
    Settings,
    Usage,
    Activity,
    AddCloud,
    AddProject,
    AddComputer,
    ProjectSettings(String),
    ComputerSettings(String),
    /// A computer's setup, or a cloud's, by its place id.
    Setup(String),
    NewThreadIn(String),
    OpenTerminal(String),
    OpenBrowser(String),
    ExportProject(String),
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
    /// The host's own page, `http://<address>:<port>/`, which a thread's link names.
    pub page: Option<String>,
}

/// The page the host serving `state` answers on, where its lock says it is dialled, as the desktop app loads it.
pub fn host_page(state: &std::path::Path) -> Option<String> {
    let held = lock::serving(state)?;
    Some(format!("http://{}/", lock::authority(&lock::dial_address(&held), held.port)))
}

/// The reader's offset from UTC in ms, for times a person reads.
pub(crate) fn utc_offset_ms() -> i64 {
    i64::from(chrono::Local::now().offset().local_minus_utc()) * 1000
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
    /// The name is on its way: the box stays as it is and takes no second Enter.
    saving: bool,
    _events: Subscription,
}

/// The preferences read before the host has said any.
pub(crate) static NO_PREFERENCES: Value = Value::Null;

/// What the foot, the switchers, the card and the extra tiles read besides the threads.
#[derive(Default)]
pub(crate) struct Around {
    pub places: Vec<Value>,
    /// Shared, so a frame that reads them clones a pointer.
    pub preferences: Option<Rc<Value>>,
    /// Shared, so the notices tell a new read from the one they last saw by its pointer, as the web's store does.
    pub job: Option<Rc<Value>>,
    /// How many `job.needs-you` pushes it took, so the notices say each one as the web's rule for the push does.
    pub needs_pushed: u64,
    /// Workspace id to its kind: a folder's thread is deleted, a cloud's forgotten.
    pub kinds: HashMap<String, String>,
    /// Every workspace record, by id.
    pub workspaces: HashMap<String, Value>,
    pub projects: Vec<(String, String)>,
    /// Each project's folder, by project id.
    pub project_paths: HashMap<String, String>,
    /// The workspaces whose checkout this connection asked for, each with the phase it was asked in.
    pub checkouts_asked: HashMap<String, String>,
    /// Each project's computer, by project id.
    pub project_computers: HashMap<String, String>,
    /// The projects that are git checkouts, by id.
    pub project_git: HashSet<String>,
    /// Each workspace's live status, by id: its checkout and pull request.
    pub statuses: HashMap<String, Value>,
    pub release: Option<Value>,
    pub forwards: Vec<Value>,
    pub creations: Vec<Creation>,
    pub launches: Vec<Launch>,
    pub images: looks::Images,
    /// Each workspace's agents: the harness, the terminal command it resumes a session with, its models' labels.
    pub catalogs: HashMap<String, Vec<Catalog>>,
    /// Each workspace's agents whole, as `harnesses.list` answered, which the thread view's flags and the header's
    /// labels read: the app's one read of them, read again on every whole read.
    pub listed: HashMap<String, Rc<Vec<Value>>>,
    catalog_waiters: HashMap<String, Vec<CatalogWaiter>>,
}

/// What waits on a workspace's agents being read, told whether they came.
type CatalogWaiter = Box<dyn FnOnce(bool, &mut App)>;

/// One agent of a workspace's catalog, as far as the sidebar reads it.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub harness: String,
    pub terminal_resume: Option<String>,
    pub models: Vec<(String, String)>,
    /// `keepsRename`: a row the runtime's table stood in for is no answer, so the box opens and the runtime says.
    pub keeps_rename: bool,
}

/// A workspace being made, as `workspace.creating` tells it step by step until `workspace.created`.
#[derive(Clone, Debug)]
pub struct Creation {
    pub key: String,
    pub workspace_id: Option<String>,
    pub name: String,
    pub project: Option<String>,
    pub lines: Vec<Value>,
    pub failed: bool,
}

/// A send in flight that the host has written no row for yet, as the composer starts it.
#[derive(Clone, Debug, PartialEq)]
pub struct Launch {
    pub workspace_id: String,
    pub request_id: String,
    pub title: String,
    pub harness: String,
    pub started: Instant,
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
    /// Every row the list draws, and what each stands for, read off what it was drawn from.
    drawn_rows: Vec<rows::Drawn>,
    /// The drawn rows that took the gap over the section they open, each with the line it took it from.
    gap_moved: RefCell<Vec<(usize, usize)>>,
    line_ids: Vec<String>,
    /// The foot's folded sections, by the web's ids: setting-up, forwards.
    foot_folds: HashSet<String>,
    folds: Folds,
    prefs: Prefs,
    around: Around,
    list: ListState,
    link: Option<Link>,
    following: Option<Following>,
    renaming: Option<Renaming>,
    armed: Option<(String, Instant)>,
    pub(crate) switcher: Option<switcher::OpenSwitcher>,
    /// Where each switcher's head was drawn, so a press on it is not read as a press outside its popup.
    switcher_heads: Rc<Cell<[Bounds<Pixels>; 2]>>,
    pub(crate) menu: ContextMenu,
    /// The tile the pointer rests on, the one whose card stands, and the wait before it opens.
    hovered: Option<String>,
    card: Option<String>,
    /// The sidebar's keyboard focus, the row the keys walk from, and whether a key put it there.
    focus: gpui_kit::FocusHandle,
    key_row: Option<usize>,
    keyboard: bool,
    /// A popup or a dialog closed where no window was at hand: the next draw hands the keys back to the list.
    keys_back: bool,
    _card_wait: Option<Task<()>>,
    /// The page's palette, for what the sidebar draws that the web portals out of it.
    page: crate::ui::Palette,
    pub(crate) dialog: Option<dialogs::Dialog>,
    /// The computer the list is narrowed to (`wsp:sidebar-computer`), none for all.
    computer: Option<String>,
    drive: Option<Drive>,
    /// The clock the list was last read at, in ms since the epoch.
    now: f64,
    revision: u64,
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
            drawn_rows: Vec::new(),
            gap_moved: RefCell::new(Vec::new()),
            foot_folds: HashSet::new(),
            folds: Folds::default(),
            prefs: Prefs::default(),
            around: Around::default(),
            list,
            link: None,
            following: None,
            renaming: None,
            armed: None,
            switcher: None,
            switcher_heads: Rc::default(),
            menu: ContextMenu::default(),
            hovered: None,
            card: None,
            focus: cx.focus_handle(),
            key_row: None,
            keyboard: false,
            keys_back: false,
            _card_wait: None,
            page: cx.palette().clone(),
            dialog: None,
            computer: None,
            drive: None,
            now: now_ms(),
            revision: 0,
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

    fn hand_keys_back(&mut self, cx: &mut Context<Self>) {
        self.keys_back = true;
        cx.notify();
    }

    /// The host refused a write: said over the list until the next write.
    fn refuse(&mut self, said: String, cx: &mut Context<Self>) {
        self.refusal = Some(said);
        cx.notify();
    }

    /// A thread's title as the host folds it, for the header that names the open thread.
    pub fn title_of(&self, thread_id: &str) -> Option<String> {
        self.thread(thread_id)
            .map(|t| t.title.clone())
            .or_else(|| self.items.iter().find(|i| i.thread_id == thread_id).map(|i| i.title.clone()))
    }

    pub fn items(&self) -> &[ThreadItem] {
        &self.items
    }

    /// What the sidebar reads besides the threads, for the palette's rows.
    pub(crate) fn around(&self) -> &Around {
        &self.around
    }

    /// The person's picks the list is narrowed by.
    pub(crate) fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    /// The rows the list draws, top to bottom.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Every row the list draws, the threads' and the rest.
    pub fn drawn(&self) -> &[rows::Drawn] {
        &self.drawn_rows
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

    /// A workspace's record and its live status as this connection last read them.
    pub fn workspace(&self, id: &str) -> (Option<&Value>, Option<&Value>) {
        (self.around.workspaces.get(id), self.around.statuses.get(id))
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
                match (at, row.and_then(Row::owned)) {
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
            Change::Job(job) => self.around.job = job.map(Rc::new),
            Change::JobNeed(need) => {
                let mut job = self.around.job.as_deref().cloned().unwrap_or_else(|| json!({}));
                job["needsYou"] = need;
                self.around.job = Some(Rc::new(job));
                self.around.needs_pushed += 1;
            }
            Change::Workspaces(w) => self.take_workspaces(&w),
            Change::WorkspaceDeleted(id) => {
                self.rows.retain(|r| r.workspace_id != id);
                self.around.workspaces.remove(&id);
                self.around.kinds.remove(&id);
                self.around.statuses.remove(&id);
                self.around.catalogs.remove(&id);
                self.around.listed.remove(&id);
                self.around.checkouts_asked.remove(&id);
                self.around.forwards.retain(|f| f.get("workspaceId").and_then(Value::as_str) != Some(id.as_str()));
                self.around.creations.retain(|c| c.workspace_id.as_deref() != Some(id.as_str()));
            }
            Change::Projects(p) => self.take_projects(&p),
            Change::Status(status) => {
                if let Some(id) = status.get("id").and_then(Value::as_str) {
                    self.around.statuses.insert(id.to_owned(), status);
                }
            }
            Change::Creating(frame) => self.creating(frame),
            Change::Created(frame) => {
                let id = frame.pointer("/workspace/id").and_then(Value::as_str).unwrap_or_default().to_owned();
                let name = frame.pointer("/workspace/name").and_then(Value::as_str).unwrap_or_default().to_owned();
                self.around
                    .creations
                    .retain(|c| c.workspace_id.as_deref() != Some(id.as_str()) && !(c.workspace_id.is_none() && c.name == name));
            }
            Change::Release(release) => self.around.release = Some(release),
            Change::Forward(forward) => {
                let key = |f: &Value| (f.get("workspaceId").cloned(), f.get("port").cloned());
                self.around.forwards.retain(|f| key(f) != key(&forward));
                self.around.forwards.push(forward);
            }
            Change::ForwardClosed { workspace_id, port } => self.around.forwards.retain(|f| {
                f.get("workspaceId").and_then(Value::as_str) != Some(workspace_id.as_str())
                    || f.get("port").and_then(Value::as_u64) != Some(port)
            }),
            Change::Launched(frame) => {
                // A start or a hold names the send's request; an end with none ends every send on its workspace.
                let request = frame.get("requestId").and_then(Value::as_str);
                let workspace = frame.get("workspaceId").and_then(Value::as_str);
                let end = frame.get("type").and_then(Value::as_str) == Some("session.end");
                self.around.launches.retain(|l| match request {
                    Some(r) => l.request_id != r,
                    None => !(end && workspace == Some(l.workspace_id.as_str())),
                });
            }
            Change::Lost(why) => {
                self.note = Some(why);
                cx.notify();
                return;
            }
        }
        self.note = None;
        self.revision += 1;
        self.refold(cx);
    }

    /// Moves with every change the host pushed: a reader of the rows or the preferences reads again only when it moved.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Every session row the host listed, on every computer whatever the list is narrowed to.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// How long a read thread sits quiet before it settles (`settleAfter`); None is never.
    pub fn settle_ms(&self) -> Option<f64> {
        self.prefs.settle_ms
    }

    /// The host's preferences, once read.
    pub fn preferences(&self) -> &Value {
        self.around.preferences.as_deref().unwrap_or(&NO_PREFERENCES)
    }

    /// Shuts or opens a lead's Threads section under its transcript, kept in the preferences for every window
    /// (`threadsShut`), apart from the list's own fold of the same tree.
    pub fn shut_threads(&mut self, lead: &str, shut: bool, cx: &mut Context<Self>) {
        let entry = if shut { json!(true) } else { Value::Null };
        self.write::<PreferencesSetRequest>(json!({ "patch": { "threadsShut": { lead: entry } } }), cx);
    }

    /// Folds the rows into threads again, on the picked computer alone where one is picked.
    fn refold(&mut self, cx: &mut Context<Self>) {
        self.threads = match &self.computer {
            None => model::fold_threads(&self.rows),
            Some(on) => {
                let rows: Vec<Row> =
                    self.rows.iter().filter(|r| self.place_of(&r.workspace_id).as_deref() == Some(on.as_str())).cloned().collect();
                model::fold_threads(&rows)
            }
        };
        // What the person opened on a thread or a group goes with it, as the web's row state goes when it is not drawn.
        let keys: HashSet<&str> = self.threads.iter().map(|t| t.key.as_str()).collect();
        self.folds.finished_open.retain(|lead, _| keys.contains(lead.as_str()));
        let attempts: HashSet<&str> = self.threads.iter().filter_map(|t| t.attempt.as_deref()).collect();
        self.folds.groups_shut.retain(|attempt| attempts.contains(attempt.as_str()));
        self.fetch_icons(cx);
        self.fetch_catalogs(cx);
        self.rebuild(cx);
    }

    /// Reads the agents' catalog of each workspace a thread runs on that has none read yet: what its menu offers for
    /// going on in a terminal.
    fn fetch_catalogs(&mut self, cx: &mut Context<Self>) {
        let mut wanted: Vec<String> = self.threads.iter().map(|t| t.workspace_id.clone()).collect();
        wanted.sort();
        wanted.dedup();
        for workspace in wanted {
            self.fetch_catalog(&workspace, cx);
        }
    }

    /// Each workspace's agents whole, as the host listed them, once read.
    pub fn listed_catalogs(&self, workspace: &str) -> Option<Rc<Vec<Value>>> {
        self.around.listed.get(workspace).cloned()
    }

    /// Reads `workspace`'s agents unless they are read or asked; `landed` hears whether they came, after the update
    /// that asks.
    pub fn want_catalogs(&mut self, workspace: &str, landed: impl FnOnce(bool, &mut App) + 'static, cx: &mut Context<Self>) {
        if self.around.listed.contains_key(workspace) {
            cx.defer(move |cx| landed(true, cx));
            return;
        }
        if self.link.is_none() {
            cx.defer(move |cx| landed(false, cx));
            return;
        }
        self.around.catalog_waiters.entry(workspace.to_owned()).or_default().push(Box::new(landed));
        self.fetch_catalog(workspace, cx);
    }

    fn fetch_catalog(&mut self, workspace: &str, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        if self.around.catalogs.contains_key(workspace) {
            return;
        }
        let workspace = workspace.to_owned();
        self.around.catalogs.insert(workspace.clone(), Vec::new());
        let task = link.runtime.spawn(host::ask::<HarnessesListRequest>(link.host.clone(), json!({ "workspaceId": workspace })));
        cx.spawn(async move |this, cx| {
            let listed: Option<Vec<Value>> = match task.await {
                Ok(Ok(listed)) => Some(listed.harnesses.iter().filter_map(|h| serde_json::to_value(h).ok()).collect()),
                _ => None,
            };
            let catalog: Vec<Catalog> = listed
                .iter()
                .flatten()
                .filter_map(|h| {
                    Some(Catalog {
                        harness: h.get("harness")?.as_str()?.to_owned(),
                        terminal_resume: h.get("terminalResume").and_then(Value::as_str).map(str::to_owned),
                        models: h
                            .get("models")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|m| Some((m.get("value")?.as_str()?.to_owned(), m.get("label")?.as_str()?.to_owned())))
                            .collect(),
                        keeps_rename: h.get("source").and_then(Value::as_str) == Some("table")
                            || h.get("renames").and_then(Value::as_bool) != Some(false),
                    })
                })
                .collect();
            // A failed ask leaves the workspace with no catalog until the next full read asks again; a workspace deleted
            // or a connection read again while it was asked has no place for it, and its waiters wait on that read.
            let told = this.update(cx, |this, cx| {
                let Some(slot) = this.around.catalogs.get_mut(&workspace) else { return Vec::new() };
                let landed = listed.is_some();
                if let Some(listed) = listed {
                    *slot = catalog;
                    this.around.listed.insert(workspace.clone(), Rc::new(listed));
                }
                cx.notify();
                let waiters = this.around.catalog_waiters.remove(&workspace).unwrap_or_default();
                waiters.into_iter().map(|w| (w, landed)).collect::<Vec<_>>()
            });
            if let Ok(told) = told {
                cx.update(|cx| told.into_iter().for_each(|(waiter, landed)| waiter(landed, cx)));
            }
        })
        .detach();
    }

    /// Asks the host for the project images the preferences name that are not loaded yet.
    fn fetch_icons(&mut self, cx: &mut Context<Self>) {
        let Some(prefs) = self.around.preferences.clone() else { return };
        self.around.images.keep_named(&prefs);
        let wanted = self.around.images.wanted(&prefs);
        let Some(link) = self.link.clone().filter(|_| !wanted.is_empty()) else { return };
        self.around.images.asking(&wanted);
        let task = link.runtime.spawn(host::ask::<ProjectsIconsRequest>(link.host.clone(), json!({ "hashes": wanted })));
        cx.spawn(async move |this, cx| {
            let reply = task.await.ok().and_then(Result::ok);
            _ = this.update(cx, |this, cx| {
                let prefs = this.around.preferences.clone().unwrap_or_default();
                match reply {
                    Some(reply) => this.around.images.take(&serde_json::to_value(&reply.icons).unwrap_or_default(), &prefs),
                    None => this.around.images.failed(&wanted),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn take(&mut self, s: Snapshot) {
        self.rows = s.sessions.into_iter().filter_map(Row::owned).collect();
        self.around.places = s.places;
        self.around.job = s.job.map(Rc::new);
        if !s.statuses.is_empty() {
            self.around.statuses = s.statuses.into_iter().filter_map(|st| Some((st.get("id")?.as_str()?.to_owned(), st))).collect();
        }
        self.around.release = s.release;
        self.around.forwards = s.forwards;
        // A full read answers every send in flight (useStore refresh).
        self.around.launches.clear();
        self.around.images.reconnected();
        self.around.catalogs.clear();
        self.around.listed.clear();
        self.around.checkouts_asked.clear();
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
        self.around.preferences = Some(Rc::new(p));
    }

    fn take_workspaces(&mut self, workspaces: &[Value]) {
        self.around.kinds =
            workspaces.iter().filter_map(|w| Some((w.get("id")?.as_str()?.to_owned(), w.get("kind")?.as_str()?.to_owned()))).collect();
        self.around.workspaces = workspaces.iter().filter_map(|w| Some((w.get("id")?.as_str()?.to_owned(), w.clone()))).collect();
    }

    /// A step of a workspace being made: the row it belongs to by id, else by name while the id is unknown, else a new
    /// row for a creation another window started.
    fn creating(&mut self, frame: Value) {
        let id = frame.get("workspaceId").and_then(Value::as_str).map(str::to_owned);
        let name = frame.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let at = self
            .around
            .creations
            .iter()
            .position(|c| c.workspace_id.is_some() && c.workspace_id == id)
            .or_else(|| self.around.creations.iter().position(|c| c.workspace_id.is_none() && c.name == name));
        let at = at.unwrap_or_else(|| {
            self.around.creations.push(Creation {
                key: format!("creating:{}", id.clone().unwrap_or_default()),
                workspace_id: None,
                name: name.clone(),
                project: None,
                lines: Vec::new(),
                failed: false,
            });
            self.around.creations.len() - 1
        });
        let creation = &mut self.around.creations[at];
        creation.workspace_id = creation.workspace_id.take().or(id);
        creation.failed |= frame.get("stage").and_then(Value::as_str) == Some("failed");
        creation.lines.push(frame);
    }

    /// A send the composer started that the host has not answered with a row yet, drawn as a tile until it does.
    pub fn launching(&mut self, workspace_id: &str, request_id: &str, title: &str, harness: &str, cx: &mut Context<Self>) {
        self.around.launches.retain(|l| l.workspace_id != workspace_id);
        self.around.launches.push(Launch {
            workspace_id: workspace_id.to_owned(),
            request_id: request_id.to_owned(),
            title: title.to_owned(),
            harness: harness.to_owned(),
            started: Instant::now(),
        });
        self.rebuild(cx);
    }

    /// The id of the computer a workspace stands on (`place-state.ts` workspacePlaceId).
    pub(crate) fn place_of(&self, workspace_id: &str) -> Option<String> {
        menu::place_of(self.around.workspaces.get(workspace_id)?, &self.around.places)
    }

    /// Whether a project is a git checkout (`ProjectView.git`), which a fork's new branch needs.
    pub(crate) fn project_git(&self, project_id: &str) -> bool {
        self.around.project_git.contains(project_id)
    }

    /// The places list's row for `id`.
    pub(crate) fn place(&self, id: &str) -> Option<&Value> {
        self.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id))
    }

    fn take_projects(&mut self, projects: &[Value]) {
        self.around.project_paths =
            projects.iter().filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("path")?.as_str()?.to_owned()))).collect();
        self.around.project_computers =
            projects.iter().filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("computer")?.as_str()?.to_owned()))).collect();
        self.around.projects =
            projects.iter().filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("name")?.as_str()?.to_owned()))).collect();
        self.around.project_git = projects
            .iter()
            .filter(|p| p.get("git").is_some_and(|g| !g.is_null()))
            .filter_map(|p| Some(p.get("id")?.as_str()?.to_owned()))
            .collect();
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
        let old_lines = std::mem::replace(&mut self.lines, lines);
        self.working = self.working_roots();
        let drawn = self.draw_rows();
        let ids: Vec<String> = drawn.iter().map(|d| self.drawn_identity(d)).collect();
        let shape_changed = drawn.len() != self.drawn_rows.len()
            || ids != self.line_ids
            || self
                .lines
                .iter()
                .zip(&old_lines)
                .any(|(a, b)| std::mem::discriminant(&a.item) != std::mem::discriminant(&b.item) || a.gap != b.gap);
        if shape_changed {
            // The row at the top stays at the top wherever it moved to, as the browser's scroll anchoring keeps it.
            let top = self.list.logical_scroll_top();
            let at = self.line_ids.get(top.item_ix).and_then(|a| ids.iter().position(|id| id == a)).unwrap_or(top.item_ix);
            self.list.reset(drawn.len());
            self.list.scroll_to(ListOffset { item_ix: at.min(drawn.len().saturating_sub(1)), offset_in_item: top.offset_in_item });
        }
        // The row the keys stand on stays on what it stood for, wherever that moved to.
        self.key_row = self.key_row.and_then(|r| self.line_ids.get(r)).and_then(|was| ids.iter().position(|id| id == was));
        self.drawn_rows = drawn;
        self.line_ids = ids;
        cx.notify();
    }

    /// What a row stands for, kept across a rebuild: a thread's key, a section, a fold, or its place.
    pub(crate) fn identity(&self, line: &Line) -> String {
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

    /// Pushes a workspace's branch and opens its pull request; what it brought back is a done toast (`broughtBackLine`),
    /// a refusal is said over the list.
    pub fn bring_back(&mut self, workspace_id: &str, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let params = json!({ "workspaceId": workspace_id });
        let place = self.around.workspaces.get(workspace_id).and_then(|w| crate::screens::notices::host::copy_name(w, &self.around.places));
        let task = link.runtime.spawn(host::write_read::<WorkspacesBringBackRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
            Ok(back) => {
                let notice = crate::ui::Notice::done(brought_back_line(&back));
                cx.update(|cx| crate::ui::notify(cx, if let Some(place) = place { notice.place(place) } else { notice }))
            }
            Err(failure) => _ = this.update(cx, |this, cx| this.refuse(failure.message, cx)),
        })
        .detach();
    }

    pub fn settle(&mut self, key: &str, cx: &mut Context<Self>) {
        self.settle_with(key, Processes::Ask, cx);
    }

    /// Settles a thread's tree; a thread the host leaves standing is said in the note, with the host's reason.
    pub fn settle_with(&mut self, key: &str, processes: Processes, cx: &mut Context<Self>) {
        // The host settles a thread with everything under it, so the web names the tree's top alone.
        self.settle_ids(vec![key.to_owned()], processes, cx);
    }

    /// `settleOpenThread`: settles the root tree of `open`, else of the workspace's top thread, as the tile's Settle
    /// does; nothing while a thread of the tree works or the tree is settled already.
    pub fn settle_open(&mut self, workspace_id: &str, open: Option<&str>, cx: &mut Context<Self>) {
        if let Some(root) = self.open_root(workspace_id, open) {
            self.settle(&root, cx);
        }
    }

    /// The root [`Sidebar::settle_open`] settles, by its key; None where it settles nothing.
    pub fn open_root(&self, workspace_id: &str, open: Option<&str>) -> Option<String> {
        fn holds(node: &model::Tree, all: &[Thread], key: &str) -> bool {
            node.thread.is_some_and(|i| all[i].key == key) || node.children.iter().any(|c| holds(c, all, key))
        }
        self.around.workspaces.get(workspace_id)?;
        let top = || {
            let walk = crate::screens::palette::switcher::walk::in_workspace(&self.threads, &self.around.workspaces, workspace_id);
            walk.into_iter().next().map(|t| t.thread_id)
        };
        let open = open.filter(|key| self.thread(key).is_some()).map(str::to_owned).or_else(top)?;
        let prefs = Prefs { project: None, ..self.prefs.clone() };
        let view = model::sidebar(&self.threads, &prefs, self.now, Some(&open));
        let mut live = view.pinned.iter().chain(view.needs_you.iter().filter(|n| n.inbox.is_none())).chain(&view.threads);
        let root = live.find(|n| holds(n, &self.threads, &open)).filter(|n| n.thread.is_some())?.key.clone();
        (!self.tree_works(&root)).then_some(root)
    }

    /// Settles threads; where the host leaves them for processes still running, asks as the web does.
    pub(crate) fn settle_ids(&mut self, ids: Vec<String>, processes: Processes, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let params = json!({ "threadIds": ids, "processes": processes.word() });
        let task = link.runtime.spawn(host::write_read::<SessionsSettleRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let said = match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                Err(failure) if failure.kind.as_deref() == Some("processes-left") => {
                    _ = this.update(cx, |this, cx| this.ask_settle(ids, failure.message, cx));
                    None
                }
                Err(failure) => Some(format!("Not settled: {}", failure.message)),
                Ok(reply) => {
                    let back = this.clone();
                    let restore = move |ids: Vec<String>, cx: &mut gpui_kit::App| _ = back.update(cx, |this, cx| this.restore_ids(ids, cx));
                    cx.update(|cx| crate::ui::notify(cx, crate::screens::notices::settled(&reply, restore)));
                    None
                }
            };
            if let Some(said) = said {
                _ = this.update(cx, |this, cx| this.refuse(said, cx));
            }
        })
        .detach();
    }

    pub fn restore(&mut self, key: &str, cx: &mut Context<Self>) {
        self.restore_ids(vec![key.to_owned()], cx);
    }

    pub(crate) fn restore_ids(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.write::<SessionsRestoreRequest>(json!({ "threadIds": ids }), cx);
    }

    /// One write by its op; a refusal is said over the list.
    pub(crate) fn write_op<O: Op + serde::de::DeserializeOwned + Send + 'static>(&mut self, params: Value, cx: &mut Context<Self>) {
        self.write::<O>(params, cx);
    }

    /// The workspaces of the other threads one send opened beside this one, which "Keep this one" deletes.
    pub(crate) fn attempt_others(&self, key: &str) -> Vec<String> {
        let Some(attempt) = self.thread(key).and_then(|t| t.attempt.clone()) else { return Vec::new() };
        self.threads
            .iter()
            .filter(|o| o.key != key && o.attempt.as_deref() == Some(attempt.as_str()))
            .map(|o| o.workspace_id.clone())
            .collect()
    }

    /// Every workspace a tree's threads run on (`treeWorkspaceIds`), which "Delete copies" deletes.
    pub(crate) fn tree_workspaces(&self, key: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in self.tree_keys(key) {
            if let Some(w) = self.thread(&k).map(|t| t.workspace_id.clone()).filter(|w| !out.contains(w)) {
                out.push(w);
            }
        }
        out
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
        let Some(task) = self.send_name(key, title) else { return };
        cx.spawn(async move |this, cx| {
            if let Some(said) = task.await.unwrap_or_else(|e| Some(e.to_string())) {
                _ = this.update(cx, |this, cx| this.refuse(said, cx));
            }
        })
        .detach();
    }

    /// Sends a thread's name, or a workspace's for a key `w:<workspace>`: none once the host takes it, else what to say.
    fn send_name(&mut self, key: &str, name: &str) -> Option<tokio::task::JoinHandle<Option<String>>> {
        let link = self.link.clone()?;
        let task = match key.strip_prefix("w:") {
            Some(id) => {
                let params = json!({ "workspaceId": id, "name": name });
                link.runtime.spawn(async move { host::write::<WorkspacesRenameRequest>(link.host, params).await.err().map(|f| f.message) })
            }
            None => {
                let t = self.thread(key)?;
                let params = json!({ "sessionId": t.session_id, "title": name });
                // The name is kept on the machine, so a napping one is woken first, as `renameThread` does.
                let napping = self.around.workspaces.get(&t.workspace_id).is_some_and(|w| w["phase"] != "running");
                let wake = napping.then(|| json!({ "workspaceId": t.workspace_id }));
                let (harness, title) = (t.harness.clone(), name.to_owned());
                link.runtime.spawn(async move {
                    let renamed = async {
                        if let Some(wake) = wake {
                            host::write::<WorkspacesWakeRequest>(link.host.clone(), wake).await?;
                        }
                        host::write_read::<SessionsRenameRequest>(link.host, params).await
                    };
                    match renamed.await {
                        Err(failure) => Some(format!("{title}: {}", failure.message)),
                        Ok(reply) if reply["outcome"] == "renamed" => None,
                        Ok(reply) => {
                            Some(rename_not_taken_line(&harness, reply["outcome"].as_str().unwrap_or_default(), reply["error"].as_str()))
                        }
                    }
                })
            }
        };
        self.refusal = None;
        Some(task)
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
                return self.refuse(menu::settle_refusal(true).unwrap_or_default(), cx);
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
    /// Opens the name box on a thread's tile, or on a workspace's own tile for a key `w:<workspace>`.
    pub fn open_rename(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let title = match key.strip_prefix("w:") {
            Some(id) => self.around.workspaces.get(id).and_then(|w| w.get("name")?.as_str()).map(str::to_owned),
            // A rename the menu refuses opens no box on a double-click either (`openerOf`).
            None => self.thread(key).filter(|t| menu::rename_refusal(self, t).is_none()).map(|t| t.title.clone()),
        };
        let Some(title) = title else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title.clone()));
        let events = cx.subscribe_in(&input, window, move |this, input, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => {
                let typed = input.read(cx).value().trim().to_owned();
                let Some(key) = this.renaming.as_ref().filter(|r| !r.saving).map(|r| r.key.clone()) else { return };
                // An empty name or the same one closes the box, as the web's does.
                if typed.is_empty() || typed == title {
                    return this.close_rename(window, cx);
                }
                let Some(task) = this.send_name(&key, &typed) else { return };
                if let Some(r) = this.renaming.as_mut() {
                    r.saving = true;
                }
                // The box closes once the host takes the name; a refusal leaves the name in it to try again (`sendName`).
                cx.spawn_in(window, async move |this, cx| {
                    let said = task.await.unwrap_or_else(|e| Some(e.to_string()));
                    _ = this.update_in(cx, |this, window, cx| {
                        let Some(r) = this.renaming.as_mut().filter(|r| r.key == key) else { return };
                        match said {
                            None => this.close_rename(window, cx),
                            Some(said) => {
                                r.saving = false;
                                this.refuse(said, cx);
                            }
                        }
                    });
                })
                .detach();
                cx.notify();
            }
            InputEvent::Blur if this.renaming.as_ref().is_some_and(|r| !r.saving) => {
                this.renaming = None;
                cx.notify();
            }
            _ => {}
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        self.renaming = Some(Renaming { key: key.to_owned(), input, saving: false, _events: events });
        cx.notify();
    }

    fn close_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// The name in the open rename box, none while no box is open.
    pub fn rename_box(&self, cx: &App) -> Option<String> {
        self.renaming.as_ref().map(|r| r.input.read(cx).value().to_string())
    }

    /// The name box, where Escape leaves the name as it was.
    pub(crate) fn rename_field(&self, input: &Entity<InputState>, cx: &mut Context<Self>) -> gpui_kit::Div {
        let p = cx.palette().clone();
        // `rounded-sm border border-input px-1`, the ring's edge while it has the keys.
        div()
            .flex()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .border_color(p.ring)
            .px(px(4.))
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" && this.renaming.as_ref().is_some_and(|r| !r.saving) {
                    this.close_rename(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(gpui_kit::component::input::Input::new(input).appearance(false).p_0().h(px(16.)).text_size(px(14.)).line_height(px(16.)))
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.around.images.drop_retired(window, cx);
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
        if std::mem::take(&mut self.keys_back) {
            window.focus(&self.focus, cx);
        }
        let frames = self.drive.as_ref().filter(|d| !d.done).map(|d| d.frames.clone());
        let page = cx.palette().clone();
        self.page = page.clone();
        let scope = page.sidebar_scope();
        let column = theme::within(scope.clone(), cx, |cx| self.column(frames, cx));
        // The menu and the dialogs stand outside the sidebar in the page's own palette, as the web portals them.
        let menu = self.menu.overlay();
        let dialog = self.dialog_overlay(&page, window, cx);
        div().size_full().child(scoped(scope, column)).children(menu).children(dialog)
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
                _ = me.update(cx, |_, cx| cx.emit(asked.clone()));
            }) as Box<dyn Fn(&gpui_kit::ClickEvent, &mut Window, &mut App)>
        };
        let page = self.page.clone();
        let projects = self.switcher_row(switcher::Noun::Projects, &page, cx);
        let computers = self.switcher_row(switcher::Noun::Computers, &page, cx);
        let rows = v_flex()
            .flex_none()
            .px(chrome::INSET)
            .pb(px(12.))
            .child(chrome::top_row("new-thread", Glyph::SquarePen, "New thread", false, cx).on_click(ask(Asked::NewThread)))
            .child(chrome::top_row("search", Glyph::Search, "Search", false, cx).on_click(ask(Asked::Search)))
            .child(projects)
            .child(computers);
        let body = list(self.list.clone(), cx.processor(|this, ix: usize, window, cx| this.render_line(ix, window, cx)))
            .flex_1()
            .min_h_0()
            .w_full();
        let setting_up = self.setting_up_section(cx);
        let update = self.update_card(cx);
        // The foot holds one card: the cloud's waits while the Setting up section or the update card stands.
        let cloud = self.offers_cloud() && setting_up.is_none() && update.is_none();
        let foot = v_flex()
            .flex_none()
            .p(px(8.))
            .gap(px(8.))
            .children(setting_up)
            .children(update)
            .when(cloud, |el| {
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
            .key_context("Sidebar")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui_kit::KeyDownEvent, window, cx| this.key_down(e, window, cx)))
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
        let row = self.render_drawn(ix, window, cx);
        let ring = self.ring(ix, &cx.palette().clone(), window);
        match ring {
            Some(ring) => div().relative().child(row).child(ring).into_any_element(),
            None => row,
        }
    }
}

/// `renameNotTakenLine`: why the agent kept no name, in the person's words.
pub(crate) fn rename_not_taken_line(harness: &str, outcome: &str, error: Option<&str>) -> String {
    match outcome {
        "unsupported" => format!("Rename in {} is not kept", catalog::name(harness)),
        "no-session" => format!("{} on the task has no session for this thread yet", catalog::name(harness)),
        "not-found" => "The runtime has no such thread any more".to_owned(),
        _ => error.unwrap_or("The task said nothing about the write").to_owned(),
    }
}

/// `broughtBackLine`: the branch, then the pull request it opened or that there is none, then the host's note.
fn brought_back_line(back: &Value) -> String {
    let branch = back["branch"].as_str().unwrap_or_default();
    let note = back["note"].as_str();
    let line = match (back["pr"]["number"].as_u64(), note) {
        (Some(number), _) => format!("{branch}: pull request #{number} {}", back["pr"]["state"].as_str().unwrap_or_default()),
        (None, None) => format!("{branch} pushed"),
        (None, Some(_)) => format!("{branch} pushed, no pull request"),
    };
    match note {
        Some(note) => format!("{line}: {note}"),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{self, Side};

    /// Catches: a thread link naming the wildcard a host bound or an unbracketed IPv6 address, where the dial and
    /// today's app name the loopback the lock is dialled on; or a lock whose process is gone read as a page.
    #[test]
    fn the_host_page_is_where_the_lock_is_dialled() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state.json");
        let lock = |pid: u32, address: &str| {
            let lock = format!(r#"{{"pid":{pid},"port":4100,"address":"{address}","startedAt":"2026-10-19T00:00:00.000Z"}}"#);
            std::fs::write(dir.path().join("host.lock"), lock).unwrap();
            host_page(&state)
        };
        let me = std::process::id();
        assert_eq!(lock(me, "0.0.0.0").as_deref(), Some("http://127.0.0.1:4100/"));
        assert_eq!(lock(me, "::1").as_deref(), Some("http://[::1]:4100/"));
        assert_eq!(lock(me, "127.0.0.2").as_deref(), Some("http://127.0.0.2:4100/"));
        assert_eq!(lock(0, "127.0.0.1"), None);
    }
    use gpui_kit::component::Root;
    use gpui_kit::{size, TestAppContext, VisualTestContext};
    use model::Item;
    use rows::Drawn;

    fn workspace(id: &str) -> Value {
        json!({ "id": id, "name": id, "kind": "local", "project": { "id": "p1" } })
    }

    /// A finished thread on its own row; every fiftieth and the one after it are one send to two models.
    fn row(i: usize, workspace: &str, now: f64) -> Value {
        let attempt = (i >= 4 && (i - 4) % 50 < 2).then(|| format!("a{}", i - (i - 4) % 50));
        json!({
            "id": format!("r{i}"), "threadId": format!("t{i}"), "workspaceId": workspace, "harness": "claude",
            "status": "completed", "prompt": format!("thread {i}"), "project": { "id": "p1", "name": "repo" },
            "startedAt": now - 60_000. - i as f64, "endedAt": now - 30_000., "readAt": now - 30_000., "attempt": attempt,
        })
    }

    fn snapshot(n: usize, now: f64) -> Snapshot {
        Snapshot {
            sessions: (0..n).map(|i| row(i, "w0", now)).collect(),
            workspaces: vec![workspace("w0")],
            projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
            places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
            preferences: json!({}),
            statuses: vec![json!({ "id": "w0", "checkout": { "branch": "main" } })],
            ..Snapshot::default()
        }
    }

    /// The entries of every store the sidebar keeps that grows with threads, workspaces or what the person opened.
    fn sizes(s: &Sidebar) -> Vec<(&'static str, usize)> {
        let a = &s.around;
        vec![
            ("rows", s.rows.len()),
            ("threads", s.threads.len()),
            ("lines", s.lines.len()),
            ("drawn_rows", s.drawn_rows.len()),
            ("line_ids", s.line_ids.len()),
            ("working", s.working.len()),
            ("folds.finished_open", s.folds.finished_open.len()),
            ("folds.groups_shut", s.folds.groups_shut.len()),
            ("kinds", a.kinds.len()),
            ("workspaces", a.workspaces.len()),
            ("statuses", a.statuses.len()),
            ("checkouts_asked", a.checkouts_asked.len()),
            ("catalogs", a.catalogs.len()),
            ("forwards", a.forwards.len()),
            ("creations", a.creations.len()),
            ("launches", a.launches.len()),
        ]
    }

    fn drawn(cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let sidebar = cx.new(Sidebar::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(sidebar.clone(), window, cx));
        cx.simulate_resize(size(px(1280.), px(800.)));
        (sidebar, cx)
    }

    /// Draws every row of the list once, a window's height at a time, as a person scrolling to its end does.
    fn scroll_through(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) {
        let n = sidebar.read_with(cx, |s, _| s.drawn_rows.len());
        for at in (0..n).step_by(8).chain([0]) {
            sidebar.update(cx, |s, cx| {
                s.list.scroll_to(ListOffset { item_ix: at, offset_in_item: px(0.) });
                cx.notify();
            });
            cx.run_until_parked();
        }
    }

    fn png(shade: u8) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(128, 128, image::Rgba([shade, 0, 0, 255])).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[gpui_kit::test]
    fn a_project_icon_no_longer_named_leaves_the_image_cache(cx: &mut TestAppContext) {
        use base64::Engine as _;
        let (sidebar, cx) = drawn(cx);
        let now = now_ms();
        let (first, second) = (png(10), png(20));
        let url = |b: &[u8]| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b));
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
        for (hash, bytes) in [("aa", &first), ("bb", &second)] {
            let prefs = json!({ "projectIcon": { "p1": hash } });
            sidebar.update(cx, |s, cx| {
                s.apply(Change::Preferences(prefs.clone()), cx);
                // The reply to projects.icons, as fetch_icons takes it with a host attached.
                s.around.images.take(&json!({ hash: url(bytes) }), &prefs);
                cx.notify();
            });
            scroll_through(&sidebar, cx);
        }
        let cached = |bytes: &[u8], cx: &mut VisualTestContext| {
            let image = Arc::new(gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, bytes.to_vec()));
            cx.update(|_, cx| image.is_asset_cached(cx))
        };
        assert!(cached(&second, cx), "the icon named now is drawn from the image cache");
        assert!(!cached(&first, cx), "the icon no longer named is still decoded in the image cache");
        sidebar.update(cx, |s, cx| s.apply(Change::Preferences(json!({})), cx));
        scroll_through(&sidebar, cx);
        assert!(!cached(&second, cx), "an icon taken off its project is still decoded in the image cache");
    }

    #[gpui_kit::test]
    fn five_hundred_threads_on_their_own_workspaces_come_and_go_and_leave_every_store_as_it_was(cx: &mut TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        let now = now_ms();
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
        scroll_through(&sidebar, cx);
        let before = sidebar.read_with(cx, |s, _| sizes(s));

        let ids: Vec<String> = (4..500).map(|i| format!("w{i}")).collect();
        sidebar.update(cx, |s, cx| {
            for (i, w) in (4..500).zip(&ids) {
                s.apply(
                    Change::Creating(json!({ "type": "workspace.creating", "workspaceId": w, "name": w, "stage": "fork-requested" })),
                    cx,
                );
                s.apply(Change::Created(json!({ "type": "workspace.created", "workspace": { "id": w, "name": w } })), cx);
                let all = std::iter::once("w0").chain(ids[..=i - 4].iter().map(String::as_str)).map(workspace).collect();
                s.apply(Change::Workspaces(all), cx);
                s.apply(Change::Status(json!({ "id": w, "checkout": { "branch": "main" } })), cx);
                s.apply(Change::Forward(json!({ "workspaceId": w, "port": 3000, "kind": "app" })), cx);
                s.apply(Change::Row { id: format!("r{i}"), row: Some(row(i, w, now)) }, cx);
                // What the reply to the workspace's harnesses.list puts in, as fetch_catalogs does with a host attached.
                s.around.catalogs.insert(w.clone(), vec![Catalog { harness: "claude".into(), ..Catalog::default() }]);
            }
            // The person opens some leads' Finished folds and shuts some groups, as their rows' clicks do.
            for i in (4..500).step_by(50) {
                s.toggle_finished(&format!("t{i}"), cx);
                s.toggle_group(&format!("a{i}"), cx);
            }
        });
        scroll_through(&sidebar, cx);
        let full = sidebar.read_with(cx, |s, _| sizes(s));

        // Each thread is deleted, which the host pushes as its row gone, then its workspace.
        sidebar.update(cx, |s, cx| {
            for (i, w) in (4..500).zip(&ids) {
                s.apply(Change::Row { id: format!("r{i}"), row: None }, cx);
                s.apply(Change::WorkspaceDeleted(w.clone()), cx);
            }
        });
        scroll_through(&sidebar, cx);
        let after = sidebar.read_with(cx, |s, _| sizes(s));

        let kept: Vec<String> = before
            .iter()
            .zip(&full)
            .zip(&after)
            .filter(|((b, _), a)| b.1 != a.1)
            .map(|((b, f), a)| format!("{} {} -> {} -> {}", b.0, b.1, f.1, a.1))
            .collect();
        assert!(kept.is_empty(), "not freed, 4 threads -> 500 -> 4: {kept:?}");
    }

    fn row_of(s: &Sidebar, key: &str) -> Option<usize> {
        s.drawn_rows.iter().position(|d| match d {
            Drawn::Line(ix) => matches!(&s.lines[*ix].item, Item::Tile(t) if s.threads[t.thread].key == key && !t.inbox),
            _ => false,
        })
    }

    #[gpui_kit::test]
    fn the_key_ring_stays_on_its_thread_when_a_row_lands_above_it(cx: &mut TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        let now = now_ms();
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
        cx.run_until_parked();
        sidebar.update_in(cx, |s, window, cx| s.point_at("t2", window, cx));
        let (ring, at) = sidebar.read_with(cx, |s, _| (s.key_row, row_of(s, "t2")));
        assert_eq!(ring, at);
        let newer = json!({
            "id": "r9", "threadId": "t9", "workspaceId": "w0", "harness": "claude", "status": "completed", "prompt": "thread 9",
            "project": { "id": "p1", "name": "repo" }, "startedAt": now - 1_000., "endedAt": now - 500., "readAt": now - 500.,
        });
        sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "r9".into(), row: Some(newer) }, cx));
        cx.run_until_parked();
        let (ring, moved) = sidebar.read_with(cx, |s, _| (s.key_row, row_of(s, "t2")));
        assert_ne!(moved, at, "the newer thread stands above the one the keys are on");
        assert_eq!(ring, moved, "the ring follows its thread to its new row");
    }

    fn processes(ends: &dialogs::Ends) -> dialogs::OpenDialog {
        dialogs::OpenDialog::Processes {
            ends: ends.clone(),
            title: "Delete b?".into(),
            line: None,
            left: None,
            busy: Some(Processes::Keep),
            said: None,
            checked: false,
        }
    }

    #[gpui_kit::test]
    fn a_reply_for_another_dialog_leaves_the_open_one_as_it_is(cx: &mut TestAppContext) {
        use wsp_host_client::Failure;
        let (sidebar, cx) = drawn(cx);
        let (a, b) = (dialogs::Ends::Delete("a".into()), dialogs::Ends::Delete("b".into()));
        sidebar.update(cx, |s, cx| {
            s.show_dialog(processes(&b), true, cx);
            s.checked(&a, Err(Failure::of_kind("a still runs a server", "processes-left")), cx);
            s.finished(&a, Ok(()), cx);
            let Some(dialogs::OpenDialog::Processes { ends, left, checked, busy, .. }) = s.dialog.as_ref().map(|d| &d.open) else {
                panic!("the late Ok for a closed b's delete dialog");
            };
            assert_eq!((ends, left, checked, busy), (&b, &None, &false, &Some(Processes::Keep)), "a's check landed on b's dialog");
            s.finished(&b, Ok(()), cx);
            assert!(s.dialog.is_none(), "b's own Ok closes it");
        });

        let (wa, wb) = (vec!["wa".to_owned()], vec!["wb".to_owned()]);
        sidebar.update(cx, |s, cx| {
            let open = dialogs::OpenDialog::Workspaces {
                road: dialogs::Road::Forget,
                ids: wb.clone(),
                title: "Forget wb?".into(),
                line: "".into(),
                holding: Some(Vec::new()),
                busy: true,
                said: None,
            };
            s.show_dialog(open, true, cx);
            s.read_holding(&wa, vec!["wa holds 1 commit not pushed".into()], true, cx);
            s.ended(&wa, None, cx);
            assert_eq!(s.dialog_words().map(|w| w.len()), Some(2), "a's read or end landed on b's dialog: {:?}", s.dialog_words());
            s.ended(&wb, Some("the host said no".into()), cx);
            assert_eq!(s.dialog_words().and_then(|w| w.last().cloned()).as_deref(), Some("the host said no"));
        });
    }

    #[gpui_kit::test]
    fn the_delete_names_the_worktree_only_for_the_last_thread_in_one_wsp_made(cx: &mut TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        let now = now_ms();
        let mut snap = snapshot(2, now);
        snap.workspaces = vec![
            json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" }, "worktree": { "made": true, "path": "/wt" } }),
        ];
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snap)), cx));
        let line = |key: &str, cx: &mut VisualTestContext| {
            sidebar.update_in(cx, |s, window, cx| {
                dialogs::delete(s, key, window, cx);
                let words = s.dialog_words().unwrap_or_default();
                s.dialog = None;
                words.get(1).cloned().unwrap_or_default()
            })
        };
        assert!(line("t0", cx).contains("the folder it worked in is left as it is"), "another thread is still in the worktree");
        sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "r1".into(), row: None }, cx));
        assert!(line("t0", cx).contains("the worktree at /wt with it"), "the last thread in it takes the worktree");
        for tree in [json!({ "made": false, "path": "/wt" }), json!({ "made": true, "gone": true, "path": "/wt" })] {
            let w = json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" }, "worktree": tree });
            sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![w]), cx));
            assert!(line("t0", cx).contains("the folder it worked in is left as it is"), "a worktree wsp did not make, or gone, stays");
        }
    }

    #[gpui_kit::test]
    fn the_switcher_head_opens_and_closes_its_popup_and_a_press_outside_closes_it(cx: &mut TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now_ms()))), cx));
        cx.run_until_parked();
        let head = cx.debug_bounds("projects").expect("the projects switcher is drawn").center();
        let open = |cx: &mut VisualTestContext| sidebar.read_with(cx, |s, _| s.switcher.is_some());
        let click = |at, cx: &mut VisualTestContext| {
            cx.simulate_click(at, gpui_kit::Modifiers::none());
            cx.run_until_parked();
        };
        click(head, cx);
        assert!(open(cx), "the head opens its popup");
        click(head, cx);
        assert!(!open(cx), "a second press on the head closes it");
        click(head, cx);
        click(gpui_kit::point(gpui_kit::px(900.), gpui_kit::px(700.)), cx);
        assert!(!open(cx), "a press outside closes it");
        click(head, cx);
        assert!(open(cx), "and the head opens it again at once");
    }

    #[gpui_kit::test]
    fn a_right_click_on_a_tile_opens_its_menu_and_a_pick_runs_the_item(cx: &mut TestAppContext) {
        use gpui_kit::{Modifiers, MouseButton};
        let (sidebar, cx) = drawn(cx);
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now_ms()))), cx));
        cx.run_until_parked();
        let tile = cx.debug_bounds("tile-t1").expect("the tile is drawn").center();
        cx.simulate_mouse_down(tile, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(tile, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let (asked, items) = sidebar.read_with(cx, |s, _| {
            let tile = s.lines.iter().find_map(|l| match &l.item {
                Item::Tile(t) if s.threads[t.thread].key == "t1" => Some(t.clone()),
                _ => None,
            });
            let items = menu::items_for(s, &tile.unwrap(), "t1");
            let labels: Vec<_> = items.iter().map(|i| i.label.clone()).collect();
            (s.menu.asked().to_vec(), labels)
        });
        assert_eq!(asked, items, "a right-click on the tile asks for its menu");
        // On the Mac AppKit draws the menu, which the test platform cannot drive.
        if cfg!(target_os = "macos") {
            return;
        }
        assert!(sidebar.read_with(cx, |s, _| s.menu.is_open()), "the tile's menu is open");
        let downs = sidebar.read_with(cx, |s, _| {
            let tile = s.lines.iter().find_map(|l| match &l.item {
                Item::Tile(t) if s.threads[t.thread].key == "t1" => Some(t.clone()),
                _ => None,
            });
            let items = menu::items_for(s, &tile.unwrap(), "t1");
            let rename = items.iter().position(|i| i.act == menu::Act::Rename).expect("a Rename item");
            // gpui-kit's first Down lands on the first row, held or not, and each one after on the next that runs.
            items[..=rename].iter().filter(|i| i.refusal.is_none()).count() + usize::from(items[0].refusal.is_some())
        });
        cx.simulate_keystrokes(&vec!["down"; downs].join(" "));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(sidebar.read_with(cx, |s, cx| s.rename_box(cx)).as_deref(), Some("thread 1"), "Rename opened the box on the thread");
        assert!(sidebar.read_with(cx, |s, _| !s.menu.is_open()));
    }
}
