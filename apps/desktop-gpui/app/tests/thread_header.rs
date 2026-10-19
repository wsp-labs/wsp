// SPDX-License-Identifier: AGPL-3.0-only
#[path = "../../host-client/tests/common/mod.rs"]
mod common;
mod wire;

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::pr_pane::files::open_split::{self, Editor, OpenSplit};
use desktop_gpui::screens::pr_pane::pull_request::git_split::{self, quick_action, GitAct, GitSplit};
use desktop_gpui::screens::thread_view::header;
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::{Transcript, TurnContext};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Bounds, Context, Entity, Global, Modifiers, Pixels, Point, TestAppContext,
    VisualTestContext, Window,
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

/// The cluster as the app mounts it, a view of its own drawn again with the thread view (`thread_view::Actions`), with
/// the panes' git button and Open, one each.
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

/// The splits the last cluster built mounts, for a test to read.
struct Mounted(Entity<GitSplit>);

impl Global for Mounted {}

fn actions_of(view: &Entity<ThreadView>, host: &desktop_gpui::screens::Host, cx: &mut TestAppContext) -> Entity<Actions> {
    let git = cx.new(|_| GitSplit::fed(host.clone()));
    let open = cx.new(OpenSplit::new);
    cx.update(|cx| cx.set_global(Mounted(git.clone())));
    cx.new(|cx: &mut Context<Actions>| {
        cx.observe(view, |_, _, cx| cx.notify()).detach();
        Actions { view: view.clone(), git, open, bar: header::BarWatch::default() }
    })
}

fn git_of(cx: &mut VisualTestContext) -> Entity<GitSplit> {
    cx.update(|_, cx| cx.global::<Mounted>().0.clone())
}

/// A host handle on `host` and the threads it lists by their first prompt.
fn connect(host: &common::Throwaway) -> (desktop_gpui::screens::Host, Listed) {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = threads
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    (desktop_gpui::screens::Host::new(Arc::new(client), runtime), threads)
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

/// The cluster over a thread view that follows nothing, its splits on the shared host only to hold a handle.
fn window(cx: &mut TestAppContext, events: Vec<SessionEvent>) -> (Entity<ThreadView>, &mut VisualTestContext) {
    let (host, _) = connect(&common::shared().host);
    window_on(cx, events, &host)
}

fn window_on<'a>(
    cx: &'a mut TestAppContext,
    events: Vec<SessionEvent>,
    host: &desktop_gpui::screens::Host,
) -> (Entity<ThreadView>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    cluster_window(cx, events, host)
}

/// The cluster over a thread view in a window of its own, in an app already set up.
fn cluster_window<'a>(
    cx: &'a mut TestAppContext,
    events: Vec<SessionEvent>,
    host: &desktop_gpui::screens::Host,
) -> (Entity<ThreadView>, &'a mut VisualTestContext) {
    let view = cx.new(ThreadView::new);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
    });
    let actions = actions_of(&view, host, cx);
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
fn the_git_split_is_the_panes_two_outline_buttons_joined_at_a_seam(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, counted(json!({"input": 1, "output": 1}), None));
    assert!(cx.debug_bounds("git-split").is_none(), "the split shows before the host said the checkout");
    cx.update(|_, cx| open_split::set_editors(Some(Vec::new()), None, cx));
    seed(cx, |b| {
        b.status = Some(status(json!({})));
        b.bases.insert("p".to_owned(), "main".to_owned());
    });
    let (ring, quick, more) = (bounds(cx, "context-ring"), bounds(cx, "git-quick"), bounds(cx, "git-more"));
    assert_eq!(quick.size.height, px(32.));
    assert_eq!(more.size, size(px(32.), px(32.)), "the chevron's 8 px pads, its 14 px glyph and its two edges");
    assert_eq!(more.left(), quick.right() - px(1.), "the chevron's left edge lies on the main part's right one, as -ms-px");
    assert_eq!(quick.left() - ring.right(), px(8.), "the cluster's gap-2");
    assert_eq!(more.right(), px(WIDTH - 21.), "with no Open the split ends the cluster");
    let quick = git_of(cx).read_with(cx, |g, _| g.state().map(|s| quick_action(&s)));
    assert_eq!(
        quick.map(|q| (q.label, q.act)),
        Some(("Create PR".to_owned(), Some(GitAct::CreatePr))),
        "a clean branch of its own offers its PR"
    );
}

