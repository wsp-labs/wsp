// SPDX-License-Identifier: AGPL-3.0-only
//! The slate panel's frame against a host started for this binary alone: a run a press starts, its held row and its
//! sheet, Run once and Not now, Cancel and Stop runs, the menu's Clear and Undo, and the host's refusal under the head.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::consent::{self, Open};
use desktop_gpui::screens::slate::{Load, Raise, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Entity, Modifiers, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

/// A run long enough to be seen running, with an output and a button that starts it.
const NAP: &str = r#"<slate title="Naps">
  <run name="nap" cmd="sleep 2; echo rested" stream timeout={30} />
  <run name="long" cmd="sleep 30" timeout={60} />
  <column>
    <button id="go-nap" label="Nap" onPress={start($nap)} />
    <button id="go-long" label="Long" onPress={start($long)} />
    <output id="nap-out" run={$nap} />
    <output id="long-out" run={$long} />
  </column>
</slate>"#;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn thread_with(file: Option<&'static str>, text: Option<&str>) -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "panel", file });
    if let Some(text) = text {
        fixture::write(host, &thread, text);
    }
    thread
}

/// The panel on `thread` at the right panel's size, inside gpui-kit's Root as the app wraps every window.
fn open<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let screen = made.unwrap();
    until(cx, &screen, "the slate drawn", |s| s.slate().is_some_and(|sl| !sl.resolved.is_empty() || sl.formulas().is_empty()));
    (screen, cx)
}

fn until(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, what: &str, done: impl Fn(&SlateScreen) -> bool) {
    let limit = Instant::now() + Duration::from_secs(60);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

fn drawn(cx: &mut VisualTestContext, selector: &str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(leak(selector.to_owned())).is_some()
}

/// A press at the centre of what a debug selector names, through the window's own hit test.
fn click(cx: &mut VisualTestContext, selector: &str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let at = cx.debug_bounds(leak(selector.to_owned())).unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
}

/// The open sheet's answer, as its buttons give it.
fn answer(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, scope: &'static str) {
    cx.update(|_, cx| screen.update(cx, |s, cx| s.decide(scope, cx)));
}

fn press(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, piece: &'static str) {
    cx.update(|_, cx| screen.update(cx, |s, cx| s.raise(piece.to_owned(), "press", Raise::default(), cx)));
    until(cx, screen, "the press answered", |s| s.slate().is_some_and(|sl| !sl.busy.contains(piece)));
}

fn state(screen: &SlateScreen, run: &str) -> Option<String> {
    screen.record().and_then(|r| r.run_state(run)).map(str::to_owned)
}

fn sheet_for(screen: &SlateScreen) -> Option<String> {
    match consent::open(screen) {
        Some(Open::One(ask)) => Some(ask.run),
        Some(other) => Some(format!("{other:?}")),
        None => None,
    }
}

#[gpui_kit::test]
fn a_held_press_asks_and_run_once_runs_it_live_through_running_to_done(cx: &mut TestAppContext) {
    let thread = thread_with(None, Some(NAP));
    let (screen, cx) = open(cx, &thread);
    assert!(drawn(cx, "slate-run-nap-idle"), "the output says Not run yet");
    press(cx, &screen, "go-nap");
    until(cx, &screen, "the nap held", |s| state(s, "nap").as_deref() == Some("held"));
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("nap"), "the press's ask opens its sheet");
    assert!(drawn(cx, "slate-held-nap"), "the held row stands under the head");
    assert!(drawn(cx, "slate-consent-nap"), "the sheet shows the command");
    answer(cx, &screen, "once");
    until(cx, &screen, "the nap running", |s| state(s, "nap").as_deref() == Some("running"));
    assert!(drawn(cx, "slate-run-nap-running"), "the slate.values push draws Running");
    assert!(!drawn(cx, "slate-held-nap"), "no held row once it runs");
    assert!(drawn(cx, "crab"), "a crab walks before Running");
    until(cx, &screen, "the nap done", |s| state(s, "nap").as_deref() == Some("done"));
    assert!(drawn(cx, "slate-run-nap-done"), "Done, drawn from the push");
    assert!(drawn(cx, "slate-log"), "its lines stand under it");
    let lines = cx.read(|cx| screen.read(cx).slate().and_then(|s| s.lines.get("nap").cloned()).unwrap_or_default());
    assert_eq!(lines, ["rested"], "the streamed line, once");
}

