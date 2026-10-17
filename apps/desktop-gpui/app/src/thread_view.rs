// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's transcript in a virtual list, following the thread live. The list builds only the rows on screen and
//! keeps the measured height of every row it has built. Each markdown row's parsed document lives here while the row
//! is within a few screens of the view, so a short scroll back does not parse again, and a long thread holds no more
//! documents than a short one. A live event refolds only the turn it lands in, and a streaming reply's document takes
//! each delta as an append.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{v_flex, ActiveTheme};
use gpui_kit::{
    canvas, div, list, prelude::*, px, AnyElement, Context, Entity, FollowMode, ListAlignment, ListOffset, ListState, Pixels, Task, Window,
};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{SessionEvent, SessionsReadRequest};
use wsp_host_client::{Failure, HostClient, Live};

use crate::screens::thread_view::{
    header,
    minimap::minimap,
    rows::{self, Clock},
};
use crate::sidebar::ThreadItem;
use crate::transcript::{Row, RowKind, Splice, Transcript};

/// How far past the viewport the list measures rows, so a row is built and measured before it scrolls in.
const OVERDRAW: Pixels = px(800.);
/// The shortest row, a one-line notice, so a screen's height over it is the most rows a screen can hold.
const ROW_MIN: Pixels = px(28.);
/// The height a row is taken to have until it is measured, the web app's estimatedItemSize: without one an unmeasured
/// row counts as 0 px, and the list cannot tell where its end is.
const ROW_ESTIMATE: Pixels = px(90.);
/// The waits between tries to follow the thread again after the host's socket closes, the last one repeating.
const RETRY: [Duration; 4] = [Duration::from_millis(250), Duration::from_secs(1), Duration::from_secs(3), Duration::from_secs(10)];

/// A scroll driven by the clock, as the bench runs it: the offset is set from elapsed time on every frame.
struct Drive {
    px_per_s: f32,
    start: Instant,
    frames: Rc<RefCell<Vec<Instant>>>,
    done: bool,
}

enum Arrival {
    History(Vec<SessionEvent>),
    Event(Box<SessionEvent>),
    /// The socket closed or the host failed; the follower tries again after a wait.
    Lost(String),
    Failed(String),
}

/// One thread followed live; dropping it stops the host task and the receiver.
struct Following {
    task: tokio::task::AbortHandle,
    _receive: Task<()>,
    /// The runtime the host task runs on, which stops every task on it when its last handle drops.
    _runtime: Arc<Runtime>,
}

impl Drop for Following {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Whether the view is in step with the host.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Live,
    /// The host's socket closed; the view shows what it had and follows again once the host answers.
    Lost(String),
}

pub struct ThreadView {
    thread: Option<String>,
    /// The project and title the header's breadcrumb reads.
    heading: Option<(String, String)>,
    transcript: Transcript,
    bodies: HashMap<String, Entity<TextViewState>>,
    clocks: HashMap<String, Entity<Clock>>,
    list: ListState,
    loaded: bool,
    failure: Option<String>,
    link: Link,
    following: Option<Following>,
    built: usize,
    parsed: usize,
    drive: Option<Drive>,
    /// A row to stand at a height once the frame that measures the rows above it has drawn.
    revealing: Option<(usize, Pixels)>,
    /// A fixed "now" for timers, in ms since the epoch, so captures read the same on every run.
    now: Option<f64>,
    /// A thread, and the cell that takes the time of the first presented frame showing its rows once it is loaded.
    presented: Option<(String, Rc<RefCell<Option<Instant>>>)>,
}

impl ThreadView {
    pub fn new(_: &mut Context<Self>) -> Self {
        ThreadView {
            thread: None,
            heading: None,
            transcript: Transcript::new(),
            bodies: HashMap::new(),
            clocks: HashMap::new(),
            list: ListState::new(0, ListAlignment::Top, OVERDRAW),
            loaded: false,
            failure: None,
            link: Link::Live,
            following: None,
            built: 0,
            parsed: 0,
            drive: None,
            revealing: None,
            now: None,
            presented: None,
        }
    }

    /// Empties the view for another thread, which shows once its transcript arrives.
    pub fn show(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.following = None;
        self.thread = Some(thread.to_owned());
        self.heading = None;
        self.transcript = Transcript::new();
        self.bodies.clear();
        self.clocks.clear();
        self.list.reset(0);
        self.loaded = false;
        self.failure = None;
        self.link = Link::Live;
        cx.notify();
    }

