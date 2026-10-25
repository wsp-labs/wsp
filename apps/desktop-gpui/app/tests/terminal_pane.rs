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
use desktop_gpui::screens::terminal::words;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell, SETTINGS};
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::ui::ActivePalette;
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, AppContext, Entity, Focusable, Modifiers, MouseButton, Rgba, TestAppContext, VisualTestContext};
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
    with_daemon_running(NO_AGENT)
}

fn with_daemon_running(claude: &str) -> Option<common::Throwaway> {
    if let Err(why) = common::wsp_daemon() {
        eprintln!("skipped: {why}");
        return None;
    }
    Some(common::Throwaway::start_with(claude, true))
}

/// A host carrying the staged daemon with the workspace a project folder's first thread opened, or why there is none.
fn with_a_workspace() -> Option<(common::Throwaway, String)> {
    let host = with_daemon_running(common::STAND_IN)?;
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let workspace = runtime().block_on(async { common::first(&host.client().await).await.workspace_id });
    Some((host, workspace))
}

/// The shells this computer's daemon lists, asked on a channel of the test's own.
fn daemon_ptys(host: &common::Throwaway) -> Vec<String> {
    ptys_on(host, json!({ "placeId": "here" }))
}

/// The shells the daemon `open` names lists, asked on a channel of the test's own.
fn ptys_on(host: &common::Throwaway, open: serde_json::Value) -> Vec<String> {
    runtime().block_on(async {
        let client = host.client().await;
        let opened = client.call(common::ask::<DaemonOpenRequest>(open)).await.unwrap();
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
fn a_shut_drawer_lets_its_channel_go_and_the_next_open_finds_its_shell(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let pty = shells(cx)[0].clone();
    let link = drawer(cx).unwrap().read_with(cx, |d, _| d.link().clone());
    press(cx, "terminal.toggle");
    for _ in 0..5 {
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
    }
    assert!(link.read_with(cx, |l, _| l.ptys().is_empty()), "a shut drawer holds no screens 30 s on");
    press(cx, "terminal.toggle");
    until(cx, "the shell adopted again", |cx| shells(cx) == std::slice::from_ref(&pty));
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

/// A launch holds no surface for a panel tab the app's store kept: its tab adopts the shell the host still holds under
/// the tab's tag, and opens no second one beside it.
#[gpui_kit::test]
fn a_kept_panel_tab_with_no_surface_adopts_its_shell_and_opens_no_other(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("terminal".into()), window, cx));
    let tab = shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).first().cloned().expect("a terminal tab").1;
    let surface = |cx: &mut VisualTestContext| cx.update(|_, cx| hub::surface(&tab, cx)).map(|ui| ui.terminal_ids).unwrap_or_default();
    until(cx, "the tab's shell", |cx| surface(cx).len() == 1);
    let pty = surface(cx)[0].clone();
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| p.toggle(window, cx));
    cx.update(|_, cx| hub::drop_surface(&tab, cx));
    panel.update_in(cx, |p, window, cx| p.toggle(window, cx));
    until(cx, "the tab's shell again", |cx| !surface(cx).is_empty());
    std::thread::sleep(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(surface(cx), std::slice::from_ref(&pty), "the kept shell, adopted");
    assert_eq!(daemon_ptys(&host), std::slice::from_ref(&pty), "and no second shell on the host");
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
    let rows = cx.update(|_, cx| drawer.update(cx, |d, cx| d.menu_rows(&pty, cx)));
    let labels: Vec<_> = rows.iter().map(|(_, i)| i.label.clone()).collect();
    assert_eq!(drawer.read_with(cx, |d, _| d.menu_asked()), labels, "a right-click asks for today's rows");
    // On the Mac AppKit draws the menu, which the test platform cannot drive.
    if !cfg!(target_os = "macos") {
        assert!(drawer.read_with(cx, |d, _| d.menu_open()), "a right-click opens the menu");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
    }
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
fn a_theme_on_the_same_side_recolors_the_open_shell(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let view = drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone());
    let ground = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let theme: Rgba = cx.palette().terminal_background.into();
            let shown = view.read(cx).look().colors.background;
            ([shown.0, shown.1, shown.2], [theme.r, theme.g, theme.b].map(|v| (v * 255.).round() as u8))
        })
    };
    let before = ground(cx).0;
    cx.update(|_, cx| desktop_gpui::ui::theme::set_theme("linen", cx));
    until(cx, "the shell drawn on the new theme's ground", |cx| {
        let (shown, theme) = ground(cx);
        shown == theme
    });
    assert_ne!(ground(cx).0, before, "Linen's ground is not the one the shell opened on");
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
fn the_panels_new_terminal_opens_a_tab_of_its_own_beside_a_lone_shell(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("terminal".into()), window, cx));
    let tab = shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).first().cloned().expect("a terminal tab").1;
    let surface = |cx: &mut VisualTestContext| cx.update(|_, cx| hub::surface(&tab, cx)).map(|ui| ui.terminal_ids.len());
    until(cx, "the tab's shell", |cx| surface(cx) == Some(1));
    let new = cx.debug_bounds("terminal-new").expect("the lone shell's toolbar");
    cx.simulate_click(new.center(), Modifiers::none());
    until(cx, "a second terminal tab", |cx| shell.read_with(cx, |s, cx| s.panel().read(cx).tabs()).len() == 2);
    assert_eq!(surface(cx), Some(1), "the first tab keeps its lone shell");
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

