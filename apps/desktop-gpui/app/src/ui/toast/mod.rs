// SPDX-License-Identifier: AGPL-3.0-only
//! The toasts, after `apps/web/src/notices/` and Sonner: one stack per window, drawn as a gpui-kit Root plugin under
//! the layers registered after it, which any screen speaks through with [`notify`]. [`store`] keeps what was said,
//! [`stack`] draws it. Where the stack stands and who else answers Escape are the app's to say, through [`Hooks`].

pub mod stack;
pub mod store;

use std::rc::Rc;
use std::time::Instant;

use gpui_kit::base::{Root, RootPlugin};
use gpui_kit::{
    div, point, prelude::*, px, App, Context, Entity, FocusHandle, Global, KeyContext, Pixels, SharedString, Subscription, Task, Window,
};

use stack::Stack;
use store::Store;

/// `NoticeKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Error,
    Done,
    Waiting,
    Note,
}

/// What a notice's button runs.
pub type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// `NoticeAction`: the word on the toast's button and what it runs.
#[derive(Clone)]
pub struct Action {
    pub word: SharedString,
    pub run: Run,
}

impl Action {
    pub fn new(word: impl Into<SharedString>, run: impl Fn(&mut Window, &mut App) + 'static) -> Action {
        Action { word: word.into(), run: Rc::new(run) }
    }
}

/// `NoticeInput`: what a notice says; its id and time are given when it is added.
#[derive(Clone)]
pub struct Notice {
    pub kind: Kind,
    pub text: SharedString,
    /// The computer or workspace it is about.
    pub place: Option<SharedString>,
    /// A second sentence under the first.
    pub detail: Option<SharedString>,
    pub action: Option<Action>,
    /// A notice that stands for a wait: its toast stays until [`end_notice`] with the key, and a second add under the
    /// key replaces it.
    pub key: Option<SharedString>,
}

impl Notice {
    pub fn new(kind: Kind, text: impl Into<SharedString>) -> Notice {
        Notice { kind, text: text.into(), place: None, detail: None, action: None, key: None }
    }

    pub fn error(text: impl Into<SharedString>) -> Notice {
        Notice::new(Kind::Error, text)
    }

    pub fn done(text: impl Into<SharedString>) -> Notice {
        Notice::new(Kind::Done, text)
    }

    pub fn note(text: impl Into<SharedString>) -> Notice {
        Notice::new(Kind::Note, text)
    }

    pub fn waiting(text: impl Into<SharedString>) -> Notice {
        Notice::new(Kind::Waiting, text)
    }

