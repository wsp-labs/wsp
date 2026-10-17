// SPDX-License-Identifier: AGPL-3.0-only
//! The frame: its keys as the web app binds them, its columns' sizes, and what it reads, writes and follows on a
//! throwaway host. Each window is the app's own, the shell inside gpui-kit's Root after `gpui_kit::init` and wsp's
//! themes, with the default key table bound as at launch.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::frame;
use desktop_gpui::screens::app_shell::keys::{self, DEFAULTS};
use desktop_gpui::screens::app_shell::panel::DEFAULT_WIDTH;
use desktop_gpui::screens::{Pane, Route};
use desktop_gpui::shell::{Launch, Shell, SETTINGS};
use desktop_gpui::sidebar::ThreadItem;
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, Action, AppContext, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::protocol::{PreferencesGetRequest, PreferencesSetRequest};
use wsp_host_client::{Env, HostClient};

const MAC: bool = cfg!(target_os = "macos");

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

/// The shell in a 1280 by 800 window, on `state` with `env`; a state no host serves leaves it with no host.
fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    // The host's answers, or its refusal, wake the window's tasks from the tokio runtime's thread.
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

fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    open(cx, std::env::temp_dir().join("desktop-gpui-shell-no-host").join("state.json"), Env::default())
}

/// Threads in two workspaces, as the host would list them, and the first one open.
fn with_threads(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let item = |w: &str, t: &str| ThreadItem { workspace_id: w.into(), thread_id: t.into(), project: "p".into(), title: t.into() };
    shell.update_in(cx, |shell, window, cx| {
        shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("w1", "a"), item("w1", "b"), item("w2", "c")], cx));
        shell.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx);
    });
    cx.run_until_parked();
}

/// The keystroke the table gives `command` here, typed into the window.
fn press(cx: &mut VisualTestContext, command: &str) {
    let rule = DEFAULTS.iter().find(|r| r.command == command).unwrap_or_else(|| panic!("no rule for {command}"));
    press_chord(cx, rule.chord);
}

fn press_chord(cx: &mut VisualTestContext, chord: &str) {
    let typed = keys::keystroke(chord, MAC).unwrap_or_else(|| panic!("{chord} does not read"));
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
}

/// Waits out the sidebar's 200 ms slide and draws the frame it ends on.
fn slid(cx: &mut VisualTestContext) {
    std::thread::sleep(Duration::from_millis(250));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn thread(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(cx, |s, _| match s.route() {
        Route::Thread { thread_id, .. } => Some(thread_id.clone()),
        _ => None,
    })
}

#[gpui_kit::test]
fn both_sidebar_chords_toggle_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    for chord in ["mod+shift+b", "mod+b"] {
        assert!(shell.read_with(cx, |s, _| s.sidebar_open()));
        press_chord(cx, chord);
        assert!(!shell.read_with(cx, |s, _| s.sidebar_open()), "{chord} closes the sidebar");
        assert!(cx.debug_bounds("sidebar-column").is_some(), "it slides out rather than vanish");
        slid(cx);
        assert!(cx.debug_bounds("sidebar-column").is_none(), "nothing of the sidebar stays on screen");
        press_chord(cx, chord);
        assert!(shell.read_with(cx, |s, _| s.sidebar_open()), "{chord} opens it again");
        slid(cx);
        assert_eq!(cx.debug_bounds("sidebar-column").unwrap().origin.x, px(0.), "and slides back in whole");
    }
}

#[gpui_kit::test]
fn the_right_panel_chord_opens_it_on_the_launcher_and_closes_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "a thread's panel starts closed");
    press(cx, "rightPanel.toggle");
    assert!(panel.read_with(cx, |p, _| p.is_open()));
    let launcher = cx.debug_bounds("panel-launcher").expect("an empty panel shows the launcher");
    let bounds = cx.debug_bounds("right-panel").unwrap();
    assert_eq!(bounds.size.width, DEFAULT_WIDTH);
    assert!(launcher.size.width <= px(384.));
    press(cx, "rightPanel.toggle");
    assert!(!panel.read_with(cx, |p, _| p.is_open()));
    assert!(cx.debug_bounds("right-panel").is_none());
}

#[gpui_kit::test]
fn a_letter_on_the_launcher_opens_its_pane(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    press(cx, "rightPanel.toggle");
    cx.simulate_keystrokes("t");
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal), "the launcher took focus as it opened");
    cx.simulate_keystrokes("d");
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Terminal], "with a tab open a letter is the tab's");
    press(cx, "rightPanel.toggle");
    press(cx, "sidebar.toggle");
    assert!(!shell.read_with(cx, |s, _| s.sidebar_open()), "closing the panel hands the keys back to the shell");
}

