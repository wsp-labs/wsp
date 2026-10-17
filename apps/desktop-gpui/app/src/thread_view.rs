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

use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{v_flex, ActiveTheme};
use gpui_kit::{
    canvas, div, list, prelude::*, px, AnyElement, Context, Entity, FollowMode, ListAlignment, ListOffset, ListState, Pixels, Task, Window,
};
use gpui_kit::{App, StyleRefinement, Subscription, WeakEntity};
use wsp_host_client::protocol::{SessionEvent, SessionsReadRequest, SessionsStartRequest};
use wsp_host_client::{Failure, HostClient, Live, Op, Page, Unopened};

use crate::screens::app_shell::keys;
use crate::screens::thread_view::{
    cards,
    find::{self, Find},
    header, markdown,
    minimap::{minimap, Rail},
    rows::{self, Clock},
    shimmer::{self, ClockLook, Follow, Shimmer, Spot},
    toolbar::{self, Quote},
};
use crate::screens::Host;
use crate::sidebar::ThreadItem;
use crate::transcript::{Can, Row, RowKind, Splice, Transcript};
use crate::ui::ActivePalette;

/// How far past the viewport the list measures rows, so a row is built and measured before it scrolls in.
const OVERDRAW: Pixels = px(800.);
/// How many rows past those the list built last keep their parsed documents: about two screens of rows each way.
const KEEP_PAST: usize = 16;
/// The height a row is taken to have until it is measured, the web app's estimatedItemSize: without one an unmeasured
/// row counts as 0 px, and the list cannot tell where its end is.
const ROW_ESTIMATE: Pixels = px(90.);
/// How far above its end a reader still follows the thread: the web app's TIMELINE_FOLLOW_REARM_THRESHOLD_PX.
const FOLLOW_BAND: Pixels = px(40.);
/// The waits between tries to follow the thread again after the host's socket closes, the last one repeating.
pub(crate) const RETRY: [Duration; 4] =
    [Duration::from_millis(250), Duration::from_secs(1), Duration::from_secs(3), Duration::from_secs(10)];

/// A scroll driven by the clock, as the bench runs it: the offset is set from elapsed time on every frame.
struct Drive {
    px_per_s: f32,
    start: Instant,
    frames: Rc<RefCell<Vec<Instant>>>,
    done: bool,
}

enum Arrival {
    /// The thread's newest page, how many events of it the host keeps, whether its caps dropped older ones, and what to
    /// tell once a frame drawing it is presented: the listing's reads wait for that, off the switch's path.
    History {
        events: Vec<SessionEvent>,
        total: u64,
        trimmed: bool,
        drawn: oneshot::Sender<()>,
    },
    /// The host refused to read the thread; the view says why and reads nothing more.
    Refused(String),
    /// The thread opened or a turn of it ended: a person who can see it has read it.
    Seen,
    /// What the thread's agent can do (Fork from here, a conversation rewind) and the title of the thread it forks.
    Facts(Can, Option<String>),
    Event(Box<SessionEvent>),
    /// The socket closed or the host failed; the follower tries again after a wait.
    Lost(String),
    Failed(String),
}

/// One thread followed live; dropping it stops the host task and the receiver.
struct Following {
    _follow: Task<()>,
    _receive: Task<()>,
    host: Host,
    workspace: String,
    thread: String,
}

/// Whether the view is in step with the host.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Live,
    /// The host's socket closed; the view shows what it had and follows again once the host answers.
    Lost(String),
}

/// What the thread view tells the screen around it.
#[derive(Clone, Debug, PartialEq)]
pub enum ThreadViewEvent {
    /// Quote pressed over a selection in a reply: the text the composer takes in, as the web words it.
    Quote(String),
    /// Fork from here pressed: the thread the press was drawn for and the turn the fork is cut at, none on the
    /// thread's first message; on a person's message, its words and its files.
    Fork { thread: String, turn: Option<String>, message: Option<(String, Vec<crate::transcript::FileTile>)> },
    /// The transcript was read whole, first or again after the socket closed.
    Loaded,
    /// One live event the view took, now the transcript's last.
    Arrived,
    /// The host's socket closed; `Loaded` follows once the view follows the thread again.
    Lost,
    /// Find closed with nothing focused before it opened: the composer takes the caret, as `requestComposerFocus` does.
    FocusComposer,
    /// The start line's Try again sent `prompt` on `thread`: where it stands as that thread's draft, it goes.
    SentElsewhere { thread: String, prompt: String },
}

/// A scroll to the end eased over time, as the web's `scrollToEnd({ animated: true })`: where it started, and when.
struct Glide {
    from: Pixels,
    start: Instant,
    took: Duration,
}

/// The most frames a reveal moves the list for before it gives up on a row that cannot stand at its height.
const REVEAL_TRIES: u32 = 60;

/// A row to stand at `y` below the list's top, and the scroll offset the last frame's move started from.
struct Reveal {
    ix: usize,
    y: Pixels,
    tries: u32,
    from: Option<Pixels>,
}

pub struct ThreadView {
    /// The list in a view of its own, drawn cached, so a view that animates beside it does not build its rows again.
    rows_view: Entity<Rows>,
    thread: Option<String>,
    transcript: Transcript,
    bodies: HashMap<String, Entity<TextViewState>>,
    /// Each row's clock, with the start it counts from.
    clocks: HashMap<String, (f64, Entity<Clock>)>,
    /// Where the reader stood in the thread's own rows while a subagent's page stands in their place: the row and the
    /// offset into it, or None where they followed the end.
    left: Option<Option<(String, Pixels)>>,
    /// Each person's message's attachments by row key, one slot per file, kept while the row is near like `bodies`.
    pub(crate) images: HashMap<String, Vec<cards::Shot>>,
    /// Images rows drew and no longer hold, which the next draw takes out of GPUI's cache and the window's atlas.
    released: Vec<Arc<gpui_kit::Image>>,
    /// What the host said about a pick it would not take, by the prompt's ask id, so it outlives the row's rebuilds.
    pub(crate) refusals: HashMap<String, cards::Refusal>,
    list: ListState,
    loaded: bool,
    failure: Option<String>,
    /// Why the host would not read the thread, said where the transcript would be.
    refused: Option<String>,
    /// Paging back: the position of the oldest event held, whether the host holds nothing older, whether its caps
    /// dropped older events, the one page read in flight, and whether the list has stood at its end since it loaded.
    oldest: Option<u64>,
    complete: bool,
    trimmed: bool,
    paging: Option<Task<()>>,
    /// Older page reads that failed in a row, which the wait before the next grows with.
    page_tries: usize,
    settled: bool,
    /// Every older page is read whatever the scroll, as the bench asks before it scrolls the whole thread.
    whole: bool,
    /// Told once a frame drawing the newest page read is presented.
    drawn: Option<oneshot::Sender<()>>,
    /// A read stamp waits for the window to be active, and whether it was when last drawn.
    stamp_due: bool,
    active: bool,
    _activation: Option<Subscription>,
    link: Link,
    following: Option<Following>,
    built: usize,
    parsed: usize,
    loads: usize,
    revision: usize,
    /// The first and last row the list built in its latest layout, and the frame that layout belongs to.
    built_span: Option<(usize, usize)>,
    span_frame: u64,
    frame: u64,
    drive: Option<Drive>,
    glide: Option<Glide>,
    /// A row to stand at a height once the frame that measures the rows above it has drawn.
    revealing: Option<Reveal>,
    /// A fixed "now" for timers, in ms since the epoch, so captures read the same on every run.
    now: Option<f64>,
    /// A thread, and the cell that takes the time of the first presented frame showing its rows once it is loaded.
    presented: Option<(String, Rc<RefCell<Option<Instant>>>)>,
    /// The overlays': the transcript's focus, find, the Quote button, the rail's hover, the scroll-to-end pill's last
    /// state (None until it first shows), each live line's shimmer, and how many times the view has drawn.
    focus: gpui_kit::FocusHandle,
    /// The docked composer's height, which the list ends with.
    composer_inset: Pixels,
    find: Find,
    quote: Option<Quote>,
    rail: Rail,
    pill: Option<bool>,
    shimmers: HashMap<String, Entity<Shimmer>>,
    /// Where each row's clock stands, which the overlay draws it at, and the frame a row last claimed it.
    clock_spots: HashMap<String, (Spot, ClockLook, u64)>,
    renders: usize,
    feed: Feed,
    /// A Threads row's right-click menu, the same acts as its More.
    pub(crate) row_menu: crate::ui::context_menu::ContextMenu,
}

