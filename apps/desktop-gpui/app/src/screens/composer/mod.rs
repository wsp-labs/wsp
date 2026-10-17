// SPDX-License-Identifier: AGPL-3.0-only
//! The composer: the frosted box at the foot of a thread where the person writes, picks the agent, model, reasoning
//! and access, attaches files, opens the command and mention menus, stashes a draft, sends, and steers, queues or
//! stops while a turn runs. Drawn after the web app's ChatComposer.tsx and the files beside it; the host is reached
//! only by `link::Link`, which turns this view's events into `wsp-host-client` calls.

pub mod link;
mod marks;
mod menus;
pub mod model;
mod parts;
pub mod screen;

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
    Focusable, KeyBinding, Pixels, SharedString, Subscription, Window,
};

use crate::ui::ActivePalette;
use model::{Agent, Command, File, Hold, Layout, MenuItem, Picks, Queued, Resolved, Stash, Stashed, Submit, Trigger};

pub use model::Choice;

actions!(composer, [StashDraft]);

/// The composer's column, `max-w-3xl`.
pub const COLUMN: Pixels = px(768.);
/// `leading-relaxed` at the prompt's 14 px.
pub const LINE: Pixels = px(22.75);
const TEXT: Pixels = px(14.);
/// The rows a menu lists at most; the web caps its file list on the host's side, this caps what one frame lays out.
const MENU_CAP: usize = 50;

/// Binds the composer's own chords. Call once after `gpui_kit::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("secondary-s", StashDraft, Some("Composer"))]);
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

/// The banner over the box: a fork's draft, or the conversation a send continues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner {
    pub fork: bool,
    pub line: String,
    pub note: String,
}

/// What the person sent, as a send carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub prompt: String,
    pub files: Vec<File>,
    pub picks: Resolved,
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
}