#[gpui_kit::test]
fn the_terminal_chord_toggles_the_drawer(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    press(cx, "terminal.toggle");
    assert!(shell.read_with(cx, |s, _| s.drawer_open()));
    press(cx, "terminal.toggle");
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()));
}

#[gpui_kit::test]
fn the_browser_chord_opens_the_browser_tab_and_closes_the_panel_on_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    press(cx, "preview.toggle");
    assert_eq!(panel.read_with(cx, |p, _| (p.is_open(), p.active())), (true, Some(Pane::Browser)));
    press(cx, "preview.toggle");
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "pressed on the open browser tab, it closes the panel");
}

#[gpui_kit::test]
fn the_settings_chord_opens_settings_hides_the_panel_and_comes_back(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    press(cx, "rightPanel.toggle");
    press(cx, "settings.toggle");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()));
    assert!(cx.debug_bounds("right-panel").is_none(), "Settings hides the right panel");
    press(cx, "rightPanel.toggle");
    press(cx, "terminal.toggle");
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()), "neither panel answers while Settings is open");
    press(cx, "settings.toggle");
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "closing Settings goes back to the thread");
    assert!(cx.debug_bounds("right-panel").is_some(), "and the panel it had open");

    press(cx, "settings.toggle");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "Escape closes Settings too");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "and does nothing outside it");
}

#[gpui_kit::test]
fn each_slot_chord_opens_that_workspace(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    press(cx, "workspace.select.2");
    assert_eq!(thread(&shell, cx).as_deref(), Some("c"));
    press(cx, "workspace.select.1");
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"));
    for slot in 3..=9 {
        press(cx, &format!("workspace.select.{slot}"));
        assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "slot {slot} has no workspace, so nothing moves");
    }
}

#[gpui_kit::test]
fn the_workspace_chords_walk_the_workspaces(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    for (chord, want) in
        [("mod+alt+arrowright", "c"), ("mod+alt+arrowright", "a"), ("mod+alt+arrowleft", "c"), ("ctrl+tab", "a"), ("ctrl+shift+tab", "c")]
    {
        press_chord(cx, chord);
        assert_eq!(thread(&shell, cx).as_deref(), Some(want), "{chord}");
    }
}

#[gpui_kit::test]
fn the_thread_chords_walk_the_workspaces_threads(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    for (command, want) in [("thread.next", "b"), ("thread.next", "a"), ("thread.previous", "b")] {
        press(cx, command);
        assert_eq!(thread(&shell, cx).as_deref(), Some(want), "{command}");
    }
}

#[gpui_kit::test]
fn the_tab_chords_walk_the_panel_tabs_while_it_has_focus(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Terminal, window, cx);
        p.add(Pane::Changes, window, cx);
        p.add(Pane::Files, window, cx);
    });
    cx.run_until_parked();
    let tab = cx.debug_bounds("panel-tab-0").unwrap();
    cx.simulate_click(tab.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal));
    press(cx, "rightPanel.nextTab");
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Changes));
    press(cx, "rightPanel.previousTab");
    press(cx, "rightPanel.previousTab");
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Files), "it wraps");
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "ctrl+tab in the panel walks its tabs, not the workspaces");
}

#[gpui_kit::test]
fn a_panel_screen_routes_to_its_tab(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(Pane::Terminal.name().into()), window, cx));
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    assert_eq!(panel.read_with(cx, |p, _| (p.is_open(), p.active())), (true, Some(Pane::Terminal)));
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "the thread stays open beside it");
}

#[gpui_kit::test]
fn the_about_and_hosts_rows_open_their_routes(cx: &mut TestAppContext) {
    use desktop_gpui::screens::app_shell::menu;
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    for (row, name) in [(Box::new(menu::About) as Box<dyn Action>, menu::ABOUT_SCREEN), (Box::new(menu::ShowHosts), menu::HOSTS_SCREEN)] {
        cx.update(|window, cx| window.dispatch_action(row, cx));
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(name.into()));
    }
}

#[gpui_kit::test]
fn the_close_tab_action_closes_the_active_tab(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Browser, window, cx);
        p.add(Pane::Terminal, window, cx);
    });
    cx.dispatch_action(keys::ClosePanelTab);
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Browser]);
    cx.dispatch_action(keys::ClosePanelTab);
    cx.run_until_parked();
    assert!(panel.read_with(cx, |p, _| p.is_open() && p.panes().is_empty()), "the last closed leaves the launcher");
}