/// Where the agent's catalog flags stand for the thread followed: its page not drawn yet, drawn and due, or asked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum FactsStep {
    #[default]
    Undrawn,
    Due,
    Asked,
}

/// The listing's feed off the sidebar's model: the sidebar followed, the revision of its rows last read, and the
/// catalog's step.
#[derive(Default)]
struct Feed {
    sidebar: Option<gpui_kit::WeakEntity<crate::sidebar::Sidebar>>,
    /// The sidebar's revision and the thread the listing was last read at.
    revision: Option<(u64, String)>,
    facts: FactsStep,
    _observed: Option<Subscription>,
}

impl gpui_kit::EventEmitter<ThreadViewEvent> for ThreadView {}

impl ThreadView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Top, OVERDRAW);
        let (me, weak) = (cx.entity(), cx.weak_entity());
        let rows_view = cx.new(|cx| Rows::new(&me, weak, list.clone(), cx));
        // Every row's height moves with the text sizes the person picks.
        cx.observe_global::<markdown::Sizes>(|this, cx| {
            this.list.remeasure();
            cx.notify();
        })
        .detach();
        ThreadView {
            rows_view,
            thread: None,
            transcript: Transcript::new(),
            bodies: HashMap::new(),
            clocks: HashMap::new(),
            left: None,
            images: HashMap::new(),
            released: Vec::new(),
            refusals: HashMap::new(),
            list,
            loaded: false,
            failure: None,
            refused: None,
            oldest: None,
            complete: true,
            trimmed: false,
            paging: None,
            page_tries: 0,
            settled: false,
            whole: false,
            drawn: None,
            stamp_due: false,
            active: false,
            _activation: None,
            link: Link::Live,
            following: None,
            built: 0,
            parsed: 0,
            loads: 0,
            revision: 0,
            built_span: None,
            span_frame: 0,
            frame: 0,
            drive: None,
            glide: None,
            revealing: None,
            now: None,
            presented: None,
            focus: cx.focus_handle(),
            composer_inset: rows::COMPOSER_INSET,
            find: Find::default(),
            quote: None,
            rail: Rail::default(),
            pill: None,
            shimmers: HashMap::new(),
            clock_spots: HashMap::new(),
            renders: 0,
            feed: Feed::default(),
            row_menu: Default::default(),
        }
    }

    /// Empties the view for another thread, which shows once its transcript arrives.
    pub fn show(&mut self, thread: &str, cx: &mut Context<Self>) {
        self.following = None;
        self.thread = Some(thread.to_owned());
        self.transcript = Transcript::new();
        self.bodies.clear();
        self.clocks.clear();
        self.left = None;
        cards::drop_images(&mut self.images, &mut self.released, |_| false);
        self.refusals.clear();
        self.built_span = None;
        self.list.reset(0);
        self.loaded = false;
        self.failure = None;
        self.refused = None;
        self.oldest = None;
        self.complete = true;
        self.trimmed = false;
        self.paging = None;
        self.page_tries = 0;
        self.settled = false;
        self.whole = false;
        self.drawn = None;
        self.stamp_due = false;
        self.link = Link::Live;
        self.find.switch();
        self.rail.active = None;
        self.quote = None;
        self.pill = None;
        self.glide = None;
        self.revealing = None;
        self.shimmers.clear();
        self.clock_spots.clear();
        cx.notify();
    }

    /// Stops following the open thread; what it shows stays until the next `show`.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.following = None;
        cx.notify();
    }

    /// Shows the thread from its newest page, then follows it live and reads older pages as the reader nears the top;
    /// when the host's socket closes it reads the newest page again and goes on following. A refusal to read the
    /// thread is said and not retried. With `strict`, as the bench runs, a failure ends the process instead.
    pub fn follow(&mut self, host: &Host, item: &ThreadItem, strict: bool, cx: &mut Context<Self>) {
        self.show(&item.thread_id, cx);
        self.feed.facts = FactsStep::Undrawn;
        self.feed.revision = None;
        let (workspace, thread) = (item.workspace_id.clone(), item.thread_id.clone());
        let (tx, mut rx) = mpsc::unbounded();
        let job = host.run(move |host| async move {
            let mut tries = 0usize;
            loop {
                let followed: Result<(), Failure> = async {
                    let mut opened = match host.open_page(&workspace, &thread).await {
                        Ok(opened) => opened,
                        Err(Unopened::Lost(failure)) => return Err(failure),
                        Err(Unopened::Refused(failure)) if strict => return Err(failure),
                        Err(Unopened::Refused(failure)) => {
                            _ = tx.unbounded_send(Arrival::Refused(failure.message));
                            return Ok(());
                        }
                    };
                    tries = 0;
                    let events = std::mem::take(&mut opened.history);
                    let (drawn, presented) = oneshot::channel();
                    _ = tx.unbounded_send(Arrival::History { events, total: opened.total, trimmed: opened.trimmed, drawn });
                    // A view dropped before drawing it ends this task with it.
                    _ = presented.await;
                    _ = tx.unbounded_send(Arrival::Seen);
                    // The listing and its rows come from the sidebar's model, which follows the host's rows itself.
                    while let Some(next) = opened.next().await {
                        if let Live::Event(event) = next? {
                            let ended = matches!(*event, SessionEvent::SessionDone { .. } | SessionEvent::SessionEnd { .. });
                            _ = tx.unbounded_send(Arrival::Event(event));
                            if ended {
                                _ = tx.unbounded_send(Arrival::Seen);
                            }
                        }
                    }
                    Err(Failure::new("The host closed the connection".to_owned()))
                }
                .await;
                let Err(failure) = followed else { return Ok(()) };
                if strict {
                    _ = tx.unbounded_send(Arrival::Failed(failure.message));
                    return Ok(());
                }
                if tx.unbounded_send(Arrival::Lost(failure.message)).is_err() {
                    return Ok(());
                }
                tokio::time::sleep(RETRY[tries.min(RETRY.len() - 1)]).await;
                tries += 1;
            }
        });
        // Dropping the task drops the run, which stops the host task with it.
        let follow = cx.spawn(async move |_, _| _ = job.await);
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        self.following = Some(Following {
            _follow: follow,
            _receive: receive,
            host: host.clone(),
            workspace: item.workspace_id.clone(),
            thread: item.thread_id.clone(),
        });
        // The listing stands before the newest page lands, so the page folds once with it.
        self.relist(cx);
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::History { events, total, trimmed, drawn } => {
                self.feed.facts = FactsStep::Undrawn;
                self.drawn = Some(drawn);
                let following = self.list.is_following_tail() || !self.loaded;
                // A reader scrolled up stays on the row they read when the thread is read again after a lost socket.
                let top = self.list.logical_scroll_top();
                let kept =
                    (!following).then(|| self.transcript.rows().get(top.item_ix).map(|r| (r.key.clone(), top.offset_in_item))).flatten();
                self.link = Link::Live;
                // Pages read before the socket closed stay under the newest one read now, where the two meet: past more
                // events than a page holds, the events between are in neither, and the older pages are read again.
                let lo = events.iter().find_map(wsp_host_client::pos);
                let newest = self.transcript.event_list().iter().rev().find_map(wsp_host_client::pos);
                let held: Vec<SessionEvent> = match lo {
                    Some(lo) if self.loaded && newest.is_some_and(|n| n >= lo) => self
                        .transcript
                        .event_list()
                        .iter()
                        .take_while(|e| wsp_host_client::pos(e).is_some_and(|p| p < lo))
                        .cloned()
                        .collect(),
                    _ => Vec::new(),
                };
                let (was_trimmed, was_settled) = (self.loaded && self.trimmed, self.loaded && self.settled);
                let events: Vec<SessionEvent> = held.into_iter().chain(events).collect();
                self.load(events, cx);
                self.settled = was_settled;
                self.complete = recorded(self.transcript.event_list()) as u64 >= total;
                self.trimmed = trimmed || was_trimmed;
                if following {
                    self.scroll_to_end(cx);
                } else if let Some((key, offset)) = kept {
                    if let Some(ix) = self.transcript.rows().iter().position(|r| r.key == key) {
                        self.list.scroll_to(ListOffset { item_ix: ix, offset_in_item: offset });
                    }
                }
                cx.emit(ThreadViewEvent::Loaded);
            }
            Arrival::Event(event) => {
                self.push(*event, cx);
                cx.emit(ThreadViewEvent::Arrived);
            }
            Arrival::Facts(can, source) => {
                let first = self.transcript.can(can);
                if self.loaded {
                    self.apply(first, cx);
                }
                let second = self.transcript.forked_from(source);
                if self.loaded {
                    self.apply(second, cx);
                }
            }
            Arrival::Refused(why) => {
                self.refused = Some(format!("history unavailable: {why}"));
                self.loaded = true;
                cx.notify();
            }
            Arrival::Seen => {
                self.stamp_due = true;
                self.stamp(cx);
                if self.feed.facts == FactsStep::Undrawn {
                    self.feed.facts = FactsStep::Due;
                }
                self.ask_facts(cx);
            }
            Arrival::Lost(why) => {
                self.link = Link::Lost(why);
                cx.emit(ThreadViewEvent::Lost);
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
        self.revision += 1;
        self.oldest = self.transcript.event_list().iter().find_map(wsp_host_client::pos);
        self.complete = true;
        self.paging = None;
        self.page_tries = 0;
        self.settled = false;
        self.loaded = true;
        self.loads += 1;
        self.bodies.clear();
        self.clocks.clear();
        self.built_span = None;
        self.list.reset_with_uniform_height(self.transcript.rows().len(), ROW_ESTIMATE);
        cx.notify();
    }

    /// An event the host pushed after the transcript was read; a view at its end stays at the end.
    pub fn push(&mut self, event: SessionEvent, cx: &mut Context<Self>) {
        let splice = self.transcript.push(event);
        if self.loaded {
            if self.revealing.is_none() && !self.list.is_following_tail() && self.near_end() {
                self.list.set_follow_mode(FollowMode::Tail);
            }
            self.apply(splice, cx);
        }
    }

    /// Whether the reader stands within `FOLLOW_BAND` of the transcript's end.
    pub(crate) fn near_end(&self) -> bool {
        self.list.max_offset_for_scrollbar().y + self.list.scroll_px_offset_for_scrollbar().y <= FOLLOW_BAND
    }

    /// What the view says in place of rows: "Loading transcript" until the thread is read, and the empty thread's line.
    pub fn placeholder(&self) -> Option<&'static str> {
        match (&self.thread, self.loaded, &self.refused) {
            (Some(_), false, _) => Some("Loading transcript"),
            (Some(_), true, None) if self.transcript.rows().is_empty() => Some("Send a message to start the conversation."),
            _ => None,
        }
    }

    /// "history unavailable: <reason>" once the host refused to read the thread.
    pub fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    /// Whether the view holds every event the host keeps of the thread.
    pub fn complete(&self) -> bool {
        self.complete
    }

    /// Whether the host's caps dropped older events of the thread, which no page brings back.
    pub fn trimmed(&self) -> bool {
        self.trimmed
    }

    /// Reads every older page whatever the scroll, as the bench does before it scrolls the whole thread.
    pub fn read_whole(&mut self, cx: &mut Context<Self>) {
        if !self.whole {
            self.whole = true;
            cx.notify();
        }
    }

    /// Asks for the page before the oldest event held, one at a time, while the host holds older ones.
    fn older(&mut self, cx: &mut Context<Self>) {
        if self.paging.is_some() || self.complete || !self.loaded {
            return;
        }
        let (Some(before), Some(following)) = (self.oldest.filter(|&p| p > 0), &self.following) else { return };
        let (workspace, thread) = (following.workspace.clone(), following.thread.clone());
        let read = following.host.run(move |host| async move { host.page(&workspace, &thread, before).await });
        self.paging = Some(cx.spawn(async move |this, cx| {
            let page = read.await;
            _ = this.update(cx, |this, cx| {
                this.paging = None;
                this.land(before, page, cx);
            });
        }));
    }

    /// Lays an older page under the events held: the transcript folds the page and the turn it meets, and only the
    /// head of the list that changed is spliced, so every row below keeps its height and the reader stays on their row.
    fn land(&mut self, before: u64, page: Result<Page, Failure>, cx: &mut Context<Self>) {
        let page = match page {
            Ok(page) => page,
            Err(failure) => {
                // Asked again after a wait that grows, so a host that refuses pages is not asked every frame.
                eprintln!("desktop-gpui: reading an older page: {}", failure.message);
                let wait = RETRY[self.page_tries.min(RETRY.len() - 1)];
                self.page_tries += 1;
                self.paging = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    _ = this.update(cx, |this, cx| {
                        this.paging = None;
                        cx.notify();
                    });
                }));
                return;
            }
        };
        self.page_tries = 0;
        if self.oldest != Some(before) {
            return;
        }
        let taken: Vec<SessionEvent> = page.events.into_iter().filter(|e| wsp_host_client::pos(e).is_some_and(|p| p < before)).collect();
        let held = (recorded(&taken) + recorded(self.transcript.event_list())) as u64;
        self.complete = taken.is_empty() || held >= page.total;
        self.trimmed |= page.trimmed == Some(true);
        let top = self.list.logical_scroll_top();
        let key = |ix: usize| self.transcript.rows().get(ix).map(|r| r.key.clone());
        let (top_key, reveal_key) = (key(top.item_ix), self.revealing.as_ref().and_then(|r| key(r.ix)));
        let splice = self.transcript.prepend(taken);
        self.oldest = self.transcript.event_list().iter().find_map(wsp_host_client::pos);
        let new = self.transcript.rows();
        // A row by its key, else one folded again with the page as far above the kept rows as it stood.
        let moved = |ix: usize, key: Option<&String>| match splice {
            Some(Splice { start, removed, inserted }) if ix >= start => {
                if ix >= start + removed {
                    ix - removed + inserted
                } else {
                    let changed = &new[start..start + inserted];
                    key.and_then(|k| changed.iter().position(|r| r.key == *k))
                        .map_or_else(|| start + inserted.saturating_sub(start + removed - ix), |p| start + p)
                }
            }
            _ => ix,
        };
        let place = moved(top.item_ix, top_key.as_ref());
        if let Some(reveal) = &mut self.revealing {
            reveal.ix = moved(reveal.ix, reveal_key.as_ref());
            reveal.from = None;
        }
        let following = self.list.is_following_tail();
        self.apply(splice, cx);
        if !following {
            self.list.scroll_to(ListOffset { item_ix: place, offset_in_item: top.offset_in_item });
        }
        cx.notify();
    }

    /// Stamps the thread read on the host while the window is active, as the web stamps only a visible page; a stamp
    /// due while it is not waits for it to be.
    fn stamp(&mut self, cx: &mut Context<Self>) {
        if !self.stamp_due || !self.active {
            return;
        }
        let Some(following) = &self.following else { return };
        self.stamp_due = false;
        let thread = following.thread.clone();
        let stamped = following.host.run(move |host| async move {
            mark_read(&host, &thread).await;
            Ok(())
        });
        cx.spawn(async move |_, _| _ = stamped.await).detach();
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

    /// How many times the transcript has been read whole since the view was made.
    pub fn loads(&self) -> usize {
        self.loads
    }

    /// The text the parsed document of row `key` holds now, while the view keeps one for it.
    pub fn body_source(&self, key: &str, cx: &App) -> Option<String> {
        self.bodies.get(key).map(|b| b.read(cx).rendered_text().source().to_owned())
    }

    /// How many parsed markdown rows the view holds now.
    pub fn kept(&self) -> usize {
        self.bodies.len()
    }

    pub fn list(&self) -> &ListState {
        &self.list
    }

    /// Whether the open thread's transcript has been read.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    pub fn composer_inset(&self) -> Pixels {
        self.composer_inset
    }

    /// The composer docked over the list's foot is `height` tall now; the last row is measured again to clear it.
    pub fn set_composer_inset(&mut self, height: Pixels, cx: &mut Context<Self>) {
        self.composer_inset = height;
        let rows = self.transcript.rows().len();
        if rows > 0 {
            self.list.remeasure_items(rows - 1..rows);
        }
        cx.notify();
    }

    /// Whether the transcript or a reply in it holds the window's focus, not a field drawn over it such as Find's.
    pub fn holds_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx) && !self.find.typing(window, cx)
    }

    /// The fixed "now" timers read, where a capture set one.
    pub fn frozen(&self) -> Option<f64> {
        self.now
    }

    /// Timers read `now_ms` from here on instead of the wall clock.
    pub fn freeze_clock(&mut self, now_ms: f64, cx: &mut Context<Self>) {
        self.now = Some(now_ms);
        self.clocks.clear();
        cx.notify();
    }

    /// Stands the agent's catalog flags as the host's facts would, for a test or a capture.
    pub fn set_can(&mut self, can: Can, cx: &mut Context<Self>) {
        let splice = self.transcript.can(can);
        if self.loaded {
            self.apply(splice, cx);
        }
    }

    /// What the thread's agent can do, as its catalog said once read.
    pub fn agent_can(&self) -> Can {
        self.transcript.agent_can()
    }

    /// What the thread listing last said about this thread, once it has been read.
    pub fn listing(&self) -> Option<&crate::transcript::Listing> {
        self.transcript.listed()
    }

    /// Stands what the thread listing says, as the host's listing would, also for a test or a capture.
    pub fn set_listing(&mut self, listing: crate::transcript::Listing, cx: &mut Context<Self>) {
        let splice = self.transcript.listing(listing);
        if self.loaded {
            self.apply(splice, cx);
        }
    }

    /// Reads the listing again off the sidebar's model once its rows or the preferences moved; with no sidebar the
    /// listing stands as set. A listing that reads the same changes no row.
    pub(crate) fn relist(&mut self, cx: &mut Context<Self>) {
        if self.feed.sidebar.is_none() {
            let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) else { return };
            self.feed._observed = Some(cx.observe(&sidebar, |view, _, cx| view.relist(cx)));
            self.feed.sidebar = Some(sidebar.downgrade());
        }
        let Some(sidebar) = self.feed.sidebar.as_ref().and_then(|s| s.upgrade()) else { return };
        let Some(thread) = self.thread.clone() else { return };
        let read = (sidebar.read(cx).revision(), thread.clone());
        if self.feed.revision.as_ref() == Some(&read) {
            return;
        }
        self.feed.revision = Some(read);
        let at = self.now.unwrap_or_else(now_ms);
        let s = sidebar.read(cx);
        let listing = crate::transcript::Listing::listed(s.rows(), &thread, at, s.settle_ms(), s.preferences(), &s.here_name());
        // The sidebar's rows never know the send in flight; the composer's link sets and clears that.
        let starting = self.transcript.listed().filter(|l| l.thread == thread).and_then(|l| l.starting.clone());
        self.set_listing(crate::transcript::Listing { starting, ..listing }, cx);
        self.ask_facts(cx);
    }

    /// Takes the thread's agent's catalog flags once its page is drawn and the listing names its agent, off the
    /// switch's path, once each time the thread is read: from the workspace's catalogs the header keeps, listed once
    /// per host, waiting on them while they are read.
    fn ask_facts(&mut self, cx: &mut Context<Self>) {
        if self.feed.facts != FactsStep::Due {
            return;
        }
        let (Some(workspace), Some(thread)) = (self.followed_workspace().map(str::to_owned), self.followed().map(str::to_owned)) else {
            return;
        };
        let Some(listing) = self.transcript.listed().filter(|l| l.thread == thread && l.workspace.is_some()) else { return };
        let harness = listing.harness.clone().unwrap_or_else(|| "claude".to_owned());
        let source = listing.source.as_ref().map(|s| s.title.clone());
        match header::catalogs_of(&workspace, cx) {
            Some(catalogs) => {
                self.feed.facts = FactsStep::Asked;
                self.arrive(Arrival::Facts(can_of(&catalogs, &harness), source), cx);
            }
            None => header::want_catalogs(self, &workspace, Some(cx.weak_entity()), cx),
        }
    }

    /// The workspace's catalogs were read, or refused: the facts are taken now, or not asked again for this read.
    pub(crate) fn catalogs_landed(&mut self, landed: bool, cx: &mut Context<Self>) {
        if landed {
            self.ask_facts(cx);
        } else if self.feed.facts == FactsStep::Due {
            self.feed.facts = FactsStep::Asked;
        }
    }

    /// Shows one subagent's page in place of the thread's own rows, by the call that launched it, from its end; None
    /// shows the thread again where the reader left it.
    pub fn set_page(&mut self, call: Option<String>, cx: &mut Context<Self>) {
        let paged = call.is_some();
        if paged && self.left.is_none() {
            let top = self.list.logical_scroll_top();
            let row = self.transcript.rows().get(top.item_ix).map(|r| (r.key.clone(), top.offset_in_item));
            self.left = Some(row.filter(|_| !self.list.is_following_tail()));
        }
        let splice = self.transcript.page(call);
        if self.loaded {
            self.apply(splice, cx);
        }
        match (paged, self.left.take_if(|_| !paged)) {
            (true, _) => self.scroll_to_end(cx),
            (false, Some(Some((key, offset)))) => match self.transcript.rows().iter().position(|r| r.key == key) {
                Some(ix) => {
                    self.list.set_follow_mode(FollowMode::Normal);
                    self.list.scroll_to(ListOffset { item_ix: ix, offset_in_item: offset });
                }
                None => self.scroll_to_end(cx),
            },
            (false, Some(None)) => self.scroll_to_end(cx),
            (false, None) => {}
        }
    }

    /// Follows the end until the person scrolls up, also while a reply past gpui-kit's 4 KB parses in the background
    /// and grows the last row after the first frame.
    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Tail);
        cx.notify();
    }

    /// Scrolls to the end over a moment, then follows it, as Scroll to end does.
    pub fn glide_to_end(&mut self, cx: &mut Context<Self>) {
        let from = -self.list.scroll_px_offset_for_scrollbar().y;
        let distance = f32::from(self.list.max_offset_for_scrollbar().y - from).max(0.);
        // Chromium's smooth scroll takes longer the farther it goes, within these bounds.
        let took = Duration::from_millis((distance.sqrt() * 10.).clamp(150., 700.) as u64);
        self.list.set_follow_mode(FollowMode::Normal);
        self.glide = Some(Glide { from, start: cx.background_executor().now(), took });
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
        // Row 0's frame holds the list's top spacer, which the web draws as an element of its own above the first row.
        let y = if ix == 0 { y - rows::END_SPACE } else { y };
        self.list.set_follow_mode(FollowMode::Normal);
        self.list.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
        self.revealing = Some(Reveal { ix, y, tries: 0, from: None });
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
        // A fold opens downward from where it was pressed, so the end lets go of the view (list.tsx's suspendEndScrollMaintenanceForDisclosure).
        self.list.pause_following_tail();
        let splice = self.transcript.toggle(id);
        self.apply(splice, cx);
    }

    /// Hands the changed rows' new text to the documents they hold, which append what extends their text, and tells
    /// the list which rows to measure again so it keeps every other row's height.
    fn apply(&mut self, splice: Option<Splice>, cx: &mut Context<Self>) {
        let Some(Splice { start, removed, inserted }) = splice else { return };
        self.revision += 1;
        let rows = self.transcript.rows();
        let keys: HashSet<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        self.bodies.retain(|key, _| keys.contains(key.as_str()));
        self.clocks.retain(|key, _| keys.contains(key.as_str()));
        cards::drop_images(&mut self.images, &mut self.released, |key| keys.contains(key));
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
        let cwd = self.transcript.cwd().map(str::to_owned);
        let body = self.bodies.entry(row.key.clone()).or_insert_with(|| {
            *parsed += 1;
            let text = rows::body_text(row);
            cx.new(|cx| {
                let extensions = markdown::extensions(cwd.as_deref(), cx);
                TextViewState::markdown_with_extensions(&text, extensions, cx)
            })
        });
        body.clone()
    }

    pub(crate) fn clock(&mut self, row: &Row, since: f64, cx: &mut Context<Self>) -> Entity<Clock> {
        let now = self.now;
        match self.clocks.get(&row.key) {
            Some((held, clock)) if *held == since => clock.clone(),
            _ => {
                let clock = cx.new(|cx| Clock::new(since, now, cx));
                self.clocks.insert(row.key.clone(), (since, clock.clone()));
                clock
            }
        }
    }

    /// Drops the parsed rows more than `KEEP_PAST` rows away from the rows the list built in its last layout. Every row
    /// it builds keeps its document, so nothing parses again within a frame, and a short scroll back finds its rows
    /// still parsed; a long thread holds no more documents than a screen's rows and the margins.
    fn trim(&mut self) {
        let Some((lo, hi)) = self.built_span else { return };
        let all = self.transcript.rows();
        let near = lo.saturating_sub(KEEP_PAST)..(hi + 1 + KEEP_PAST).min(all.len());
        let keep: HashSet<&str> = all.get(near).unwrap_or_default().iter().map(|r| r.key.as_str()).collect();
        self.bodies.retain(|key, _| keep.contains(key.as_str()));
        self.clocks.retain(|key, _| keep.contains(key.as_str()));
        cards::drop_images(&mut self.images, &mut self.released, |key| keep.contains(key));
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.built += 1;
        self.built_span = Some(match self.built_span.filter(|_| self.span_frame == self.frame) {
            Some((lo, hi)) => (lo.min(ix), hi.max(ix)),
            None => (ix, ix),
        });
        self.span_frame = self.frame;
        let Some(mut row) = self.transcript.rows().get(ix).cloned() else { return div().into_any_element() };
        // Rewind to here stands on no reply while a turn runs. The rows keep it, so a turn starting or ending leaves every
        // row above it, and the height the list measured for it, alone.
        if let RowKind::Agent { rewind, .. } = &mut row.kind {
            rewind.take_if(|_| self.transcript.working());
        }
        rows::row(self, ix, &row, window, cx)
    }
}

