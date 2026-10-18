// SPDX-License-Identifier: AGPL-3.0-only
//! The thread switcher, after `components/switcher/WorkspaceSwitcher.tsx` and the hold `shell/KeybindingDispatcher.tsx`
//! keeps: the switch chord's first step puts up one card per thread it walks, the open one and the last opened first,
//! and every step after moves the highlight; letting the chord's modifier go opens the highlighted thread, Escape or
//! the window losing focus leaves the person where they were. Only the paint waits out [`PAINT_DELAY`], so a tap
//! switches without ever dimming the window. The cards read the sidebar's threads as it follows the host, and the
//! switcher keeps nothing of its own past the cards' ids and, per window, the threads last opened.

pub(crate) mod walk;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::base::input::{AddCursorAbove, AddCursorBelow};
use gpui_kit::base::Root;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, Action as _, AnyElement, App, Context, ElementId, Entity, EntityId, FocusHandle, Global, Hsla, KeyBinding,
    KeyContext, KeyDownEvent, Keystroke, Modifiers, ModifiersChangedEvent, SharedString, Subscription, Task, Unbind, WeakEntity, Window,
};

pub use walk::Target;

use super::{frame, overlays};
use crate::screens::app_shell::keys;
use crate::screens::sidebar::{looks, model, tile, Sidebar, NO_PREFERENCES};
use crate::screens::Route;
use crate::shell::Shell;
use crate::ui::{catalog, menu, motion, text, ActivePalette, StatusMark};

/// `SWITCHER_PAINT_DELAY_MS`: a chord let go inside it switches without the overlay ever drawing.
pub const PAINT_DELAY: Duration = Duration::from_millis(150);
/// The card's `w-56`.
const CARD_WIDTH: f32 = 224.;

/// Which list a step walks: the sidebar's tasks, or the threads of the task on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Workspace,
    Thread,
}

/// The threads each window last opened, newest first, by its shell.
#[derive(Default)]
struct Visits(HashMap<EntityId, VecDeque<Target>>);

impl Global for Visits {}

pub struct Switcher {
    cards: Vec<Target>,
    at: usize,
    /// The modifiers the chord that opened it holds; letting one of them go commits the walk.
    hold: Modifiers,
    /// The key contexts focus stood in when it opened, which every chord but Escape is read against.
    contexts: Vec<KeyContext>,
    painted: bool,
    focus: FocusHandle,
    sidebar: WeakEntity<Sidebar>,
    _paint: Task<()>,
    _subscriptions: Vec<Subscription>,
}

/// The window's shell, without reading it: a step runs inside the shell's own action.
fn shell_of(window: &Window, cx: &App) -> Option<Entity<Shell>> {
    window.root::<Root>().flatten()?.read(cx).view().clone().downcast::<Shell>().ok()
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |d| d.as_secs_f64() * 1000.)
}

/// Starts noting each thread the window opens, for the switcher's order; once per window, from when the host is
/// attached. A step starts it too where nothing did.
pub fn follow(window: &Window, cx: &mut App) {
    let Some(shell) = shell_of(window, cx) else { return };
    let id = shell.entity_id();
    if cx.default_global::<Visits>().0.contains_key(&id) {
        return;
    }
    cx.default_global::<Visits>().0.insert(id, VecDeque::new());
    let noted = shell.clone();
    cx.defer(move |cx| note(&noted, cx));
    cx.observe(&shell, |shell, cx| note(&shell, cx)).detach();
    cx.observe_release(&shell, move |_, cx| {
        cx.default_global::<Visits>().0.remove(&id);
    })
    .detach();
}

fn note(shell: &Entity<Shell>, cx: &mut App) {
    let Route::Thread { workspace_id, thread_id } = shell.read(cx).route().clone() else { return };
    let visits = cx.default_global::<Visits>().0.entry(shell.entity_id()).or_default();
    if visits.front().is_none_or(|v| v.thread_id != thread_id || v.workspace_id != workspace_id) {
        walk::visit(visits, Target { workspace_id, thread_id });
    }
}