#[gpui_kit::test]
fn the_git_split_stands_down_where_the_machine_was_released_and_a_lead_opened_copy_offers_no_pr(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| b.status = Some(status(json!({"phase": "released"}))));
    assert!(cx.debug_bounds("git-split").is_none(), "a released machine drew the split");
    seed(cx, |b| b.status = Some(status(json!({"parentThreadId": "lead"}))));
    assert!(cx.debug_bounds("git-split").is_some(), "a lead's copy drew no split");
    let items = git_of(cx).read_with(cx, |g, _| g.state().map(|s| git_split::menu(&s).into_iter().map(|i| i.id).collect::<Vec<_>>()));
    assert_eq!(items, Some(vec!["commit", "push"]), "a lead's copy offered a pull request");
}

#[gpui_kit::test]
fn under_640_px_the_git_button_and_open_are_their_marks_alone_with_their_words_on_hover(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    cx.update(|_, cx| open_split::set_editors(Some(vec![editor("zed", "Zed", false)]), None, cx));
    seed(cx, |b| {
        b.status = Some(status(json!({})));
        b.bases.insert("p".to_owned(), "main".to_owned());
    });
    assert!(bounds(cx, "git-quick").size.width > px(60.), "at 1024 px the git button says its act");
    assert!(bounds(cx, "open-main").size.width > px(60.), "at 1024 px Open says Open");
    cx.simulate_resize(size(px(600.), px(600.)));
    cx.run_until_parked();
    let (quick, open) = (bounds(cx, "git-quick"), bounds(cx, "open-main"));
    assert_eq!(quick.size.width, px(40.), "under 640 px the git button is more than its 14 px glyph, its 12 px pads and its edges");
    assert_eq!(open.size.width, px(42.), "under 640 px Open is more than its 16 px mark, its 12 px pads and its edges");
    assert_eq!(bounds(cx, "git-more").left(), quick.right() - px(1.), "the seam moved off the main part's edge");
    cx.simulate_mouse_move(open.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "Open's words are not on its hover");
}

fn editor(id: &str, name: &str, remote: bool) -> Editor {
    Editor { id: id.to_owned(), name: name.to_owned(), remote }
}

