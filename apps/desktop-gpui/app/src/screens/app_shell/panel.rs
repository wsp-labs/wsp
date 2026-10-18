// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel: its tabs, the launcher it shows with none open, and its width. Whether it is open, its tabs and
//! the active one are kept per thread, as the web app's `rightPanelStore.ts` keeps them; its width is one value for
//! the run. Each tab's screen lives only while its thread is on screen and finds its session again by the tab's id.

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnchoredPositionMode, AnyView, App, Context, FocusHandle, Focusable, Hsla,
    KeyDownEvent, MouseButton, Pixels, ScrollHandle, SharedString, StyleRefinement, Window,
};

use super::{Entry, Host, Opened, Pane, Place, Title};
use crate::screens;
use crate::ui::card::keycap;
use crate::ui::{motion, ActivePalette};

/// `PreviewPanelShell`'s width: 540 at first, never under 360.
pub const DEFAULT_WIDTH: Pixels = px(540.);
pub const MIN_WIDTH: Pixels = px(360.);
/// The tab bar, the header row's `--workspace-topbar-height`.
pub const BAR_HEIGHT: Pixels = px(52.);
/// How long after the add menu closes on an outside press a click still belongs to that press.
const MENU_CLOSE: Duration = Duration::from_millis(500);
/// How many threads keep their panel's tabs; the oldest is forgotten past it, as `KEPT_THREAD_PANELS` in the web app.
const KEPT: usize = 100;

/// What the panel is open on, for one thread or for the computer itself: each tab's pane and id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Saved {
    pub open: bool,
    pub tabs: Vec<(Pane, SharedString)>,
    pub active: Option<usize>,
}

/// Where the panel is: a thread on a workspace that runs or not, or the computer with no thread picked.
#[derive(Clone, Debug, PartialEq)]
pub enum At {
    Here,
    Thread { workspace_id: String, thread_id: String, running: bool },
}

impl Pane {
    /// The launcher's line under the label.
    pub fn description(self) -> &'static str {
        match self {
            Pane::Browser => "Open your dev server or a URL.",
            Pane::Terminal => "Start a shell here.",
            Pane::Changes => "Review the changes here.",
            Pane::PullRequest => "Checks, review and comments.",
            Pane::Files => "Browse and read the files here.",
            Pane::Computer => "Load, memory and disk.",
            Pane::Processes => "Inspect and kill what runs.",
            Pane::Agents => "Agents, skills and servers here.",
            Pane::Aside => "A question the thread never keeps.",
            Pane::Slate => "What the agent built for this thread.",
        }
    }

    pub fn glyph(self) -> IconName {
        match self {
            Pane::Browser => IconName::Earth,
            Pane::Terminal => IconName::SquareTerminal,
            Pane::Changes => IconName::FileDiff,
            Pane::PullRequest => IconName::GitPullRequest,
            Pane::Files => IconName::Folder,
            Pane::Computer => IconName::Cpu,
            Pane::Processes => IconName::Activity,
            Pane::Agents => IconName::Bot,
            Pane::Aside => IconName::MessageSquareDashed,
            Pane::Slate => IconName::PanelsTopLeft,
        }
    }

    /// The name a screen mounted in this pane registers under in [`crate::screens::screen`].
    pub fn name(self) -> &'static str {
        match self {
            Pane::Browser => "browser",
            Pane::Terminal => "terminal",
            Pane::Changes => "changes",
            Pane::PullRequest => "pull-request",
            Pane::Files => "files",
            Pane::Computer => "computer",
            Pane::Processes => "processes",
            Pane::Agents => "agents",
            Pane::Aside => "aside",
            Pane::Slate => "slate",
        }
    }

    /// Listed by the launcher only while it can open: the pull request and the side question.
    fn only_when_available(self) -> bool {
        matches!(self, Pane::PullRequest | Pane::Aside)
    }

    /// Whether the pane opens here, and the line it carries when it does not, as `panes.ts` decides both.
    pub fn availability(self, at: &At) -> Result<(), &'static str> {
        let (here, running, thread) = match at {
            At::Here => (true, false, false),
            At::Thread { running, .. } => (false, *running, true),
        };
        let open = match self {
            Pane::Browser | Pane::Terminal | Pane::Processes => here || running,
            Pane::Changes | Pane::Files => running,
            Pane::Computer => here || thread,
            Pane::Agents | Pane::Slate => thread,
            Pane::PullRequest | Pane::Aside => false,
        };
        if open {
            return Ok(());
        }
        Err(match self {
            Pane::Changes if here => "Pick a project to review its changes.",
            Pane::Files if here => "Pick a thread to read its files.",
            Pane::Slate if !thread => "Pick a thread to see its slate.",
            Pane::Browser | Pane::Terminal | Pane::Files | Pane::Processes => "Available while this runs.",
            Pane::Changes => "Review the changes once it runs.",
            Pane::PullRequest => "Available once the work has a pull request.",
            Pane::Computer | Pane::Agents => "Pick a thread to use it.",
            Pane::Aside => "Ask one with /btw in the composer.",
            Pane::Slate => "The agent's panel for the open thread.",
        })
    }
}