/// Chords whose command another screen answers: each reaches the window as its action, for that screen to handle.
#[gpui_kit::test]
fn the_other_screens_chords_reach_the_window_as_their_actions(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let commands = [
        "commandPalette.toggle",
        "files.quickOpen",
        "files.search",
        "chat.new",
        "thread.settle",
        "thread.nextNeedsYou",
        "editor.open",
        "thread.find",
    ];
    fn listen<A: Action>(heard: &std::rc::Rc<std::cell::RefCell<Vec<String>>>, cx: &mut VisualTestContext) {
        let heard = heard.clone();
        cx.update(|_, cx| {
            cx.on_action(move |action: &A, _| heard.borrow_mut().push(action.name().to_owned()));
        });
    }
    listen::<keys::ToggleCommandPalette>(&heard, cx);
    listen::<keys::QuickOpen>(&heard, cx);
    listen::<keys::SearchFiles>(&heard, cx);
    listen::<keys::NewThread>(&heard, cx);
    listen::<keys::SettleThread>(&heard, cx);
    listen::<keys::NextNeedsYou>(&heard, cx);
    listen::<keys::OpenInEditor>(&heard, cx);
    listen::<keys::FindInThread>(&heard, cx);
    for command in commands {
        for rule in DEFAULTS.iter().filter(|r| r.command == command) {
            heard.borrow_mut().clear();
            press_chord(cx, rule.chord);
            let name = keys::action(command).unwrap().name();
            assert_eq!(*heard.borrow(), vec![name.to_owned()], "{} for {command}", rule.chord);
        }
    }
}