#[gpui_kit::test]
fn open_holds_its_slot_while_the_editors_are_listed_and_lists_them_under_its_chevron(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| b.status = Some(status(json!({}))));
    let held = bounds(cx, "open-split");
    let git = bounds(cx, "git-split");
    assert_eq!(held.left() - git.right(), px(8.), "Open's slot stands 8 px past the git split while it waits");
    assert_eq!(held.right(), px(WIDTH - 21.));
    cx.update(|_, cx| open_split::set_editors(Some(Vec::new()), None, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood with no editor to open in");
    cx.update(|_, cx| {
        open_split::set_editors(Some(vec![editor("vscode", "VS Code", false), editor("zed", "Zed", false)]), Some("zed".to_owned()), cx)
    });
    cx.run_until_parked();
    let (split, main, choose) = (bounds(cx, "open-split"), bounds(cx, "open-main"), bounds(cx, "open-choose"));
    assert_eq!(split.size, held.size, "the held slot was not as wide as Open once listed");
    assert_eq!((main.size.height, choose.size), (px(32.), size(px(32.), px(32.))));
    assert_eq!(choose.left(), main.right() - px(1.), "the chevron's left edge lies on Open's right one");
    assert!(cx.debug_bounds("open-item-zed").is_none(), "the menu stood before a press");
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    let (zed, vscode) = (bounds(cx, "open-item-zed"), bounds(cx, "open-item-vscode"));
    assert!(zed.top() > choose.bottom() && zed.right() <= choose.right(), "the menu does not hang under the chevron, its end on its end");
    assert!(vscode.top() < zed.top(), "the editors are not in the host's order");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-item-zed").is_none(), "Escape left the menu up");
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(point(px(100.), px(300.)), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-item-zed").is_none(), "a click outside left the menu up");
}

#[gpui_kit::test]
fn open_wears_the_default_editors_mark_and_its_menu_the_keycaps_of_its_chord_on_the_default(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    cx.update(|_, cx| {
        open_split::set_editors(Some(vec![editor("vscode", "VS Code", false), editor("zed", "Zed", false)]), Some("zed".to_owned()), cx)
    });
    seed(cx, |b| b.status = Some(status(json!({}))));
    let (main, mark) = (bounds(cx, "open-main"), bounds(cx, "open-mark"));
    let zed_mark = cx.debug_bounds("editor-mark-zed");
    assert!(zed_mark.is_some_and(|m| m == mark), "the main part wears no mark of the default, Zed");
    assert_eq!(mark.size, size(px(16.), px(16.)), "the mark is not 16 px");
    assert_eq!(mark.left() - main.left(), px(13.), "the mark stands past Open's edge and its 12 px pad");
    let choose = bounds(cx, "open-choose");
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    let (zed, vscode, cap) = (bounds(cx, "open-item-zed"), bounds(cx, "open-item-vscode"), bounds(cx, "keycap"));
    let vscode_mark = bounds(cx, "editor-mark-vscode");
    assert!(vscode.contains(&vscode_mark.center()), "VS Code's row wears no mark of its own");
    assert!(zed.contains(&cap.center()), "the default, Zed, holds no keycap of its chord");
    assert!(!vscode.contains(&cap.center()), "a keycap stands on an editor that is not the default");
    assert_eq!(cap.size.height, px(20.), "the chord is not drawn in the app's 20 px keycaps");
    assert_eq!(open_split::chord_keys("⌘O"), ["⌘", "O"]);
    assert_eq!(open_split::chord_keys("Ctrl+O"), ["Ctrl", "O"]);
}

#[gpui_kit::test]
fn open_on_a_small_machine_elsewhere_offers_its_roads_and_says_what_an_editor_costs_there(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    cx.update(|_, cx| open_split::set_editors(Some(vec![editor("vscode", "VS Code", false), editor("zed", "Zed", true)]), None, cx));
    seed(cx, |b| b.status = Some(status(json!({"kind": "cloud", "phase": "running", "size": {"memMb": 4096}}))));
    let choose = bounds(cx, "open-choose");
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    let (small, zed) = (bounds(cx, "open-small"), bounds(cx, "open-item-zed"));
    assert!(small.bottom() <= zed.top(), "the line on what an editor costs does not stand over the editors");
    assert!(small.size.width <= px(256.), "the line runs past max-w-64");
    assert!(cx.debug_bounds("open-item-vscode").is_none(), "an editor with no road to another computer was offered");
    cx.simulate_keystrokes("escape");
    seed(cx, |b| b.status = Some(status(json!({"kind": "cloud", "phase": "running", "size": {"memMb": 8192}}))));
    let choose = bounds(cx, "open-choose");
    cx.simulate_click(center(choose), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-item-zed").is_some());
    assert!(cx.debug_bounds("open-small").is_none(), "an 8 GB machine heard what an editor costs");
}

#[gpui_kit::test]
fn open_draws_nothing_where_the_host_refused_to_list_its_editors(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    seed(cx, |b| b.status = Some(status(json!({}))));
    assert!(cx.debug_bounds("open-split").is_some(), "Open's slot stands while the host lists its editors");
    cx.update(|_, cx| open_split::list_refused(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-split").is_none(), "Open's slot stood after the host refused to list its editors");
    cx.update(|_, cx| open_split::set_editors(Some(vec![editor("zed", "Zed", false)]), None, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("open-split").is_some(), "Open stayed down once a list landed");
}

#[gpui_kit::test]
fn open_draws_nothing_for_a_napping_machine_elsewhere(cx: &mut TestAppContext) {
    let (_, cx) = window(cx, Vec::new());
    cx.update(|_, cx| open_split::set_editors(Some(vec![editor("zed", "Zed", true)]), None, cx));
    seed(cx, |b| b.status = Some(status(json!({"kind": "cloud", "phase": "napping"}))));
    assert!(cx.debug_bounds("git-split").is_some(), "a napping machine still shows its git split, held");
    assert!(cx.debug_bounds("open-split").is_none(), "Open stood for a napping machine on another computer");
}

/// The frame's crumbs slot as the thread screen fills it (`thread_view::Crumbs`).
struct Crumbs {
    view: Entity<ThreadView>,
    bar: header::BarWatch,
}

impl Render for Crumbs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_menu: header::OnMenu = Rc::new(|_, _, _| {});
        self.bar.follow(&self.view, cx);
        self.view.update(cx, |v, cx| header::crumbs(v, on_menu, window, cx))
    }
}

/// The shell's header row with the crumbs in their slot, over the thread view.
struct CrumbRow {
    crumbs: Entity<Crumbs>,
    view: Entity<ThreadView>,
    bar: header::BarWatch,
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
        self.bar.follow(&self.view, cx);
        div().size_full().flex().flex_col().child(row.child(slot)).child(div().relative().flex_1().child(self.view.clone()))
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
        Crumbs { view: view.clone(), bar: header::BarWatch::default() }
    });
    let row = cx.new(|cx: &mut Context<CrumbRow>| {
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        CrumbRow { crumbs, view: view.clone(), bar: header::BarWatch::default() }
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
fn a_pushed_row_that_leaves_out_the_opening_prompt_keeps_the_title(cx: &mut TestAppContext) {
    let (view, cx) = crumb_window(cx);
    seed(cx, |b| b.row = Some(row(json!({}))));
    // What a read stamp pushes: the thread's row again, without the prompt that opened it.
    let mut stamped = row(json!({"readAt": 2}));
    stamped.as_object_mut().unwrap().remove("prompt");
    let frame = json!({"type": "session.row", "workspaceId": "w", "threadId": "t", "id": "s", "row": stamped});
    view.update(cx, |v, cx| header::heard(v, "session.row", &frame, cx));
    cx.run_until_parked();
    let title = cx.update(|_, cx| header::bar("t", cx).read(cx).row.as_ref().map(|r| header::crumb(r).title));
    assert_eq!(title.as_deref(), Some("Fix the cart"), "the crumb lost the thread's title to a pushed row");
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
fn under_640_px_the_project_crumb_is_its_glyph_alone_its_name_on_hover_and_no_slash_after_it(cx: &mut TestAppContext) {
    let (_, cx) = crumb_window(cx);
    seed(cx, |b| b.row = Some(row(json!({"harnessTitle": "Cart fix"}))));
    let (project, title) = (bounds(cx, "crumbs-project"), bounds(cx, "crumbs-title"));
    assert!(project.size.width > px(16.), "at 1024 px the project's name stands beside its glyph");
    assert!(title.left() - project.right() > px(8.), "at 1024 px no slash stands between the project and the title");
    cx.simulate_resize(size(px(600.), px(600.)));
    cx.run_until_parked();
    let (project, title) = (bounds(cx, "crumbs-project"), bounds(cx, "crumbs-title"));
    assert_eq!(project.size.width, px(16.), "under 640 px the project crumb is more than its glyph");
    assert_eq!(title.left() - project.right(), px(8.), "under 640 px the slash after the project still stands");
    assert!(cx.debug_bounds("tooltip").is_none());
    cx.simulate_mouse_move(project.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "the project's name is not on the glyph's hover");
}

#[gpui_kit::test]
fn the_crumbs_name_the_thread_that_opened_this_one_between_its_project_and_its_title(cx: &mut TestAppContext) {
    use desktop_gpui::screens::sidebar::host::Change;
    let (_, cx) = crumb_window(cx);
    let sidebar = cx.update(|_, cx| {
        let sidebar = cx.new(desktop_gpui::sidebar::Sidebar::new);
        desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx);
        sidebar
    });
    let lead = json!({"id": "s-lead", "threadId": "lead", "workspaceId": "w0", "harness": "claude", "status": "completed",
        "harnessTitle": "Plan the cart", "startedAt": 1.0, "project": {"id": "p", "name": "demo"}});
    let child = row(json!({"parentThreadId": "lead", "harnessTitle": "Cart fix", "startedAt": 2.0}));
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Row { id: "s-lead".into(), row: Some(lead) }, cx);
        s.apply(Change::Row { id: "s".into(), row: Some(child.clone()) }, cx);
    });
    seed(cx, |b| b.row = Some(child));
    let (project, title) = (bounds(cx, "crumbs-project"), bounds(cx, "crumbs-title"));
    let opener = cx.debug_bounds("crumbs-opener").expect("the crumbs do not name the thread that opened this one");
    assert!(
        project.right() < opener.left() && opener.right() < title.left(),
        "the opener does not stand between the project and the title"
    );
    assert!(title.left() - opener.right() > px(8.), "no slash stands after the opener");
    seed(cx, |b| b.row = Some(row(json!({"harnessTitle": "Cart fix"}))));
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s".into(), row: Some(row(json!({"harnessTitle": "Cart fix"}))) }, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("crumbs-opener").is_none(), "a thread the person opened names an opener");
}

#[gpui_kit::test]
fn a_change_to_the_cluster_draws_the_header_and_builds_no_row_of_the_transcript(cx: &mut TestAppContext) {
    let (view, cx) = crumb_window(cx);
    view.update(cx, |v, cx| v.load(counted(json!({"input": 1, "output": 1}), None), cx));
    cx.run_until_parked();
    let built = view.read_with(cx, |v, _| v.built());
    assert!(built > 0, "the transcript drew no row");
    seed(cx, |b| b.row = Some(row(json!({"harnessTitle": "Cart fix"}))));
    assert!(cx.debug_bounds("crumbs-title").is_some(), "the crumbs were not drawn again");
    assert_eq!(view.read_with(cx, |v, _| v.built()), built, "a change to the cluster built the transcript's rows again");
}

/// Each thread the host lists: its first prompt, its workspace and its id.
type Listed = Vec<(String, String, String)>;

/// The cluster over a thread view that follows `thread` on the host, as the screen moves it there.
fn following<'a>(
    cx: &'a mut TestAppContext,
    host: &common::Throwaway,
) -> (Entity<ThreadView>, Listed, desktop_gpui::screens::Host, &'a mut VisualTestContext) {
    let (shell, threads) = connect(host);
    cx.executor().allow_parking();
    let (view, cx) = window_on(cx, Vec::new(), &shell);
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
    let listed = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.try_global::<open_split::Editors>().and_then(|e| e.list.clone()));
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

/// The cluster's header row over the thread view it reads, both drawn, as the thread screen draws them.
struct Drawn {
    actions: Entity<Actions>,
    view: Entity<ThreadView>,
}

impl Render for Drawn {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = div().flex().flex_none().items_center().h(px(52.)).pr(px(20.)).child(div().flex_1()).child(self.actions.clone());
        div().size_full().flex().flex_col().child(row).child(div().relative().flex_1().child(self.view.clone()))
    }
}

