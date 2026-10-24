// SPDX-License-Identifier: AGPL-3.0-only
//! One shell drawn on screen: its grid, cursor, selection and links from the session's engine, and the person's keys,
//! mouse and paste sent back to it. Focused, it puts the `Terminal` key context on the window. Read off today's
//! `terminal/ghostty/renderer.ts` and `surface/terminalSurface.ts`.

use std::collections::HashSet;
use std::ops::Range;
use std::time::{Duration, Instant};

use gpui_kit::base::{transition, Easing, Transition};
use gpui_kit::{
    canvas, div, fill, outline, point, prelude::*, px, size, App, Bounds, ClipboardItem, Context, CursorStyle as Pointer,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Hsla, KeyDownEvent, KeyUpEvent,
    ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent,
    SharedString, Subscription, Task, TextRun, UTF16Selection, Window,
};
use libghostty_vt::key::Action;
use libghostty_vt::mouse;
use libghostty_vt::terminal::{Mode, ScrollViewport};

use super::engine::{CursorShape, Frame, MouseInput, Row, ScreenPoint};
use super::input::{key_input, mods_of, types_text};
use super::links::{self, Found, Kind};
use super::look::{hsla_of, Look};
use super::session::{Changed, Session};
use crate::screens::app_shell::keys;

/// What a view asks of the pane around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewEvent {
    /// A press inside it, which makes its split the active one.
    Activated,
    /// A Ctrl or Cmd click on a web address.
    OpenUrl(String),
    /// A Ctrl or Cmd click on a path, for the Files pane: resolved against the shell's OSC 7 folder, else as written.
    OpenPath(String),
    /// A right-click the program does not take, at the pointer in the window's points.
    Menu(Point<Pixels>),
}

/// `CURSOR_BLINK_INTERVAL_MS`: half a blink cycle.
const BLINK: Duration = Duration::from_millis(500);
/// `SELECTION_MULTI_CLICK_INTERVAL_MS` and its distance.
const MULTI_CLICK: Duration = Duration::from_millis(500);
const MULTI_CLICK_PX: f32 = 4.;
/// The pty hears a new size this long after the grid settles.
const FIT_SETTLE: Duration = Duration::from_millis(150);
/// A drag past the grid's edge scrolls a row this often.
const AUTOSCROLL: Duration = Duration::from_millis(80);
/// The scrollbar's box (`top-1 right-px bottom-1 w-[--app-scrollbar-width]`) and its thumb, inset 1 px.
const BAR_INSET_Y: f32 = 4.;
const BAR_RIGHT: f32 = 1.;
const BAR_WIDTH: f32 = 6.;
const THUMB_MIN: f32 = 18.;
/// `transition: background-color 120ms ease-out` on the thumb.
const THUMB_FADE: Duration = Duration::from_millis(120);

#[cfg(target_os = "macos")]
const MAC: bool = true;
#[cfg(not(target_os = "macos"))]
const MAC: bool = false;

/// The cell box the font draws in, measured as `measureGhosttyCell` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

/// Where the grid sits in the view's bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Geometry {
    pub bounds: Bounds<Pixels>,
    pub cols: u16,
    pub rows: u16,
    pub x0: f32,
    /// From the view's top to the first row: the padding until scrollback exists, then pinned to the bottom edge.
    pub origin_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectMode {
    Cell,
    Word,
    Line,
}

struct Selecting {
    mode: SelectMode,
    anchor: ScreenPoint,
    /// The word or line the press landed on, which the drag keeps on its far side.
    base: (ScreenPoint, ScreenPoint),
    moved: bool,
}

struct Bar {
    total: f32,
    len: f32,
    top: f32,
    height: f32,
    track: f32,
}

