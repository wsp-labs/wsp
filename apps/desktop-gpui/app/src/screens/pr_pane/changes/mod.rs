// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel's Changes pane as today's app draws it (`diffs/DiffSurface.tsx`): git.diff over the workspace's
//! daemon link in the panes' shared folder, a scope menu (Branch changes first, kept per workspace on the local
//! store), the folder's branch, the changed-files tree under its fold, and the shared code view with each file's
//! viewed tick, discard and in-place edit. The head opens the commit box, whose message the workspace's own agent
//! drafts. A reply's changed-files card opens it on one turn's range (`git.turn`) through [`model::open_turn`].

pub mod commit;
pub mod discard;
pub mod host;
pub mod model;
pub mod not_running;
pub mod tree;
pub mod words;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::input::{Editor, EditorState, RopeExt as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, AnyWindowHandle, App, AppContext, BoxShadow, Context, Entity, FocusHandle, Focusable, Pixels,
    SharedString, Subscription, Task, UniformListScrollHandle, WeakEntity, Window,
};
use serde_json::{json, Map, Value};

use self::commit::CommitState;
use self::discard::{DiscardDialog, DiscardState};
use self::model::{base_name, Folds, Kept, Scope, TreeFile, TurnRange};
use self::tree::ChangedFilesTree;
use super::comments::{send_to_thread, LineComments, LineCommentsEvent};
use super::daemon::{DaemonLink, Refused};
use super::diff::{self, CodeView, DiffModel, FileHead, FilePatch, FileStatus, Layout};
use super::files::crumbs::{CrumbRow, Pick};
use super::files::paths::{parent_within, roots};
use super::files::ShowInChanges;
use crate::local_store::LocalStore;
use crate::screens::computer::daemon_down::{DaemonDown, Restart};
use crate::screens::terminal::link::Status;
use crate::screens::terminal::state::{pane_state, WorkspaceState};
use crate::screens::{Host, Opened, Pane, Place, Screen};
use crate::ui::controls::checkbox;
use crate::ui::menu::menu_on;
use crate::ui::{spinner, tagged, tip, ActivePalette as _, Button, Glyph, MenuAct};

/// Tailwind's `sm`: under it the head keeps the scope and its acts and lets the folder, branch and view go.
const NARROW: Pixels = px(640.);
/// `RIGHT_PANEL_SHEET_MAX_VIEWPORT`: past it the panel stands inline and the head is 28 px.
const SHEET_MAX: Pixels = px(980.);

/// The diff read for the folder: in flight, read, or failed with the line that stands in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load {
    Idle,
    Pending,
    Ready,
    /// A refused read keeps the last diff and shows git's sentence in red; a folder outside any repository and a
    /// pruned turn snapshot draw their own line in place of the diff.
    Error {
        message: String,
        line: Option<&'static str>,
    },
}

/// The repository git resolved for the folder: its top level and branch, once read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub root: String,
    pub branch: String,
}

/// The changed-files tree's rows and what each file row shows: its viewed tick (none for a file that is gone).
#[derive(Default)]
struct TreeCache {
    files: Rc<Vec<TreeFile>>,
    rows: Rc<Vec<model::TreeRow>>,
    shut: Rc<HashSet<String>>,
    viewed: Rc<HashMap<String, Option<bool>>>,
    any_blob: bool,
    viewed_count: usize,
}

struct Editing {
    path: String,
    editor: Entity<EditorState>,
    /// The text the editor opened on: a save that leaves it as it was writes nothing.
    loaded: String,
    saving: bool,
}

pub struct ChangesScreen {
    host: Host,
    opened: Opened,
    link: Option<Entity<DaemonLink>>,
    facts: Option<host::Facts>,
    kept: Kept,
    load: Load,
    repo: Option<Repo>,
    /// The top of the project's own checkout as git names it, where the host's discard and commit act.
    project_root: Option<String>,
    /// The folder git last ran in.
    folder: Option<String>,
    /// What the diff shows, a turn's narrowing applied, and how many files the whole read held.
    diff: Option<Rc<DiffModel>>,
    whole: usize,
    /// The scope or turn the shown diff was read for: a new one is a new set of files.
    read_for: Option<String>,
    marks: HashMap<String, String>,
    folds: Folds,
    shut: HashSet<String>,
    /// The changed-files tree as drawn, built again on a read, a push of marks or a folder's toggle, not per frame.
    tree: TreeCache,
    tree_open: bool,
    commit: Option<CommitState>,
    discarding: Option<DiscardState>,
    editing: Option<Editing>,
    write_note: Option<String>,
    reveal_note: Option<String>,
    /// The link would not open: the thread's computer is not running.
    not_running: bool,
    /// Whether a read came over the link before, and how the computer's channel stands.
    live_once: bool,
    socket: Status,
    /// The last Start again the computer-absent form asked, kept past the frames it is drawn in.
    restart: Entity<Restart>,
    code: Entity<CodeView>,
    /// This tab's line comments, handed to the thread's composer by Send to thread.
    comments: Entity<LineComments>,
    tree_scroll: UniformListScrollHandle,
    focus: FocusHandle,
    /// The window the pane is in, where a read started outside a window's update runs.
    window: AnyWindowHandle,
    width: Pixels,
    /// Counts the scopes and turns shown: a commit draft or whole-file read asked under another is dropped.
    asks: u64,
    /// The last commit draft and whole-file read asked; a reply to an older one is dropped.
    draft_ask: u64,
    edit_ask: u64,
    /// The capture words (`WSP_GPUI_SHOT`, split by commas) still to put on, each once the read it needs came.
    shot: std::collections::VecDeque<String>,
    seq: u64,
    _fetch: Option<Task<()>>,
    _road: Option<Task<()>>,
    _pushes: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Screen for ChangesScreen {
    const NAME: &'static str = "changes";
    const PLACE: Place = Place::Panel(Pane::Changes);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        ChangesScreen::open(host, opened, window, cx)
    }
}

impl ChangesScreen {
    pub fn open(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let kept = cx.try_global::<LocalStore>().map(Kept::read).unwrap_or_default();
        let code = cx.new(CodeView::new);
        let comments = cx.new(|_| LineComments::new());
        let restart = cx.new(|_| Restart::default());
        let me = cx.weak_entity();
        code.update(cx, |view, cx| {
            view.set_layout(kept.layout, cx);
            let heads = me.clone();
            view.on_head(move |head, window, cx| match heads.upgrade() {
                Some(screen) => ChangesScreen::head_row(&screen, head, window, cx),
                None => div().into_any_element(),
            });
            let folds = me.clone();
            view.on_fold(move |path, _, _, cx| {
                _ = folds.update(cx, |this, cx| this.toggle_file(path, cx));
            });
            let notes = comments.clone();
            view.on_note(move |note, window, cx| {
                LineComments::render_note(&notes, note, window, cx).unwrap_or_else(|| div().into_any_element())
            });
            let adds = me.clone();
            view.on_add_comment(move |range, window, cx| {
                _ = adds.update(cx, |this, cx| this.begin_comment(range, window, cx));
            });
        });
        let mut subscriptions = vec![cx.subscribe(&comments, |this, comments, _: &LineCommentsEvent, cx| {
            let notes = comments.read(cx).notes();
            this.code.update(cx, |view, cx| view.set_notes(notes, cx));
            cx.notify();
        })];
        if cx.try_global::<LocalStore>().is_some() {
            subscriptions.push(cx.observe_global::<LocalStore>(|this, cx| this.kept_changed(cx)));
        }
        subscriptions.push(cx.observe_global::<ShowInChanges>(|this, cx| this.take_show(cx)));
        subscriptions.push(cx.observe(&restart, |_, _, cx| cx.notify()));
        let link = opened.workspace_id.as_deref().map(|id| DaemonLink::of(&host, id, cx));
        if let Some(link) = &link {
            subscriptions.push(cx.observe(link, |this, _, cx| this.link_changed(cx)));
            if let Some(thread) = &opened.thread_id {
                link.read(cx).follow_shell(thread);
            }
        }
        let socket = link.as_ref().map_or(Status::Opening, |l| l.read(cx).status());
        let mut screen = ChangesScreen {
            host,
            opened,
            link,
            facts: None,
            kept,
            load: Load::Idle,
            repo: None,
            project_root: None,
            folder: None,
            diff: None,
            whole: 0,
            read_for: None,
            marks: HashMap::new(),
            folds: Folds::default(),
            shut: HashSet::new(),
            tree: TreeCache::default(),
            tree_open: true,
            commit: None,
            discarding: None,
            editing: None,
            write_note: None,
            reveal_note: None,
            not_running: false,
            live_once: false,
            socket,
            restart,
            code,
            comments,
            tree_scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            window: window.window_handle(),
            width: px(0.),
            asks: 0,
            draft_ask: 0,
            edit_ask: 0,
            shot: super::shot().map(|s| s.split(',').map(|w| w.trim().to_owned()).filter(|w| !w.is_empty()).collect()).unwrap_or_default(),
            seq: 0,
            _fetch: None,
            _road: None,
            _pushes: None,
            _subscriptions: subscriptions,
        };
        screen.follow(window, cx);
        screen
    }