/// Follows `prompt`'s thread and waits until the view has read it and the cluster has its agents, then a little more
/// for anything the switch sends late.
fn switch_to(view: &Entity<ThreadView>, host: &desktop_gpui::screens::Host, threads: &Listed, prompt: &str, cx: &mut VisualTestContext) {
    let (_, thread) = follow(view, host, threads, prompt, cx);
    let bar = cx.update(|_, cx| header::bar(&thread, cx));
    wait(cx, "the thread read", |cx| view.read_with(cx, |v, _| v.loaded()));
    view.update(cx, header::refresh);
    wait(cx, "its row and its agents", |cx| bar.read_with(cx, |b, _| b.row.is_some() && !b.catalogs.is_empty()));
    let none = desktop_gpui::transcript::Can::default();
    wait(cx, "its agent's flags", |cx| view.read_with(cx, |v, _| v.agent_can() != none));
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn a_switch_asks_the_host_nothing_for_the_header_once_it_has_read_the_projects_and_agents(cx: &mut TestAppContext) {
    let server = common::Throwaway::start();
    for repo in ["repo", "other"] {
        let folder = server.repo(repo);
        server.wsp(&["add", &folder.to_string_lossy()]);
    }
    for (repo, prompt) in [("repo", "first"), ("repo", "second"), ("repo", "third"), ("other", "fourth")] {
        server.wsp(&["run", repo, prompt, "--agent", "claude"]);
    }
    let (sent, _dir, state) = wire::proxied(&server);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(wsp_host_client::HostClient::connect(&state, &server.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads: Listed = runtime
        .block_on(client.threads())
        .unwrap_or_else(|f| panic!("{}", f.message))
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    let client = Arc::new(client);
    let host = desktop_gpui::screens::Host::new(client.clone(), runtime.clone());
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let sidebar = cx.new(desktop_gpui::sidebar::Sidebar::new);
    sidebar.update(cx, |s, cx| s.attach(desktop_gpui::sidebar::Link { host: client, runtime, page: None }, cx));
    cx.update(|cx| desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx));
    let order = ["first", "fourth", "second", "third"];
    let view = cx.new(ThreadView::new);
    let actions = actions_of(&view, &host, cx);
    let drawn = cx.new(|_| Drawn { actions, view: view.clone() });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(drawn, window, cx));
    cx.simulate_resize(size(px(WIDTH), px(600.)));
    wait(cx, "the sidebar's rows", |cx| sidebar.read_with(cx, |s, _| s.rows().len() >= 4));
    // The sidebar's own reads, its agents' list among them, go out before any thread opens.
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    sent.lock().unwrap().clear();
    for prompt in order {
        switch_to(&view, &host, &threads, prompt, cx);
    }
    let first = (wire::count(&sent, "projects.list"), wire::count(&sent, "harnesses.list"), cx.update(|_, cx| header::reads(cx)));
    let can = view.read_with(cx, |v, _| v.agent_can());
    sent.lock().unwrap().clear();
    for at in 0..8 {
        switch_to(&view, &host, &threads, order[at % 4], cx);
        assert_eq!(view.read_with(cx, |v, _| v.agent_can()), can, "switch {at} took other flags for the same agent");
    }
    let views = wire::count(&sent, "sessions.history");
    let rows = sent.lock().unwrap().iter().filter(|(op, named)| op == "sessions.list" && *named).count();
    let after = (wire::count(&sent, "projects.list"), wire::count(&sent, "harnesses.list"), cx.update(|_, cx| header::reads(cx)));
    println!("opening four threads in two workspaces: {first:?}; eight switches: {after:?}");
    // Each thread holds more than a page, and a short window stands near its top: a switch reads its newest page and
    // the one before it.
    assert!(views >= 8, "eight switches read their threads {views} times");
    // The sidebar's read of each workspace's agents is the only one: the cluster and the view's facts take it.
    assert_eq!(first, (1, 0, 1), "opening four threads of two workspaces: (projects.list, harnesses.list, the header's asks)");
    assert_eq!(after, (0, 0, 1), "eight switches: (projects.list, harnesses.list, the header's asks)");
    assert_eq!(rows, 0, "eight switches read a workspace's rows {rows} times though the sidebar holds them");
}

/// A subagent's page under its crumbs, as the frame draws the page's main view and its crumbs slot.
struct SubPage {
    screen: gpui_kit::AnyView,
    crumbs: gpui_kit::AnyView,
}

impl Render for SubPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = div().debug_selector(|| "header".into()).flex().items_center().h(px(52.)).pl(px(20.)).child(self.crumbs.clone());
        div().size_full().flex().flex_col().child(row).child(self.screen.clone())
    }
}

