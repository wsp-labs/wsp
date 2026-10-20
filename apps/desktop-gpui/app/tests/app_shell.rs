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
use desktop_gpui::screens::browser::store::Store;
use desktop_gpui::screens::{self, entry, Host, Opened, Pane, Parts, Place, Route, Screen, THREAD};
use desktop_gpui::shell::{Launch, Shell, SETTINGS};
use desktop_gpui::sidebar::{Asked, ThreadItem};
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, Action, AnyView, App, AppContext, Entity, Modifiers, MouseButton, SharedString, TestAppContext,
    VisualTestContext, Window,
};
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
        desktop_gpui::screens::palette::init(cx);
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
    let dir = std::env::temp_dir().join("desktop-gpui-shell-no-host");
    // With no HOME of its own the client reads this user's ~/.wsp and dials the one host held there.
    let home = [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect();
    open(cx, dir.join("state.json"), home)
}

/// Threads in two workspaces, as the host would list them, and the first one open.
fn with_threads(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let item = |w: &str, t: &str| ThreadItem { workspace_id: w.into(), thread_id: t.into(), project: "p".into(), title: t.into() };
    shell.update_in(cx, |shell, window, cx| {
        shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("w1", "a"), item("w1", "b"), item("w2", "c")], cx));
        shell.workspace_runs("w1", true, window, cx);
        shell.workspace_runs("w2", true, window, cx);
        shell.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx);
    });
    cx.run_until_parked();
}

/// `with_threads`' threads as the host's rows too, so the sidebar's own list and the walks it draws hold them.
fn seeded(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let row = |i: f64, w: &str, t: &str| {
        json!({ "id": format!("r-{t}"), "threadId": t, "workspaceId": w, "harness": "claude", "status": "completed",
            "prompt": t, "startedAt": now - 120_000. - i * 1_000., "endedAt": now - 60_000., "readAt": now - 60_000. })
    };
    let snapshot = desktop_gpui::sidebar::host::Snapshot {
        sessions: vec![row(0., "w1", "a"), row(1., "w1", "b"), row(2., "w2", "c")],
        workspaces: ["w1", "w2"].map(|w| json!({ "id": w, "name": w, "kind": "local" })).to_vec(),
        ..Default::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(desktop_gpui::sidebar::host::Change::Whole(Box::new(snapshot)), cx));
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

/// The launcher's keys (`RightPanelTabs.tsx`): down and right light the next pane that opens here, up and left the one
/// before, both wrapping from none lit; Enter opens the lit pane and does nothing with none; the pointer lights a row.
#[gpui_kit::test]
fn the_launchers_arrows_move_its_highlight_and_enter_opens_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let opened = |keys: &str, cx: &mut VisualTestContext| {
        press(cx, "rightPanel.toggle");
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
        let panes = panel.read_with(cx, |p, _| p.panes());
        panel.update_in(cx, |p, window, cx| {
            while !p.panes().is_empty() {
                p.close(0, window, cx);
            }
        });
        press(cx, "rightPanel.toggle");
        panes
    };
    assert_eq!(opened("enter", cx), [], "nothing lit, Enter opens nothing");
    assert_eq!(opened("down enter", cx), [Pane::Browser]);
    assert_eq!(opened("right right enter", cx), [Pane::Terminal]);
    assert_eq!(opened("up enter", cx), [Pane::Slate], "up from none lit is the last that opens");
    assert_eq!(opened("left left enter", cx), [Pane::Agents]);
    assert_eq!(opened("up down enter", cx), [Pane::Browser], "it wraps");

    press(cx, "rightPanel.toggle");
    let launcher = cx.debug_bounds("panel-launcher").unwrap();
    // Each tile is 54 px with 8 px between; the third is Changes, the fourth Files.
    let files = point(launcher.center().x, launcher.top() + px(3. * 62. + 27.));
    cx.simulate_mouse_move(files, None, Modifiers::none());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), [Pane::Files], "the pointer lights the row Enter opens");
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

/// The palette's Next and Previous task: a step at once along the sidebar's tasks, wrapping.
#[gpui_kit::test]
fn a_task_step_walks_the_workspaces(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    for (step, want) in [(1, "c"), (1, "a"), (-1, "c"), (1, "a"), (-1, "c")] {
        cx.dispatch_action(keys::StepWorkspace(step));
        cx.run_until_parked();
        assert_eq!(thread(&shell, cx).as_deref(), Some(want), "a step of {step}");
    }
}

/// The palette's Next and Previous thread: a step at once along the open task's threads, as the thread chords walk them.
#[gpui_kit::test]
fn a_thread_step_walks_the_open_tasks_threads(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    seeded(&shell, cx);
    for (step, want) in [(1, "b"), (1, "a"), (-1, "b")] {
        cx.dispatch_action(keys::StepThread(step));
        cx.run_until_parked();
        assert_eq!(thread(&shell, cx).as_deref(), Some(want), "a step of {step}");
    }
}

/// Each switch chord, held, puts the switcher up over the window, and letting its modifiers go opens the thread it
/// stands on.
#[gpui_kit::test]
fn the_switch_chords_open_the_switcher_and_letting_go_lands_on_its_card(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    seeded(&shell, cx);
    let switcher = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| desktop_gpui::screens::palette::overlays::of(window, cx).and_then(|o| o.read(cx).switcher.clone()))
    };
    let commands = ["workspace.next", "workspace.previous", "thread.next", "thread.previous"];
    for rule in DEFAULTS.iter().filter(|r| commands.contains(&r.command)) {
        let typed = keys::keystroke(rule.chord, MAC).unwrap();
        cx.simulate_modifiers_change(gpui_kit::Keystroke::parse(&typed).unwrap().modifiers);
        cx.simulate_keystrokes(&typed);
        cx.run_until_parked();
        let open = switcher(cx).unwrap_or_else(|| panic!("{} puts the switcher up", rule.chord));
        let lands = open.read_with(cx, |s, _| s.highlighted().cloned()).expect("a card is highlighted");
        assert_ne!(Some(lands.thread_id.clone()), thread(&shell, cx), "{} steps off the open thread", rule.chord);
        cx.simulate_modifiers_change(Modifiers::none());
        cx.run_until_parked();
        assert!(switcher(cx).is_none(), "letting {} go takes it down", rule.chord);
        assert_eq!(thread(&shell, cx), Some(lands.thread_id), "letting {} go opens its card", rule.chord);
    }
}

#[gpui_kit::test]
fn the_tab_chords_walk_the_panel_tabs_while_it_has_focus(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Terminal, None, window, cx);
        p.add(Pane::Changes, None, window, cx);
        p.add(Pane::Files, None, window, cx);
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

/// A page no screen is registered under, as a route a screen has not landed on yet names one.
const NO_SCREEN: &str = "no-screen";

/// About is AppKit's own panel on a Mac (`role: "about"`) and no row elsewhere, so it opens no page in the window.
#[gpui_kit::test]
fn the_about_row_opens_no_page(cx: &mut TestAppContext) {
    use desktop_gpui::screens::app_shell::menu;
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    cx.update(|window, cx| window.dispatch_action(Box::new(menu::About), cx));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() });
}

#[gpui_kit::test]
fn a_page_with_no_screen_yet_hides_the_panel_and_escape_does_not_leave_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    press(cx, "rightPanel.toggle");
    assert!(cx.debug_bounds("right-panel").is_some());
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(NO_SCREEN.into()), window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("right-panel").is_none(), "a page hides the panel, as Settings does");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(NO_SCREEN.into()), "Escape leaves the page alone");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("right-panel").is_some(), "back on the thread, its panel");
}

#[gpui_kit::test]
fn the_close_tab_action_closes_the_active_tab(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Browser, None, window, cx);
        p.add(Pane::Terminal, None, window, cx);
    });
    cx.dispatch_action(keys::ClosePanelTab);
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Browser]);
    cx.dispatch_action(keys::ClosePanelTab);
    cx.run_until_parked();
    assert!(panel.read_with(cx, |p, _| p.is_open() && p.panes().is_empty()), "the last closed leaves the launcher");
}

#[gpui_kit::test]
fn mod_w_closes_the_active_panel_tab_only_with_focus_in_the_panel(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Changes, None, window, cx);
        p.add(Pane::Files, None, window, cx);
    });
    cx.run_until_parked();
    press(cx, "rightPanel.closeTab");
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Changes, Pane::Files], "focus outside the panel closes nothing");
    let tab = cx.debug_bounds("panel-tab-1").unwrap();
    cx.simulate_click(tab.center(), Modifiers::none());
    press(cx, "rightPanel.closeTab");
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Changes], "a press in the panel closes its active tab");
    press(cx, "rightPanel.closeTab");
    assert!(panel.read_with(cx, |p, _| p.is_open() && p.panes().is_empty()), "the focus stays in the panel for the next one");
}