    /// Shows the thread's transcript, read whole, then follows it live; when the host's socket closes it reads the
    /// transcript again and goes on following. With `strict`, as the bench runs, a failure ends the process instead.
    pub fn follow(&mut self, host: Arc<HostClient>, runtime: &Arc<Runtime>, item: &ThreadItem, strict: bool, cx: &mut Context<Self>) {
        self.show(&item.thread_id, cx);
        self.heading = Some((item.project.clone(), item.title.clone()));
        let (workspace, thread) = (item.workspace_id.clone(), item.thread_id.clone());
        let (tx, mut rx) = mpsc::unbounded();
        let task = runtime.spawn(async move {
            let mut tries = 0usize;
            loop {
                let followed: Result<(), Failure> = async {
                    let mut opened = host.open(&workspace, &thread).await?;
                    tries = 0;
                    _ = tx.unbounded_send(Arrival::History(std::mem::take(&mut opened.history)));
                    mark_read(&host, &thread).await;
                    while let Some(next) = opened.next().await {
                        if let Live::Event(event) = next? {
                            let ended = matches!(*event, SessionEvent::SessionDone { .. } | SessionEvent::SessionEnd { .. });
                            _ = tx.unbounded_send(Arrival::Event(event));
                            if ended {
                                mark_read(&host, &thread).await;
                            }
                        }
                    }
                    Err(Failure::new("The host closed the connection".to_owned()))
                }
                .await;
                let Err(failure) = followed else { return };
                if strict {
                    _ = tx.unbounded_send(Arrival::Failed(failure.message));
                    return;
                }
                if tx.unbounded_send(Arrival::Lost(failure.message)).is_err() {
                    return;
                }
                tokio::time::sleep(RETRY[tries.min(RETRY.len() - 1)]).await;
                tries += 1;
            }
        });
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        self.following = Some(Following { task: task.abort_handle(), _receive: receive, _runtime: runtime.clone() });
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::History(events) => {
                let following = self.list.is_following_tail() || !self.loaded;
                self.link = Link::Live;
                self.load(events, cx);
                if following {
                    self.scroll_to_end(cx);
                }
            }
            Arrival::Event(event) => self.push(*event, cx),
            Arrival::Lost(why) => {
                self.link = Link::Lost(why);
                cx.notify();
            }
            Arrival::Failed(failure) => {
                eprintln!("desktop-gpui: {failure}");
                std::process::exit(1);
            }
        }
    }

    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    pub fn link(&self) -> &Link {
        &self.link
    }

    /// The whole transcript as it stands, which replaces anything shown before.
    pub fn load(&mut self, events: Vec<SessionEvent>, cx: &mut Context<Self>) {
        self.transcript.load(events);
        self.loaded = true;
        self.bodies.clear();
        self.clocks.clear();
        self.list.reset_with_uniform_height(self.transcript.rows().len(), ROW_ESTIMATE);
        cx.notify();
    }

    /// An event the host pushed after the transcript was read; a view at its end stays at the end.
    pub fn push(&mut self, event: SessionEvent, cx: &mut Context<Self>) {
        let splice = self.transcript.push(event);
        if self.loaded {
            self.apply(splice, cx);
        }
    }

    pub fn fail(&mut self, failure: String, cx: &mut Context<Self>) {
        self.failure = Some(failure);
        cx.notify();
    }

    pub fn rows(&self) -> &[Row] {
        self.transcript.rows()
    }

    /// How many times a row has been built since the view was made.
    pub fn built(&self) -> usize {
        self.built
    }

    /// How many times a markdown row has been parsed from scratch since the view was made.
    pub fn parsed(&self) -> usize {
        self.parsed
    }

    /// How many parsed markdown rows the view holds now.
    pub fn kept(&self) -> usize {
        self.bodies.len()
    }

    pub fn list(&self) -> &ListState {
        &self.list
    }

    /// Timers read `now_ms` from here on instead of the wall clock.
    pub fn freeze_clock(&mut self, now_ms: f64, cx: &mut Context<Self>) {
        self.now = Some(now_ms);
        self.clocks.clear();
        cx.notify();
    }

    /// Follows the end until the person scrolls up, also while a reply past gpui-kit's 4 KB parses in the background
    /// and grows the last row after the first frame.
    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Tail);
        cx.notify();
    }

    /// Shows the transcript from its first row, no longer following the end.
    pub fn scroll_to_top(&mut self, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Normal);
        self.list.scroll_to(ListOffset::default());
        cx.notify();
    }

    /// Stands row `ix` at `y` below the list's top, no longer following the end. Rows above it only have estimated
    /// heights until drawn, so each frame moves it by what the last one drew until it stands there.
    pub fn reveal(&mut self, ix: usize, y: Pixels, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Normal);
        self.list.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
        self.revealing = Some((ix, y));
        cx.notify();
    }

    /// Scrolls from the top to the bottom at `px_per_s`, pushing the time of every presented frame into `frames`.
    pub fn drive(&mut self, px_per_s: f32, start: Instant, frames: Rc<RefCell<Vec<Instant>>>, cx: &mut Context<Self>) {
        self.list.scroll_to(ListOffset::default());
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

    /// Opens or shuts a turn's fold, a tool group or a call.
    pub fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        let splice = self.transcript.toggle(id);
        self.apply(splice, cx);
    }

    /// Hands the changed rows' new text to the documents they hold, which append what extends their text, and tells
    /// the list which rows to measure again so it keeps every other row's height.
    fn apply(&mut self, splice: Option<Splice>, cx: &mut Context<Self>) {
        let Some(Splice { start, removed, inserted }) = splice else { return };
        let rows = self.transcript.rows();
        let keys: HashSet<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        self.bodies.retain(|key, _| keys.contains(key.as_str()));
        self.clocks.retain(|key, _| keys.contains(key.as_str()));
        for row in &rows[start..start + inserted] {
            if let Some(body) = self.bodies.get(&row.key) {
                let text = rows::body_text(row);
                body.update(cx, |body, cx| body.set_text(&text, cx));
            }
        }
        let reaches_end = start + inserted == rows.len();
        if removed == inserted {
            // A splice would move a view whose top sits inside one of these rows to that row's top.
            self.list.remeasure_items(start..start + inserted);
        } else {
            self.list.splice(start..start + removed, inserted);
        }
        if reaches_end && start > 0 && removed != inserted {
            // The row that was last keeps the list's end space below it until it is measured again.
            self.list.remeasure_items(start - 1..start);
        }
        cx.notify();
    }

    pub(crate) fn body(&mut self, row: &Row, cx: &mut Context<Self>) -> Entity<TextViewState> {
        let parsed = &mut self.parsed;
        let body = self.bodies.entry(row.key.clone()).or_insert_with(|| {
            *parsed += 1;
            let text = rows::body_text(row);
            cx.new(|cx| TextViewState::markdown(&text, cx))
        });
        body.clone()
    }

    pub(crate) fn clock(&mut self, row: &Row, since: f64, cx: &mut Context<Self>) -> Entity<Clock> {
        let now = self.now;
        self.clocks.entry(row.key.clone()).or_insert_with(|| cx.new(|cx| Clock::new(since, now, cx))).clone()
    }

    /// Drops the parsed rows more than two screens above the view's top or one screen past its overdraw. The list
    /// builds from up to a screen above its top (while it follows the end) to the overdraw below, so no row it builds
    /// is dropped, and a scroll back of a screen finds its rows still parsed.
    fn trim(&mut self) {
        let screen = self.list.viewport_bounds().size.height;
        if screen <= px(0.) {
            return;
        }
        let rows = |height: Pixels| (height / ROW_MIN).ceil() as usize;
        let top = self.list.logical_scroll_top().item_ix;
        let all = self.transcript.rows();
        let near = top.saturating_sub(2 * rows(screen))..(top + rows(screen + OVERDRAW + screen)).min(all.len());
        let keep: HashSet<&str> = all.get(near).unwrap_or_default().iter().map(|r| r.key.as_str()).collect();
        self.bodies.retain(|key, _| keep.contains(key.as_str()));
        self.clocks.retain(|key, _| keep.contains(key.as_str()));
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.built += 1;
        let Some(row) = self.transcript.rows().get(ix).cloned() else { return div().into_any_element() };
        rows::row(self, ix, &row, window, cx)
    }
}

