// SPDX-License-Identifier: AGPL-3.0-only
//! The page: the system's web view (WebKitGTK on Linux, WKWebView on the Mac) as a child of the window, laid over the
//! slot the pane leaves for it. GPUI draws under it, so whatever a frame draws over the slot (a menu, a tooltip, a
//! notice) is cut out of the page, and a press or a chord the page takes is handed back to GPUI where the app owns it.

#[cfg(target_os = "linux")]
mod glib_loop;
pub mod keys;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod mac;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{
    canvas, div, prelude::*, AnyWindowHandle, App, Bounds, Context, DispatchPhase, Entity, EventEmitter, FocusHandle, Global,
    HitboxBehavior, HitboxId, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PlatformInput, Point, Subscription, Task,
    Window,
};
use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use mac as native;

/// What the page says about itself, from the web view's own callbacks.
#[derive(Clone, Debug, PartialEq)]
pub enum PageEvent {
    Loading(bool),
    /// The main frame is somewhere else now: a load, a link, its own history, a hash or `pushState`.
    Moved(String),
    Titled(String),
    /// The page asked for a new window on this http(s) address.
    NewWindow(String),
    /// A link to another app, which asks before it leaves.
    Leave(String),
    /// A press on the page: it holds the keys from now on.
    Focused,
    /// The page's web process ended under it.
    Gone(String),
    /// The page came on screen, or left it.
    Shown(bool),
}

/// What the web view's callbacks send to the page's task; they run inside the web view's own loop.
#[derive(Debug)]
pub(crate) enum Raw {
    Load(bool),
    /// The page's script says it moved in place; the address is read off the web view, never taken from the page.
    MovedInPlace,
    Titled(String),
    NewWindow(String),
    Leave(String),
    /// A press on the page, in its own coordinates.
    Pressed(Point<Pixels>),
    Gone(String),
}

/// Where a page can be held in this window: in it, or nowhere, with the line that says why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Road {
    Here,
    SystemBrowser(&'static str),
}

/// What the app does on a Wayland session, where neither WebKitGTK nor wry embeds a page in a Wayland window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnWayland {
    /// The window stays native and each page opens in the system browser.
    SystemBrowser,
    /// The whole app runs on XWayland, so its pages are held in the pane: [`application`] asks GPUI for X11.
    XWayland,
}

pub const ON_WAYLAND: OnWayland = OnWayland::SystemBrowser;

/// The line the pane shows where its pages open in the system browser.
pub const WAYLAND_LINE: &str = "This window runs on Wayland, where wsp cannot hold a page, so pages open in your browser.";
/// The same where the system has no web view wsp embeds.
pub const ELSEWHERE_LINE: &str = "wsp cannot hold a page on this system, so pages open in your browser.";

/// The platform the app opens on, X11 alone where its pages are held on XWayland.
pub fn application() -> gpui_kit::Application {
    #[cfg(target_os = "linux")]
    if ON_WAYLAND == OnWayland::XWayland {
        return gpui_kit::platform::linux(gpui_kit::WindowingModes::X11);
    }
    gpui_kit::application()
}

/// Where `window` can hold a page.
pub fn road(window: &Window) -> Road {
    match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) | RawWindowHandle::AppKit(_)) => Road::Here,
        Ok(RawWindowHandle::Wayland(_)) => Road::SystemBrowser(WAYLAND_LINE),
        _ => Road::SystemBrowser(ELSEWHERE_LINE),
    }
}

/// Where the pages keep their cookies and storage: one store for every tab, as today's `persist:wsp-browser`, in the
/// app's data folder beside its local store; WKWebView keeps the app's own default store. With none set, as in a
/// check run, a page keeps nothing past itself.
#[derive(Clone)]
pub struct DataDir(pub PathBuf);

impl Global for DataDir {}

