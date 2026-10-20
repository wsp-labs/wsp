// SPDX-License-Identifier: AGPL-3.0-only
//! Ctrl+Tab and Ctrl+Shift+Tab in the window main.rs builds, the shell inside gpui-kit's Root with the default key
//! table bound as at launch: each press, and each repeat of a held press, opens the next top-level thread down or up
//! the sidebar at once and keeps its row in view; the right panel's tab strip and a focused terminal keep the chord.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::app_shell::vibrancy;
use desktop_gpui::screens::terminal::look::Look;
use desktop_gpui::screens::terminal::session::{Out, Session};
use desktop_gpui::screens::terminal::view::TerminalView;
use desktop_gpui::screens::{Pane, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::ui::{self, Side};
use gpui_kit::base::input::{AddCursorAbove, AddCursorBelow};
use gpui_kit::component::Root;
use gpui_kit::{
    px, size, AppContext, Entity, Focusable, KeyBinding, KeyDownEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext,
};
use serde_json::json;
use wsp_host_client::Env;

const MAC: bool = cfg!(target_os = "macos");

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

/// The shell in a 1280 by 800 window on `state` with `env`, built as main.rs builds it.
fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    // The host's answers, or its refusal, wake the window's tasks from the tokio runtime's thread.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let runtime = runtime();
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        vibrancy::clear_root(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (shell.unwrap(), cx)
}

/// A HOME of the test's own, so nothing reads this user's `~/.wsp`; it stands as long as the test holds it.
fn home() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let env = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    (dir, env)
}

/// `n` read top-level threads t00, t01, ... in workspace w1, listed newest first, as the host would list them.
fn seed(shell: &Entity<Shell>, n: usize, cx: &mut VisualTestContext) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let sessions = (0..n)
        .map(|i| {
            let t = format!("t{i:02}");
            json!({
                "id": format!("r-{t}"), "threadId": t, "workspaceId": "w1", "harness": "claude", "status": "completed",
                "prompt": format!("thread {t}"), "project": { "id": "p1", "name": "repo" },
                "startedAt": now - 120_000. - i as f64 * 1_000., "endedAt": now - 60_000., "readAt": now - 60_000.,
            })
        })
        .collect();
    let snapshot = Snapshot {
        sessions,
        workspaces: vec![json!({ "id": "w1", "name": "w1", "kind": "local", "project": { "id": "p1" } })],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        ..Snapshot::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: "w1".into(), thread_id: "t00".into() }, window, cx));
    drawn(cx);
}

/// The shell hostless with [`seed`]'s `n` threads and t00 open.
fn listed(n: usize, cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let (dir, env) = home();
    let (shell, cx) = open(cx, dir.path().join("state.json"), env);
    seed(&shell, n, cx);
    (shell, cx, dir)
}