impl ThreadView {
    /// The open thread's id on the host, while the view follows one.
    pub fn followed(&self) -> Option<&str> {
        self.following.as_ref().map(|f| f.thread.as_str())
    }

    /// Sends `prompt` to the open thread as the person's message, as the web app's "Send output to the agent" does
    /// (`startSession` with the thread named).
    pub fn send(&mut self, prompt: String, cx: &mut Context<Self>) {
        let (Some(workspace), Some(thread)) = (self.followed_workspace(), self.followed()) else { return };
        let request = serde_json::json!({ "workspaceId": workspace, "thread": thread, "prompt": prompt, "requestId": crate::screens::composer::link::request_id() });
        let Ok(start) = serde_json::from_value::<SessionsStartRequest>(request) else { return };
        self.call(start, cx, |view, answer, cx| {
            if let Err(failure) = answer {
                view.fail(failure.message, cx);
            }
        });
    }

    /// Whether a subagent's page stands in place of the thread's own rows.
    pub(crate) fn paged(&self) -> bool {
        self.left.is_some()
    }

    /// The host the view follows a thread on, while it follows one.
    pub(crate) fn host(&self) -> Option<&Host> {
        self.following.as_ref().map(|f| &f.host)
    }

    /// The open thread's workspace on the host, while the view follows one.
    pub fn followed_workspace(&self) -> Option<&str> {
        self.following.as_ref().map(|f| f.workspace.as_str())
    }