#[gpui_kit::test]
fn a_thread_switch_keeps_the_drawers_shell_and_coming_back_shows_it(cx: &mut TestAppContext) {
    let Some((host, workspace)) = with_a_workspace() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let thread = |id: &str| Route::Thread { workspace_id: workspace.clone(), thread_id: id.into() };
    shell.update_in(cx, |s, window, cx| s.go(thread("a"), window, cx));
    press(cx, "terminal.toggle");
    until(cx, "thread a's shell", |cx| shells(cx).len() == 1);
    let pty = shells(cx)[0].clone();
    let view = drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone());
    view.update(cx, |v, cx| v.session().update(cx, |s, cx| s.send(b"echo kept-$((3*4))\r", cx)));
    until(cx, "the shell's output", |cx| screen_of(&view, cx).iter().any(|l| l == "kept-12"));

    shell.update_in(cx, |s, window, cx| s.go(thread("c"), window, cx));
    press(cx, "terminal.toggle");
    until(cx, "thread c's own shell", |cx| shells(cx).len() == 1 && shells(cx)[0] != pty);
    let listed = ptys_on(&host, json!({ "workspaceId": workspace }));
    assert!(listed.contains(&pty), "the switch kept thread a's shell: {listed:?}");

    shell.update_in(cx, |s, window, cx| s.go(thread("a"), window, cx));
    until(cx, "thread a's shell back", |cx| shells(cx) == [pty.clone()]);
    let back = drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone());
    assert!(screen_of(&back, cx).iter().any(|l| l == "kept-12"), "with what it showed: {:?}", screen_of(&back, cx));
}

/// A drawer or a panel tab alone in a window on `host`, reading its workspace from a sidebar of the test's own.
fn alone<'a>(
    cx: &'a mut TestAppContext,
    host: &common::Throwaway,
    mode: Mode,
) -> (Entity<Sidebar>, Entity<Drawer>, &'a mut VisualTestContext) {
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
    let mut drawer = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Drawer::new(shell_host, mode, window, cx));
        drawer = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    (sidebar, drawer.unwrap(), cx)
}

fn workspace_on(kind: &str, place: Option<&str>) -> serde_json::Value {
    let mut record = json!({
        "id": "w1", "name": "w1-box", "machineId": "m1", "phase": "running", "golden": "g", "createdAt": "2026-10-18T00:00:00Z",
        "kind": kind, "project": { "id": "p1", "name": "p", "path": "/p", "computer": "c" },
    });
    if let Some(place) = place {
        record["place"] = place.into();
    }
    record
}