/// `useReadStamp`: the open thread is read when it opens and again whenever a turn of it ends. A refusal is the host's
/// to log; the transcript shows either way.
async fn mark_read(host: &HostClient, thread: &str) {
    if let Err(failure) = host.call(SessionsReadRequest { thread_id: thread.to_owned() }).await {
        eprintln!("desktop-gpui: marking {thread} read: {}", failure.message);
    }
}

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trim();
        if let Some((ix, y)) = self.revealing.take() {
            let top = self.list.viewport_bounds().origin.y;
            let off = self.list.bounds_for_item(ix).map(|b| b.origin.y - top - y);
            match off {
                Some(off) if off.abs() < px(0.5) => {}
                Some(off) => {
                    self.list.scroll_by(off);
                    self.revealing = Some((ix, y));
                }
                None => self.revealing = Some((ix, y)),
            }
            window.request_animation_frame();
        }
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
        let shown = self.loaded && !self.transcript.rows().is_empty();
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
            (None, Some(_), true) if self.transcript.rows().is_empty() => Some("Nothing in this thread yet".to_owned()),
            _ => None,
        };
        let lost = match &self.link {
            Link::Lost(_) => Some(rows::lost_line(cx)),
            Link::Live => None,
        };
        let heading = self.heading.as_ref().map(|(project, title)| header::header(project, title, cx));
        let marks: Vec<(usize, bool)> = self
            .transcript
            .rows()
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.kind, RowKind::Person { .. }))
            .map(|(ix, _)| (ix, self.list.item_is_above_viewport(ix) == Some(false) && self.list.item_is_below_viewport(ix) == Some(false)))
            .collect();
        let rail = minimap(marks, self.list.viewport_bounds().size.width, window.viewport_size().height, cx);
        v_flex()
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .children(heading)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(list(self.list.clone(), cx.processor(Self::render_row)).size_full())
                    .children(rail),
            )
            .children(status.map(|s| div().absolute().top_4().left_4().text_sm().text_color(muted).child(s)))
            .children(lost)
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
