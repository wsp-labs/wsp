// SPDX-License-Identifier: AGPL-3.0-only
//! The composer: the frosted box at the foot of a thread where the person writes, picks the agent, model, reasoning
//! and access, attaches files, opens the command and mention menus, stashes a draft, sends, and steers, queues or
//! stops while a turn runs. Drawn after the web app's ChatComposer.tsx and the files beside it; the host is reached
//! only by `link::Link`, which turns this view's events into `wsp-host-client` calls.

mod attach;
pub mod dock;
pub mod fork;
pub mod hero;
mod keys;
pub mod link;
mod menus;
pub mod model;
pub(crate) mod parts;
mod render;
pub mod screen;
mod send;
pub mod store;

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{
    actions, prelude::*, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Global, KeyBinding, Pixels, Subscription, Unbind,
    WeakEntity, Window,
};
use std::collections::HashMap;

use crate::local_store::LocalStore;
use model::{
    Agent, Command, Drawer, File, Hold, HostItem, Layout, MenuItem, Picks, Queued, Recall, Recalled, Resolved, Skill, Stash, Stashed,
    Trigger,
};

pub use model::Choice;

actions!(composer, [StashDraft]);

/// Cmd or Ctrl and a digit in the model picker: its row of that number.
#[derive(Clone, PartialEq, Eq, serde::Deserialize, gpui_kit::Action)]
#[action(namespace = composer, no_json)]
pub struct PickRow(pub usize);

/// The composer's column, `max-w-3xl`.
pub const COLUMN: Pixels = px(768.);
/// The surface's width past which the access stands in the box's bar, else in the tray (`ACCESS_IN_BAR_PX`).
pub const ACCESS_IN_BAR: Pixels = px(640.);

/// Whether a box this wide holds the access in its bar; the web measures the surface inside the box's 1 px ring.
pub(super) fn wide(box_width: Pixels) -> bool {
    box_width - px(2.) > ACCESS_IN_BAR
}

/// The last box's width, so a new composer's first frame puts the access where the last one stood.
pub(super) struct BoxWidth(pub(super) Pixels);
impl Global for BoxWidth {}
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

/// A bar standing in the composer's place, opened from its drawer row (composerBar.ts `ComposerBar`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Queue,
    Tasks,
    Usage,
    Capped,
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
    /// The usage limit's Resume at reset, armed or taken back (`sessions.mark` `resumeAtReset`).
    Mark(bool),
    /// A side question (`/btw <question>`) for the thread's agent, answered on a tab of its own.
    Aside(String),
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
    /// The window the composer draws in, which a pane's words reach it through.
    window: gpui_kit::AnyWindowHandle,
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
    /// The open bar's own focus, which takes the keys as it opens so Esc folds it (composerBar.ts `useBarRoot`).
    bar_focus: FocusHandle,
    focus_bar: bool,
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
    /// The computer's mark in the tray, as the places list draws it (`RowComputer`'s `ComputerGlyph`).
    computer_icon: Option<crate::ui::computer_glyph::ComputerIcon>,
    /// The branch a project home's new thread starts on, as the host reads the project's folder (`useProjectBranch`).
    project_branch: Option<String>,
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
    /// The box's width as last drawn, or the last composer's before its first draw (`useWiderThan` on the surface).
    box_width: Rc<Cell<Pixels>>,
    /// The composer's height as last drawn, and how many times it was built, for a parent that draws it cached.
    drawn: Rc<Cell<Pixels>>,
    /// The height a parent lays the composer out at while it draws it cached, none while it draws it whole.
    laid: Rc<Cell<Option<Pixels>>>,
    /// The frames that laid the composer out cached at a height it no longer had.
    stale: Rc<Cell<u64>>,
    built: u64,
    lines: usize,
    dragging: bool,
    /// The window is under 640 px wide (`max-sm`), as of the last draw.
    narrow: bool,
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