    /// Sends `op` to the host the view follows and hands its answer to `done` on the view; with no host, `done`
    /// hears why at once.
    pub fn call<O>(
        &mut self,
        op: O,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<O::Reply, Failure>, &mut Context<Self>) + 'static,
    ) where
        O: Op + Send + 'static,
        O::Reply: Send + 'static,
    {
        let Some(following) = &self.following else {
            done(self, Err(Failure::new("The thread view follows no host".to_owned())), cx);
            return;
        };
        let answer = following.host.run(move |host| async move { host.call(op).await });
        cx.spawn(async move |this, cx| {
            let answer = answer.await;
            _ = this.update(cx, |this, cx| done(this, answer, cx));
        })
        .detach();
    }
}

/// What the overlays read and keep on the view.
impl ThreadView {
    pub(crate) fn find(&mut self) -> &mut Find {
        &mut self.find
    }

    pub(crate) fn find_ref(&self) -> &Find {
        &self.find
    }

    /// Find's state, for a test or a capture.
    pub fn find_state(&self) -> &Find {
        &self.find
    }

    pub(crate) fn quote(&mut self) -> &mut Option<Quote> {
        &mut self.quote
    }

    /// The quote the Quote button would hand over, while it shows.
    pub fn quote_shown(&self) -> Option<String> {
        self.quote.as_ref().map(|q| toolbar::quote_text(&q.text, q.reply_to.as_deref()))
    }