#[gpui_kit::test]
fn escape_closes_the_sheet_and_leaves_the_held_row_and_not_now_refuses(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    press(cx, &screen, "go-quick");
    until(cx, &screen, "the sheet open", |s| sheet_for(s).as_deref() == Some("quick"));
    until(cx, &screen, "its held row", |s| s.record().is_some_and(|r| r.asks.iter().any(|a| a.run == "quick")));
    assert!(drawn(cx, "slate-sheet"), "the sheet's layer is drawn");
    cx.simulate_keystrokes("escape");
    until(cx, &screen, "the sheet closed", |s| consent::open(s).is_none());
    assert!(!drawn(cx, "slate-sheet"), "Escape closed the sheet");
    assert!(drawn(cx, "slate-held-quick"), "the held row stays after Escape (W2-SLT-070)");
    assert_eq!(cx.read(|cx| state(screen.read(cx), "quick")).as_deref(), Some("held"));
    click(cx, "slate-refuse-quick");
    until(cx, &screen, "the run refused", |s| state(s, "quick").as_deref() != Some("held"));
    assert!(!drawn(cx, "slate-held-quick"), "Not now took the held row away (W2-SLT-065)");
    assert!(cx.read(|cx| consent::open(screen.read(cx)).is_none()), "and opened no sheet");
}

#[gpui_kit::test]
fn tab_keeps_the_keys_in_the_sheet(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    press(cx, &screen, "go-quick");
    until(cx, &screen, "the sheet open", |s| sheet_for(s).as_deref() == Some("quick"));
    assert!(drawn(cx, "slate-sheet"), "the sheet's layer is drawn");
    // Past its last button Tab comes round to its first, never to a piece under the scrim.
    for _ in 0..4 {
        cx.simulate_keystrokes("tab");
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.read(|cx| consent::open(screen.read(cx))).is_none(), "Escape still reaches the sheet after four Tabs");
}

#[gpui_kit::test]
fn the_sheet_occludes_what_is_under_it(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    press(cx, &screen, "go-danger");
    until(cx, &screen, "the confirm sheet", |s| sheet_for(s).as_deref() == Some("danger"));
    until(cx, &screen, "its held row", |s| s.record().is_some_and(|r| r.asks.iter().any(|a| a.run == "danger")));
    // The row's Not now lies under the alert's scrim, which does not close on a press.
    click(cx, "slate-refuse-danger");
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| state(screen.read(cx), "danger")).as_deref(), Some("held"), "a press under the sheet reaches nothing");
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("danger"), "the alert stays open");
}

/// One timer the person has not allowed: its sheet opens on its own once the slate draws.
const TICK: &str = r#"<slate title="Tick">
  <run name="tick" cmd="echo tick" every={10} timeout={10} />
  <column>
    <output id="tick-out" run={$tick} />
  </column>
</slate>"#;

/// A text field where the person types, beside the panel cached as the right panel caches it.
struct Beside {
    field: Entity<gpui_kit::component::input::TextareaState>,
    slate: Entity<SlateScreen>,
}

impl Render for Beside {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut Context<Self>) -> impl IntoElement {
        let slate = gpui_kit::AnyView::from(self.slate.clone()).cached(gpui_kit::StyleRefinement::default().size_full());
        gpui_kit::div()
            .flex()
            .size_full()
            .child(gpui_kit::div().w(px(200.)).child(gpui_kit::component::input::Textarea::new(&self.field)))
            .child(gpui_kit::div().flex_1().h_full().child(slate))
    }
}

