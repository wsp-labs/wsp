// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal's drawer and panel tab in the app's own window: the shell inside gpui-kit's Root after
//! `gpui_kit::init` and wsp's themes, with the default key table bound as at launch. Where a shell is needed it runs on
//! a real wsp-daemon (scripts/stage-daemon.sh stages one) carried by a host started for each test alone, through this
//! computer's own channel; without a staged daemon those tests say so and skip.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys::{self, DEFAULTS};
use desktop_gpui::screens::sidebar::host::Change;
use desktop_gpui::screens::sidebar::{Asked, Sidebar};
use desktop_gpui::screens::terminal;
use desktop_gpui::screens::terminal::drawer::{Drawer, Mode};
use desktop_gpui::screens::terminal::groups::{Direction, Place, Ui};
use desktop_gpui::screens::terminal::hub::{self, Key};
use desktop_gpui::screens::terminal::menu;
use desktop_gpui::screens::terminal::state::PaneState;
use desktop_gpui::screens::terminal::toolbar::{self, Act};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell, SETTINGS};
use desktop_gpui::sidebar::ThreadItem;
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, AppContext, Entity, Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::protocol::{DaemonCloseRequest, DaemonOpenRequest, DaemonSendRequest};
use wsp_host_client::Env;

const MAC: bool = cfg!(target_os = "macos");
const NO_AGENT: &str = "#!/bin/sh\nexit 1\n";

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

/// The shell in a 1280 by 800 window, on `state` with `env`.
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

/// The shell on a home of its own with no host in it, which goes when the test does.
fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    (shell, cx, dir)
}

/// A host carrying the staged daemon, or why there is none.
fn with_daemon() -> Option<common::Throwaway> {
    if let Err(why) = common::wsp_daemon() {
        eprintln!("skipped: {why}");
        return None;
    }
    Some(common::Throwaway::start_with(NO_AGENT, true))
}

/// The shells this computer's daemon lists, asked on a channel of the test's own.
fn daemon_ptys(host: &common::Throwaway) -> Vec<String> {
    runtime().block_on(async {
        let client = host.client().await;
        let opened = client.call(common::ask::<DaemonOpenRequest>(json!({ "placeId": "here" }))).await.unwrap();
        let channel = serde_json::to_value(&opened).unwrap()["channel"].as_str().unwrap().to_owned();
        let sent =
            client.call(common::ask::<DaemonSendRequest>(json!({ "channel": channel, "frame": { "op": "pty.list" } }))).await.unwrap();
        _ = client.call(common::ask::<DaemonCloseRequest>(json!({ "channel": channel }))).await;
        sent.reply["ptys"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_str().map(str::to_owned)).collect()
    })
}

fn press(cx: &mut VisualTestContext, command: &str) {
    let rule = DEFAULTS.iter().find(|r| r.command == command).unwrap_or_else(|| panic!("no rule for {command}"));
    let typed = keys::keystroke(rule.chord, MAC).unwrap();
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
}

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
        cx.update(|window, _| window.refresh());
    }
}

fn drawer(cx: &mut VisualTestContext) -> Option<Entity<Drawer>> {
    cx.update(|_, cx| terminal::shown_drawer(cx))
}

fn shells(cx: &mut VisualTestContext) -> Vec<String> {
    let Some(drawer) = drawer(cx) else { return Vec::new() };
    cx.update(|_, cx| drawer.update(cx, |d, cx| d.ui(cx).terminal_ids))
}

fn with_threads(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let item = |w: &str, t: &str| ThreadItem { workspace_id: w.into(), thread_id: t.into(), project: "p".into(), title: t.into() };
    shell.update_in(cx, |shell, window, cx| {
        shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("w1", "a"), item("w2", "c")], cx));
        shell.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx);
    });
    cx.run_until_parked();
}

/// This computer's drawer, open on a live channel with its first shell.
fn here_with_a_shell(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    press(cx, "terminal.toggle");
    until(cx, "the first shell", |cx| shells(cx).len() == 1);
}