#[gpui_kit::test]
fn the_pane_names_the_computer_and_says_when_an_own_one_is_off(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (sidebar, drawer, cx) = alone(cx, &host, Mode::Drawer(Key::Workspace("w1".into())));
    let studio =
        |present: bool| json!({ "id": "studio", "kind": "computer", "name": "studio-host", "label": "Studio", "present": present });
    let here = json!({ "id": "here", "kind": "computer", "name": "boat", "label": "Ana's Mac" });
    let pane = |cx: &mut VisualTestContext| cx.update(|_, cx| drawer.read(cx).pane(cx));
    let title = |cx: &mut VisualTestContext| words::title(&pane(cx)).unwrap_or_default();
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Places(vec![here.clone(), studio(true)]), cx);
        s.apply(Change::Workspaces(vec![workspace_on("place", Some("studio"))]), cx);
    });
    until(cx, "a state over the empty pane", |cx| !title(cx).is_empty());
    assert!(title(cx).contains("Studio"), "a computer of the person's own goes by its own name: {}", title(cx));
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![workspace_on("local", None)]), cx));
    cx.run_until_parked();
    assert!(title(cx).contains("Ana's Mac"), "this computer by its label: {}", title(cx));
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![workspace_on("cloud", None)]), cx));
    cx.run_until_parked();
    assert!(
        !title(cx).contains("Studio") && !title(cx).contains("Ana's Mac"),
        "a machine a cloud lends is no computer of theirs: {}",
        title(cx)
    );

    sidebar.update(cx, |s, cx| {
        s.apply(Change::Workspaces(vec![workspace_on("place", Some("studio"))]), cx);
        s.apply(Change::Places(vec![here, studio(false)]), cx);
    });
    cx.run_until_parked();
    assert_eq!(pane(cx), PaneState::Absent(words::absent_computer("Studio")));
    assert_eq!(
        words::empty_line(&pane(cx)).as_deref(),
        Some("Studio is not answering; it connects on its own when it is on, and a terminal opens then")
    );
    assert!(cx.debug_bounds("terminal-empty").is_some(), "the empty pane says it");
}

#[gpui_kit::test]
fn the_new_thread_pages_drawer_becomes_the_started_threads_and_a_fresh_page_gets_its_own(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::link::{Link as ComposerLink, Target};
    use desktop_gpui::screens::composer::{self, Composer};
    let Some((host, workspace)) = with_a_workspace() else { return };
    cx.update(composer::init);
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(format!("composer:{workspace}").into()), window, cx));
    press(cx, "terminal.toggle");
    let page = Key::Thread { workspace_id: workspace.clone(), thread_id: String::new() };
    assert!(cx.update(|_, cx| hub::is_open(&page, cx)), "Ctrl+J opens the page's own drawer");
    until(cx, "the page's first shell", |cx| shells(cx).len() == 1);
    let pty = shells(cx)[0].clone();

    let runtime = runtime();
    let client = Arc::new(runtime.block_on(host.client()));
    let composer_host = desktop_gpui::screens::Host::new(client, runtime.clone());
    let target = Target { workspace_id: workspace.clone(), thread_id: None, project: None };
    let opened = runtime.block_on(ComposerLink::read(&composer_host, &target)).unwrap_or_else(|f| panic!("{}", f.message));
    let composer = cx.update(|window, cx| cx.new(|cx| Composer::new(window, cx)));
    let _link = cx.update(|window, cx| {
        let composer = composer.clone();
        cx.new(|cx| ComposerLink::new(composer_host, target, opened, composer, window, cx))
    });
    composer.update_in(cx, |c, window, cx| {
        c.set_text("a thread with a shell", window, cx);
        c.submit(window, cx);
    });
    let mut thread = None;
    until(cx, "the thread the send opened", |_| {
        let threads = runtime.block_on(async { host.client().await.threads().await }).unwrap();
        thread = threads.into_iter().find(|t| t.prompt.as_deref() == Some("a thread with a shell")).and_then(|t| t.thread_id);
        thread.is_some()
    });
    let thread = thread.unwrap();
    let carried = Key::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() };
    let shown = |cx: &mut VisualTestContext| drawer(cx).map(|d| d.read_with(cx, |d, _| d.mode().clone()));
    until(cx, "the page showing the drawer of the thread it started", |cx| {
        shown(cx) == Some(Mode::Drawer(carried.clone())) && shells(cx) == std::slice::from_ref(&pty)
    });
    let opened = Route::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() };
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), opened, "the send opened its thread in place");
    assert!(shell.read_with(cx, |s, _| s.drawer_open()), "still open on the thread, as it was on the page before the send");

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() }, window, cx));
    cx.run_until_parked();
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(format!("composer:{workspace}").into()), window, cx));
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()), "a fresh New thread page starts with its own drawer shut");
    press(cx, "terminal.toggle");
    until(cx, "the fresh page's own first shell", |cx| shown(cx) == Some(Mode::Drawer(page.clone())) && shells(cx).len() == 1);
    assert_ne!(shells(cx)[0], pty, "a shell of its own, not the started thread's");
}