/// The web view, its place in the window and what it has said.
pub struct Page {
    native: native::Native,
    window: AnyWindowHandle,
    /// The slot's hitbox in the frame that drew it last, set as the slot lays itself out.
    hitbox: Rc<Cell<Option<HitboxId>>>,
    /// Where it stands in window pixels, and what is cut out of it, as the last frame left them; None while hidden.
    at: Option<(Bounds<Pixels>, Vec<Bounds<Pixels>>)>,
    scale: f32,
    zoom: f32,
    pub loading: bool,
    pub url: String,
    pub title: String,
    /// Shown in its slot by the pane; a page told to stay hidden stays so whatever its slot does.
    held_hidden: bool,
    focus: FocusHandle,
    _drawn: Subscription,
    _raised: Subscription,
    _events: Task<()>,
}

impl EventEmitter<PageEvent> for Page {}

impl Page {
    /// Builds the page on `url` in `window`, hidden until its slot is drawn. Fails where the window holds no page.
    pub fn new(url: &str, zoom: f32, window: &mut Window, cx: &mut App) -> Result<Entity<Page>, String> {
        if let Road::SystemBrowser(why) = road(window) {
            return Err(why.to_owned());
        }
        let dir = cx.try_global::<DataDir>().map(|d| d.0.clone());
        let (tx, mut rx) = mpsc::unbounded::<Raw>();
        let native = native::Native::new(url, dir, tx, window, cx)?;
        let handle = Window::window_handle(window);
        let scale = window.scale_factor();
        let page = cx.new(|cx: &mut Context<Page>| {
            let hitbox: Rc<Cell<Option<HitboxId>>> = Rc::default();
            let this = cx.entity().downgrade();
            let drawn = window.on_drawn(move |window, cx| this.update(cx, |page, cx| page.drawn(window, cx)).is_ok());
            // The window manager hands a window it raises back its own focus; a page holding the keys takes them again.
            let raised = cx.observe_window_activation(window, |page: &mut Page, window, _| {
                if window.is_window_active() && page.native.focused() {
                    page.native.take_focus();
                }
            });
            let events = cx.spawn(async move |this, cx| {
                while let Some(raw) = rx.next().await {
                    if this.update(cx, |page, cx| page.raw(raw, cx)).is_err() {
                        break;
                    }
                }
            });
            Page {
                native,
                window: handle,
                hitbox,
                at: None,
                scale,
                zoom,
                loading: true,
                url: url.to_owned(),
                title: String::new(),
                held_hidden: false,
                focus: cx.focus_handle(),
                _drawn: drawn,
                _raised: raised,
                _events: events,
            }
        });
        page.update(cx, |p, _| p.native.set_zoom(zoom, scale));
        Ok(page)
    }