fn split(cx: &mut VisualTestContext, act: Act) {
    let drawer = drawer(cx).unwrap();
    drawer.update_in(cx, |d, window, cx| match act {
        Act::New => d.open_shell(None, false, window, cx),
        _ => d.open_shell(Some(Direction::Horizontal), false, window, cx),
    });
}

#[gpui_kit::test]
fn the_drawer_opens_under_the_thread_view_at_280_px_and_closes_again(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let bounds = cx.debug_bounds("terminal-drawer").expect("the drawer is drawn");
    let center = cx.debug_bounds("center").unwrap();
    assert_eq!(bounds.size.height, px(280.));
    assert_eq!(bounds.bottom(), center.bottom(), "the drawer sits at the foot of the centre column");
    assert_eq!(bounds.size.width, center.size.width);
    assert!(cx.debug_bounds("terminal-toolbar").is_some(), "a lone shell carries the floating toolbar");
    let view = drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone());
    until(cx, "the shell's focus", |cx| cx.update(|window, cx| view.read(cx).focus_handle(cx).is_focused(window)));
    press(cx, "terminal.new");
    cx.dispatch_action(keys::NewTerminal);
    cx.dispatch_action(keys::SplitTerminal);
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(shells(cx).len(), 1, "with no workspace selected new and split do nothing");
    press(cx, "terminal.toggle");
    assert!(cx.debug_bounds("terminal-drawer").is_none());
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()));
}

#[gpui_kit::test]
fn the_strip_drags_the_height_between_180_px_and_three_quarters_of_the_window(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let drag = |cx: &mut VisualTestContext, to: f32| {
        let strip = cx.debug_bounds("terminal-drawer-strip").unwrap();
        let (x, y) = (strip.center().x, strip.center().y);
        cx.simulate_mouse_down(point(x, y), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(x, y - px(10.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(x, px(to)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(point(x, px(to)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.debug_bounds("terminal-drawer").unwrap().size.height
    };
    assert_eq!(drag(cx, 10.), px(600.), "75% of the 800 px window");
    assert_eq!(drag(cx, 790.), px(180.));
    assert_eq!(cx.update(|_, cx| hub::drawer(&Key::Here, cx).terminal_height), 180., "the drag's end saves the height");
}

#[gpui_kit::test]
fn a_second_shell_brings_the_144_px_tab_column_and_a_fifth_split_is_refused(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    assert!(cx.debug_bounds("terminal-tabs").is_none(), "one shell, no tab column");
    split(cx, Act::Split);
    until(cx, "a second shell", |cx| shells(cx).len() == 2);
    let tabs = cx.debug_bounds("terminal-tabs").expect("two shells show the tab column");
    assert_eq!(tabs.size.width, px(144.));
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    let area = cx.debug_bounds("terminal-area").unwrap();
    let views = drawer(cx).unwrap().read_with(cx, |d, _| d.views().iter().map(|(_, v)| v.clone()).collect::<Vec<_>>());
    for view in views {
        let grid = view.read_with(cx, |v, _| v.geometry().bounds);
        assert_eq!(grid.size.height, area.size.height, "each shell side by side takes the area's whole height");
    }
    assert!(cx.debug_bounds("terminal-toolbar").unwrap().size.height == px(22.), "the toolbar is the column's 22 px head");
    split(cx, Act::Split);
    until(cx, "a third shell", |cx| shells(cx).len() == 3);
    split(cx, Act::Split);
    until(cx, "a fourth shell", |cx| shells(cx).len() == 4);
    let ui: Ui = cx.update(|_, cx| hub::drawer(&Key::Here, cx));
    assert!(!ui.can_split());
    assert_eq!(toolbar::label(Act::Split, true, false), "Split terminal horizontally (Max 4 per group)");
    split(cx, Act::Split);
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(shells(cx).len(), 4, "a fifth split opens no shell");
    let held = Ui { ..ui.clone() }.split("e", Direction::Horizontal, Place::AfterActive);
    assert_eq!(held.terminal_ids.len(), 4);
}

#[gpui_kit::test]
fn a_panel_tab_keeps_its_shell_across_a_switch_and_ends_it_when_it_closes(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("terminal".into()), window, cx));
    let tab = shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).first().cloned().expect("a terminal tab").1;
    let surface = |cx: &mut VisualTestContext| cx.update(|_, cx| hub::surface(&tab, cx)).map(|ui| ui.terminal_ids).unwrap_or_default();
    until(cx, "the tab's shell", |cx| surface(cx).len() == 1);
    let pty = surface(cx)[0].clone();
    let listed = |pty: &str| daemon_ptys(&host).iter().any(|p| p == pty);
    assert!(listed(&pty));
    with_threads(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Empty, window, cx));
    cx.run_until_parked();
    std::thread::sleep(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(surface(cx), std::slice::from_ref(&pty), "the same shell after a thread switch, and no second one");
    assert_eq!(daemon_ptys(&host), std::slice::from_ref(&pty));
    assert!(listed(&pty));
    shell.update_in(cx, |s, window, cx| s.panel().update(cx, |p, cx| p.close(0, window, cx)));
    cx.run_until_parked();
    assert!(surface(cx).is_empty());
    assert!(!listed(&pty), "closing the tab ends its shell");
}

#[gpui_kit::test]
fn the_sidebars_open_terminal_opens_the_drawer_and_never_the_panel(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    with_threads(&shell, cx);
    let ask = |workspace: &str, cx: &mut VisualTestContext| {
        let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
        sidebar.update(cx, |_, cx| cx.emit(Asked::OpenTerminal(workspace.into())));
        cx.run_until_parked();
    };
    ask("w1", cx);
    assert!(shell.read_with(cx, |s, _| s.drawer_open()), "the open thread's drawer");
    assert!(!shell.read_with(cx, |s, cx| s.panel().read(cx).is_open()));
    ask("w1", cx);
    assert!(shell.read_with(cx, |s, _| s.drawer_open()), "it opens, it does not toggle");
    ask("w2", cx);
    assert!(cx.update(|_, cx| hub::is_open(&Key::Workspace("w2".into()), cx)), "another workspace's own drawer");
}

#[gpui_kit::test]
fn each_thread_keeps_its_own_drawer_open_or_shut(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    with_threads(&shell, cx);
    press(cx, "terminal.toggle");
    assert!(shell.read_with(cx, |s, _| s.drawer_open()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, window, cx));
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()), "the other thread's drawer is its own");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |s, _| s.drawer_open()));
}