    pub(crate) fn rail(&mut self) -> &mut Rail {
        &mut self.rail
    }

    /// The rail's hovered or focused message, as its index among the person messages.
    pub fn rail_active(&self) -> Option<usize> {
        self.rail.active
    }

    /// The rail's focus, which takes the keyboard's arrows, Home, End and Enter.
    pub fn rail_focus(&self) -> Option<gpui_kit::FocusHandle> {
        self.rail.focus_handle().cloned()
    }

    pub(crate) fn pill(&mut self) -> &mut Option<bool> {
        &mut self.pill
    }

    pub(crate) fn clock_spots(&mut self) -> &mut HashMap<String, (Spot, ClockLook, u64)> {
        &mut self.clock_spots
    }

    /// The frame being drawn, which a row building now is stamped with.
    pub(crate) fn frame(&self) -> u64 {
        self.frame
    }

    pub(crate) fn shimmers(&mut self) -> &mut HashMap<String, Entity<Shimmer>> {
        &mut self.shimmers
    }

    /// The live lines' shimmers the view holds now.
    pub fn shimmer_lines(&self) -> Vec<Entity<Shimmer>> {
        self.shimmers.values().cloned().collect()
    }

    pub(crate) fn bodies(&self) -> &HashMap<String, Entity<TextViewState>> {
        &self.bodies
    }

