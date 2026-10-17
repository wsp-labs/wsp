// SPDX-License-Identifier: AGPL-3.0-only
use std::time::Duration;

use desktop_gpui::screens::thread_view::header::{self, git_menu, git_quick, Checkout, Editor, GitAct, GitState, MenuAct};
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::{Transcript, TurnContext};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Bounds, Context, Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext,
    Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionEvent;

const WIDTH: f32 = 1024.;

fn event(turn: &str, kind: &str, pos: u64, extra: Value) -> SessionEvent {
    let mut e = json!({"workspaceId": "w", "sessionId": format!("s-{turn}"), "threadId": "t", "turnId": turn, "type": kind, "pos": pos});
    e.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(e).unwrap()
}

/// A finished turn whose reply counted `tokens`, then a turn still running whose last call held `held`.
fn counted(tokens: Value, held: Option<f64>) -> Vec<SessionEvent> {
    let mut events = vec![
        event("u1", "session.start", 1, json!({"prompt": "one"})),
        event("u1", "session.delta", 2, json!({"kind": "text", "text": "Done.", "messageId": "m1"})),
        event("u1", "session.done", 3, json!({"result": {"status": "completed", "tokens": tokens}})),
        event("u1", "session.end", 4, json!({"exitCode": 0, "sawResult": true})),
    ];
    if let Some(held) = held {
        events.push(event("u2", "session.start", 5, json!({"prompt": "two"})));
        events.push(event("u2", "session.context", 6, json!({"context": held})));
    }
    events
}

fn turn(tokens: Option<(f64, Option<f64>)>, running: bool) -> TurnContext {
    TurnContext {
        running,
        counted: tokens.is_some(),
        tokens_context: tokens.map(|t| t.0),
        tokens_window: tokens.and_then(|t| t.1),
        ..TurnContext::default()
    }
}

#[test]
fn the_ring_reads_the_newest_turn_that_said_what_the_model_held() {
    let mut t = Transcript::new();
    t.load(counted(json!({"input": 1, "output": 1, "context": 20_000, "window": 1_000_000}), Some(60_000.)));
    let turns = t.turn_contexts();
    assert_eq!(turns.len(), 2);
    assert_eq!((turns[0].tokens_context, turns[0].tokens_window, turns[0].running), (Some(20_000.), Some(1_000_000.), false));
    assert_eq!((turns[1].held_context, turns[1].running), (Some(60_000.), true));
    let s = header::snapshot(&turns).expect("a reading");
    assert_eq!(s, header::Snapshot { used: 60_000., max: Some(1_000_000.), share: Some(0.06) });
    assert_eq!(header::missing(&turns, "Claude Code"), None);
    assert_eq!(header::figure(&s), "60k / 1M");
    assert_eq!(header::title(Some(&s), None), "Context: 6% used, 60k of 1M tokens");

    let at = |context: f64| header::percent(&header::snapshot(&[turn(Some((context, Some(200_000.))), false)]).unwrap()).unwrap();
    assert_eq!([at(4_269.), at(10_000.), at(68_250.), at(250_000.)], ["2.1%", "5%", "34%", "100%"]);
    let codex = [turn(Some((24_763., None)), false)];
    let s = header::snapshot(&codex).unwrap();
    assert_eq!((s.share, header::figure(&s)), (None, "24.8k".to_owned()));
    assert_eq!(
        header::title(Some(&s), header::missing(&codex, "Codex").as_deref()),
        "Context: 24.8k tokens. Codex does not report its limit"
    );
}

#[test]
fn the_agent_is_named_by_the_catalog_until_the_host_lists_its_label() {
    assert_eq!(header::agent_label(&[], "codex"), "Codex");
    assert_eq!(header::agent_label(&[], "nobody"), "nobody");
    let listed = header::Catalog { harness: "codex".to_owned(), label: "Codex CLI".to_owned(), compacts: None };
    assert_eq!(header::agent_label(&[listed], "codex"), "Codex CLI");
}