#[gpui_kit::test]
fn the_toggle_does_nothing_while_settings_is_open(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    with_threads(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(SETTINGS.into()), window, cx));
    press(cx, "terminal.toggle");
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()));
    assert!(!cx.update(|_, cx| hub::is_open(&Key::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, cx)));
}

#[gpui_kit::test]
fn keys_reach_the_active_shell_when_shells_left_behind_are_adopted(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    runtime().block_on(async {
        let client = host.client().await;
        let opened = client.call(common::ask::<DaemonOpenRequest>(json!({ "placeId": "here" }))).await.unwrap();
        let channel = serde_json::to_value(&opened).unwrap()["channel"].as_str().unwrap().to_owned();
        for _ in 0..12 {
            let frame = json!({ "op": "pty.create", "cols": 80, "rows": 24 });
            client.call(common::ask::<DaemonSendRequest>(json!({ "channel": channel, "frame": frame }))).await.unwrap();
        }
        _ = client.call(common::ask::<DaemonCloseRequest>(json!({ "channel": channel }))).await;
    });
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    press(cx, "terminal.toggle");
    until(cx, "the shells left behind adopted", |cx| shells(cx).len() >= 12);
    std::thread::sleep(Duration::from_millis(1500));
    cx.run_until_parked();
    let drawer = drawer(cx).unwrap();
    let active = cx.update(|_, cx| drawer.update(cx, |d, cx| d.ui(cx).active_terminal_id));
    let view = drawer
        .read_with(cx, |d, _| d.views().iter().find(|(id, _)| *id == active).map(|(_, v)| v.clone()))
        .expect("the active shell's view");
    until(cx, "the active shell's focus", |cx| cx.update(|window, cx| view.read(cx).focus_handle(cx).is_focused(window)));
    cx.simulate_input("echo keys-$((5*7))");
    cx.simulate_keystrokes("enter");
    let session = view.read_with(cx, |v, _| v.session().clone());
    until(cx, "the typed line answered on the active shell's screen", |cx| {
        session.update(cx, |s, _| {
            let mut frame = desktop_gpui::screens::terminal::engine::Frame::default();
            s.engine_mut().frame(&mut frame);
            frame.rows.iter().any(|r| r.plain().trim() == "keys-35")
        })
    });
}