/// The panel on `thread` beside a field that has the keys first where `typing`, and a way to build the panel again.
fn open_beside<'a>(
    cx: &'a mut TestAppContext,
    thread: &str,
    typing: bool,
) -> (Entity<Beside>, impl Fn(&mut VisualTestContext), &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        use gpui_kit::Focusable as _;
        let field = cx.new(|cx| gpui_kit::component::input::TextareaState::new(window, cx));
        if typing {
            let keys = field.read(cx).focus_handle(cx);
            keys.focus(window, cx);
        }
        let slate = cx.new(|cx| SlateScreen::new(handle.clone(), opened.clone(), window, cx));
        let beside = cx.new(|_| Beside { field, slate });
        made = Some(beside.clone());
        Root::new(beside, window, cx)
    });
    cx.simulate_resize(size(px(740.), px(748.)));
    let beside = made.unwrap();
    let target = beside.clone();
    let again = move |cx: &mut VisualTestContext| {
        target.update_in(cx, |b, window, cx| {
            b.slate = cx.new(|cx| SlateScreen::new(handle.clone(), opened.clone(), window, cx));
            cx.notify();
        })
    };
    (beside, again, cx)
}

fn slate_of(cx: &mut VisualTestContext, beside: &Entity<Beside>) -> Entity<SlateScreen> {
    cx.read(|cx| beside.read(cx).slate.clone())
}

/// Keys as the platform sends them, down then up: a focused button takes Space or Enter as a press on the up.
fn keys(cx: &mut VisualTestContext, keys: &[&str]) {
    for key in keys {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui_kit::KeyUpEvent { keystroke: gpui_kit::Keystroke::parse(key).unwrap() });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn no_sheet_opens_on_its_own_while_the_person_types_outside_the_slate(cx: &mut TestAppContext) {
    let thread = thread_with(None, Some(TICK));
    let (beside, _, cx) = open_beside(cx, &thread, true);
    let screen = slate_of(cx, &beside);
    until(cx, &screen, "the timer held with its ask", |s| s.record().is_some_and(|r| r.asks.iter().any(|a| a.run == "tick")));
    // Keys a composer takes, which reach a sheet's buttons once a sheet has them.
    cx.simulate_input("ok");
    keys(cx, &["tab", "tab", "space"]);
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    let approvals = cx.read(|cx| screen.read(cx).record().map(|r| r.approvals.clone()).unwrap_or_default());
    assert!(approvals.is_empty(), "keys typed outside the slate answered nothing: {approvals:?}");
    assert!(cx.read(|cx| consent::open(screen.read(cx))).is_none(), "no sheet opens while the person types elsewhere");
    assert!(drawn(cx, "slate-review-tick"), "the held row offers Review meanwhile");
    let typed = cx.read(|cx| beside.read(cx).field.read(cx).value().to_string());
    assert!(typed.starts_with("ok") && typed.ends_with(' '), "the keys stay in the field: {typed:?}");
    cx.update(|window, cx| window.blur(cx));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("tick"), "the sheet opens once the person leaves the field");
    cx.simulate_keystrokes("escape");
    until(cx, &screen, "the sheet closed", |s| consent::open(s).is_none());
    click(cx, "slate-review-tick");
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("tick"), "Review opens it again");
}

#[gpui_kit::test]
fn a_sheet_closed_does_not_open_on_its_own_again_when_the_panel_is_built_again(cx: &mut TestAppContext) {
    let thread = thread_with(None, Some(TICK));
    let (beside, again, cx) = open_beside(cx, &thread, false);
    let screen = slate_of(cx, &beside);
    until(cx, &screen, "the sheet open on its own", |s| sheet_for(s).as_deref() == Some("tick"));
    cx.simulate_keystrokes("escape");
    until(cx, &screen, "the sheet closed", |s| consent::open(s).is_none());
    again(cx);
    let screen = slate_of(cx, &beside);
    until(cx, &screen, "the record read again", |s| s.record().is_some_and(|r| r.asks.iter().any(|a| a.run == "tick")));
    assert!(drawn(cx, "slate-held-tick"), "the held row stands");
    assert!(cx.read(|cx| consent::open(screen.read(cx))).is_none(), "a sheet shown once on this thread waits on Review");
}

