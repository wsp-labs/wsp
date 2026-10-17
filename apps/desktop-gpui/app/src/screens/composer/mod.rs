// SPDX-License-Identifier: AGPL-3.0-only
//! The composer: the frosted box at the foot of a thread where the person writes, picks the agent, model, reasoning
//! and access, attaches files, opens the command and mention menus, stashes a draft, sends, and steers, queues or
//! stops while a turn runs. Drawn after the web app's ChatComposer.tsx and the files beside it; the host is reached
//! only by `link::Link`, which turns this view's events into `wsp-host-client` calls.

pub mod dock;
pub mod fork;
pub mod link;
mod menus;
pub mod model;
pub(crate) mod parts;
pub mod screen;
pub mod store;

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::input::{
    Enter, Escape, IndentInline, InlineToken, InputEvent, InputState, MoveDown, MoveUp, Textarea, TextareaState,
};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Sizable as _};
use gpui_kit::{
    actions, canvas, div, prelude::*, px, App, Bounds, ClipboardEntry, Context, Entity, EventEmitter, ExternalPaths, FocusHandle,
    Focusable, Global, KeyBinding, Pixels, SharedString, Subscription, Unbind, WeakEntity, Window,
};
use std::collections::HashMap;

use crate::local_store::LocalStore;
use crate::ui::ActivePalette;
use model::{
    Agent, Command, Drawer, File, Hold, HostItem, ItemKind, Layout, MenuItem, Picks, Queued, Recall, Recalled, Resolved, Skill, Sources,
    Stash, Stashed, Submit, Trigger,
};

pub use model::Choice;

actions!(composer, [StashDraft]);

/// Cmd or Ctrl and a digit in the model picker: its row of that number.
#[derive(Clone, PartialEq, Eq, serde::Deserialize, gpui_kit::Action)]
#[action(namespace = composer, no_json)]
pub struct PickRow(pub usize);

/// The composer's column, `max-w-3xl`.
pub const COLUMN: Pixels = px(768.);
/// `leading-relaxed` at the prompt's 14 px.
pub const LINE: Pixels = px(22.75);
const TEXT: Pixels = px(14.);
/// The rows a menu lists at most; the web caps its file list on the host's side, this caps what one frame lays out.
const MENU_CAP: usize = 50;
/// How long the draft waits after a keystroke before the store writes it.
const DRAFT_SAVE: std::time::Duration = std::time::Duration::from_millis(300);

/// Binds the composer's own chords. Call once after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("secondary-s", StashDraft, Some("Composer"))]);
    cx.bind_keys((1..=9).map(|n| KeyBinding::new(&format!("secondary-{n}"), PickRow(n), Some("Composer"))));
    // The text field's Cmd or Ctrl and Alt and an arrow adds a cursor in gpui-kit; here it steps through the threads,
    // as the shell binds it and today's app does.
    cx.bind_keys([
        KeyBinding::new("secondary-alt-up", Unbind("input::AddCursorAbove".into()), Some("Composer > Input")),
        KeyBinding::new("secondary-alt-down", Unbind("input::AddCursorBelow".into()), Some("Composer > Input")),
    ]);
}

/// Add to chat from a pane (a terminal's selection): `text` goes into `thread`'s composer at the caret on lines of its
/// own and the box takes the keys, Quote's road, where that thread's composer is the one on screen; for any other
/// thread it goes after that thread's kept draft on lines of its own, so it shows when the thread opens. The composer
/// is found through the window's shell after the caller's update ends, so a pane needs no handle to it.
pub fn add_to_chat(thread: &str, text: &str, cx: &mut App) {
    let (thread, text) = (thread.to_owned(), text.to_owned());
    cx.defer(move |cx| {
        for handle in cx.windows() {
            let taken = handle.update(cx, |_, window, cx| {
                let Some(root) = window.root::<gpui_kit::component::Root>().flatten() else { return false };
                let Ok(shell) = root.read(cx).view().clone().downcast::<crate::shell::Shell>() else { return false };
                let Some(screen) = shell.read(cx).thread_screen() else { return false };
                let composer = screen.read(cx).composer().clone();
                if !composer.read(cx).holds(&thread) {
                    return false;
                }
                composer.update(cx, |c, cx| c.insert_block(&text, window, cx));
                true
            });
            if taken.unwrap_or(false) {
                return;
            }
        }
        let Some(s) = cx.try_global::<LocalStore>() else { return };
        let draft = store::draft(s, &thread).map(|(draft, _)| draft).unwrap_or_default();
        let joined = match draft.as_str() {
            "" => format!("{text}\n"),
            kept if kept.ends_with('\n') => format!("{kept}{text}\n"),
            kept => format!("{kept}\n{text}\n"),
        };
        if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, &thread, &joined, joined.len())) {
            eprintln!("desktop-gpui: store: {e}");
        }
    });
}

/// A popup the composer has open over its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    Model,
    Reasoning,
    Access,
    Stash,
}

/// The command or mention menu, open on the token before the caret.
struct Menu {
    trigger: Trigger,
    range: Range<usize>,
    items: Vec<MenuItem>,
    active: usize,
}

/// The banner over the box: a fork's draft, or the outside conversation a send continues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner {
    pub fork: bool,
    pub line: String,
    pub note: String,
    /// The conversation's id where the send resumes one, and whether it resumes a copy; the send button then reads
    /// Continue.
    pub resume: Option<(String, bool)>,
}

impl Banner {
    /// The dismiss's name: "Cancel the fork" over a fork's draft, `CONVERSATION_WORDS.startFresh` over a resume.
    pub fn dismiss(&self) -> &'static str {
        if self.fork {
            fork::words::DISMISS
        } else {
            "Start fresh"
        }
    }

    /// The project home's line over the box while the person continues an outside conversation (ProjectHome.tsx,
    /// conversations.ts `CONVERSATION_WORDS.continues`), the agent's name its note.
    pub fn resume(id: String, title: &str, copy: bool, agent: &str) -> Banner {
        let line = if copy { format!("Continues a copy of {title}") } else { format!("Continues {title}") };
        Banner { fork: false, line, note: agent.to_owned(), resume: Some((id, copy)) }
    }
}

/// A bar standing in the composer's place, opened from its drawer row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Queue,
}

/// Fork from here: the workspace and thread a fork's draft forks, the turn it carries through (none on the thread's
/// first message), and the new branch it runs on, none in the source's folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fork {
    pub workspace: String,
    pub thread: String,
    pub turn: Option<String>,
    pub branch: Option<String>,
}

/// What the person sent, as a send carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub prompt: String,
    pub files: Vec<File>,
    pub picks: Resolved,
    /// Fast mode, where the model offers it.
    pub fast: bool,
    /// The outside conversation this send continues, and whether as a copy.
    pub resume: Option<(String, bool)>,
    /// A send the queue made, which a refusal puts back at the queue's head, held, as the web's `requeue` does.
    pub from_queue: bool,
    /// The thread and turn a fork's first send forks.
    pub fork: Option<Fork>,
}

/// What the composer asks of the host; `link::Link` answers each.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerEvent {
    /// A new turn.
    Start(Outgoing),
    /// Into the running turn.
    Steer(Outgoing),
    Stop,
    /// The access pick changed.
    Access(String),
    /// A mention menu opened and wants the thread folder's files.
    Files,
    /// A `#` menu opened and wants the repository's pull requests and issues.
    Items,
    /// Something the person should hear about that the box cannot show in place, for the app's notices.
    Notice(String),
    /// A pick on the docked prompt, with the words typed into a deny's field.
    Answer(dock::Answer),
    /// The line under a send whose agent is still starting (`agentStartingLine`), none once its turn starts.
    Starting(Option<String>),
}

/// A thread's queue as the box left it on a switch, with the files the store does not keep, whether it was held, and
/// the row a steer is out for.
#[derive(Default)]
struct Parked {
    queue: Vec<Queued>,
    held: bool,
    steering: Option<u64>,
}

