// SPDX-License-Identifier: AGPL-3.0-only
#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    cx.update(|_, cx| header::set_editors(Some(Vec::new()), None, cx));
    seed(cx, |b| {
        b.status = Some(status(json!({})));
        b.bases.insert("p".to_owned(), "main".to_owned());
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
    cx.update(|_, cx| header::set_editors(Some(Vec::new()), None, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood with no editor to open in");
    let editor = |id: &str, name: &str| Editor { id: id.to_owned(), name: name.to_owned(), remote: false };
    cx.update(|_, cx| header::set_editors(Some(vec![editor("vscode", "VS Code"), editor("zed", "Zed")]), Some("zed".to_owned()), cx));
    cx.run_until_parked();
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
    cx.update(|_, cx| header::set_editors(Some(vec![Editor { id: "zed".to_owned(), name: "Zed".to_owned(), remote: true }]), None, cx));
    seed(cx, |b| b.status = Some(status(json!({"kind": "cloud", "phase": "napping"}))));
    assert!(cx.debug_bounds("git-split").is_some(), "a napping machine still shows its git split, held");
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood for a napping machine on another computer");
}

/// The frame's crumbs slot as the thread screen fills it (`thread_view::Crumbs`).
struct Crumbs {
    view: Entity<ThreadView>,
}

impl Render for Crumbs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_menu: header::OnMenu = Rc::new(|_, _, _| {});
        self.view.update(cx, |v, cx| header::crumbs(v, on_menu, window, cx))
    }
}

/// The shell's header row with the crumbs in their slot, and the notice the thread screen draws under it.
struct CrumbRow {
    crumbs: Entity<Crumbs>,
    view: Entity<ThreadView>,
}

impl Render for CrumbRow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row =
            div().debug_selector(|| "header".into()).flex().flex_none().items_center().h(px(52.)).pl(px(20.)).pr(px(20.)).gap(px(16.));
        let slot = div()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .child(self.crumbs.clone())
            .child(div().flex_none().ml_auto().w(px(240.)).h(px(32.)));
        let notice = self.view.update(cx, header::notice);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(row.child(slot))
            .child(div().relative().flex_1().child(div().absolute().top(px(8.)).right(px(20.)).children(notice)))
    }
}

fn crumb_window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    view.update(cx, |v, cx| v.show("t", cx));
    let crumbs = cx.new(|cx: &mut Context<Crumbs>| {
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        Crumbs { view: view.clone() }
    });
    let row = cx.new(|cx: &mut Context<CrumbRow>| {
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        CrumbRow { crumbs, view: view.clone() }
    });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(row, window, cx));
    cx.simulate_resize(size(px(WIDTH), px(600.)));
    cx.run_until_parked();
    (view, cx)
}

fn row(extra: Value) -> Value {
    let mut r = json!({"id": "s", "threadId": "t", "workspaceId": "w", "harness": "claude", "status": "running", "prompt": "Fix the cart", "project": {"id": "p", "name": "demo"}});
    r.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    r
}

#[test]
fn a_waiting_thread_says_what_it_waits_on() {
    assert_eq!(header::waiting_line(&row(json!({}))), None);
    assert_eq!(header::waiting_line(&row(json!({"asking": "Run a command: ls"}))).as_deref(), Some("Run a command: ls"));
    let behind = json!({"waitingOn": {"threadId": "k", "workspaceId": "w", "sessionId": "ks", "title": "kid", "prompt": {"askId": "a", "toolName": "Bash", "input": "{\"command\":\"ls -la\"}", "options": []}}});
    assert_eq!(header::waiting_line(&row(behind)).as_deref(), Some("Run a command: ls -la needs an answer"));
    let named = header::crumb(&row(json!({"harnessTitle": "Cart fix"})));
    assert_eq!((named.project.as_deref(), named.title.as_str()), (Some("demo"), "Cart fix"));
    assert_eq!(header::crumb(&row(json!({"prompt": null}))).title, header::NEW_THREAD);
}

#[gpui_kit::test]
fn the_crumb_wears_the_question_glyph_while_the_thread_waits_on_the_person(cx: &mut TestAppContext) {
    let (_, cx) = crumb_window(cx);
    assert!(cx.debug_bounds("crumbs-title").is_none(), "a title stood before the host listed the thread");
    let title = "A medium thread title that runs some way across the header row";
    seed(cx, |b| b.row = Some(row(json!({"harnessTitle": title, "asking": "Run a command: ls"}))));
    let (shown, glyph, line) = (bounds(cx, "crumbs-title"), bounds(cx, "crumbs-waiting"), bounds(cx, "crumbs"));
    assert_eq!(glyph.size.width, px(14.), "the 14 px glyph");
    assert_eq!(glyph.left() - shown.right(), px(8.), "the line's gap-2 after the title");
    assert_eq!(glyph.center().y, line.center().y);
    let project = bounds(cx, "crumbs-project");
    // `max-w-[70%]` of a line sized to all it holds uncut, the glyph with it, as Blink resolves a percentage there.
    let (natural, slash) = cx.update(|window, cx| {
        let family = gpui_kit::component::ActiveTheme::theme(cx).font_family.clone();
        (desktop_gpui::ui::text::width(title, 14., family.clone(), window), desktop_gpui::ui::text::width("/", 14., family, window))
    });
    let whole = project.size.width + slash + natural + px(14.) + px(8.) * 3.;
    assert!((line.size.width - whole).abs() < px(1.), "the line is {:?}, not the {:?} it holds", line.size.width, whole);
    assert!(natural > line.size.width * 0.7, "the title is long enough to be cut");
    assert!(
        (shown.size.width - line.size.width * 0.7).abs() < px(0.5),
        "{:?} is not 70% of the line's {:?}",
        shown.size.width,
        line.size.width
    );
    seed(cx, |b| b.row = Some(row(json!({}))));
    assert!(cx.debug_bounds("crumbs-waiting").is_none(), "the glyph stayed once the question was answered");
}

