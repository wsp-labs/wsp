// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view as the frame mounts it, its pieces, and how a capture or the bench stands it at launch.

pub mod cards;
pub mod find;
pub mod header;
pub mod markdown;
pub mod math;
pub mod minimap;
pub mod rewind;
pub mod rows;
pub mod run;
pub mod shimmer;
pub mod subagent;
pub mod toolbar;

use std::cell::Cell;
use std::rc::Rc;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::{
    canvas, div, prelude::*, px, App, Context, Entity, Global, KeyDownEvent, Pixels, Point, StyleRefinement, Subscription, Task,
    WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::Failure;

use super::composer::link;
use super::composer::link::{Link, Seen, Target};
use super::composer::model::Hold;
use super::composer::screen::types_into_composer;
use super::composer::{dock, fork, Composer, ComposerEvent};
use super::pr_pane::files::open_split::OpenSplit;
use super::pr_pane::pull_request::git_split::GitSplit;
use super::{Entry, Host, Opened, Pane, Parts, Place, Route, Screen, THREAD};
use crate::screens::app_shell::keys;
use crate::sidebar::ThreadItem;
use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::RowKind;
use crate::ui::context_menu::{ContextMenu, MenuItem};

/// How a capture or the bench stands the thread view at launch, set once before the window opens (`bench::Args`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capture {
    /// The wsp theme the window draws in, whatever the host's preference.
    pub theme: Option<String>,
    pub unfold: bool,
    pub now: Option<f64>,
    pub reveal: Option<(String, f32)>,
    /// A failure to follow the thread ends the process, as the bench needs.
    pub strict: bool,
}

impl Global for Capture {}

/// The thread view the frame keeps for the thread screen, which a subagent's page of the thread it follows draws
/// rather than read and follow that thread a second time.
struct Kept(WeakEntity<ThreadView>);

impl Global for Kept {}

/// The kept thread view, while it follows `thread`.
pub(crate) fn kept_view(thread: &str, cx: &App) -> Option<Entity<ThreadView>> {
    let view = cx.try_global::<Kept>()?.0.upgrade()?;
    (view.read(cx).followed() == Some(thread)).then_some(view)
}

/// The thread view in the frame's main column, kept across thread switches, with the composer docked under its
/// transcript: one composer for the kept screen, told each thread's key as the screen moves to it.
pub struct ThreadScreen {
    view: Entity<ThreadView>,
    composer: Entity<Composer>,
    /// The header's git button and Open, the panes' own, one each for the kept screen, told each thread in turn.
    git: Entity<GitSplit>,
    open: Entity<OpenSplit>,
    /// The composer's tie to the open thread, built again for each thread once the host has read it.
    link: Option<Entity<Link>>,
    host: Host,
    capture: Capture,
    thread: Option<String>,
    /// The composer's height as last drawn, which the transcript ends with so its last row clears it.
    docked: Rc<Cell<Pixels>>,
    /// The composer changed since the last frame, so it is drawn whole; else it is drawn cached at its own height.
    composer_moved: Rc<Cell<bool>>,
    _moved: Subscription,
    _linking: Option<Task<()>>,
    _steps: Option<Subscription>,
    _fed: Subscription,
    /// A pick on the docked prompt goes to the host whether or not the thread's link stands yet.
    _answers: Subscription,
    _heard: Task<()>,
}

impl ThreadScreen {
    pub fn view(&self) -> &Entity<ThreadView> {
        &self.view
    }

    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    pub fn git_split(&self) -> &Entity<GitSplit> {
        &self.git
    }

    pub fn open_split(&self) -> &Entity<OpenSplit> {
        &self.open
    }