pub struct Composer {
    editor: Entity<TextareaState>,
    agents: Vec<Agent>,
    picks: Picks,
    /// The thread's own agent and choices, once it has run.
    ran: Picks,
    commands: Vec<Command>,
    skills: Vec<Skill>,
    folder_files: Option<Vec<String>>,
    host_items: Option<Vec<HostItem>>,
    /// What the `#` menu says where it lists nothing: still loading, or why the list is not there.
    items_note: Option<String>,
    /// The thread's sent prompts for Up and Down, and where a walk through them stands.
    recalls: Vec<Recall>,
    recalled: Option<Recalled>,
    drawer: Drawer,
    bar: Option<Bar>,
    /// The models starred, as `harness:model`.
    favourites: Vec<String>,
    /// The model picker shows the starred models in place of one agent's.
    favourites_view: bool,
    /// The person's Fast mode pick for this thread, none until they make one.
    fast: Option<bool>,
    /// The note the model picker's foot says after another agent was asked for on a thread that has run.
    pinned_note: Option<String>,
    /// Where the store keeps this thread's draft, queue and fast mode.
    thread_key: Option<String>,
    /// The last notice raised, shown under the box until the person writes again.
    notice: Option<String>,
    /// Each item chip's one-line head, and the quoted text it stands for; emptied by each send.
    expansions: std::collections::HashMap<String, String>,
    saving: Option<gpui_kit::Task<()>>,
    /// A thread with a message or a running turn: the box takes one line.
    compact: bool,
    running: bool,
    stopping: bool,
    hold: Option<Hold>,
    steer_preferred: bool,
    files: Vec<File>,
    refused: Vec<(String, String)>,
    queue: Vec<Queued>,
    /// The queued row a steer is out for, which stays put until the host answers.
    steering: Option<u64>,
    next_id: u64,
    /// A queue read back from the store waits for the person's next send before it drains (composerDraftStore.ts).
    queue_held: bool,
    /// Each thread's queue as the box left it, so a switch back is not read as a launch.
    parked: HashMap<String, Parked>,
    /// The files of a typed send the host refused after the person left its thread, which the store does not keep,
    /// back in that thread's box when it shows again.
    refused_files: HashMap<String, Vec<File>>,
    /// The ask each thread's person folded, by thread key, which stays folded when the thread shows again.
    folded: HashMap<String, String>,
    /// The thread's own prompt in the composer's place, and the field and the focus it answers with.
    dock: Option<dock::Dock>,
    dock_field: Entity<InputState>,
    /// The docked edit's lines, built once per ask.
    dock_edit: Option<dock::EditLines>,
    dock_focus: FocusHandle,
    /// The thread this composer has said in `Docked` its prompt stands for.
    docked_key: Option<String>,
    /// The fork a fork's draft makes on its first send.
    forking: Option<Fork>,
    /// The thread's own words from before a fork's draft took the box, kept as its draft and put back on a cancel.
    before_fork: Option<String>,
    /// The threads, by key, with a send out that the host has not answered, so a queue left by a switch and found
    /// again does not send its next message alongside it.
    sending: std::collections::HashSet<String>,
    /// What the host said that every thread of a workspace shares, kept for the process.
    facts: link::Facts,
    /// The person's `midTurn` followed from the host, once a link stands.
    watching: Option<gpui_kit::Task<()>>,
    stash: Stash,
    banner: Option<Banner>,
    computer: Option<String>,
    /// The workspace the composer writes into, once a link stands, and its computer while it is not answering.
    workspace: Option<String>,
    absent: Option<String>,
    /// The sidebar, whose listing says which computers answer.
    places: Option<WeakEntity<crate::sidebar::Sidebar>>,
    _places: Option<Subscription>,
    branch: Option<String>,
    menu: Option<Menu>,
    /// The token an Esc closed the menu on; the menu opens again once the token changes.
    dismissed: Option<Range<usize>>,
    popup: Option<Popup>,
    /// The popup's row the keyboard is on, none until a key moves it.
    popup_active: Option<usize>,
    /// The agent whose models the model picker shows.
    rail: Option<String>,
    /// The legacy models' fold as the person left it.
    legacy_open: Option<bool>,
    /// The model picker's search, which takes focus while the picker is open.
    search: Entity<InputState>,
    focus_search: bool,
    /// The search's rule takes the primary ink while the search has the keys; a click on the rail takes them.
    search_lit: bool,
    /// The width the draft has beside the pickers, measured while the box is one line.
    line_width: Rc<Cell<Pixels>>,
    /// The composer's height as last drawn, and how many times it was built, for a parent that draws it cached.
    drawn: Rc<Cell<Pixels>>,
    /// The height a parent lays the composer out at while it draws it cached, none while it draws it whole.
    laid: Rc<Cell<Option<Pixels>>>,
    /// The frames that laid the composer out cached at a height it no longer had.
    stale: Rc<Cell<u64>>,
    built: u64,
    lines: usize,
    dragging: bool,
    placeholder: &'static str,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Focusable for Composer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

/// The threads whose own prompt stands in an open composer's place now (composerBar.ts `docked`), which leaves no box
/// for a slate's Ask for one to write into.
#[derive(Default)]
pub struct Docked(std::collections::HashSet<String>);

impl Global for Docked {}

/// Whether `thread`'s own prompt stands in its composer's place now.
pub fn docked(thread: &str, cx: &App) -> bool {
    cx.try_global::<Docked>().is_some_and(|d| d.0.contains(thread))
}

/// The composers open now, so a pane can put words in the one for its thread, as a slate's Ask for one does.
#[derive(Default)]
struct Open(Vec<WeakEntity<Composer>>);

impl Global for Open {}

/// The open composer whose thread, or New thread page's workspace, is `thread`: the newest, since a page's box the
/// window has just left lives until its next frame.
pub fn open_on(thread: &str, cx: &App) -> Option<Entity<Composer>> {
    let open: Vec<Entity<Composer>> =
        cx.try_global::<Open>().map(|o| o.0.iter().filter_map(WeakEntity::upgrade).collect()).unwrap_or_default();
    open.into_iter().rev().find(|c| c.read(cx).thread_key.as_deref() == Some(thread))
}

/// Puts `text` in the open composer whose thread is `thread`, the caret after it, its bar folded and the focus in it;
/// false where no composer has that thread open.
pub fn fill(thread: &str, text: &str, window: &mut Window, cx: &mut App) -> bool {
    let Some(composer) = open_on(thread, cx) else { return false };
    composer.update(cx, |c, cx| {
        c.set_text(text, window, cx);
        c.fold_bar(window, cx);
    });
    true
}

/// Puts `block` at the caret of the open composer whose thread is `thread`, on lines of its own, the caret after it, its
/// bar folded and the focus in it (`insertIntoComposer`); false where no composer has that thread open.
pub fn insert(thread: &str, block: &str, window: &mut Window, cx: &mut App) -> bool {
    let Some(composer) = open_on(thread, cx) else { return false };
    composer.update(cx, |c, cx| {
        let (text, at) = c.editor.read_with(cx, |e, _| (e.value().to_string(), e.cursor()));
        let (text, caret) = with_block(&text, at, block);
        c.editor.update(cx, |e, cx| {
            e.set_value(text, window, cx);
            e.set_selected_range(caret..caret, cx);
        });
        c.edited_in(window, cx);
        c.fold_bar(window, cx);
    });
    true
}

/// A typed send the host refused where no box holds its thread: its words go ahead of the draft the store keeps for it.
fn refused_into_store(key: &str, prompt: &str, cx: &mut App) {
    let Some(s) = cx.try_global::<LocalStore>() else { return };
    let draft = store::draft(s, key).map(|(d, _)| d).unwrap_or_default();
    let text = if draft.trim().is_empty() { prompt.to_owned() } else { format!("{prompt}\n{draft}") };
    if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, key, &text, text.len())) {
        eprintln!("desktop-gpui: store: {e}");
    }
}