    /// The slot the pane leaves for the page: white, as a page's ground is, so a corner a cut leaves square reads as
    /// the page. A press replayed from the page lands here and focuses the pane.
    pub fn slot(page: &Entity<Page>, cx: &App) -> impl IntoElement {
        let hitbox = page.read(cx).hitbox.clone();
        let this = page.downgrade();
        div().id("page-slot").track_focus(&page.read(cx).focus).flex_1().min_h_0().bg(gpui_kit::white()).child(
            canvas(
                move |bounds, window, _| {
                    let inserted = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    hitbox.set(Some(inserted.id));
                },
                move |_, _, window, _| {
                    let this = this.clone();
                    // A press anywhere the page does not show takes the keys back for the app, focusable or not.
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture {
                            _ = this.update(cx, |page, _| {
                                if page.native.focused() && !page.shows(event.position) {
                                    page.native.release_focus();
                                }
                            });
                        }
                    });
                },
            )
            .size_full(),
        )
    }

    /// Whether the page shows at `at`, in window pixels: inside where it stands and outside what is cut from it.
    pub fn shows(&self, at: Point<Pixels>) -> bool {
        self.at.as_ref().is_some_and(|(bounds, holes)| bounds.contains(&at) && !holes.iter().any(|h| h.contains(&at)))
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The window the page is a child of, which it lives and dies with.
    pub fn window_id(&self) -> gpui_kit::WindowId {
        self.window.window_id()
    }

    /// After each frame: the page follows its slot, hides when the frame drew none, and cuts out what it drew over it.
    fn drawn(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A focus the page held goes back to the window once the pane's own focus moves elsewhere.
        if self.native.focused() && !self.focus.contains_focused(window, cx) {
            self.native.release_focus();
        }
        let at = if self.held_hidden { None } else { self.hitbox.get().and_then(|id| window.drawn_over(id)) };
        let scale = window.scale_factor();
        if at == self.at && scale == self.scale {
            return;
        }
        if scale != self.scale {
            self.scale = scale;
            self.native.set_zoom(self.zoom, scale);
        }
        match &at {
            Some((bounds, holes)) => self.native.show(*bounds, holes, scale),
            None => {
                self.native.hide();
                if self.native.focused() {
                    self.native.release_focus();
                }
            }
        }
        if at.is_some() != self.at.is_some() {
            cx.emit(PageEvent::Shown(at.is_some()));
        }
        self.at = at;
    }

    fn raw(&mut self, raw: Raw, cx: &mut Context<Self>) {
        match raw {
            Raw::Load(loading) => {
                self.loading = loading;
                if let Some(url) = self.native.url() {
                    self.moved(url, cx);
                }
                cx.emit(PageEvent::Loading(loading));
            }
            Raw::MovedInPlace => {
                if let Some(url) = self.native.url() {
                    self.moved(url, cx);
                }
            }
            Raw::Titled(title) => {
                self.title = title.clone();
                cx.emit(PageEvent::Titled(title));
            }
            Raw::NewWindow(url) => cx.emit(PageEvent::NewWindow(url)),
            Raw::Leave(url) => cx.emit(PageEvent::Leave(url)),
            Raw::Gone(why) => {
                let why = if why.is_empty() { format!("renderer gone at {}", self.url) } else { why };
                eprintln!("desktop-gpui: browser: {why}");
                self.loading = false;
                cx.emit(PageEvent::Gone(why));
            }
            Raw::Pressed(at) => {
                let Some((bounds, _)) = self.at else { return };
                let position = bounds.origin + at;
                let (focus, handle) = (self.focus.clone(), self.window);
                self.native.take_focus();
                _ = handle.update(cx, |_, window, cx| window.focus(&focus, cx));
                // Replayed once this update ends, since the press reaches this page's own listeners.
                cx.defer(move |cx| {
                    _ = handle.update(cx, |_, window, cx| {
                        replay_press(position, window, cx);
                        if !focus.contains_focused(window, cx) {
                            window.focus(&focus, cx);
                        }
                    });
                });
                cx.emit(PageEvent::Focused);
            }
        }
        cx.notify();
    }

    fn moved(&mut self, url: String, cx: &mut Context<Self>) {
        if url != self.url && !url.is_empty() {
            self.url = url.clone();
            cx.emit(PageEvent::Moved(url));
        }
    }

    pub fn load(&mut self, url: &str) {
        self.native.load(url);
    }

    pub fn reload(&mut self) {
        self.native.reload();
    }

    pub fn stop(&mut self) {
        self.native.stop();
    }

    pub fn back(&mut self) {
        self.native.back();
    }

    pub fn forward(&mut self) {
        self.native.forward();
    }

    pub fn can_go_back(&self) -> bool {
        self.native.can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.native.can_go_forward()
    }

    /// Per tab, 50% to 200%, on top of the window's own scale where the web view does not follow it.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
        self.native.set_zoom(zoom, self.scale);
    }

    /// Kept hidden whatever its slot does, as a tab whose place is the servers list keeps its page for Forward.
    pub fn hold_hidden(&mut self, hidden: bool, _cx: &mut Context<Self>) {
        self.held_hidden = hidden;
    }

    /// Where the page stands and what is cut out of it, in window pixels, as the last frame left them.
    pub fn placed(&self) -> Option<&(Bounds<Pixels>, Vec<Bounds<Pixels>>)> {
        self.at.as_ref()
    }

    /// Runs `script` in the page; the checks read their answers off the page's own reports.
    pub fn run_script(&self, script: &str) {
        self.native.run_script(script);
    }

    pub(crate) fn native(&self) -> &native::Native {
        &self.native
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.native.hide();
    }
}