#[gpui_kit::test]
fn a_subagents_crumbs_are_its_lead_a_slash_and_its_title(cx: &mut TestAppContext) {
    use desktop_gpui::screens::thread_view::subagent::SubagentScreen;
    use desktop_gpui::screens::{self, Opened, Screen as _};
    let shared = common::shared();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime
        .block_on(wsp_host_client::HostClient::connect(&shared.host.state(), &shared.host.env()))
        .unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let lead = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the shared host's first thread");
    let (workspace, thread) = (lead.workspace_id.clone(), lead.thread_id.clone().expect("a thread id"));
    let host = desktop_gpui::screens::Host::new(Arc::new(client), runtime);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let arg = format!("{workspace}/{thread}/toolu-survey");
    let mut mounted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let opened = Opened { arg: Some(arg.into()), ..Opened::default() };
        let page = (screens::screen(SubagentScreen::NAME).unwrap().mount)(&host, opened, window, cx);
        let shown = cx.new(|_| SubPage { screen: page.view.clone(), crumbs: page.parts.crumbs.clone().unwrap() });
        mounted = Some(page);
        Root::new(shown, window, cx)
    });
    cx.simulate_resize(size(px(WIDTH), px(600.)));
    let screen = mounted.as_ref().unwrap().view.clone().downcast::<SubagentScreen>().ok().unwrap();
    let view = screen.read_with(cx, |s, _| s.view().clone());
    wait(cx, "the lead read", |cx| view.read_with(cx, |v, _| v.loaded()));
    let survey = desktop_gpui::transcript::ListedSubagent {
        call: "toolu-survey".to_owned(),
        title: "Survey".to_owned(),
        state: "done".to_owned(),
        model: None,
        asked: None,
        started: 1_000.,
        ended: Some(9_000.),
        failure: None,
    };
    let listing =
        desktop_gpui::transcript::Listing { thread, title: Some("Cart fix".to_owned()), subagents: vec![survey], ..Default::default() };
    view.update(cx, |v, cx| v.set_listing(listing, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (lead, title) = (bounds(cx, "crumbs-lead"), bounds(cx, "crumbs-title"));
    let between = title.left() - lead.right();
    assert!(between < px(28.), "{between:?} stand between the lead and the title: more than a slash and its gaps");
}

/// A thread of `prompt` as the sidebar would hand it to the view.
fn item_of(threads: &Listed, prompt: &str) -> desktop_gpui::sidebar::ThreadItem {
    let (_, workspace, thread) = threads.iter().find(|(p, _, _)| p == prompt).unwrap_or_else(|| panic!("no thread {prompt}")).clone();
    desktop_gpui::sidebar::ThreadItem { workspace_id: workspace, thread_id: thread, project: String::new(), title: String::new() }
}

#[gpui_kit::test]
fn a_view_closed_while_its_agents_are_read_leaves_them_for_the_next_thread(cx: &mut TestAppContext) {
    let server = common::Throwaway::start();
    let folder = server.repo("repo");
    server.wsp(&["add", &folder.to_string_lossy()]);
    for prompt in ["first", "second"] {
        server.wsp(&["run", "repo", prompt, "--agent", "claude"]);
    }
    let (view, threads, host, cx) = following(cx, &server);
    // A view that asks for the workspace's agents and is gone before they are answered.
    let second = item_of(&threads, "second");
    cx.update(|window, cx| {
        let gone = cx.new(ThreadView::new);
        let git = cx.new(|_| GitSplit::fed(host.clone()));
        let open = cx.new(OpenSplit::new);
        gone.update(cx, |v, cx| {
            v.follow(&host, &second, false, cx);
            _ = header::actions(v, &git, &open, window, cx).into_any_element();
        });
    });
    let (_, thread) = follow(&view, &host, &threads, "first", cx);
    let bar = cx.update(|_, cx| header::bar(&thread, cx));
    wait(cx, "the next thread's agents", |cx| bar.read_with(cx, |b, _| !b.catalogs.is_empty()));
}

#[gpui_kit::test]
fn a_host_read_again_whole_gives_the_header_and_the_view_its_agents_again(cx: &mut TestAppContext) {
    let mut server = common::Throwaway::start();
    let folder = server.repo("repo");
    server.wsp(&["add", &folder.to_string_lossy()]);
    server.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(
        runtime.block_on(wsp_host_client::HostClient::connect(&server.state(), &server.env())).unwrap_or_else(|f| panic!("{}", f.message)),
    );
    let threads: Listed = runtime
        .block_on(client.threads())
        .unwrap_or_else(|f| panic!("{}", f.message))
        .iter()
        .map(|t| (t.prompt.clone().unwrap_or_default(), t.workspace_id.clone(), t.thread_id.clone().expect("a thread id")))
        .collect();
    let host = desktop_gpui::screens::Host::new(client.clone(), runtime.clone());
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let sidebar = cx.new(desktop_gpui::sidebar::Sidebar::new);
    sidebar.update(cx, |s, cx| s.attach(desktop_gpui::sidebar::Link { host: client, runtime, page: None }, cx));
    cx.update(|cx| desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx));
    let view = cx.new(ThreadView::new);
    let actions = actions_of(&view, &host, cx);
    let drawn = cx.new(|_| Drawn { actions, view: view.clone() });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(drawn, window, cx));
    cx.simulate_resize(size(px(WIDTH), px(600.)));
    let item = item_of(&threads, "first");
    view.update(cx, |v, cx| v.follow(&host, &item, false, cx));
    let read = |cx: &mut VisualTestContext| cx.update(|_, cx| header::catalogs_of(&item.workspace_id, cx));
    wait(cx, "the workspace's agents", |cx| read(cx).is_some());
    let before = read(cx).unwrap();
    // A host started again may run a newer build of an agent: what it said before is not kept past a whole read.
    server.restart();
    wait(cx, "the agents read again after the host came back", |cx| read(cx).is_some_and(|now| !Rc::ptr_eq(&now, &before)));
    let none = desktop_gpui::transcript::Can::default();
    wait(cx, "the view's flags again", |cx| view.read_with(cx, |v, _| v.loads() >= 2 && v.agent_can() != none));
}