    /// The parsed document of the row keyed `key`, while the view holds one.
    pub fn body_of(&self, key: &str) -> Option<Entity<TextViewState>> {
        self.bodies.get(key).cloned()
    }

    /// The rows with every fold and group open, which find searches.
    pub(crate) fn unfolded(&self) -> Vec<Row> {
        self.transcript.unfolded()
    }

    pub(crate) fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub(crate) fn events_seen(&self) -> usize {
        self.transcript.events()
    }

    /// Moves whenever the rows change: a read, a live event, a page, a fold, the listing, the page shown.
    pub(crate) fn revision(&self) -> usize {
        self.revision
    }

    pub(crate) fn is_revealing(&self) -> bool {
        self.revealing.is_some()
    }

    /// The last row whose top shows above the docked composer, which a fresh find picks its first match at or above.
    pub(crate) fn last_seen(&self) -> Option<usize> {
        let top = self.list.logical_scroll_top().item_ix;
        let limit = self.list.viewport_bounds().bottom() - self.composer_inset;
        (top..self.transcript.rows().len()).take_while(|&ix| self.list.bounds_for_item(ix).is_some_and(|b| b.top() < limit)).last()
    }

    pub(crate) fn focus_transcript(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    /// How many times the view has drawn since it was made.
    pub fn renders(&self) -> usize {
        self.renders
    }
}

impl gpui_kit::Focusable for ThreadView {
    fn focus_handle(&self, _: &gpui_kit::App) -> gpui_kit::FocusHandle {
        self.focus.clone()
    }
}

/// The thread's agent's catalog flags in `harnesses.list`'s answer, as ChatView reads them; an agent the catalog does
/// not hold takes the defaults: no Fork from here, rewinds by files.
fn can_of<C: serde::Serialize>(harnesses: &[C], harness: &str) -> Can {
    let catalog =
        harnesses.iter().filter_map(|c| serde_json::to_value(c).ok()).find(|c| c.get("harness").and_then(|h| h.as_str()) == Some(harness));
    let flag = |name: &str| catalog.as_ref().and_then(|c| c.get(name)).and_then(|f| f.as_bool()).unwrap_or(false);
    Can {
        forks: flag("forks"),
        forks_by_count: flag("forksByCount"),
        rewinds_conversation: flag("rewindsConversation"),
        rewinds_by_count: flag("rewindsByCount"),
    }
}

/// The wall clock in ms since the epoch, which the listing's states are read at.
fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64)
}

/// How many of `events` the host's transcript recorded, which a page's `total` counts.
fn recorded(events: &[SessionEvent]) -> usize {
    events.iter().filter(|e| wsp_host_client::pos(e).is_some()).count()
}

/// `useReadStamp`: the open thread is read when it opens and again whenever a turn of it ends. A refusal is the host's
/// to log; the transcript shows either way.
async fn mark_read(host: &HostClient, thread: &str) {
    if let Err(failure) = host.call(SessionsReadRequest { thread_id: thread.to_owned() }).await {
        eprintln!("desktop-gpui: marking {thread} read: {}", failure.message);
    }
}

/// The transcript's list, built row by row through the thread view, and drawn again whenever the thread view is.
pub struct Rows {
    view: WeakEntity<ThreadView>,
    list: ListState,
    _redraw: Subscription,
}