/// A press the page took, as GPUI would have had it: an open menu closes, the slot takes focus.
fn replay_press(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let modifiers = Modifiers::default();
    let down = MouseDownEvent { button: MouseButton::Left, position, modifiers, click_count: 1, first_mouse: false };
    window.dispatch_event(PlatformInput::MouseDown(down), cx);
    let up = MouseUpEvent { button: MouseButton::Left, position, modifiers, click_count: 1 };
    window.dispatch_event(PlatformInput::MouseUp(up), cx);
}

/// A window-pixel rectangle in device pixels, edges rounded half up so neighbours share an edge at a fractional scale.
pub fn device(bounds: Bounds<Pixels>, scale: f32) -> (i32, i32, i32, i32) {
    let edge = |v: Pixels| (f32::from(v) * scale + 0.5).floor() as i32;
    let (left, top) = (edge(bounds.origin.x), edge(bounds.origin.y));
    let (right, bottom) = (edge(bounds.origin.x + bounds.size.width), edge(bounds.origin.y + bounds.size.height));
    (left, top, (right - left).max(0), (bottom - top).max(0))
}

/// `holes` in device pixels relative to the page at `at`.
pub fn device_holes(at: Bounds<Pixels>, holes: &[Bounds<Pixels>], scale: f32) -> Vec<(i32, i32, i32, i32)> {
    let (left, top, _, _) = device(at, scale);
    holes
        .iter()
        .map(|hole| {
            let (x, y, w, h) = device(*hole, scale);
            (x - left, y - top, w, h)
        })
        .collect()
}

/// A rectangle of `(x, y, width, height)`.
pub type Rect = (i32, i32, i32, i32);

/// What of `whole` shows once every hole is cut from it, as disjoint rectangles: a hole inside another, as a dialog
/// inside its backdrop, cuts once, as the union of the holes does.
pub fn visible(whole: Rect, holes: &[Rect]) -> Vec<Rect> {
    let mut kept = vec![whole];
    for &(hx, hy, hw, hh) in holes.iter().filter(|h| h.2 > 0 && h.3 > 0) {
        let mut next = Vec::with_capacity(kept.len() + 4);
        for (x, y, w, h) in kept {
            let (l, t, r, b) = (hx.max(x), hy.max(y), (hx + hw).min(x + w), (hy + hh).min(y + h));
            if l >= r || t >= b {
                next.push((x, y, w, h));
                continue;
            }
            next.extend(
                [(x, y, w, t - y), (x, b, w, y + h - b), (x, t, l - x, b - t), (r, t, x + w - r, b - t)]
                    .into_iter()
                    .filter(|p| p.2 > 0 && p.3 > 0),
            );
        }
        kept = next;
    }
    kept
}

/// The script every page runs before its own: it says when it moves in place, and a loopback page registers no
/// service worker, as today's session cancels the worker's script.
pub const INIT_SCRIPT: &str = r#"(() => {
  const say = () => { try { window.ipc.postMessage("wsp:moved"); } catch (_) {} };
  for (const name of ["pushState", "replaceState"]) {
    const original = history[name];
    history[name] = function (...args) { const out = original.apply(this, args); say(); return out; };
  }
  addEventListener("popstate", say);
  addEventListener("hashchange", say);
  const loopback = ["localhost", "127.0.0.1", "0.0.0.0", "[::1]", "::1"];
  if (loopback.includes(location.hostname) && navigator.serviceWorker) {
    navigator.serviceWorker.register = () => Promise.reject(new DOMException("blocked for a local preview", "SecurityError"));
  }
})();"#;