    fn open(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace_id), Some(thread_id)) = (opened.workspace_id, opened.thread_id) else {
            // No thread shows: stop following the last one rather than keep reading a thread nobody looks at.
            if self.thread.take().is_some() {
                self.link = None;
                self._linking = None;
                self.view.update(cx, |v, cx| v.stop(cx));
                self.composer.update(cx, |c, cx| c.set_prompt(None, window, cx));
            }
            return;
        };
        if self.thread.as_deref() == Some(thread_id.as_str()) {
            return;
        }
        self.thread = Some(thread_id.clone());
        let item =
            ThreadItem { workspace_id: workspace_id.clone(), thread_id: thread_id.clone(), project: String::new(), title: String::new() };
        let (host, strict) = (self.host.clone(), self.capture.strict);
        self.view.update(cx, |v, cx| v.follow(&host, &item, strict, cx));
        // A send typed before the new thread's link stands waits on it rather than go to no one.
        self.composer.update(cx, |c, cx| {
            c.set_prompt(None, window, cx);
            c.set_hold(Some(Hold::Connecting), cx);
            c.set_thread_key(thread_id.clone(), window, cx);
        });
        self.link = None;
        self.link_to(Target { workspace_id, thread_id: Some(thread_id) }, 0, window, cx);
    }

    /// Ties the composer to `target` once the host has said what it needs, trying again after a refusal with the
    /// thread view's waits, while the screen still shows that thread.
    fn link_to(&mut self, target: Target, tries: usize, window: &mut Window, cx: &mut Context<Self>) {
        let reading = self.host.clone();
        let known = self.composer.read(cx).known(&target.workspace_id);
        self._linking = Some(cx.spawn_in(window, async move |this, cx| {
            let opened = Link::read_facts(&reading, &target, known).await;
            _ = this.update_in(cx, |this, window, cx| {
                if this.thread != target.thread_id {
                    return;
                }
                match opened {
                    Ok(opened) => {
                        let (host, composer) = (this.host.clone(), this.composer.clone());
                        let link = cx.new(|cx| Link::fed(host, target, opened, composer, window, cx));
                        if this.view.read(cx).loaded() {
                            let seen = Seen::of(this.view.read(cx).transcript().event_list());
                            link.update(cx, |l, cx| l.take_history(seen, window, cx));
                        }
                        this.link = Some(link);
                    }
                    Err(failure) => {
                        eprintln!("desktop-gpui: {}", failure.message);
                        this.composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
                        let wait = crate::thread_view::RETRY[tries.min(crate::thread_view::RETRY.len() - 1)];
                        let retry = cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor().timer(wait).await;
                            _ = this.update_in(cx, |this, window, cx| this.link_to(target, tries + 1, window, cx));
                        });
                        this._linking = Some(retry);
                    }
                }
            });
        }));
    }

    /// Hands the composer a quote, and its link what the thread view just took, so the thread is read and followed once.
    fn arrived(&mut self, event: &ThreadViewEvent, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(event, ThreadViewEvent::Loaded) {
            self.view.update(cx, header::refresh);
        }
        match event {
            ThreadViewEvent::Lost => {
                self.composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
                return;
            }
            ThreadViewEvent::Quote(quote) => {
                self.composer.update(cx, |c, cx| c.insert_block(quote, window, cx));
                return;
            }
            ThreadViewEvent::SentElsewhere { thread, prompt } => {
                self.composer.update(cx, |c, cx| c.sent_elsewhere(thread, prompt, window, cx));
                return;
            }
            ThreadViewEvent::FocusComposer => {
                gpui_kit::Focusable::focus_handle(self.composer.read(cx), cx).focus(window, cx);
                return;
            }
            // A press on a frame drawn for the thread before a switch forks nothing.
            ThreadViewEvent::Fork { thread, turn, message } => {
                let view = self.view.read(cx);
                if let (true, Some(workspace)) = (self.thread.as_deref() == Some(thread.as_str()), view.followed_workspace()) {
                    let title = self.link.as_ref().and_then(|l| l.read(cx).title()).unwrap_or_else(|| "this thread".to_owned());
                    let kept = turn.as_deref().and_then(|t| fork::kept(view.transcript().event_list(), t));
                    let cwd = view.transcript().cwd().map(str::to_owned);
                    let picks = self.composer.read(cx).ran().clone();
                    let (workspace, thread, turn, message) = (workspace.to_owned(), thread.clone(), turn.clone(), message.clone());
                    fork::ask(&self.host, fork::Asked { workspace, thread, title, turn, picks, message }, kept, cwd, window, cx);
                }
                return;
            }
            ThreadViewEvent::Loaded | ThreadViewEvent::Arrived => {
                let view = self.view.read(cx);
                let transcript = view.transcript();
                // Only a prompt, its close, or a turn's start or end moves the dock; a streamed line does not.
                use wsp_host_client::protocol::SessionEvent as E;
                let moves = matches!(event, ThreadViewEvent::Loaded)
                    || matches!(
                        transcript.event_list().last(),
                        Some(
                            E::SessionPermission { .. } | E::SessionPermissionClosed { .. } | E::SessionStart { .. } | E::SessionEnd { .. }
                        )
                    );
                if moves {
                    let prompt = transcript.docked().and_then(|d| dock::Prompt::of(&d, transcript.harness()));
                    self.composer.update(cx, |c, cx| c.set_prompt(prompt, window, cx));
                }
            }
        }
        let Some(link) = self.link.clone() else { return };
        match event {
            ThreadViewEvent::Loaded => {
                let seen = Seen::of(self.view.read(cx).transcript().event_list());
                link.update(cx, |l, cx| l.take_history(seen, window, cx));
            }
            ThreadViewEvent::Arrived => {
                if let Some(event) = self.view.read(cx).transcript().event_list().last().cloned() {
                    link.update(cx, |l, cx| l.take_event(event, window, cx));
                }
            }
            _ => {}
        }
    }

    /// Hands the header what the host pushes about the open thread: every workspace's status, read once a socket is up
    /// and followed after, the threads' rows and the person's preferences. A closed socket is dialled again a second
    /// later. The thread's own follow hears only frames that name its thread, which these do not.
    fn hear(host: &Host, cx: &mut Context<Self>) -> Task<()> {
        let (tx, mut rx) = mpsc::unbounded::<(String, Value)>();
        let job = host.run(move |client| async move {
            loop {
                let ended: Result<(), Failure> = async {
                    let mut events = client.events().await?;
                    for status in client.statuses().await? {
                        let frame = json!({ "type": "workspace.status", "status": status });
                        tx.unbounded_send(("workspace.status".to_owned(), frame)).map_err(|_| Failure::new("gone"))?;
                    }
                    while let Some(event) = events.next().await {
                        let Some(heard) = event.ok().as_ref().and_then(header::frame_of) else { continue };
                        tx.unbounded_send(heard).map_err(|_| Failure::new("gone"))?;
                    }
                    Err(Failure::new("the host closed the socket"))
                }
                .await;
                if ended.is_ok() || tx.is_closed() {
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
        cx.spawn(async move |this, cx| {
            let _job = job;
            while let Some((kind, frame)) = rx.next().await {
                let heard = this.update(cx, |this, cx| this.view.update(cx, |v, cx| header::heard(v, &kind, &frame, cx)));
                if heard.is_err() {
                    break;
                }
            }
        })
    }

    /// A capture's clicks and scroll, once the transcript has rows to take them.
    fn take_steps(&mut self, cx: &mut Context<Self>) {
        if self.view.read(cx).rows().is_empty() {
            return;
        }
        if std::mem::take(&mut self.capture.unfold) {
            self.view.update(cx, unfold);
        }
        if let Some((text, y)) = self.capture.reveal.clone() {
            let done = self.view.update(cx, |v, cx| {
                let row = v.rows().iter().position(|r| matches!(r.kind, RowKind::Person { .. }) && r.text.contains(&text));
                match row {
                    Some(row) => v.reveal(row, px(y), cx),
                    // The row may sit on a page not read yet: read them all, and look again as each lands.
                    None if !v.complete() => {
                        v.read_whole(cx);
                        return false;
                    }
                    None => {}
                }
                true
            });
            if !done {
                return;
            }
            self.capture.reveal = None;
        }
        self._steps = None;
    }
}

impl Screen for ThreadScreen {
    const NAME: &'static str = THREAD;
    const PLACE: Place = Place::Main;

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let capture = cx.try_global::<Capture>().cloned().unwrap_or_default();
        let view = cx.new(ThreadView::new);
        cx.set_global(Kept(view.downgrade()));
        let composer = cx.new(|cx| {
            let mut composer = Composer::new(window, cx);
            composer.set_hold(Some(Hold::Connecting), cx);
            composer
        });
        if let Some(now) = capture.now {
            view.update(cx, |v, cx| v.freeze_clock(now, cx));
            cx.set_global(crate::ui::FrozenNow(now));
        }
        let steps = (capture.unfold || capture.reveal.is_some()).then(|| cx.observe(&view, |this, _, cx| this.take_steps(cx)));
        let docked = Rc::new(Cell::new(super::thread_view::rows::COMPOSER_INSET));
        let composer_moved = Rc::new(Cell::new(true));
        let moved = composer_moved.clone();
        let moved = cx.observe(&composer, move |_, _, _| moved.set(true));
        let fed = cx.subscribe_in(&view, window, |this, _, event, window, cx| this.arrived(event, window, cx));
        let answers = cx.subscribe(&composer, |this, composer, event: &ComposerEvent, cx| {
            match event {
                ComposerEvent::Answer(answer) => {
                    let key = composer.read(cx).thread_key().unwrap_or_default().to_owned();
                    link::answer(&this.host, answer.clone(), composer.downgrade(), key, cx);
                }
                // The line under the send stands at the transcript's foot, as the listing's notice; with none the send's
                // turn started or was refused, and the words drawn for it go too.
                ComposerEvent::Starting(line) => {
                    let thread = this.thread.clone().unwrap_or_default();
                    this.view.update(cx, |v, cx| {
                        let listed = v.transcript().listed().cloned();
                        let listing = listed.unwrap_or_else(|| crate::transcript::Listing { thread, ..Default::default() });
                        let sent = listing.sent.clone().filter(|_| line.is_some());
                        v.set_listing(crate::transcript::Listing { starting: line.clone(), sent, ..listing }, cx);
                    });
                }
                // A send stands as the person's message at once, before the host answers.
                ComposerEvent::Start(out) if out.fork.is_none() => {
                    let thread = this.thread.clone().unwrap_or_default();
                    this.view.update(cx, |v, cx| {
                        let listed = v.transcript().listed().cloned();
                        let listing = listed.unwrap_or_else(|| crate::transcript::Listing { thread, ..Default::default() });
                        v.set_listing(crate::transcript::Listing { sent: Some(out.prompt.clone()), ..listing }, cx);
                    });
                }
                _ => {}
            }
        });
        let heard = Self::hear(&host, cx);
        let git = cx.new(|_| GitSplit::fed(host.clone()));
        let open = cx.new(OpenSplit::new);
        let mut screen = ThreadScreen {
            view,
            composer,
            git,
            open,
            link: None,
            host,
            capture,
            thread: None,
            docked,
            composer_moved,
            _moved: moved,
            _linking: None,
            _steps: steps,
            _fed: fed,
            _answers: answers,
            _heard: heard,
        };
        screen.open(opened, window, cx);
        screen
    }

    fn show(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.open(opened, window, cx);
    }

    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        let screen = this.read(cx);
        let (view, git, open) = (screen.view.clone(), screen.git.clone(), screen.open.clone());
        let actions = cx.new(|cx| Actions::new(view.clone(), git, open, cx));
        let crumbs = cx.new(|cx| Crumbs::new(view, cx));
        Parts { actions: Some(actions.into()), crumbs: Some(crumbs.into()), ..Parts::default() }
    }
}

impl Render for ThreadScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let inset = self.docked.get();
        if self.view.read(cx).composer_inset() != inset {
            self.view.update(cx, |v, cx| v.set_composer_inset(inset, cx));
        }
        let fresh = self.composer.read(cx).fresh() && self.view.read(cx).rows().is_empty();
        let (docked, me) = (self.docked.clone(), cx.entity().downgrade());
        // The dock floats over the transcript's foot, as ChatView's does; its height is read back for the list's end.
        let measure = canvas(
            move |bounds, _, cx| {
                if docked.get() != bounds.size.height {
                    docked.set(bounds.size.height);
                    let me = me.clone();
                    cx.defer(move |cx| _ = me.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        // A sub-thread's machine that holds or blocks it says so in one strip over the composer, inside the dock.
        let strip = self.view.read(cx).transcript().listed().and_then(|l| l.strip.as_ref()).map(|s| cards::strip(&self.view, s, cx));
        let height = self.composer.read(cx).drawn_height().filter(|_| !self.composer_moved.replace(false));
        self.composer.read(cx).lay_at(height);
        let composer = match height {
            Some(height) => self.composer.clone().cached(StyleRefinement::default().w_full().h(height)).into_any_element(),
            None => self.composer.clone().into_any_element(),
        };
        let dock = div().debug_selector(|| "thread-dock".into()).relative().w_full().children(strip).child(composer).child(measure);
        div()
            .relative()
            .size_full()
            .when(self.view.read(cx).find_state().is_open(), |el| el.key_context(keys::THREAD_FIND))
            .on_action(cx.listener(|this, _: &keys::FindInThread, window, cx| this.view.update(cx, |v, cx| find::open(v, window, cx))))
            .on_action(cx.listener(|this, _: &keys::FindOlder, window, cx| this.view.update(cx, |v, cx| find::step(v, -1, window, cx))))
            .on_action(cx.listener(|this, _: &keys::FindNewer, window, cx| this.view.update(cx, |v, cx| find::step(v, 1, window, cx))))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.view.read(cx).holds_focus(window, cx) {
                    return;
                }
                if let Some(key) = types_into_composer(event) {
                    let key = key.to_owned();
                    this.composer.update(cx, |c, cx| c.type_key(&key, window, cx));
                    cx.stop_propagation();
                }
            }))
            .child(self.view.clone())
            .child(if fresh {
                super::composer::screen::column(true, dock).absolute().inset_0()
            } else {
                div().absolute().bottom_0().left_0().right_0().child(dock)
            })
    }
}