struct Tab {
    pane: Pane,
    id: SharedString,
    /// The pane's screen, or None until a screen registers under the pane's name.
    view: Option<AnyView>,
    title: Option<Title>,
}

pub struct RightPanel {
    at: At,
    open: bool,
    tabs: Vec<Tab>,
    active: Option<usize>,
    width: Pixels,
    kept: Vec<(String, Saved)>,
    /// The last tab this panel numbered, so each Browser and Terminal opened from the launcher is a page or a shell of
    /// its own.
    minted: u64,
    strip: ScrollHandle,
    host: Option<Host>,
    /// The add menu under the bar's plus, open.
    adding: bool,
    closed_at: Option<Instant>,
    focus: FocusHandle,
    /// The host's agent writes, each of which opens its thread's Slate tab once.
    _slates: Option<gpui_kit::Task<()>>,
}

impl Focusable for RightPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RightPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        RightPanel {
            at: At::Here,
            open: false,
            tabs: Vec::new(),
            active: None,
            width: DEFAULT_WIDTH,
            kept: Vec::new(),
            minted: 0,
            strip: ScrollHandle::new(),
            host: None,
            adding: false,
            closed_at: None,
            focus: cx.focus_handle(),
            _slates: None,
        }
    }

    pub fn set_host(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        let (follow, mut writes) = screens::slate::agent_writes(&host);
        self._slates = Some(cx.spawn_in(window, async move |this, cx| {
            let _follow = follow;
            while let Some(thread) = futures::StreamExt::next(&mut writes).await {
                let on = |at: &At| matches!(at, At::Thread { thread_id, .. } if *thread_id == thread);
                if this
                    .update_in(cx, |this, window, cx| {
                        if on(&this.at) {
                            this.show_slate_once(window, cx)
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.host = Some(host);
        self.mount_active(window, cx);
        self.show_slate_once(window, cx);
        cx.notify();
    }

    /// Opens the Slate tab once on the thread here when its slate was never shown, as the web's `showOnce` does as a
    /// thread is picked and as its agent writes.
    fn show_slate_once(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (At::Thread { thread_id, .. }, Some(host)) = (&self.at, &self.host) else { return };
        let (thread, never) = (thread_id.clone(), screens::slate::never_shown(host, thread_id.clone()));
        cx.spawn_in(window, async move |this, cx| {
            if never.await {
                _ = this.update_in(cx, |this, window, cx| {
                    let here = matches!(&this.at, At::Thread { thread_id, .. } if *thread_id == thread);
                    if here && screens::slate::first_showing(&thread, cx) {
                        this.add(Pane::Slate, None, window, cx);
                    }
                });
            }
        })
        .detach();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn panes(&self) -> Vec<Pane> {
        self.tabs.iter().map(|t| t.pane).collect()
    }

    /// Each tab's pane and id, in the strip's order.
    pub fn tabs(&self) -> Vec<(Pane, SharedString)> {
        self.tabs.iter().map(|t| (t.pane, t.id.clone())).collect()
    }

    pub fn active(&self) -> Option<Pane> {
        self.active.map(|ix| self.tabs[ix].pane)
    }

    pub fn width(&self) -> Pixels {
        self.width
    }

    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.width = width;
        cx.notify();
    }

    fn saved(&self) -> Saved {
        Saved { open: self.open, tabs: self.tabs(), active: self.active }
    }

    /// The computer's own panel is kept under the empty key, which the cap does not count.
    fn key(at: &At) -> &str {
        match at {
            At::Here => "",
            At::Thread { thread_id, .. } => thread_id,
        }
    }

    /// Moves the panel to `at`, keeping what the last place had open and restoring what `at` had; a place it has not
    /// seen starts closed, as a thread's and the computer's do in the web app.
    pub fn show_for(&mut self, at: At, window: &mut Window, cx: &mut Context<Self>) {
        if Self::key(&at) == Self::key(&self.at) {
            self.at = at;
            cx.notify();
            return;
        }
        let key = Self::key(&self.at).to_owned();
        let saved = self.saved();
        self.kept.retain(|(k, _)| *k != key);
        // A panel left as a fresh one starts is not kept, as the web app drops an empty record.
        if saved != Saved::default() {
            self.kept.push((key, saved));
            if self.kept.iter().filter(|(k, _)| !k.is_empty()).count() > KEPT {
                if let Some(oldest) = self.kept.iter().position(|(k, _)| !k.is_empty()) {
                    self.kept.remove(oldest);
                }
            }
        }
        self.keep_focus(window, cx);
        let saved = self.kept.iter().find(|(k, _)| k == Self::key(&at)).map(|(_, s)| s.clone()).unwrap_or_default();
        self.at = at;
        self.open = saved.open;
        self.tabs = saved.tabs.into_iter().map(|(pane, id)| Tab { pane, id, view: None, title: None }).collect();
        self.active = saved.active.filter(|&ix| ix < self.tabs.len());
        self.mount_active(window, cx);
        self.show_slate_once(window, cx);
        cx.notify();
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    /// `toggle` of `rightPanelStore.ts`: closes the panel when it is open on `pane`, else opens `pane`, the browser on
    /// its first page or a new one, as `openedSurface` picks it.
    pub fn toggle_pane(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        if self.open && self.active() == Some(pane) {
            self.open = false;
            cx.notify();
        } else {
            let page = (pane == Pane::Browser)
                .then(|| self.tabs.iter().find(|t| t.pane == pane).map_or_else(|| "browser:new".into(), |t| t.id.clone()));
            self.add(pane, page, window, cx);
        }
    }

    /// Focus inside a tab's screen that is about to go stays in the panel, where the panel's own keys still hear it.
    fn keep_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus.contains_focused(window, cx) {
            self.focus.focus(window, cx);
        }
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        if self.open {
            self.mount_active(window, cx);
            // The launcher takes focus as it opens, so its letters answer at once.
            if self.active.is_none() {
                self.focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// A pane's letter typed while the launcher shows and focus is in the panel opens that pane, as the web app's
    /// launcher answers it; a letter with a modifier is a chord, not the launcher's.
    fn launch_by_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &event.keystroke.modifiers;
        if (self.active.is_some() && !self.adding) || m.control || m.alt || m.platform || m.function {
            return;
        }
        let Some(pane) = Pane::ALL.into_iter().find(|p| {
            p.key().is_some_and(|k| event.keystroke.key.eq_ignore_ascii_case(&k.to_string())) && p.availability(&self.at).is_ok()
        }) else {
            return;
        };
        cx.stop_propagation();
        self.add(pane, None, window, cx);
    }

    /// The id a tab opened from the launcher or the add menu takes, as the panes' openers make it: a new page for each
    /// Browser and a new shell for each Terminal, as `openNewBrowser` and `openPanelTerminal` do, the files tree, and
    /// the pane's own name for the rest.
    fn mint(&mut self, pane: Pane) -> SharedString {
        match pane {
            Pane::Browser | Pane::Terminal => loop {
                self.minted += 1;
                let id = SharedString::from(format!("{}:{}", pane.name(), self.minted));
                if !self.tabs.iter().any(|t| t.id == id) {
                    break id;
                }
            },
            Pane::Files => "files".into(),
            _ => pane.name().into(),
        }
    }

    /// Opens `pane` on the tab `id` names, or on the one [`Self::mint`] gives, moving to that tab when it is open: a
    /// tab is the same tab only when its pane and its id both are.
    pub fn add(&mut self, pane: Pane, id: Option<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.adding = false;
        let id = id.unwrap_or_else(|| self.mint(pane));
        let ix = match self.tabs.iter().position(|t| t.pane == pane && t.id == id) {
            Some(ix) => ix,
            None => {
                self.tabs.push(Tab { pane, id, view: None, title: None });
                self.tabs.len() - 1
            }
        };
        self.activate(ix, window, cx);
    }

    pub fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        self.active = Some(ix);
        self.strip.scroll_to_item(ix);
        self.mount_active(window, cx);
        cx.notify();
    }

    /// Moves by `step` tabs, wrapping, as `stepPanelTab` does.
    pub fn step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.tabs.len() as isize;
        if n == 0 {
            return;
        }
        let from = self.active.map_or(0, |ix| ix as isize);
        self.activate((from + step).rem_euclid(n) as usize, window, cx);
    }

    /// Closes the tab at `ix`; closing the last leaves the panel open on the launcher.
    pub fn close(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        self.keep_focus(window, cx);
        let tab = self.tabs.remove(ix);
        if let (Some(host), Some(entry)) = (&self.host, Self::entry(tab.pane)) {
            (entry.closed)(host, self.opened(&tab), cx);
        }
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > ix || a == self.tabs.len() => Some(a.saturating_sub(1)),
            other => other,
        };
        self.mount_active(window, cx);
        cx.notify();
    }

    /// The screen registered for `pane`'s tabs.
    fn entry(pane: Pane) -> Option<Entry> {
        screens::screen(pane.name()).filter(|e| e.place == Place::Panel(pane))
    }

    /// What `tab` is opened on: the panel's thread and workspace, and the tab's id.
    fn opened(&self, tab: &Tab) -> Opened {
        let (workspace_id, thread_id) = match &self.at {
            At::Here => (None, None),
            At::Thread { workspace_id, thread_id, .. } => (Some(workspace_id.clone()), Some(thread_id.clone())),
        };
        Opened { workspace_id, thread_id, tab: Some(tab.id.clone()), arg: None }
    }

    /// Builds the active tab's screen while the panel is open; the web app mounts no pane while its panel is shut.
    fn mount_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active.filter(|_| self.open) else { return };
        let (Some(host), Some(tab)) = (self.host.clone(), self.tabs.get(ix)) else { return };
        if tab.view.is_some() {
            return;
        }
        let Some(entry) = Self::entry(tab.pane) else { return };
        let mounted = (entry.mount)(&host, self.opened(tab), window, cx);
        let tab = &mut self.tabs[ix];
        tab.view = Some(mounted.view);
        tab.title = mounted.parts.title;
    }

    fn bar(&self, controls: impl IntoElement, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(ix, tab)| {
                let active = self.active == Some(ix);
                let label = tab.title.as_ref().map_or_else(|| SharedString::new_static(tab.pane.label()), |title| title(cx));
                h_flex()
                    .id(("panel-tab", ix))
                    .debug_selector(move || format!("panel-tab-{ix}"))
                    .group("panel-tab")
                    .flex_none()
                    .h(px(24.))
                    .max_w(px(144.))
                    .gap(px(2.))
                    .rounded(px(6.))
                    .pl(px(6.))
                    .pr(px(8.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .cursor_pointer()
                    .map(|el| {
                        if active {
                            el.bg(p.accent).text_color(p.foreground)
                        } else {
                            el.text_color(p.muted_foreground).hover(|s| s.bg(p.accent.opacity(0.6)).text_color(p.foreground))
                        }
                    })
                    .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _, window, cx| this.close(ix, window, cx)))
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
                    .child(
                        div()
                            .id(("panel-tab-close", ix))
                            .flex()
                            .flex_none()
                            .size(px(16.))
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .hover(|s| s.bg(p.muted))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(ix, window, cx)
                            }))
                            .child(div().group_hover("panel-tab", |s| s.invisible()).child(Icon::new(tab.pane.glyph()).with_size(px(12.))))
                            .child(
                                div()
                                    .absolute()
                                    .invisible()
                                    .group_hover("panel-tab", |s| s.visible())
                                    .child(Icon::new(IconName::X).with_size(px(12.))),
                            ),
                    )
                    .child(div().min_w_0().truncate().child(label))
            })
            .collect::<Vec<_>>();
        let add = (!self.tabs.is_empty()).then(|| {
            div()
                .relative()
                .flex_none()
                .child(
                    div()
                        .id("panel-add")
                        .flex()
                        .size(px(28.))
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.accent))
                        .child(Icon::new(IconName::Plus).with_size(px(14.)).text_color(p.muted_foreground))
                        .on_click(cx.listener(|this, _, window, cx| {
                            // A press on the plus while the menu is open has just closed it through its outside press.
                            if this.closed_at.take().is_some_and(|at| at.elapsed() < MENU_CLOSE) {
                                return;
                            }
                            this.adding = true;
                            this.focus.focus(window, cx);
                            cx.notify();
                        })),
                )
                .when(self.adding, |el| {
                    el.child(deferred(
                        anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(34.))).child(self.add_menu(cx)),
                    ))
                })
        });
        h_flex()
            .id("panel-bar")
            .debug_selector(|| "panel-bar".into())
            .flex_none()
            .h(BAR_HEIGHT)
            .items_center()
            .gap_1()
            .pl_2()
            .pr_3()
            .child(
                h_flex()
                    .id("panel-tabs")
                    .debug_selector(|| "panel-tabs".into())
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.strip)
                    .children(tabs)
                    .children(add),
            )
            .child(controls)
    }

    /// The plus's menu: each pane the launcher lists, with its glyph and letter, 176 px at the least.
    fn add_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let rows = Pane::ALL.into_iter().filter(|pane| !(pane.only_when_available() && pane.availability(&self.at).is_err())).map(|pane| {
            let open = pane.availability(&self.at).is_ok();
            h_flex()
                .id(SharedString::from(format!("add-{}", pane.name())))
                .h(px(32.))
                .px_2()
                .gap_2()
                .rounded(px(8.))
                .text_size(px(14.))
                .text_color(p.foreground)
                .when(!open, |el| el.opacity(0.64))
                .when(open, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(p.accent))
                        .on_click(cx.listener(move |this, _, window, cx| this.add(pane, None, window, cx)))
                })
                .child(Icon::new(pane.glyph()).with_size(px(16.)).text_color(p.muted_foreground))
                .child(div().flex_1().child(SharedString::new_static(pane.label())))
                .children(
                    pane.key().map(|k| div().text_size(px(12.)).text_color(p.muted_foreground).child(SharedString::from(k.to_string()))),
                )
        });
        v_flex()
            .id("panel-add-menu")
            .occlude()
            .min_w(px(176.))
            .p_1()
            .rounded(px(12.))
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.popover_shadow.clone())
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.adding = false;
                this.closed_at = Some(Instant::now());
                cx.notify();
            }))
            .children(rows)
    }

    fn launcher(&self, (card, edge): (Hsla, Hsla), cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let tiles = Pane::ALL.into_iter().filter_map(|pane| {
            let open = pane.availability(&self.at);
            if pane.only_when_available() && open.is_err() {
                return None;
            }
            let line = SharedString::new_static(match open {
                Ok(()) => pane.description(),
                Err(reason) => reason,
            });
            let dim = if open.is_ok() { 1. } else { 0.64 };
            let key = pane.key().map(|k| SharedString::from(k.to_string()));
            let tile = h_flex()
                .id(SharedString::new_static(pane.name()))
                .w_full()
                .h(px(54.))
                .gap_3()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .border_1()
                .border_color(edge)
                .when(open.is_ok(), |el| {
                    el.cursor_pointer().on_click(cx.listener(move |this, _, window, cx| this.add(pane, None, window, cx)))
                })
                .child(Icon::new(pane.glyph()).with_size(px(16.)).text_color(p.foreground.opacity(0.8)).opacity(dim))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .text_color(p.foreground)
                                .opacity(dim)
                                .child(SharedString::new_static(pane.label())),
                        )
                        .child(div().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(line)),
                )
                .children(key.map(|k| div().opacity(dim).child(keycap(k, cx))));
            Some(Tile { name: pane.name(), rest: card, hover: open.is_ok().then_some(p.card_hover), tile })
        });
        div().flex().flex_1().min_h_0().items_center().justify_center().px_6().pt_6().pb(BAR_HEIGHT + px(24.)).child(
            div()
                .relative()
                .debug_selector(|| "panel-launcher".into())
                .w_full()
                .max_w(px(384.))
                .child(
                    v_flex()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_full()
                        .mb_5()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child("Open a panel"),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(p.muted_foreground)
                                .child("A browser, a terminal, the changes, the files, the computer or what runs on it."),
                        ),
                )
                .child(v_flex().gap_2().children(tiles)),
        )
    }

    /// The panel's bar and content, with `controls` at the bar's right end.
    /// `ink` is the card fill and the line the panel's surface takes, which the Mac's glass moves.
    pub fn draw(&mut self, controls: impl IntoElement, ink: (Hsla, Hsla), cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.active.and_then(|ix| self.tabs.get(ix)) {
            Some(Tab { view: Some(view), .. }) => {
                div().flex_1().min_h_0().child(view.clone().cached(StyleRefinement::default().size_full())).into_any_element()
            }
            Some(_) => v_flex()
                .debug_selector(|| "pane-empty".into())
                .flex_1()
                .items_center()
                .justify_center()
                .p_6()
                .text_center()
                .child(div().text_size(px(13.)).line_height(px(20.)).text_color(cx.palette().muted_foreground).child("Nothing open here."))
                .child(
                    div()
                        .mt_1()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(cx.palette().muted_foreground)
                        .child("Pick a panel from the tab strip above."),
                )
                .into_any_element(),
            None => self.launcher(ink, cx).into_any_element(),
        };
        v_flex()
            .track_focus(&self.focus)
            .when(self.tabs.len() > 1, |el| el.key_context(super::keys::PANEL_TABS))
            .on_key_down(cx.listener(Self::launch_by_key))
            .size_full()
            .child(self.bar(controls, cx))
            .child(content)
    }
}

/// A launcher tile, its fill eased to the hover's over `COLORS` as `transition-colors duration-150` does.
#[derive(IntoElement)]
struct Tile {
    name: &'static str,
    rest: Hsla,
    hover: Option<Hsla>,
    tile: gpui_kit::Stateful<gpui_kit::Div>,
}

impl RenderOnce for Tile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let over = motion::hovered(self.name, window, cx);
        let fill = self.hover.filter(|_| *over.read(cx)).unwrap_or(self.rest);
        let fill = motion::ease_ink(self.name, "fill", fill, motion::COLORS, window, cx);
        self.tile.bg(fill).when(self.hover.is_some(), |el| el.on_hover(motion::track_hover(over)))
    }
}