pub struct TerminalView {
    session: Entity<Session>,
    look: Look,
    focus: FocusHandle,
    frame: Frame,
    metrics: Option<(SharedString, f32, Metrics)>,
    geometry: Geometry,
    /// The grid size the session last took, and the wait before the pty hears it.
    told: Option<(u16, u16)>,
    fitting: Option<Task<()>>,
    blink_on: bool,
    blinking: Option<Task<()>>,
    /// Output holds the cursor in its visible phase for a whole blink.
    last_output: Instant,
    bar_focus: FocusHandle,
    selecting: Option<Selecting>,
    last_click: Option<(Instant, Point<Pixels>, u8)>,
    autoscroll: Option<Task<()>>,
    pointer: Point<Pixels>,
    /// The link gesture's key is held: Cmd on the Mac, Ctrl elsewhere.
    gesture: bool,
    hovered: Option<Found>,
    pressed_link: Option<Found>,
    /// A press the program takes, and the last motion it was told.
    reported: Option<mouse::Button>,
    last_motion: Vec<u8>,
    grab: Option<f32>,
    bar_hover: bool,
    wheel_rest: f32,
    /// Keys whose press the view handled itself, so their release is not sent.
    suppressed: HashSet<SharedString>,
    /// Keys whose press reached the view, so a chord the app's keymap took sends no release either.
    down: HashSet<SharedString>,
    marked: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ViewEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn is_gesture(m: &gpui_kit::Modifiers) -> bool {
    if MAC {
        m.platform && !m.control
    } else {
        m.control && !m.platform
    }
}

fn before(a: ScreenPoint, b: ScreenPoint) -> bool {
    (a.y, a.x) < (b.y, b.x)
}

/// The window root's Tab, Shift-Tab and copy chord, held back from a focused terminal so the program gets them.
pub fn key_bindings() -> [gpui_kit::KeyBinding; 3] {
    let copy = if MAC { "cmd-c" } else { "ctrl-c" };
    ["tab", "shift-tab", copy].map(|chord| gpui_kit::KeyBinding::new(chord, gpui_kit::NoAction, Some(keys::TERMINAL)))
}

impl TerminalView {
    pub fn new(session: Entity<Session>, look: Look, window: &mut Window, cx: &mut Context<Self>) -> TerminalView {
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.subscribe(&session, |this, _, changed: &Changed, cx| {
                if *changed == Changed::Output {
                    this.blink_on = true;
                    this.last_output = Instant::now();
                }
                cx.notify();
            }),
            cx.observe(&session, |_, _, cx| cx.notify()),
            cx.on_focus(&focus, window, |this, _, cx| {
                this.blink_on = true;
                this.focus_report(true, cx);
                this.restart_blink(cx);
                cx.notify();
            }),
            cx.on_blur(&focus, window, |this, _, cx| {
                this.gesture = false;
                this.hovered = None;
                this.blinking = None;
                // An input method that lost the focus never commits, and a composition left marked would hold back every key.
                this.marked = None;
                this.focus_report(false, cx);
                cx.notify();
            }),
        ];
        session.update(cx, |s, _| {
            let engine = s.engine_mut();
            engine.set_colors(&look.colors);
            engine.set_cursor_defaults(look.cursor_style, look.cursor_blink);
        });
        TerminalView {
            session,
            look,
            focus,
            frame: Frame::default(),
            metrics: None,
            geometry: Geometry::default(),
            told: None,
            fitting: None,
            blink_on: true,
            blinking: None,
            last_output: Instant::now(),
            bar_focus: cx.focus_handle(),
            selecting: None,
            last_click: None,
            autoscroll: None,
            pointer: Point::default(),
            gesture: false,
            hovered: None,
            pressed_link: None,
            reported: None,
            last_motion: Vec::new(),
            grab: None,
            bar_hover: false,
            wheel_rest: 0.,
            suppressed: HashSet::new(),
            down: HashSet::new(),
            marked: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn session(&self) -> &Entity<Session> {
        &self.session
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        self.session.update(cx, |s, _| {
            let engine = s.engine_mut();
            engine.set_colors(&look.colors);
            engine.set_cursor_defaults(look.cursor_style, look.cursor_blink);
        });
        self.look = look;
        cx.notify();
    }

    /// The grid as last laid out, and the cell box.
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    pub fn metrics(&self) -> Option<Metrics> {
        self.metrics.as_ref().map(|m| m.2)
    }

    /// The screen as the last frame drew it.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// What a cover over the grid paints, so lifting it changes no colour.
    pub fn ground(&self) -> Hsla {
        ground_of(&self.frame, &self.look)
    }

    /// The link under the pointer while the link gesture's key is held.
    pub fn hovered_link(&self) -> Option<&Found> {
        self.hovered.as_ref()
    }

    /// A viewport cell's box in window coordinates.
    pub fn cell_bounds(&self, x: u16, y: u16) -> Option<Bounds<Pixels>> {
        let m = self.metrics()?;
        let g = self.geometry;
        Some(Bounds {
            origin: point(g.bounds.origin.x + px(g.x0 + x as f32 * m.width), g.bounds.origin.y + px(g.origin_y + y as f32 * m.height)),
            size: size(px(m.width), px(m.height)),
        })
    }

    pub fn has_selection(&self, cx: &App) -> bool {
        self.session.read(cx).engine().has_selection()
    }

    pub fn selection_text(&self, cx: &App) -> Option<String> {
        self.session.read(cx).engine().selection_text()
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.selection_text(cx) else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    pub fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.paste_text(&text, cx);
        }
    }

    pub fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let bytes = self.session.read(cx).engine().encode_paste(text);
        self.session.update(cx, |s, cx| s.send(&bytes, cx));
    }