#[gpui_kit::test]
fn cancel_and_stop_runs_cancel(cx: &mut TestAppContext) {
    let thread = thread_with(None, Some(NAP));
    let (screen, cx) = open(cx, &thread);
    for (piece, run) in [("go-long", "long"), ("go-nap", "nap")] {
        press(cx, &screen, piece);
        until(cx, &screen, "a sheet", |s| sheet_for(s).as_deref() == Some(run));
        answer(cx, &screen, "once");
        until(cx, &screen, "running", |s| state(s, run).as_deref() == Some("running"));
    }
    click(cx, "slate-cancel-long");
    until(cx, &screen, "long cancelled", |s| state(s, "long").as_deref() == Some("cancelled"));
    assert!(drawn(cx, "slate-run-long-cancelled"), "Cancel's run says Cancelled");
    press(cx, &screen, "go-long");
    until(cx, &screen, "a sheet", |s| sheet_for(s).as_deref() == Some("long"));
    answer(cx, &screen, "once");
    until(cx, &screen, "running again", |s| state(s, "long").as_deref() == Some("running"));
    cx.update(|_, cx| screen.update(cx, |s, cx| s.stop_runs(cx)));
    until(cx, &screen, "long stopped", |s| state(s, "long").as_deref() == Some("cancelled"));
    assert!(drawn(cx, "slate-run-long-cancelled"), "Stop runs says Cancelled");
}

/// A run a reaction starts when the person writes a value, which no press answers with its ask.
const REACTS: &str = r#"<slate title="Reacts">
  <value name="name" start="" />
  <run name="greet" cmd="echo hi" timeout={10} />
  <when change={$name} do={start($greet)} />
  <column>
    <input id="name" label="Name" value={$name} />
    <output id="greet-out" run={$greet} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_run_held_by_a_push_brings_its_ask_and_its_sheet(cx: &mut TestAppContext) {
    let thread = thread_with(None, Some(REACTS));
    let (screen, cx) = open(cx, &thread);
    cx.update(|_, cx| screen.update(cx, |s, cx| s.write("$name".into(), serde_json::json!("Ada"), cx)));
    until(cx, &screen, "the reaction's run held", |s| state(s, "greet").as_deref() == Some("held"));
    until(cx, &screen, "its ask read", |s| s.record().is_some_and(|r| r.asks.iter().any(|a| a.run == "greet")));
    assert!(drawn(cx, "slate-review-greet"), "the held row offers Review");
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("greet"), "its sheet opens on its own");
}

#[gpui_kit::test]
fn the_menu_clears_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    click(cx, "slate-menu");
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    until(cx, &screen, "cleared", |s| matches!(&s.load, Load::Empty(r) if r.empty.as_deref() == Some("cleared")));
    assert!(drawn(cx, "slate-empty-cleared"), "the empty state says Cleared");
    click(cx, "slate-empty-undo");
    until(cx, &screen, "restored", |s| s.slate().is_some());
    assert!(drawn(cx, "slate-header"), "Undo drew the slate again");
}

#[gpui_kit::test]
fn a_menu_act_the_host_refuses_draws_its_words_under_the_head(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    assert!(!drawn(cx, "slate-refused"));
    // The first version has nothing to undo: the host refuses, as it would a stale menu.
    cx.update(|_, cx| screen.update(cx, |s, cx| s.menu_write("<undo />", cx)));
    until(cx, &screen, "refused", |s| s.refused.is_some());
    assert!(drawn(cx, "slate-refused"), "the refusal line under the head");
    let said = cx.read(|cx| screen.read(cx).refused.clone().unwrap_or_default());
    assert!(!said.is_empty(), "in the host's words: {said}");
}

#[gpui_kit::test]
fn a_click_inside_a_sheet_keeps_it_open_and_one_on_its_scrim_closes_it(cx: &mut TestAppContext) {
    let thread = thread_with(Some("runs.slate"), None);
    let (screen, cx) = open(cx, &thread);
    press(cx, &screen, "go-quick");
    until(cx, &screen, "the sheet open", |s| sheet_for(s).as_deref() == Some("quick"));
    click(cx, "slate-consent-quick");
    assert_eq!(cx.read(|cx| sheet_for(screen.read(cx))).as_deref(), Some("quick"), "a click inside keeps it open");
    cx.simulate_click(gpui_kit::point(px(4.), px(4.)), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.read(|cx| consent::open(screen.read(cx))).is_none(), "a click on the scrim closes a plain sheet");
}