fn drawn(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn on_screen(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(cx, |s, _| match s.route() {
        Route::Thread { thread_id, .. } => Some(thread_id.clone()),
        _ => None,
    })
}

fn chord(command: &str) -> String {
    let rule = keys::DEFAULTS.iter().find(|r| r.command == command).unwrap();
    keys::keystroke(rule.chord, MAC).unwrap()
}

/// Ctrl+Tab and Ctrl+Shift+Tab as typed, once the table is seen to bind them to the step along the level.
fn tabs() -> (String, String) {
    for (tab, command) in [("ctrl+tab", "workspace.next"), ("ctrl+shift+tab", "workspace.previous")] {
        let rule = keys::DEFAULTS.iter().find(|r| r.chord == tab && r.when.is_some_and(|w| w.contains("!panelTabsFocus"))).unwrap();
        assert_eq!(rule.command, command, "{tab}");
    }
    (keys::keystroke("ctrl+tab", MAC).unwrap(), keys::keystroke("ctrl+shift+tab", MAC).unwrap())
}

/// The platform's repeat of a key held down.
fn repeat(typed: &str, cx: &mut VisualTestContext) {
    let keystroke = Keystroke::parse(typed).unwrap();
    cx.simulate_event(KeyDownEvent { keystroke, is_held: true, prefer_character_input: false });
    drawn(cx);
}

/// Whether the open thread's row is drawn inside the list's viewport.
fn in_view(shell: &Entity<Shell>, key: &str, cx: &mut VisualTestContext) -> bool {
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    let viewport = sidebar.read_with(cx, |s, _| s.list_state().viewport_bounds());
    let selector: &'static str = Box::leak(format!("tile-{key}").into_boxed_str());
    cx.debug_bounds(selector).is_some_and(|row| row.top() >= viewport.top() && row.bottom() <= viewport.bottom())
}

/// Catches: a press or a held repeat that opens nothing until the key comes up (the switcher's hold), a walk that
/// stops at either end, and an opened row left scrolled out of the list.
#[gpui_kit::test]
fn a_held_ctrl_tab_opens_each_thread_down_the_sidebar_and_keeps_its_row_in_view(cx: &mut TestAppContext) {
    let (shell, cx, _home) = listed(24, cx);
    let (down, up) = tabs();
    assert_eq!((down.as_str(), up.as_str()), ("ctrl-tab", "ctrl-shift-tab"));
    cx.simulate_keystrokes(&down);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t01"), "a press opens the next thread at once");
    for i in 2..=24 {
        repeat(&down, cx);
        let want = format!("t{:02}", i % 24);
        assert_eq!(on_screen(&shell, cx), Some(want.clone()), "repeat {i} of the held chord opens the next thread");
        assert!(in_view(&shell, &want, cx), "{want}'s row is scrolled into view");
    }
    repeat(&up, cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t23"), "up from the first wraps to the last");
    assert!(in_view(&shell, "t23", cx), "and scrolls back to it");
}

/// Catches: Ctrl+Tab walking the threads where the right panel's tab strip has the focus (the `panelTabsFocus` rule).
#[gpui_kit::test]
fn ctrl_tab_on_the_panels_tab_strip_walks_its_tabs_and_leaves_the_thread(cx: &mut TestAppContext) {
    let (shell, cx, _home) = listed(3, cx);
    let panel = shell.read_with(cx, |s, _| s.panel().clone());
    panel.update_in(cx, |p, window, cx| {
        p.add(Pane::Terminal, None, window, cx);
        p.add(Pane::Changes, None, window, cx);
    });
    drawn(cx);
    let tab = cx.debug_bounds("panel-tab-0").unwrap();
    cx.simulate_click(tab.center(), Modifiers::none());
    drawn(cx);
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal));
    cx.simulate_keystrokes(&tabs().0);
    drawn(cx);
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Changes), "Ctrl+Tab steps the panel's tabs");
    cx.simulate_keystrokes(&tabs().1);
    drawn(cx);
    assert_eq!(panel.read_with(cx, |p, _| p.active()), Some(Pane::Terminal), "Ctrl+Shift+Tab steps back");
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t00"), "and neither opens another thread");
}

/// A pane around one terminal that counts the thread steps reaching it, as the shell's actions would take them.
struct Counting {
    view: Entity<TerminalView>,
    steps: std::rc::Rc<std::cell::Cell<usize>>,
}

impl gpui_kit::Render for Counting {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{InteractiveElement, ParentElement, Styled};
        let (down, up) = (self.steps.clone(), self.steps.clone());
        gpui_kit::div()
            .size_full()
            .key_context(keys::SHELL)
            .on_action(move |_: &keys::NextWorkspace, _, _| down.set(down.get() + 1))
            .on_action(move |_: &keys::PreviousWorkspace, _, _| up.set(up.get() + 1))
            .child(self.view.clone())
    }
}