/// `panes.ts`' Files pane: the tree's tab is "Files" under a folder, a file's tab its name under a file glyph.
#[gpui_kit::test]
fn a_file_tab_is_titled_by_its_name_under_a_file_glyph(cx: &mut TestAppContext) {
    use gpui_kit::assets::IconName;
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Files, None, window, cx);
        p.add(Pane::Files, Some("file:src/app/main.rs".into()), window, cx);
    });
    cx.run_until_parked();
    let faces = panel.read_with(cx, |p, cx| p.faces(cx));
    assert_eq!(faces, vec![("Files".into(), IconName::Folder), ("main.rs".into(), IconName::File)]);
}

#[gpui_kit::test]
fn the_panel_keeps_a_hundred_threads_it_was_used_on_and_the_computers_own(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    let go = |shell: &Entity<Shell>, route: Route, cx: &mut VisualTestContext| {
        shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
        cx.run_until_parked();
    };
    let at = |t: usize| Route::Thread { workspace_id: "w".into(), thread_id: format!("t{t}") };
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let open = |cx: &mut VisualTestContext| panel.read_with(cx, |p, _| p.is_open());
    press(cx, "rightPanel.toggle");
    go(&shell, at(0), cx);
    press(cx, "rightPanel.toggle");
    for t in 1..=100 {
        go(&shell, at(t), cx);
    }
    go(&shell, at(0), cx);
    assert!(open(cx), "threads whose panel was never used take no place among the hundred");
    go(&shell, Route::Empty, cx);
    assert!(open(cx), "the computer's own panel is kept, as HERE_KEY is");
    for t in 1..=100 {
        go(&shell, at(t), cx);
        press(cx, "rightPanel.toggle");
    }
    go(&shell, at(0), cx);
    assert!(!open(cx), "past a hundred used, the oldest is forgotten");
}

/// Chords whose command another screen answers: each reaches the window as its action, for that screen to handle.
#[gpui_kit::test]
fn the_other_screens_chords_reach_the_window_as_their_actions(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    // The finder's chords are the shell's own: `thread_files_card`'s finder chord test holds them.
    let commands = ["thread.nextNeedsYou", "thread.find"];
    fn listen<A: Action>(heard: &std::rc::Rc<std::cell::RefCell<Vec<String>>>, cx: &mut VisualTestContext) {
        let heard = heard.clone();
        cx.update(|_, cx| {
            cx.on_action(move |action: &A, _| heard.borrow_mut().push(action.name().to_owned()));
        });
    }
    listen::<keys::NextNeedsYou>(&heard, cx);
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

/// Chords the shell answers itself. With no host, Settle and Open in editor have nothing to write to, so what shows is
/// that the shell takes them (no listener past it hears them) and that the window stays where it was.
#[gpui_kit::test]
fn the_shells_own_chords_reach_its_handlers(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let palette = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| desktop_gpui::screens::palette::overlays::of(window, cx).is_some_and(|o| o.read(cx).palette.is_some()))
    };
    for rule in DEFAULTS.iter().filter(|r| r.command == "commandPalette.toggle") {
        press_chord(cx, rule.chord);
        assert!(palette(cx), "{} opens the palette", rule.chord);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!palette(cx), "Escape closes it");
    }

    let heard = std::rc::Rc::new(std::cell::Cell::new(0));
    let (settles, opens) = (heard.clone(), heard.clone());
    cx.update(|_, cx| {
        cx.on_action(move |_: &keys::SettleThread, _| settles.set(settles.get() + 1));
        cx.on_action(move |_: &keys::OpenInEditor, _| opens.set(opens.get() + 1));
    });
    press(cx, "thread.settle");
    press(cx, "editor.open");
    assert_eq!(heard.get(), 0, "the shell takes Settle and Open in editor before anything past it");
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"), "and neither moves the window");

    press(cx, "chat.new");
    assert_eq!(
        shell.read_with(cx, |s, _| s.route().clone()),
        Route::Screen("composer:w1".into()),
        "New thread opens the composer on the task"
    );
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

