// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's transcript in a virtual list. The list builds only the rows on screen and keeps the measured height
//! of every row it has built, and each markdown row's parsed document lives here, made the first time the row is
//! built, so a row that scrolls away and back is not parsed or highlighted again.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::component::text::{TextView, TextViewState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{canvas, div, list, prelude::*, px, AnyElement, Context, Entity, ListAlignment, ListState, Pixels, SharedString, Window};
use wsp_host_client::protocol::SessionEvent;

use crate::transcript::{rows, Row, RowKind};

/// How far past the viewport the list measures rows, so a row is built and measured before it scrolls in.
const OVERDRAW: Pixels = px(800.);
/// The height a row is taken to have until it is measured, the web app's estimatedItemSize: without one an unmeasured
/// row counts as 0 px, and the list cannot tell where its end is.
const ROW_ESTIMATE: Pixels = px(90.);
/// The web app's transcript column, max-w-3xl, so both apps wrap the same text at the same width.
const COLUMN: Pixels = px(768.);
const PERSON_WIDTH: Pixels = px(614.);

/// A scroll driven by the clock, as the bench runs it: the offset is set from elapsed time on every frame.
struct Drive {
    px_per_s: f32,
    start: Instant,
    frames: Rc<RefCell<Vec<Instant>>>,
    done: bool,
}

pub struct ThreadView {
    thread: Option<String>,
    events: Vec<SessionEvent>,
    opened: HashSet<String>,
    rows: Vec<Row>,
    bodies: HashMap<String, Entity<TextViewState>>,
    list: ListState,
    loaded: bool,
    failure: Option<String>,
    built: usize,
    drive: Option<Drive>,
    /// A thread, and the cell that takes the time of the first presented frame showing its rows once it is loaded.
    presented: Option<(String, Rc<RefCell<Option<Instant>>>)>,
}

impl ThreadView {
    pub fn new(_: &mut Context<Self>) -> Self {
        ThreadView {
            thread: None,
            events: Vec::new(),
            opened: HashSet::new(),
            rows: Vec::new(),
            bodies: HashMap::new(),
            list: ListState::new(0, ListAlignment::Top, OVERDRAW),
            loaded: false,
            failure: None,
            built: 0,
            drive: None,
            presented: None,
        }
    }

    /// Empties the view for another thread, which shows once its transcript arrives.
    pub fn show(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.thread = Some(thread.to_owned());
        self.events.clear();
        self.opened.clear();
        self.rows.clear();
        self.bodies.clear();
        self.list.reset(0);
        self.loaded = false;
        self.failure = None;
        cx.notify();
    }

    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    /// The whole transcript as it stands, which replaces anything shown before.
    pub fn load(&mut self, events: Vec<SessionEvent>, cx: &mut Context<Self>) {
        self.events = events;
        self.loaded = true;
        self.rows = rows(&self.events, &self.opened);
        self.bodies.clear();
        self.list.reset_with_uniform_height(self.rows.len(), ROW_ESTIMATE);
        cx.notify();
    }

    /// An event the host pushed after the transcript was read; a view at its end stays at the end.
    pub fn push(&mut self, event: SessionEvent, cx: &mut Context<Self>) {
        self.events.push(event);
        if self.loaded {
            let last = self.rows.len().checked_sub(1).and_then(|ix| self.list.bounds_for_item(ix));
            let at_end = last.is_some_and(|row| row.bottom() <= self.list.viewport_bounds().bottom() + px(1.));
            self.refold(cx);
            if at_end {
                self.list.scroll_to_end();
            }
        }
    }

    pub fn fail(&mut self, failure: String, cx: &mut Context<Self>) {
        self.failure = Some(failure);
        cx.notify();
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// How many times a row has been built since the view was made.
    pub fn built(&self) -> usize {
        self.built
    }

    /// How many markdown rows have been parsed, each once.
    pub fn parsed(&self) -> usize {
        self.bodies.len()
    }

    pub fn list(&self) -> &ListState {
        &self.list
    }

    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to_end();
        cx.notify();
    }

    /// Scrolls from the top to the bottom at `px_per_s`, pushing the time of every presented frame into `frames`.
    pub fn drive(&mut self, px_per_s: f32, start: Instant, frames: Rc<RefCell<Vec<Instant>>>, cx: &mut Context<Self>) {
        self.list.scroll_to(gpui_kit::ListOffset::default());
        self.drive = Some(Drive { px_per_s, start, frames, done: false });
        cx.notify();
    }

    pub fn driven_to_end(&self) -> bool {
        self.drive.as_ref().is_some_and(|d| d.done)
    }