/// Catches: Ctrl+Tab bound so it reaches the app from a focused terminal, where today's sends it to the shell: its
/// `!terminalFocus`, and the `!Terminal` keys.rs adds to a Control chord `TERMINAL_PASSES` does not let out.
#[gpui_kit::test]
fn ctrl_tab_in_a_focused_terminal_stays_the_terminals(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.bind_keys(keys::bindings_of(MAC, &keys::bound(MAC, &Default::default())));
        cx.bind_keys(desktop_gpui::screens::terminal::view::key_bindings());
    });
    let steps = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
        let look = Look::new(None, None, cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        made = Some(view.clone());
        let pane = cx.new(|_| Counting { view, steps: steps.clone() });
        Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    let view = made.unwrap();
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    drawn(cx);
    let session = view.read_with(cx, |v, _| v.session().clone());
    let sent = |cx: &mut VisualTestContext| -> usize {
        session.update(cx, |s, _| s.take_out()).into_iter().filter(|o| matches!(o, Out::Write(_))).count()
    };
    _ = sent(cx);
    cx.simulate_keystrokes(&tabs().0);
    cx.simulate_keystrokes(&tabs().1);
    drawn(cx);
    assert_eq!(steps.get(), 0, "neither chord steps the threads from a focused terminal");
    assert_eq!(sent(cx), 2, "both reach the shell");
}

/// Catches: a press on the palette's scrim reaching the sidebar's tile under it.
#[gpui_kit::test]
fn a_press_on_the_palettes_scrim_shuts_it_and_takes_nothing_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = listed(2, cx);
    let tile = cx.debug_bounds("tile-t01").expect("t01's tile is drawn").center();
    cx.simulate_keystrokes(&keys::keystroke("mod+k", MAC).unwrap());
    drawn(cx);
    cx.simulate_click(tile, Modifiers::none());
    drawn(cx);
    assert!(!cx.update(|window, cx| desktop_gpui::screens::palette::palette::is_open(window, cx)), "the press shuts the palette");
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t00"), "and the tile under the scrim takes nothing");
}

/// Catches: gpui-kit's add-a-cursor chords on a focused field taking the thread chords (`free_thread_chords`), and a
/// field keeping Ctrl+Shift+Tab.
#[gpui_kit::test]
fn the_step_chords_reach_the_shell_from_a_focused_field(cx: &mut TestAppContext) {
    // gpui-kit binds mod+alt+up and down to a field's add-a-cursor on a Mac and on Windows; bound here as it binds them
    // there, so the chord meets the same field binding on every platform.
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new(&keys::keystroke("mod+alt+arrowup", MAC).unwrap(), AddCursorAbove, Some("Input")),
            KeyBinding::new(&keys::keystroke("mod+alt+arrowdown", MAC).unwrap(), AddCursorBelow, Some("Input")),
        ])
    });
    let (shell, cx, _home) = listed(3, cx);
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update_in(cx, |s, window, cx| s.open_rename("t02", window, cx));
    drawn(cx);
    assert!(sidebar.read_with(cx, |s, cx| s.rename_box(cx)).is_some(), "a field holds the keys");
    // A one-line field leaves an add-a-cursor it does not take to the next binding; the composer takes it, so the
    // chord must not resolve to it at all.
    for command in ["thread.next", "thread.previous"] {
        let typed = Keystroke::parse(&chord(command)).unwrap();
        let first = cx.update(|window, cx| {
            let (found, _) = cx.key_bindings().borrow().bindings_for_input(&[typed], &window.context_stack());
            found.first().map(|b| b.action().name().to_owned())
        });
        assert_eq!(first.as_deref(), Some(keys::action(command).unwrap().name()), "{command} resolves to the shell's step in a field");
    }
    cx.simulate_keystrokes(&chord("thread.next"));
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t01"), "the task's next thread opens from the field");
    cx.simulate_keystrokes(&chord("thread.previous"));
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t00"), "and its previous");
    cx.simulate_keystrokes(&tabs().1);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t02"), "Ctrl+Shift+Tab steps up from the field too");
}

/// Catches: a step opening the thread in another window than the one the key was pressed in.
#[gpui_kit::test]
fn a_step_moves_the_window_its_key_was_pressed_in(cx: &mut TestAppContext) {
    let (shell, cx, _home) = listed(2, cx);
    let (_other_home, env) = home();
    let state = _other_home.path().join("state.json");
    let mut other = None;
    cx.add_window(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime(), Launch::default(), window, cx));
        other = Some(view.clone());
        Root::new(view, window, cx)
    });
    let other = other.unwrap();
    drawn(cx);
    cx.simulate_keystrokes(&tabs().0);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t01"), "the window the key went to opens the thread");
    assert_eq!(on_screen(&other, cx), None, "the other window stays where it was");
}