#[gpui_kit::test]
fn the_git_split_says_what_the_folder_holds_now_and_not_the_hosts_last_read(cx: &mut TestAppContext) {
    // git is read through the copy's daemon.
    if let Err(why) = common::wsp_daemon() {
        eprintln!("skipped: {why}");
        return;
    }
    let server = common::Throwaway::start_with(common::STAND_IN, true);
    let folder = server.repo("repo");
    let git = std::process::Command::new("git").arg("-C").arg(&folder).args(["checkout", "-q", "-b", "work"]).status().unwrap();
    assert!(git.success(), "git checkout -b work");
    server.wsp(&["add", &folder.to_string_lossy()]);
    server.wsp(&["run", "repo", "first", "--agent", "claude"]);
    // The host read the checkout as the turn ended; a file written since is in no read until one is asked for, and a
    // read is held ten seconds.
    std::fs::write(folder.join("notes.txt"), "written after the turn\n").unwrap();
    std::thread::sleep(Duration::from_secs(11));
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let statuses = runtime.block_on(async {
        let client = wsp_host_client::HostClient::connect(&server.state(), &server.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
        let ask: wsp_host_client::protocol::StatusSubscribeRequest = serde_json::from_value(json!({})).unwrap();
        client.call(ask).await.unwrap_or_else(|f| panic!("{}", f.message)).statuses
    });
    let (view, threads, host, cx) = following(cx, &server);
    follow(&view, &host, &threads, "first", cx);
    // What the thread screen hears as it connects: each workspace's status as the host holds it.
    for status in statuses {
        let frame = json!({ "type": "workspace.status", "status": status });
        view.update(cx, |v, cx| header::heard(v, "workspace.status", &frame, cx));
    }
    let git = git_of(cx);
    let label = |cx: &mut VisualTestContext| git.read_with(cx, |g, _| g.state().map(|s| quick_action(&s).label));
    wait(cx, "the split", |cx| label(cx).is_some());
    let until = Instant::now() + Duration::from_secs(10);
    while label(cx).as_deref() != Some("Commit, push and PR") && Instant::now() < until {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(label(cx).as_deref(), Some("Commit, push and PR"), "the split read the host's checkout from before the write");
}

#[gpui_kit::test]
fn a_refused_git_act_says_the_hosts_sentence_beside_the_workspace_in_the_windows_toasts(cx: &mut TestAppContext) {
    // The split stands on the checkout, which git gives through the copy's daemon.
    if let Err(why) = common::wsp_daemon() {
        eprintln!("skipped: {why}");
        return;
    }
    let server = common::Throwaway::start_with(common::STAND_IN, true);
    let folder = server.repo("repo");
    let git = std::process::Command::new("git").arg("-C").arg(&folder).args(["checkout", "-q", "-b", "work"]).status().unwrap();
    assert!(git.success(), "git checkout -b work");
    server.wsp(&["add", &folder.to_string_lossy()]);
    server.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    // The split stands on the checkout, which the host first reads as the turn ends; a status from before has none.
    let statuses = runtime.block_on(async {
        let client = wsp_host_client::HostClient::connect(&server.state(), &server.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            let ask: wsp_host_client::protocol::StatusSubscribeRequest = serde_json::from_value(json!({})).unwrap();
            let statuses = client.call(ask).await.unwrap_or_else(|f| panic!("{}", f.message)).statuses;
            let read = statuses.iter().map(|s| serde_json::to_value(s).unwrap()).all(|s| !s["checkout"].is_null());
            if read && !statuses.is_empty() {
                break statuses;
            }
            assert!(Instant::now() < until, "the host read no checkout for the workspace in 30 s");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let (view, threads, host, cx) = following(cx, &server);
    let (workspace, _) = follow(&view, &host, &threads, "first", cx);
    let mut name = String::new();
    for status in statuses {
        let status = serde_json::to_value(status).unwrap();
        if status["id"] == workspace.as_str() {
            name = status["name"].as_str().unwrap_or_default().to_owned();
        }
        let frame = json!({ "type": "workspace.status", "status": status });
        view.update(cx, |v, cx| header::heard(v, "workspace.status", &frame, cx));
    }
    let git = git_of(cx);
    let act = |cx: &mut VisualTestContext| git.read_with(cx, |g, _| g.state().and_then(|s| quick_action(&s).act));
    // A folder with no remote: bring back has nowhere to push, and the host says why.
    wait(cx, "a git act to press", |cx| act(cx) == Some(GitAct::CreatePr));
    let toasts = cx.update(|window, cx| desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts"));
    let standing =
        |cx: &mut VisualTestContext| toasts.read_with(cx, |t, _| t.standing().iter().map(|n| n.text.to_string()).collect::<Vec<_>>());
    // A toast enters over the header row on the clock and takes presses there until it stands under it.
    toasts.update(cx, |_, cx| cx.notify());
    for _ in 0..30 {
        cx.executor().advance_clock(Duration::from_millis(20));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    let refused = |cx: &mut VisualTestContext| {
        toasts.read_with(cx, |t, _| {
            t.standing().into_iter().find(|n| n.kind == desktop_gpui::ui::NoticeKind::Error && !n.text.starts_with("Live readings for "))
        })
    };
    assert!(refused(cx).is_none(), "a refusal stood before the press: {:?}", standing(cx));
    // The host can answer before the click's own wait ends, so the act may already be over; a press that lands while
    // a status push redraws the split is pressed again.
    let mut presses = 0;
    while !(git.read_with(cx, |g, _| g.busy()) || refused(cx).is_some()) {
        assert!(presses < 3, "three presses started no act; the toasts standing: {:?}", standing(cx));
        let quick = bounds(cx, "git-quick").center();
        cx.simulate_click(quick, Modifiers::none());
        presses += 1;
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until && !(git.read_with(cx, |g, _| g.busy()) || refused(cx).is_some()) {
            std::thread::sleep(Duration::from_millis(20));
            cx.run_until_parked();
        }
    }
    let until = Instant::now() + Duration::from_secs(40);
    while refused(cx).is_none() {
        assert!(
            Instant::now() < until,
            "no refusal in the toasts after 40 s; busy {}, standing {:?}",
            git.read_with(cx, |g, _| g.busy()),
            standing(cx)
        );
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
    let notice = refused(cx).unwrap();
    assert!(!notice.text.is_empty(), "the refusal said nothing");
    assert_eq!(notice.place.as_ref().map(|p| p.to_string()), Some(name), "the refusal does not name the workspace");
    assert!(cx.debug_bounds("thread-notice").is_none(), "the header drew a notice of its own");
}