#[gpui_kit::test]
fn the_pane_follows_the_workspace_state_the_sidebar_holds(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let runtime = runtime();
    let client = Arc::new(runtime.block_on(host.client()));
    let shell_host = desktop_gpui::screens::Host::new(client, runtime);
    let sidebar = cx.new(Sidebar::new);
    cx.update(|cx| terminal::read_workspaces_from(&sidebar, cx));
    let record = |phase: &str| {
        json!({
            "id": "w1", "name": "w1", "machineId": "m1", "phase": phase, "golden": "g", "createdAt": "2026-10-18T00:00:00Z",
            "kind": "cloud", "project": { "id": "p1", "name": "p", "path": "/p", "computer": "c" },
        })
    };
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![record("running")]), cx));
    let mut drawer = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Drawer::new(shell_host, Mode::Drawer(Key::Workspace("w1".into())), window, cx));
        drawer = Some(view.clone());
        Root::new(view, window, cx)
    });
    let drawer = drawer.unwrap();
    cx.run_until_parked();
    let paused = |cx: &mut VisualTestContext| matches!(cx.update(|_, cx| drawer.read(cx).pane(cx)), PaneState::Paused { .. });
    assert!(!paused(cx), "running as the record first says");
    let mut status = record("napping");
    status["machineState"] = "paused".into();
    status["reach"] = json!({ "state": "napping" });
    status["size"] = json!({ "cpu": 2, "memMb": 4096 });
    status["rateUsdPerHour"] = json!(0.1);
    let stopped = |cx: &mut VisualTestContext| cx.update(|_, cx| drawer.read(cx).link().read(cx).is_stopped());
    let mut kept = record("running");
    kept["pauseMode"] = "memory".into();
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![kept]), cx));
    sidebar.update(cx, |s, cx| s.apply(Change::Status(status.clone()), cx));
    cx.run_until_parked();
    assert!(paused(cx), "the status the host pushed puts the open drawer's pane to sleep");
    assert!(!stopped(cx), "a nap that keeps memory keeps the shells");
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![record("running")]), cx));
    cx.run_until_parked();
    assert!(stopped(cx), "a nap that ends processes ends the shells, which the next attach says");
}

fn screen_of(view: &Entity<desktop_gpui::screens::terminal::view::TerminalView>, cx: &mut VisualTestContext) -> Vec<String> {
    let session = view.read_with(cx, |v, _| v.session().clone());
    session.update(cx, |s, _| {
        let mut frame = desktop_gpui::screens::terminal::engine::Frame::default();
        s.engine_mut().frame(&mut frame);
        frame.rows.iter().map(|r| r.plain().trim_end().to_owned()).collect()
    })
}

