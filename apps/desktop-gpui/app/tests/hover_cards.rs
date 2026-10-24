// SPDX-License-Identifier: AGPL-3.0-only
//! The app's hover cards side by side, as the frame mounts them: a sidebar tile's card, the message ruler's, the
//! context ring's, a usage segment's tip and a chart's. One stands at a time, and a card the pointer opened stands only
//! while the pointer is on it or on what opened it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::files::open_split::OpenSplit;
use desktop_gpui::screens::pr_pane::pull_request::git_split::GitSplit;
use desktop_gpui::screens::thread_view::header;
use desktop_gpui::screens::usage::{Tab, UsagePage};
use desktop_gpui::screens::Host;
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::sidebar::Sidebar;
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::ui::{self, Chart, ChartLine, Side};
use gpui_kit::component::{h_flex, v_flex, Root};
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Bounds, Context, Entity, Modifiers, MouseExitEvent, Pixels, Point, TestAppContext,
    VisualTestContext, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::HostClient;

/// Every card's selector.
const CARDS: [&str; 5] = ["tile-card", "minimap-preview", "context-card", "segment-tip", "chart-tip"];

fn event(turn: &str, kind: &str, pos: u64, extra: Value) -> SessionEvent {
    let mut v = json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": turn, "type": kind, "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// `n` settled turns, the person's question and one reply each.
fn turns(n: usize) -> Vec<SessionEvent> {
    (0..n)
        .flat_map(|n| {
            let (t, pos) = (format!("u{n}"), 10 * n as u64 + 1);
            vec![
                event(&t, "session.start", pos, json!({"prompt": format!("Question {n}")})),
                event(&t, "session.delta", pos + 1, json!({"kind": "text", "text": format!("Reply {n}")})),
                event(&t, "session.done", pos + 2, json!({"result": {"status": "completed", "tokens": {"input": 1, "output": 1}}})),
                event(&t, "session.end", pos + 3, json!({"exitCode": 0, "sawResult": true})),
            ]
        })
        .collect()
}

/// Ten finished threads on one workspace.
fn snapshot() -> Snapshot {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
    let row = |i: usize| {
        json!({
            "id": format!("r{i}"), "threadId": format!("t{i}"), "workspaceId": "w0", "harness": "claude",
            "status": "completed", "prompt": format!("thread {i}"), "project": { "id": "p1", "name": "repo" },
            "startedAt": now - 60_000. - i as f64, "endedAt": now - 30_000., "readAt": now - 30_000.,
        })
    };
    Snapshot {
        sessions: desktop_gpui::sidebar::shared::rows_of((0..10).map(row).collect()),
        workspaces: vec![json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" } })],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        ..Snapshot::default()
    }
}

/// The ring's cluster, a view of its own drawn again with the thread view, as `thread_view::Actions` is.
struct Actions {
    view: Entity<ThreadView>,
    git: Entity<GitSplit>,
    open: Entity<OpenSplit>,
    bar: header::BarWatch,
}

impl Render for Actions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.bar.follow(&self.view, cx);
        let (git, open) = (self.git.clone(), self.open.clone());
        self.view.update(cx, |v, cx| header::actions(v, &git, &open, window, cx).into_any_element())
    }
}

/// The sidebar beside the thread view under its header, then the usage page over a chart, each where it may be.
struct Frame {
    sidebar: Entity<Sidebar>,
    view: Entity<ThreadView>,
    actions: Option<Entity<Actions>>,
    usage: Option<Entity<UsagePage>>,
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let line =
            ChartLine { key: "claude".into(), label: "Claude Code".into(), points: vec![1., 3., 2., 5., 4., 6., 3.], ink: gpui_kit::red() };
        let chart = Chart::new("chart", (0..7).map(f64::from).collect(), vec![line], Vec::new())
            .figure(|v| format!("{v:.0} tokens"))
            .step_word(|_| "Sun 18 Oct".to_owned());
        h_flex()
            .size_full()
            .items_start()
            .child(div().flex_none().w(px(256.)).h_full().child(self.sidebar.clone()))
            .child(
                v_flex()
                    .flex_none()
                    .w(px(900.))
                    .h_full()
                    .children(self.actions.clone().map(|a| h_flex().h(px(52.)).pr(px(20.)).child(div().flex_1()).child(a)))
                    .child(div().flex_1().child(self.view.clone())),
            )
            .when_some(self.usage.clone(), |el, usage| {
                el.child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .gap(px(40.))
                        .child(div().h(px(500.)).overflow_hidden().child(usage))
                        .child(div().debug_selector(|| "chart-plot".into()).mx(px(60.)).child(chart)),
                )
            })
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