/// `getPreviewPanelMaxWidth` and its clamp: never under 360, never over 70% of the window nor the row less the
/// centre's 360, the floor winning where the two caps fall under it.
#[gpui_kit::test]
fn the_panel_stops_at_its_360_px_floor_and_its_caps_at_every_width(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let drag = |cx: &mut VisualTestContext, to: f32| {
        let from = f32::from(cx.debug_bounds("right-panel").unwrap().origin.x);
        cx.simulate_mouse_down(point(px(from), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(px(from - 10.), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(px(to), px(400.)), MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(point(px(to), px(400.)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    };
    let width = |cx: &mut VisualTestContext| cx.debug_bounds("right-panel").unwrap().size.width;
    press(cx, "rightPanel.toggle");
    assert_eq!(width(cx), DEFAULT_WIDTH);
    drag(cx, 1279.);
    assert_eq!(width(cx), px(360.), "dragged to the window's edge it stops at its floor");

    // At 1000 px the row is 744 beside the 256 px sidebar: its cap, 384, is under 70% of the window.
    cx.simulate_resize(size(px(1000.), px(800.)));
    cx.run_until_parked();
    drag(cx, 0.);
    assert_eq!(width(cx), px(1000. - 256. - 360.));
    drag(cx, 999.);
    assert_eq!(width(cx), px(360.));
    // Beside a 280 px sidebar the row's cap is 360 itself; at 2000 px 70% is the cap, 1400 against the row's 1384.
    assert_eq!(frame::clamp_panel(px(900.), px(1000.), px(720.)), px(360.));
    assert_eq!(frame::clamp_panel(px(100.), px(1000.), px(600.)), px(360.), "the floor wins over a cap under it");
    assert_eq!(frame::clamp_panel(px(1900.), px(2000.), px(2000.)), px(1400.));
    assert_eq!(frame::clamp_panel(px(1900.), px(2000.), px(1744.5)), px(1384.), "the row is floored first");
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
    assert!(topbar.contains(&format!("{}px", f32::from(frame::TOPBAR))), "{topbar}");
}

/// Moves the clock `ms` on and draws the frame there, as a sheet's motion samples it.
fn tick(cx: &mut VisualTestContext, ms: u64) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

/// Both sheets come in as `SheetPopup` does, 200 ms of a 32 px slide from their side and a fade; the sidebar's goes out
/// the same way. Under reduced motion each stands where it ends on its first frame.
#[gpui_kit::test]
fn the_sheets_slide_in_and_out_over_200_ms_and_stand_still_under_reduced_motion(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    with_threads(&shell, cx);
    cx.simulate_resize(size(px(760.), px(800.)));
    cx.run_until_parked();
    let left = |cx: &mut VisualTestContext| cx.debug_bounds("sidebar-sheet").map(|b| f32::from(b.origin.x));
    press_chord(cx, "mod+b");
    assert_eq!(left(cx), Some(-32.), "it starts 32 px out");
    tick(cx, 100);
    let midway = left(cx).unwrap();
    assert!(midway > -32. && midway < 0., "on its way at 100 ms: {midway}");
    tick(cx, 150);
    assert_eq!(left(cx), Some(0.), "in by 200 ms");
    press_chord(cx, "mod+b");
    tick(cx, 100);
    let midway = left(cx).expect("still drawn on its way out");
    assert!(midway > -32. && midway < 0., "on its way out at 100 ms: {midway}");
    tick(cx, 150);
    assert_eq!(left(cx), None, "gone by 200 ms");

    cx.update(|_, cx| cx.set_reduce_motion(true));
    press_chord(cx, "mod+b");
    assert_eq!(left(cx), Some(0.), "under reduced motion it stands in on its first frame");
    press_chord(cx, "mod+b");
    assert_eq!(left(cx), None, "and goes at once");

    cx.update(|_, cx| cx.set_reduce_motion(false));
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    let right = |cx: &mut VisualTestContext| cx.debug_bounds("panel-sheet").map(|b| f32::from(b.right()));
    press(cx, "rightPanel.toggle");
    assert_eq!(right(cx), Some(932.), "the panel's sheet starts 32 px out at the right");
    tick(cx, 250);
    assert_eq!(right(cx), Some(900.));
    press(cx, "rightPanel.toggle");
    cx.update(|_, cx| cx.set_reduce_motion(true));
    press(cx, "rightPanel.toggle");
    assert_eq!(right(cx), Some(900.), "under reduced motion it stands in on its first frame");
}

/// The panel's sheet fades out over 200 ms still holding its pane, and only once it has gone does the panel shut and
/// let the pane's screen go, so a shut sheet holds nothing; asked again or handed a pane on its way out it comes back,
/// and under reduced motion it goes at once.
#[gpui_kit::test]
fn the_panels_sheet_fades_out_holding_its_panes_then_lets_them_go(cx: &mut TestAppContext) {
    screens::add("terminal", entry::<ShellProbe>());
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    cx.update(|_, cx| cx.set_reduce_motion(false));
    with_threads(&shell, cx);
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| p.add(Pane::Terminal, Some("terminal:7".into()), window, cx));
    tick(cx, 250);
    let right = |cx: &mut VisualTestContext| cx.debug_bounds("panel-sheet").map(|b| f32::from(b.right()));
    assert_eq!(right(cx), Some(900.));
    let pane = panel.read_with(cx, |p, _| p.view()).expect("the tab's screen is built").downgrade();

    press(cx, "rightPanel.toggle");
    tick(cx, 100);
    let midway = right(cx).expect("still drawn on its way out");
    assert!(midway > 900. && midway < 932., "on its way out at 100 ms: {midway}");
    assert!(cx.debug_bounds("probe-shell").is_some(), "its pane is drawn while it fades");
    assert!(pane.upgrade().is_some(), "and still built");
    tick(cx, 150);
    assert_eq!(right(cx), None, "gone by 200 ms");
    assert!(cx.debug_bounds("probe-shell").is_none());
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "the panel shuts once the sheet has gone");
    assert!(pane.upgrade().is_none(), "and lets its pane's screen go");

    press(cx, "rightPanel.toggle");
    tick(cx, 250);
    press(cx, "rightPanel.toggle");
    tick(cx, 100);
    press(cx, "rightPanel.toggle");
    tick(cx, 250);
    assert_eq!(right(cx), Some(900.), "asked again on its way out, it comes back");
    assert!(panel.read_with(cx, |p, _| p.is_open()) && cx.debug_bounds("probe-shell").is_some());

    press(cx, "rightPanel.toggle");
    tick(cx, 100);
    panel.update_in(cx, |p, window, cx| p.add(Pane::Terminal, Some("terminal:8".into()), window, cx));
    tick(cx, 250);
    assert_eq!(right(cx), Some(900.), "a pane opened on its way out keeps it open");
    assert!(panel.read_with(cx, |p, _| p.is_open()));

    cx.update(|_, cx| cx.set_reduce_motion(true));
    press(cx, "rightPanel.toggle");
    assert_eq!(right(cx), None, "under reduced motion it goes at once");
    assert!(!panel.read_with(cx, |p, _| p.is_open()));
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
fn the_persons_palette_chord_shuts_the_palette_it_opened(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    set(&host, json!({ "keybindings": { "commandPalette.toggle": "mod+shift+y" } }));
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the person's chord", |cx| shell.read_with(cx, |s, _| s.chords("commandPalette.toggle")) == ["mod+shift+y"]);
    let open = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| desktop_gpui::screens::palette::overlays::of(window, cx).is_some_and(|o| o.read(cx).palette.is_some()))
    };
    press_chord(cx, "mod+shift+y");
    assert!(open(cx), "the person's chord opens the palette");
    press_chord(cx, "mod+k");
    assert!(open(cx), "a default the person moved shuts nothing");
    press_chord(cx, "mod+shift+y");
    assert!(!open(cx), "and their chord shuts it");
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

#[gpui_kit::test]
fn a_press_inside_the_sheet_stays_in_the_sheet(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    cx.simulate_resize(size(px(900.), px(800.)));
    cx.run_until_parked();
    press(cx, "rightPanel.toggle");
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let bar = cx.debug_bounds("panel-bar").expect("the panel shows as a sheet");
    cx.simulate_click(bar.center(), Modifiers::none());
    assert!(panel.read_with(cx, |p, _| p.is_open()), "a press on the sheet's bar does not reach the backdrop under it");
    // The header's panel toggle, 28 px wide at 21 px from the right, lies under the sheet's bar.
    let toggle = point(px(900.) - frame::HEADER_INSET - px(15.), px(26.));
    assert!(cx.debug_bounds("right-panel").unwrap().contains(&toggle));
    cx.simulate_click(toggle, Modifiers::none());
    assert!(panel.read_with(cx, |p, _| p.is_open()), "nor the header's toggle under it");
    cx.simulate_keystrokes("escape");
    tick(cx, 250);
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "Escape closes the sheet");
}

thread_local! {
    /// Each probe screen built or moved, by its name, with what it was opened on.
    static PROBED: std::cell::RefCell<Vec<(&'static str, &'static str, Opened)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn probed() -> Vec<(&'static str, &'static str, Opened)> {
    PROBED.with(|p| p.borrow().clone())
}

/// A view a probe hands the frame, found again by its selector.
struct Part(&'static str);

impl Render for Part {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let name = self.0;
        div().debug_selector(move || name.into()).w(px(40.)).h(px(20.))
    }
}

macro_rules! probe {
    ($ty:ident, $name:expr, $place:expr, $selector:literal) => {
        struct $ty;
        impl Render for $ty {
            fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                div().debug_selector(|| $selector.into()).size_full()
            }
        }
        impl Screen for $ty {
            const NAME: &'static str = $name;
            const PLACE: Place = $place;
            fn new(_: Host, opened: Opened, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> Self {
                PROBED.with(|p| p.borrow_mut().push(($name, "new", opened)));
                $ty
            }
            fn show(&mut self, opened: Opened, _: &mut Window, _: &mut gpui_kit::Context<Self>) {
                PROBED.with(|p| p.borrow_mut().push(($name, "show", opened)));
            }
            fn closed(_: &Host, opened: Opened, _: &mut App) {
                PROBED.with(|p| p.borrow_mut().push(($name, "closed", opened)));
            }
            fn parts(_: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
                let part = |name: &'static str, cx: &mut App| Some(AnyView::from(cx.new(|_| Part(name))));
                Parts {
                    sidebar: part(concat!($selector, "-sidebar"), cx),
                    crumbs: part(concat!($selector, "-crumbs"), cx),
                    actions: part(concat!($selector, "-actions"), cx),
                    title: Some(std::rc::Rc::new(|_: &App| SharedString::from(concat!($selector, " tab")))),
                }
            }
        }
    };
}

probe!(PageProbe, "probe-page", Place::Main, "probe-page");
probe!(ShellProbe, "terminal", Place::Panel(Pane::Terminal), "probe-shell");
probe!(ThreadProbe, THREAD, Place::Main, "probe-thread");
probe!(BrowserProbe, "browser", Place::Panel(Pane::Browser), "probe-browser");

fn opened(workspace: &str, thread: &str, tab: Option<&str>, arg: Option<&str>) -> Opened {
    Opened {
        workspace_id: Some(workspace.to_owned()),
        thread_id: Some(thread.to_owned()),
        tab: tab.map(|t| SharedString::from(t.to_owned())),
        arg: arg.map(|a| SharedString::from(a.to_owned())),
    }
}

/// Screens mounted through the registry, as a screen's own arm in screens::screen mounts it, once in each place.
#[gpui_kit::test]
fn a_registered_screen_mounts_in_its_place_on_what_it_was_opened_on(cx: &mut TestAppContext) {
    screens::add("probe-page", entry::<PageProbe>());
    screens::add("terminal", entry::<ShellProbe>());
    screens::add(THREAD, entry::<ThreadProbe>());
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    with_threads(&shell, cx);
    let go = |route: Route, cx: &mut VisualTestContext| {
        shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
        cx.run_until_parked();
    };

    // The thread view: built once for the first thread, told of each switch after it, and drawn in the main column.
    go(Route::Thread { workspace_id: "w1".into(), thread_id: "b".into() }, cx);
    assert_eq!(
        probed(),
        [(THREAD, "new", opened("w1", "a", None, None)), (THREAD, "show", opened("w1", "b", None, None))],
        "a switch moves the kept thread view rather than build another"
    );
    assert!(cx.debug_bounds("probe-thread").is_some());
    assert!(cx.debug_bounds("probe-thread-actions").is_some(), "its actions ride the header");
    assert!(cx.debug_bounds("probe-thread-crumbs").is_some(), "its crumbs stand in the shell's");
    let center = cx.debug_bounds("center").unwrap();
    assert!(center.contains(&cx.debug_bounds("probe-thread").unwrap().center()));

    // A pane: its route opens a tab of its own id beside the thread, on the thread's workspace.
    PROBED.with(|p| p.borrow_mut().clear());
    go(Route::Screen("terminal:terminal:7".into()), cx);
    assert_eq!(probed(), [("terminal", "new", opened("w1", "b", Some("terminal:7"), None))]);
    assert_eq!(thread(&shell, cx).as_deref(), Some("b"), "the thread stays open beside it");
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    assert!(cx.debug_bounds("right-panel").unwrap().contains(&cx.debug_bounds("probe-shell").unwrap().center()));
    panel.update_in(cx, |p, window, cx| p.add(Pane::Terminal, None, window, cx));
    cx.run_until_parked();
    let tabs = panel.read_with(cx, |p, _| p.tabs());
    assert_eq!(tabs.len(), 2, "a second Terminal is a shell of its own: {tabs:?}");
    assert!(tabs.iter().all(|(pane, _)| *pane == Pane::Terminal) && tabs[0].1 != tabs[1].1);
    PROBED.with(|p| p.borrow_mut().clear());
    go(Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, cx);
    go(Route::Thread { workspace_id: "w1".into(), thread_id: "b".into() }, cx);
    panel.update_in(cx, |p, window, cx| p.activate(0, window, cx));
    cx.run_until_parked();
    assert!(
        probed().contains(&("terminal", "new", opened("w1", "b", Some("terminal:7"), None))),
        "back on the thread the pane is built again on the same tab, to find its session: {:?}",
        probed()
    );
    assert!(!probed().iter().any(|(_, what, _)| *what == "closed"), "a thread switch closes no tab: {:?}", probed());
    panel.update_in(cx, |p, window, cx| p.close(0, window, cx));
    cx.run_until_parked();
    assert!(
        probed().contains(&("terminal", "closed", opened("w1", "b", Some("terminal:7"), None))),
        "a closed tab tells its screen, so its shell can go: {:?}",
        probed()
    );

    // A shut panel builds none of its tabs, as the web app mounts none, and builds the open one as it opens.
    let built = || probed().iter().any(|(name, what, _)| *name == "terminal" && *what == "new");
    panel.update_in(cx, |p, window, cx| p.toggle(window, cx));
    go(Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, cx);
    PROBED.with(|p| p.borrow_mut().clear());
    go(Route::Thread { workspace_id: "w1".into(), thread_id: "b".into() }, cx);
    assert!(!built(), "a shut panel builds no pane on the way back: {:?}", probed());
    panel.update_in(cx, |p, window, cx| p.toggle(window, cx));
    cx.run_until_parked();
    assert!(built(), "and builds the open tab as it opens");

    // A page: in the main column with its sidebar, crumbs and actions, on its argument, and the panel hidden.
    PROBED.with(|p| p.borrow_mut().clear());
    go(Route::Screen("probe-page:usage".into()), cx);
    assert_eq!(probed(), [("probe-page", "new", opened("w1", "b", None, Some("usage")))]);
    for part in ["probe-page", "probe-page-sidebar", "probe-page-crumbs", "probe-page-actions"] {
        assert!(cx.debug_bounds(part).is_some(), "{part} is drawn");
    }
    assert!(cx.debug_bounds("sidebar-column").unwrap().contains(&cx.debug_bounds("probe-page-sidebar").unwrap().center()));
    assert!(cx.debug_bounds("header-actions").unwrap().contains(&cx.debug_bounds("probe-page-actions").unwrap().center()));
    assert!(cx.debug_bounds("right-panel").is_none(), "a page hides the panel");
    assert!(cx.debug_bounds("probe-thread").is_none());

    // The open page is told its new argument rather than built again, as Settings moves between its pages.
    go(Route::Screen("probe-page:general".into()), cx);
    assert_eq!(
        probed(),
        [("probe-page", "new", opened("w1", "b", None, Some("usage"))), ("probe-page", "show", opened("w1", "b", None, Some("general")))]
    );
    assert!(cx.debug_bounds("probe-page-sidebar").is_some());

    // With no thread on screen the kept thread view is told so, and can let the last one go.
    go(Route::Empty, cx);
    assert!(probed().contains(&(THREAD, "show", Opened::default())), "{:?}", probed());
}

/// A shut panel, or one a page hides, holds no pane's screen, as the web mounts none; it builds the active one again,
/// on the same tab, as it shows.
#[gpui_kit::test]
fn a_shut_or_hidden_panel_lets_its_panes_go_and_builds_the_active_one_again(cx: &mut TestAppContext) {
    screens::add("terminal", entry::<ShellProbe>());
    screens::add("browser", entry::<BrowserProbe>());
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| p.add(Pane::Terminal, Some("terminal:7".into()), window, cx));
    cx.run_until_parked();
    let built = || {
        probed().iter().filter(|(name, what, o)| *name == "terminal" && *what == "new" && o.tab.as_deref() == Some("terminal:7")).count()
    };
    assert_eq!(built(), 1);
    let released = |cx: &mut VisualTestContext| {
        let weak = panel.read_with(cx, |p, _| p.view()).expect("the tab's screen is built").downgrade();
        move |cx: &mut VisualTestContext| {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            weak.upgrade().is_none()
        }
    };

    let gone = released(cx);
    press(cx, "rightPanel.toggle");
    assert!(gone(cx), "a shut panel lets its pane's screen go");
    press(cx, "rightPanel.toggle");
    assert_eq!(built(), 2, "and builds it again on the same tab as it opens");
    assert!(!probed().iter().any(|(_, what, _)| *what == "closed"), "a hide is not a close: {:?}", probed());

    let gone = released(cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(SETTINGS.into()), window, cx));
    assert!(gone(cx), "Settings hides the panel and lets its pane's screen go");
    assert!(panel.read_with(cx, |p, _| p.view().is_none()));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(thread(&shell, cx).as_deref(), Some("a"));
    assert_eq!(built(), 3, "back on the thread its panel builds the active tab again");
    assert!(cx.debug_bounds("probe-shell").is_some());
    assert!(!probed().iter().any(|(_, what, _)| *what == "closed"), "{:?}", probed());

    panel.update_in(cx, |p, window, cx| p.add(Pane::Browser, Some("browser:b".into()), window, cx));
    cx.run_until_parked();
    let gone = released(cx);
    press(cx, "preview.toggle");
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "the browser chord on the open Browser tab shuts the panel");
    assert!(gone(cx), "and lets its pane's screen go");
}