#[gpui_kit::test]
fn the_right_click_menu_reads_todays_rows_and_each_row_does_its_work(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let drawer = drawer(cx).unwrap();
    let (pty, view) = drawer.read_with(cx, |d, _| d.views()[0].clone());
    let at = cx.update(|_, cx| view.read(cx).cell_bounds(2, 1).unwrap().center());
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert!(drawer.read_with(cx, |d, _| d.menu_open()), "a right-click opens the menu");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let rows = cx.update(|_, cx| drawer.update(cx, |d, cx| d.menu_rows(&pty, cx)));
    let read: Vec<(String, Option<String>)> =
        rows.iter().map(|(_, i)| (i.label.to_string(), i.refusal.as_ref().map(|r| r.to_string()))).collect();
    assert_eq!(read[0], ("Copy".to_owned(), Some(menu::NOTHING_SELECTED.to_owned())), "{read:?}");
    assert!(!read.iter().any(|(l, _)| l == "Add to chat"), "Add to chat only with a selection");
    let run = |row: menu::Row, cx: &mut VisualTestContext| {
        drawer.update_in(cx, |d, window, cx| d.run_menu_row(&pty, row, window, cx));
        cx.run_until_parked();
    };

    view.update(cx, |v, cx| v.session().update(cx, |s, cx| s.send(b"seq 1 200\r", cx)));
    until(cx, "the output", |cx| screen_of(&view, cx).iter().any(|l| l == "200"));
    assert!(view.read_with(cx, |v, cx| v.session().read(cx).engine().scrollback_rows()) > 0);
    run(menu::Row::Clear, cx);
    assert!(screen_of(&view, cx).iter().all(|l| l.is_empty()), "Clear empties the screen: {:?}", screen_of(&view, cx));
    assert_eq!(view.read_with(cx, |v, cx| v.session().read(cx).engine().scrollback_rows()), 0, "and the scrollback");

    cx.update(|_, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("echo pasted-$((2*4))".into())));
    run(menu::Row::Paste, cx);
    view.update(cx, |v, cx| v.session().update(cx, |s, cx| s.send(b"\r", cx)));
    until(cx, "the pasted line run", |cx| screen_of(&view, cx).iter().any(|l| l == "pasted-8"));

    split(cx, Act::Split);
    until(cx, "a second shell", |cx| shells(cx).len() == 2);
    let active = cx.update(|_, cx| drawer.update(cx, |d, cx| d.ui(cx).active_terminal_id));
    drawer.update_in(cx, |d, window, cx| d.run_menu_row(&active, menu::Row::Pane(Act::Close), window, cx));
    cx.run_until_parked();
    assert_eq!(shells(cx), vec![pty.clone()], "Close ends the active shell");
}

#[gpui_kit::test]
fn a_panel_tab_closes_with_its_last_shell(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("terminal".into()), window, cx));
    let tab = shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).first().cloned().expect("a terminal tab").1;
    until(cx, "the tab's shell", |cx| cx.update(|_, cx| hub::surface(&tab, cx)).is_some_and(|ui| ui.terminal_ids.len() == 1));
    let close = cx.debug_bounds("terminal-close").expect("the lone shell's toolbar");
    cx.simulate_click(close.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).is_empty(), "the tab goes with its last shell");
}

#[gpui_kit::test]
fn a_saved_zoom_is_read_back_from_the_host_at_launch(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    runtime().block_on(async {
        let client = host.client().await;
        let patch = json!({ "patch": { "terminalZoom": { "w1": 3 } } });
        client.call(common::ask::<wsp_host_client::protocol::PreferencesSetRequest>(patch)).await.unwrap();
    });
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    until(cx, "the saved zoom", |cx| cx.update(|_, cx| hub::zoom("w1", cx)) == Some(3.));
}

#[gpui_kit::test]
fn a_refused_open_says_why_in_the_pane(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let runtime = runtime();
    let client = Arc::new(runtime.block_on(host.client()));
    let shell_host = desktop_gpui::screens::Host::new(client, runtime);
    let mut drawer = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Drawer::new(shell_host, Mode::Drawer(Key::Workspace("w-none".into())), window, cx));
        drawer = Some(view.clone());
        Root::new(view, window, cx)
    });
    let drawer = drawer.unwrap();
    cx.run_until_parked();
    drawer.update_in(cx, |d, window, cx| d.open_shell(None, false, window, cx));
    until(cx, "the refusal", |cx| drawer.read_with(cx, |d, _| d.refused().is_some()));
    assert_eq!(drawer.read_with(cx, |d, _| d.refused().map(str::to_owned)).as_deref(), Some("No terminal: not answering"));
}