    /// Clear screen and scrollback, as the menu's Clear writes it into the screen alone.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.session.update(cx, |s, cx| s.feed(b"\x1b[H\x1b[2J\x1b[3J", cx));
    }

    pub fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.session.update(cx, |s, _| s.engine_mut().scroll(ScrollViewport::Bottom));
        cx.notify();
    }

    fn focus_report(&mut self, gained: bool, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        if !session.engine().mode(Mode::FOCUS_EVENT) {
            return;
        }
        let event = if gained { libghostty_vt::focus::Event::Gained } else { libghostty_vt::focus::Event::Lost };
        let mut buf = [0u8; 8];
        if let Ok(n) = event.encode(&mut buf) {
            self.session.update(cx, |s, cx| s.send(&buf[..n], cx));
        }
    }

    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        self.blinking = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(BLINK).await;
            let alive = this.update(cx, |this, cx| {
                let blinks = !cx.reduce_motion() && this.frame.cursor.is_some_and(|c| c.blinking) && this.last_output.elapsed() >= BLINK;
                let was = this.blink_on;
                this.blink_on = !this.blink_on || !blinks;
                if this.blink_on != was {
                    cx.notify_in_place();
                }
            });
            if alive.is_err() {
                break;
            }
        }));
    }

    fn measure(&mut self, window: &mut Window) -> Metrics {
        let key = (self.look.font_family.clone(), self.look.font_size);
        if let Some((family, size, m)) = &self.metrics {
            if *family == key.0 && *size == key.1 {
                return *m;
            }
        }
        let size = self.look.font_size;
        let ts = window.text_system();
        let id = ts.resolve_font(&self.look.font(false, false));
        let width = ts.advance(id, px(size), 'M').map(|s| f32::from(s.width)).unwrap_or(size * 0.6);
        let (font_ascent, font_descent) = (f32::from(ts.ascent(id, px(size))), f32::from(ts.descent(id, px(size))).abs());
        let ink = |c: char| ts.typographic_bounds(id, px(size), c).ok();
        let (mut ascent, mut descent) = (0f32, 0f32);
        for c in ['M', 'g'] {
            if let Some(b) = ink(c) {
                ascent = ascent.max(f32::from(b.origin.y + b.size.height));
                descent = descent.max(-f32::from(b.origin.y));
            }
        }
        if ascent <= 0. {
            ascent = font_ascent.min(size);
            descent = font_descent;
        }
        let height = (size * 1.35).round().max((ascent + descent).ceil()).max(1.);
        let baseline = ((height - (ascent + descent)) / 2. + ascent).round();
        let m = Metrics { width, height, baseline };
        self.metrics = Some((key.0, key.1, m));
        m
    }

    /// Lays the grid out in `bounds`: the screen reflows at once, and the pty hears the size once it has held for
    /// 150 ms, as today's surface tells it (`notifyResize`).
    fn layout(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.measure(window);
        let pad = self.look.padding;
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let cols = ((w - pad.left - pad.right) / m.width).floor().max(1.) as u16;
        let rows = ((h - pad.top - pad.bottom) / m.height).floor().max(1.) as u16;
        let scrollback = self.session.read(cx).engine().scrollbar().has_scrollback();
        let origin_y = if scrollback { pad.top + (h - pad.top - pad.bottom - rows as f32 * m.height).max(0.) } else { pad.top };
        self.geometry = Geometry { bounds, cols, rows, x0: pad.left, origin_y };
        if self.told == Some((cols, rows)) {
            return;
        }
        let (cw, ch) = (m.width.round() as u32, m.height.round() as u32);
        self.told = Some((cols, rows));
        self.session.update(cx, |s, cx| s.resize(cols, rows, cw, ch, cx));
        self.fitting = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FIT_SETTLE).await;
            _ = this.update(cx, |this, cx| {
                this.fitting = None;
                this.session.update(cx, |s, cx| s.tell_size(cx));
            });
        }));
    }

    fn cell_at(&self, position: Point<Pixels>) -> (u16, u16) {
        let g = self.geometry;
        let Some((_, _, m)) = &self.metrics else { return (0, 0) };
        let x = f32::from(position.x - g.bounds.origin.x) - g.x0;
        let y = f32::from(position.y - g.bounds.origin.y) - g.origin_y;
        let col = (x / m.width).floor().clamp(0., g.cols.saturating_sub(1) as f32) as u16;
        let row = (y / m.height).floor().clamp(0., g.rows.saturating_sub(1) as f32) as u16;
        (col, row)
    }

    /// The cell under `position`, or none in the padding around the grid, as today's `terminalGridCellAt`.
    fn grid_cell_at(&self, position: Point<Pixels>) -> Option<(u16, u16)> {
        let (g, m) = (self.geometry, self.metrics()?);
        let x = f32::from(position.x - g.bounds.origin.x) - g.x0;
        let y = f32::from(position.y - g.bounds.origin.y) - g.origin_y;
        let inside = x >= 0. && y >= 0. && x < g.cols as f32 * m.width && y < g.rows as f32 * m.height;
        inside.then(|| ((x / m.width) as u16, (y / m.height) as u16))
    }

    fn bar(&self) -> Option<Bar> {
        let s = self.frame.scrollbar;
        let (total, len) = (s.total as f32, (s.len.min(s.total)) as f32);
        let max = total - len;
        if max <= 0. || len <= 0. {
            return None;
        }
        let track = f32::from(self.geometry.bounds.size.height) - BAR_INSET_Y * 2.;
        let height = track.min(THUMB_MIN.max(track * len / total));
        let offset = s.offset as f32;
        Some(Bar { total, len, top: (track - height) * offset / max, height, track })
    }

    fn over_bar(&self, position: Point<Pixels>) -> bool {
        let b = self.geometry.bounds;
        self.bar().is_some() && f32::from(b.right() - position.x) <= BAR_WIDTH + BAR_RIGHT && b.contains(&position)
    }

    /// Mouse events go to the program when it tracks the mouse and no Shift, Ctrl or Cmd overrides it.
    fn reports(&self, m: &gpui_kit::Modifiers, cx: &App) -> bool {
        !m.shift && !m.control && !m.platform && self.session.read(cx).engine().mouse_tracking()
    }

    fn mouse_bytes(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        position: Point<Pixels>,
        m: &gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) -> Vec<u8> {
        let g = self.geometry;
        let Some((_, _, metrics)) = self.metrics.as_ref().map(|m| (0, 0, m.2)) else { return Vec::new() };
        let (w, h) = (f32::from(g.bounds.size.width), f32::from(g.bounds.size.height));
        // The encoder counts cells of whole pixels, so x is measured in those; a fractional advance would drift a column every few.
        let cell_width = metrics.width.round().max(1.);
        let across = |x: f32| g.x0 + (x - g.x0) * cell_width / metrics.width;
        let width = across(w);
        let size = mouse::EncoderSize {
            screen_width: width as u32,
            screen_height: h as u32,
            cell_width: cell_width as u32,
            cell_height: metrics.height.round() as u32,
            padding_top: g.origin_y as u32,
            padding_bottom: (h - g.origin_y - g.rows as f32 * metrics.height).max(0.) as u32,
            padding_right: (width - g.x0 - g.cols as f32 * cell_width).max(0.) as u32,
            padding_left: g.x0 as u32,
        };
        let input = MouseInput {
            action,
            button,
            mods: mods_of(m),
            x: across(f32::from(position.x - g.bounds.origin.x)).max(0.),
            y: f32::from(position.y - g.bounds.origin.y).max(0.),
            size,
            any_pressed: self.reported.is_some(),
        };
        self.session.update(cx, |s, _| s.engine_mut().encode_mouse(&input))
    }

    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if !bytes.is_empty() {
            self.session.update(cx, |s, cx| s.send(bytes, cx));
        }
    }

    /// `linkAt`: an OSC 8 link runs over the cells that carry its address, across soft wraps.
    fn link_at(&self, position: Point<Pixels>, cx: &App) -> Option<Found> {
        let (x, y) = self.grid_cell_at(position)?;
        let engine = self.session.read(cx).engine();
        let Some(uri) = engine.hyperlink_at(x, y) else { return links::at(&self.frame.rows, y as usize, x) };
        let (rows, cols) = (&self.frame.rows, self.frame.cols);
        let row = |y: u16| rows.get(y as usize);
        let same = |at: links::At| engine.hyperlink_at(at.x, at.y).as_deref() == Some(uri.as_str());
        let (mut start, mut end) = (links::At { x, y }, links::At { x, y });
        loop {
            let previous = if start.x > 0 {
                Some(links::At { x: start.x - 1, y: start.y })
            } else {
                (start.y > 0 && row(start.y).is_some_and(|r| r.wrap_continuation))
                    .then(|| links::At { x: cols.saturating_sub(1), y: start.y - 1 })
            };
            match previous.filter(|&at| same(at)) {
                Some(at) => start = at,
                None => break,
            }
        }
        loop {
            let next = if end.x + 1 < cols {
                Some(links::At { x: end.x + 1, y: end.y })
            } else {
                (row(end.y + 1).is_some() && row(end.y).is_some_and(|r| r.wraps)).then(|| links::At { x: 0, y: end.y + 1 })
            };
            match next.filter(|&at| same(at)) {
                Some(at) => end = at,
                None => break,
            }
        }
        // Only a web address may reach the system opener; any other scheme is a path for Files, as today's pane reads it.
        let kind = if links::is_url(&uri) { Kind::Url } else { Kind::Path };
        Some(Found { kind, text: uri, start, end })
    }

    fn hover_link(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let next = if self.gesture { self.link_at(position, cx) } else { None };
        if next != self.hovered {
            self.hovered = next;
            cx.notify();
        }
    }

    fn open(&mut self, link: Found, cx: &mut Context<Self>) {
        match link.kind {
            Kind::Url => cx.emit(ViewEvent::OpenUrl(link.text)),
            Kind::Path => {
                let path = match self.session.read(cx).engine().pwd() {
                    Some(cwd) => links::resolve_path(&link.text, &cwd),
                    None => link.text,
                };
                cx.emit(ViewEvent::OpenPath(path));
            }
        }
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ViewEvent::Activated);
        if e.button == MouseButton::Left && self.over_bar(e.position) {
            window.focus(&self.bar_focus, cx);
            if let Some(bar) = self.bar() {
                let y = f32::from(e.position.y - self.geometry.bounds.origin.y) - BAR_INSET_Y;
                let on_thumb = y >= bar.top && y <= bar.top + bar.height;
                let grab = if on_thumb { y - bar.top } else { bar.height / 2. };
                self.grab = Some(grab);
                self.drag_bar(e.position, cx);
            }
            cx.stop_propagation();
            return;
        }
        window.focus(&self.focus, cx);
        if self.reports(&e.modifiers, cx) {
            let button = match e.button {
                MouseButton::Left => mouse::Button::Left,
                MouseButton::Middle => mouse::Button::Middle,
                MouseButton::Right => mouse::Button::Right,
                MouseButton::Navigate(gpui_kit::NavigationDirection::Back) => mouse::Button::Four,
                MouseButton::Navigate(gpui_kit::NavigationDirection::Forward) => mouse::Button::Five,
            };
            self.reported = Some(button);
            let bytes = self.mouse_bytes(mouse::Action::Press, Some(button), e.position, &e.modifiers, cx);
            self.send(&bytes, cx);
            cx.stop_propagation();
            return;
        }
        if e.button != MouseButton::Left {
            return;
        }
        if is_gesture(&e.modifiers) {
            self.gesture = true;
            self.pressed_link = self.link_at(e.position, cx);
            cx.stop_propagation();
            return;
        }
        let now = Instant::now();
        let clicks = match self.last_click {
            Some((at, pos, n))
                if now - at <= MULTI_CLICK
                    && (f32::from(pos.x - e.position.x)).hypot(f32::from(pos.y - e.position.y)) <= MULTI_CLICK_PX =>
            {
                n % 3 + 1
            }
            _ => 1,
        };
        self.last_click = Some((now, e.position, clicks));
        let (x, y) = self.cell_at(e.position);
        let engine_pick = self.session.read(cx).engine();
        let picked = match clicks {
            2 => engine_pick.word_at(x, y).map(|r| (SelectMode::Word, r)),
            3 => engine_pick.line_at(x, y).map(|r| (SelectMode::Line, r)),
            _ => None,
        };
        let anchor = engine_pick.screen_point(x, y);
        let Some(anchor) = anchor else { return };
        let (mode, base) = picked.unwrap_or((SelectMode::Cell, (anchor, anchor)));
        self.session.update(cx, |s, _| s.engine_mut().select(base.0, base.1));
        self.selecting = Some(Selecting { mode, anchor, base, moved: false });
        cx.notify();
    }

    fn drag_bar(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(grab), Some(bar)) = (self.grab, self.bar()) else { return };
        let travel = (bar.track - bar.height).max(1.);
        let y = f32::from(position.y - self.geometry.bounds.origin.y) - BAR_INSET_Y;
        let top = (y - grab).clamp(0., travel);
        let offset = (top / travel * (bar.total - bar.len)).round() as usize;
        self.session.update(cx, |s, _| s.engine_mut().scroll(ScrollViewport::Row(offset)));
        cx.notify();
    }

    fn extend_selection(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(sel) = &mut self.selecting else { return };
        let (mode, anchor, base) = (sel.mode, sel.anchor, sel.base);
        let g = self.geometry;
        let (x, y) = {
            let inside = Point {
                x: position.x.clamp(g.bounds.left(), g.bounds.right() - px(1.)),
                y: position.y.clamp(g.bounds.top(), g.bounds.bottom() - px(1.)),
            };
            self.cell_at(inside)
        };
        let engine = self.session.read(cx).engine();
        let Some(at) = engine.screen_point(x, y) else { return };
        let range = match mode {
            SelectMode::Cell => Some((anchor, at)),
            SelectMode::Word | SelectMode::Line => {
                let snapped = if mode == SelectMode::Word { engine.word_at(x, y) } else { engine.line_at(x, y) };
                let (s, e) = snapped.unwrap_or((at, at));
                Some(if before(s, base.0) { (s, base.1) } else { (base.0, e) })
            }
        };
        if let Some((a, b)) = range {
            if let Some(sel) = &mut self.selecting {
                sel.moved |= a != sel.base.0 || b != sel.base.1;
            }
            self.session.update(cx, |s, _| s.engine_mut().select(a, b));
            cx.notify();
        }
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.pointer = e.position;
        let over_bar = self.over_bar(e.position);
        if over_bar != self.bar_hover {
            self.bar_hover = over_bar;
            cx.notify();
        }
        if self.grab.is_some() {
            self.drag_bar(e.position, cx);
            return;
        }
        if self.gesture != is_gesture(&e.modifiers) {
            self.gesture = is_gesture(&e.modifiers);
        }
        self.hover_link(e.position, cx);
        let tracking = self.session.read(cx).engine().mouse_tracking();
        if tracking && (self.reported.is_some() || self.session.read(cx).engine().mode(Mode::ANY_MOUSE)) && self.reports(&e.modifiers, cx) {
            let button = self.reported;
            let bytes = self.mouse_bytes(mouse::Action::Motion, button, e.position, &e.modifiers, cx);
            if !bytes.is_empty() && bytes != self.last_motion {
                self.last_motion = bytes.clone();
                self.send(&bytes, cx);
            }
            return;
        }
        if self.selecting.is_some() && e.pressed_button == Some(MouseButton::Left) {
            self.extend_selection(e.position, cx);
            let b = self.geometry.bounds;
            let step = if e.position.y < b.top() {
                -1
            } else if e.position.y >= b.bottom() {
                1
            } else {
                0
            };
            if step == 0 {
                self.autoscroll = None;
            } else if self.autoscroll.is_none() {
                self.autoscroll = Some(cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(AUTOSCROLL).await;
                    let alive = this.update(cx, |this, cx| {
                        this.session.update(cx, |s, _| s.engine_mut().scroll(ScrollViewport::Delta(step)));
                        let at = this.pointer;
                        this.extend_selection(at, cx);
                    });
                    if alive.is_err() {
                        break;
                    }
                }));
            }
        }
    }

    fn on_mouse_up(&mut self, e: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.autoscroll = None;
        if self.grab.take().is_some() {
            return;
        }
        if let Some(button) = self.reported.take() {
            let bytes = self.mouse_bytes(mouse::Action::Release, Some(button), e.position, &e.modifiers, cx);
            self.send(&bytes, cx);
            self.last_motion.clear();
            return;
        }
        if let Some(link) = self.pressed_link.take() {
            if self.link_at(e.position, cx).as_ref() == Some(&link) {
                self.open(link, cx);
            }
            return;
        }
        if let Some(sel) = self.selecting.take() {
            if sel.mode == SelectMode::Cell && !sel.moved {
                self.session.update(cx, |s, _| s.engine_mut().clear_selection());
            }
            cx.notify();
        }
    }

    fn on_scroll(&mut self, e: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, _, m)) = &self.metrics else { return };
        let ch = m.height;
        let dy = match e.delta {
            ScrollDelta::Pixels(p) => -f32::from(p.y),
            ScrollDelta::Lines(l) => -l.y * ch,
        };
        let raw = self.wheel_rest + dy / ch;
        let rows = raw.trunc() as i32;
        self.wheel_rest = raw - rows as f32;
        if rows == 0 {
            return;
        }
        cx.stop_propagation();
        if self.reports(&e.modifiers, cx) {
            let button = if rows < 0 { mouse::Button::Four } else { mouse::Button::Five };
            for _ in 0..rows.unsigned_abs() {
                let bytes = self.mouse_bytes(mouse::Action::Press, Some(button), e.position, &e.modifiers, cx);
                self.send(&bytes, cx);
            }
            return;
        }
        let engine = self.session.read(cx).engine();
        if engine.alternate_screen() {
            let app = engine.mode(Mode::DECCKM);
            let seq: &[u8] = match (rows < 0, app) {
                (true, false) => b"\x1b[A",
                (true, true) => b"\x1bOA",
                (false, false) => b"\x1b[B",
                (false, true) => b"\x1bOB",
            };
            let bytes = seq.repeat(rows.unsigned_abs() as usize);
            self.send(&bytes, cx);
            return;
        }
        self.session.update(cx, |s, _| s.engine_mut().scroll(ScrollViewport::Delta(rows as isize)));
        cx.notify();
    }

    /// `onScrollbarKeyDown`: the focused scrollbar moves the viewport by a row, a page, or to either end.
    fn on_bar_key(&mut self, e: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let s = self.frame.scrollbar;
        let (offset, len, total) = (s.offset as isize, s.len.min(s.total) as isize, s.total as isize);
        let delta = match e.keystroke.key.as_str() {
            "up" => -1,
            "down" => 1,
            "pageup" => -len.max(1),
            "pagedown" => len.max(1),
            "home" => -offset,
            "end" => total - len - offset,
            _ => return,
        };
        cx.stop_propagation();
        self.session.update(cx, |s, _| s.engine_mut().scroll(ScrollViewport::Delta(delta)));
        cx.notify();
    }

    fn on_key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.bar_focus.is_focused(window) {
            return;
        }
        self.down.insert(e.keystroke.key.clone().into());
        let k = &e.keystroke;
        let m = &k.modifiers;
        self.gesture = is_gesture(m);
        let key = k.key.as_str();
        let has_selection = self.has_selection(cx);
        let copy_chord = key == "c" && if MAC { m.platform && !m.control && !m.alt } else { m.control && !m.platform && !m.alt };
        if copy_chord && has_selection {
            self.copy(cx);
            if !MAC && !m.shift {
                self.session.update(cx, |s, _| s.engine_mut().clear_selection());
            }
            self.suppress(k.key.clone(), cx);
            return;
        }
        let paste_chord = if MAC {
            key == "v" && m.platform && !m.control && !m.alt
        } else {
            (key == "v" && m.control && m.shift && !m.platform && !m.alt) || (key == "insert" && m.shift && !m.control && !m.platform)
        };
        if paste_chord {
            self.paste(cx);
            self.suppress(k.key.clone(), cx);
            return;
        }
        if self.marked.is_some() {
            return;
        }
        if MAC && m.platform && !m.control && !m.alt && !m.shift {
            let edit: Option<&[u8]> = match key {
                "backspace" => Some(b"\x15"),
                "left" => Some(b"\x01"),
                "right" => Some(b"\x05"),
                _ => None,
            };
            if let Some(bytes) = edit {
                self.send(bytes, cx);
                self.suppress(k.key.clone(), cx);
                return;
            }
        }
        let every_key = self
            .session
            .read(cx)
            .engine()
            .terminal()
            .kitty_keyboard_flags()
            .is_ok_and(|f| f.contains(libghostty_vt::key::KittyKeyFlags::REPORT_ALL));
        if types_text(k) && !every_key {
            return;
        }
        let input = key_input(k, if e.is_held { Action::Repeat } else { Action::Press });
        let bytes = self.session.update(cx, |s, _| s.engine_mut().encode_key(&input));
        if MAC && m.platform && (bytes.is_empty() || k.key_char.as_deref().is_some_and(|t| t.as_bytes() == bytes.as_slice())) {
            return;
        }
        if bytes.is_empty() {
            return;
        }
        self.send(&bytes, cx);
        cx.stop_propagation();
    }

    fn suppress(&mut self, key: String, cx: &mut Context<Self>) {
        self.suppressed.insert(key.into());
        cx.stop_propagation();
    }

    fn on_key_up(&mut self, e: &KeyUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.gesture = is_gesture(&e.keystroke.modifiers);
        let key = e.keystroke.key.as_str();
        if !self.down.remove(key) || self.suppressed.remove(key) {
            return;
        }
        if !self.session.read(cx).engine().reports_releases() {
            return;
        }
        let input = key_input(&e.keystroke, Action::Release);
        let bytes = self.session.update(cx, |s, _| s.engine_mut().encode_key(&input));
        self.send(&bytes, cx);
    }

    fn on_modifiers(&mut self, e: &ModifiersChangedEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let gesture = is_gesture(&e.modifiers);
        if gesture != self.gesture {
            self.gesture = gesture;
            let at = self.pointer;
            self.hover_link(at, cx);
        }
    }

    fn pointer_style(&self, cx: &App) -> Pointer {
        if self.hovered.is_some() {
            Pointer::PointingHand
        } else if self.bar_hover || self.session.read(cx).engine().mouse_tracking() {
            Pointer::Arrow
        } else {
            Pointer::IBeam
        }
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|m| 0..m.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        self.send(text.as_bytes(), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!text.is_empty()).then(|| text.to_owned());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (_, _, m) = self.metrics.as_ref()?;
        let c = self.frame.cursor?;
        let g = self.geometry;
        Some(Bounds {
            origin: point(element.origin.x + px(g.x0 + c.x as f32 * m.width), element.origin.y + px(g.origin_y + c.y as f32 * m.height)),
            size: size(px(m.width), px(m.height)),
        })
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

/// What a frame paints, taken out of the view while it paints and put back after.
struct Painting {
    frame: Frame,
    look: Look,
    metrics: Metrics,
    geometry: Geometry,
    focused: bool,
    blink_on: bool,
    hovered: Option<Found>,
    thumb: Option<(Bounds<Pixels>, Hsla)>,
    /// An input method's composition, drawn at the cursor until it commits.
    marked: Option<String>,
}

/// The ground the pane shows: the program's background (OSC 11) once a frame is drawn, the theme's before it and under
/// glass, where the drawer lays it.
fn ground_of(frame: &Frame, look: &Look) -> Hsla {
    hsla_of(if frame.cols == 0 || look.opacity < 1. { look.colors.background } else { frame.background })
}

fn paint_grid(p: &Painting, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let (m, g, look) = (p.metrics, p.geometry, &p.look);
    // A see-through pane's drawer already lays the ground at the config's opacity; a second layer would thicken it.
    if look.opacity >= 1. {
        window.paint_quad(fill(bounds, ground_of(&p.frame, look)));
    }
    let ox = bounds.origin.x + px(g.x0);
    let oy = bounds.origin.y + px(g.origin_y);
    let cell = |x: u16, y: usize, w: u16| Bounds {
        origin: point(ox + px(x as f32 * m.width), oy + px(y as f32 * m.height)),
        size: size(px(w as f32 * m.width), px(m.height)),
    };
    let font_size = px(look.font_size);
    for (y, row) in p.frame.rows.iter().enumerate() {
        let mut x = 0usize;
        while x < row.cells.len() {
            let c = row.cells[x];
            let mut end = x + 1;
            while end < row.cells.len() && row.cells[end].bg == c.bg && row.cells[end].flags.selected == c.flags.selected {
                end += 1;
            }
            if c.bg != p.frame.background {
                window.paint_quad(fill(cell(x as u16, y, (end - x) as u16), hsla_of(c.bg)));
            }
            if c.flags.selected {
                window.paint_quad(fill(cell(x as u16, y, (end - x) as u16), look.selection));
            }
            x = end;
        }
        paint_text(row, y, &cell, font_size, look, m, None, window, cx);
        for (x, c) in row.cells.iter().enumerate() {
            let linked = p.hovered.as_ref().is_some_and(|l| l.covers(x as u16, y as u16));
            let ink = hsla_of(c.fg);
            let b = cell(x as u16, y, 1);
            if c.flags.underline != super::engine::Line::None || linked {
                window.paint_quad(fill(
                    Bounds { origin: point(b.origin.x, b.origin.y + px(m.height - 2.)), size: size(b.size.width, px(1.)) },
                    ink,
                ));
            }
            if c.flags.strikethrough {
                window.paint_quad(fill(
                    Bounds { origin: point(b.origin.x, b.origin.y + px((m.height * 0.55).floor())), size: size(b.size.width, px(1.)) },
                    ink,
                ));
            }
            if c.flags.overline {
                window.paint_quad(fill(Bounds { origin: point(b.origin.x, b.origin.y + px(1.)), size: size(b.size.width, px(1.)) }, ink));
            }
        }
    }
    if let Some(c) = p.frame.cursor.filter(|_| p.blink_on || !p.focused) {
        let b = cell(c.x, c.y as usize, 1);
        let ink = hsla_of(c.color);
        let shape = if p.focused { c.shape } else { CursorShape::Hollow };
        match shape {
            CursorShape::Hollow => window.paint_quad(outline(
                Bounds { origin: b.origin, size: size(b.size.width, b.size.height) },
                ink,
                gpui_kit::BorderStyle::Solid,
            )),
            CursorShape::Bar => window.paint_quad(fill(Bounds { origin: b.origin, size: size(px(2.), b.size.height) }, ink)),
            CursorShape::Underline => window.paint_quad(fill(
                Bounds { origin: point(b.origin.x, b.origin.y + b.size.height - px(2.)), size: size(b.size.width, px(2.)) },
                ink,
            )),
            CursorShape::Block => {
                window.paint_quad(fill(b, ink));
                if let Some(row) = p.frame.rows.get(c.y as usize) {
                    let ground = hsla_of(p.frame.background);
                    paint_text(row, c.y as usize, &cell, font_size, look, m, Some((c.x as usize, ground)), window, cx);
                }
            }
        }
    }
    if let (Some(marked), Some(c)) = (&p.marked, p.frame.cursor) {
        let ink = hsla_of(p.frame.foreground);
        let underline = Some(gpui_kit::UnderlineStyle { thickness: px(1.), color: Some(ink), wavy: false });
        let run = TextRun {
            len: marked.len(),
            font: look.font(false, false),
            color: ink,
            background_color: None,
            underline,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(SharedString::from(marked.clone()), font_size, &[run], None);
        let at = cell(c.x, c.y as usize, 1).origin;
        window.paint_quad(fill(Bounds { origin: at, size: size(line.width, px(m.height)) }, hsla_of(p.frame.background)));
        let lead = f32::from(gpui_kit::line_baseline(px(m.height), line.ascent, line.descent, window.scale_factor()));
        _ = line.paint(point(at.x, at.y + px(m.baseline - lead)), px(m.height), gpui_kit::TextAlign::Left, None, window, cx);
    }
    if let Some((thumb, color)) = p.thumb {
        window.paint_quad(fill(thumb, color).corner_radii(px(3.)));
    }
}

/// The row's text in runs on the cell grid; with `only`, just that cell, in the given colour and squeezed into it.
#[allow(clippy::too_many_arguments)]
fn paint_text(
    row: &Row,
    y: usize,
    cell: &dyn Fn(u16, usize, u16) -> Bounds<Pixels>,
    font_size: Pixels,
    look: &Look,
    m: Metrics,
    only: Option<(usize, Hsla)>,
    window: &mut Window,
    cx: &mut App,
) {
    let cells = &row.cells;
    let mut x = only.map_or(0, |(x, _)| x);
    let stop = only.map_or(cells.len(), |(x, _)| (x + 1).min(cells.len()));
    let mut text = String::new();
    while x < stop {
        let c = cells[x];
        if c.len == 0 || c.flags.invisible {
            x += 1;
            continue;
        }
        let mut end = x + 1;
        if c.width < 2 && only.is_none() {
            while end < stop {
                let n = cells[end];
                if n.width == 0 {
                    end += 1;
                    continue;
                }
                if n.len == 0
                    || n.width == 2
                    || n.fg != c.fg
                    || n.flags.bold != c.flags.bold
                    || n.flags.italic != c.flags.italic
                    || n.flags.invisible
                {
                    break;
                }
                end += 1;
            }
        }
        text.clear();
        let mut count = 0u16;
        for n in &cells[x..end] {
            if n.width > 0 {
                text.push_str(row.cell_text(n));
                count += n.width as u16;
            }
        }
        if !text.trim().is_empty() {
            let color = only.map_or(hsla_of(c.fg), |(_, ink)| ink);
            let run = TextRun {
                len: text.len(),
                font: look.font(c.flags.bold, c.flags.italic),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let force = if c.width == 2 { None } else { Some(px(m.width)) };
            let shape = |size: Pixels, window: &mut Window| {
                window.text_system().shape_line(SharedString::from(text.clone()), size, std::slice::from_ref(&run), force)
            };
            let mut line = shape(font_size, window);
            // The block cursor is one cell wide and GPUI's text API has no horizontal-only scale, so a wide glyph under it shrinks whole.
            if only.is_some() && line.width > px(m.width) {
                line = shape(font_size * (m.width / f32::from(line.width)), window);
            }
            let b = cell(x as u16, y, count);
            let lead = f32::from(gpui_kit::line_baseline(px(m.height), line.ascent, line.descent, window.scale_factor()));
            let origin = point(b.origin.x, b.origin.y + px(m.baseline - lead));
            _ = line.paint(origin, px(m.height), gpui_kit::TextAlign::Left, None, window, cx);
        }
        x = end;
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let focused = self.focus.is_focused(window);
        let thumb_target = if self.bar_hover || self.grab.is_some() || self.bar_focus.is_focused(window) {
            self.look.scrollbar_thumb_hover
        } else {
            self.look.scrollbar_thumb
        };
        let thumb_ink = transition(
            gpui_kit::ElementId::from(("terminal-thumb", view.entity_id().as_u64() as usize)),
            ThumbInk(thumb_target),
            Transition::new(THUMB_FADE).easing(Easing::cubic_bezier(0., 0., 0.58, 1.).expect("a valid curve")),
            window,
            cx,
        )
        .0;
        let pointer = self.pointer_style(cx);
        let prepaint_view = view.clone();
        let paint_view = view.clone();
        div()
            .id(("terminal", view.entity_id().as_u64() as usize))
            .key_context(keys::TERMINAL)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .cursor(pointer)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_modifiers_changed(cx.listener(Self::on_modifiers))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    if this.reports(&e.modifiers, cx) {
                        this.on_mouse_down(e, window, cx);
                    } else {
                        window.focus(&this.focus, cx);
                        cx.emit(ViewEvent::Activated);
                        cx.emit(ViewEvent::Menu(e.position));
                        cx.stop_propagation();
                    }
                }),
            )
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .when(self.bar().is_some(), |el| {
                el.child(
                    div()
                        .id("terminal-scrollbar")
                        .debug_selector(|| "terminal-scrollbar".into())
                        .track_focus(&self.bar_focus)
                        .absolute()
                        .top(px(BAR_INSET_Y))
                        .bottom(px(BAR_INSET_Y))
                        .right(px(BAR_RIGHT))
                        .w(px(BAR_WIDTH))
                        .on_key_down(cx.listener(Self::on_bar_key)),
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        prepaint_view.update(cx, |this, cx| {
                            this.layout(bounds, window, cx);
                            this.session.update(cx, |s, _| s.engine_mut().frame(&mut this.frame));
                            let thumb = this.bar().map(|bar| {
                                let b = Bounds {
                                    origin: point(
                                        bounds.right() - px(BAR_RIGHT + BAR_WIDTH - 1.),
                                        bounds.top() + px(BAR_INSET_Y + bar.top),
                                    ),
                                    size: size(px(BAR_WIDTH - 2.), px(bar.height)),
                                };
                                (b, thumb_ink)
                            });
                            Painting {
                                metrics: this.metrics.as_ref().map(|m| m.2).expect("measured in layout"),
                                geometry: this.geometry,
                                look: this.look.clone(),
                                focused,
                                blink_on: this.blink_on,
                                hovered: this.hovered.clone(),
                                thumb,
                                marked: this.marked.clone(),
                                frame: std::mem::take(&mut this.frame),
                            }
                        })
                    },
                    move |bounds, painting, window, cx| {
                        paint_grid(&painting, bounds, window, cx);
                        crate::bench::painted(cx);
                        let focus = paint_view.read(cx).focus.clone();
                        window.handle_input(&focus, ElementInputHandler::new(bounds, paint_view.clone()), cx);
                        let dragging = paint_view.clone();
                        // A move off the grid reaches none of its listeners, so a drag that leaves it is followed from the window.
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase == gpui_kit::DispatchPhase::Bubble && e.pressed_button.is_some() && !bounds.contains(&e.position) {
                                dragging.update(cx, |this, cx| {
                                    if this.grab.is_some() || this.selecting.is_some() || this.reported.is_some() {
                                        this.on_mouse_move(e, window, cx);
                                    }
                                });
                            }
                        });
                        paint_view.update(cx, |this, _| this.frame = painting.frame);
                    },
                )
                .size_full(),
            )
    }
}

#[derive(Clone, Copy, PartialEq)]
struct ThumbInk(Hsla);

impl gpui_kit::base::animation::Lerp for ThumbInk {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let (a, b) = (self.0.to_rgb(), target.0.to_rgb());
        let mix = |x: f32, y: f32| x + (y - x) * t;
        ThumbInk(gpui_kit::Rgba { r: mix(a.r, b.r), g: mix(a.g, b.g), b: mix(a.b, b.b), a: mix(a.a, b.a) }.into())
    }
}