/// A Ghostty config in the host's home: 60% opacity, and a theme whose light and dark sides draw different grounds.
fn with_ghostty_config(host: &common::Throwaway) {
    let ghostty = host.dir.path().join(".config/ghostty");
    std::fs::create_dir_all(ghostty.join("themes")).unwrap();
    std::fs::write(ghostty.join("themes/Day"), "background = #ffeedd\n").unwrap();
    std::fs::write(ghostty.join("themes/Night"), "background = #112233\n").unwrap();
    std::fs::write(ghostty.join("config.ghostty"), "theme = light:Day,dark:Night\nbackground-opacity = 0.6\n").unwrap();
}

fn set_preference(host: &common::Throwaway, patch: serde_json::Value) {
    runtime().block_on(async {
        let client = host.client().await;
        client.call(common::ask::<wsp_host_client::protocol::PreferencesSetRequest>(json!({ "patch": patch }))).await.unwrap();
    });
}

fn first_view(cx: &mut VisualTestContext) -> Entity<desktop_gpui::screens::terminal::view::TerminalView> {
    drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone())
}

#[gpui_kit::test]
fn the_pane_is_see_through_under_glass_and_opaque_without_it(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    with_ghostty_config(&host);
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let view = first_view(cx);
    let opacity = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.look().opacity);
    let see_through = |cx: &mut VisualTestContext| drawer(cx).unwrap().read_with(cx, |d, _| d.see_through());
    until(cx, "the config's opacity under the page's glass", |cx| opacity(cx) == 0.6);
    assert!(shell.read_with(cx, |s, _| s.look().glass));
    assert!(see_through(cx), "the drawer drops its ground for the glass");
    set_preference(&host, json!({ "transparency": false }));
    until(cx, "the window solid", |cx| !shell.read_with(cx, |s, _| s.look().glass));
    until(cx, "an opaque pane whatever the config says", |cx| opacity(cx) == 1.);
    assert!(!see_through(cx), "and keeps its ground");
    set_preference(&host, json!({ "transparency": true }));
    until(cx, "the pane see-through again", |cx| opacity(cx) == 0.6);
}

#[gpui_kit::test]
fn glass_over_a_config_with_no_opacity_keeps_the_drawers_ground(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let ghostty = host.dir.path().join(".config/ghostty");
    std::fs::create_dir_all(&ghostty).unwrap();
    std::fs::write(ghostty.join("config.ghostty"), "cursor-style = bar\n").unwrap();
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    until(cx, "the config read", |cx| first_view(cx).read_with(cx, |v, _| v.look().cursor_style.is_some()));
    assert!(shell.read_with(cx, |s, _| s.look().glass));
    assert!(!drawer(cx).unwrap().read_with(cx, |d, _| d.see_through()), "an opaque config keeps the ground under glass");
}

#[gpui_kit::test]
fn a_flip_to_the_other_side_reads_that_sides_theme_from_the_config(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    with_ghostty_config(&host);
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let view = first_view(cx);
    let ground = |cx: &mut VisualTestContext| {
        let c = view.read_with(cx, |v, _| v.look().colors.background);
        [c.0, c.1, c.2]
    };
    until(cx, "the light side's ground", |cx| ground(cx) == [0xff, 0xee, 0xdd]);
    set_preference(&host, json!({ "theme": "dark" }));
    until(cx, "the dark side's ground once the app is dark", |cx| ground(cx) == [0x11, 0x22, 0x33]);
    set_preference(&host, json!({ "theme": "light" }));
    until(cx, "the light side's ground again", |cx| ground(cx) == [0xff, 0xee, 0xdd]);
}