#[gpui_kit::test]
fn each_browser_the_add_menu_opens_is_a_page_of_its_own_and_the_toggle_goes_to_the_first(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Browser, None, window, cx);
        p.add(Pane::Changes, None, window, cx);
        p.add(Pane::Browser, None, window, cx);
    });
    let tabs = panel.read_with(cx, |p, _| p.tabs());
    assert_eq!(tabs.iter().filter(|(pane, _)| *pane == Pane::Browser).count(), 2, "openNewBrowser makes a page each time: {tabs:?}");
    panel.update_in(cx, |p, window, cx| {
        p.activate(1, window, cx);
        p.toggle_pane(Pane::Browser, window, cx);
    });
    assert_eq!(
        panel.read_with(cx, |p, _| (p.tabs().len(), p.active_index())),
        (3, Some(0)),
        "the toggle opens the first page, as openedSurface does"
    );
}

/// `openBrowser`: the first place typed in the placeholder opens a tab of its own, which takes the placeholder's place.
#[gpui_kit::test]
fn a_browser_tab_on_a_page_takes_the_placeholders_place(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Changes, None, window, cx);
        p.toggle_pane(Pane::Browser, window, cx);
    });
    assert_eq!(panel.read_with(cx, |p, _| p.tabs())[1].1.as_ref(), "browser:new", "mod+shift+j with no browser tab opens the placeholder");
    panel.update_in(cx, |p, window, cx| p.add(Pane::Browser, Some("browser:p1".into()), window, cx));
    let tabs: Vec<String> = panel.read_with(cx, |p, _| p.tabs().iter().map(|(_, id)| id.to_string()).collect());
    assert_eq!(tabs, ["changes", "browser:p1"], "the placeholder went, the new tab is at the strip's end");
    assert_eq!(panel.read_with(cx, |p, _| p.active_index()), Some(1), "and it shows");
}

/// The browser pane's last-3 rule follows every thread the shell shows (`followThreadPanels`), and starts following from
/// inside the shell's own update, as a pane mounts.
#[gpui_kit::test]
fn the_browser_store_follows_every_thread_the_shell_shows(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let store = cx.update(|_, cx| Store::get(cx));
    shell.update_in(cx, |_, window, cx| store.update(cx, |s, cx| s.follow(window, cx)));
    cx.run_until_parked();
    for (workspace, thread) in [("w1", "b"), ("w2", "c")] {
        let route = Route::Thread { workspace_id: workspace.into(), thread_id: thread.into() };
        shell.update_in(cx, |shell, window, cx| shell.go(route, window, cx));
        cx.run_until_parked();
    }
    let shown: Vec<String> = store.read_with(cx, |s, _| s.threads_shown().iter().map(|t| t.to_string()).collect());
    assert_eq!(shown, ["c", "b", "a"], "each thread the shell showed, newest first, none of them with a browser tab");
}

/// The sidebar's Open browser (`openSurface(workspace, "preview")`): on the workspace on screen it opens the panel on the
/// browser and never shuts it; another workspace's opens nothing here.
#[gpui_kit::test]
fn the_sidebars_open_browser_opens_the_panel_on_the_browser_and_never_shuts_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let ask = |workspace: &str, cx: &mut VisualTestContext| {
        let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
        sidebar.update(cx, |_, cx| cx.emit(Asked::OpenBrowser(workspace.into())));
        cx.run_until_parked();
    };
    ask("w2", cx);
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "another workspace's Open browser opens nothing here");
    ask("w1", cx);
    assert_eq!(panel.read_with(cx, |p, _| (p.is_open(), p.active())), (true, Some(Pane::Browser)));
    ask("w1", cx);
    assert_eq!(panel.read_with(cx, |p, _| (p.is_open(), p.active())), (true, Some(Pane::Browser)), "asked again, it stays open");
}