    fn workspace_id(&self) -> Option<&str> {
        self.opened.workspace_id.as_deref()
    }

    fn thread_id(&self) -> Option<&str> {
        self.opened.thread_id.as_deref()
    }

    pub fn scope(&self) -> Scope {
        self.workspace_id().map(|w| self.kept.scope(w)).unwrap_or_default()
    }

    pub fn turn(&self) -> Option<&TurnRange> {
        self.kept.turn(self.workspace_id()?, self.thread_id()?)
    }

    pub fn load(&self) -> &Load {
        &self.load
    }

    pub fn diff(&self) -> Option<&DiffModel> {
        self.diff.as_deref()
    }

    pub fn repo(&self) -> Option<&Repo> {
        self.repo.as_ref()
    }

    pub fn folder(&self) -> Option<&str> {
        self.folder.as_deref()
    }

    pub fn marks(&self) -> &HashMap<String, String> {
        &self.marks
    }

    pub fn not_running(&self) -> bool {
        self.not_running
    }

    /// What the pane draws while there is no daemon to read over; none while there is one.
    pub fn not_running_form(&self) -> Option<not_running::Form> {
        if !self.not_running {
            return None;
        }
        let Some(mut input) = self.facts.as_ref().and_then(|f| f.pane.clone()) else {
            return Some(not_running::Form::Thread { line: words::READ_ON_COMPUTER.to_owned() });
        };
        input.socket = self.socket.clone();
        Some(not_running::form(&pane_state(&input), words::READ_ON_COMPUTER))
    }

    pub fn restart(&self) -> &Entity<Restart> {
        &self.restart
    }

    pub fn commit_state(&self) -> Option<&CommitState> {
        self.commit.as_ref()
    }

    pub fn discard_state(&self) -> Option<&DiscardState> {
        self.discarding.as_ref()
    }

    pub fn write_note(&self) -> Option<&str> {
        self.write_note.as_deref()
    }

    pub fn reveal_note(&self) -> Option<&str> {
        self.reveal_note.as_deref()
    }

    pub fn code_view(&self) -> &Entity<CodeView> {
        &self.code
    }

    pub fn editing(&self) -> Option<&str> {
        self.editing.as_ref().map(|e| e.path.as_str())
    }

    /// The file `path` shows folded now.
    pub fn is_folded(&self, path: &str) -> bool {
        self.file(path).is_some_and(|f| self.folds.folded(path, model::is_viewed(f, &self.marks)))
    }

    fn file(&self, path: &str) -> Option<&FilePatch> {
        self.diff.as_ref()?.files.iter().find(|f| f.path.as_ref() == path)
    }

    fn is_turn(&self) -> bool {
        self.turn().is_some()
    }

    /// The panes show another repository than the project's: the host's discard and commit act in the project's
    /// checkout by the paths this one names, so neither is offered here.
    fn elsewhere(&self) -> bool {
        matches!((&self.repo, &self.project_root), (Some(repo), Some(top)) if repo.root != *top)
    }