#[test]
fn the_ring_says_why_a_thread_that_ran_a_turn_has_no_reading() {
    assert_eq!(header::reading(&[], "Claude Code", true), None);
    assert_eq!(header::missing(&[turn(None, true)], "Claude Code").as_deref(), Some(header::NOT_YET));
    let failed = TurnContext { failed: true, ..TurnContext::default() };
    assert_eq!(header::missing(&[failed], "Claude Code").as_deref(), Some(header::FAILED_EARLY));
    let counted_bare = TurnContext { counted: true, ..TurnContext::default() };
    assert_eq!(header::missing(&[counted_bare], "Cursor").as_deref(), Some("Cursor does not report how much context it holds"));
    let first = [TurnContext { running: true, held_context: Some(28_514.), ..TurnContext::default() }];
    assert_eq!(header::missing(&first, "Claude Code").as_deref(), Some(header::LIMIT_AT_END));

    let mut t = Transcript::new();
    t.load(counted(json!({"input": 1, "output": 1}), None));
    let r = header::reading(&t.turn_contexts(), "Claude Code", true).expect("a thread that ran a turn draws the ring");
    assert_eq!((r.snapshot, r.missing.as_deref(), r.compact), (None, Some("Claude Code does not report how much context it holds"), None));
    let running = [turn(Some((1_000., Some(10_000.))), false), turn(None, true)];
    assert_eq!(header::reading(&running, "Claude Code", true).unwrap().compact, Some(Some(header::TURN_IN_FLIGHT.to_owned())));
    assert_eq!(header::reading(&running[..1], "Claude Code", true).unwrap().compact, Some(None));
}

fn checkout(branch: &str, ahead: i64, changed: i64) -> Option<Checkout> {
    Some(Checkout { branch: branch.to_owned(), ahead, changed })
}

fn state(checkout: Option<Checkout>) -> GitState {
    GitState { checkout, base: Some("main".to_owned()), opens_pr: true, ..GitState::default() }
}

#[test]
fn the_git_button_takes_the_act_the_checkout_calls_for() {
    let quick = |s: GitState| {
        let q = git_quick(&s);
        (q.label, q.act)
    };
    assert_eq!(quick(state(checkout("fix/cart", 0, 3))), ("Commit, push and PR", Some(GitAct::CommitPushPr)));
    assert_eq!(quick(GitState { pr_open: true, ..state(checkout("fix/cart", 1, 3)) }), ("Commit and push", Some(GitAct::CommitPush)));
    assert_eq!(quick(GitState { pr_open: true, ..state(checkout("fix/cart", 2, 0)) }), ("Push", Some(GitAct::Push)));
    assert_eq!(quick(state(checkout("fix/cart", 2, 0))), ("Push and create PR", Some(GitAct::CreatePr)));
    assert_eq!(quick(GitState { pr_open: true, ..state(checkout("fix/cart", 0, 0)) }), ("View PR", Some(GitAct::ViewPr)));
    assert_eq!(quick(state(checkout("fix/cart", 0, 0))), ("Create PR", Some(GitAct::CreatePr)));
    assert_eq!(quick(state(checkout("main", 2, 1))), ("Commit", None));
    assert_eq!(quick(state(checkout("(detached)", 0, 2))), ("Commit", None));
    assert_eq!(quick(GitState { busy: true, ..state(checkout("fix/cart", 0, 2)) }), ("Commit, push and PR", None));
    assert_eq!(quick(GitState { opens_pr: false, ..state(checkout("fix/cart", 0, 0)) }), ("Push", None));
    let away = header::away_line("paused");
    assert_eq!(away.as_deref(), Some("Workspace is paused; wake it to push"));
    assert_eq!(header::away_line("waking").as_deref(), Some("Workspace is waking; git actions open when it is running"));
    let held = git_quick(&GitState { away: away.clone(), ..state(checkout("fix/cart", 2, 0)) });
    assert_eq!((held.act, held.holds, held.hint), (None, Some(GitAct::CreatePr), away.clone()));
    assert_eq!(git_quick(&GitState { away, pr_open: true, ..state(checkout("fix/cart", 0, 0)) }).act, Some(GitAct::ViewPr));

    let menu = |s: GitState| git_menu(&s).into_iter().map(|i| (i.id, i.label, i.act, i.disabled)).collect::<Vec<_>>();
    assert_eq!(
        menu(state(checkout("fix/cart", 0, 2))),
        [
            ("commit", "Commit", MenuAct::Changes, false),
            ("push", "Push", MenuAct::Push, true),
            ("pr", "Create PR", MenuAct::CreatePr, true)
        ]
    );
    assert_eq!(
        menu(GitState { pr_open: true, ..state(checkout("fix/cart", 1, 0)) }),
        [("commit", "Commit", MenuAct::Changes, true), ("push", "Push", MenuAct::Push, false), ("pr", "View PR", MenuAct::ViewPr, false)]
    );
    assert_eq!(menu(GitState { opens_pr: false, ..state(checkout("fix/cart", 0, 1)) }).len(), 2);
}