/// `max-w-[70%]` on the title in a line that takes what it holds: Chromium gives a long title 70% of the room the row
/// leaves the line, a short one its own width.
#[gpui_kit::test]
fn the_title_takes_seventy_percent_of_the_room_the_row_leaves(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    let item =
        |t: &str, title: &str| ThreadItem { workspace_id: "w".into(), thread_id: t.into(), project: "bench".into(), title: title.into() };
    let long = "Add a test that the paging stops at the first event and then a great many more words than any row holds";
    shell.update_in(cx, |shell, window, cx| {
        shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("long", long), item("short", "Short title")], cx));
        shell.go(Route::Thread { workspace_id: "w".into(), thread_id: "long".into() }, window, cx);
    });
    cx.run_until_parked();
    let (line, title, actions) =
        (cx.debug_bounds("crumbs").unwrap(), cx.debug_bounds("crumbs-title").unwrap(), cx.debug_bounds("header-actions").unwrap());
    assert!(f32::from(actions.left() - line.right()).abs() < 1., "the line takes the room the row leaves: {line:?} against {actions:?}");
    assert!((f32::from(title.size.width) - 0.7 * f32::from(line.size.width)).abs() < 1., "{title:?} is 70% of {line:?}");

    shell.update_in(cx, |shell, window, cx| shell.go(Route::Thread { workspace_id: "w".into(), thread_id: "short".into() }, window, cx));
    cx.run_until_parked();
    let (line, title) = (cx.debug_bounds("crumbs").unwrap(), cx.debug_bounds("crumbs-title").unwrap());
    assert!(f32::from(title.size.width) < 0.7 * f32::from(line.size.width), "a short title keeps its width: {title:?} in {line:?}");
    assert!(f32::from(line.size.width) < 300., "and the line its own: {line:?}");
}

#[gpui_kit::test]
fn a_rail_held_off_its_centre_does_not_jump_and_a_release_past_the_window_still_writes(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    // Held 6 px right of the rail's centre line at 256 and moved to 300: the edge moves the 38 px the pointer did.
    cx.simulate_mouse_down(point(px(262.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(272.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(300.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("sidebar-column").unwrap().size.width, px(294.));
    cx.simulate_mouse_up(point(px(1500.), px(900.)), MouseButton::Left, Modifiers::none());
    let deadline = Instant::now() + Duration::from_secs(30);
    while get(&host).get("sidebarWidth").and_then(|w| w.as_i64()) != Some(294) {
        assert!(Instant::now() < deadline, "a release past the window never wrote 294: {:?}", get(&host).get("sidebarWidth"));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn the_active_tab_is_brought_into_the_strip_and_a_pane_with_no_screen_says_so(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let all = [Pane::Browser, Pane::Terminal, Pane::Changes, Pane::Files, Pane::Computer, Pane::Processes, Pane::Agents, Pane::Slate];
    for pane in all {
        panel.update_in(cx, |p, window, cx| p.add(pane, None, window, cx));
        cx.run_until_parked();
    }
    let strip = cx.debug_bounds("panel-tabs").unwrap();
    let last = cx.debug_bounds("panel-tab-7").unwrap();
    assert!(last.left() >= strip.left() && last.right() <= strip.right(), "the last tab opened is in view: {last:?} in {strip:?}");
    panel.update_in(cx, |p, window, cx| p.activate(0, window, cx));
    cx.run_until_parked();
    let first = cx.debug_bounds("panel-tab-0").unwrap();
    assert!(first.left() >= strip.left() && first.right() <= strip.right(), "and so is the first once it is active again: {first:?}");
    assert!(cx.debug_bounds("pane-empty").is_some(), "a pane no screen draws says nothing is open here");
}

#[gpui_kit::test]
fn a_thread_whose_workspace_does_not_run_cannot_open_its_shells(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    shell.update_in(cx, |s, window, cx| {
        s.workspace_runs("w2", false, window, cx);
        s.go(Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, window, cx);
    });
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    press(cx, "rightPanel.toggle");
    cx.simulate_keystrokes("t");
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.active()), None, "Terminal waits while the workspace does not run");
    shell.update_in(cx, |s, window, cx| s.workspace_runs("w2", true, window, cx));
    cx.simulate_keystrokes("t");
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal), "and opens once it runs");
}

/// Whether a workspace runs is read from the host's own list as the shell connects, and the panes go by it.
#[gpui_kit::test]
fn a_thread_on_a_workspace_the_host_lists_as_running_opens_a_shell(cx: &mut TestAppContext) {
    let shared = common::shared();
    let (shell, cx) = open(cx, shared.host.state(), shared.host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    let item = shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().first().cloned()).expect("the host lists a thread");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx));
    cx.run_until_parked();
    press(cx, "rightPanel.toggle");
    cx.simulate_keystrokes("t");
    cx.run_until_parked();
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal), "the host lists the thread's workspace as running");
}

/// New thread's chord, and the header's glyph that sends the same action, answered by the frame wherever the window
/// is: a page open closes, as the web's `openProjectHome` closes Settings, and the composer opens on the thread's
/// workspace.
#[gpui_kit::test]
fn new_thread_closes_settings_and_opens_the_composer_on_the_threads_workspace(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let route = |cx: &mut VisualTestContext| shell.read_with(cx, |s, _| s.route().clone());
    press(cx, "chat.new");
    assert_eq!(route(cx), Route::Screen("composer:w1".into()), "from a thread");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, window, cx));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(SETTINGS.into()), window, cx));
    cx.run_until_parked();
    press(cx, "chat.new");
    assert_eq!(route(cx), Route::Screen("composer:w2".into()), "from Settings, on the thread it was opened over");
    cx.dispatch_action(keys::CloseSettings);
    cx.run_until_parked();
    assert_eq!(route(cx), Route::Screen("composer:w2".into()), "Settings is closed behind it, and the New thread page is no page to close");
}

/// `ThreadBreadcrumb` on the New thread page: the project, a slash, "New thread", both roads; no thread is selected
/// there, so no tile is lit.
#[gpui_kit::test]
fn the_new_thread_page_reads_its_project_and_new_thread_in_the_crumbs(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    seeded(&shell, cx);
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    for here in [false, true] {
        shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
        assert_eq!(sidebar.read_with(cx, |s, _| s.lit_row()).as_deref(), Some("tile-a"));
        if here {
            new_thread_here(&shell, cx);
        } else {
            press(cx, "chat.new");
        }
        assert!(cx.debug_bounds("crumbs-project").is_some(), "the project's crumb, here {here}");
        let title = cx.debug_bounds("crumbs-title").expect("the title's crumb");
        assert!(title.left() > cx.debug_bounds("crumbs-project").unwrap().right(), "after the project");
        assert_eq!(sidebar.read_with(cx, |s, _| s.lit_row()), None, "no thread's tile is lit on the New thread page, here {here}");
    }
}

/// A launcher tile is as tall as it holds (`px-3 py-2`, its line and its reason): 54 px on one line of reason, taller
/// where the reason wraps, as "Pick a project to review its changes." does in a 360 px panel.
#[gpui_kit::test]
fn a_launcher_tile_grows_with_a_reason_that_wraps(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Empty, window, cx));
    shell.update(cx, |s, cx| s.panel().update(cx, |p, cx| p.set_width(px(360.), cx)));
    press(cx, "rightPanel.toggle");
    let (browser, changes) = (cx.debug_bounds("launcher-browser").unwrap(), cx.debug_bounds("launcher-changes").unwrap());
    assert_eq!(browser.size.height, px(54.), "one line of reason");
    assert!(changes.size.height > px(54.), "a reason that wraps grows its tile: {:?}", changes.size);
}

/// The workspace row's New thread here (`newThread(ws)`), as the sidebar asks it.
fn new_thread_here(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |s, cx| s.sidebar().update(cx, |_, cx| cx.emit(Asked::NewThreadIn("w1".into()))));
    cx.run_until_parked();
}