/// A step from a held key: opens the switcher on the next card, or moves it along while it stands open.
pub fn step(level: Level, by: isize, window: &mut Window, cx: &mut App) {
    let Some(overlays) = overlays::of(window, cx) else { return };
    if let Some(open) = overlays.read(cx).switcher.clone() {
        open.update(cx, |s, cx| s.step(by, cx));
        return;
    }
    let hold = held(window.modifiers());
    // The shell is mid-update while its action runs, so what it shows is read once that update ends.
    window.defer(cx, move |window, cx| open(level, by, hold, window, cx));
}

/// Takes gpui-kit's add-a-cursor chords off a field where the thread chords lie (mod+alt+up and down on a Mac and on
/// Windows), so the switcher steps with the composer focused, as the web's does; once, after `gpui_kit::init`.
pub(super) fn init(cx: &mut App) {
    let mac = cfg!(target_os = "macos");
    let unbind = [("thread.previous", AddCursorAbove.name()), ("thread.next", AddCursorBelow.name())];
    let bindings = keys::DEFAULTS.iter().flat_map(|rule| {
        let (_, action) = unbind.iter().find(|(command, _)| *command == rule.command)?;
        Some(KeyBinding::new(&keys::keystroke(rule.chord, mac)?, Unbind((*action).into()), Some("Input")))
    });
    cx.bind_keys(bindings.collect::<Vec<_>>());
}

/// The modifiers a chord holds that end a walk when let go: shift steps back and never commits.
fn held(m: Modifiers) -> Modifiers {
    Modifiers { control: m.control, alt: m.alt, platform: m.platform, ..Modifiers::default() }
}

fn open(level: Level, by: isize, hold: Modifiers, window: &mut Window, cx: &mut App) {
    let (Some(overlays), Some(shell)) = (overlays::of(window, cx), shell_of(window, cx)) else { return };
    if overlays.read(cx).switcher.is_some() {
        return;
    }
    follow(window, cx);
    note(&shell, cx);
    let (sidebar, open) = {
        let shell = shell.read(cx);
        let open = match shell.route() {
            Route::Thread { workspace_id, thread_id } => Some((workspace_id.clone(), thread_id.clone())),
            _ => None,
        };
        (shell.sidebar().clone(), open)
    };
    let open_thread = open.as_ref().map(|(_, t)| t.as_str());
    let walked = {
        let s = sidebar.read(cx);
        match (level, &open) {
            (Level::Workspace, _) => walk::live(s.threads(), s.prefs(), now_ms(), open_thread),
            (Level::Thread, Some((workspace, _))) => walk::thread_walk(s, workspace),
            (Level::Thread, None) => Vec::new(),
        }
    };
    let visits = cx.default_global::<Visits>().0.get(&shell.entity_id()).cloned().unwrap_or_default();
    let cards = walk::recent(&walked, &visits, open_thread);
    let Some(at) = walk::opening(&cards, open_thread, by) else { return };
    overlays.update(cx, |o, cx| {
        o.take_focus(window, cx);
        let contexts = window.context_stack();
        let switcher = cx.new(|cx| Switcher::new(cards, at, hold, contexts, &sidebar, window, cx));
        let focus = switcher.read(cx).focus.clone();
        window.focus(&focus, cx);
        o.switcher = Some(switcher);
        cx.notify();
    });
}

/// Takes the switcher down and hands focus back where it stood; the switcher may be the one asking.
fn close(window: &mut Window, cx: &mut App) {
    let Some(overlays) = overlays::of(window, cx) else { return };
    overlays.update(cx, |o, cx| {
        if o.switcher.take().is_some() {
            // A palette opened over the switcher holds the keys now, and gives them back when it shuts.
            if o.palette.is_none() {
                o.give_back(window, cx);
            }
            cx.notify();
        }
    });
}