    pub fn watch_presented(&mut self, thread: &str, cell: Rc<RefCell<Option<Instant>>>, cx: &mut Context<Self>) {
        self.presented = Some((thread.to_owned(), cell));
        cx.notify();
    }

    /// Folds the events again and replaces only the rows that changed, so the list keeps every other row's height.
    fn refold(&mut self, cx: &mut Context<Self>) {
        let next = rows(&self.events, &self.opened);
        let same_start = self.rows.iter().zip(&next).take_while(|(a, b)| a == b).count();
        let room = self.rows.len().min(next.len()) - same_start;
        let same_end = self.rows.iter().rev().zip(next.iter().rev()).take(room).take_while(|(a, b)| a == b).count();
        let keys: HashSet<&str> = next.iter().map(|r| r.key.as_str()).collect();
        self.bodies.retain(|key, _| keys.contains(key.as_str()));
        for row in &next[same_start..next.len() - same_end] {
            if let Some(body) = self.bodies.get(&row.key) {
                body.update(cx, |body, cx| body.set_text(&row.text, cx));
            }
        }
        self.list.splice(same_start..self.rows.len() - same_end, next.len() - same_start - same_end);
        self.rows = next;
        cx.notify();
    }

    fn toggle(&mut self, turn: String, cx: &mut Context<Self>) {
        if !self.opened.remove(&turn) {
            self.opened.insert(turn);
        }
        self.refold(cx);
    }

    fn body(&mut self, row: &Row, cx: &mut Context<Self>) -> Entity<TextViewState> {
        self.bodies.entry(row.key.clone()).or_insert_with(|| cx.new(|cx| TextViewState::markdown(&row.text, cx))).clone()
    }

    fn render_row(&mut self, ix: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.built += 1;
        let Some(row) = self.rows.get(ix).cloned() else { return div().into_any_element() };
        let theme = cx.theme();
        let (muted, danger, bubble) = (theme.muted_foreground, theme.danger, theme.secondary);
        let line = || div().px_4().py_1().text_sm();
        let shown = match row.kind {
            RowKind::Person => {
                let body = self.body(&row, cx);
                h_flex()
                    .w_full()
                    .px_4()
                    .py_2()
                    .justify_end()
                    .child(div().max_w(PERSON_WIDTH).px_3().py_2().rounded_lg().bg(bubble).child(TextView::new(&body)))
                    .into_any_element()
            }
            RowKind::Agent => {
                let body = self.body(&row, cx);
                div().px_4().py_2().child(TextView::new(&body)).into_any_element()
            }
            RowKind::Fold => {
                let turn = row.turn.clone().unwrap_or_default();
                line()
                    .id(SharedString::from(row.key.clone()))
                    .text_color(muted)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(turn.clone(), cx)))
                    .child(row.text)
                    .into_any_element()
            }
            RowKind::Calls | RowKind::Notice => line().text_color(muted).child(row.text).into_any_element(),
            RowKind::Error => line().text_color(danger).child(row.text).into_any_element(),
        };
        let column = div().debug_selector(|| format!("row-{ix}")).mx_auto().w_full().max_w(COLUMN);
        div().w_full().child(column.child(shown)).into_any_element()
    }
}

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let frames = self.drive.as_ref().filter(|d| !d.done).map(|d| d.frames.clone());
        let shown = self.loaded && !self.rows.is_empty();
        let presented = match self.presented.take() {
            Some((thread, cell)) if shown && self.thread.as_deref() == Some(thread.as_str()) => Some(cell),
            waiting => {
                self.presented = waiting;
                None
            }
        };
        let muted = cx.theme().muted_foreground;
        let status = match (&self.failure, &self.thread, self.loaded) {
            (Some(failure), _, _) => Some(failure.clone()),
            (None, None, _) => Some("Pick a thread".to_owned()),
            (None, Some(_), false) => Some("Loading".to_owned()),
            (None, Some(_), true) if self.rows.is_empty() => Some("Nothing in this thread yet".to_owned()),
            _ => None,
        };
        v_flex()
            .size_full()
            .relative()
            .child(list(self.list.clone(), cx.processor(Self::render_row)).size_full())
            .children(status.map(|s| div().absolute().top_4().left_4().text_sm().text_color(muted).child(s)))
            .when(frames.is_some() || presented.is_some(), |this| {
                this.child(
                    canvas(
                        |_, _, _| {},
                        // A deferred callback runs when the update that paints ends, right after GPUI presents the frame.
                        move |_, _, _, cx| {
                            if let Some(frames) = frames.clone() {
                                cx.defer(move |_| frames.borrow_mut().push(Instant::now()));
                            }
                            if let Some(cell) = presented.clone() {
                                cx.defer(move |_| {
                                    cell.borrow_mut().get_or_insert_with(Instant::now);
                                });
                            }
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
    }
}