/// New thread (the sidebar's row, mod+t, the palette's project) opens `openProjectHome`: no workspace selected, so the
/// panel and the drawer are this computer's own (`place:here`), where the workspace's panes say to pick a project or a
/// thread; New thread here keeps the workspace's (E6).
#[gpui_kit::test]
fn new_thread_opens_the_project_home_with_this_computers_panel_and_drawer(cx: &mut TestAppContext) {
    screens::add("terminal", entry::<ShellProbe>());
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    shell.update_in(cx, |s, window, cx| s.go(Route::Empty, window, cx));
    press(cx, "rightPanel.toggle");
    panel.update_in(cx, |p, window, cx| p.add(Pane::Computer, None, window, cx));
    press(cx, "terminal.toggle");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    press(cx, "chat.new");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("composer:w1".into()));
    assert_eq!(panel.read_with(cx, |p, _| p.panes()), vec![Pane::Computer], "this computer's panel, as it was left");
    assert!(shell.read_with(cx, |s, _| s.drawer_open()), "and this computer's drawer");
    panel.update_in(cx, |p, window, cx| p.close(0, window, cx));
    press(cx, "rightPanel.toggle");
    press(cx, "rightPanel.toggle");
    for (letter, pane) in [("d", Pane::Changes), ("f", Pane::Files), ("a", Pane::Agents), ("t", Pane::Terminal)] {
        cx.simulate_keystrokes(letter);
        cx.run_until_parked();
        let opened = panel.read_with(cx, |p, _| p.panes());
        assert_eq!(opened.contains(&pane), pane == Pane::Terminal, "{pane:?} on this computer's panel: {opened:?}");
    }
    let built: Vec<Opened> = probed().into_iter().filter(|(n, what, _)| *n == "terminal" && *what == "new").map(|(_, _, o)| o).collect();
    assert_eq!(built.last().map(|o| o.workspace_id.clone()), Some(None), "its panes open on no workspace: {built:?}");

    new_thread_here(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("composer:w1".into()));
    assert!(!panel.read_with(cx, |p, _| p.is_open()), "New thread here is the workspace's own page, its panel shut");
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()), "and its drawer");
}

/// The New thread page keeps its workspace's sidebar, header toggles and panel, under the page's own key
/// (`<ws>#`): shut each time the page is shown, its panes opened on the workspace with no thread, Slate not offered.
#[gpui_kit::test]
fn the_new_thread_page_has_its_workspaces_panel_with_no_thread(cx: &mut TestAppContext) {
    screens::add("terminal", entry::<ShellProbe>());
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    press(cx, "rightPanel.toggle");
    new_thread_here(&shell, cx);
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("composer:w1".into()));
    assert!(cx.debug_bounds("right-panel").is_none(), "the page's own panel starts shut");
    for part in ["toggle-panel", "toggle-drawer", "sidebar"] {
        assert!(cx.debug_bounds(part).is_some(), "{part} shows on the New thread page");
    }

    press(cx, "rightPanel.toggle");
    assert!(cx.debug_bounds("panel-launcher").is_some(), "the header's toggle opens it on the launcher");
    for (letter, pane) in [("s", Pane::Slate), ("t", Pane::Terminal)] {
        cx.simulate_keystrokes(letter);
        cx.run_until_parked();
        let opened = panel.read_with(cx, |p, _| p.panes());
        assert_eq!(opened.contains(&pane), pane == Pane::Terminal, "{pane:?} on a page with no thread: {opened:?}");
    }
    let built: Vec<Opened> = probed().into_iter().filter(|(n, what, _)| *n == "terminal" && *what == "new").map(|(_, _, o)| o).collect();
    assert_eq!(built.len(), 1, "{built:?}");
    assert_eq!((built[0].workspace_id.as_deref(), built[0].thread_id.as_deref()), (Some("w1"), None));
    assert!(cx.debug_bounds("probe-shell").is_some());

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("right-panel").is_some(), "the thread keeps its own panel");
    assert!(panel.read_with(cx, |p, _| p.panes().is_empty()), "and not the page's tabs");
    new_thread_here(&shell, cx);
    assert!(cx.debug_bounds("right-panel").is_none(), "shown again, the page's panel starts shut");
    press(cx, "rightPanel.toggle");
    assert!(panel.read_with(cx, |p, _| p.panes().is_empty()), "with nothing of the last visit");
}

/// `carryNewThreadPanel`: the page's first send makes its thread, and the page's panel goes on as that thread's.
#[gpui_kit::test]
fn the_new_thread_pages_panel_goes_to_the_thread_its_first_send_made(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    new_thread_here(&shell, cx);
    panel.update_in(cx, |p, window, cx| p.add(Pane::Terminal, Some("terminal:9".into()), window, cx));
    shell.update(cx, |s, cx| {
        desktop_gpui::screens::terminal::carry_new_thread("w1", "sent", cx);
        s.sidebar().update(cx, |_, cx| cx.notify());
    });
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.tabs()), vec![(Pane::Terminal, "terminal:9".into())], "the page keeps its tabs");
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "b".into() }, window, cx));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "sent".into() }, window, cx));
    cx.run_until_parked();
    assert!(panel.read_with(cx, |p, _| p.is_open()), "the new thread's panel is the page's");
    assert_eq!(panel.read_with(cx, |p, _| p.tabs()), vec![(Pane::Terminal, "terminal:9".into())]);
}

/// `panes.ts`: the Pull request pane is listed only while the workspace's status names a pull request by its number.
#[gpui_kit::test]
fn the_pull_request_pane_is_listed_only_with_a_pull_request(cx: &mut TestAppContext) {
    use desktop_gpui::sidebar::host::Change;
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    press(cx, "rightPanel.toggle");
    let offered = |cx: &mut VisualTestContext| {
        cx.simulate_keystrokes("r");
        cx.run_until_parked();
        let open = panel.read_with(cx, |p, _| p.panes()) == [Pane::PullRequest];
        if open {
            panel.update_in(cx, |p, window, cx| p.close(0, window, cx));
        }
        open
    };
    assert!(!offered(cx), "no status, no pull request");
    sidebar.update(cx, |s, cx| s.apply(Change::Status(json!({ "id": "w1", "pr": { "error": "gh is not signed in" } })), cx));
    assert!(!offered(cx), "a refusal names no pull request");
    sidebar.update(cx, |s, cx| s.apply(Change::Status(json!({ "id": "w1", "pr": { "number": 42, "state": "open" } })), cx));
    assert!(offered(cx), "a pull request by its number is");
}

/// A subagent's page (`subagentPage` in AppShell.tsx): the sidebar stays; the panel and the drawer hide, kept for the
/// lead; no layout toggles stand in the header.
#[gpui_kit::test]
fn the_subagent_page_keeps_the_sidebar_and_hides_the_panel_drawer_and_toggles(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    press(cx, "rightPanel.toggle");
    press(cx, "terminal.toggle");
    assert!(cx.debug_bounds("right-panel").is_some() && cx.debug_bounds("toggle-panel").is_some());
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("subagent:w1/a/call-1".into()), window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar").is_some(), "the sidebar stays");
    for gone in ["right-panel", "toggle-panel", "toggle-drawer"] {
        assert!(cx.debug_bounds(gone).is_none(), "{gone} hides on the subagent page");
    }
    assert!(!shell.read_with(cx, |s, _| s.drawer_open()), "the lead's drawer is not drawn");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |s, _| s.route().clone()),
        Route::Screen("subagent:w1/a/call-1".into()),
        "Escape does not leave a subagent's page; Back to lead does"
    );
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("right-panel").is_some(), "back on the lead, its panel");
    assert!(shell.read_with(cx, |s, _| s.drawer_open()), "and its drawer");
}

/// Under 768 px the sidebar is a sheet (`sidebar.tsx`'s `isMobile`): shut at first, mod+b opens it over a backdrop the
/// window wide less 12 px, its toggle taking the focus as Base UI's dialog gives it to its first tabbable; Escape and a
/// press on the backdrop shut it, a press inside stays inside. Settings at 760 px, as W1-SET-030 drives it.
#[gpui_kit::test]
fn under_768_px_the_sidebar_is_a_sheet_over_the_window(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    with_threads(&shell, cx);
    cx.simulate_resize(size(px(760.), px(800.)));
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen(SETTINGS.into()), window, cx));
    cx.run_until_parked();
    let visible = |cx: &mut VisualTestContext| shell.update_in(cx, |s, window, _| s.sidebar_visible(window));
    assert!(!visible(cx) && cx.debug_bounds("sidebar-column").is_none(), "shut at first, and no column beside the page");
    assert!(cx.debug_bounds("sidebar-sheet").is_none());

    press_chord(cx, "mod+b");
    tick(cx, 250);
    let sheet = cx.debug_bounds("sidebar-sheet").expect("mod+b opens the sheet");
    assert_eq!((sheet.origin.x, sheet.size.height), (px(0.), px(800.)));
    let column = cx.debug_bounds("sidebar-column").unwrap();
    assert!(sheet.size.width < px(400.) && sheet.size.width == column.size.width, "as wide as what it holds: {:?}", sheet.size);
    assert!(cx.debug_bounds("sidebar-sheet-backdrop").is_some());
    assert!(
        shell.update_in(cx, |s, window, _| s.sheet_toggle_focused(window)),
        "the focus lands on its first tabbable, its own toggle, as Base UI's dialog puts it"
    );
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!visible(cx), "Escape shuts the sheet");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()), "and leaves the page open");

    press_chord(cx, "mod+b");
    cx.simulate_click(point(sheet.right() - px(10.), px(26.)), Modifiers::none());
    assert!(visible(cx), "a press inside the sheet stays in it");
    cx.simulate_click(point(sheet.right() + px(20.), px(400.)), Modifiers::none());
    assert!(!visible(cx), "a press on the backdrop beside it shuts it");

    press_chord(cx, "mod+b");
    tick(cx, 250);
    let row = cx.debug_bounds("settings-keybindings").expect("the sheet lists Settings' pages");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(!visible(cx), "picking a page shuts the sheet, as Settings' go does");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(SETTINGS.into()));

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    press_chord(cx, "mod+b");
    assert!(visible(cx) && cx.debug_bounds("sidebar").is_some(), "on a thread the sheet holds the threads");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!visible(cx), "and Escape shuts it there too");

    press_chord(cx, "mod+b");
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-sheet").is_none() && cx.debug_bounds("sidebar-column").is_some(), "wide, the column is back");
}