/// The header's thread cluster in the frame's actions slot, drawn again whenever the thread view or its cluster is.
struct Actions {
    view: Entity<ThreadView>,
    git: Entity<GitSplit>,
    open: Entity<OpenSplit>,
    bar: header::BarWatch,
    _redraw: Subscription,
}

impl Actions {
    fn new(view: Entity<ThreadView>, git: Entity<GitSplit>, open: Entity<OpenSplit>, cx: &mut Context<Self>) -> Self {
        let redraw = cx.observe(&view, |_, _, cx| cx.notify());
        Actions { view, git, open, bar: header::BarWatch::default(), _redraw: redraw }
    }
}

impl Render for Actions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.bar.follow(&self.view, cx);
        let (git, open) = (self.git.clone(), self.open.clone());
        self.view.update(cx, |v, cx| header::actions(v, &git, &open, window, cx).into_any_element())
    }
}

/// The thread's crumbs in the frame's crumbs slot, drawn again whenever the thread view or its cluster is, with the
/// title's menu.
struct Crumbs {
    view: Entity<ThreadView>,
    menu: ContextMenu,
    bar: header::BarWatch,
    _redraw: Subscription,
}

impl Crumbs {
    fn new(view: Entity<ThreadView>, cx: &mut Context<Self>) -> Self {
        let redraw = cx.observe(&view, |_, _, cx| cx.notify());
        Crumbs { view, menu: ContextMenu::default(), bar: header::BarWatch::default(), _redraw: redraw }
    }