/// `insertComposerBlock`: `text` with `block` at byte `at` on lines of its own, and the caret after it.
pub fn with_block(text: &str, at: usize, block: &str) -> (String, usize) {
    let at = (0..=at.min(text.len())).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    let before = if at == 0 || text[..at].ends_with('\n') { "" } else { "\n" };
    let after = if text[at..].starts_with('\n') { "" } else { "\n" };
    let block = format!("{before}{block}{after}");
    (format!("{}{block}{}", &text[..at], &text[at..]), at + block.len())
}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let me = cx.entity().downgrade();
        let open = cx.default_global::<Open>();
        open.0.retain(|c| c.upgrade().is_some());
        open.0.push(me);
        let editor = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 8).submit_on_enter(true).placeholder("Ask anything"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models..."));
        let dock_field = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.subscribe(&dock_field, |this, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.on_dock_field(&field, cx);
                }
            }),
            cx.on_app_quit(|this, cx| {
                this.save_draft(cx);
                async {}
            }),
            cx.subscribe_in(&editor, window, Self::on_input),
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.popup_active = Some(0);
                    cx.notify();
                }
            }),
            // A closed window or a quit drops the draft's pending save with this view.
            cx.on_release(|this, cx| {
                this.save_draft(cx);
                this.dock = None;
                this.sync_docked(cx);
            }),
        ];
        let mut composer = Composer {
            editor,
            agents: Vec::new(),
            picks: Picks::default(),
            ran: Picks::default(),
            commands: Vec::new(),
            skills: Vec::new(),
            folder_files: None,
            host_items: None,
            items_note: None,
            recalls: Vec::new(),
            recalled: None,
            drawer: Drawer::default(),
            bar: None,
            favourites: Vec::new(),
            favourites_view: false,
            fast: None,
            pinned_note: None,
            thread_key: None,
            notice: None,
            expansions: Default::default(),
            saving: None,
            compact: false,
            running: false,
            stopping: false,
            hold: None,
            steer_preferred: false,
            files: Vec::new(),
            refused: Vec::new(),
            queue: Vec::new(),
            steering: None,
            next_id: 0,
            queue_held: false,
            parked: Default::default(),
            refused_files: Default::default(),
            folded: Default::default(),
            dock: None,
            dock_field,
            dock_edit: None,
            dock_focus: cx.focus_handle(),
            docked_key: None,
            forking: None,
            before_fork: None,
            sending: Default::default(),
            facts: Default::default(),
            watching: None,
            stash: Stash::default(),
            banner: None,
            computer: None,
            workspace: None,
            absent: None,
            places: None,
            _places: None,
            branch: None,
            menu: None,
            dismissed: None,
            popup: None,
            popup_active: None,
            rail: None,
            legacy_open: None,
            search,
            focus_search: false,
            search_lit: false,
            line_width: Rc::new(Cell::new(px(0.))),
            drawn: Rc::new(Cell::new(px(0.))),
            laid: Rc::new(Cell::new(None)),
            stale: Rc::new(Cell::new(0)),
            built: 0,
            lines: 1,
            dragging: false,
            placeholder: "Ask anything",
            _subscriptions: subscriptions,
        };
        if let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) {
            composer.watch(&sidebar, cx);
        }
        composer
    }

    pub fn editor(&self) -> &Entity<TextareaState> {
        &self.editor
    }

    pub fn text(&self, cx: &App) -> String {
        self.editor.read(cx).value().to_string()
    }

    pub fn set_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| {
            e.set_value(text.to_owned(), window, cx);
            e.set_selected_range(text.len()..text.len(), cx);
        });
        self.edited_in(window, cx);
    }

    /// A kept draft back in the box, the caret at `at`.
    fn show_draft(&mut self, text: &str, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        let at = (0..=at.min(text.len())).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
        self.editor.update(cx, |e, cx| {
            e.set_value(text.to_owned(), window, cx);
            e.set_selected_range(at..at, cx);
        });
        self.edited_in(window, cx);
    }

    /// The agents `harnesses.list` gave, the choices the thread ran with, and the commands its agent announced.
    pub fn set_catalog(&mut self, agents: Vec<Agent>, ran: Picks, commands: Vec<Command>, cx: &mut Context<Self>) {
        self.agents = agents;
        self.ran = ran;
        self.commands = commands;
        cx.notify();
    }

    /// The skills `agents.read` reports for the thread's agent.
    pub fn set_skills(&mut self, skills: Vec<Skill>, cx: &mut Context<Self>) {
        self.skills = skills;
        cx.notify();
    }

    /// The repository's pull requests and issues for the `#` menu, or why there are none.
    pub fn set_host_items(&mut self, items: Vec<HostItem>, note: Option<String>, cx: &mut Context<Self>) {
        self.host_items = Some(items);
        self.items_note = note;
        self.refresh_menu(cx);
    }

    /// The thread's sent prompts, oldest first, by id.
    pub fn set_recalls(&mut self, prompts: Vec<(String, String)>, cx: &mut Context<Self>) {
        self.recalls = model::recalls(prompts);
        cx.notify();
    }

    pub fn set_drawer(&mut self, drawer: Drawer, cx: &mut Context<Self>) {
        self.drawer = drawer;
        cx.notify();
    }

    /// The key the store keeps this thread's draft, queue and fast mode under: the thread's id, or its workspace's
    /// before it has one. The key's draft, queue and fast mode are read back; a queue read back waits for the next send.
    /// Moving to another key keeps the box's draft under the key it leaves, parks its queue, lets go of everything else
    /// the thread held, as a composer per thread would, and shows the new key's draft, else an empty box.
    pub fn set_thread_key(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let moved = self.thread_key.as_ref().is_some_and(|k| *k != key);
        let fresh = self.thread_key.as_deref() != Some(key.as_str());
        if moved {
            self.saving = None;
            self.save_draft(cx);
            if let Some(left) = self.thread_key.clone() {
                let parked = Parked { queue: std::mem::take(&mut self.queue), held: self.queue_held, steering: self.steering.take() };
                if !parked.queue.is_empty() || parked.steering.is_some() {
                    self.parked.insert(left, parked);
                }
            }
            self.leave_thread(window, cx);
        }
        self.thread_key = Some(key.clone());
        let store = cx.try_global::<LocalStore>();
        let draft = store.and_then(|s| store::draft(s, &key));
        let restored = store.map(|s| store::queue(s, &key)).unwrap_or_default();
        if let Some(s) = store {
            (self.stash, self.favourites) = (store::stash(s), store::favourites(s));
        }
        self.fast = store.and_then(|s| store::fast(s, &key));
        // The caret stands where the store kept it, so showing the thread again writes nothing new to the store.
        if moved {
            let (text, at) = draft.unwrap_or_default();
            self.show_draft(&text, at, window, cx);
        } else if let Some((text, at)) = draft.filter(|_| self.text(cx).is_empty()) {
            self.show_draft(&text, at, window, cx);
        }
        if let Some(files) = self.refused_files.remove(&key) {
            self.files.extend(files);
        }
        if fresh {
            let parked = self.parked.remove(&key).unwrap_or(Parked { queue: restored.rows, held: restored.held, steering: None });
            self.next_id = parked.queue.iter().map(|q| q.id).max().unwrap_or(0).max(self.next_id);
            self.queue_held = parked.held && !parked.queue.is_empty();
            self.queue = parked.queue;
            self.steering = parked.steering;
        }
        self.sync_docked(cx);
        cx.notify();
    }

    /// What one thread's composer held and the next thread's starts without: its picks, files, banner, bar, menus,
    /// the folder's files, the prompt it was asked, where Up stood, its running turn, computer and agents. Whether the
    /// thread has a message stays until the next link says, so the box does not jump to a new thread's for a frame.
    fn leave_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picks = Picks::default();
        self.ran = Picks::default();
        self.commands.clear();
        self.skills.clear();
        self.files.clear();
        self.refused.clear();
        self.banner = None;
        self.forking = None;
        self.before_fork = None;
        self.bar = None;
        self.folder_files = None;
        self.host_items = None;
        self.items_note = None;
        self.menu = None;
        self.dismissed = None;
        self.popup = None;
        self.popup_active = None;
        self.rail = None;
        self.legacy_open = None;
        self.favourites_view = false;
        self.pinned_note = None;
        self.recalls.clear();
        self.recalled = None;
        self.expansions.clear();
        self.notice = None;
        self.drawer = Drawer::default();
        self.stopping = false;
        self.running = false;
        self.computer = None;
        self.agents.clear();
        self.absent = None;
        self.workspace = None;
        self.dock_edit = None;
        if self.dock.take().is_some() && self.dock_focus.contains_focused(window, cx) {
            self.editor.focus_handle(cx).focus(window, cx);
        }
    }

    /// What the composer already knows of `workspace`, which a link need not read again.
    pub fn known(&self, workspace: &str) -> link::Known {
        self.facts.known(workspace)
    }

    /// Whether the composer still writes into the thread `key` names; a link's late answer for another is not this one's.
    pub fn holds(&self, key: &str) -> bool {
        self.thread_key.as_deref() == Some(key)
    }

    /// The thread a send on the workspace's key opened: what waited under the workspace's key goes along to it.
    pub fn thread_started(&mut self, thread: String, cx: &mut Context<Self>) {
        let from = self.thread_key.replace(thread.clone());
        if let Some(from) = from.filter(|f| *f != thread).filter(|_| cx.has_global::<LocalStore>()) {
            cx.update_global(|s: &mut LocalStore, _| store::rekey(s, &from, &thread))
                .unwrap_or_else(|e| eprintln!("desktop-gpui: store: {e}"));
        }
    }

    /// Writes the thread's queue through to the store.
    fn keep(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.thread_key.clone().filter(|_| cx.has_global::<LocalStore>()) else { return };
        let queue = self.queue.clone();
        if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_queue(s, &key, &queue, Some(&key))) {
            eprintln!("desktop-gpui: store: {e}");
        }
    }

    /// Reads the stash and the favourites again before a change to them, since another composer of the app shares them.
    fn reload_shared(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = cx.try_global::<LocalStore>() {
            self.stash = store::stash(s);
            self.favourites = store::favourites(s);
        }
    }

    /// Writes the stash through to the store; a stash the store did not take is an error the caller keeps its draft on.
    fn keep_stash(&mut self, cx: &mut Context<Self>) -> std::io::Result<()> {
        if !cx.has_global::<LocalStore>() {
            return Ok(());
        }
        let (stash, keep) = (self.stash.clone(), self.thread_key.clone());
        cx.update_global(|s: &mut LocalStore, _| store::set_stash(s, &stash, keep.as_deref()))
    }

    /// Empties the box, and the draft the store keeps with it.
    fn clear_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.set_value(String::new(), window, cx));
        self.lines = 1;
        self.keep_draft(cx);
    }

    /// Writes the draft a moment after the last keystroke, since each write rewrites the store's whole file.
    fn keep_draft(&mut self, cx: &mut Context<Self>) {
        if self.thread_key.is_none() || !cx.has_global::<LocalStore>() {
            return;
        }
        self.saving = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DRAFT_SAVE).await;
            _ = this.update(cx, |this, cx| this.save_draft(cx));
        }));
    }

    fn save_draft(&mut self, cx: &mut App) {
        let Some(key) = self.thread_key.clone().filter(|_| cx.has_global::<LocalStore>()) else { return };
        let (text, cursor) = match &self.before_fork {
            Some(own) => (own.clone(), own.len()),
            None => (self.prompt(cx), self.editor.read(cx).cursor()),
        };
        if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_draft(s, &key, &text, cursor)) {
            eprintln!("desktop-gpui: store: {e}");
        }
    }

    pub fn set_folder_files(&mut self, files: Vec<String>, cx: &mut Context<Self>) {
        self.folder_files = Some(files);
        self.refresh_menu(cx);
    }

    /// Whether the thread has a message or a running turn, and whether a turn runs now. A turn that ends sends the
    /// queue's head.
    pub fn set_turn(&mut self, compact: bool, running: bool, cx: &mut Context<Self>) {
        self.compact = compact || running;
        self.running = running;
        if running && self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        if running {
            self.answered();
        }
        if !running {
            self.stopping = false;
        }
        if !running {
            self.drain(cx);
        }
        cx.notify();
    }

    /// Sends the queue's head as the next turn, once no turn runs, nothing holds the send and no steer is out.
    fn drain(&mut self, cx: &mut Context<Self>) {
        let out = self.thread_key.as_ref().is_some_and(|k| self.sending.contains(k));
        if self.running
            || self.hold.is_some()
            || self.absent.is_some()
            || self.steering.is_some()
            || self.queue_held
            || self.queue.is_empty()
            || out
        {
            return;
        }
        let next = self.queue.remove(0);
        self.hold = Some(Hold::InFlight);
        self.sending.extend(self.thread_key.clone());
        let mut out = self.outgoing(next.prompt, next.files);
        out.from_queue = true;
        cx.emit(ComposerEvent::Start(out));
        self.keep(cx);
    }

    /// The thread on screen has no send out any more.
    fn answered(&mut self) {
        if let Some(key) = &self.thread_key {
            self.sending.remove(key);
        }
    }

    /// The host answered a send on the thread `key`; answered after a switch, it lets that thread send again.
    pub fn sent_on(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.sent(cx);
        }
        self.sending.remove(key);
    }

    /// The host took a send: a turn starts on it, so nothing waits on the reply any more.
    pub fn sent(&mut self, cx: &mut Context<Self>) {
        self.answered();
        if self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        // The conversation is continued once the host took the send; a refused one keeps the banner for the retry.
        if self.banner.as_ref().is_some_and(|b| b.resume.is_some()) {
            self.banner = None;
        }
        self.compact = true;
        cx.notify();
    }

    pub fn set_hold(&mut self, hold: Option<Hold>, cx: &mut Context<Self>) {
        self.hold = hold;
        cx.notify();
    }

    /// Whether a message sent mid-turn goes into the turn (the person's `midTurn`).
    pub fn steer_preferred(&self) -> bool {
        self.steer_preferred
    }

    pub fn set_steer_preferred(&mut self, steer: bool, cx: &mut Context<Self>) {
        self.steer_preferred = steer;
        cx.notify();
    }

    /// A steer the host has answered: taken by the turn, or left to wait for its end.
    pub fn steered(&mut self, taken: bool, cx: &mut Context<Self>) {
        if let Some(id) = self.steering.take() {
            if taken {
                self.queue.retain(|q| q.id != id);
                self.keep(cx);
            }
        }
        self.drain(cx);
        cx.notify();
    }

    /// A steer on the thread `key` the host has answered. Answered after a switch, it settles that thread's parked queue,
    /// and nothing of the thread on screen.
    pub fn steered_on(&mut self, key: &str, taken: bool, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.steered(taken, cx);
        }
        if let Some(parked) = self.parked.get_mut(key) {
            if let Some(id) = parked.steering.take().filter(|_| taken) {
                parked.queue.retain(|q| q.id != id);
                let (queue, here) = (parked.queue.clone(), self.thread_key.clone());
                if cx.has_global::<LocalStore>() {
                    if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_queue(s, key, &queue, here.as_deref())) {
                        eprintln!("desktop-gpui: store: {e}");
                    }
                }
            }
        }
    }

    /// A send on the thread `key` the host refused. Refused after a switch, a queued send goes back to the head of that
    /// thread's parked queue, held, and a typed one ahead of the draft kept for it.
    pub fn refused_on(&mut self, key: &str, sent: Outgoing, window: &mut Window, cx: &mut Context<Self>) {
        if self.holds(key) {
            return self.refused_send(sent, window, cx);
        }
        self.sending.remove(key);
        if sent.from_queue {
            self.next_id += 1;
            let parked = self.parked.entry(key.to_owned()).or_default();
            parked.queue.insert(0, Queued { id: self.next_id, prompt: sent.prompt, files: sent.files });
            parked.held = true;
            let (queue, here) = (parked.queue.clone(), self.thread_key.clone());
            if cx.has_global::<LocalStore>() {
                if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_queue(s, key, &queue, here.as_deref())) {
                    eprintln!("desktop-gpui: store: {e}");
                }
            }
            return;
        }
        if !sent.files.is_empty() {
            self.refused_files.entry(key.to_owned()).or_default().extend(sent.files);
        }
        refused_into_store(key, &sent.prompt, cx);
    }

    /// `prompt` went to the thread `key` another way, as the start line's Try again sends it: the draft that is those
    /// words goes, so it is not sent twice (SpawnLine.tsx).
    pub fn sent_elsewhere(&mut self, key: &str, prompt: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.holds(key) && self.text(cx) == prompt {
            self.set_text("", window, cx);
        }
    }

    /// A send the host refused: its words go back in the box ahead of any draft written since, and its files with them.
    pub fn refused_send(&mut self, sent: Outgoing, window: &mut Window, cx: &mut Context<Self>) {
        self.answered();
        self.hold = None;
        if sent.from_queue {
            self.next_id += 1;
            self.queue.insert(0, Queued { id: self.next_id, prompt: sent.prompt, files: sent.files });
            self.queue_held = true;
            self.keep(cx);
            cx.notify();
            return;
        }
        let draft = self.text(cx);
        let text = if draft.trim().is_empty() { sent.prompt } else { format!("{}\n{draft}", sent.prompt) };
        self.set_text(&text, window, cx);
        self.files.extend(sent.files);
        cx.notify();
    }

    pub fn stopped(&mut self, cx: &mut Context<Self>) {
        self.stopping = false;
        cx.notify();
    }

    pub fn set_banner(&mut self, banner: Option<Banner>, cx: &mut Context<Self>) {
        if !banner.as_ref().is_some_and(|b| b.fork) {
            self.forking = None;
            self.before_fork = None;
        }
        self.banner = banner;
        cx.notify();
    }

    /// Cancel the fork: the banner goes, the box holds the page's own words again, and the window goes back to the
    /// thread the fork came off.
    pub fn cancel_fork(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.forking.clone();
        if let Some(own) = self.before_fork.take() {
            self.set_text(&own, window, cx);
        }
        self.set_banner(None, cx);
        if let Some(source) = source {
            fork::cancelled(&source, cx);
        }
    }

    /// The fork the next send makes, while the box is a fork's draft.
    pub fn forking(&self) -> Option<&Fork> {
        self.forking.as_ref()
    }

    /// The host took a fork's first send: the draft is spent.
    pub fn forked(&mut self, cx: &mut Context<Self>) {
        self.forking = None;
        self.banner = None;
        cx.notify();
    }

    /// Follows the sidebar's listing for whether the workspace's computer answers.
    fn watch(&mut self, sidebar: &Entity<crate::sidebar::Sidebar>, cx: &mut Context<Self>) {
        self.places = Some(sidebar.downgrade());
        self._places = Some(cx.observe(sidebar, |this, _, cx| this.read_absent(cx)));
        self.read_absent(cx);
    }

    /// The workspace the composer writes into now.
    pub fn set_workspace(&mut self, workspace: String, cx: &mut Context<Self>) {
        self.workspace = Some(workspace);
        self.read_absent(cx);
    }

    /// Whether the workspace's computer is not answering (`absenceOf`), as the sidebar's listing says.
    fn read_absent(&mut self, cx: &mut Context<Self>) {
        let sidebar = self.places.as_ref().and_then(WeakEntity::upgrade);
        let absent = match (sidebar, &self.workspace) {
            // A daemon this host started is not the road a turn takes, so its absence leaves the box open
            // (ChatComposer.tsx `daemonOnly`); another computer not answering holds the send in its name.
            (Some(sidebar), Some(workspace)) => {
                let sidebar = sidebar.read(cx);
                sidebar
                    .absence(workspace)
                    .filter(|a| a.start.is_none())
                    .map(|_| sidebar.own_computer_name(workspace).unwrap_or_else(|| workspace.clone()))
            }
            _ => None,
        };
        if absent != self.absent {
            self.absent = absent;
            cx.notify();
        }
    }

    pub fn set_place(&mut self, computer: Option<String>, branch: Option<String>, cx: &mut Context<Self>) {
        self.computer = computer;
        self.branch = branch;
        cx.notify();
    }

    /// A thread with no message yet, whose composer the thread view centres.
    pub fn fresh(&self) -> bool {
        !self.compact
    }

    /// The key the composer keeps its draft and queue under now.
    pub fn thread_key(&self) -> Option<&str> {
        self.thread_key.as_deref()
    }

    pub fn hold(&self) -> Option<&Hold> {
        self.hold.as_ref()
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn queue(&self) -> &[Queued] {
        &self.queue
    }

    pub fn files(&self) -> &[File] {
        &self.files
    }

    pub fn stash(&self) -> &Stash {
        &self.stash
    }

    pub fn picks(&self) -> &Picks {
        &self.picks
    }

    /// The agent and the choices the thread ran with, which a fork's draft carries.
    pub fn ran(&self) -> &Picks {
        &self.ran
    }

    pub fn popup(&self) -> Option<Popup> {
        self.popup
    }

    /// The values the open popup's rows pick, in order.
    pub fn popup_rows(&self, cx: &App) -> Vec<String> {
        self.popup.map(|popup| menus::popup_rows(self, popup, cx)).unwrap_or_default()
    }

    /// The rows of the open command or mention menu.
    pub fn menu_items(&self) -> Option<&[MenuItem]> {
        self.menu.as_ref().map(|m| m.items.as_slice())
    }

    pub fn layout(&self) -> Layout {
        model::layout(self.compact, "", self.lines)
    }

    /// Why the send is held where the box is open, in `composerSendBlock`'s order: no road to wsp, then a computer not
    /// answering, which leaves the box open (`composerHeldLine`), then the other holds, else a slash draft that names
    /// no command.
    pub fn send_held(&self, cx: &App) -> Option<String> {
        if let Some(hold @ (Hold::Connecting | Hold::HostDown | Hold::NotFound)) = &self.hold {
            return Some(hold.words().to_owned());
        }
        if let Some(computer) = &self.absent {
            return Some(format!("held until {computer} answers"));
        }
        if let Some(hold) = &self.hold {
            return Some(hold.words().to_owned());
        }
        let commands = self.commands_offered();
        model::slash_hold(&self.text(cx), &commands)
    }

    /// The announced commands, on a thread that has run; a new thread has no agent session to ask.
    fn commands_offered(&self) -> Vec<Command> {
        if self.ran.agent.is_none() {
            return Vec::new();
        }
        model::commands(&self.commands)
    }

    pub fn resolved(&self) -> Resolved {
        model::resolve(&self.agents, &self.picks, &self.ran)
    }

    /// Picks made elsewhere, as a fork's draft carries its source's.
    pub fn set_picks(&mut self, picks: Picks, cx: &mut Context<Self>) {
        self.picks = picks;
        cx.notify();
    }

    /// Opens or closes a popup, as a click on its trigger does.
    pub fn toggle_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        self.popup = if self.popup == Some(popup) { None } else { Some(popup) };
        if matches!(self.popup, Some(Popup::Model | Popup::Stash)) {
            self.reload_shared(cx);
        }
        self.focus_search = self.popup == Some(Popup::Model);
        self.search_lit = self.focus_search;
        self.popup_active = matches!(popup, Popup::Stash | Popup::Model).then_some(0);
        self.rail = self.resolved().agent.map(|a| a.harness);
        self.favourites_view = false;
        self.pinned_note = None;
        cx.notify();
    }

    /// Picks a choice in the open popup.
    pub fn pick(&mut self, popup: Popup, value: String, window: &mut Window, cx: &mut Context<Self>) {
        match popup {
            Popup::Model => {
                let (agent, value) = match value.split_once(':').filter(|_| self.favourites_view) {
                    Some((harness, model)) => (Some(harness.to_owned()), model.to_owned()),
                    None => (self.rail.clone(), value),
                };
                let offered = self
                    .agents
                    .iter()
                    .find(|a| Some(&a.harness) == agent.as_ref())
                    .is_some_and(|a| a.models.iter().chain(&a.legacy_models).any(|m| m.value == value));
                if !offered || self.pinned().is_some_and(|p| agent.as_deref() != Some(p)) {
                    return;
                }
                if agent.is_some() && agent != self.resolved().agent.map(|a| a.harness) {
                    self.picks.effort = None;
                    self.picks.window = None;
                    self.picks.access = None;
                }
                self.picks.agent = agent;
                self.picks.model = Some(value);
            }
            Popup::Reasoning if value.starts_with("fast:") => {
                let on = value == "fast:on";
                self.fast = Some(on);
                if let Some(key) = self.thread_key.clone().filter(|_| cx.has_global::<LocalStore>()) {
                    if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_fast(s, &key, on)) {
                        eprintln!("desktop-gpui: store: {e}");
                    }
                }
            }
            Popup::Reasoning => {
                let resolved = self.resolved();
                let window_pick = resolved.windows.iter().any(|c| c.value == value);
                if window_pick {
                    self.picks.window = Some(value);
                } else {
                    self.picks.effort = Some(value);
                }
            }
            Popup::Access => {
                self.picks.access = Some(value.clone());
                cx.emit(ComposerEvent::Access(value));
            }
            Popup::Stash => {
                if let Ok(ix) = value.parse::<usize>() {
                    self.restore(ix, window, cx);
                }
            }
        }
        self.popup = None;
        self.editor.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Shows an agent's models in the picker; on a thread that has run, only its own agent's, and a note for another.
    pub fn set_rail(&mut self, harness: String, cx: &mut Context<Self>) {
        self.favourites_view = false;
        self.search_lit = false;
        self.popup_active = Some(0);
        self.pinned_note = None;
        match self.pinned() {
            Some(pinned) if pinned != harness => {
                let label = self.agents.iter().find(|a| a.harness == harness).map_or(harness.clone(), |a| a.label.clone());
                self.pinned_note = Some(format!("Start a new thread to use {label} here"));
            }
            _ => self.rail = Some(harness),
        }
        cx.notify();
    }

    /// Takes files as a pick, a drop or a paste does: each refused one is kept with its reason.
    pub fn attach(&mut self, files: Vec<File>, cx: &mut Context<Self>) {
        let no_images = self.resolved().agent.filter(|a| !a.images).map(|a| a.label);
        for file in files {
            match model::refuse(&file.name, &file.media_type, file.bytes.len(), self.files.len(), no_images.as_deref()) {
                Some(why) => self.refused.push((file.name, why)),
                None => self.files.push(file),
            }
        }
        cx.notify();
    }

    pub fn remove_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.files.len() {
            self.files.remove(ix);
        }
        cx.notify();
    }

    /// Files from paths: one past the size cap is refused by its length, unread; the rest are read off the window's
    /// thread.
    fn attach_paths(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        let mut reads = Vec::new();
        for path in paths {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            let len = std::fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
            if len > model::FILE_MAX_BYTES {
                if let Some(why) = model::refuse(&name, model::media_type(&name), len, self.files.len(), None) {
                    self.refused.push((name, why));
                }
                continue;
            }
            reads.push((name, path.clone()));
        }
        cx.notify();
        if reads.is_empty() {
            return;
        }
        let read = cx.background_spawn(async move {
            reads
                .into_iter()
                .filter_map(|(name, path)| {
                    let bytes = std::fs::read(path).ok()?;
                    Some(File { media_type: model::media_type(&name).to_owned(), name, bytes: Arc::new(bytes) })
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = read.await;
            _ = this.update(cx, |this, cx| this.attach(files, cx));
        })
        .detach();
    }

    fn pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                _ = this.update(cx, |this, cx| this.attach_paths(&paths, cx));
            }
        })
        .detach();
    }

    fn outgoing(&mut self, prompt: String, files: Vec<File>) -> Outgoing {
        let picks = self.resolved();
        let fast = self.fast() && picks.model.as_ref().is_some_and(|m| m.fast == Some(true));
        let resume = self.banner.as_ref().and_then(|b| b.resume.clone());
        Outgoing { prompt, files, picks, fast, resume, from_queue: false, fork: self.forking.clone() }
    }

    /// The draft as it goes: each pull request's or issue's chip opened back into its quoted text.
    fn prompt(&self, cx: &App) -> String {
        let editor = self.editor.read(cx);
        let mut text = editor.value().to_string();
        let mut spans: Vec<_> = editor.tokens().iter().map(|t| (t.range(), t.token().text().to_string())).collect();
        spans.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, head) in spans {
            if let Some(whole) = self.expansions.get(&head) {
                text.replace_range(range, whole);
            }
        }
        text
    }

    /// Enter: a new turn, a steer, or a message onto the queue.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Enter holds what the send button holds (ChatComposer.tsx `send`), leaving the draft in the box.
        if model::slash_hold(&self.text(cx), &self.commands_offered()).is_some() {
            return;
        }
        // A computer not answering holds the send alone; the words wait in the box for the person's own send.
        if self.absent.is_some() {
            return;
        }
        let prompt = self.prompt(cx);
        let empty = prompt.trim().is_empty();
        let resolved = self.resolved();
        // One steer is out at a time: the steer's answer names the row it carried (ChatComposer.tsx `canSteer`).
        let steers = self.steer_preferred && self.steering.is_none();
        // A fork's draft starts a thread of its own, whatever this one is doing.
        let running = self.running && self.forking.is_none();
        let how = model::submit(empty, self.hold.as_ref(), running, steers, resolved.agent.as_ref(), &self.files);
        if how == Submit::Nothing {
            return;
        }
        let files = std::mem::take(&mut self.files);
        self.refused.clear();
        self.menu = None;
        self.clear_box(window, cx);
        self.saving = None;
        self.save_draft(cx);
        let out = self.outgoing(prompt.trim_end().to_owned(), files);
        self.expansions.clear();
        self.recalled = None;
        let held = std::mem::take(&mut self.queue_held);
        match how {
            Submit::Start => {
                self.hold = Some(Hold::InFlight);
                self.sending.extend(self.thread_key.clone());
                self.compact = true;
                cx.emit(ComposerEvent::Start(out));
            }
            Submit::Steer => {
                self.next_id += 1;
                self.queue.insert(0, Queued { id: self.next_id, prompt: out.prompt.clone(), files: out.files.clone() });
                self.steering = Some(self.next_id);
                cx.emit(ComposerEvent::Steer(out));
            }
            Submit::Queue => {
                self.next_id += 1;
                let row = Queued { id: self.next_id, prompt: out.prompt, files: out.files };
                // Over a queue read back from the store, the new message goes first (composerDraftStore.ts).
                if held {
                    self.queue.insert(0, row);
                } else {
                    self.queue.push(row);
                }
            }
            Submit::Nothing => {}
        }
        self.keep(cx);
        cx.notify();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.running && !self.stopping {
            self.stopping = true;
            cx.emit(ComposerEvent::Stop);
            cx.notify();
        }
    }

    /// Cmd or Ctrl and S: the draft onto the stash; from an empty box, the only entry back, or the stash menu.
    pub fn stash_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload_shared(cx);
        let prompt = self.prompt(cx);
        if !prompt.trim().is_empty() || !self.files.is_empty() {
            let before = self.stash.clone();
            self.stash.push(Stashed { prompt, files: self.files.clone(), at: std::time::SystemTime::now() });
            if let Err(e) = self.keep_stash(cx) {
                // A stash the store did not take leaves the draft where it was, as promptStashStore.ts does.
                eprintln!("desktop-gpui: store: {e}");
                self.stash = before;
                return;
            }
            self.files.clear();
            self.clear_box(window, cx);
            self.saving = None;
            self.save_draft(cx);
            self.menu = None;
        } else if self.stash.entries.len() == 1 {
            self.restore(0, window, cx);
        } else if !self.stash.entries.is_empty() {
            self.toggle_popup(Popup::Stash, cx);
        }
        cx.notify();
    }

    /// Puts a stashed draft back in the box, stashing what the box held first (ChatComposer.tsx `restoreStashed`).
    fn restore(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.stash.take(ix) else { return };
        let draft = self.prompt(cx);
        if !draft.trim().is_empty() || !self.files.is_empty() {
            self.stash.push(Stashed { prompt: draft, files: std::mem::take(&mut self.files), at: std::time::SystemTime::now() });
        }
        self.set_text(&entry.prompt, window, cx);
        self.files.extend(entry.files);
        if let Err(e) = self.keep_stash(cx) {
            eprintln!("desktop-gpui: store: {e}");
        }
    }

    pub fn delete_stashed(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.stash.take(ix);
        if let Err(e) = self.keep_stash(cx) {
            eprintln!("desktop-gpui: store: {e}");
        }
        if self.stash.entries.is_empty() && self.popup == Some(Popup::Stash) {
            self.popup = None;
        }
        cx.notify();
    }

    /// Takes a queued message back into the box, after any draft there, and folds the bar (ChatComposer.tsx `editCard`).
    pub fn edit_queued(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.queue.iter().position(|q| q.id == id) else { return };
        let queued = self.queue.remove(ix);
        let draft = self.text(cx);
        let text = if draft.trim().is_empty() { queued.prompt } else { format!("{}\n{}", draft.trim(), queued.prompt) };
        self.set_text(&text, window, cx);
        self.files.extend(queued.files);
        self.fold_bar(window, cx);
        self.keep(cx);
    }

    /// Drops a queued message, files and all (ChatComposer.tsx `removeCard`).
    pub fn cancel_queued(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.queue.retain(|q| q.id != id);
        if self.queue.is_empty() && self.bar == Some(Bar::Queue) {
            self.fold_bar(window, cx);
        }
        self.keep(cx);
        cx.notify();
    }

    pub fn open_bar(&mut self, bar: Bar, cx: &mut Context<Self>) {
        self.bar = Some(bar);
        self.popup = None;
        self.menu = None;
        cx.notify();
    }

    /// Puts the bar away and gives the box the keys again.
    pub fn fold_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bar = None;
        self.editor.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Raises a notice: emitted for the app's notices, and shown under the box until the person writes again.
    pub fn raise(&mut self, text: String, cx: &mut Context<Self>) {
        self.notice = Some(text.clone());
        cx.emit(ComposerEvent::Notice(text));
        cx.notify();
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn drawer(&self) -> &Drawer {
        &self.drawer
    }

    pub fn banner(&self) -> Option<&Banner> {
        self.banner.as_ref()
    }

    /// What the `#` menu says where it lists nothing.
    pub fn items_note(&self) -> Option<&str> {
        self.items_note.as_deref()
    }

    pub fn bar(&self) -> Option<Bar> {
        self.bar
    }

    /// A printable key typed in the thread outside any field (composerTypeToFocus.ts): the box takes the focus and the
    /// key, after the draft; a bar standing in its place folds first.
    pub fn type_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        // A digit the standing dock owns goes to the dock wherever it was typed (useTypeToWrite's `keyBelongsElsewhere`).
        if let Some(n) = key.parse::<usize>().ok().filter(|n| (1..=9).contains(n)) {
            if self.dock.as_ref().is_some_and(|d| !d.folded) {
                self.dock_focus.focus(window, cx);
                return self.dock_digit(n, window, cx);
            }
        }
        if self.hold.as_ref().is_some_and(Hold::shuts) {
            return;
        }
        self.bar = None;
        // A letter typed while the prompt stands in the box's place folds it and starts a message (useTypeToWrite).
        self.fold_dock();
        self.sync_docked(cx);
        let text = format!("{}{key}", self.text(cx));
        self.set_text(&text, window, cx);
        self.editor.focus_handle(cx).focus(window, cx);
    }

    /// A block from the thread, a reply's Quote (`insertIntoComposer`): at the caret on lines of its own, the caret after
    /// it, the box focused so the next key writes the message.
    pub fn insert_block(&mut self, block: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.bar = None;
        self.editor.update(cx, |e, cx| {
            let (text, at) = (e.value(), e.cursor());
            let before = if text[..at].is_empty() || text[..at].ends_with('\n') { "" } else { "\n" };
            let after = if text[at..].starts_with('\n') { "" } else { "\n" };
            e.insert(format!("{before}{block}{after}"), window, cx);
        });
        self.edited_in(window, cx);
        // Under a standing dock the block joins the draft and the dock keeps the keys, since the box is not drawn.
        if self.dock.as_ref().is_none_or(|d| d.folded) {
            self.editor.focus_handle(cx).focus(window, cx);
        }
    }

    /// Stars or unstars a model in the picker, as `harness:model`.
    pub fn toggle_favourite(&mut self, key: String, cx: &mut Context<Self>) {
        self.reload_shared(cx);
        if let Some(at) = self.favourites.iter().position(|f| *f == key) {
            self.favourites.remove(at);
        } else {
            self.favourites.push(key);
        }
        if cx.has_global::<LocalStore>() {
            let favourites = self.favourites.clone();
            if let Err(e) = cx.update_global(|s: &mut LocalStore, _| store::set_favourites(s, &favourites)) {
                eprintln!("desktop-gpui: store: {e}");
            }
        }
        cx.notify();
    }

    pub fn favourites(&self) -> &[String] {
        &self.favourites
    }

    /// Fast mode in force: the pick, else whether the thread's newest turn ran fast (ChatComposer.tsx).
    pub fn fast(&self) -> bool {
        self.fast.or(self.ran.fast).unwrap_or(false)
    }

    /// Shows the starred models in the picker in place of one agent's.
    pub fn show_favourites(&mut self, cx: &mut Context<Self>) {
        self.favourites_view = true;
        self.search_lit = false;
        self.popup_active = Some(0);
        cx.notify();
    }

    /// The agent the thread ran on, which no later send can change.
    fn pinned(&self) -> Option<&str> {
        self.ran.agent.as_deref()
    }

    fn on_input(&mut self, _: &Entity<TextareaState>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => {
                self.notice = None;
                if matches!(self.popup, Some(Popup::Reasoning | Popup::Access | Popup::Stash)) {
                    self.popup = None;
                }
                self.edited_in(window, cx)
            }
            InputEvent::PressEnter { shift: false, secondary: false } => self.submit(window, cx),
            // A click back into the text puts away the model picker, whose search had the keys.
            InputEvent::Focus if self.popup == Some(Popup::Model) => {
                self.popup = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.measure(window, cx);
        self.keep_draft(cx);
        cx.notify();
    }

    fn edited_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edited(window, cx);
        self.refresh_menu(cx);
    }

    /// How many lines the draft takes at the width it has beside the pickers.
    fn measure(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.text(cx);
        let width = self.line_width.get();
        self.lines = if text.contains('\n') {
            2
        } else if width <= px(0.) || text.is_empty() {
            1
        } else {
            let font = gpui_kit::font(cx.theme().font_family.clone());
            let run = gpui_kit::TextRun {
                len: text.len(),
                font,
                color: gpui_kit::Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_text(text.into(), TEXT, &[run], Some(width), None)
                .map(|lines| lines.iter().map(|l| l.wrap_boundaries().len() + 1).sum())
                .unwrap_or(1)
        };
    }

    fn refresh_menu(&mut self, cx: &mut Context<Self>) {
        let (text, cursor) = self.editor.read_with(cx, |e, _| (e.value().to_string(), e.cursor()));
        let found = model::trigger(&text, cursor);
        let Some((trigger, range)) = found else {
            self.menu = None;
            self.dismissed = None;
            cx.notify();
            return;
        };
        if self.dismissed.as_ref() == Some(&range) || self.hold.as_ref().is_some_and(Hold::shuts) {
            self.menu = None;
            return;
        }
        self.dismissed = None;
        if matches!(trigger, Trigger::Mention(_)) && self.folder_files.is_none() {
            self.folder_files = Some(Vec::new());
            cx.emit(ComposerEvent::Files);
        }
        if matches!(trigger, Trigger::Hash(_)) && self.host_items.is_none() {
            self.host_items = Some(Vec::new());
            self.items_note = Some("Loading pull requests and issues".to_owned());
            cx.emit(ComposerEvent::Items);
        }
        let commands = self.commands_offered();
        let from = Sources {
            commands: &commands,
            skills: &self.skills,
            files: self.folder_files.as_deref().unwrap_or_default(),
            items: self.host_items.as_deref().unwrap_or_default(),
        };
        let items = model::menu_items(&trigger, from, MENU_CAP);
        let active = self.menu.as_ref().filter(|m| m.trigger == trigger).map_or(0, |m| m.active.min(items.len().saturating_sub(1)));
        self.menu = Some(Menu { trigger, range, items, active });
        cx.notify();
    }

    /// Puts the menu's active row in place of its token: a command as words, a file as a chip.
    fn take_menu(&mut self, ix: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        let Some(item) = menu.items.get(ix.unwrap_or(menu.active)).cloned() else { return };
        let range = menu.range.clone();
        let chip = match &item.kind {
            ItemKind::Command => None,
            ItemKind::File => Some((format!("file:{}", item.insert), item.label.clone())),
            ItemKind::Skill => Some((format!("skill:{}", item.insert), item.label.trim_start_matches('$').to_owned())),
            ItemKind::Reference { pull } => Some((format!("{}:{}", if *pull { "pr" } else { "issue" }, item.label), item.label.clone())),
        };
        // A chip holds one line, so an item's chip carries its head and the send puts its quoted body back.
        let insert = match &item.kind {
            ItemKind::Reference { .. } => {
                let head = item.insert.lines().next().unwrap_or_default().to_owned();
                self.expansions.insert(head.clone(), item.insert.clone());
                head
            }
            _ => item.insert.clone(),
        };
        self.editor.update(cx, |e, cx| match chip {
            Some((id, label)) => {
                let token = InlineToken::new(SharedString::from(id), insert.clone()).with_label(label);
                if e.replace_range_with_token(range, token, window, cx).is_ok() {
                    e.replace(" ", window, cx);
                }
            }
            None => {
                e.set_selected_range(range, cx);
                e.replace(insert.clone(), window, cx);
            }
        });
        self.menu = None;
        self.edited(window, cx);
    }

    fn menu_step(&mut self, by: isize, cx: &mut Context<Self>) -> bool {
        if let Some(menu) = &mut self.menu {
            let n = menu.items.len() as isize;
            if n > 0 {
                menu.active = (menu.active as isize + by).rem_euclid(n) as usize;
            }
            cx.notify();
            return true;
        }
        if let Some(popup) = self.popup {
            let n = menus::popup_rows(self, popup, cx).len() as isize;
            if n > 0 {
                let from = self.popup_active.map_or(if by > 0 { -1 } else { n }, |a| a as isize);
                self.popup_active = Some((from + by).rem_euclid(n) as usize);
            }
            cx.notify();
            return true;
        }
        false
    }

    fn on_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(-1, cx) || self.recall(true, window, cx) {
            cx.stop_propagation();
        }
    }

    fn on_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(1, cx) || self.recall(false, window, cx) {
            cx.stop_propagation();
        }
    }

    /// Up on the first line or Down on the last walks the thread's sent prompts (composerPromptHistory.ts); the caret
    /// lands at the start going back and at the end going forward.
    fn recall(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (text, cursor) = self.editor.read_with(cx, |e, _| (e.value().to_string(), e.cursor()));
        let edge = if back { !text[..cursor.min(text.len())].contains('\n') } else { !text[cursor.min(text.len())..].contains('\n') };
        if !edge {
            return false;
        }
        let Some((at, prompt)) = model::recall_step(&self.recalls, self.recalled.as_ref(), &text, back) else { return false };
        self.recalled = at;
        self.set_text(&prompt, window, cx);
        if back {
            self.editor.update(cx, |e, cx| e.set_selected_range(0..0, cx));
        }
        true
    }

    fn on_pick_row(&mut self, action: &PickRow, window: &mut Window, cx: &mut Context<Self>) {
        if self.popup != Some(Popup::Model) {
            return;
        }
        cx.stop_propagation();
        if let Some(value) = menus::popup_rows(self, Popup::Model, cx).get(action.0 - 1).cloned() {
            self.pick(Popup::Model, value, window, cx);
        }
    }

    /// Enter sends, or picks the menu's row; taken here before the text field's own Enter, which would otherwise let
    /// the key through to the platform and leave a newline in the emptied box. While an IME composes, the platform
    /// routes Enter to the input method before any binding, so nothing here sees that Enter.
    fn on_enter(&mut self, action: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if action.shift || action.secondary {
            return;
        }
        cx.stop_propagation();
        if self.menu.as_ref().is_some_and(|m| !m.items.is_empty()) {
            self.take_menu(None, window, cx);
        } else if let Some(popup) = self.popup.filter(|_| self.popup_active.is_some()) {
            if let Some(value) = self.popup_active.and_then(|ix| menus::popup_rows(self, popup, cx).get(ix).cloned()) {
                self.pick(popup, value, window, cx);
            }
        } else {
            self.submit(window, cx);
        }
    }

    fn on_tab(&mut self, _: &IndentInline, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.as_ref().is_some_and(|m| !m.items.is_empty()) {
            self.take_menu(None, window, cx);
            cx.stop_propagation();
        }
    }

    fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.bar.is_some() {
            self.fold_bar(window, cx);
            cx.stop_propagation();
        } else if let Some(menu) = self.menu.take() {
            self.dismissed = Some(menu.range);
            cx.stop_propagation();
        } else if self.popup.take().is_some() {
            self.editor.focus_handle(cx).focus(window, cx);
            cx.stop_propagation();
        }
        cx.notify();
    }

    fn on_stash(&mut self, _: &StashDraft, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_draft(window, cx);
    }

    fn on_paste(&mut self, entries: Vec<ClipboardEntry>, cx: &mut Context<Self>) -> bool {
        let mut files = Vec::new();
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                ClipboardEntry::String(text) => {
                    let taken: Vec<&str> = self.files.iter().chain(&files).map(|f| f.name.as_str()).collect();
                    if let Some(file) = model::pasted_file(text.text(), &taken) {
                        files.push(file);
                    }
                }
                ClipboardEntry::Image(image) => {
                    let n = self.files.len() + files.len() + 1;
                    let media = image.format().mime_type().to_owned();
                    let ext = media.rsplit('/').next().unwrap_or("png").to_owned();
                    files.push(File {
                        name: format!("pasted-image-{n}.{ext}"),
                        media_type: media,
                        bytes: Arc::new(image.bytes().to_vec()),
                    });
                }
                ClipboardEntry::ExternalPaths(p) => paths.extend(p.paths().iter().cloned()),
            }
        }
        if files.is_empty() && paths.is_empty() {
            return false;
        }
        self.attach(files, cx);
        self.attach_paths(&paths, cx);
        true
    }
}

