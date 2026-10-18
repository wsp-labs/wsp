// SPDX-License-Identifier: AGPL-3.0-only
//! Each sheet's own buttons clicked through the window's hit test against a host started for this binary alone, and
//! the answer the host kept read back: Run once runs and keeps nothing, Always keeps the key allowed, Not now keeps it
//! refused, a link's Open once opens and Always keeps its domain, a batch's Allow all and Not now answer every row,
//! Revoke takes an approval back, and Forget secrets empties the secret where Cancel leaves it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::consent::{self, Open, Standing};
use desktop_gpui::screens::slate::{model, Raise, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, AlertDialog, Button, Side, Still};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::Value;
use wsp_host_client::HostClient;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn thread_on(file: &'static str) -> String {
    let (host, folder) = bench();
    fixture::staged(host, folder, &fixture::State { name: "sheets", file: Some(file) })
}

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

/// A click at the centre of what a debug selector names, through the window's own hit test.
fn click(cx: &mut VisualTestContext, selector: &str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let at = cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())).unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
}

/// The foot's `n`th act from the left.
fn act(cx: &mut VisualTestContext, n: usize) {
    click(cx, &format!("dialog-act-{n}"));
}

fn press(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, piece: &'static str) {
    cx.update(|_, cx| screen.update(cx, |s, cx| s.raise(piece.to_owned(), "press", Raise::default(), cx)));
    until(cx, screen, "the press answered", |s| s.slate().is_some_and(|sl| !sl.busy.contains(piece)));
}

/// The one ask whose sheet is open.
fn asking(screen: &SlateScreen) -> Option<model::Ask> {
    match consent::open(screen) {
        Some(Open::One(ask)) => Some(*ask),
        _ => None,
    }
}

fn kept(screen: &SlateScreen, key: &str) -> Option<String> {
    screen.record()?.approvals.get(key)?.get("state").and_then(Value::as_str).map(str::to_owned)
}

fn state(screen: &SlateScreen, run: &str) -> Option<String> {
    screen.record().and_then(|r| r.run_state(run)).map(str::to_owned)
}

/// Presses `piece` and waits for the sheet of `run` it holds.
fn held(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, piece: &'static str, run: &str) -> model::Ask {
    press(cx, screen, piece);
    until(cx, screen, &format!("the sheet of {run}"), |s| asking(s).is_some_and(|a| a.run == run));
    cx.read(|cx| asking(screen.read(cx))).unwrap()
}

#[gpui_kit::test]
fn the_run_sheets_own_buttons_send_their_own_scope(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);

    let ask = held(cx, &screen, "go-quick", "quick");
    let always = ask.no_always != Some(true);
    act(cx, if always { 2 } else { 1 });
    until(cx, &screen, "Run once ran quick", |s| state(s, "quick").as_deref() == Some("done"));
    assert_eq!(cx.read(|cx| kept(screen.read(cx), &ask.key)), None, "Run once keeps nothing for the thread");

    let ask = held(cx, &screen, "go-broken", "broken");
    act(cx, 0);
    until(cx, &screen, "Not now kept broken refused", |s| kept(s, &ask.key).as_deref() == Some("refused"));

    let ask = held(cx, &screen, "go-slow", "slow");
    assert_ne!(ask.no_always, Some(true), "the fixture's slow is a command the host pins, so its sheet draws Always");
    act(cx, 1);
    until(cx, &screen, "Always kept slow allowed", |s| kept(s, &ask.key).as_deref() == Some("allowed"));

    menu(cx, &screen, "up up enter");
    until(cx, &screen, "the menu's Approvals open", |s| s.consent.standing.is_some());
    click(cx, &format!("slate-revoke-{}", ask.key));
    until(cx, &screen, "Revoke took slow's approval back", |s| kept(s, &ask.key).is_none());
}

#[gpui_kit::test]
fn the_confirm_alerts_run_runs_once(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    let ask = held(cx, &screen, "go-danger", "danger");
    act(cx, 1);
    until(cx, &screen, "Run ran danger", |s| state(s, "danger").as_deref() == Some("done"));
    assert_eq!(cx.read(|cx| kept(screen.read(cx), &ask.key)), None, "a confirmed run keeps nothing for the thread");
}