/// Hands the open composer whose thread is `thread` the draft `joined` makes of the words in its box, the caret after
/// them and the focus in it (sendToThread.ts `setDraft` and `requestComposerFocus`), once the caller's update ends;
/// false where no composer has that thread open. The box's words, not the store's, since the store waits out a pause.
pub fn take_draft(thread: &str, joined: impl FnOnce(&str) -> String + 'static, cx: &mut App) -> bool {
    let Some(composer) = open_on(thread, cx) else { return false };
    let handle = composer.read(cx).window;
    cx.defer(move |cx| {
        _ = handle.update(cx, |_, window, cx| {
            composer.update(cx, |c, cx| {
                let text = joined(&c.text(cx));
                c.set_text(&text, window, cx);
                c.fold_bar(window, cx);
            })
        });
    });
    true
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
            window: window.window_handle(),
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
            bar_focus: cx.focus_handle(),
            focus_bar: false,
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
            computer_icon: None,
            project_branch: None,
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
            box_width: Rc::new(Cell::new(cx.try_global::<BoxWidth>().map_or(px(0.), |w| w.0))),
            drawn: Rc::new(Cell::new(px(0.))),
            laid: Rc::new(Cell::new(None)),
            stale: Rc::new(Cell::new(0)),
            built: 0,
            lines: 1,
            dragging: false,
            narrow: false,
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
        self.drop_bar(window, cx);
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
        self.computer_icon = None;
        self.branch = None;
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
    /// The side question in the box went to the host: the box empties, as after a send.
    pub(crate) fn aside_asked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_box(window, cx);
        self.save_draft(cx);
    }

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
        // The tray's branch comes back on the workspace's pushed status once asked for.
        if let Some(sidebar) = self.places.as_ref().and_then(WeakEntity::upgrade) {
            sidebar.update(cx, |s, cx| s.want_checkout(&workspace, cx));
        }
        self.workspace = Some(workspace);
        self.read_absent(cx);
    }

    /// The workspace's computer by name and mark, its branch, and whether it is not answering (`absenceOf`), as the
    /// sidebar's listing says.
    fn read_absent(&mut self, cx: &mut Context<Self>) {
        let sidebar = self.places.as_ref().and_then(WeakEntity::upgrade);
        let (mut icon, mut branch) = (self.computer_icon, self.branch.clone());
        let (computer, absent) = match (sidebar, &self.workspace) {
            (Some(sidebar), Some(workspace)) => {
                let sidebar = sidebar.read(cx);
                let computer = sidebar.computer_name(workspace);
                let prefs = sidebar.around().preferences.clone().unwrap_or_default();
                icon = sidebar
                    .place_of(workspace)
                    .and_then(|id| sidebar.place(&id))
                    .and_then(|row| crate::sidebar::looks::computer_icon(row, &prefs));
                branch = sidebar.branch_of(workspace);
                // A daemon this host started is not the road a turn takes, so its absence leaves the box open
                // (ChatComposer.tsx `daemonOnly`); another computer not answering holds the send in its name.
                let absent =
                    sidebar.absence(workspace).filter(|a| a.start.is_none()).map(|_| computer.clone().unwrap_or_else(|| workspace.clone()));
                (computer.or_else(|| self.computer.clone()), absent)
            }
            _ => (self.computer.clone(), None),
        };
        if computer != self.computer || absent != self.absent || icon != self.computer_icon || branch != self.branch {
            self.computer = computer;
            self.computer_icon = icon;
            self.branch = branch;
            self.absent = absent;
            cx.notify();
        }
    }

    /// The mark the tray draws its computer by.
    pub fn computer_icon(&self) -> Option<crate::ui::computer_glyph::ComputerIcon> {
        self.computer_icon
    }

    /// The words the empty box shows, as last drawn.
    pub fn placeholder(&self) -> &'static str {
        self.placeholder
    }

    /// The branch the project home's send starts its thread on, which the tray names over the workspace's own.
    pub fn set_project_branch(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        if self.project_branch != branch {
            self.project_branch = branch;
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

    /// Whether the access picker stands in the box's bar: the full layout, in a box `wide` enough.
    pub fn access_in_bar(&self) -> bool {
        self.layout() == Layout::Full && wide(self.box_width.get())
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

    /// Whether `/btw` asks a side question here (catalog.ts `asides`): the thread's agent takes one and the thread has a
    /// row to name it by.
    fn asides(&self) -> bool {
        self.ran.agent.is_some() && self.resolved().agent.is_some_and(|a| a.asides)
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
        self.focus_bar = true;
        self.popup = None;
        self.menu = None;
        cx.notify();
    }

    /// The usage limit's acts: Resume at reset arms the host to go on with the turn at the reset, Cancel takes that back;
    /// either folds the bar (UsageBar.tsx `mark`).
    pub fn mark_resume(&mut self, resume: bool, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ComposerEvent::Mark(resume));
        self.fold_bar(window, cx);
    }

    /// Puts the bar away and gives the box the keys again.
    pub fn fold_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bar = None;
        self.focus_box(window, cx);
        cx.notify();
    }

    /// A bar that went on its own hands the keys it held to the box.
    fn drop_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bar = None;
        if self.bar_focus.is_focused(window) {
            self.focus_box(window, cx);
        }
    }

    /// The box takes the keys, unless the agent's question stands in its place and keeps them.
    fn focus_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dock.as_ref().is_none_or(|d| d.folded) {
            self.editor.focus_handle(cx).focus(window, cx);
        }
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
        self.focus_box(window, cx);
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
}
