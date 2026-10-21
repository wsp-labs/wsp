// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar as today's app draws it (`apps/web/src/sidebar/`): the header, the four rows over the list, the list
//! of thread tiles in its sections with their trees and Settled, and the foot with the cloud card and the corner. It
//! reads and writes through `wsp-host-client` and follows the host's pushes; the list is virtual, so only the rows
//! on screen are built, whatever the number of threads.

pub(crate) mod age;
pub mod alone;
pub(crate) mod card;
pub mod chrome;
mod dialogs;
mod drag;
mod foot;
pub mod host;
mod keys;
pub mod looks;
pub(crate) mod markdown;
pub(crate) mod menu;
pub mod model;
mod rename;
pub mod rows;
pub mod shared;
mod switcher;
pub(crate) mod tile;
pub(crate) mod walk;
mod writes;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::input::InputState;
use gpui_kit::component::v_flex;
use gpui_kit::{
    div, list, prelude::*, px, App, Bounds, Context, Entity, EventEmitter, ListAlignment, ListOffset, ListState, Pixels, Subscription,
    Task, Window,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{HarnessesListRequest, PreferencesSetRequest, ProjectsIconsRequest, SessionView};
use wsp_host_client::{lock, HostClient};

use crate::local_store::LocalStore;
use crate::screens::app_shell::title::thread_title;
use crate::ui::context_menu::ContextMenu;
use crate::ui::status::hold_crabs;
use crate::ui::theme::{self, scoped};
use crate::ui::{ActivePalette, Glyph};
use host::{Change, Snapshot};
use model::{Folds, Item, Line, Prefs, Row, Section, Shape, Thread};

pub(crate) use rename::rename_not_taken_line;

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

/// Whether `row` stands in the tree of `thread`: the thread itself, or a thread under it.
fn in_tree(row: &Row, thread: &str) -> bool {
    [row.thread_id.as_deref(), row.root_thread_id.as_deref(), row.parent_thread_id.as_deref()].contains(&Some(thread))
}

/// A workspace's agents as one copy shared with every workspace that listed the same ones: the workspaces of one
/// computer list one catalog, and a host with hundreds of them held it hundreds of times.
fn shared_catalog(held: &HashMap<String, Rc<Vec<Value>>>, listed: Vec<Value>) -> Rc<Vec<Value>> {
    held.values().find(|kept| ***kept == listed).cloned().unwrap_or_else(|| Rc::new(listed))
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
    /// The call of the selected thread's subagent whose page is open, which marks its row in place of the lead's tile.
    opened: Option<String>,
    /// The open thread, where it was opened from the live list: the time fold leaves it out of Settled.
    held: Option<String>,
    /// The roots of every tree a thread of which works, which take Stop in place of Settle.
    working: HashSet<String>,
    note: Option<String>,
    refusal: Option<String>,
    /// Shared with the feed and Activity ([`shared`]).
    rows: Vec<Arc<Row>>,
    threads: Vec<Thread>,
    lines: Vec<Line>,
    /// Every row the list draws, and what each stands for, read off what it was drawn from.
    drawn_rows: Vec<rows::Drawn>,
    line_ids: Vec<String>,
    /// The foot's folded sections, by the web's ids: setting-up, forwards.
    foot_folds: HashSet<String>,
    folds: Folds,
    prefs: Prefs,
    around: Around,
    list: ListState,
    link: Option<Link>,
    /// Whether the host is this computer's own, read once as the link is made: no link reads as here, as a page
    /// with no shell does.
    here: bool,
    following: Option<Following>,
    renaming: Option<Renaming>,
    armed: Option<(String, Instant)>,
    _disarm: Task<()>,
    pub(crate) switcher: Option<switcher::OpenSwitcher>,
    /// Where each switcher's head was drawn, so a press on it is not read as a press outside its popup.
    switcher_heads: Rc<Cell<[Bounds<Pixels>; 2]>>,
    pub(crate) menu: ContextMenu,
    /// The tile the pointer rests on, and the wait before its card opens.
    hovered: Option<String>,
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
    drag: drag::Drag,
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
        // A settle or a kind by time moves by the minute; the rows' times step on their own.
        let clock = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(30)).await;
            if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                break;
            }
        });
        cx.observe_global::<crate::update::Updates>(|_, cx| cx.notify()).detach();
        let (kept_sections, kept_setting_up) = model::stored_folds(cx.try_global::<LocalStore>());
        Sidebar {
            items: Vec::new(),
            selected: None,
            opened: None,
            held: None,
            working: HashSet::new(),
            note: Some("Connecting".to_owned()),
            refusal: None,
            rows: Vec::new(),
            threads: Vec::new(),
            lines: Vec::new(),
            line_ids: Vec::new(),
            drawn_rows: Vec::new(),
            foot_folds: if kept_setting_up { HashSet::from(["setting-up".to_owned()]) } else { HashSet::new() },
            folds: Folds { sections: kept_sections, ..Folds::default() },
            prefs: Prefs::default(),
            around: Around::default(),
            list,
            link: None,
            here: true,
            following: None,
            renaming: None,
            armed: None,
            _disarm: Task::ready(()),
            switcher: None,
            switcher_heads: Rc::default(),
            menu: ContextMenu::default(),
            hovered: None,
            focus: cx.focus_handle(),
            key_row: None,
            keyboard: false,
            keys_back: false,
            _card_wait: None,
            page: cx.palette().clone(),
            dialog: None,
            computer: None,
            drive: None,
            drag: drag::Drag::default(),
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
        let left = self.selected.replace(item.thread_id.clone()).filter(|left| *left != item.thread_id);
        self.opened = None;
        self.folds.open = self.selected.clone();
        if let Some(left) = left {
            for row in self.rows.iter_mut().filter(|r| r.cuts() && in_tree(r, &left)) {
                *row = shared::share((**row).clone().kept());
            }
        }
        self.rebuild(cx);
        self.read_whole(cx);
        Some(item)
    }

    /// A row as the sidebar keeps it: whole in the open thread's tree, whose view draws what a settled row leaves out,
    /// else cut and shared with the other readers ([`Row::kept`]).
    fn keep(&self, row: Row) -> Arc<Row> {
        match &self.selected {
            Some(open) if row.cuts() && in_tree(&row, open) => Arc::new(row),
            _ => shared::share(row.kept()),
        }
    }

    /// Reads whole the open thread's rows that are kept cut, on each workspace they stand on; the thread left has its
    /// rows cut again ([`Sidebar::mark`]).
    fn read_whole(&mut self, cx: &mut Context<Self>) {
        let Some(open) = self.selected.clone() else { return };
        let workspaces: HashSet<String> =
            self.rows.iter().filter(|r| r.cuts() && in_tree(r, &open) && shared::holds(r)).map(|r| r.workspace_id.clone()).collect();
        let Some(link) = self.link.clone().filter(|_| !workspaces.is_empty()) else { return };
        let task = link.runtime.spawn(async move {
            let mut whole = Vec::new();
            for workspace in workspaces {
                whole.extend(shared::whole(&link.host, &workspace).await?);
            }
            Ok::<_, wsp_host_client::Failure>(whole)
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(whole)) = task.await else { return };
            _ = this.update(cx, |this, cx| {
                if this.selected.as_deref() != Some(open.as_str()) {
                    return;
                }
                let whole: HashMap<String, Row> = whole.into_iter().filter(|r| in_tree(r, &open)).map(|r| (r.id.clone(), r)).collect();
                let mut moved = false;
                for row in &mut this.rows {
                    // Only a row still as it was read: a push or a full read that landed since is newer than the answer.
                    if let Some(read) = whole.get(&row.id).filter(|read| (*read).clone().kept() == **row) {
                        *row = Arc::new(read.clone());
                        moved = true;
                    }
                }
                if moved {
                    this.revision += 1;
                    this.refold(cx);
                }
            });
        })
        .detach();
    }

    /// Marks no thread as the open one, as a page with none selected (the New thread page, the computer's own) shows.
    pub fn unmark(&mut self, cx: &mut Context<Self>) {
        if self.selected.take().is_some() | self.opened.take().is_some() {
            self.rebuild(cx);
        }
    }

    /// The row drawn as the open one, by its selector.
    pub fn lit_row(&self) -> Option<String> {
        tile::lit_row(self)
    }

    /// Marks the page of `lead`'s subagent launched by `call` as the open one: its row, not its lead's tile.
    pub fn mark_subagent(&mut self, lead: &str, call: &str, cx: &mut Context<Self>) {
        self.mark(lead, cx);
        self.opened = Some(call.to_owned());
        cx.notify();
    }

    fn select(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(item) = self.mark(key, cx) {
            cx.emit(Selected(item));
        }
    }

    /// The sections and trees as they stand now.
    fn view(&self) -> model::View<'_> {
        model::sidebar(&self.threads, &self.prefs, self.now, self.held.as_deref(), self.machines())
    }

    /// A workspace's record and its live status as this connection last read them.
    pub fn workspace(&self, id: &str) -> (Option<&Value>, Option<&Value>) {
        (self.around.workspaces.get(id), self.around.statuses.get(id))
    }

    /// The branch a workspace's folder is on (`ComposerCheckoutRow`'s head): its checkout as the host last read it,
    /// else the branch its worktree was made on; none on a detached head.
    pub(crate) fn branch_of(&self, workspace: &str) -> Option<String> {
        let checkout = self.around.statuses.get(workspace).and_then(|s| s.get("checkout")).filter(|c| !c.is_null());
        let branch = match checkout {
            Some(c) => c.get("branch").and_then(Value::as_str),
            None => self.around.workspaces.get(workspace)?.pointer("/worktree/branch").and_then(Value::as_str),
        }?;
        // `DETACHED_HEAD`, the word the checkout carries for a head on no branch.
        (!branch.is_empty() && branch != "(detached)").then(|| branch.to_owned())
    }

    /// The workspace a project's New thread page opens on: the project's own folder, else any of its tasks.
    pub(crate) fn home_of(&self, project: &str) -> Option<String> {
        let of = || self.around.workspaces.values().filter(|w| w.pointer("/project/id").and_then(Value::as_str) == Some(project));
        of().find(|w| matches!(w["kind"].as_str(), Some("local" | "place")) && w.get("worktree").is_none())
            .or_else(|| of().next())
            .and_then(|w| w["id"].as_str())
            .map(str::to_owned)
    }

    /// `useHomeProject`'s page with nothing picked: the New thread page of the first project the host lists that has
    /// a workspace to open it on.
    pub(crate) fn home_workspace(&self) -> Option<String> {
        self.around.projects.iter().find_map(|(id, _)| self.home_of(id))
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
        self.here = link.host.here();
        self.link = Some(link);
        self.following = Some(Following { task, _receive: receive });
    }

    /// One change the host pushed, or its first read.
    pub fn apply(&mut self, change: Change, cx: &mut Context<Self>) {
        match change {
            Change::Whole(snapshot) => {
                self.take(*snapshot);
                self.read_whole(cx);
                self.update_in_background(cx);
            }
            Change::Row { id, row } => {
                let at = self.rows.iter().position(|r| r.id == id);
                match (at, row.and_then(Row::owned)) {
                    (Some(at), Some(row)) => {
                        // A pushed row may leave out the prompt the row it replaces carried.
                        let prompt = row.prompt.clone().or_else(|| self.rows[at].prompt.clone());
                        self.rows[at] = self.keep(Row { prompt, ..row });
                    }
                    (None, Some(row)) => {
                        let row = self.keep(row);
                        self.rows.push(row);
                    }
                    (Some(at), None) => {
                        self.rows.remove(at);
                    }
                    (None, None) => {}
                }
            }
            Change::Preferences(p) => {
                self.take_preferences(p);
                self.update_in_background(cx);
            }
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
            Change::Release(release) => {
                self.around.release = Some(release);
                self.update_in_background(cx);
            }
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
    pub fn rows(&self) -> &[Arc<Row>] {
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
                let rows: Vec<Arc<Row>> =
                    self.rows.iter().filter(|r| self.place_of(&r.workspace_id).as_deref() == Some(on.as_str())).cloned().collect();
                model::fold_threads(&rows)
            }
        };
        // threadTree.ts `thread.parentThreadId ?? forkedBy`: a thread with no opener on its rows hangs under the
        // thread that started the machine it runs on.
        for t in self.threads.iter_mut().filter(|t| t.parent.is_none()) {
            let forked = |k: &str| self.around.workspaces.get(&t.workspace_id).and_then(|w| w.get(k)?.as_str().map(str::to_owned));
            if let Some(lead) = forked("parentThreadId").filter(|lead| *lead != t.key) {
                t.root = t.root.take().or_else(|| forked("rootThreadId"));
                t.parent = Some(lead);
            }
        }
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
                    let shared = shared_catalog(&this.around.listed, listed);
                    this.around.listed.insert(workspace.clone(), shared);
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
        self.rows = s.sessions;
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

    /// Reads the clock again, and draws the list again only where a thread settled or changed its kind by time.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        let offset = utc_offset_ms() as f64;
        // A kind's word moves by time too: Resuming's "resets in N min".
        let kind_moved = |l: &Line| match &l.item {
            Item::Tile(tile) => self.threads.get(tile.thread).is_some_and(|t| {
                let (kind, was) = (model::tile_kind(t, tile, now), model::tile_kind(t, tile, self.now));
                kind != was || model::word(t, kind, now, offset) != model::word(t, was, self.now, offset)
            }),
            _ => false,
        };
        // The lines as a rebuild lays them, Working's head counting the workspaces being made.
        let mut folds = self.folds.clone();
        folds.making = self.creations_shown().1.len();
        let lines = model::sidebar(&self.threads, &self.prefs, now, self.held.as_deref(), self.machines()).lines(&folds);
        if lines != self.lines || self.lines.iter().any(kind_moved) {
            self.rebuild(cx);
        }
    }

    /// Draws the list again from the threads, keeping the scroll where it was.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.now = now_ms();
        self.folds.making = self.creations_shown().1.len();
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
            Item::Machine { workspace } => format!("w:{workspace}"),
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
        self.keep_fold(section.id(), self.folds.sections.contains(&section), cx);
        self.rebuild(cx);
    }

    pub(super) fn keep_fold(&self, id: &str, folded: bool, cx: &mut Context<Self>) {
        if cx.has_global::<LocalStore>() {
            if let Err(e) = model::keep_fold(cx.global_mut::<LocalStore>(), id, folded) {
                eprintln!("desktop-gpui: sidebar fold not kept: {e}");
            }
        }
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
        let Some(marks) = self.view().move_root(key, to) else { return };
        for m in marks {
            self.hold_marks(&m.thread_ids, &m.marks);
            self.mark_threads(m.thread_ids, m.marks, cx);
        }
        self.refold(cx);
    }

    pub fn dismiss_cloud_card(&mut self, cx: &mut Context<Self>) {
        self.write::<PreferencesSetRequest>(json!({ "patch": { "cloudCardDismissed": true } }), cx);
    }
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("Sidebar");
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
        let scope = theme::sidebar_scope(cx);
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
            // Each row reads the place under the pointer in the same move; this takes it once they all have.
            .on_drag_move(cx.listener(|_, _: &gpui_kit::DragMoveEvent<tile::DraggedTile>, window, cx| {
                cx.defer_in(window, |this, _, cx| this.commit_place(cx))
            }))
            .on_drop(cx.listener(|this, d: &tile::DraggedTile, _, cx| this.drop_tree(&d.key.clone(), cx)))
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
        let row = if self.folds.dragging.is_some() { self.drag_row(ix, row, cx) } else { row };
        let ring = self.ring(ix, &cx.palette().clone(), window);
        match ring {
            Some(ring) => div().relative().child(row).child(ring).into_any_element(),
            None => row,
        }
    }
}

#[cfg(test)]
mod tests;