pub struct Composer {
    editor: Entity<TextareaState>,
    agents: Vec<Agent>,
    picks: Picks,
    /// The thread's own agent and choices, once it has run.
    ran: Picks,
    commands: Vec<Command>,
    folder_files: Option<Vec<String>>,
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
    stash: Stash,
    banner: Option<Banner>,
    computer: Option<String>,
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
    /// The width the draft has beside the pickers, measured while the box is one line.
    line_width: Rc<Cell<Pixels>>,
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

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, 8).submit_on_enter(true).placeholder("Ask anything"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models..."));
        let subscriptions = vec![
            cx.subscribe_in(&editor, window, Self::on_input),
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.popup_active = Some(0);
                    cx.notify();
                }
            }),
        ];
        Composer {
            editor,
            agents: Vec::new(),
            picks: Picks::default(),
            ran: Picks::default(),
            commands: Vec::new(),
            folder_files: None,
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
            stash: Stash::default(),
            banner: None,
            computer: None,
            branch: None,
            menu: None,
            dismissed: None,
            popup: None,
            popup_active: None,
            rail: None,
            legacy_open: None,
            search,
            focus_search: false,
            line_width: Rc::new(Cell::new(px(0.))),
            lines: 1,
            dragging: false,
            placeholder: "Ask anything",
            _subscriptions: subscriptions,
        }
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

    /// The agents `harnesses.list` gave, the choices the thread ran with, and the commands its agent announced.
    pub fn set_catalog(&mut self, agents: Vec<Agent>, ran: Picks, commands: Vec<Command>, cx: &mut Context<Self>) {
        self.agents = agents;
        self.ran = ran;
        self.commands = commands;
        cx.notify();
    }

    pub fn set_folder_files(&mut self, files: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_files = Some(files);
        self.refresh_menu(window, cx);
    }

    /// Whether the thread has a message or a running turn, and whether a turn runs now. A turn that ends sends the
    /// queue's head.
    pub fn set_turn(&mut self, compact: bool, running: bool, cx: &mut Context<Self>) {
        let ended = self.running && !running;
        self.compact = compact || running;
        self.running = running;
        if running && self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        if !running {
            self.stopping = false;
        }
        if ended {
            self.drain(cx);
        }
        cx.notify();
    }

    /// Sends the queue's head as the next turn, once no turn runs, nothing holds the send and no steer is out.
    fn drain(&mut self, cx: &mut Context<Self>) {
        if self.running || self.hold.is_some() || self.steering.is_some() || self.queue.is_empty() {
            return;
        }
        let next = self.queue.remove(0);
        self.hold = Some(Hold::InFlight);
        cx.emit(ComposerEvent::Start(Outgoing { prompt: next.prompt, files: next.files, picks: self.resolved() }));
    }

    /// The host took a send: a turn starts on it, so nothing waits on the reply any more.
    pub fn sent(&mut self, cx: &mut Context<Self>) {
        if self.hold == Some(Hold::InFlight) {
            self.hold = None;
        }
        self.compact = true;
        cx.notify();
    }

    pub fn set_hold(&mut self, hold: Option<Hold>, cx: &mut Context<Self>) {
        self.hold = hold;
        cx.notify();
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
            }
        }
        self.drain(cx);
        cx.notify();
    }

    /// A send the host refused: its words go back in the box ahead of any draft written since, and its files with them.
    pub fn refused_send(&mut self, sent: Outgoing, window: &mut Window, cx: &mut Context<Self>) {
        self.hold = None;
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
        self.banner = banner;
        cx.notify();
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

    /// Why the send is held where the box is open: the hold, else a slash draft that names no command.
    pub fn send_held(&self, cx: &App) -> Option<String> {
        if let Some(hold) = &self.hold {
            return Some(hold.words().to_owned());
        }
        let commands = self.commands_offered();
        model::slash_hold(&self.text(cx), &commands)
    }

    /// The announced commands and wsp's own, on a thread that has run; a new thread has no agent session to ask.
    fn commands_offered(&self) -> Vec<Command> {
        if self.ran.agent.is_none() {
            return Vec::new();
        }
        model::commands(&self.commands, self.resolved().agent.as_ref())
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
        self.focus_search = self.popup == Some(Popup::Model);
        self.popup_active = matches!(popup, Popup::Stash | Popup::Model).then_some(0);
        self.rail = self.resolved().agent.map(|a| a.harness);
        cx.notify();
    }

    /// Picks a choice in the open popup.
    pub fn pick(&mut self, popup: Popup, value: String, window: &mut Window, cx: &mut Context<Self>) {
        match popup {
            Popup::Model => {
                let agent = self.rail.clone();
                if agent.is_some() && agent != self.resolved().agent.map(|a| a.harness) {
                    self.picks.effort = None;
                    self.picks.window = None;
                    self.picks.access = None;
                }
                self.picks.agent = agent;
                self.picks.model = Some(value);
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

    pub fn set_rail(&mut self, harness: String, cx: &mut Context<Self>) {
        self.rail = Some(harness);
        self.popup_active = None;
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

    /// Enter: a new turn, a steer, or a message onto the queue.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.text(cx);
        let empty = prompt.trim().is_empty();
        let resolved = self.resolved();
        let how = model::submit(empty, self.hold.as_ref(), self.running, self.steer_preferred, resolved.agent.as_ref(), &self.files);
        if how == Submit::Nothing {
            return;
        }
        let files = std::mem::take(&mut self.files);
        self.refused.clear();
        self.menu = None;
        self.editor.update(cx, |e, cx| e.set_value(String::new(), window, cx));
        self.lines = 1;
        let out = Outgoing { prompt: prompt.trim_end().to_owned(), files, picks: resolved };
        match how {
            Submit::Start => {
                self.hold = Some(Hold::InFlight);
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
                self.queue.push(Queued { id: self.next_id, prompt: out.prompt, files: out.files });
            }
            Submit::Nothing => {}
        }
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
        let prompt = self.text(cx);
        if !prompt.trim().is_empty() || !self.files.is_empty() {
            self.stash.push(Stashed { prompt, files: std::mem::take(&mut self.files), at: std::time::Instant::now() });
            self.editor.update(cx, |e, cx| e.set_value(String::new(), window, cx));
            self.menu = None;
        } else if self.stash.entries.len() == 1 {
            self.restore(0, window, cx);
        } else if !self.stash.entries.is_empty() {
            self.toggle_popup(Popup::Stash, cx);
        }
        cx.notify();
    }

    fn restore(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.stash.take(ix) {
            self.set_text(&entry.prompt, window, cx);
            self.files.extend(entry.files);
        }
    }

    pub fn delete_stashed(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.stash.take(ix);
        if self.stash.entries.is_empty() && self.popup == Some(Popup::Stash) {
            self.popup = None;
        }
        cx.notify();
    }

    /// Takes a queued message back into the box, after any draft there.
    pub fn edit_queued(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.queue.len() {
            return;
        }
        let queued = self.queue.remove(ix);
        let draft = self.text(cx);
        let text = if draft.trim().is_empty() { queued.prompt } else { format!("{draft}\n{}", queued.prompt) };
        self.set_text(&text, window, cx);
        self.files.extend(queued.files);
    }

    fn on_input(&mut self, _: &Entity<TextareaState>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => {
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
        cx.notify();
    }

    fn edited_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edited(window, cx);
        self.refresh_menu(window, cx);
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
                color: gpui_kit::black(),
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

    fn refresh_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let commands = self.commands_offered();
        let items = model::menu_items(&trigger, &commands, self.folder_files.as_deref().unwrap_or_default(), MENU_CAP);
        let active = self.menu.as_ref().filter(|m| m.trigger == trigger).map_or(0, |m| m.active.min(items.len().saturating_sub(1)));
        let _ = window;
        self.menu = Some(Menu { trigger, range, items, active });
        cx.notify();
    }

    /// Puts the menu's active row in place of its token: a command as words, a file as a chip.
    fn take_menu(&mut self, ix: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        let Some(item) = menu.items.get(ix.unwrap_or(menu.active)).cloned() else { return };
        let range = menu.range.clone();
        self.editor.update(cx, |e, cx| {
            if item.file {
                let token = InlineToken::new(SharedString::from(format!("file:{}", item.insert)), format!("@{}", item.insert))
                    .with_label(item.label.clone());
                if e.replace_range_with_token(range, token, window, cx).is_ok() {
                    e.replace(" ", window, cx);
                }
            } else {
                e.set_selected_range(range, cx);
                e.replace(item.insert.clone(), window, cx);
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

    fn on_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(-1, cx) {
            cx.stop_propagation();
        }
    }

    fn on_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(1, cx) {
            cx.stop_propagation();
        }
    }

    /// Enter sends, or picks the menu's row; taken here before the text field's own Enter, which would otherwise let
    /// the key through to the platform and leave a newline in the emptied box. An IME composition keeps its Enter.
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
        if let Some(menu) = self.menu.take() {
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
                _ => {}
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let commands = self.commands_offered();
        let placeholder = model::placeholder(!commands.is_empty(), if editor_width > px(0.) { f32::from(editor_width) } else { 600. });
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
                Textarea::new(&self.editor)
                    .appearance(false)
                    .xsmall()
                    .disabled(shut)
                    .text_size(TEXT)
                    .line_height(LINE)
                    .mx(px(-4.))
                    .token(|ctx, _, cx| parts::chip(ctx, cx))
                    .on_paste(move |item, _, cx| {
                        let entries = item.entries().to_vec();
                        paste.update(cx, |this, cx| this.on_paste(entries, cx)).unwrap_or(false)
                    }),
            )
            .when(layout == Layout::Line, |el| {
                el.child(canvas(move |bounds: Bounds<Pixels>, _, _| width.set(bounds.size.width), |_, _, _, _| {}).absolute().size_full())
            });

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
            .children((!self.queue.is_empty()).then(|| parts::drawer(self, cx)))
            .child(parts::glass_box(cx).child(body))
            .child(parts::tray(self, cx))
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
    }
}