impl Render for Composer {
    /// The composer, and its height as drawn, which a parent that draws it cached lays it out at.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.built += 1;
        let (drawn, laid, stale, me) = (self.drawn.clone(), self.laid.clone(), self.stale.clone(), cx.entity().downgrade());
        // Laid out cached at a height it no longer has, the composer asks to be drawn whole in the next frame.
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                let height = bounds.size.height;
                drawn.set(height);
                if laid.get().is_some_and(|at| at != height) {
                    stale.set(stale.get() + 1);
                    let me = me.clone();
                    cx.defer(move |cx| _ = me.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        div().relative().w_full().child(self.body(window, cx)).child(measure)
    }
}

impl Composer {
    /// The height the composer last drew at, none before its first draw.
    pub fn drawn_height(&self) -> Option<Pixels> {
        Some(self.drawn.get()).filter(|h| *h > px(0.))
    }

    /// Draws the composer cached at `height` from the next frame, or whole where none.
    pub fn lay_at(&self, height: Option<Pixels>) {
        self.laid.set(height);
    }

    /// How many times the composer was built; a frame that reuses its cached drawing builds nothing.
    pub fn built(&self) -> u64 {
        self.built
    }

    /// How many frames laid the composer out cached at a height it no longer had.
    pub fn stale_layouts(&self) -> u64 {
        self.stale.get()
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if std::mem::take(&mut self.focus_search) {
            self.search.update(cx, |s, cx| s.set_value("", window, cx));
            self.search.focus_handle(cx).focus(window, cx);
        }
        if self.dragging && !cx.has_active_drag() {
            self.dragging = false;
        }
        let p = cx.palette().clone();
        let layout = self.layout();
        let shut = self.hold.as_ref().is_some_and(Hold::shuts);
        let width = self.line_width.clone();
        let editor_width = width.get();
        // The slash hint stands while `/` opens a menu: the announced commands, or the skills it lists under them.
        let slash_lists = !self.commands_offered().is_empty() || (self.ran.agent.is_some() && !self.skills.is_empty());
        let placeholder = model::placeholder(slash_lists, if editor_width > px(0.) { f32::from(editor_width) } else { 600. });
        if self.placeholder != placeholder {
            self.placeholder = placeholder;
            self.editor.update(cx, |e, cx| e.set_placeholder(placeholder, window, cx));
        }
        let this = cx.entity();
        let paste = this.downgrade();
        let editor = div()
            .id("composer-editor")
            .debug_selector(|| "composer-editor".into())
            .relative()
            .flex_1()
            .min_w_0()
            .text_size(TEXT)
            .line_height(LINE)
            .text_color(p.foreground)
            .when(layout == Layout::Full, |el| el.min_h(px(74.)).pb_1())
            .child(
                // gpui-kit pads the field 4 px a side and wraps its text 10 px short of its right edge for the caret;
                // the web's text runs edge to edge of its column. The field is w_full, so the margins go on a wrapper.
                div().ml(px(-4.)).mr(px(-14.)).child(
                    Textarea::new(&self.editor)
                        .appearance(false)
                        .xsmall()
                        .disabled(shut)
                        .text_size(TEXT)
                        .line_height(LINE)
                        .token(|ctx, _, cx| parts::chip(ctx, cx))
                        .on_paste(move |item, _, cx| {
                            let entries = item.entries().to_vec();
                            paste.update(cx, |this, cx| this.on_paste(entries, cx)).unwrap_or(false)
                        }),
                ),
            )
            .when(layout == Layout::Line, |el| {
                el.child(canvas(move |bounds: Bounds<Pixels>, _, _| width.set(bounds.size.width), |_, _, _, _| {}).absolute().size_full())
            });

        if self.bar == Some(Bar::Queue) && self.queue.is_empty() {
            self.bar = None;
        }
        if let Some(dock) = self.dock.as_ref().filter(|d| !d.folded) {
            return dock::view(self, dock, cx);
        }
        if self.bar == Some(Bar::Queue) {
            return div()
                .id("composer")
                .key_context("Composer")
                .debug_selector(|| "composer".into())
                .w_full()
                .px_4()
                .pb_4()
                .capture_action(cx.listener(Self::on_escape))
                .child(parts::queue_bar(self, cx))
                .into_any_element();
        }
        let pickers = parts::pickers(self, layout, cx);
        let actions = parts::actions(self, shut, cx);
        let grid = match layout {
            Layout::Full => v_flex()
                .px_4()
                .pt_4()
                .pb_4()
                .gap_2()
                .child(editor)
                .child(h_flex().items_center().justify_between().gap_2().child(pickers).child(actions)),
            Layout::Line => h_flex().items_center().gap_1().pl_5().pr_2().py_2().child(editor).child(pickers).child(actions),
            Layout::Tall => v_flex()
                .gap_1()
                .pl_5()
                .pr_2()
                .pt(px(14.))
                .pb_2()
                .child(editor)
                .child(h_flex().items_center().justify_end().gap_1().child(pickers).child(actions)),
        };
        let body = v_flex().rounded(px(20.)).overflow_hidden().children(parts::files_row(self, cx)).child(grid);
        let shell = v_flex()
            .relative()
            .mx_auto()
            .w_full()
            .max_w(COLUMN)
            .children(self.banner.as_ref().map(|b| parts::banner(b, cx)))
            .children(parts::drawer(self, cx))
            .child(parts::glass_box(cx).child(body))
            .child(parts::tray(self, cx))
            .children(self.notice.clone().map(|n| parts::under(n, cx)))
            .children(menus::menu_layer(self, cx));
        div()
            .id("composer")
            .key_context("Composer")
            .debug_selector(|| "composer".into())
            .relative()
            .w_full()
            .px_5()
            .pt_2()
            .pb_5()
            .capture_action(cx.listener(Self::on_up))
            .capture_action(cx.listener(Self::on_down))
            .capture_action(cx.listener(Self::on_enter))
            .capture_action(cx.listener(Self::on_tab))
            .capture_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_stash))
            .capture_action(cx.listener(Self::on_pick_row))
            .on_drag_move::<ExternalPaths>(cx.listener(|this, _, _, cx| {
                if !this.dragging {
                    this.dragging = true;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.dragging = false;
                if !this.hold.as_ref().is_some_and(Hold::shuts) {
                    this.attach_paths(paths.paths(), cx);
                }
            }))
            .child(shell)
            .when(self.dragging && !shut, |el| el.child(parts::drop_overlay(cx)))
            .into_any_element()
    }
}