/// Runs the app's tasks until `done` holds, as pushes arrive from the host's own threads.
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

fn key_of(shell: &Entity<Shell>, title: &str, cx: &mut VisualTestContext) -> Option<(String, String)> {
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.read_with(cx, |s, _| s.threads().iter().find(|t| t.title == title).map(|t| (t.workspace_id.clone(), t.key.clone())))
}

/// Catches: the walk reading a list of its own rather than the sidebar's rows as the host changes them: a thread the
/// host deleted still stepped onto, or one it made since skipped; and a step leaving the caret off the opened thread's
/// composer.
#[gpui_kit::test]
fn the_walk_follows_the_threads_the_host_lists(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "both threads", |cx| key_of(&shell, "first", cx).is_some() && key_of(&shell, "second", cx).is_some());
    let (workspace, first) = key_of(&shell, "first", cx).unwrap();
    let (_, second) = key_of(&shell, "second", cx).unwrap();
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: workspace, thread_id: first.clone() }, window, cx));
    drawn(cx);
    let composer_focused = |cx: &mut VisualTestContext| {
        let screen = shell.read_with(cx, |s, _| s.thread_screen()).expect("the thread screen stands");
        let composer = screen.read_with(cx, |s, _| s.composer().clone());
        cx.update(|window, cx| composer.focus_handle(cx).is_focused(window))
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update_in(cx, |s, window, cx| s.open_rename(&first, window, cx));
    drawn(cx);
    assert!(!composer_focused(cx), "a field in the sidebar holds the keys");
    cx.simulate_keystrokes(&tabs().0);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx), Some(second.clone()), "the host's other thread");
    assert!(composer_focused(cx), "opens in its composer, the caret there");

    runtime().block_on(async {
        let client = host.client().await;
        let delete = common::ask::<wsp_host_client::protocol::SessionsDeleteRequest>(json!({ "threadId": first, "processes": "keep" }));
        client.call(delete).await.unwrap();
    });
    until(cx, "the deleted thread to leave the list", |cx| key_of(&shell, "first", cx).is_none());
    cx.simulate_keystrokes(&tabs().0);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx), Some(second.clone()), "with the other deleted there is nothing to step to");

    host.wsp(&["run", "repo", "third", "--agent", "claude"]);
    until(cx, "the new thread", |cx| key_of(&shell, "third", cx).is_some());
    let (_, third) = key_of(&shell, "third", cx).unwrap();
    cx.simulate_keystrokes(&tabs().1);
    drawn(cx);
    assert_eq!(on_screen(&shell, cx), Some(third), "a thread the host made since is stepped onto without reconnecting");
}

/// Catches: the mod arrows and the palette's rows stepping anything but the level Ctrl+Tab steps (`workspace.next` and
/// `workspace.previous` are one command on both chords, as today's table binds them).
#[gpui_kit::test]
fn the_mod_arrows_and_the_palette_rows_take_the_same_step(cx: &mut TestAppContext) {
    let (shell, cx, _home) = listed(3, cx);
    cx.simulate_keystrokes(&keys::keystroke("mod+alt+arrowright", MAC).unwrap());
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t01"), "mod+alt+right steps down the level");
    cx.simulate_keystrokes(&keys::keystroke("mod+alt+arrowleft", MAC).unwrap());
    drawn(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("t00"), "mod+alt+left steps back up");
    for (typed, want) in [("ctrl shift tab", "t02"), ("ctrl tab", "t00")] {
        cx.simulate_keystrokes(&keys::keystroke("mod+k", MAC).unwrap());
        drawn(cx);
        cx.simulate_input(typed);
        drawn(cx);
        cx.simulate_keystrokes("enter");
        drawn(cx);
        assert_eq!(on_screen(&shell, cx).as_deref(), Some(want), "the palette's {typed}");
    }
}
