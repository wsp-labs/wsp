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
pub mod shimmer;
pub mod toolbar;

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{canvas, div, prelude::*, px, App, Context, Entity, Global, KeyDownEvent, Pixels, Subscription, Task, Window};

use super::composer::link::{Link, Seen, Target};
use super::composer::model::Hold;
use super::composer::screen::types_into_composer;
use super::composer::Composer;
use super::{Host, Opened, Parts, Place, Screen, THREAD};
use crate::screens::app_shell::keys;
use crate::sidebar::ThreadItem;
use crate::thread_view::{ThreadView, ThreadViewEvent};
use crate::transcript::RowKind;

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

/// The thread view in the frame's main column, kept across thread switches, with the composer docked under its
/// transcript: one composer for the kept screen, told each thread's key as the screen moves to it.
pub struct ThreadScreen {
    view: Entity<ThreadView>,
    composer: Entity<Composer>,
    /// The composer's tie to the open thread, built again for each thread once the host has read it.
    link: Option<Entity<Link>>,
    host: Host,
    capture: Capture,
    thread: Option<String>,
    /// The composer's height as last drawn, which the transcript ends with so its last row clears it.
    docked: Rc<Cell<Pixels>>,
    _linking: Option<Task<()>>,
    _steps: Option<Subscription>,
    _fed: Subscription,
}

impl ThreadScreen {
    pub fn view(&self) -> &Entity<ThreadView> {
        &self.view
    }

    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    fn open(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace_id), Some(thread_id)) = (opened.workspace_id, opened.thread_id) else {
            // No thread shows: stop following the last one rather than keep reading a thread nobody looks at.
            if self.thread.take().is_some() {
                self.link = None;
                self._linking = None;
                self.view.update(cx, |v, cx| v.stop(cx));
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
        self._linking = Some(cx.spawn_in(window, async move |this, cx| {
            let opened = Link::read_facts(&reading, &target).await;
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

    /// Hands the composer's link what the thread view just took, so the thread is read and followed once.
    fn arrived(&mut self, event: &ThreadViewEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    /// A capture's clicks and scroll, once the transcript has rows to take them.
    fn take_steps(&mut self, cx: &mut Context<Self>) {
        if self.view.read(cx).rows().is_empty() {
            return;
        }
        if std::mem::take(&mut self.capture.unfold) {
            self.view.update(cx, unfold);
        }
        if let Some((text, y)) = self.capture.reveal.take() {
            self.view.update(cx, |v, cx| {
                let row = v.rows().iter().position(|r| matches!(r.kind, RowKind::Person { .. }) && r.text.contains(&text));
                if let Some(row) = row {
                    v.reveal(row, px(y), cx);
                }
            });
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
        let composer = cx.new(|cx| {
            let mut composer = Composer::new(window, cx);
            composer.set_hold(Some(Hold::Connecting), cx);
            composer
        });
        if let Some(now) = capture.now {
            view.update(cx, |v, cx| v.freeze_clock(now, cx));
        }
        let steps = (capture.unfold || capture.reveal.is_some()).then(|| cx.observe(&view, |this, _, cx| this.take_steps(cx)));
        let docked = Rc::new(Cell::new(super::thread_view::rows::COMPOSER_INSET));
        let fed = cx.subscribe_in(&view, window, |this, _, event, window, cx| this.arrived(event, window, cx));
        let mut screen =
            ThreadScreen { view, composer, link: None, host, capture, thread: None, docked, _linking: None, _steps: steps, _fed: fed };
        screen.open(opened, window, cx);
        screen
    }

    fn show(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.open(opened, window, cx);
    }

    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        let view = this.read(cx).view.clone();
        let actions = cx.new(|cx| Actions::new(view, cx));
        Parts { actions: Some(actions.into()), ..Parts::default() }
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
        let dock = div().debug_selector(|| "thread-dock".into()).relative().w_full().child(self.composer.clone()).child(measure);
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
                super::composer::screen::column(true, gpui_kit::transparent_black(), dock).absolute().inset_0()
            } else {
                div().absolute().bottom_0().left_0().right_0().child(dock)
            })
    }
}

/// The header's thread cluster in the frame's actions slot, drawn again whenever the thread view is.
struct Actions {
    view: Entity<ThreadView>,
    _redraw: Subscription,
}

impl Actions {
    fn new(view: Entity<ThreadView>, cx: &mut Context<Self>) -> Self {
        let redraw = cx.observe(&view, |_, _, cx| cx.notify());
        Actions { view, _redraw: redraw }
    }
}

impl Render for Actions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.view.update(cx, |v, cx| header::actions(v, window, cx).into_any_element())
    }
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