#[gpui_kit::test]
fn the_rails_drag_the_columns_within_their_bounds(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, frame::SIDEBAR_DEFAULT);
    let drag = |cx: &mut VisualTestContext, from: f32, to: f32| {
        cx.simulate_mouse_down(point(px(from), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(px(from + 10.), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(px(to), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(point(px(to), px(400.)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    };
    drag(cx, 256., 360.);
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, px(360.));
    drag(cx, 360., 900.);
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, frame::SIDEBAR_MAX);
    drag(cx, 480., 10.);
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, frame::SIDEBAR_MIN);

    press(cx, "rightPanel.toggle");
    let left = cx.debug_bounds("right-panel").unwrap().origin.x;
    drag(cx, f32::from(left), f32::from(left) - 100.);
    assert_eq!(cx.debug_bounds("right-panel").unwrap().size.width, DEFAULT_WIDTH + px(100.));
    drag(cx, f32::from(left) - 100., 100.);
    // 1280 less the 220 px sidebar is the row; the centre keeps 360 of it.
    assert_eq!(cx.debug_bounds("right-panel").unwrap().size.width, px(1280. - 220. - 360.));
}

#[gpui_kit::test]
fn the_frame_rows_are_the_web_apps(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let header = cx.debug_bounds("header-row").unwrap();
    let frame_row = cx.debug_bounds("sidebar-frame-row").unwrap();
    assert_eq!((header.size.height, frame_row.size.height), (frame::TOPBAR, frame::TOPBAR));
    assert_eq!(header.origin.x, frame::SIDEBAR_DEFAULT);
    let center = cx.debug_bounds("center").unwrap();
    assert_eq!(center.size.width, px(1280.) - frame::SIDEBAR_DEFAULT);
    press(cx, "rightPanel.toggle");
    let bar = cx.debug_bounds("panel-bar").unwrap();
    assert_eq!(bar.size.height, frame::TOPBAR);
    assert_eq!(cx.debug_bounds("center").unwrap().size.width, px(1280.) - frame::SIDEBAR_DEFAULT - DEFAULT_WIDTH);
}

#[test]
fn the_sizes_are_the_web_apps() {
    let src = |path: &str| std::fs::read_to_string(format!("{}/../../web/src/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let number = |text: &str, name: &str| -> f32 {
        let at = text.find(&format!("{name} =")).unwrap_or_else(|| panic!("no {name}"));
        text[at..].split('=').nth(1).unwrap().trim().split(|c: char| !c.is_ascii_digit() && c != '.').next().unwrap().parse().unwrap()
    };
    let widths = src("shell/sidebarWidth.ts");
    assert_eq!(px(number(&widths, "SIDEBAR_DEFAULT_WIDTH")), frame::SIDEBAR_DEFAULT);
    assert_eq!(px(number(&widths, "SIDEBAR_MIN_WIDTH")), frame::SIDEBAR_MIN);
    assert_eq!(px(number(&widths, "SIDEBAR_MAX_WIDTH")), frame::SIDEBAR_MAX);
    let layout = src("rightPanelLayout.ts");
    assert_eq!(px(number(&layout, "RIGHT_PANEL_SHEET_MAX_VIEWPORT")), frame::SHEET_MAX_VIEWPORT);
    assert_eq!(px(number(&layout, "RIGHT_PANEL_MIN_WIDTH")), desktop_gpui::screens::app_shell::panel::MIN_WIDTH);
    assert_eq!(px(number(&layout, "CENTER_COLUMN_MIN_WIDTH")), frame::CENTER_MIN);
    assert_eq!(px(number(&src("components/preview/PreviewPanelShell.tsx"), "PREVIEW_PANEL_DEFAULT_WIDTH")), DEFAULT_WIDTH);
    let css = src("index.css");
    let topbar = css.lines().find(|l| l.trim_start().starts_with("--workspace-topbar-height:")).unwrap();
    assert!(topbar.contains("52px"), "{topbar}");
}

/// Polls `done` while the host answers on its own threads.
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

fn set(host: &common::Throwaway, patch: serde_json::Value) {
    let rt = runtime();
    rt.block_on(async {
        let client: HostClient = host.client().await;
        client.call(common::ask::<PreferencesSetRequest>(json!({ "patch": patch }))).await.unwrap();
    });
}

fn get(host: &common::Throwaway) -> serde_json::Value {
    let rt = runtime();
    rt.block_on(async {
        let client: HostClient = host.client().await;
        let reply = client.call(common::ask::<PreferencesGetRequest>(json!({}))).await.unwrap();
        serde_json::to_value(reply.preferences).unwrap()
    })
}

#[gpui_kit::test]
fn it_reads_the_hosts_width_chords_and_theme(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    set(&host, json!({ "sidebarWidth": 333, "keybindings": { "sidebar.toggle": "mod+shift+s" }, "theme": "dark", "transparency": false }));
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host's width", |cx| shell.read_with(cx, |s, _| s.sidebar_width()) == px(333.));
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, px(333.));
    assert_eq!(cx.update(|_, cx| desktop_gpui::ui::theme::current(cx).id), "graphite", "dark draws the dark pick, graphite");
    assert!(!shell.read_with(cx, |s, _| s.look().glass), "transparency off draws no glass");

    for chord in ["mod+b", "mod+shift+b"] {
        press_chord(cx, chord);
        assert!(shell.read_with(cx, |s, _| s.sidebar_open()), "the person's chord replaced both defaults, {chord} among them");
    }
    press_chord(cx, "mod+shift+s");
    assert!(!shell.read_with(cx, |s, _| s.sidebar_open()), "and is the one that toggles");
}

#[gpui_kit::test]
fn a_drag_writes_the_sidebar_width_to_the_host(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    cx.simulate_mouse_down(point(px(256.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(266.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(300.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(point(px(300.), px(400.)), MouseButton::Left, Modifiers::none());
    let deadline = Instant::now() + Duration::from_secs(30);
    while get(&host).get("sidebarWidth").and_then(|w| w.as_i64()) != Some(300) {
        assert!(Instant::now() < deadline, "the host never held 300: {:?}", get(&host).get("sidebarWidth"));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn it_follows_preferences_another_window_changes(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    assert_eq!(cx.update(|_, cx| desktop_gpui::ui::theme::current(cx).id), "paper");
    assert!(shell.read_with(cx, |s, _| s.look().glass), "a host that says nothing of transparency leaves it on");
    set(
        &host,
        json!({ "transparency": false, "sidebarWidth": 400, "theme": "dark", "darkTheme": "pitch", "keybindings": { "settings.toggle": "mod+shift+," } }),
    );
    until(cx, "the pushed width", |cx| shell.read_with(cx, |s, _| s.sidebar_width()) == px(400.));
    assert!(!shell.read_with(cx, |s, _| s.look().glass), "a pushed solid setting takes the glass away");
    assert_eq!(cx.update(|_, cx| desktop_gpui::ui::theme::current(cx).id), "pitch");
    press_chord(cx, "mod+,");
    assert_ne!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()), "the old chord is gone");
    press_chord(cx, "mod+shift+,");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()));
    press_chord(cx, "mod+shift+,");

    set(&host, json!({ "keybindings": { "settings.toggle": null } }));
    until(cx, "the default chord back", |cx| shell.read_with(cx, |s, _| s.chords("settings.toggle")) == ["mod+,"]);
    press_chord(cx, "mod+shift+,");
    assert_ne!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()), "the person's chord is gone");
    press_chord(cx, "mod+,");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()), "and the default answers again");
}