    /// Reads the workspace's facts and marks and follows their pushes; the first facts start the first read.
    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded();
        let thread = self.opened.thread_id.clone();
        let road = self.host.run(move |client| host::follow(client, workspace_id, thread, tx));
        self._road = Some(cx.background_spawn(async move {
            _ = road.await;
        }));
        self._pushes = Some(cx.spawn_in(window, async move |this, cx| {
            while let Some(push) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.pushed(push, window, cx)).is_err() {
                    return;
                }
            }
        }));
    }

    fn pushed(&mut self, push: host::Push, window: &mut Window, cx: &mut Context<Self>) {
        match push {
            host::Push::Facts(facts) => {
                let facts = *facts;
                let first = self.facts.is_none();
                let moved = self.facts.as_ref().map(|f| (&f.followed, &f.project)) != Some((&facts.followed, &facts.project));
                // A daemon can come back while the phase stays running, so any word of the workspace reads again.
                let woke = self.not_running && facts.pane.as_ref().is_some_and(|p| p.state == WorkspaceState::Running);
                self.facts = Some(facts);
                if let Some(link) = self.link.clone().filter(|_| woke) {
                    DaemonLink::redial(&link, true, cx);
                }
                if first || moved || woke {
                    self.fetch(window, cx);
                }
            }
            host::Push::Marks(marks) => {
                self.marks = marks;
                self.rebuild_tree();
                self.push_folds(cx);
            }
        }
        cx.notify();
    }

    fn kept_changed(&mut self, cx: &mut Context<Self>) {
        let Some(kept) = cx.try_global::<LocalStore>().map(Kept::read) else { return };
        // The store holds every screen's keys, the composer's drafts among them: only this key's change draws again.
        if kept == self.kept {
            return;
        }
        let before = (self.scope(), self.turn().cloned());
        self.kept = kept;
        let layout = self.kept.layout;
        self.code.update(cx, |view, cx| {
            if view.layout() != layout {
                view.set_layout(layout, cx);
            }
        });
        if before != (self.scope(), self.turn().cloned()) {
            self.asks += 1;
            self.refetch(cx);
        }
        cx.notify();
    }

    fn link_changed(&mut self, cx: &mut Context<Self>) {
        let Some(link) = &self.link else { return };
        self.socket = link.read(cx).status();
        let folder = self.facts.as_ref().and_then(|f| link.read(cx).folder(self.thread_id(), f.followed.as_deref(), f.project.as_deref()));
        // The channel moved: one that came up is read over, and one that went is found out by reading again.
        let up = link.read(cx).up();
        let back = self.not_running && up;
        let lost = !self.not_running && self.live_once && !up && self.load != Load::Pending;
        if back || lost || (self.turn().is_none() && folder.is_some() && folder != self.folder) {
            self.refetch(cx);
        }
        cx.notify();
    }

    fn refetch(&mut self, cx: &mut Context<Self>) {
        let window = self.window;
        let me = cx.entity();
        cx.defer(move |cx| {
            _ = window.update(cx, |_, window, cx| me.update(cx, |this, cx| this.fetch(window, cx)));
        });
    }

    /// The scope, or the turn, the shown diff is read for.
    fn read_key(&self) -> String {
        match self.turn() {
            Some(t) => format!("{} {}..{} {:?}", t.cwd, t.from, t.to, t.only),
            None => format!("{} {}", self.folder.as_deref().unwrap_or_default(), self.scope().wire()),
        }
    }

    /// Reads the diff and the folder's branch again; the last diff stays through a refresh of the same read.
    pub fn fetch(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        DaemonLink::redial(&link, false, cx);
        self.seq += 1;
        let seq = self.seq;
        self.load = Load::Pending;
        let turn = self.turn().cloned();
        let scope = self.scope();
        let thread = self.opened.thread_id.clone();
        let (followed, project) = self.facts.as_ref().map(|f| (f.followed.clone(), f.project.clone())).unwrap_or_default();
        cx.notify();
        self._fetch = Some(cx.spawn(async move |this, cx| {
            if link.read_with(cx, |l, _| l.root().is_none()) {
                let dial = link.read_with(cx, |l, _| l.request("ping", json!({})));
                if let Err(refused) = dial.await {
                    _ = this.update(cx, |this, cx| this.link_refused(seq, refused, cx));
                    return;
                }
                for _ in 0..100 {
                    if link.read_with(cx, |l, _| l.root().is_some()) {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_millis(20)).await;
                }
            }
            let folder = link.read_with(cx, |l, _| l.folder(thread.as_deref(), followed.as_deref(), project.as_deref()));
            let cwd = match &turn {
                Some(t) => t.cwd.clone(),
                None => match folder {
                    Some(f) => f,
                    None => {
                        _ = this.update(cx, |this, cx| {
                            if seq == this.seq {
                                this.load = Load::Error { message: String::new(), line: Some(words::READ_ON_COMPUTER) };
                                cx.notify();
                            }
                        });
                        return;
                    }
                },
            };
            let asked = match &turn {
                Some(t) => link.read_with(cx, |l, _| l.request("git.turn", json!({ "cwd": cwd, "from": t.from, "to": t.to }))),
                None => link.read_with(cx, |l, _| l.request("git.diff", json!({ "cwd": cwd, "scope": scope.wire() }))),
            };
            let status = link.read_with(cx, |l, _| l.request("git.status", json!({ "cwd": cwd })));
            let project = project.filter(|p| *p != cwd).map(|p| link.read_with(cx, |l, _| l.request("git.status", json!({ "cwd": p }))));
            let (read, status, project) = futures::join!(asked, status, async move {
                match project {
                    Some(asked) => Some(asked.await),
                    None => None,
                }
            });
            let top = |status: &Map<String, Value>| status.get("root").and_then(Value::as_str).map(str::to_owned);
            let project_root = match project {
                Some(status) => status.ok().as_ref().and_then(top),
                None => status.as_ref().ok().and_then(top),
            };
            // Reading parses every patch, up to the daemon's 2 MiB, so it runs off the UI thread.
            let read = cx
                .background_spawn(async move {
                    read.map_err(|r| (r.code.clone(), r.message)).and_then(|reply| diff::read_reply(&reply).map_err(|e| (None, e)))
                })
                .await;
            let up = link.read_with(cx, |l, _| l.up());
            _ = this.update(cx, |this, cx| {
                if seq == this.seq {
                    this.project_root = project_root;
                }
                this.read(seq, cwd, read, status, up, cx)
            });
        }));
    }

    fn link_refused(&mut self, seq: u64, refused: Refused, cx: &mut Context<Self>) {
        if seq != self.seq {
            return;
        }
        self.not_running = true;
        self.load = Load::Error { message: refused.message, line: None };
        cx.notify();
    }

    fn read(
        &mut self,
        seq: u64,
        cwd: String,
        read: Result<DiffModel, (Option<String>, String)>,
        status: Result<Map<String, Value>, Refused>,
        up: bool,
        cx: &mut Context<Self>,
    ) {
        if seq != self.seq {
            return;
        }
        self.not_running = !up && read.is_err();
        if up {
            self.live_once = true;
            self.restart.update(cx, |r, _| *r = Restart::default());
        }
        let moved = self.folder.as_deref() != Some(cwd.as_str());
        self.folder = Some(cwd);
        if moved {
            self.repo = None;
        }
        let key = self.read_key();
        if self.read_for.as_deref() != Some(key.as_str()) {
            self.read_for = Some(key);
            self.folds = Folds::default();
            self.shut.clear();
            self.reveal_note = None;
            self.commit = None;
            self.stop_edit(cx);
            self.asks += 1;
            self.write_note = None;
            self.diff = None;
            // Comments point at lines of the last read, which a new scope no longer holds.
            self.comments.update(cx, |comments, cx| {
                comments.cancel(cx);
                _ = comments.take_quote(cx);
            });
        }
        match read {
            Ok(mut model) => {
                self.whole = model.files.len();
                if let Some(only) = self.turn().and_then(|t| t.only.clone()) {
                    model.files.retain(|f| only.iter().any(|p| p == f.path.as_ref()));
                }
                self.diff = Some(Rc::new(model));
                self.load = Load::Ready;
                self.reveal_turn_path(cx);
                if !self.shot.is_empty() {
                    self.open_shot(cx);
                }
            }
            Err((code, message)) => {
                let line = match code.as_deref() {
                    Some("not-a-git-repo") => Some(words::NOT_A_REPO),
                    Some("not-found") if self.is_turn() => Some(words::SNAPSHOT_GONE),
                    _ => None,
                };
                if line.is_some() {
                    self.diff = None;
                }
                self.load = Load::Error { message, line };
            }
        }
        self.repo = match status {
            Ok(status) => Some(Repo {
                root: status.get("root").and_then(Value::as_str).unwrap_or_default().to_owned(),
                branch: status.get("branch").and_then(|b| b.get("head")).and_then(Value::as_str).unwrap_or_default().to_owned(),
            }),
            Err(_) => None,
        };
        self.rebuild_tree();
        self.push_files(cx);
        self.take_show(cx);
        cx.notify();
    }

    /// Shows the file the Files pane's Show in Changes asked for, once this workspace's diff is read; the ask is taken
    /// so a later open does not show it again.
    fn take_show(&mut self, cx: &mut Context<Self>) {
        let Some(ask) = cx.try_global::<ShowInChanges>().cloned() else { return };
        if self.diff.is_none() || self.workspace_id() != Some(ask.workspace_id.as_str()) {
            return;
        }
        cx.remove_global::<ShowInChanges>();
        self.show_file(&ask.path, cx);
    }

    /// Opens a comment field under the lines the gutter's + or a drag picked.
    fn begin_comment(&mut self, range: &diff::LineRange, window: &mut Window, cx: &mut Context<Self>) {
        let Some(patch) = self.file(range.path()).and_then(|f| f.patch.clone()) else { return };
        self.comments.update(cx, |comments, cx| comments.begin(range, &patch, window, cx));
    }

    pub fn comments(&self) -> &Entity<LineComments> {
        &self.comments
    }

    /// Send to thread: every kept comment, as one block under the thread's composer draft.
    pub fn send_comments(&mut self, cx: &mut Context<Self>) {
        let key = self.opened.thread_id.clone().or_else(|| self.opened.workspace_id.clone());
        let (Some(key), Some(quote)) = (key, self.comments.update(cx, |c, cx| c.take_quote(cx))) else { return };
        if let Err(e) = send_to_thread(&key, &quote, cx) {
            self.write_note = Some(e.to_string());
        }
        cx.notify();
    }

    /// Opens straight into the states a capture names, in order: `uncommitted`, `split`, `commit`, `discard:<path>`,
    /// `edit:<path>`.
    fn open_shot(&mut self, cx: &mut Context<Self>) {
        let window = self.window;
        let me = cx.entity();
        cx.defer(move |cx| {
            _ = window.update(cx, |_, window, cx| {
                me.update(cx, |this, cx| {
                    while let Some(word) = this.shot.pop_front() {
                        match word.split_once(':') {
                            Some(("discard", path)) => this.ask_discard(path, window, cx),
                            Some(("edit", path)) => this.start_edit(path, window, cx),
                            _ if word == "commit" => this.open_commit(window, cx),
                            _ if word == "split" => this.set_layout(Layout::Split, cx),
                            _ if word == "stacked" => this.set_layout(Layout::Stacked, cx),
                            // A new scope is a new read; the words after it wait for that read.
                            _ if word == "uncommitted" && this.scope() != Scope::Head => return this.set_scope(Scope::Head, cx),
                            _ => {}
                        }
                    }
                })
            });
        });
    }

    /// Hands the code view the files it draws, their folds and the editor in place of the one being edited.
    fn push_files(&mut self, cx: &mut Context<Self>) {
        let files: Vec<FilePatch> = self.diff.as_ref().map(|d| d.files.clone()).unwrap_or_default();
        self.code.update(cx, |view, cx| view.set_files(&files, cx));
        self.push_folds(cx);
    }

    fn push_folds(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        let editing = self.editing.as_ref().map(|e| e.path.clone());
        let folded: Vec<SharedString> = diff
            .files
            .iter()
            .filter(|f| editing.as_deref() != Some(f.path.as_ref()) && self.folds.folded(&f.path, model::is_viewed(f, &self.marks)))
            .map(|f| f.path.clone())
            .collect();
        self.code.update(cx, |view, cx| view.set_folded_paths(folded, cx));
    }

    /// The file a reply's tree was pressed on, named from the checkout's top; one the range lacks is named in a line.
    fn reveal_turn_path(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.turn().and_then(|t| t.path.clone()) else { return };
        if !self.reveal(&path, cx) {
            self.reveal_note = Some(words::no_diff(base_name(&path), words::TURN_NOUN));
        }
    }

    /// Shows `path` in the code view, opened; false where the diff does not touch it.
    pub fn reveal(&mut self, path: &str, cx: &mut Context<Self>) -> bool {
        if self.file(path).is_none() {
            return false;
        }
        self.folds.collapsed.remove(path);
        self.folds.opened.insert(path.to_owned());
        self.push_folds(cx);
        self.code.update(cx, |view, cx| view.scroll_to(path, None, cx));
        true
    }

    /// Another pane asked for `path` (absolute, as the daemon names it): shown, or named in the reveal line.
    pub fn show_file(&mut self, path: &str, cx: &mut Context<Self>) {
        // git.diff names files from the checkout's top, whichever folder git ran in.
        let top = self.repo.as_ref().map(|r| r.root.as_str()).or(self.folder.as_deref());
        let relative = top.map_or(path, |top| model::relative_to(top, path)).to_owned();
        self.reveal_note = if self.reveal(&relative, cx) { None } else { Some(words::no_diff(base_name(path), self.scope().noun())) };
        cx.notify();
    }

    pub fn toggle_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let viewed = self.file(path).is_some_and(|f| model::is_viewed(f, &self.marks));
        self.folds.toggle(path, viewed);
        self.push_folds(cx);
        cx.notify();
    }

    pub fn toggle_all(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        self.folds.toggle_all(diff.files.iter().map(|f| f.path.as_ref()));
        self.push_folds(cx);
        cx.notify();
    }

    pub fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        let Some(workspace) = self.opened.workspace_id.clone() else { return };
        self.asks += 1;
        let thread = self.opened.thread_id.clone();
        self.kept.set_scope(&workspace, thread.as_deref(), scope);
        self.keep(cx);
        self.refetch(cx);
    }

    pub fn set_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        self.kept.layout = layout;
        self.code.update(cx, |view, cx| view.set_layout(layout, cx));
        self.keep(cx);
        cx.notify();
    }

    /// Show all: the same turn with nothing narrowed.
    pub fn show_all(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(turn)) = (self.opened.workspace_id.clone(), self.turn().cloned()) else { return };
        self.kept.open_turn(&workspace, turn.whole());
        self.keep(cx);
        self.refetch(cx);
    }

    fn keep(&mut self, cx: &mut Context<Self>) {
        let kept = self.kept.clone();
        if cx.try_global::<LocalStore>().is_some() {
            if let Err(e) = cx.update_global(|store: &mut LocalStore, _| kept.write(store)) {
                eprintln!("desktop-gpui: changes: keeping the scope: {e}");
            }
        }
    }

    pub fn toggle_pin(&mut self, cx: &mut Context<Self>) {
        let (Some(link), Some(thread)) = (self.link.clone(), self.opened.thread_id.clone()) else { return };
        let folder = self.folder.clone();
        link.update(cx, |link, cx| match (link.pinned(&thread).is_some(), folder) {
            (true, _) => link.unpin(&thread, cx),
            (false, Some(folder)) => link.pin(&thread, &folder, cx),
            (false, None) => {}
        });
    }

    /// The roots the crumbs and the up keys walk: the daemon's home, then the project's folder.
    fn roots(&self, cx: &App) -> Vec<String> {
        let home = self.link.as_ref().and_then(|l| l.read(cx).root());
        roots(home.as_deref(), self.facts.as_ref().and_then(|f| f.project.as_deref()))
    }

    /// A crumb pins the thread's panes to its folder, as the Files pane's do.
    pub fn go(&mut self, folder: &str, cx: &mut Context<Self>) {
        let (Some(link), Some(thread)) = (self.link.clone(), self.opened.thread_id.clone()) else { return };
        link.update(cx, |link, cx| link.pin(&thread, folder, cx));
    }

    /// Backspace or Alt+Up on the pane itself, not in a field: the folder above, inert at a root's edge.
    fn up_key(&mut self, event: &gpui_kit::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (k, m) = (&event.keystroke, &event.keystroke.modifiers);
        let up = !(m.control || m.platform || m.shift) && ((k.key == "backspace" && !m.alt) || (k.key == "up" && m.alt));
        if !up || !self.focus.is_focused(window) || self.turn().is_some() {
            return;
        }
        let Some(above) = self.folder.as_deref().and_then(|folder| parent_within(&self.roots(cx), folder)) else { return };
        cx.stop_propagation();
        self.go(&above, cx);
    }

    fn pinned(&self, cx: &App) -> bool {
        match (&self.link, self.thread_id()) {
            (Some(link), Some(thread)) => link.read(cx).pinned(thread).is_some(),
            _ => false,
        }
    }

    /// Marks `path` viewed at its blob, or clears the mark; marking folds it, an unmark leaves it as it shows.
    pub fn toggle_viewed(&mut self, path: &str, cx: &mut Context<Self>) {
        let (Some(workspace), Some(file)) = (self.opened.workspace_id.clone(), self.file(path)) else { return };
        let Some(blob) = file.blob.clone() else { return };
        let on = self.marks.get(path) == Some(&blob);
        if !on {
            self.folds.marked(path);
        }
        let path = path.to_owned();
        let asked =
            self.host.run(move |client| async move { host::mark(&client, &workspace, &path, (!on).then_some(blob.as_str())).await });
        cx.spawn(async move |this, cx| {
            let marked = asked.await;
            _ = this.update(cx, |this, cx| {
                match marked {
                    Ok(marks) => {
                        this.marks = marks;
                        this.rebuild_tree();
                    }
                    Err(f) => this.write_note = Some(f.message),
                }
                this.push_folds(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks before discarding `path`: the dialog opens in the window's dialog layer.
    pub fn ask_discard(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.discarding.is_some() || self.elsewhere() {
            return;
        }
        let deletes = self.file(path).is_some_and(|f| model::discard_deletes(f, self.scope()));
        self.discarding = Some(DiscardState::new(path.to_owned(), deletes));
        DiscardDialog::open(&cx.entity(), window, cx);
        cx.notify();
    }

    /// Cancel: closes the dialog, unless the discard is on its way.
    pub fn close_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.discarding.as_ref().is_none_or(|d| d.busy) {
            return;
        }
        self.discarding = None;
        crate::screens::dialogs::layer::close(discard::KEY, window, cx);
        cx.notify();
    }

    /// The dialog layer closed the dialog on its own (Escape).
    pub fn discard_closed(&mut self, cx: &mut Context<Self>) {
        self.discarding = None;
        cx.notify();
    }

    /// Sends the discard the dialog asked for: it closes once the host puts the file back, or keeps the host's
    /// sentence in the note's place.
    pub fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace), Some(state)) = (self.opened.workspace_id.clone(), self.discarding.as_mut()) else { return };
        if state.busy {
            return;
        }
        state.busy = true;
        state.refusal = None;
        let path = state.path.clone();
        let asked_for = path.clone();
        let asked = self.host.run(move |client| async move { host::discard(&client, &workspace, &asked_for).await });
        crate::screens::dialogs::layer::hold(discard::KEY, true, cx);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let done = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                // A reply for another file's dialog is not this one's.
                let Some(state) = this.discarding.as_mut().filter(|d| d.path == path) else { return };
                state.busy = false;
                crate::screens::dialogs::layer::hold(discard::KEY, false, cx);
                match done {
                    Ok(()) => {
                        this.close_discard(window, cx);
                        this.fetch(window, cx);
                    }
                    Err(f) => state.refusal = Some(f.message),
                }
                window.refresh();
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the commit box with every listed file ticked, and asks the workspace's agent for a message.
    pub fn open_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace), Some(diff)) = (self.opened.workspace_id.clone(), self.diff.clone()) else { return };
        if self.elsewhere() {
            return;
        }
        let paths: Vec<String> = diff.files.iter().map(|f| f.path.to_string()).collect();
        self.commit = Some(CommitState::new(&paths, window, cx));
        self.draft_ask += 1;
        let asked_at = (self.asks, self.draft_ask);
        let thread = self.opened.thread_id.clone();
        let asked = self.host.run(move |client| async move { host::commit_draft(&client, &workspace, &paths, thread.as_deref()).await });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let draft = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                if (this.asks, this.draft_ask) != asked_at {
                    return;
                }
                let Some(state) = this.commit.as_mut() else { return };
                state.drafting = false;
                match draft {
                    Ok((Some(message), _)) => {
                        if state.message(cx).is_empty() {
                            state.message.update(cx, |field, cx| field.set_value(message, window, cx));
                        }
                    }
                    Ok((None, note)) => state.note = Some(note.unwrap_or_else(|| words::NO_DRAFT.to_owned())),
                    Err(f) => state.note = Some(f.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn tick(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(state) = self.commit.as_mut() {
            if !state.ticked.remove(path) {
                state.ticked.insert(path.to_owned());
            }
        }
        cx.notify();
    }

    pub fn cancel_commit(&mut self, cx: &mut Context<Self>) {
        self.commit = None;
        cx.notify();
    }

    /// Commits the ticked files with the message; a toast says so and the diff is read again, or the host's sentence
    /// stays in the box.
    pub fn make_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace), Some(diff)) = (self.opened.workspace_id.clone(), self.diff.clone()) else { return };
        let Some(state) = self.commit.as_mut() else { return };
        if state.held(cx) {
            return;
        }
        let message = state.message(cx);
        let paths: Vec<String> = diff.files.iter().map(|f| f.path.to_string()).filter(|p| state.ticked.contains(p)).collect();
        state.busy = true;
        state.error = None;
        let asked = self.host.run(move |client| async move { host::commit(&client, &workspace, &message, &paths).await });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let made = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                match made {
                    Ok(subject) => {
                        this.commit = None;
                        crate::ui::notify(cx, crate::ui::Notice::done(words::committed(&subject)));
                        this.fetch(window, cx);
                    }
                    Err(f) => {
                        if let Some(state) = this.commit.as_mut() {
                            state.busy = false;
                            state.error = Some(f.message);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens `path` in the in-place editor over its whole file, read from a whole-file diff; a file holding a carriage
    /// return is left alone with the note saying why.
    pub fn start_edit(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(link), Some(folder)) = (self.link.clone(), self.folder.clone()) else { return };
        let Some(language) = self.file(path).map(FilePatch::language) else { return };
        self.write_note = None;
        self.edit_ask += 1;
        let asked_at = (self.asks, self.edit_ask);
        let asked =
            link.read(cx).request("git.diff", json!({ "cwd": folder, "scope": self.scope().wire(), "paths": [path], "whole": true }));
        let path = path.to_owned();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let whole = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                if (this.asks, this.edit_ask) != asked_at || this.editing.is_some() {
                    return;
                }
                match whole.map_err(|r| r.message).and_then(|reply| whole_text(&reply, &path)) {
                    Ok(Some(text)) => {
                        let loaded = text.clone();
                        let editor =
                            cx.new(|cx| EditorState::new(window, cx).language(language.clone()).line_number(true).default_value(text));
                        this.editing = Some(Editing { path: path.clone(), editor, loaded, saving: false });
                        this.push_body(cx);
                        this.push_folds(cx);
                    }
                    Ok(None) => this.write_note = Some(words::LINE_ENDINGS.to_owned()),
                    Err(message) => this.write_note = Some(message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn push_body(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = &self.editing else { return };
        let editor = editing.editor.clone();
        let path = editing.path.clone();
        // Today's edit stays in the file's own rows: no box, one 20 px code row per line in the code view's 13 px mono,
        // and no cap, as the pane scrolls.
        let body: diff::BodyRenderer = Rc::new(move |_, _, cx| {
            let tall = px(20.) * editor.read(cx).text().lines_len() as f32;
            let field = Editor::new(&editor).xsmall().appearance(false).text_size(px(13.)).line_height(px(20.)).h(tall);
            div().debug_selector(|| "changes-editor".into()).w_full().h(tall).child(field).into_any_element()
        });
        self.code.update(cx, |view, cx| view.set_body(&path, Some(body), cx));
    }

    /// The open editor's text, for a test or a capture.
    pub fn editor_text(&self, cx: &App) -> Option<String> {
        self.editing.as_ref().map(|e| e.editor.read(cx).value().to_string())
    }

    /// The open editor's row as laid out, for a test.
    pub fn editor_row(&self, cx: &App) -> Option<Pixels> {
        self.editing.as_ref().and_then(|e| e.editor.read(cx).line_height())
    }

    pub fn set_editor_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editing) = &self.editing {
            let text = text.to_owned();
            editing.editor.update(cx, |editor, cx| editor.set_value(text, window, cx));
        }
    }

    pub fn stop_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(editing) = self.editing.take() {
            self.code.update(cx, |view, cx| view.set_body(&editing.path, None, cx));
            self.push_folds(cx);
        }
        cx.notify();
    }

    /// Writes the editor's text over the file whole, then reads the diff again.
    pub fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(link), Some(root)) = (self.link.clone(), self.repo.as_ref().map(|r| r.root.clone())) else { return };
        let Some(editing) = self.editing.as_mut() else { return };
        if editing.saving {
            return;
        }
        let contents = editing.editor.read(cx).value().to_string();
        if contents == editing.loaded {
            self.stop_edit(cx);
            return;
        }
        editing.saving = true;
        let asked = link.read(cx).request("fs.write", json!({ "path": format!("{root}/{}", editing.path), "contents": contents }));
        let workspace = self.opened.workspace_id.clone();
        let host = self.host.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let written = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                match written {
                    Ok(_) => {
                        this.stop_edit(cx);
                        this.fetch(window, cx);
                        if let Some(workspace) = workspace {
                            let checked = host.run(move |client| async move { host::checkout(&client, &workspace).await });
                            cx.background_spawn(async move { _ = checked.await }).detach();
                        }
                    }
                    Err(r) => {
                        if let Some(editing) = this.editing.as_mut() {
                            editing.saving = false;
                        }
                        this.write_note = Some(r.message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn rebuild_tree(&mut self) {
        let files: Vec<TreeFile> = self
            .diff
            .as_ref()
            .map(|d| {
                d.files.iter().map(|f| TreeFile { path: f.path.to_string(), additions: f.additions, deletions: f.deletions }).collect()
            })
            .unwrap_or_default();
        let viewed: HashMap<String, Option<bool>> = self
            .diff
            .as_ref()
            .map(|d| d.files.iter().map(|f| (f.path.to_string(), f.blob.as_ref().map(|_| model::is_viewed(f, &self.marks)))).collect())
            .unwrap_or_default();
        self.tree = TreeCache {
            rows: Rc::new(model::tree_rows(&files, &self.shut)),
            files: Rc::new(files),
            shut: Rc::new(self.shut.clone()),
            any_blob: viewed.values().any(Option::is_some),
            viewed_count: viewed.values().filter(|v| **v == Some(true)).count(),
            viewed: Rc::new(viewed),
        };
    }

    pub fn toggle_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.shut.remove(path) {
            self.shut.insert(path.to_owned());
        }
        self.rebuild_tree();
        cx.notify();
    }

    pub fn toggle_tree(&mut self, cx: &mut Context<Self>) {
        self.tree_open = !self.tree_open;
        cx.notify();
    }

    pub fn viewed_count(&self) -> usize {
        self.tree.viewed_count
    }
}

/// Keeps `scope` for the workspace, leaving the turn `thread_id` had open, and opens the Changes pane: a pane already
/// open hears it through the local store. The PR Files tab's Open in Changes and the git button's Commit take this.
pub fn open_on(workspace_id: &str, thread_id: Option<&str>, scope: Scope, cx: &mut App) {
    if cx.try_global::<LocalStore>().is_some() {
        let kept = cx.update_global(|store: &mut LocalStore, _| {
            let mut kept = Kept::read(store);
            kept.set_scope(workspace_id, thread_id, scope);
            kept.write(store)
        });
        if let Err(e) = kept {
            eprintln!("desktop-gpui: changes: keeping the scope: {e}");
        }
    }
    crate::screens::navigate(crate::screens::Route::Screen(ChangesScreen::NAME.into()), cx);
}

/// The new side of a whole-file patch as text: its context and added lines. None where the file carries a carriage
/// return, which the editor is not trusted with yet.
fn whole_text(reply: &Map<String, Value>, path: &str) -> Result<Option<String>, String> {
    // A whole file past the daemon's cut would be saved short, so it is not opened.
    if reply.get("truncated") == Some(&Value::Bool(true)) {
        return Err(diff::CUT_NOTE.to_owned());
    }
    let files = reply.get("files").and_then(Value::as_array).ok_or("git.diff answered no files")?;
    let file = files.iter().find(|f| f.get("path").and_then(Value::as_str) == Some(path)).ok_or("git.diff did not list the file")?;
    let patch = file.get("patch").and_then(Value::as_str).unwrap_or_default();
    if patch.contains('\r') {
        return Ok(None);
    }
    let mut text = String::new();
    let mut in_hunk = false;
    let mut last_newline = true;
    for line in patch.split_inclusive('\n') {
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with("\\ No newline at end of file") {
            if last_newline && text.ends_with('\n') {
                text.pop();
            }
            continue;
        }
        if let Some(kept) = line.strip_prefix(' ').or_else(|| line.strip_prefix('+')) {
            text.push_str(kept);
            last_newline = true;
        } else {
            last_newline = false;
        }
    }
    // A patch with no hunk carries no text (a mode change, a rename alone); only a new empty file is truly empty.
    if !in_hunk && !patch.lines().any(|l| l.starts_with("new file mode")) {
        return Err(words::NO_TEXT.to_owned());
    }
    Ok(Some(text))
}

/// A file head's counts as pierre writes them: deletions first in red, then additions in green, each left out at
/// zero unless both are.
pub fn head_counts(additions: usize, deletions: usize, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex()
        .debug_selector(|| "head-counts".into())
        .flex_none()
        .gap(px(4.))
        .mr(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .children(head_count_words(additions, deletions).into_iter().map(|word| {
            let ink = if word.starts_with('-') { p.error_foreground } else { p.success };
            let tag = format!("head-count:{word}");
            div().debug_selector(move || tag.clone()).text_color(ink).child(word)
        }))
}

/// A diff head's file title as today's `[data-title]` draws it: under the pointer its ink takes 16% of the primary and
/// its underline comes in, both over 120 ms in CSS's `ease`. GPUI draws an underline at its own offset, not 2 px.
pub fn file_title(path: &str, el: gpui_kit::Div, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let id = SharedString::from(format!("file-title:{path}"));
    let hovered = crate::ui::motion::hovered(id.clone(), window, cx);
    let over = *hovered.read(cx);
    let target = if over { p.foreground.blend(p.primary.opacity(0.16)) } else { p.foreground };
    let ease = gpui_kit::base::Easing::cubic_bezier(0.25, 0.1, 0.25, 1.).expect("CSS's ease is a valid curve");
    let timing = gpui_kit::base::Transition::new(Duration::from_millis(120)).easing(ease);
    let ink = crate::ui::motion::ease_ink_by(id.clone(), "ink", target, timing.clone(), window, cx);
    let line = crate::ui::motion::ease_ink_by(id.clone(), "underline", if over { target } else { target.opacity(0.) }, timing, window, cx);
    let tag = id.clone();
    el.id(id)
        .debug_selector(move || format!("{tag}:{}", hex(ink)))
        .cursor_pointer()
        .text_color(ink)
        .text_decoration_1()
        .text_decoration_color(line)
        .on_hover(crate::ui::motion::track_hover(hovered))
}

/// `#rrggbb`, for a selector a test reads a drawn ink by.
pub fn hex(ink: gpui_kit::Hsla) -> String {
    let c = ink.to_rgb();
    let byte = |x: f32| (x * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// The words a file head's counts say, in their order.
pub fn head_count_words(additions: usize, deletions: usize) -> Vec<String> {
    let mut words = Vec::new();
    if deletions > 0 || additions == 0 {
        words.push(format!("-{deletions}"));
    }
    if additions > 0 || deletions == 0 {
        words.push(format!("+{additions}"));
    }
    words
}

impl Focusable for ChangesScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ChangesScreen {
    /// One file's head row inside the code view's 32 px frame: its chevron and viewed tick, its path and counts, then
    /// its edit or save and stop, and its discard.
    fn head_row(screen: &Entity<ChangesScreen>, head: &FileHead, window: &mut Window, cx: &mut App) -> AnyElement {
        let this = screen.read(cx);
        let p = cx.palette().clone();
        let path = head.path.to_string();
        let name = base_name(&path).to_owned();
        let file = this.file(&path);
        let blob = file.and_then(|f| f.blob.clone());
        let viewed = file.is_some_and(|f| model::is_viewed(f, &this.marks));
        let turn = this.is_turn();
        let scope = this.scope();
        let editing = this.editing.as_ref().filter(|e| e.path == path).map(|e| e.saving);
        let can_edit = this.editing.is_none() && this.repo.is_some() && file.is_some_and(|f| model::editable(f, scope, turn));
        let can_discard = model::discardable(scope, turn) && !this.elsewhere();
        let weak = screen.downgrade();
        let act =
            |weak: &WeakEntity<ChangesScreen>, f: fn(&mut ChangesScreen, &str, &mut Window, &mut Context<ChangesScreen>), path: &str| {
                let (weak, path) = (weak.clone(), path.to_owned());
                move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                    _ = weak.update(cx, |this, cx| f(this, &path, window, cx));
                }
            };
        let mut controls: Vec<(String, AnyElement)> = Vec::new();
        match editing {
            Some(saving) => {
                controls.push((
                    format!("save:{path}"),
                    Button::icon_xs(SharedString::from(format!("save:{path}")), Glyph::Check)
                        .side(px(20.))
                        .held(saving)
                        .tooltip(words::save(&name))
                        .on_click(act(&weak, |this, _, window, cx| this.save_edit(window, cx), &path))
                        .into_any_element(),
                ));
                controls.push((
                    format!("stop:{path}"),
                    Button::icon_xs(SharedString::from(format!("stop:{path}")), Glyph::X)
                        .side(px(20.))
                        .held(saving)
                        .tooltip(words::stop_editing(&name))
                        .on_click(act(&weak, |this, _, _, cx| this.stop_edit(cx), &path))
                        .into_any_element(),
                ));
            }
            None if can_edit => controls.push((
                format!("edit:{path}"),
                Button::icon_xs(SharedString::from(format!("edit:{path}")), Glyph::Pencil)
                    .side(px(20.))
                    .tooltip(words::edit(&name))
                    .on_click(act(&weak, |this, path, window, cx| this.start_edit(path, window, cx), &path))
                    .into_any_element(),
            )),
            None => {}
        }
        if can_discard {
            controls.push((
                format!("discard:{path}"),
                Button::icon_xs(SharedString::from(format!("discard:{path}")), Glyph::Trash)
                    .side(px(20.))
                    .tooltip(words::discard_control(&name))
                    .on_click(act(&weak, |this, path, window, cx| this.ask_discard(path, window, cx), &path))
                    .into_any_element(),
            ));
        }
        let controls: Vec<AnyElement> = controls.into_iter().map(|(tag, button)| tagged(tag, button)).collect();
        let renamed_from = head.old_path.clone().filter(|_| head.status == FileStatus::Renamed);
        // Pierre's head marks: its own glyphs in its addition, deletion and modified bases, which the code view's
        // colors read off the same highlight status (diff/look.rs).
        let status = &cx.theme().highlight_theme.style.status;
        let (mark, ink) = match head.status {
            FileStatus::Added => (crate::ui::icons::DIFF_ADDED, status.success(cx)),
            FileStatus::Deleted => (crate::ui::icons::DIFF_DELETED, status.error(cx)),
            FileStatus::Renamed => (crate::ui::icons::DIFF_MOVED, status.info(cx)),
            FileStatus::Modified | FileStatus::Binary => (crate::ui::icons::DIFF_MODIFIED, status.info(cx)),
        };
        let mark = div()
            .debug_selector({
                let tag = format!("head-mark:{path}");
                move || tag.clone()
            })
            .flex_none()
            .ml(px(4.))
            .child(Icon::empty().path(mark).with_size(px(16.)).text_color(ink));
        let sans = cx.theme().font_family.clone();
        // Pierre's `[data-header-content]`: names cut from the start (`direction: rtl`), 8 px apart, the old one at 70%.
        let name_el = h_flex()
            .flex_1()
            .min_w_0()
            .ml(px(4.))
            .gap(px(8.))
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(sans)
            .text_size(px(13.))
            .children(
                renamed_from
                    .map(|old| div().min_w_0().overflow_hidden().text_ellipsis_start().text_color(p.foreground.opacity(0.7)).child(old)),
            )
            .when(head.status == FileStatus::Renamed && head.old_path.is_some(), |el| {
                el.child(Icon::empty().path(crate::ui::icons::DIFF_RENAMED_TO).with_size(px(16.)).flex_none().text_color(p.foreground))
            })
            .child(file_title(&path, div().min_w_0().overflow_hidden().text_ellipsis_start().child(path.clone()), window, cx));
        let chevron = div()
            .id(SharedString::from(format!("fold:{path}")))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(20.))
            .ml(px(-2.))
            .rounded(px(4.))
            .hover(|s| s.bg(p.foreground.opacity(0.1)))
            .cursor_pointer()
            .tooltip(tip(if head.folded { words::EXPAND_FILE } else { words::COLLAPSE_FILE }))
            .on_click(act(&weak, |this, path, _, cx| this.toggle_file(path, cx), &path))
            .child(diff::file_chevron(head, cx));
        let tick = blob.map(|_| {
            div()
                .id(SharedString::from(format!("viewed:{path}")))
                .debug_selector({
                    let tag = format!("viewed-tick:{path}");
                    move || tag.clone()
                })
                .ml(px(4.))
                .cursor_pointer()
                .tooltip(tip(words::viewed_tick(&name)))
                .on_click(act(&weak, |this, path, _, cx| this.toggle_viewed(path, cx), &path))
                .child(checkbox(SharedString::from(format!("viewed-box:{path}")), viewed, false, window, cx))
        });
        h_flex()
            .w_full()
            .h_full()
            .gap(px(4.))
            .child(chevron)
            .children(tick)
            .child(mark)
            .child(name_el)
            .child(head_counts(head.additions, head.deletions, cx))
            .child(h_flex().flex_none().gap(px(4.)).children(controls))
            .into_any_element()
    }

    fn head(&mut self, inline: bool, narrow: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let turn = self.turn().cloned();
        let scope = self.scope();
        let scope_label: SharedString = if turn.is_some() { words::TURN_SCOPE.into() } else { scope.label().into() };
        let me = cx.weak_entity();
        let acts: Vec<MenuAct> = Scope::ALL
            .iter()
            .map(|&s| {
                let me = me.clone();
                MenuAct {
                    group: 0,
                    glyph: None,
                    title: s.label().into(),
                    destructive: false,
                    held: false,
                    why: None,
                    keys: None,
                    run: Rc::new(move |_, cx| _ = me.update(cx, |this, cx| this.set_scope(s, cx))),
                }
            })
            .collect();
        let trigger = h_flex()
            .id("changes-scope")
            .debug_selector(|| "changes-scope".into())
            .h(px(24.))
            .flex_none()
            .gap(px(4.))
            .px(px(8.))
            .rounded(px(8.))
            .bg(p.accent)
            .hover(|s| s.bg(p.accent.opacity(0.8)))
            .cursor_pointer()
            .text_size(px(12.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(p.foreground)
            .child(scope_label)
            .child(Icon::new(Glyph::ChevronDown).with_size(px(14.)).text_color(p.foreground.opacity(0.7)));
        let scope_menu = menu_on("changes-scope", trigger, gpui_kit::Anchor::TopLeft, Rc::new(acts));
        let mono = cx.theme().mono_font_family.clone();
        let commenting = !self.comments.read(cx).kept().is_empty();
        let crumbs = (!narrow && !commenting).then(|| self.folder.clone()).flatten().map(|folder| {
            let me = cx.weak_entity();
            let pick: Pick = Rc::new(move |folder, _, cx| _ = me.update(cx, |this, cx| this.go(folder, cx)));
            let row = CrumbRow::new("changes-folder-crumbs", self.roots(cx), folder, pick);
            // `size-full flex-initial shrink-[999]`: a basis of the whole row, so the path takes what is left and the
            // branch gives the sliver that cuts it, as Chromium cuts "main" to "ma…".
            div().debug_selector(|| "changes-crumbs".into()).flex().w_full().h(px(24.)).min_w_0().flex_grow(0.).flex_shrink(999.).child(row)
        });
        let branch = (!narrow).then(|| self.repo.clone()).flatten().filter(|r| !r.branch.is_empty()).map(|repo| {
            h_flex()
                .id("changes-branch")
                .debug_selector(|| "changes-branch".into())
                .h(px(24.))
                .min_w_0()
                .gap(px(4.))
                .px(px(4.))
                .font_family(mono.clone())
                .text_size(px(11.))
                .text_color(p.muted_foreground)
                .tooltip(tip(format!("git: {}", repo.root)))
                .child(Icon::new(Glyph::FolderGit).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                .child(div().min_w_0().max_w(px(160.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(repo.branch))
        });
        let pinned = self.pinned(cx);
        let pin = (!narrow && self.thread_id().is_some()).then(|| {
            div().debug_selector(|| "changes-pin".into()).flex_none().child(
                Button::icon_xs("changes-pin", if pinned { Glyph::PinOff } else { Glyph::Pin })
                    .side(px(20.))
                    .tooltip(if pinned { words::UNPIN } else { words::PIN })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_pin(cx))),
            )
        });
        let narrowed = turn.as_ref().filter(|t| t.only.is_some()).zip(self.diff.as_ref()).map(|(_, diff)| {
            h_flex()
                .flex_none()
                .gap(px(8.))
                .text_size(px(12.))
                .text_color(p.muted_foreground)
                .child(words::narrowed(diff.files.len(), self.whole))
                .child(Button::text("changes-show-all", words::SHOW_ALL).on_click(cx.listener(|this, _, _, cx| this.show_all(cx))))
        });
        let base =
            (turn.is_none() && scope == Scope::Branch).then(|| self.diff.as_ref().and_then(|d| d.base.clone())).flatten().map(|base| {
                h_flex()
                    .debug_selector(|| "changes-base".into())
                    .min_w_0()
                    .gap(px(8.))
                    .overflow_hidden()
                    .text_size(px(12.))
                    .text_color(p.muted_foreground)
                    .child(div().flex_none().child("HEAD"))
                    .child(Icon::new(Glyph::ArrowRight).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                    .child(div().min_w_0().max_w(px(192.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(base))
            });
        let files = self.diff.as_ref().map_or(0, |d| d.files.len());
        let send = commenting.then(|| {
            div().debug_selector(|| "changes-send".into()).child(
                Button::ghost("changes-send", words::SEND_TO_THREAD).xs().on_click(cx.listener(|this, _, _, cx| this.send_comments(cx))),
            )
        });
        let stat = self.diff.as_ref().filter(|d| !d.files.is_empty()).map(|d| {
            div().debug_selector(|| "changes-stat".into()).mr(px(4.)).child(tree::stat_inline(d.additions(), d.deletions(), px(11.), cx))
        });
        let commit = (model::committable(scope, turn.is_some(), files) && !self.elsewhere()).then(|| {
            div().debug_selector(|| "changes-commit".into()).child(
                Button::outline("changes-commit", words::COMMIT)
                    .xs()
                    .held(self.commit.is_some())
                    .on_click(cx.listener(|this, _, window, cx| this.open_commit(window, cx))),
            )
        });
        let pending = self.load == Load::Pending;
        let refresh = Button::icon("changes-refresh", Glyph::RefreshCw)
            .busy(pending)
            .tooltip(if pending { words::REFRESHING } else { words::REFRESH })
            .on_click(cx.listener(|this, _, window, cx| this.fetch(window, cx)));
        let all_collapsed = self.diff.as_ref().is_some_and(|d| self.folds.all_collapsed(d.files.iter().map(|f| f.path.as_ref())));
        let fold_all = (!narrow && files > 0).then(|| {
            Button::icon("changes-fold-all", if all_collapsed { Glyph::ChevronsUpDown } else { Glyph::ChevronsDownUp })
                .tooltip(if all_collapsed { words::EXPAND_ALL } else { words::COLLAPSE_ALL })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx)))
        });
        let layout = self.kept.layout;
        let view = (!narrow).then(|| {
            let toggle = |id: &'static str, glyph: Glyph, picked: Layout| {
                let fill = (layout == picked).then_some(p.accent);
                let drawn = fill.map_or_else(|| "none".to_owned(), hex);
                div()
                    .id(id)
                    .debug_selector(move || format!("{id}:{drawn}"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(gpui_kit::transparent_black())
                    .when_some(fill, |el, fill| el.bg(fill))
                    .hover(|s| s.bg(p.accent))
                    .cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![crate::ui::focus::ring(p.ring)]))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_layout(picked, cx)))
                    .child(Icon::new(glyph).with_size(px(14.)).text_color(p.foreground.opacity(0.8)))
            };
            h_flex().flex_none().gap(px(4.)).child(toggle("changes-stacked", Glyph::Rows3, Layout::Stacked)).child(toggle(
                "changes-split",
                Glyph::Columns2,
                Layout::Split,
            ))
        });
        let _ = window;
        h_flex()
            .debug_selector(|| "changes-head".into())
            .flex_none()
            .h(if inline { px(28.) } else { px(40.) })
            .when(inline, |el| el.mb(px(12.)))
            .when(!inline, |el| el.border_b_1().border_color(p.border.opacity(0.6)))
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .bg(p.background)
            .child(
                h_flex()
                    .min_w_0()
                    .flex_1()
                    .gap(px(8.))
                    .child(scope_menu)
                    .children(crumbs)
                    .children(branch)
                    .children(pin)
                    .children(narrowed)
                    .children(base),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(4.))
                    .children(send)
                    .children(stat)
                    .children(commit)
                    .child(refresh)
                    .children(fold_all)
                    .children(view),
            )
    }

    fn note(
        text: impl Into<SharedString>,
        tag: &'static str,
        ink: gpui_kit::Hsla,
        fill: Option<gpui_kit::Hsla>,
        mono: bool,
        cx: &App,
    ) -> impl IntoElement {
        let p = cx.palette();
        div()
            .debug_selector(move || tag.into())
            .flex_none()
            .px(px(12.))
            .py(px(6.))
            .border_b_1()
            .border_color(p.border.opacity(0.7))
            .when_some(fill, |el, fill| el.bg(fill))
            .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
            .text_size(px(11.))
            // `text-[11px]` sets no leading, so the page's 1.5 holds.
            .line_height(gpui_kit::relative(1.5))
            .text_color(ink)
            .child(text.into())
    }

    fn pane_line(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
        div()
            .debug_selector(|| "changes-line".into())
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px(px(20.))
            .text_center()
            .text_size(px(13.))
            .text_color(cx.palette().muted_foreground)
            .child(text.into())
    }

    fn changed_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let files = self.tree.files.clone();
        let count = files.len();
        let any_blob = self.tree.any_blob;
        let open = self.tree_open;
        let me = cx.weak_entity();
        let toggle = h_flex()
            .id("changed-files-fold")
            .debug_selector(|| "changed-files-fold".into())
            .w_full()
            .gap(px(6.))
            .px(px(12.))
            .py(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(p.accent.opacity(0.6)))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(p.foreground)
            .on_click(cx.listener(|this, _, _, cx| this.toggle_tree(cx)))
            .child(diff::fold_chevron("changed-files-chevron", open, window, cx))
            .child(div().child(words::changed_files(count)))
            .when(any_blob, |el| {
                el.child(
                    div()
                        .debug_selector(|| "viewed-count".into())
                        .ml(px(12.))
                        .font_weight(gpui_kit::FontWeight::NORMAL)
                        .text_color(p.muted_foreground)
                        .child(words::viewed_count(self.viewed_count(), count)),
                )
            });
        let tree = open.then(|| {
            let rows = self.tree.rows.clone();
            let scope = self.scope();
            let can_discard = model::discardable(scope, self.is_turn()) && !self.elsewhere();
            let committing = self.commit.as_ref().map(|c| c.ticked.clone());
            let viewed = self.tree.viewed.clone();
            let any_tick = any_blob;
            let controls_width = px(if can_discard { 24. } else { 0. } + if any_tick { 20. } else { 0. } + 8.);
            let (on_folder, on_file, lead_me, control_me) = (me.clone(), me.clone(), me.clone(), me.clone());
            let width = self.width - px(8.);
            ChangedFilesTree::new("changes-tree", rows, self.tree.shut.clone(), &files, width)
                .scroll(self.tree_scroll.clone())
                .on_folder(move |path, _, cx| _ = on_folder.update(cx, |this, cx| this.toggle_folder(path, cx)))
                .on_file(move |path, _, cx| {
                    _ = on_file.update(cx, |this, cx| {
                        this.reveal(path, cx);
                    })
                })
                .when_some(committing, |tree, ticked| {
                    tree.lead(move |path, window, cx| {
                        let me = lead_me.clone();
                        let path_owned = path.to_owned();
                        Some(
                            div()
                                .id(SharedString::from(format!("commit-tick:{path}")))
                                .debug_selector({
                                    let tag = format!("commit-tick:{path}");
                                    move || tag.clone()
                                })
                                .flex_none()
                                .cursor_pointer()
                                .tooltip(tip(words::commit_tick(base_name(path))))
                                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.tick(&path_owned, cx)))
                                .child(checkbox(SharedString::from(format!("commit-box:{path}")), ticked.contains(path), false, window, cx))
                                .into_any_element(),
                        )
                    })
                })
                .controls(controls_width, move |path, window, cx| {
                    let name = base_name(path).to_owned();
                    let tick = viewed.get(path).copied().flatten();
                    let mut row = h_flex().gap(px(4.));
                    if can_discard {
                        let me = control_me.clone();
                        let path = path.to_owned();
                        row = row.child(tagged(
                            format!("tree-discard:{path}"),
                            Button::icon_xs(SharedString::from(format!("tree-discard:{path}")), Glyph::Trash)
                                .side(px(20.))
                                .tooltip(words::discard_control(&name))
                                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.ask_discard(&path, window, cx))),
                        ));
                    }
                    if let Some(viewed) = tick {
                        let me = control_me.clone();
                        let path = path.to_owned();
                        let mark = checkbox(SharedString::from(format!("tree-viewed-box:{path}")), viewed, false, window, cx);
                        row = row.child(
                            div()
                                .id(SharedString::from(format!("tree-viewed:{path}")))
                                .ml(px(4.))
                                .cursor_pointer()
                                .tooltip(tip(words::viewed_tick(&name)))
                                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_viewed(&path, cx)))
                                .child(mark),
                        );
                    }
                    Some(row.into_any_element())
                })
        });
        v_flex()
            .debug_selector(|| "changed-files".into())
            .flex_none()
            .border_b_1()
            .border_color(p.border.opacity(0.6))
            .child(toggle)
            .children(tree)
    }
}