#[gpui_kit::test]
fn a_relative_path_link_opens_against_the_threads_folder(cx: &mut TestAppContext) {
    let Some((host, workspace)) = with_a_workspace() else { return };
    let folder = runtime().block_on(async {
        let client = host.client().await;
        let listed = client.call(common::ask::<wsp_host_client::protocol::WorkspacesListRequest>(json!({}))).await.unwrap();
        let row = serde_json::to_value(listed.workspaces)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == workspace.as_str())
            .cloned()
            .unwrap();
        row["folder"].as_str().or(row["project"]["path"].as_str()).unwrap().to_owned()
    });
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let thread = Route::Thread { workspace_id: workspace.clone(), thread_id: "a".into() };
    shell.update_in(cx, |s, window, cx| s.go(thread.clone(), window, cx));
    press(cx, "terminal.toggle");
    until(cx, "the thread's shell", |cx| shells(cx).len() == 1);
    let target = drawer(cx).unwrap().read_with(cx, |d, _| d.path_route("src/../lib.rs:3"));
    assert_eq!(target, Some(Route::Screen(format!("files:file:{folder}/lib.rs:3").into())));
    let view = first_view(cx);
    view.update(cx, |_, cx| cx.emit(desktop_gpui::screens::terminal::view::ViewEvent::OpenPath("src/../lib.rs:3".into())));
    cx.run_until_parked();
    let opened = shell.read_with(cx, |s, _| s.route().clone());
    assert_eq!(opened, thread, "this build has no Files screen, so the thread stays on screen instead of a blank page");
}

#[gpui_kit::test]
fn a_reopened_drawer_never_draws_an_adopted_shell_before_its_replay(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let view = first_view(cx);
    view.update(cx, |v, cx| v.session().update(cx, |s, cx| s.send(b"echo kept-$((6*7))\r", cx)));
    until(cx, "the output", |cx| screen_of(&view, cx).iter().any(|l| l == "kept-42"));
    let rule = DEFAULTS.iter().find(|r| r.command == "terminal.toggle").unwrap();
    // A frame after every task, so the one drawn between the adoption and its replay's landing is seen.
    let frame = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| _ = window.draw(cx));
        let Some(back) = drawer(cx).and_then(|d| d.read_with(cx, |d, _| d.views().first().map(|(_, v)| v.clone()))) else { return false };
        if cx.debug_bounds("terminal-grid").is_none() {
            return false;
        }
        assert!(
            !back.read_with(cx, |v, cx| v.session().read(cx).awaiting_replay()),
            "a frame drew the adopted shell's grid before its replay landed"
        );
        assert!(screen_of(&back, cx).iter().any(|l| l == "kept-42"), "drawn with what it showed: {:?}", screen_of(&back, cx));
        true
    };
    let mut typed = None;
    // A replay can land before the reopen's first frame, which then has no cover to type under; that reopen is tried again.
    for _ in 0..8 {
        press(cx, "terminal.toggle");
        for _ in 0..5 {
            cx.executor().advance_clock(Duration::from_secs(10));
            cx.run_until_parked();
        }
        cx.simulate_keystrokes(&keys::keystroke(rule.chord, MAC).unwrap());
        let mut type_while_held = |cx: &mut VisualTestContext| {
            if typed.is_some() || cx.debug_bounds("terminal-held").is_none() {
                return;
            }
            let held = drawer(cx).unwrap().read_with(cx, |d, _| d.views()[0].1.clone());
            cx.simulate_input("echo held-$((5*9))");
            cx.simulate_keystrokes("enter");
            typed = Some(held);
        };
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            let mut shown = frame(cx);
            type_while_held(cx);
            while !shown && cx.executor().tick() {
                shown = frame(cx);
                type_while_held(cx);
            }
            if shown {
                break;
            }
            assert!(Instant::now() < deadline, "timed out waiting for the adopted shell drawn");
            std::thread::sleep(Duration::from_millis(5));
        }
        if typed.is_some() {
            break;
        }
    }
    let held = typed.expect("a frame covered the adopted shell while its replay was awaited");
    until(cx, "the keys typed under the cover answered on the shell", |cx| screen_of(&held, cx).iter().any(|l| l == "held-45"));
}