    /// The title's menu, `Rename thread` alone, as the web's title menu holds it.
    fn open_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let rename = MenuItem { label: header::RENAME.into(), group: "edit", refusal: None, keys: None, destructive: false };
        self.menu.open(at, vec![rename], window, cx, |this, pick, window, cx| {
            if pick == Some(0) {
                this.view.update(cx, |v, cx| header::open_rename(v, window, cx));
            }
        });
    }
}

impl Render for Crumbs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me: WeakEntity<Self> = cx.entity().downgrade();
        let on_menu: header::OnMenu = Rc::new(move |at, window, cx| _ = me.update(cx, |this, cx| this.open_menu(at, window, cx)));
        self.bar.follow(&self.view, cx);
        let crumbs = self.view.update(cx, |v, cx| header::crumbs(v, on_menu, window, cx));
        div().flex().min_w_0().child(crumbs).children(self.menu.overlay())
    }
}

/// Opens `pane` in the right panel through the frame, on the tab `tab` names or the pane's own, and says whether it
/// did: the frame opens a pane only once a screen is registered to draw it, and takes any other name as a page.
pub(crate) fn open_pane(pane: Pane, tab: Option<&str>, cx: &mut App) -> bool {
    if !matches!(super::screen(pane.name()), Some(Entry { place: Place::Panel(_), .. })) {
        return false;
    }
    let route = match tab {
        Some(tab) => format!("{}:{tab}", pane.name()),
        None => pane.name().to_owned(),
    };
    super::navigate(Route::Screen(route.into()), cx);
    true
}

/// What the captures click in the web app: every turn's fold, the first tool group, and in an open group every failed
/// call and every edit; then, as they do, the transcript from its top.
pub fn unfold(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let folds: Vec<String> =
        view.rows().iter().filter(|r| matches!(r.kind, RowKind::Fold { open: false })).filter_map(|r| r.turn.clone()).collect();
    for turn in folds {
        view.toggle(&turn, cx);
    }
    let group = view.rows().iter().find_map(|r| match &r.kind {
        RowKind::Group { group, open: false, .. } => Some(group.clone()),
        _ => None,
    });
    if let Some(group) = group {
        view.toggle(&group, cx);
    }
    let failed: Vec<String> = view
        .rows()
        .iter()
        .flat_map(|r| match &r.kind {
            RowKind::Calls { calls, .. } => {
                calls.iter().filter(|c| (c.failed || c.patch.is_some()) && !c.open).map(|c| c.key.clone()).collect()
            }
            _ => Vec::new(),
        })
        .collect();
    for call in failed {
        view.toggle(&call, cx);
    }
    view.scroll_to_top(cx);
}