impl Switcher {
    fn new(
        cards: Vec<Target>,
        at: usize,
        hold: Modifiers,
        contexts: Vec<KeyContext>,
        sidebar: &Entity<Sidebar>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let paint = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PAINT_DELAY).await;
            _ = this.update(cx, |this, cx| {
                this.painted = true;
                cx.notify();
            });
        });
        // With focus elsewhere the key up never reaches the switcher; a window that loses focus blurs it too, as GPUI
        // reads an inactive window's focus path as empty, so a chord that takes the window away lands here.
        let subscriptions = vec![cx.on_blur(&focus, window, |_, window, cx| close(window, cx))];
        Switcher {
            cards,
            at,
            hold,
            contexts,
            painted: false,
            focus,
            sidebar: sidebar.downgrade(),
            _paint: paint,
            _subscriptions: subscriptions,
        }
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        self.at = walk::stepped(self.cards.len(), self.at, by);
        cx.notify();
    }

    /// The card the highlight stands on.
    pub fn highlighted(&self) -> Option<&Target> {
        self.cards.get(self.at)
    }

    pub fn cards(&self) -> &[Target] {
        &self.cards
    }

    fn released(&self, now: Modifiers) -> bool {
        (self.hold.control && !now.control) || (self.hold.alt && !now.alt) || (self.hold.platform && !now.platform)
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.highlighted().cloned();
        close(window, cx);
        if let (Some(Target { workspace_id, thread_id }), Some(shell)) = (target, shell_of(window, cx)) {
            shell.update(cx, |s, cx| s.go(Route::Thread { workspace_id, thread_id }, window, cx));
        }
    }

    /// Runs the chord `keystroke` is bound to where focus stood before the switcher, as the web leaves focus there:
    /// the switch chord steps the walk, any other does what it does under the switcher. Read against the keys' places
    /// as they stood when it opened, so a chord bound in a terminal or the thread view resolves as it would there.
    fn forward(&self, keystroke: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let (bindings, _) = cx.key_bindings().borrow().bindings_for_input(std::slice::from_ref(keystroke), &self.contexts);
        let Some(action) = bindings.first().map(|b| b.action().boxed_clone()) else { return };
        let Some(behind) = overlays::of(window, cx).and_then(|o| o.read(cx).behind.clone()) else { return };
        window.defer(cx, move |window, cx| behind.dispatch_action(action.as_ref(), window, cx));
    }

    /// The cards as they stand now, out of the sidebar's threads; a thread that has since gone leaves no card.
    pub fn shown(&self, cx: &App) -> Vec<Card> {
        let Some(sidebar) = self.sidebar.upgrade() else { return Vec::new() };
        let sidebar = sidebar.read(cx);
        let now = now_ms();
        self.cards.iter().filter_map(|target| Card::of(target, sidebar, now)).collect()
    }
}

/// What one card says of its thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub thread_id: String,
    pub title: String,
    pub harness: String,
    /// The computer the thread's workspace stands on.
    pub place: String,
    pub project_id: Option<String>,
    pub kind: model::Kind,
    /// A working thread's running time, a resting one's age.
    pub time: Option<String>,
    /// The state's word on the mark's hover; a resting thread has none.
    pub word: String,
}