/// The cluster as the app mounts it, a view of its own drawn again with the thread view (`thread_view::Actions`).
struct Actions {
    view: Entity<ThreadView>,
}

impl Render for Actions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.view.update(cx, |v, cx| header::actions(v, window, cx).into_any_element())
    }
}

/// The shell's 52 px header row with its 20 px right pad, the cluster in its actions slot.
struct Probe {
    actions: Entity<Actions>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = div().debug_selector(|| "header".into()).flex().flex_none().items_center().h(px(52.)).pr(px(20.));
        div().size_full().flex().flex_col().child(row.child(div().flex_1()).child(self.actions.clone()))
    }
}

fn window(cx: &mut TestAppContext, events: Vec<SessionEvent>) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
    });
    let actions = cx.new(|cx: &mut Context<Actions>| {
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        Actions { view: view.clone() }
    });
    let probe = cx.new(|_| Probe { actions });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(probe, window, cx));
    cx.simulate_resize(size(px(WIDTH), px(600.)));
    cx.run_until_parked();
    (view, cx)
}

fn seed(cx: &mut VisualTestContext, f: impl FnOnce(&mut header::Bar)) {
    cx.update(|_, cx| {
        header::bar("t", cx).update(cx, |b, cx| {
            f(b);
            cx.notify();
        })
    });
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name).unwrap_or_else(|| panic!("{name} was not drawn"))
}

fn center(b: Bounds<Pixels>) -> Point<Pixels> {
    b.center()
}

fn status(extra: Value) -> Value {
    let mut s = json!({
        "id": "w", "phase": "running", "kind": "local", "project": {"id": "p", "name": "demo", "path": "/demo"},
        "checkout": {"branch": "fix/cart", "ahead": 0, "behind": 0, "changed": 0, "readAt": 1}
    });
    s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s
}

#[gpui_kit::test]
fn the_ring_is_a_28_px_round_button_at_the_headers_right_and_its_card_opens_under_it(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, counted(json!({"input": 1, "output": 1, "context": 50_000, "window": 200_000}), None));
    let head = bounds(cx, "header");
    assert_eq!(head.size.height, px(52.));
    let ring = bounds(cx, "context-ring");
    assert_eq!(ring.size, size(px(28.), px(28.)));
    assert_eq!(ring.center().y, head.center().y, "the ring stands mid-header");
    let cluster = bounds(cx, "header-actions");
    assert_eq!(cluster.right(), px(WIDTH - 21.), "the cluster ends at the header's 20 px pad and its 1 px margin");
    assert!(cx.debug_bounds("context-card").is_none(), "the card shows before a hover");

    cx.simulate_mouse_move(center(ring), None, Modifiers::none());
    cx.run_until_parked();
    let card = bounds(cx, "context-card");
    assert_eq!(card.size.width, px(288.));
    assert_eq!(card.top(), ring.bottom() + px(6.));
    assert_eq!(card.right(), ring.right());
    let meter = bounds(cx, "context-meter");
    assert_eq!(meter.size.height, px(4.));
    assert_eq!(bounds(cx, "context-meter-fill").size.width, meter.size.width * 0.25, "50k of 200k fills a quarter");

    cx.simulate_mouse_move(point(px(100.), px(300.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("context-card").is_none(), "the card stays after the pointer left");
}

#[gpui_kit::test]
fn a_ring_with_no_figures_says_why_and_a_thread_with_no_turn_draws_none(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, counted(json!({"input": 1, "output": 1}), None));
    let ring = bounds(cx, "context-ring");
    cx.simulate_mouse_move(center(ring), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("context-missing").is_some(), "the card holds no line saying why");
    assert!(cx.debug_bounds("context-meter").is_none(), "a meter with nothing to measure");
    view.update(cx, |v, cx| v.load(Vec::new(), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("context-ring").is_none(), "a thread with no turn drew the ring");
}

#[gpui_kit::test]
fn the_git_split_is_two_32_px_keycaps_sharing_an_edge_and_its_menu_opens_under_the_chevron(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, counted(json!({"input": 1, "output": 1}), None));
    assert!(cx.debug_bounds("git-split").is_none(), "the split shows before the host said the checkout");
    seed(cx, |b| {
        b.status = Some(status(json!({})));
        b.bases.insert("p".to_owned(), "main".to_owned());
        b.editors = Some(Vec::new());
    });
    let (ring, quick, more) = (bounds(cx, "context-ring"), bounds(cx, "git-quick"), bounds(cx, "git-more"));
    assert_eq!(quick.size.height, px(32.));
    assert_eq!(more.size, size(px(31.), px(32.)), "the chevron's 8 px pads, its 14 px glyph and its right edge");
    assert_eq!(more.left(), quick.right(), "the chevron shares the main part's right edge as its left");
    assert_eq!(quick.left() - ring.right(), px(8.), "the cluster's gap-2");
    assert_eq!(more.right(), px(WIDTH - 21.), "with no Open the split ends the cluster");

    assert!(cx.debug_bounds("git-menu").is_none());
    cx.simulate_click(center(more), Modifiers::none());
    cx.run_until_parked();
    let menu = bounds(cx, "git-menu");
    assert_eq!(menu.top(), more.bottom() + px(4.));
    assert_eq!(menu.right(), more.right());
    assert!(menu.size.width >= px(128.));
    let item = bounds(cx, "git-item-pr");
    assert_eq!(item.size.height, px(28.));
    assert_eq!(item.left() - menu.left(), px(5.), "the popup's 1 px edge and 4 px pad");
    assert!(cx.debug_bounds("git-item-commit").is_some() && cx.debug_bounds("git-item-push").is_some());

    cx.simulate_click(point(px(100.), px(300.)), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("git-menu").is_none(), "a click outside left the menu up");
}