/// The window, the thread view holding `turns` person messages, the sidebar its threads.
fn mount<'a>(
    cx: &'a mut TestAppContext,
    host: Option<&Host>,
    width: f32,
) -> (Entity<Sidebar>, Entity<ThreadView>, Option<Entity<UsagePage>>, &'a mut VisualTestContext) {
    let sidebar = cx.new(Sidebar::new);
    let view = cx.new(ThreadView::new);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(20), cx);
        v.scroll_to_end(cx);
    });
    let actions = host.map(|host| {
        let (git, open) = (cx.new(|_| GitSplit::fed(host.clone())), cx.new(OpenSplit::new));
        cx.new(|cx: &mut Context<Actions>| {
            cx.observe(&view, |_, _, cx| cx.notify()).detach();
            Actions { view: view.clone(), git, open, bar: header::BarWatch::default() }
        })
    });
    let mut usage = None;
    let (held_sidebar, held_view) = (sidebar.clone(), view.clone());
    let (_, cx) = cx.add_window_view(|window, cx| {
        usage = host.map(|host| cx.new(|cx| UsagePage::new(host.clone(), window, cx)));
        let both = cx.new(|_| Frame { sidebar: held_sidebar, view: held_view, actions, usage: usage.clone() });
        Root::new(both, window, cx)
    });
    cx.simulate_resize(size(px(width), px(900.)));
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot())), cx));
    frames(cx, 4);
    (sidebar, view, usage, cx)
}

fn frames(cx: &mut VisualTestContext, n: usize) {
    for _ in 0..n {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} was not drawn"))
}

/// The cards drawn now.
fn drawn(cx: &mut VisualTestContext) -> Vec<&'static str> {
    CARDS.into_iter().filter(|card| cx.debug_bounds(card).is_some()).collect()
}

/// The pointer at `at` resting long enough for any card's delay.
fn rest(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    frames(cx, 2);
    cx.executor().advance_clock(Duration::from_millis(460));
    frames(cx, 3);
}

/// Catches: the ruler's card opened by the rail's focus or a hover kept standing while a tile's card opened beside
/// it after the pointer rested on the tile, two cards at once.
#[gpui_kit::test]
fn a_tiles_card_never_stands_beside_the_rulers(cx: &mut TestAppContext) {
    init(cx);
    let (_, view, _, cx) = mount(cx, None, 1280.);
    let rail_focus = view.read_with(cx, |v, _| v.rail_focus()).expect("the rail has no focus");
    cx.update(|window, cx| window.focus(&rail_focus, cx));
    frames(cx, 2);
    assert_eq!(drawn(cx), ["minimap-preview"], "the rail's focus opens its card");
    let tile = bounds(cx, "tile-t1").center();
    rest(cx, tile);
    assert_eq!(drawn(cx), ["tile-card"], "the ruler's card stood beside the tile's");

    let rail = bounds(cx, "minimap");
    rest(cx, point(rail.left() + px(4.), rail.top() + px(16.)));
    assert_eq!(drawn(cx), ["minimap-preview"], "a rail tick under the pointer opens the ruler's card alone");
    let tile = bounds(cx, "tile-t2").center();
    rest(cx, tile);
    assert_eq!(drawn(cx), ["tile-card"], "the ruler's card stood beside the tile's");
}

/// Catches: the ruler's card, its rail drawn away under one message, kept as standing though no frame draws it, so a
/// tile's card then opens at once rather than after the pointer rests its 450 ms.
#[gpui_kit::test]
fn a_tiles_card_waits_its_delay_after_a_card_whose_opener_was_drawn_away(cx: &mut TestAppContext) {
    init(cx);
    let (_, view, _, cx) = mount(cx, None, 1280.);
    let rail = bounds(cx, "minimap");
    rest(cx, point(rail.left() + px(4.), rail.top() + px(16.)));
    assert_eq!(drawn(cx), ["minimap-preview"]);
    view.update(cx, |v, cx| v.load(turns(1), cx));
    frames(cx, 3);
    assert_eq!(drawn(cx), Vec::<&str>::new(), "the rail is drawn away");
    cx.executor().advance_clock(Duration::from_millis(500));
    let tile = bounds(cx, "tile-t1").center();
    cx.simulate_mouse_move(tile, None, Modifiers::none());
    frames(cx, 2);
    cx.executor().advance_clock(Duration::from_millis(300));
    frames(cx, 1);
    assert_eq!(drawn(cx), Vec::<&str>::new(), "the tile's card opened before its 450 ms");
    cx.executor().advance_clock(Duration::from_millis(160));
    frames(cx, 2);
    assert_eq!(drawn(cx), ["tile-card"], "the tile's card did not open after its 450 ms");
}

/// One card at most, and a drawn card has the pointer on it or on what opened it.
fn holds(cx: &mut VisualTestContext, at: Point<Pixels>, opener: Option<Bounds<Pixels>>, step: &str) {
    let shown = drawn(cx);
    assert!(shown.len() <= 1, "{step}: {shown:?} stand at once");
    if let Some(card) = shown.first() {
        let on_card = cx.debug_bounds(card).is_some_and(|b| b.contains(&at));
        let on_opener = opener.is_some_and(|b| b.contains(&at));
        assert!(on_card || on_opener, "{step}: {card} stands with the pointer at {at:?}, off it and off what opened it");
    }
}