impl Render for ChangesScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let viewport = window.viewport_size().width;
        let (narrow, inline) = (viewport < NARROW, viewport > SHEET_MAX);
        let me = cx.weak_entity();
        let measure = canvas(
            move |bounds, _, cx| {
                _ = me.update(cx, |this, cx| {
                    if this.width != bounds.size.width {
                        this.width = bounds.size.width;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let frame = v_flex()
            .id("changes")
            .debug_selector(|| "changes".into())
            .key_context("ChangesPane")
            .on_key_down(cx.listener(Self::up_key))
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .min_w_0()
            .bg(p.background)
            .focus_visible(|s| {
                s.shadow(vec![BoxShadow {
                    color: p.border,
                    offset: gpui_kit::point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(1.),
                    inset: true,
                }])
            })
            .child(measure);
        if self.opened.workspace_id.is_none() {
            return frame.child(ChangesScreen::pane_line(words::PICK_A_PROJECT, cx)).into_any_element();
        }
        if let Some(form) = self.not_running_form() {
            let (host, workspace, restart) =
                (self.host.clone(), self.opened.workspace_id.clone().unwrap_or_default(), self.restart.clone());
            let down = move |said: &str, start: &str| {
                DaemonDown::new(host, workspace, said.to_owned(), Some(start.to_owned().into())).state(restart).into_any_element()
            };
            return frame.child(not_running::view(&form, down, window, cx)).into_any_element();
        }
        let head = self.head(inline, narrow, window, cx);
        let me = cx.weak_entity();
        let commit = self.commit.as_ref().map(|state| {
            let (go, stop) = (me.clone(), me.clone());
            commit::commit_box(
                state,
                self.facts.as_ref().is_some_and(|f| f.working),
                move |window, cx| _ = go.update(cx, |this, cx| this.make_commit(window, cx)),
                move |_, cx| _ = stop.update(cx, |this, cx| this.cancel_commit(cx)),
                cx,
            )
        });
        let mut notes: Vec<AnyElement> = Vec::new();
        if self.diff.as_ref().is_some_and(|d| d.cut) {
            notes.push(
                ChangesScreen::note(diff::CUT_NOTE, "changes-cut", p.muted_foreground, Some(p.muted.opacity(0.4)), false, cx)
                    .into_any_element(),
            );
        }
        if let Some(note) = &self.write_note {
            notes.push(ChangesScreen::note(note.clone(), "changes-write-note", p.muted_foreground, None, false, cx).into_any_element());
        }
        if let Load::Error { message, line: None } = &self.load {
            notes.push(ChangesScreen::note(message.clone(), "changes-error", p.error_foreground, None, false, cx).into_any_element());
        }
        if let Some(note) = &self.reveal_note {
            notes.push(ChangesScreen::note(note.clone(), "changes-reveal-note", p.muted_foreground, None, true, cx).into_any_element());
        }
        let body: AnyElement = match (&self.diff, &self.load) {
            (None, Load::Pending | Load::Idle) => div()
                .debug_selector(|| "changes-loading".into())
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(spinner(cx))
                .into_any_element(),
            (None, Load::Error { line: Some(line), .. }) => ChangesScreen::pane_line(*line, cx).into_any_element(),
            (None, _) => div().into_any_element(),
            (Some(diff), _) if diff.raw.is_some() => diff::raw_patch(diff.raw.as_ref().expect("raw"), cx).into_any_element(),
            (Some(diff), _) if diff.files.is_empty() => {
                let folder = self.repo.as_ref().map(|r| r.root.clone()).or_else(|| self.folder.clone()).unwrap_or_default();
                ChangesScreen::pane_line(words::empty(self.scope().noun(), &folder), cx).into_any_element()
            }
            (Some(diff), _) => {
                let all_cut = diff.all_cut();
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.changed_files(window, cx))
                    .child(if all_cut {
                        ChangesScreen::pane_line(diff::ALL_CUT, cx).into_any_element()
                    } else {
                        div().flex_1().min_h_0().child(self.code.clone()).into_any_element()
                    })
                    .into_any_element()
            }
        };
        frame
            .child(head)
            .children(commit)
            .child(v_flex().flex_1().min_h_0().min_w_0().overflow_hidden().bg(p.background).children(notes).child(body))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::head_count_words;

    #[test]
    fn a_heads_counts_read_deletions_first_and_leave_out_a_zero() {
        assert_eq!(head_count_words(2, 1), ["-1", "+2"]);
        assert_eq!(head_count_words(0, 1), ["-1"]);
        assert_eq!(head_count_words(1, 0), ["+1"]);
        assert_eq!(head_count_words(0, 0), ["-0", "+0"]);
    }
}