#[gpui_kit::test]
fn a_replay_that_never_lands_uncovers_its_shell_after_two_seconds(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let session = first_view(cx).read_with(cx, |v, _| v.session().clone());
    press(cx, "terminal.toggle");
    session.update(cx, |s, _| s.begin_replay());
    press(cx, "terminal.toggle");
    let covered = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| _ = window.draw(cx));
        cx.debug_bounds("terminal-held").is_some()
    };
    assert!(covered(cx), "a shell awaiting its replay opens covered");
    cx.executor().advance_clock(Duration::from_millis(1900));
    cx.run_until_parked();
    assert!(covered(cx), "still covered short of 2 s");
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert!(!covered(cx) && cx.debug_bounds("terminal-grid").is_some(), "its grid shows once 2 s pass with no replay");
}

#[gpui_kit::test]
fn a_window_that_clamps_the_drawer_saves_the_clamped_height(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    cx.update(|window, cx| {
        let ui = hub::drawer(&Key::Here, cx).set_height(600.);
        hub::set_drawer(&Key::Here, ui, cx);
        window.refresh();
    });
    cx.run_until_parked();
    let height = |cx: &mut VisualTestContext| cx.debug_bounds("terminal-drawer").unwrap().size.height;
    assert_eq!(height(cx), px(600.));
    cx.simulate_resize(size(px(1280.), px(400.)));
    cx.run_until_parked();
    assert_eq!(height(cx), px(300.), "75% of the 400 px window");
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    assert_eq!(height(cx), px(300.), "the clamped height holds as the window regrows");
    assert_eq!(cx.update(|_, cx| hub::drawer(&Key::Here, cx).terminal_height), 300., "and is the one saved");
}

#[gpui_kit::test]
fn a_refused_new_over_shells_says_why_and_a_refused_key_goes_with_its_overlay(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    let view = first_view(cx);
    until(cx, "the shell's focus", |cx| cx.update(|window, cx| view.read(cx).focus_handle(cx).is_focused(window)));
    let drawer = drawer(cx).unwrap();
    let link = drawer.read_with(cx, |d, _| d.link().clone());
    link.update(cx, |l, cx| l.drop_for_test(cx));
    until(cx, "the overlay over the shell", |cx| cx.debug_bounds("terminal-overlay").is_some());
    cx.simulate_keystrokes("a");
    assert!(drawer.read_with(cx, |d, _| d.key_refused().is_some()), "a key on the overlay is refused with its words");
    drawer.update_in(cx, |d, window, cx| d.open_shell(None, false, window, cx));
    until(cx, "the New refused", |cx| drawer.read_with(cx, |d, _| d.refused().is_some()));
    cx.executor().advance_clock(Duration::from_secs(1));
    until(cx, "the channel back and the overlay gone", |cx| {
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.debug_bounds("terminal-overlay").is_none()
    });
    assert_eq!(shells(cx).len(), 1);
    assert!(cx.debug_bounds("terminal-refused").is_some(), "the refusal stays in the pane over the live shell");
    assert_eq!(drawer.read_with(cx, |d, _| d.key_refused().map(str::to_owned)), None, "the key's refusal went with its overlay");
}