/// What an address asks of the page's navigation: stay in the page, or leave for another app after asking.
pub fn stays(url: &str) -> bool {
    super::words::window_road(url) != super::words::WindowRoad::Ask
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, px, size};

    #[test]
    fn device_edges_meet_at_a_fractional_scale() {
        let a = Bounds::new(point(px(10.3), px(0.)), size(px(100.4), px(50.)));
        let b = Bounds::new(point(px(110.7), px(0.)), size(px(20.), px(50.)));
        let (ax, _, aw, _) = device(a, 1.5);
        let (bx, _, _, _) = device(b, 1.5);
        assert_eq!(ax + aw, bx, "the right edge of one is the left edge of the next");
        assert_eq!(device(a, 2.), (21, 0, 200, 100));
    }

    #[test]
    fn holes_are_relative_to_the_page() {
        let page = Bounds::new(point(px(600.), px(92.)), size(px(540.), px(700.)));
        let menu = Bounds::new(point(px(900.), px(92.)), size(px(224.), px(180.)));
        assert_eq!(device_holes(page, &[menu], 2.), vec![(600, 0, 448, 360)]);
    }

    /// A slot with a hitbox, an occluding box drawn before it (under it), a plain box over it, and a menu over it.
    struct Layers(Rc<Cell<Option<HitboxId>>>);

    impl gpui_kit::Render for Layers {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let slot = self.0.clone();
            div()
                .relative()
                .size_full()
                .child(div().id("under").occlude().absolute().top_0().left_0().w(px(300.)).h(px(300.)))
                .child(
                    canvas(move |b, window, _| slot.set(Some(window.insert_hitbox(b, HitboxBehavior::Normal).id)), |_, _, _, _| {})
                        .absolute()
                        .top(px(50.))
                        .left(px(100.))
                        .w(px(200.))
                        .h(px(200.)),
                )
                .child(div().id("plain").on_click(|_, _, _| {}).absolute().top(px(60.)).left(px(110.)).size(px(20.)))
                .child(gpui_kit::deferred(
                    gpui_kit::anchored().position(point(px(250.), px(50.))).child(div().id("menu").occlude().w(px(100.)).h(px(80.))),
                ))
        }
    }

    #[gpui_kit::test]
    fn the_frame_says_where_the_slot_stands_and_what_blocks_it_from_above(cx: &mut gpui_kit::TestAppContext) {
        let slot: Rc<Cell<Option<HitboxId>>> = Rc::default();
        let made = slot.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Layers(made));
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.run_until_parked();
        let id = slot.get().expect("the slot drew its hitbox");
        let (at, over) = cx.update(|window, _| window.drawn_over(id)).expect("the frame drew the slot");
        assert_eq!(at, Bounds::new(point(px(100.), px(50.)), size(px(200.), px(200.))));
        assert_eq!(
            over,
            vec![Bounds::new(point(px(250.), px(50.)), size(px(50.), px(80.)))],
            "only the menu over it blocks it, clipped to the slot: not the box under it, not the plain one over it"
        );
    }

    fn area(rects: &[Rect]) -> i32 {
        rects.iter().map(|r| r.2 * r.3).sum()
    }

    #[test]
    fn a_hole_inside_another_cuts_once() {
        let backdrop = (0, 0, 100, 100);
        let dialog = (20, 20, 40, 40);
        assert!(visible((0, 0, 100, 100), &[backdrop, dialog]).is_empty(), "a dialog in its backdrop leaves no page showing");
        let menu = (60, 0, 40, 30);
        let kept = visible((0, 0, 100, 100), &[menu, (70, 5, 10, 10)]);
        assert_eq!(area(&kept), 100 * 100 - 40 * 30, "the tooltip inside the menu cuts nothing more");
        assert!(!kept.iter().any(|&(x, y, w, h)| 75 >= x && 75 < x + w && 10 >= y && 10 < y + h), "the tooltip's middle stays cut");
    }

    #[test]
    fn holes_side_by_side_leave_the_rest_whole() {
        let kept = visible((0, 0, 100, 50), &[(0, 0, 10, 10), (90, 40, 20, 20)]);
        assert_eq!(area(&kept), 100 * 50 - 100 - 100, "each hole cuts only what meets the page");
        assert_eq!(visible((0, 0, 100, 50), &[]), vec![(0, 0, 100, 50)]);
    }

    #[test]
    fn another_apps_link_does_not_stay() {
        assert!(stays("https://example.com/"));
        assert!(stays("about:blank"));
        assert!(!stays("mailto:a@b.c"));
        assert!(!stays("zoommtg://join?x=1"));
    }
}