impl Card {
    fn of(target: &Target, sidebar: &Sidebar, now: f64) -> Option<Card> {
        let t = sidebar.threads().iter().find(|t| t.key == target.thread_id)?;
        let kind = model::kind_of(t, now);
        let around = sidebar.around();
        let place = sidebar
            .place_of(&t.workspace_id)
            .and_then(|id| around.places.iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some(id.as_str())))
            .map(looks::place_name)
            .or_else(|| t.computer.clone())
            .unwrap_or_default();
        Some(Card {
            thread_id: t.key.clone(),
            title: t.title.clone(),
            harness: t.harness.clone(),
            place,
            project_id: t.project_id.clone(),
            kind,
            time: model::slot(t, kind, now, t.ended_at.or(t.started_at)).text,
            word: model::word(t, kind, now, crate::screens::sidebar::utc_offset_ms() as f64).into_owned(),
        })
    }

    /// `SwitcherCardView`: the well with the project's glyph, the title, and the agent, computer and state under it.
    fn draw(self, fill: Hsla, sidebar: &Sidebar, cx: &App) -> AnyElement {
        let p = cx.palette();
        let around = sidebar.around();
        let preferences = around.preferences.as_deref().unwrap_or(&NO_PREFERENCES);
        let glyph = looks::project_glyph(self.project_id.as_deref(), preferences, &around.images, px(24.), p.muted_foreground, true, p);
        let agent = (!self.harness.is_empty())
            .then(|| catalog::mark(&self.harness, &catalog::name(&self.harness), px(12.), p.muted_foreground, cx));
        let mark = StatusMark::new(tile::status_of(self.kind, self.time))
            .rest_ink(p.muted_foreground)
            .key(SharedString::from(format!("switcher-crab-{}", self.thread_id)));
        let mark = if self.word.is_empty() { mark } else { mark.tooltip(self.word) };
        let id = card_id(&self.thread_id);
        // An empty well wears a hairline drawn inside its box, which reads on a light card where the muted fill does not.
        let well = div()
            .mb_1()
            .h(px(88.))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(frame::RADIUS_SM)
            .bg(p.muted.opacity(0.4))
            .border_1()
            .border_color(p.border)
            .child(glyph);
        let line = h_flex()
            .min_w_0()
            .items_center()
            .gap_2()
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.muted_foreground)
            .children(agent)
            .child(div().flex_1().min_w_0().truncate().child(self.place))
            .child(mark);
        v_flex()
            .id(id.clone())
            .debug_selector(move || id.to_string())
            .w(px(CARD_WIDTH))
            .min_w_0()
            .gap_1()
            .p_2()
            .rounded(frame::RADIUS_MD)
            .bg(fill)
            .child(well)
            .child(
                div()
                    .truncate()
                    .text_size(text::SETTING_TITLE.0)
                    .line_height(text::SETTING_TITLE.1)
                    .text_color(p.foreground)
                    .child(self.title),
            )
            .child(line)
            .into_any_element()
    }
}

fn card_id(thread_id: &str) -> SharedString {
    format!("switcher-card-{thread_id}").into()
}

impl Render for Switcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layer = if self.painted {
            let p = cx.palette().clone();
            let highlighted = self.highlighted().map(|t| t.thread_id.clone());
            let shown = self.shown(cx);
            let fills: Vec<Hsla> = shown
                .iter()
                .map(|card| {
                    let fill = if highlighted.as_ref() == Some(&card.thread_id) {
                        p.foreground.opacity(0.09)
                    } else {
                        gpui_kit::transparent_black()
                    };
                    motion::ease_ink(ElementId::Name(card_id(&card.thread_id)), "fill", fill, motion::COLORS, window, cx)
                })
                .collect();
            let cards: Vec<AnyElement> = match self.sidebar.upgrade() {
                Some(sidebar) => {
                    let sidebar = sidebar.read(cx);
                    shown.into_iter().zip(fills).map(|(card, fill)| card.draw(fill, sidebar, cx)).collect()
                }
                None => Vec::new(),
            };
            let row = menu::glass(h_flex(), &p)
                .id("switcher")
                .debug_selector(|| "switcher".into())
                .max_w(window.viewport_size().width - px(64.))
                .gap_2()
                .p_2()
                .rounded(px(10.))
                .text_color(p.foreground)
                .font_family(cx.theme().font_family.clone())
                .children(cards);
            frame::scrim("switcher-scrim", cx).flex().items_center().justify_center().child(row)
        } else {
            div().id("switcher-scrim")
        };
        layer
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                cx.stop_propagation();
                if e.keystroke.key == "escape" {
                    close(window, cx);
                } else {
                    this.forward(&e.keystroke, window, cx);
                }
            }))
            .on_modifiers_changed(cx.listener(|this, e: &ModifiersChangedEvent, window, cx| {
                if this.released(e.modifiers) {
                    this.commit(window, cx);
                }
            }))
    }
}