impl Rows {
    fn new(view: &Entity<ThreadView>, weak: WeakEntity<ThreadView>, list: ListState, cx: &mut Context<Self>) -> Self {
        Rows { view: weak, list, _redraw: cx.observe(view, |_, _, cx| cx.notify()) }
    }
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let view = self.view.clone();
        list(self.list.clone(), move |ix, window, cx| {
            view.update(cx, |view, cx| view.render_row(ix, window, cx)).unwrap_or_else(|_| div().into_any_element())
        })
        .size_full()
    }
}

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        self.frame += 1;
        if self._activation.is_none() {
            self._activation = Some(cx.observe_window_activation(window, |this, window, cx| {
                this.active = window.is_window_active();
                this.stamp(cx);
            }));
        }
        if self.active != window.is_window_active() {
            self.active = window.is_window_active();
            self.stamp(cx);
        }
        if window.focused(cx).is_none() {
            // Find's keys reach the view only through focus, and a window nobody clicked yet holds none.
            self.focus.focus(window, cx);
        }
        self.trim();
        for image in std::mem::take(&mut self.released) {
            if let Some(decoded) = image.clone().get_render_image(window, cx) {
                cx.drop_image(decoded, Some(window));
            }
            image.remove_asset(cx);
        }
        if let Some(Reveal { ix, y, tries, from }) = self.revealing.take() {
            let top = self.list.viewport_bounds().origin.y;
            let off = self.list.bounds_for_item(ix).map(|b| b.origin.y - top - y);
            let at = self.list.scroll_px_offset_for_scrollbar().y;
            // The list clamps at its ends, so a row near one may never reach its height.
            let stuck = from.is_some_and(|from| (from - at).abs() < px(0.5))
                || off.is_some_and(|off| off > px(0.) && self.list.is_scrolled_to_end() == Some(true));
            match off {
                _ if tries >= REVEAL_TRIES => {}
                Some(off) if off.abs() < px(0.5) || stuck => {}
                Some(off) => {
                    self.list.scroll_by(off);
                    self.revealing = Some(Reveal { ix, y, tries: tries + 1, from: Some(at) });
                }
                None => self.revealing = Some(Reveal { ix, y, tries: tries + 1, from: None }),
            }
            window.request_animation_frame();
        }
        if let Some(Glide { from, start, took }) = &self.glide {
            let t = (cx.background_executor().now().saturating_duration_since(*start).as_secs_f32() / took.as_secs_f32()).min(1.);
            if t >= 1. {
                self.glide = None;
                self.list.set_follow_mode(FollowMode::Tail);
            } else {
                let end = self.list.max_offset_for_scrollbar().y;
                let target = *from + (end - *from) * toolbar::ease_in_out(t);
                self.list.scroll_by(target + self.list.scroll_px_offset_for_scrollbar().y);
                window.request_animation_frame();
            }
        }
        if let Some(drive) = &mut self.drive {
            if !drive.done {
                // The end check stays unanswered while a row is not laid out, and for a thread shorter than the list.
                if self.list.is_scrolled_to_end() == Some(true) || self.list.max_offset_for_scrollbar().y <= px(0.) {
                    drive.done = true;
                } else {
                    // The list counts a row it has not laid out as no height, so a scroll from such a row, or further
                    // than the rows laid out under it, would pass over every one of them in one frame.
                    if self.list.bounds_for_item(self.list.logical_scroll_top().item_ix).is_some() {
                        let at = -self.list.scroll_px_offset_for_scrollbar().y;
                        let target = px(drive.px_per_s * drive.start.elapsed().as_secs_f32());
                        self.list.scroll_by((target - at).min(self.list.viewport_bounds().size.height));
                    }
                    window.request_animation_frame();
                }
            }
        }
        let frames = self.drive.as_ref().filter(|d| !d.done).map(|d| d.frames.clone());
        let shown = self.loaded && !self.transcript.rows().is_empty();
        let height = self.list.viewport_bounds().size.height;
        if shown && !self.settled && height > px(0.) && self.list.is_scrolled_to_end() != Some(false) {
            self.settled = true;
        }
        // An older page once the list first stood at its end and the reader is within two screens of the top;
        // every one while find is open.
        let near_top = -self.list.scroll_px_offset_for_scrollbar().y < height * 2.;
        if self.loaded && (self.whole || self.find.is_open() || (self.settled && near_top)) {
            self.older(cx);
        }
        let presented = match self.presented.take() {
            Some((thread, cell)) if shown && self.thread.as_deref() == Some(thread.as_str()) => Some(cell),
            waiting => {
                self.presented = waiting;
                None
            }
        };
        let drawn = self.drawn.take_if(|_| self.loaded);
        let muted = cx.theme().muted_foreground;
        let status = match (&self.failure, &self.thread) {
            (Some(failure), _) => Some(failure.clone()),
            (None, None) => Some("Pick a thread".to_owned()),
            _ => None,
        };
        // `TRANSCRIPT_LOADING` over the composer's room, and list.tsx's empty line in the placeholder ink, each centred.
        let words = match self.placeholder() {
            Some(_) if !self.loaded => Some(cards::loading(self.composer_inset, cx).into_any_element()),
            Some(said) => Some(
                div()
                    .debug_selector(|| "transcript-words".into())
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.palette().placeholder)
                    .child(said)
                    .into_any_element(),
            ),
            None => None,
        };
        // useChatThread's local error row, a runtime error where the transcript would be.
        let refused = self.refused.clone().map(|said| {
            let row = Row { key: "local-error:0".to_owned(), kind: RowKind::Error, text: said, turn: None };
            div()
                .debug_selector(|| "history-refused".into())
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .child(rows::row(self, 0, &row, window, cx))
        });
        let lost = match &self.link {
            Link::Lost(_) => Some(rows::lost_line(cx)),
            Link::Live => None,
        };
        // On screen means at or past the list's top and not yet past its bottom; past the first row below, none are.
        let top = self.list.logical_scroll_top().item_ix;
        let mut below = false;
        let marks: Vec<(usize, bool)> = self
            .transcript
            .rows()
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.kind, RowKind::Person { .. }))
            .map(|(ix, _)| {
                below = below || (ix >= top && self.list.item_is_below_viewport(ix) != Some(false));
                (ix, ix >= top && !below && self.list.item_is_above_viewport(ix) == Some(false))
            })
            .collect();
        let rail = minimap(self, marks, self.list.viewport_bounds().size.width, window.viewport_size().height, window, cx);
        let rows = self.transcript.rows();
        self.shimmers
            .retain(|k, _| rows.iter().any(|r| &r.key == k && matches!(r.kind, RowKind::Live { active: true, .. } | RowKind::Thinking)));
        let built = self.span_frame;
        self.clock_spots.retain(|_, (_, _, claimed)| *claimed == built);
        let at = |key: &str| self.transcript.rows().iter().position(|r| r.key == key);
        let mut over: Vec<Follow> = Vec::new();
        for (key, band) in &self.shimmers {
            if let Some(ix) = at(key) {
                let (spot, band) = shimmer::band(band, cx);
                over.push(Follow::new(spot, self.list.clone(), ix, band));
            }
        }
        for (key, (spot, look, _)) in &self.clock_spots {
            if let (Some(ix), Some((_, clock))) = (at(key), self.clocks.get(key)) {
                // A retried model call's clock is in the mono face, as `WorkingTimelineRow` draws its retry line.
                let mono = matches!(self.transcript.rows()[ix].kind, RowKind::Working { retry: Some(_), .. });
                let text = div()
                    .debug_selector(|| "clock".into())
                    .flex()
                    .items_center()
                    .h(look.height)
                    .whitespace_nowrap()
                    .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                    .text_size(look.size)
                    .when_some(look.line_height, |el, lh| el.line_height(lh))
                    .text_color(cx.theme().muted_foreground)
                    .child(clock.clone());
                over.push(Follow::new(spot.clone(), self.list.clone(), ix, text.into_any_element()));
            }
        }
        find::paint(self, cx);
        let find_bar = find::bar(self, window, cx);
        let pill = toolbar::scroll_to_end(self, shown, cx);
        let quote = toolbar::toolbar(self, cx);
        let me = cx.entity().downgrade();
        v_flex()
            .size_full()
            .relative()
            .when(self.find.is_open(), |el| el.key_context(keys::THREAD_FIND))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &keys::FindInThread, window, cx| find::open(this, window, cx)))
            .on_action(cx.listener(|this, _: &keys::FindOlder, window, cx| find::step(this, -1, window, cx)))
            .on_action(cx.listener(|this, _: &keys::FindNewer, window, cx| find::step(this, 1, window, cx)))
            .on_scroll_wheel(cx.listener(|this, _, _, _| {
                this.revealing = None;
                this.glide = None;
            }))
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                if event.keystroke.key != "escape" {
                    return;
                }
                if toolbar::dismiss(this, cx) {
                    cx.stop_propagation();
                } else if this.find.is_open() {
                    find::close(this, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if !this.focus.contains_focused(window, cx) {
                        this.focus.focus(window, cx);
                    }
                }),
            )
            .bg(cx.theme().background)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.rows_view.clone().cached(StyleRefinement::default().size_full()))
                    .child(div().absolute().top_0().left_0().size_0().children(over))
                    .children(words)
                    .children(refused)
                    .children(rail)
                    .children(pill)
                    .children(find_bar),
            )
            .children(quote)
            .children(self.row_menu.overlay())
            .child(canvas(|_, _, _| {}, move |_, _, window, _| toolbar::listen(me.clone(), window)).absolute().size_0())
            .children(status.map(|s| div().absolute().top_4().left_4().text_sm().text_color(muted).child(s)))
            .children(lost)
            .when(frames.is_some() || presented.is_some() || drawn.is_some(), |this| {
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
                            if let Some(drawn) = drawn {
                                cx.defer(move |_| _ = drawn.send(()));
                            }
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::Root;
    use gpui_kit::{size, TestAppContext, VisualTestContext};

    /// `turns` settled turns from `from`, four events each, placed by pos as the host places them.
    fn turns(from: usize, to: usize) -> Vec<SessionEvent> {
        (from..to)
            .flat_map(|n| {
                let pos = 4 * n as u64 + 1;
                let event = |kind: &str, at: u64, extra: serde_json::Value| {
                    let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": format!("u{n}"), "type": kind, "pos": at});
                    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
                    serde_json::from_value::<SessionEvent>(v).unwrap()
                };
                [
                    event("session.start", pos, serde_json::json!({"prompt": format!("Question {n}")})),
                    event("session.delta", pos + 1, serde_json::json!({"kind": "text", "text": format!("Answer {n}.\n\nMore.\n\nAnd more.")})),
                    event("session.done", pos + 2, serde_json::json!({"result": {"status": "completed"}})),
                    event("session.end", pos + 3, serde_json::json!({"exitCode": 0, "sawResult": true})),
                ]
            })
            .collect()
    }

    fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let view = cx.new(ThreadView::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        cx.simulate_resize(size(px(900.), px(600.)));
        (view, cx)
    }

    fn frames(cx: &mut VisualTestContext, n: usize) {
        for _ in 0..n {
            cx.executor().advance_clock(Duration::from_millis(16));
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        }
    }

    fn page(events: Vec<SessionEvent>) -> Page {
        Page { events, pos: 80, total: 80, trimmed: None }
    }

    #[gpui_kit::test]
    fn a_page_landing_under_a_reveal_keeps_it_on_its_row(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(10, 20), cx);
            v.scroll_to_end(cx);
        });
        frames(cx, 3);
        let at = |v: &ThreadView| v.rows().iter().position(|r| r.text == "Question 15").unwrap();
        view.update(cx, |v, cx| {
            let ix = at(v);
            v.reveal(ix, px(100.), cx);
            v.land(41, Ok(page(turns(0, 10))), cx);
        });
        let (revealing, row) = view.read_with(cx, |v, _| (v.revealing.as_ref().map(|r| r.ix), at(v)));
        assert_eq!(revealing, Some(row), "the reveal still names the row it was asked for");
        frames(cx, 20);
        let top = view.read_with(cx, |v, _| v.list.bounds_for_item(at(v)).map(|b| b.top() - v.list.viewport_bounds().top()));
        assert!(top.is_some_and(|top| (top - px(100.)).abs() < px(1.)), "the revealed row stands at {top:?}, not 100 px");
    }

    #[gpui_kit::test]
    fn a_page_landing_over_a_reader_at_the_top_stands_its_rows_above_them(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(100, 110), cx);
            v.scroll_to_top(cx);
        });
        frames(cx, 3);
        view.update(cx, |v, cx| v.land(401, Ok(page(turns(50, 100))), cx));
        frames(cx, 3);
        let (above, screen) = view.read_with(cx, |v, _| (-v.list.scroll_px_offset_for_scrollbar().y, v.list.viewport_bounds().size.height));
        assert!(above > screen * 2., "the page's rows count {above:?} above the reader, so the next page is asked for at once");
    }

    #[gpui_kit::test]
    fn a_turn_starting_leaves_a_reader_scrolled_up_on_their_row(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            _ = v.transcript.can(Can { rewinds_conversation: true, rewinds_by_count: true, ..Can::default() });
            v.load(turns(0, 30), cx);
            v.scroll_to_end(cx);
        });
        frames(cx, 3);
        let at = |v: &ThreadView, text: &str| v.rows().iter().position(|r| r.text == text).unwrap();
        view.update(cx, |v, cx| {
            let ix = at(v, "Question 15");
            v.reveal(ix, px(100.), cx);
        });
        frames(cx, 20);
        let top = |v: &ThreadView| v.rows()[v.list.logical_scroll_top().item_ix].key.clone();
        let reply = view.read_with(cx, |v, _| v.rows()[at(v, "Answer 15.\n\nMore.\n\nAnd more.")].key.clone());
        let rewind: &'static str = Box::leak(format!("rewind-to-here-{reply}").into_boxed_str());
        let read = view.read_with(cx, |v, _| top(v));
        assert!(cx.debug_bounds(rewind).is_some(), "a settled thread offers Rewind to here on an earlier reply");
        let start = turns(30, 31).swap_remove(0);
        view.update(cx, |v, cx| v.push(start, cx));
        frames(cx, 5);
        let (now, following) = view.read_with(cx, |v, _| (top(v), v.list.is_following_tail()));
        assert_eq!((now, following), (read, false), "a turn starting moved the reader off the row they read");
        assert!(cx.debug_bounds(rewind).is_none(), "Rewind to here stands while the turn runs");
    }

    #[gpui_kit::test]
    fn a_driven_scroll_lays_out_every_row_a_page_landed_before_it_reaches_the_end(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(150, 200), cx);
            v.scroll_to_end(cx);
        });
        frames(cx, 3);
        view.update(cx, |v, cx| v.land(601, Ok(Page { events: turns(0, 150), pos: 800, total: 800, trimmed: None }), cx));
        frames(cx, 3);
        let long_ago = Instant::now() - Duration::from_secs(1000);
        view.update(cx, |v, cx| v.drive(2400., long_ago, Rc::new(RefCell::new(Vec::new())), cx));
        for _ in 0..1000 {
            if view.read_with(cx, |v, _| v.driven_to_end()) {
                break;
            }
            frames(cx, 1);
        }
        // The list answers the end check only once every row is laid out.
        let end = view.read_with(cx, |v, _| (v.driven_to_end(), v.list.is_scrolled_to_end()));
        assert_eq!(end, (true, Some(true)), "the scroll passed over rows of the page it never laid out");
    }

    #[gpui_kit::test]
    fn a_page_landing_folds_the_page_and_the_turn_it_meets_not_the_thread(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(25, 125), cx);
            v.scroll_to_end(cx);
        });
        frames(cx, 3);
        let folded = view.update(cx, |v, cx| {
            v.transcript.take_cost();
            v.land(101, Ok(Page { events: turns(0, 25), pos: 500, total: 500, trimmed: None }), cx);
            v.transcript.take_cost()
        });
        assert!(folded < 300, "a page of 100 events under 400 held cost {folded} to land");
        let whole = view.read_with(cx, |v, _| {
            let mut t = Transcript::new();
            t.load(turns(0, 125));
            (t.rows() == v.rows(), v.oldest, v.complete)
        });
        assert_eq!(whole, (true, Some(1), true), "the landed page reads as the whole thread");
    }

    #[gpui_kit::test]
    fn a_page_landing_keeps_a_reader_on_a_row_folded_again_with_rows_that_moved_below_it(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        // The failed start's line stands after the thread's first message, so a page above moves it from under the
        // reader's row, which the head that changed holds.
        let failed = crate::transcript::StartFailed {
            on: "Boat".into(),
            step: "Asking Boat for a machine".into(),
            said: "no machine free".into(),
            fix: None,
            prompt: None,
            log: Vec::new(),
        };
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.set_listing(crate::transcript::Listing { thread: "t".into(), start_failed: Some(failed), ..Default::default() }, cx);
            v.load(turns(10, 20), cx);
            v.scroll_to_top(cx);
        });
        frames(cx, 3);
        let top = |v: &ThreadView| v.rows()[v.list.logical_scroll_top().item_ix].text.clone();
        assert_eq!(view.read_with(cx, |v, _| top(v)), "Question 10");
        view.update(cx, |v, cx| v.land(41, Ok(page(turns(0, 10))), cx));
        frames(cx, 3);
        assert_eq!(view.read_with(cx, |v, _| top(v)), "Question 10", "the page moved the reader off the row they read");
    }

    #[gpui_kit::test]
    fn escape_out_of_find_on_a_subagents_page_keeps_the_keys_in_its_transcript(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let heard = Rc::new(RefCell::new(Vec::new()));
        let into = heard.clone();
        cx.update(|_, cx| cx.subscribe(&view, move |_, event: &ThreadViewEvent, _| into.borrow_mut().push(event.clone())).detach());
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(0, 3), cx);
            v.set_page(Some("call-1".to_owned()), cx);
        });
        frames(cx, 2);
        cx.update(|window, cx| view.update(cx, |v, cx| find::open(v, window, cx)));
        frames(cx, 2);
        cx.update(|window, cx| view.update(cx, |v, cx| find::close(v, window, cx)));
        frames(cx, 2);
        let asked = heard.borrow().contains(&ThreadViewEvent::FocusComposer);
        let held = cx.update(|window, cx| view.read(cx).holds_focus(window, cx));
        assert_eq!((asked, held), (false, true), "closing find on the page sent the keys to a composer it does not draw");
    }

    #[gpui_kit::test]
    fn a_failed_page_read_is_asked_again_after_a_wait(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(10, 20), cx);
            v.complete = false;
            v.land(41, Err(Failure::new("the host is busy".to_owned())), cx);
        });
        assert_eq!(view.read_with(cx, |v, _| (v.complete, v.paging.is_some())), (false, true), "the thread still has older pages");
        cx.executor().advance_clock(RETRY[0] + Duration::from_millis(10));
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.paging.is_none()), "after the wait the next page may be asked for");
    }

    #[gpui_kit::test]
    fn a_newest_page_past_a_gap_is_not_laid_under_the_pages_held(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(turns(0, 10), cx);
            let (drawn, _) = oneshot::channel();
            v.arrive(Arrival::History { events: turns(80, 90), total: 360, trimmed: false, drawn }, cx);
        });
        let (held, oldest) = view.read_with(cx, |v, _| (v.rows().iter().any(|r| r.text == "Question 5"), v.oldest));
        assert_eq!((held, oldest), (false, Some(321)), "the turns between the pages held and the newest page are missing");
    }
}