/// A host seeded with the Limits tab's accounts (`tests/usage/seed.mjs`).
fn seeded() -> common::Throwaway {
    let host = common::Throwaway::start();
    let folder = host.repo("usage");
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "usage"]);
    let state = host.state().parent().unwrap().to_string_lossy().into_owned();
    let seed = common::root().join("apps/desktop-gpui/app/tests/usage/seed.mjs");
    let out = Command::new("node").arg(&seed).arg(&state).env_clear().envs(host.env()).current_dir(host.dir.path()).output().unwrap();
    assert!(out.status.success(), "seed: {}", String::from_utf8_lossy(&out.stderr));
    host
}

/// Catches: a card left standing as the pointer goes straight from it to the next opener, out of the window, or as
/// its opener is drawn away, so two stand at once or one stands with the pointer nowhere near it.
#[gpui_kit::test]
fn the_pointer_walking_every_opener_leaves_one_card_under_it(cx: &mut TestAppContext) {
    let host = seeded();
    init(cx);
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let (_, view, usage, cx) = mount(cx, Some(&handle), 1900.);
    let usage = usage.unwrap();
    usage.update(cx, |p, cx| p.pick_tab(Tab::Limits, cx));
    let limit = Instant::now() + Duration::from_secs(30);
    while cx.debug_bounds("usage-segment").is_none() {
        assert!(Instant::now() < limit, "the Limits tab drew no segment");
        std::thread::sleep(Duration::from_millis(20));
        frames(cx, 1);
    }

    walk(cx, "a tile", "tile-t1", "tile-card");
    walk(cx, "a rail tick", "minimap", "minimap-preview");
    walk(cx, "the ring", "context-ring", "context-card");
    walk(cx, "a usage segment", "usage-segment", "segment-tip");
    walk(cx, "the chart", "chart-plot", "chart-tip");
    walk(cx, "a tile again", "tile-t2", "tile-card");
    for (step, opener, card) in
        [("a tile", "tile-t3", "tile-card"), ("the ring", "context-ring", "context-card"), ("the chart", "chart-plot", "chart-tip")]
    {
        walk(cx, step, opener, card);
        let away = point(px(700.), px(850.));
        cx.simulate_mouse_move(away, None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(200));
        frames(cx, 2);
        assert_eq!(drawn(cx), Vec::<&str>::new(), "{step}: its card stood after the pointer went from it to nothing");
    }

    cx.simulate_event(MouseExitEvent { position: point(px(-1.), px(-1.)), pressed_button: None, modifiers: Modifiers::none() });
    frames(cx, 2);
    assert_eq!(drawn(cx), Vec::<&str>::new(), "the pointer left the window and a card stood");

    // The rail drawn away under one message, the pointer moved off, then drawn again.
    let rail = bounds(cx, "minimap");
    let tick = point(rail.left() + px(4.), rail.top() + px(16.));
    rest(cx, tick);
    assert_eq!(drawn(cx), ["minimap-preview"]);
    view.update(cx, |v, cx| v.load(turns(1), cx));
    frames(cx, 3);
    holds(cx, tick, None, "the rail drawn away");
    let away = point(px(700.), px(850.));
    cx.simulate_mouse_move(away, None, Modifiers::none());
    frames(cx, 2);
    view.update(cx, |v, cx| v.load(turns(20), cx));
    frames(cx, 4);
    holds(cx, away, None, "the rail drawn again with the pointer elsewhere");

    // A switch of thread with the ruler's card open.
    let rail = bounds(cx, "minimap");
    rest(cx, point(rail.left() + px(4.), rail.top() + px(16.)));
    assert_eq!(drawn(cx), ["minimap-preview"]);
    view.update(cx, |v, cx| {
        v.show("t2", cx);
        v.load(turns(20), cx);
    });
    frames(cx, 4);
    let on = cx.debug_bounds("minimap");
    holds(cx, point(rail.left() + px(4.), rail.top() + px(16.)), on, "the thread switched");
}

/// The pointer from where it is straight onto `opener`, resting, then onto the card it opened.
fn walk(cx: &mut VisualTestContext, step: &str, opener: &'static str, card: &'static str) {
    let b = bounds(cx, opener);
    let at = if opener == "minimap" { point(b.left() + px(4.), b.top() + px(16.)) } else { b.center() };
    rest(cx, at);
    // The rail widens while its card shows.
    let on = if opener == "minimap" { bounds(cx, "minimap") } else { b };
    holds(cx, at, Some(on), step);
    assert_eq!(drawn(cx), [card], "{step}: its card did not open");
    let inside = bounds(cx, card).center();
    cx.simulate_mouse_move(inside, None, Modifiers::none());
    frames(cx, 2);
    holds(cx, inside, Some(on), &format!("{step}, on its card"));
    assert_eq!(drawn(cx), [card], "{step}: the card closed under the pointer");
}
