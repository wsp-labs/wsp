// SPDX-License-Identifier: AGPL-3.0-only
//! A slate's sheet and the dialogs' layer in one window, as the shell draws both: an ask that arrives while a dialog
//! stands, against a host started for this binary alone.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::layer::{self, Closes};
use desktop_gpui::screens::slate::consent::{self, Open};
use desktop_gpui::screens::slate::{Raise, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use wsp_host_client::HostClient;

/// The panel with the dialogs' layer drawn last over it, as the shell's root draws it.
struct Stage(Entity<SlateScreen>);

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone()).children(layer::draw(window, cx))
    }
}

/// A 200 by 120 dialog with nothing in it.
struct Probe;

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().debug_selector(|| "probe".into()).w(px(200.)).h(px(120.))
    }
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

/// The panel on a thread whose slate asks to run a command, with the dialogs' layer over it.
fn stage<'a>(cx: &'a mut TestAppContext, name: &'static str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    // One project for the binary: the two tests run at once on the shared host, and a second would make the same repo.
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let shared = common::shared();
    let folder = FOLDER.get_or_init(|| fixture::project(&shared.host));
    let thread = fixture::staged(&shared.host, folder, &fixture::State { name, file: Some("runs.slate") });
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(HostClient::connect(&shared.host.state(), &shared.host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let handle = Host::new(Arc::new(client), runtime);
    let opened = Opened { thread_id: Some(thread), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        let stage = cx.new(|_| Stage(screen));
        Root::new(stage, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let screen = made.unwrap();
    until(cx, &screen, "the slate drawn", |s| s.slate().is_some_and(|sl| !sl.resolved.is_empty() || sl.formulas().is_empty()));
    (screen, cx)
}

fn settle(cx: &mut VisualTestContext) {
    // Past the layer's 200 ms moves.
    std::thread::sleep(Duration::from_millis(250));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Catches: an ask that arrives while a dialog stands opening its sheet under the dialog's scrim, where it takes the
/// keys and no press reaches it, so a press on its Not now closes the dialog instead.
#[gpui_kit::test]
fn an_ask_that_arrives_over_a_dialog_stands_on_top_of_it(cx: &mut TestAppContext) {
    let (screen, cx) = stage(cx, "over-dialog");
    cx.update(|window, cx| layer::open("probe", cx.new(|_| Probe).into(), Closes::Outside, None, window, cx));
    cx.run_until_parked();
    cx.update(|_, cx| screen.update(cx, |s, cx| s.raise("go-quick".to_owned(), "press", Raise::default(), cx)));
    until(cx, &screen, "the ask's sheet", |s| matches!(consent::open(s), Some(Open::One(_))));
    settle(cx);
    let not_now = cx.debug_bounds("dialog-act-0").expect("the sheet's Not now is drawn");
    cx.simulate_click(not_now.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), Some("probe"), "the press reached the sheet, not the scrim under it");
    until(cx, &screen, "Not now answered the ask", |s| consent::open(s).is_none());
}

/// Catches: a dialog opened while a sheet stands drawing under the sheet, which keeps the presses the dialog should
/// take; the newer stands on top either way.
#[gpui_kit::test]
fn a_dialog_opened_over_an_ask_stands_on_top_of_it(cx: &mut TestAppContext) {
    let (screen, cx) = stage(cx, "under-dialog");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.raise("go-quick".to_owned(), "press", Raise::default(), cx)));
    until(cx, &screen, "the ask's sheet", |s| matches!(consent::open(s), Some(Open::One(_))));
    settle(cx);
    let not_now = cx.debug_bounds("dialog-act-0").expect("the sheet's Not now is drawn");
    cx.update(|window, cx| layer::open("probe", cx.new(|_| Probe).into(), Closes::Outside, None, window, cx));
    settle(cx);
    cx.simulate_click(not_now.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| layer::top(cx)), None, "the press reached the dialog's scrim, not the sheet under it");
    assert!(cx.read(|cx| matches!(consent::open(screen.read(cx)), Some(Open::One(_)))), "the ask still waits");
}