    pub fn place(mut self, place: impl Into<SharedString>) -> Notice {
        self.place = Some(place.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Notice {
        self.detail = Some(detail.into());
        self
    }

    pub fn action(mut self, action: Action) -> Notice {
        self.action = Some(action);
        self
    }

    pub fn key(mut self, key: impl Into<SharedString>) -> Notice {
        self.key = Some(key.into());
        self
    }
}

/// The focus holders that answer Escape before a toast does: a field, a dialog, a menu (`ownsEscape`).
const OWNS_ESCAPE: [&str; 15] = [
    "Input",
    "Composer",
    "Terminal",
    "Editor",
    "Dialog",
    "Sheet",
    "PopupMenu",
    "Select",
    "Combobox",
    "Popover",
    "Command",
    "NumberInput",
    "TimeField",
    "DatePicker",
    "ColorPicker",
];

/// What the app tells the stack about the window it stands in.
#[derive(Clone, Copy)]
pub struct Hooks {
    /// The column the stack stands in: its left and right edges, and the top it hangs under.
    pub column: fn(&Window, &App) -> (Pixels, Pixels, Pixels),
    /// A page is open whose own Escape goes back: the toasts take the key before it.
    pub page_open: fn(&Window, &App) -> bool,
    /// Something of the app's that reads no key context holds Escape: a layer over the window, a dialog.
    pub holds_escape: fn(&Window, &App) -> bool,
}

impl Global for Hooks {}

impl Default for Hooks {
    fn default() -> Self {
        Hooks { column: |window, _| (px(0.), window.viewport_size().width, px(0.)), page_open: |_, _| false, holds_escape: |_, _| false }
    }
}

/// Tells every window's stack where it stands and who else answers Escape.
pub fn set_hooks(hooks: Hooks, cx: &mut App) {
    cx.set_global(hooks);
}

fn hooks(cx: &App) -> Hooks {
    cx.try_global::<Hooks>().copied().unwrap_or_default()
}

/// Registers the stack for every window opened after it; `ui::init` calls it.
pub fn init(cx: &mut App) {
    Root::register_plugin::<Toasts>(cx, Toasts::new);
}

/// The window's stack, where its root was built after [`init`].
pub fn of(window: &Window, cx: &App) -> Option<Entity<Toasts>> {
    window.root::<Root>().flatten()?.read(cx).plugin::<Toasts>()
}

/// Says `notice` in the active window's toasts, once the update it is called from ends, so the window that update holds
/// is free to take it.
pub fn notify(cx: &mut App, notice: Notice) {
    cx.defer(move |cx| {
        let windows = cx.active_window().into_iter().chain(cx.windows());
        for handle in windows.collect::<Vec<_>>() {
            let notice = notice.clone();
            if handle.update(cx, |_, window, cx| of(window, cx).map(|_| notify_in(window, cx, notice))).ok().flatten().is_some() {
                return;
            }
        }
    });
}

/// Says `notice` in `window`'s toasts.
pub fn notify_in(window: &mut Window, cx: &mut App, notice: Notice) {
    let Some(toasts) = of(window, cx) else { return };
    toasts.update(cx, |t, cx| t.add(notice, cx));
}

/// The wait the notice under `key` stood for is over, in every window, once the update it is called from ends.
pub fn end_notice(cx: &mut App, key: &str) {
    let key = key.to_owned();
    cx.defer(move |cx| {
        for handle in cx.windows() {
            _ = handle.update(cx, |_, window, cx| {
                if let Some(toasts) = of(window, cx) {
                    toasts.update(cx, |t, cx| t.end(&key, cx));
                }
            });
        }
    });
}

pub struct Toasts {
    store: Store,
    pub(crate) stack: Stack,
    /// Whether the last Escape was taken before the window's own keys saw it, or held by a field, a dialog or a menu.
    escape_taken: bool,
    escape_owned: bool,
    /// What held the focus before a press on a toast took it (Sonner's `lastFocusedElementRef`).
    behind: Option<FocusHandle>,
    _wake: Option<Task<()>>,
    _keys: [Subscription; 2],
}

impl Toasts {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let handle = window.window_handle();
        let weak = cx.entity().downgrade();
        // Before the window's keys: a page's Escape goes back, and the toasts take it first, as the web's document
        // listener runs ahead of the shell's.
        let intercept = cx.intercept_keystrokes(move |e, window, cx| {
            if window.window_handle() != handle {
                return;
            }
            _ = weak.update(cx, |t, cx| {
                t.escape_taken = false;
                if e.keystroke.key != "escape" {
                    return;
                }
                t.escape_owned = owns_escape(&e.context_stack, window, cx);
                if (hooks(cx).page_open)(window, cx) && !t.escape_owned && t.escape(false, window, cx) {
                    t.escape_taken = true;
                    cx.stop_propagation();
                }
            });
        });
        // After them: an Escape nothing else took (`defaultPrevented`).
        let observe = cx.observe_keystrokes(move |t, e, window, cx| {
            if window.window_handle() != handle || e.keystroke.key != "escape" || e.action.is_some() || t.escape_taken {
                return;
            }
            let owned = t.escape_owned || owns_escape(&e.context_stack, window, cx);
            t.escape(owned, window, cx);
        });
        Toasts {
            store: Store::default(),
            stack: Stack::default(),
            escape_taken: false,
            escape_owned: false,
            behind: None,
            _wake: None,
            _keys: [intercept, observe],
        }
    }

    pub fn add(&mut self, notice: Notice, cx: &mut Context<Self>) {
        self.store.add(notice);
        self.changed(cx);
    }

    pub fn end(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.store.end(key) {
            self.changed(cx);
        }
    }

    /// Ends every wait whose key starts with `prefix`.
    pub fn end_prefixed(&mut self, prefix: &str, cx: &mut Context<Self>) {
        if self.store.end_prefixed(prefix) {
            self.changed(cx);
        }
    }

    /// Takes a toast off the screen; its row stays in the list.
    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.store.dismiss(id) {
            self.changed(cx);
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The notices whose toast stands, oldest first.
    pub fn standing(&self) -> Vec<Notice> {
        self.store.toasts().iter().filter_map(|id| self.store.get(*id)).map(|s| s.notice.clone()).collect()
    }

    fn now(cx: &App) -> Instant {
        cx.background_executor().now()
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let now = Self::now(cx);
        self.stack.prune(now);
        let standing: Vec<(u64, Notice)> =
            self.store.toasts().iter().filter_map(|id| self.store.get(*id)).map(|s| (s.id, s.notice.clone())).collect();
        self.stack.sync(&standing, now, cx);
        self.schedule(cx);
        cx.notify();
    }

    /// Wakes when the next timer runs out or the next leaving toast is done.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        let now = Self::now(cx);
        self._wake = self.stack.next_wake().map(|at| {
            let wait = at.saturating_duration_since(now);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                _ = this.update(cx, |t, cx| t.tick(cx));
            })
        });
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        for id in self.stack.expired(Self::now(cx)) {
            self.store.dismiss(id);
        }
        self.changed(cx);
    }

    /// Escape on the toasts: the focused one first, else the newest unless something else holds the key.
    fn escape(&mut self, owned: bool, window: &Window, cx: &mut Context<Self>) -> bool {
        let id = self.stack.focused(window, cx).or_else(|| if owned { None } else { self.store.toasts().last().copied() });
        let Some(id) = id else { return false };
        self.dismiss(id, cx);
        true
    }

    fn paused_by(&mut self, change: impl FnOnce(&mut Stack), cx: &mut Context<Self>) {
        let was = self.stack.paused();
        change(&mut self.stack);
        if was != self.stack.paused() {
            self.stack.set_paused(was, Self::now(cx));
            self.schedule(cx);
            cx.notify();
        }
    }

    pub(crate) fn pointer_moved(&mut self, at: gpui_kit::Point<Pixels>, over: bool, cx: &mut Context<Self>) {
        if let Some(swipe) = self.stack.swipe.as_mut() {
            swipe.moved(at);
            cx.notify();
        }
        if over != self.stack.hovered {
            self.paused_by(|s| s.hovered = over, cx);
            cx.notify();
        }
    }

    /// The pointer left the window: the stack comes together and its timers run again, as on `mouseleave`.
    pub(crate) fn pointer_left(&mut self, cx: &mut Context<Self>) {
        if self.stack.hovered {
            self.paused_by(|s| s.hovered = false, cx);
            cx.notify();
        }
    }

    pub(crate) fn press_held(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.stack.focused(window, cx).is_none() {
            self.behind = window.focused(cx);
        }
        self.paused_by(|s| s.interacting = true, cx);
    }

    /// Hands the focus back to what held it before a press, once the toast holding it leaves, as Sonner does.
    fn give_back(&mut self, window: &mut Window, cx: &mut App) {
        let Some(behind) = self.behind.clone() else { return };
        let left = self
            .stack
            .shown
            .iter()
            .enumerate()
            .any(|(index, s)| (s.removed.is_some() || index >= store::NOTICE_LIMIT) && s.focus.is_focused(window));
        if left {
            window.focus(&behind, cx);
            self.behind = None;
        }
    }

    pub(crate) fn press_swipe(&mut self, id: u64, at: gpui_kit::Point<Pixels>, cx: &mut Context<Self>) {
        let began = Self::now(cx);
        self.stack.swipe = Some(stack::Swipe { id, from: at, began, axis: None, amount: point(px(0.), px(0.)) });
    }

    pub(crate) fn released(&mut self, _at: gpui_kit::Point<Pixels>, cx: &mut Context<Self>) {
        let now = Self::now(cx);
        if let Some(swipe) = self.stack.swipe.take() {
            if let Some(axis) = swipe.dismisses(now) {
                self.stack.swiped_out(swipe.id, axis, now);
                self.store.dismiss(swipe.id);
                self.changed(cx);
            }
            cx.notify();
        }
        if self.stack.interacting {
            self.paused_by(|s| s.interacting = false, cx);
        }
    }
}

/// Whether a field, a dialog or a menu holds the focus, or something the app names does.
fn owns_escape(contexts: &[KeyContext], window: &Window, cx: &App) -> bool {
    contexts.iter().any(|c| OWNS_ESCAPE.iter().any(|owner| c.contains(owner))) || (hooks(cx).holds_escape)(window, cx)
}

impl RootPlugin for Toasts {}

impl Render for Toasts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.give_back(window, cx);
        if self.stack.shown.is_empty() {
            return div().into_any_element();
        }
        let (left, right, top) = (hooks(cx).column)(window, cx);
        let (x, width) = stack::place(left, right, window.viewport_size().width);
        self.draw_stack(point(x, top + stack::TOP), width, window, cx)
    }
}