/// The panel's width and each place's tabs live in the app's store under today's localStorage keys and shapes: read
/// at launch, the tabs written as they change, the width as a drag ends.
#[gpui_kit::test]
fn the_panels_width_and_tabs_are_kept_in_the_store_under_todays_keys(cx: &mut TestAppContext) {
    use desktop_gpui::local_store::{self, LocalStore};
    use desktop_gpui::screens::app_shell::panel::{STATE_KEY, WIDTH_KEY};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local-store.json");
    let state = json!({ "state": { "byWorkspaceId": {
        "place:here": { "isOpen": true, "activeSurfaceId": "browser:abc", "surfaces": [
            { "id": "machine", "kind": "machine" },
            { "id": "browser:abc", "kind": "preview", "resourceId": "abc" },
            { "id": "aside", "kind": "aside" },
            { "id": "x", "kind": "no-such-pane" } ] },
        "w1#b": { "isOpen": true, "activeSurfaceId": "terminal:4", "surfaces": [
            { "id": "terminal:4", "kind": "terminal", "resourceId": "4", "terminalIds": ["4"], "activeTerminalId": "4" },
            { "id": "file:src/a.rs", "kind": "files", "path": "src/a.rs", "line": null, "reveal": 0 } ] } } }, "version": 2 });
    std::fs::write(&path, json!({ STATE_KEY: state, WIDTH_KEY: 500.5 }).to_string()).unwrap();
    cx.update(|cx| local_store::init(&path, cx));
    let (shell, cx) = hostless(cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    assert_eq!(panel.read_with(cx, |p, _| p.width()), px(500.5), "the width is read at launch");
    assert_eq!(panel.read_with(cx, |p, _| p.tabs()), vec![(Pane::Computer, "computer".into()), (Pane::Browser, "browser:abc".into())]);
    assert!(panel.read_with(cx, |p, _| p.is_open() && p.active() == Some(Pane::Browser)), "this computer's panel as it was left");

    with_threads(&shell, cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "b".into() }, window, cx));
    cx.run_until_parked();
    assert_eq!(panel.read_with(cx, |p, _| p.tabs()), vec![(Pane::Terminal, "terminal:4".into()), (Pane::Files, "file:src/a.rs".into())]);
    cx.update(|_, cx| {
        use desktop_gpui::screens::terminal::{groups::Ui, hub};
        let split = Ui { terminal_ids: vec!["p1".into(), "p2".into()], active_terminal_id: "p2".into(), ..Ui::default() };
        hub::set_surface(&"terminal:4".into(), split, cx);
    });
    panel.update_in(cx, |p, window, cx| p.add(Pane::Changes, None, window, cx));
    let kept = cx.update(|_, cx| cx.global::<LocalStore>().get(STATE_KEY).cloned()).unwrap();
    assert_eq!(
        kept["state"]["byWorkspaceId"]["w1#b"],
        json!({ "isOpen": true, "activeSurfaceId": "diff", "surfaces": [
            { "id": "terminal:4", "kind": "terminal", "resourceId": "4", "terminalIds": ["p1", "p2"], "activeTerminalId": "p2" },
            { "id": "file:src/a.rs", "kind": "files", "path": "src/a.rs", "line": null, "reveal": 0 },
            { "id": "diff", "kind": "diff" } ] }),
        "a change is written in today's shape, a terminal with every shell its surface holds on the host"
    );
    assert_eq!(kept["state"]["byWorkspaceId"]["place:here"]["activeSurfaceId"], json!("browser:abc"));
    assert_eq!(kept["version"], json!(2));

    let left = f32::from(cx.debug_bounds("right-panel").unwrap().origin.x);
    cx.simulate_mouse_down(point(px(left), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(left - 10.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(px(left - 40.), px(400.)), MouseButton::Left, Modifiers::none());
    let width = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.global::<LocalStore>().get(WIDTH_KEY).cloned());
    assert_eq!(width(cx), Some(json!(500.5)), "not written mid-drag");
    cx.simulate_mouse_up(point(px(left - 40.), px(400.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(width(cx), Some(json!(540.5)), "written as the drag ends");
}

/// Today's one width rule for the sidebar is `useIsMobile`, `(max-width: 767px)`: at 767 px it is a shut sheet whatever
/// the column was, at 768 the column is back as the person left it, and a toggle on one side moves only that side's
/// state (`openMobile` against `open`).
#[gpui_kit::test]
fn the_sidebar_is_shut_from_767_px_and_comes_back_as_left_at_768(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let at = |width: f32, cx: &mut VisualTestContext| {
        cx.simulate_resize(size(px(width), px(800.)));
        cx.run_until_parked();
        slid(cx);
        (cx.debug_bounds("sidebar-column").is_some(), shell.update_in(cx, |s, window, _| s.sidebar_visible(window)))
    };
    assert_eq!(at(768., cx), (true, true));
    assert_eq!(at(767., cx), (false, false), "narrowed, the sidebar shuts");
    assert_eq!(at(600., cx), (false, false));
    assert_eq!(at(768., cx), (true, true), "widened, it opens again as it was");

    press_chord(cx, "mod+b");
    assert_eq!(at(768., cx), (false, false), "the person shut it");
    assert_eq!(at(767., cx), (false, false));
    press_chord(cx, "mod+b");
    assert_eq!(at(767., cx), (true, true), "under 768 the toggle opens the sheet");
    assert!(cx.debug_bounds("sidebar-sheet").is_some());
    assert_eq!(at(768., cx), (false, false), "and leaves the column as the person shut it");
}

/// AppShell.tsx puts `SettingsSidebar` in the sidebar's place for Settings alone; every other page keeps the threads.
#[gpui_kit::test]
fn only_settings_takes_the_sidebars_place(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    for (route, threads) in [(NO_SCREEN, true), ("subagent:w1/a/c", true), (SETTINGS, false)] {
        shell.update_in(cx, |s, window, cx| s.go(Route::Screen(route.into()), window, cx));
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("sidebar").is_some(), threads, "{route}");
        assert!(cx.debug_bounds("sidebar-column").is_some(), "{route}");
    }
}

/// W1-SBR-037: a subagent's page marks its row and not its lead's tile (`useTileActive`, `SubagentRow`'s `isActive`),
/// and a press on the row opens that page.
#[gpui_kit::test]
fn a_subagents_page_marks_its_row_not_its_leads_tile(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let lead = json!({ "id": "r-a", "threadId": "a", "workspaceId": "w1", "harness": "claude", "status": "running",
        "prompt": "a", "startedAt": now - 120_000.,
        "subagents": [{ "id": "s1", "title": "Read the tests", "state": "running", "startedAt": now - 60_000., "parentToolUseId": "call-1" },
            { "id": "s2", "title": "Trace the scroll writes", "state": "done", "startedAt": now - 90_000., "endedAt": now - 80_000., "parentToolUseId": "call-2" }] });
    let snapshot = desktop_gpui::sidebar::host::Snapshot {
        sessions: vec![lead],
        workspaces: vec![json!({ "id": "w1", "name": "w1", "kind": "local" })],
        ..Default::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(desktop_gpui::sidebar::host::Change::Whole(Box::new(snapshot)), cx));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    let lit = |cx: &mut VisualTestContext| sidebar.read_with(cx, |s, _| s.lit_row());
    assert_eq!(lit(cx).as_deref(), Some("tile-a"), "the thread's own tile while the thread shows");

    let row = cx.debug_bounds("sub-a-s1").expect("the subagent's row stands under its lead");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("subagent:w1/a/call-1".into()), "a press opens its page");
    assert_eq!(lit(cx).as_deref(), Some("sub-a-s1"), "its row is marked, its lead's tile is not");

    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx));
    cx.run_until_parked();
    assert_eq!(lit(cx).as_deref(), Some("tile-a"), "back on the lead, its tile");

    // An ended subagent waits in its lead's shut Finished fold, as the transcript's card opens it: nothing is lit.
    assert!(cx.debug_bounds("sub-a-s2").is_none(), "the ended one's row is folded away");
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("subagent:w1/a/call-2".into()), window, cx));
    cx.run_until_parked();
    assert_eq!(lit(cx), None, "its lead's tile is not lit for a row folded out of sight");
}