#[gpui_kit::test]
fn the_link_sheets_buttons_open_once_or_keep_the_domain(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    let linking = |cx: &mut VisualTestContext| cx.read(|cx| screen.read(cx).consent.linking.clone());

    press(cx, &screen, "good-link");
    assert_eq!(linking(cx).as_deref(), Some("https://example.com"), "the link waits on its sheet");
    act(cx, 0);
    assert_eq!(linking(cx), None, "Not now closes the sheet");
    assert_eq!(cx.opened_url(), None, "and opens nothing");

    press(cx, &screen, "good-link");
    act(cx, 2);
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com"), "Open once opens it");
    assert_eq!(cx.read(|cx| kept(screen.read(cx), "domain:example.com")), None, "and keeps nothing");

    press(cx, &screen, "good-link");
    act(cx, 1);
    until(cx, &screen, "Always kept example.com", |s| kept(s, "domain:example.com").as_deref() == Some("allowed"));
}

/// Opens the slate's head menu with a press and walks it with `keys`; Up from no pick starts at the last act.
fn menu(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, keys: &str) {
    until(cx, screen, "no sheet open", |s| consent::open(s).is_none() && s.consent.standing.is_none() && s.consent.forgetting.is_none());
    click(cx, "slate-menu");
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

/// The batch timers.slate opens on its own, its two keys.
fn batch(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>) -> Vec<String> {
    until(cx, screen, "the batch open", |s| matches!(consent::open(s), Some(Open::Batch(_))));
    match cx.read(|cx| consent::open(screen.read(cx))) {
        Some(Open::Batch(asks)) => asks.into_iter().map(|a| a.key).collect(),
        _ => unreachable!(),
    }
}

#[gpui_kit::test]
fn the_batchs_allow_all_keeps_every_row_allowed(cx: &mut TestAppContext) {
    let thread = thread_on("timers.slate");
    let (screen, cx) = open(cx, &thread);
    let keys = batch(cx, &screen);
    act(cx, 1);
    until(cx, &screen, "Allow all kept both allowed", |s| keys.iter().all(|k| kept(s, k).as_deref() == Some("allowed")));
}

#[gpui_kit::test]
fn a_row_switched_off_is_left_out_of_allow(cx: &mut TestAppContext) {
    let thread = thread_on("timers.slate");
    let (screen, cx) = open(cx, &thread);
    let keys = batch(cx, &screen);
    click(cx, &format!("slate-batch-switch-{}", keys[0]));
    until(cx, &screen, "the first row switched off", |s| s.consent.batch.as_ref().is_some_and(|b| !b.on.contains(&keys[0])));
    act(cx, 1);
    until(cx, &screen, "Allow 1 kept the second row allowed", |s| kept(s, &keys[1]).as_deref() == Some("allowed"));
    assert_eq!(cx.read(|cx| kept(screen.read(cx), &keys[0])), None, "the row switched off is not answered");
}

#[gpui_kit::test]
fn the_batchs_not_now_keeps_every_row_refused(cx: &mut TestAppContext) {
    let thread = thread_on("timers.slate");
    let (screen, cx) = open(cx, &thread);
    let keys = batch(cx, &screen);
    act(cx, 0);
    until(cx, &screen, "Not now kept both refused", |s| keys.iter().all(|k| kept(s, k).as_deref() == Some("refused")));
}

#[gpui_kit::test]
fn forget_secrets_empties_the_secret_and_cancel_leaves_it(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    let token = |s: &SlateScreen| s.record().and_then(|r| r.values.get("token")?.get("set")?.as_bool()) == Some(true);
    cx.update(|_, cx| screen.update(cx, |s, cx| s.secret("$token".into(), "hunter2".into(), cx).detach()));
    until(cx, &screen, "the secret kept", token);
    // Undo, Clear, Stop runs, Forget secrets, Approvals, Copy: Forget is the third from the end.
    let forgetting = |cx: &mut VisualTestContext| {
        menu(cx, &screen, "up up up enter");
        until(cx, &screen, "the menu's Forget secrets open", |s| s.consent.forgetting.is_some());
    };

    forgetting(cx);
    act(cx, 0);
    assert!(cx.read(|cx| screen.read(cx).consent.forgetting.is_none()), "Cancel closes the alert");
    std::thread::sleep(Duration::from_millis(200));
    cx.run_until_parked();
    assert!(cx.read(|cx| token(screen.read(cx))), "and the secret stays");

    forgetting(cx);
    act(cx, 1);
    until(cx, &screen, "Forget secrets emptied the secret and closed", |s| s.consent.forgetting.is_none() && !token(s));
}

/// The open popup, a plain dialog or an alert.
fn popup(cx: &mut VisualTestContext) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("dialog").or_else(|| cx.debug_bounds("alert-dialog")).expect("a popup is drawn")
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"))
}

