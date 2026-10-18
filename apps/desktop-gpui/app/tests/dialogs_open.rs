// SPDX-License-Identifier: AGPL-3.0-only
//! The dialogs' layer as the shell draws it, and the sidebar's asks reaching each flow: the app's own window, the shell
//! inside gpui-kit's Root on a throwaway host.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::dialogs::{self, layer, layer::Closes};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::Asked;
use gpui_kit::component::Root;
use gpui_kit::{div, point, prelude::*, px, size, AppContext, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use wsp_host_client::Env;

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let runtime = runtime();
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (shell.unwrap(), cx)
}

/// The shell on a home of its own that no host serves; the folder goes when the returned guard drops.
fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    (shell, cx, dir)
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A 200 by 120 surface with nothing in it.
struct Probe;

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().debug_selector(|| "probe".into()).w(px(200.)).h(px(120.))
    }
}

fn stand(key: &'static str, closes: Closes, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let view = cx.new(|_| Probe).into();
        layer::open(key, view, closes, None, window, cx);
    });
    cx.run_until_parked();
}

fn top(cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.update(|_, cx| layer::top(cx))
}

#[gpui_kit::test]
fn a_step_dialog_stays_on_a_press_inside_and_closes_on_the_scrim_or_escape(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand("probe", Closes::Outside, cx);
    let probe = cx.debug_bounds("probe").expect("the layer draws the dialog");
    assert_eq!(probe.center(), point(px(640.), px(400.)), "centred in the window");
    cx.simulate_click(probe.center(), Modifiers::none());
    assert_eq!(top(cx), Some("probe"), "a press inside keeps it open");
    cx.simulate_click(point(px(40.), px(400.)), Modifiers::none());
    assert_eq!(top(cx), None, "a press on the scrim closes it");

    stand("probe", Closes::Outside, cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None, "Escape closes it");
}

#[gpui_kit::test]
fn an_alert_ignores_the_scrim_and_closes_on_escape(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand("alert", Closes::Escape, cx);
    cx.simulate_click(point(px(40.), px(400.)), Modifiers::none());
    assert_eq!(top(cx), Some("alert"), "a press outside is no answer");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None);
}

#[gpui_kit::test]
fn the_layer_keeps_the_pointer_from_the_sidebar_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    stand("probe", Closes::Escape, cx);
    // The sidebar's trigger sits at the frame row's left; under the layer a press there must not reach it.
    let trigger = cx.debug_bounds("sidebar-column").expect("the sidebar is drawn").origin + point(px(30.), px(26.));
    cx.simulate_click(trigger, Modifiers::none());
    assert!(shell.read_with(cx, |s, _| s.sidebar_open()), "the press stopped at the layer");
    assert_eq!(top(cx), Some("probe"));
}

#[gpui_kit::test]
fn each_sidebar_ask_opens_its_flow(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    for (asked, key) in [
        (Asked::AddComputer, dialogs::ADD_COMPUTER),
        (Asked::Setup("setup-1".into()), dialogs::ADD_COMPUTER),
        (Asked::AddCloud, dialogs::ADD_CLOUD),
        (Asked::AddProject, dialogs::ADD_PROJECT),
        (Asked::ExportProject("workspace-1".into()), dialogs::EXPORT_PROJECT),
    ] {
        let said = format!("{asked:?}");
        sidebar.update(cx, |_, cx| cx.emit(asked));
        until(cx, &said, |cx| top(cx) == Some(key));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(top(cx), None, "Escape closes what {said} opened");
    }
}