/// Today's page loads from each host's own address, so its localStorage, the panel's records and width among it, is
/// that host's alone: a shell built on another host (a Hosts pick builds the window's shell again) starts from that
/// host's, and the way home finds this computer's again.
#[gpui_kit::test]
fn the_panels_records_are_kept_per_host(cx: &mut TestAppContext) {
    use desktop_gpui::local_store;
    use desktop_gpui::screens::app_shell::panel::RightPanel;
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| local_store::init(dir.path().join("local-store.json"), cx));
    let (_shell, cx) = hostless(cx);
    let built = |host: Option<&str>, cx: &mut VisualTestContext| {
        let host = host.map(str::to_owned);
        cx.update(|_, cx| cx.new(|cx| RightPanel::new(host, cx)))
    };
    let here = built(None, cx);
    here.update_in(cx, |p, window, cx| {
        p.add(Pane::Terminal, Some("terminal:7".into()), window, cx);
        p.set_width(px(600.), cx);
        p.keep_width(cx);
    });
    let studio = built(Some("studio"), cx);
    assert!(studio.read_with(cx, |p, _| p.tabs().is_empty() && !p.is_open()), "another host starts from its own records");
    assert_eq!(studio.read_with(cx, |p, _| p.width()), DEFAULT_WIDTH, "and its own width");
    studio.update_in(cx, |p, window, cx| p.add(Pane::Browser, Some("browser:s".into()), window, cx));
    let home = built(None, cx);
    assert_eq!(home.read_with(cx, |p, _| p.tabs()), vec![(Pane::Terminal, "terminal:7".into())], "home again, this computer's tabs");
    assert_eq!(home.read_with(cx, |p, _| p.width()), px(600.));
    assert_eq!(built(Some("studio"), cx).read_with(cx, |p, _| p.tabs()), vec![(Pane::Browser, "browser:s".into())]);
}

/// An Escape the sidebar's sheet refuses, a dialog standing over it, is no press on the panel's sheet behind.
#[gpui_kit::test]
fn an_escape_the_sidebar_sheet_refuses_under_a_dialog_opens_no_panel(cx: &mut TestAppContext) {
    use desktop_gpui::screens::dialogs::layer::{self, Closes};
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    cx.simulate_resize(size(px(760.), px(800.)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let view = AnyView::from(cx.new(|_| Part("probe-dialog")));
        layer::open("probe-dialog", view, Closes::Outside, None, window, cx);
    });
    press_chord(cx, "mod+b");
    assert!(shell.update_in(cx, |s, window, _| s.sheet_toggle_focused(window)), "the sheet's toggle holds the focus, outside the dialog");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(shell.update_in(cx, |s, window, _| s.sidebar_visible(window)), "the sheet stays under the dialog");
    assert!(!shell.read_with(cx, |s, cx| s.panel().read(cx).is_open()), "and the panel stays shut");
}

#[gpui_kit::test]
fn the_sidebars_asks_with_a_page_today_open_it(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let ask = |asked: Asked, cx: &mut VisualTestContext| {
        shell.update(cx, |s, cx| s.sidebar().update(cx, |_, cx| cx.emit(asked)));
        cx.run_until_parked();
        shell.read_with(cx, |s, _| s.route().clone())
    };
    assert_eq!(ask(Asked::NewThread, cx), Route::Screen("composer:w1".into()), "a new thread on the open thread's workspace");
    assert_eq!(ask(Asked::Settings, cx), Route::Screen(SETTINGS.into()));
    assert_eq!(ask(Asked::Usage, cx), Route::Screen(format!("{SETTINGS}:usage").into()));
    assert_eq!(ask(Asked::Activity, cx), Route::Screen(format!("{SETTINGS}:activity").into()));
    assert_eq!(ask(Asked::NewThreadIn("w2".into()), cx), Route::Screen("composer:w2".into()));
    assert_eq!(ask(Asked::Search, cx), Route::Screen("composer:w2".into()), "Search opens the palette over the route");
}

/// Two trees on w1, r over k at rest and x over y working, and two roots on w2, d ended after c.
fn trees(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let row = |i: f64, w: &str, t: &str, parent: Option<&str>, running: bool| {
        let ended = (!running).then_some(now - 60_000. + i * 1_000.);
        json!({ "id": format!("r-{t}"), "threadId": t, "workspaceId": w, "harness": "claude",
            "status": if running { "running" } else { "completed" }, "prompt": t, "parentThreadId": parent,
            "startedAt": now - 120_000. + i * 1_000., "endedAt": ended, "readAt": now - 30_000. })
    };
    let snapshot = desktop_gpui::sidebar::host::Snapshot {
        sessions: vec![
            row(0., "w1", "r", None, false),
            row(1., "w1", "k", Some("r"), false),
            row(2., "w1", "x", None, false),
            row(3., "w1", "y", Some("x"), true),
            row(4., "w2", "c", None, false),
            row(5., "w2", "d", None, false),
        ],
        workspaces: ["w1", "w2"].map(|w| json!({ "id": w, "name": w, "kind": "local" })).to_vec(),
        ..Default::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(desktop_gpui::sidebar::host::Change::Whole(Box::new(snapshot)), cx));
    cx.run_until_parked();
}

/// What mod+shift+e settles where the window stands: the root of the selected thread's tree.
fn settles(shell: &Entity<Shell>, cx: &mut VisualTestContext, route: Route) -> Option<String> {
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    cx.run_until_parked();
    shell.read_with(cx, |s, cx| {
        let (workspace, thread, _) = s.selected()?;
        s.sidebar().read(cx).open_root(&workspace, thread.as_deref())
    })
}

#[gpui_kit::test]
fn the_settle_key_settles_the_open_threads_root_tree(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    trees(&shell, cx);
    let on = |w: &str, t: &str| Route::Thread { workspace_id: w.into(), thread_id: t.into() };
    assert_eq!(settles(&shell, cx, on("w1", "k")), Some("r".into()), "the tree's root, not the thread open under it");
    assert_eq!(settles(&shell, cx, Route::Screen(SETTINGS.into())), Some("r".into()), "and from under a page, as the web's");
}

#[gpui_kit::test]
fn the_settle_key_leaves_a_tree_with_a_thread_at_work(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    trees(&shell, cx);
    let on = |w: &str, t: &str| Route::Thread { workspace_id: w.into(), thread_id: t.into() };
    assert_eq!(settles(&shell, cx, on("w1", "x")), None, "y works under x");
    assert_eq!(settles(&shell, cx, on("w1", "y")), None);
}

#[gpui_kit::test]
fn with_no_thread_open_the_settle_key_takes_the_tasks_top_thread(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    trees(&shell, cx);
    assert_eq!(settles(&shell, cx, Route::Screen("composer:w2".into())), Some("d".into()), "w2's top thread, the last to end");
    assert_eq!(settles(&shell, cx, Route::Empty), None, "and nothing with no task selected");
}

#[gpui_kit::test]
fn mod_o_opens_the_selected_task_and_nothing_from_under_a_page(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    trees(&shell, cx);
    let opens = |cx: &mut VisualTestContext, route: Route| {
        shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
        cx.run_until_parked();
        shell.read_with(cx, |s, _| s.editor_workspace())
    };
    assert_eq!(opens(cx, Route::Thread { workspace_id: "w1".into(), thread_id: "k".into() }), Some("w1".into()));
    assert_eq!(opens(cx, Route::Screen(SETTINGS.into())), None, "Settings over the thread opens nothing");
    assert_eq!(opens(cx, Route::Screen("composer:w2".into())), Some("w2".into()), "a new thread's composer names its task");
    assert_eq!(opens(cx, Route::Empty), None, "nor with no task selected");
}

/// Every tab chip is `h-6 pl-1.5 pr-2 gap-0.5` around a `size-4` close box and its label (`RightPanelTabs.tsx`).
#[gpui_kit::test]
fn each_tab_chip_is_its_close_box_and_label_in_todays_padding(cx: &mut TestAppContext) {
    let (shell, cx) = hostless(cx);
    with_threads(&shell, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    let panes = [Pane::Browser, Pane::Terminal, Pane::Changes, Pane::Files, Pane::Computer, Pane::Processes, Pane::Agents, Pane::Slate];
    panel.update_in(cx, |p, window, cx| {
        for pane in panes {
            p.add(pane, None, window, cx);
        }
    });
    cx.run_until_parked();
    let at =
        |what: &str, ix: usize, cx: &mut VisualTestContext| cx.debug_bounds(Box::leak(format!("{what}{ix}").into_boxed_str())).unwrap();
    for (ix, pane) in panes.into_iter().enumerate() {
        let (chip, close, label) = (at("panel-tab-", ix, cx), at("panel-tab-close-", ix, cx), at("panel-tab-label-", ix, cx));
        assert_eq!(chip.size.height, px(24.), "{pane:?}");
        assert_eq!(close.size, size(px(16.), px(16.)), "{pane:?}");
        assert_eq!(close.left() - chip.left(), px(6.), "{pane:?}: pl-1.5");
        assert_eq!(label.left() - close.right(), px(2.), "{pane:?}: gap-0.5");
        assert_eq!(chip.right() - label.right(), px(8.), "{pane:?}: pr-2");
    }
}