#[gpui_kit::test]
fn a_sheets_body_starts_where_the_webs_does(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    press(cx, &screen, "good-link");
    let (sheet, title, href) = (popup(cx), bounds(cx, "dialog-title"), bounds(cx, "slate-link-href"));
    assert_eq!(sheet.size.width, px(440.), "a sheet is `max-w-110`");
    assert_eq!(href.top() - title.bottom(), px(8.), "the header's `pb-1` and the panel's `pt-1` between the title and the body");
}

#[gpui_kit::test]
fn the_run_sheet_is_440_px_with_8_px_from_title_to_body(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    held(cx, &screen, "go-quick", "quick");
    let (sheet, title, body) = (popup(cx), bounds(cx, "dialog-title"), bounds(cx, "slate-consent-quick"));
    assert_eq!(sheet.size.width, px(440.), "the run sheet is `max-w-110`");
    assert_eq!(body.top() - title.bottom(), px(8.), "the run sheet's body sits 8 px under its title");
}

#[gpui_kit::test]
fn the_batch_sheet_is_440_px_with_8_px_from_title_to_body(cx: &mut TestAppContext) {
    let thread = thread_on("timers.slate");
    let (screen, cx) = open(cx, &thread);
    batch(cx, &screen);
    let (sheet, title, body) = (popup(cx), bounds(cx, "dialog-title"), bounds(cx, "slate-approvals"));
    assert_eq!(sheet.size.width, px(440.), "the batch sheet is `max-w-110`");
    assert_eq!(body.top() - title.bottom(), px(8.), "the batch sheet's body sits 8 px under its title");
}

#[gpui_kit::test]
fn the_approvals_dialog_ends_at_its_panel_with_no_foot(cx: &mut TestAppContext) {
    let thread = thread_on("runs.slate");
    let (screen, cx) = open(cx, &thread);
    cx.update(|_, cx| {
        screen.update(cx, |s, cx| {
            s.consent.standing = Some(Standing::default());
            cx.notify();
        })
    });
    let (sheet, body) = (popup(cx), bounds(cx, "slate-standing"));
    assert_eq!(sheet.bottom() - body.bottom(), px(13.), "the panel's `pb-3` and the popup's edge, and no foot");
}

/// An alert drawn alone, as a sheet over the panel draws it.
struct Alone;

impl Render for Alone {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        AlertDialog::new("Forget this slate's secrets?").act(Button::outline("cancel", "Cancel"))
    }
}

/// The frames one drawn frame asks for, after what earlier frames asked for is spent.
fn asked(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx)
    })
}

fn alone(cx: &mut TestAppContext, still: bool) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(still));
    });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(cx.new(|_| Alone), window, cx));
    cx.simulate_resize(size(px(540.), px(400.)));
    cx
}

#[gpui_kit::test]
fn a_popup_fades_in_over_200_ms_then_asks_for_no_frame(cx: &mut TestAppContext) {
    let cx = alone(cx, false);
    assert!(asked(cx) > 0, "the popup fades in as it opens");
    std::thread::sleep(std::time::Duration::from_millis(250));
    asked(cx);
    assert_eq!(asked(cx), 0, "past its 200 ms the fade is done and asks for no frame");
}

#[gpui_kit::test]
fn under_reduced_motion_a_popup_stands_at_once(cx: &mut TestAppContext) {
    let cx = alone(cx, true);
    assert_eq!(asked(cx), 0, "with motion reduced the popup asks for no frame");
}