#[gpui_kit::test]
fn a_tab_close_button_reached_by_the_keyboard_shows_its_x(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    split(cx, Act::Split);
    until(cx, "a second shell", |cx| shells(cx).len() == 2);
    cx.simulate_mouse_move(point(px(4.), px(4.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-tab-x").is_none(), "no X with the pointer away and nothing focused on a tab");
    cx.simulate_keystrokes("shift");
    let mut shown = false;
    for _ in 0..40 {
        cx.update(|window, cx| window.focus_next(cx));
        cx.run_until_parked();
        if cx.debug_bounds("terminal-tab-x").is_some() {
            shown = true;
            break;
        }
    }
    assert!(shown, "a close button the keyboard reached shows its X");
}

#[gpui_kit::test]
fn a_pane_that_wakes_dials_at_once(cx: &mut TestAppContext) {
    let Some((host, workspace)) = with_a_workspace() else { return };
    let record = runtime().block_on(async {
        let client = host.client().await;
        let listed = client.call(common::ask::<wsp_host_client::protocol::WorkspacesListRequest>(json!({}))).await.unwrap();
        serde_json::to_value(listed.workspaces)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == workspace.as_str())
            .cloned()
            .unwrap()
    });
    let (sidebar, drawer, cx) = alone(cx, &host, Mode::Drawer(Key::Workspace(workspace.clone())));
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![record.clone()]), cx));
    let link = drawer.read_with(cx, |d, _| d.link().clone());
    let live = |cx: &mut VisualTestContext| link.read_with(cx, |l, _| *l.status() == terminal::link::Status::Live);
    until(cx, "the channel live", live);
    let mut status = record.clone();
    status["phase"] = "napping".into();
    status["machineState"] = "paused".into();
    status["reach"] = json!({ "state": "napping" });
    status["size"] = json!({ "cpu": 2, "memMb": 4096 });
    status["rateUsdPerHour"] = json!(0.1);
    sidebar.update(cx, |s, cx| s.apply(Change::Status(status.clone()), cx));
    cx.run_until_parked();
    assert!(matches!(cx.update(|_, cx| drawer.read(cx).pane(cx)), PaneState::Paused { .. }));
    link.update(cx, |l, cx| l.drop_for_test(cx));
    cx.run_until_parked();
    assert!(!live(cx), "the nap took the channel");
    status["phase"] = "running".into();
    status["machineState"] = "running".into();
    status["reach"] = json!({ "state": "reachable" });
    sidebar.update(cx, |s, cx| s.apply(Change::Status(status), cx));
    until(cx, "the channel dialled the moment the thread runs, with no backoff waited out", live);
}

#[gpui_kit::test]
fn a_panel_tab_on_a_workspace_the_host_has_not_listed_says_it_has_no_link(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let mode = Mode::Panel { key: Key::Workspace("w-none".into()), tab: "terminal:1".into() };
    let (_sidebar, _drawer, cx) = alone(cx, &host, mode);
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-no-link").is_some(), "the panel says there is no link yet");
    assert_eq!(words::NO_LINK_TITLE, "No terminal link for this thread yet.");
    assert_eq!(words::no_link_line(None), "Terminals connect once the thread's computer answers.");
    assert_eq!(words::no_link_line(Some("Studio")), "Terminals connect once Studio answers.");
}

#[gpui_kit::test]
fn a_machine_gone_quiet_with_its_memory_full_says_what_took_it(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (sidebar, drawer, cx) = alone(cx, &host, Mode::Drawer(Key::Workspace("w1".into())));
    let record = workspace_on("cloud", None);
    let mut status = record.clone();
    status["machineState"] = "running".into();
    status["reach"] = json!({ "state": "zombie" });
    status["size"] = json!({ "cpu": 2, "memMb": 4096 });
    status["rateUsdPerHour"] = json!(0.1);
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Workspaces(vec![record]), cx);
        s.apply(Change::Status(status), cx);
    });
    cx.run_until_parked();
    let pane = |cx: &mut VisualTestContext| cx.update(|_, cx| drawer.read(cx).pane(cx));
    assert_eq!(words::title(&pane(cx)).as_deref(), Some("The thread's computer is not answering"));
    let gib = 1024. * 1024. * 1024.;
    let full = words::Memory { used: 3.6 * gib, total: 4. * gib, load1: 1.25 };
    let link = drawer.read_with(cx, |d, _| d.link().clone());
    link.update(cx, |l, cx| l.sample_for_test(full, cx));
    cx.run_until_parked();
    assert_eq!(words::title(&pane(cx)), Some(words::out_of_memory_line(&full)), "the last reading before the machine went quiet");
}

#[gpui_kit::test]
fn a_path_link_opens_nothing_while_the_threads_folder_is_unknown(cx: &mut TestAppContext) {
    let Some(host) = with_daemon() else { return };
    let (shell, cx) = open(cx, host.state(), host.env());
    here_with_a_shell(&shell, cx);
    assert_eq!(drawer(cx).unwrap().read_with(cx, |d, _| d.path_route("/etc/hosts:2")), None, "no folder, no target");
    let view = first_view(cx);
    view.update(cx, |_, cx| cx.emit(desktop_gpui::screens::terminal::view::ViewEvent::OpenPath("/etc/hosts:2".into())));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Empty, "this computer's drawer holds no thread's folder");
}