#[gpui_kit::test]
fn the_git_split_stands_down_where_the_machine_was_released_and_a_lead_opened_copy_offers_no_pr(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| b.status = Some(status(json!({"phase": "released"}))));
    assert!(cx.debug_bounds("git-split").is_none(), "a released machine drew the split");
    seed(cx, |b| b.status = Some(status(json!({"parentThreadId": "lead"}))));
    let more = bounds(cx, "git-more");
    cx.simulate_click(center(more), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("git-item-push").is_some());
    assert!(cx.debug_bounds("git-item-pr").is_none(), "a lead's copy offered a pull request");
}

#[gpui_kit::test]
fn open_holds_its_slot_while_the_editors_are_listed_and_lists_them_under_its_chevron(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| b.status = Some(status(json!({}))));
    let held = bounds(cx, "open-split");
    let git = bounds(cx, "git-split");
    assert_eq!(held.left() - git.right(), px(8.), "Open's slot stands 8 px past the git split while it waits");
    assert_eq!(held.right(), px(WIDTH - 21.));
    seed(cx, |b| b.editors = Some(Vec::new()));
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood with no editor to open in");
    let editor = |id: &str, name: &str| Editor { id: id.to_owned(), name: name.to_owned(), remote: false };
    seed(cx, |b| {
        b.editors = Some(vec![editor("vscode", "VS Code"), editor("zed", "Zed")]);
        b.picked = Some("zed".to_owned());
    });
    let (main, choose) = (bounds(cx, "open-main"), bounds(cx, "open-choose"));
    assert_eq!((main.size.height, choose.size), (px(32.), size(px(31.), px(32.))));
    assert_eq!(choose.left(), main.right());
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    let menu = bounds(cx, "open-menu");
    assert_eq!((menu.top(), menu.right()), (choose.bottom() + px(4.), choose.right()));
    assert_eq!(bounds(cx, "open-item-zed").size.height, px(28.));
    assert!(cx.debug_bounds("open-item-vscode").is_some());
    cx.executor().advance_clock(Duration::from_millis(16));
}

#[gpui_kit::test]
fn open_draws_nothing_for_a_napping_machine_elsewhere(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| {
        b.status = Some(status(json!({"kind": "cloud", "phase": "napping"})));
        b.editors = Some(vec![Editor { id: "zed".to_owned(), name: "Zed".to_owned(), remote: true }]);
    });
    assert!(cx.debug_bounds("git-split").is_some(), "a napping machine still shows its git split, held");
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood for a napping machine on another computer");
}
