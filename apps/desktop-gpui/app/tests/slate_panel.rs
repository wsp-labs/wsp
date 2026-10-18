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