#[gpui_kit::test]
fn a_refused_wake_is_not_left_as_a_red_line_over_live_shells(cx: &mut TestAppContext) {
    let Some((host, workspace)) = with_a_workspace() else { return };
    let record = runtime().block_on(async {
        let client = host.client().await;
        let listed = client.call(common::ask::<wsp_host_client::protocol::WorkspacesListRequest>(json!({}))).await.unwrap();
        serde_json::to_value(listed.workspaces)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == workspace.as_str())
            .cloned()
            .unwrap()
    });
    let (sidebar, drawer, cx) = alone(cx, &host, Mode::Drawer(Key::Workspace(workspace.clone())));
    sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![record.clone()]), cx));
    let link = drawer.read_with(cx, |d, _| d.link().clone());
    until(cx, "the channel live", |cx| link.read_with(cx, |l, _| *l.status() == terminal::link::Status::Live));
    drawer.update_in(cx, |d, window, cx| d.open_shell(None, false, window, cx));
    until(cx, "a shell", |cx| drawer.read_with(cx, |d, _| d.views().len()) == 1);
    let mut status = record.clone();
    status["phase"] = "napping".into();
    status["machineState"] = "paused".into();
    status["pauseMode"] = "memory".into();
    status["reach"] = json!({ "state": "napping" });
    status["size"] = json!({ "cpu": 2, "memMb": 4096 });
    status["rateUsdPerHour"] = json!(0.1);
    sidebar.update(cx, |s, cx| s.apply(Change::Status(status.clone()), cx));
    cx.run_until_parked();
    drawer.update(cx, |d, cx| d.answered(Err("the provider refused the resume".into()), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-overlay").is_some(), "the refusal under the paused overlay's Wake");
    status["phase"] = "running".into();
    status["machineState"] = "running".into();
    status["reach"] = json!({ "state": "reachable" });
    sidebar.update(cx, |s, cx| s.apply(Change::Status(status), cx));
    until(cx, "the pane live over its shell", |cx| cx.update(|_, cx| drawer.read(cx).pane(cx)) == PaneState::Live);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-refused").is_none(), "the wake's refusal is not the pane's");
}

#[gpui_kit::test]
fn a_first_send_with_no_terminal_open_dials_no_channel(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::link::{Link as ComposerLink, Target};
    use desktop_gpui::screens::composer::{self, Composer};
    use desktop_gpui::screens::terminal::link::{Link, Target as Reach};
    let Some((host, workspace)) = with_a_workspace() else { return };
    cx.update(composer::init);
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let page = Route::Screen(format!("composer:{workspace}").into());
    let runtime = runtime();
    let composer_host = desktop_gpui::screens::Host::new(Arc::new(runtime.block_on(host.client())), runtime.clone());
    let target = Target { workspace_id: workspace.clone(), thread_id: None, project: None };
    let opened = runtime.block_on(ComposerLink::read(&composer_host, &target)).unwrap_or_else(|f| panic!("{}", f.message));
    let composer = cx.update(|window, cx| cx.new(|cx| Composer::new(window, cx)));
    let _link = cx.update(|window, cx| {
        let composer = composer.clone();
        cx.new(|cx| ComposerLink::new(composer_host, target, opened, composer, window, cx))
    });
    composer.update_in(cx, |c, window, cx| {
        c.set_text("a thread with no terminal", window, cx);
        c.submit(window, cx);
    });
    until(cx, "the page's drawer handed to the thread the send started", |cx| {
        cx.update(|_, cx| terminal::drawer_key(&page, cx)).is_some_and(|k| k.thread_id().is_some_and(|t| !t.is_empty()))
    });
    let reach = Reach::Workspace(workspace.clone());
    assert!(cx.update(|_, cx| Link::find(&reach, cx)).is_none(), "the carry dialled no channel to the workspace");
}