#[gpui_kit::test]
fn a_notice_stands_under_the_header_until_it_is_closed(cx: &mut TestAppContext) {
    let (_, cx) = crumb_window(cx);
    seed(cx, |b| {
        b.status = Some(status(json!({"name": "cart"})));
        b.said = Some(header::Said { text: "Push refused".to_owned(), failed: true, about: Some("cart".to_owned()) });
    });
    let (notice, head) = (bounds(cx, "thread-notice"), bounds(cx, "header"));
    assert_eq!((notice.size.width, notice.top(), notice.right()), (px(356.), head.bottom() + px(8.), px(WIDTH - 20.)));
    let close = bounds(cx, "thread-notice-close");
    cx.simulate_click(close.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("thread-notice").is_none(), "the close left the notice up");
}

/// Each thread the host lists: its first prompt, its workspace and its id.
type Listed = Vec<(String, String, String)>;

/// The cluster over a thread view that follows `thread` on the host, as the screen moves it there.
fn following<'a>(
    cx: &'a mut TestAppContext,
    host: &common::Throwaway,
) -> (Entity<ThreadView>, Listed, desktop_gpui::screens::Host, &'a mut VisualTestContext) {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = threads
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    let shell = desktop_gpui::screens::Host::new(Arc::new(client), runtime);
    cx.executor().allow_parking();
    let (view, cx) = window(cx, Vec::new());
    (view, threads, shell, cx)
}

fn follow(
    view: &Entity<ThreadView>,
    host: &desktop_gpui::screens::Host,
    threads: &[(String, String, String)],
    prompt: &str,
    cx: &mut VisualTestContext,
) -> (String, String) {
    let (_, workspace, thread) = threads.iter().find(|(p, _, _)| p == prompt).unwrap_or_else(|| panic!("no thread {prompt}")).clone();
    let item = desktop_gpui::sidebar::ThreadItem {
        workspace_id: workspace.clone(),
        thread_id: thread.clone(),
        project: String::new(),
        title: String::new(),
    };
    view.update(cx, |v, cx| v.follow(host, &item, false, cx));
    cx.run_until_parked();
    (workspace, thread)
}

fn wait(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(40);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn status_of(id: &str) -> Value {
    json!({"type": "workspace.status", "status": status(json!({"id": id}))})
}

#[gpui_kit::test]
fn a_followed_thread_takes_its_own_workspaces_status_and_no_other(cx: &mut TestAppContext) {
    let shared = common::shared();
    let (view, threads, host, cx) = following(cx, &shared.host);
    let (workspace, thread) = follow(&view, &host, &threads, "first", cx);
    let bar = cx.update(|_, cx| header::bar(&thread, cx));
    wait(cx, "the cluster's reads", |cx| bar.read_with(cx, |b, _| !b.catalogs.is_empty()));
    view.update(cx, |v, cx| header::heard(v, "workspace.status", &status_of("elsewhere"), cx));
    assert_eq!(bar.read_with(cx, |b, _| b.status.clone()), None, "another workspace's status landed on the thread's split");
    view.update(cx, |v, cx| header::heard(v, "workspace.status", &status_of(&workspace), cx));
    assert_eq!(bar.read_with(cx, |b, _| b.status.as_ref().and_then(|s| s["id"].as_str().map(str::to_owned))), Some(workspace));
    assert!(cx.debug_bounds("git-split").is_some(), "the pushed status drew no split");
}

#[gpui_kit::test]
fn a_refused_editor_list_is_asked_again_by_the_next_thread(cx: &mut TestAppContext) {
    let mut server = common::Throwaway::start();
    let folder = server.repo("repo");
    server.wsp(&["add", &folder.to_string_lossy()]);
    server.wsp(&["run", "repo", "first", "--agent", "claude"]);
    server.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (view, threads, host, cx) = following(cx, &server);
    server.stop();
    follow(&view, &host, &threads, "first", cx);
    std::thread::sleep(Duration::from_secs(2));
    cx.run_until_parked();
    let listed = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.try_global::<header::Editors>().and_then(|e| e.list.clone()));
    assert_eq!(listed(cx), None, "a refused list stood as an empty one");
    server.restart();
    follow(&view, &host, &threads, "second", cx);
    wait(cx, "the editors listed again", |cx| listed(cx).is_some());
}

#[gpui_kit::test]
fn the_rename_box_lets_its_bar_go_with_the_thread(cx: &mut TestAppContext) {
    let shared = common::shared();
    let (view, threads, host, cx) = following(cx, &shared.host);
    let (_, thread) = follow(&view, &host, &threads, "first", cx);
    let bar = cx.update(|_, cx| header::bar(&thread, cx));
    view.update(cx, header::refresh);
    wait(cx, "the thread's row and the cluster's reads", |cx| bar.read_with(cx, |b, _| b.row.is_some() && !b.catalogs.is_empty()));
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| view.update(cx, |v, cx| header::open_rename(v, window, cx)));
    assert!(bar.read_with(cx, |b, cx| b.rename_box(cx)).is_some(), "the box did not open");
    let gone = bar.downgrade();
    drop(bar);
    let _another = cx.update(|_, cx| header::bar("another", cx));
    // The last frame's handlers hold the bar until the next frame is drawn.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(gone.upgrade().is_none(), "the open box kept the last thread's bar alive");
}
