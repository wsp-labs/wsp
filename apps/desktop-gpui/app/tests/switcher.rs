// SPDX-License-Identifier: AGPL-3.0-only
//! The thread switcher in the window main.rs builds: the shell inside gpui-kit's Root after `gpui_kit::init`, wsp's
//! themes and the layers' plugin, with the default key table bound as at launch. The keys arrive as a person presses
//! them: the modifier goes down, then the chord, then the modifier comes up. The paint waits on the test clock, which
//! moves only when a test moves it.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys::{self, DEFAULTS};
use desktop_gpui::screens::palette::overlays;
use desktop_gpui::screens::palette::switcher::{self, Switcher, PAINT_DELAY};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::{Change, Snapshot};
use gpui_kit::base::input::{AddCursorAbove, AddCursorBelow};
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, AppContext, Entity, KeyBinding, Keystroke, Modifiers, MouseButton, TestAppContext, VisualTestContext};
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

/// A shell with no host, whose HOME is a folder of its own so nothing reads this user's `~/.wsp`.
/// The folder is the test's own and stands as long as the test holds it.
fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    (shell, cx, dir)
}

/// The sidebar holding `threads`, each `(workspace, thread)` and newest first, all finished and read a minute ago.
fn threads(shell: &Entity<Shell>, cx: &mut VisualTestContext, threads: &[(&str, &str)]) {
    let rows: Vec<(&str, &str, Option<&str>)> = threads.iter().map(|&(w, t)| (w, t, None)).collect();
    threads_under(shell, cx, &rows);
}

/// [`threads`], each with the thread that opened it, if one did.
fn threads_under(shell: &Entity<Shell>, cx: &mut VisualTestContext, threads: &[(&str, &str, Option<&str>)]) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let sessions = threads
        .iter()
        .enumerate()
        .map(|(i, (w, t, parent))| {
            let started = now - 120_000. - i as f64 * 1_000.;
            json!({
                "id": format!("r-{t}"), "threadId": t, "workspaceId": w, "harness": "claude", "status": "completed",
                "prompt": format!("thread {t}"), "project": { "id": "p1", "name": "repo" },
                "startedAt": started, "endedAt": now - 60_000., "readAt": now - 60_000., "parentThreadId": parent,
            })
        })
        .collect();
    let mut workspaces: Vec<&str> = threads.iter().map(|(w, _, _)| *w).collect();
    workspaces.sort();
    workspaces.dedup();
    let snapshot = Snapshot {
        sessions,
        workspaces: workspaces.iter().map(|w| json!({ "id": w, "name": w, "kind": "local", "project": { "id": "p1" } })).collect(),
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        ..Snapshot::default()
    };
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
    cx.update(|window, cx| switcher::follow(window, cx));
    cx.run_until_parked();
}

fn go(shell: &Entity<Shell>, cx: &mut VisualTestContext, workspace: &str, thread: &str) {
    let route = Route::Thread { workspace_id: workspace.into(), thread_id: thread.into() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    cx.run_until_parked();
}

fn on_screen(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(cx, |s, _| match s.route() {
        Route::Thread { thread_id, .. } => Some(thread_id.clone()),
        _ => None,
    })
}

fn the_switcher(cx: &mut VisualTestContext) -> Option<Entity<Switcher>> {
    cx.update(|window, cx| overlays::of(window, cx).and_then(|o| o.read(cx).switcher.clone()))
}

/// The cards' threads, and the one the highlight stands on.
fn cards(cx: &mut VisualTestContext) -> (Vec<String>, String) {
    let switcher = the_switcher(cx).expect("the switcher is open");
    switcher.read_with(cx, |s, _| {
        (s.cards().iter().map(|c| c.thread_id.clone()).collect(), s.highlighted().map(|c| c.thread_id.clone()).unwrap_or_default())
    })
}

/// `chord`, which the table binds to `command`, pressed with its modifiers held down first, as a keyboard sends it.
fn hold(cx: &mut VisualTestContext, command: &str, chord: &str) {
    assert!(DEFAULTS.iter().any(|r| r.command == command && r.chord == chord), "the table binds {chord} to {command}");
    let typed = keys::keystroke(chord, MAC).unwrap_or_else(|| panic!("{chord} does not read"));
    cx.simulate_modifiers_change(Keystroke::parse(&typed).unwrap().modifiers);
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
}

fn let_go(cx: &mut VisualTestContext) {
    cx.simulate_modifiers_change(Modifiers::none());
    cx.run_until_parked();
}

fn wait(cx: &mut VisualTestContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

fn painted(cx: &mut VisualTestContext) -> bool {
    cx.run_until_parked();
    cx.debug_bounds("switcher").is_some()
}

#[gpui_kit::test]
fn a_held_tab_paints_after_150_ms_and_letting_go_opens_the_highlighted_thread(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b"), ("w2", "c"), ("w2", "d")]);
    go(&shell, cx, "w2", "c");
    go(&shell, cx, "w1", "a");

    hold(cx, "workspace.next", "ctrl+tab");
    assert_eq!(cards(cx), (vec!["a".into(), "c".into(), "b".into(), "d".into()], "c".into()), "the open thread, then the last opened");
    assert!(!painted(cx), "nothing draws on the press");
    wait(cx, PAINT_DELAY - Duration::from_millis(1));
    assert!(!painted(cx), "nor before 150 ms");
    wait(cx, Duration::from_millis(1));
    assert!(painted(cx), "the held chord paints at 150 ms");
    let row = cx.debug_bounds("switcher").unwrap();
    let card = cx.debug_bounds("switcher-card-a").expect("a card per thread");
    assert_eq!(card.size.width, px(224.));
    assert_eq!(row.size.width, px(4. * 224. + 3. * 8. + 2. * 9.), "four cards 8 px apart inside 8 px of padding and a 1 px edge");
    assert_eq!(row.center(), point(px(640.), px(400.)), "centred in the window");

    hold(cx, "workspace.next", "ctrl+tab");
    assert_eq!(cards(cx).1, "b", "a step while it stands open moves the highlight");
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"), "and opens nothing");
    let_go(cx);
    assert!(the_switcher(cx).is_none(), "letting the modifier go takes the switcher down");
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("b"), "and opens the highlighted thread");
}

#[gpui_kit::test]
fn a_tap_switches_without_ever_painting(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b"), ("w1", "c")]);
    go(&shell, cx, "w1", "a");
    hold(cx, "workspace.next", "ctrl+tab");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("b"));
    wait(cx, Duration::from_secs(1));
    assert!(!painted(cx), "a tap never draws the switcher");
    hold(cx, "workspace.next", "ctrl+tab");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"), "a second tap goes back to the thread before");
    hold(cx, "workspace.next", "mod+alt+arrowright");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("b"), "the arrow chord switches the same way");
}

#[gpui_kit::test]
fn ctrl_shift_tab_walks_back_and_shift_alone_does_not_commit(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b"), ("w1", "c")]);
    go(&shell, cx, "w1", "a");
    hold(cx, "workspace.previous", "ctrl+shift+tab");
    assert_eq!(cards(cx), (vec!["a".into(), "b".into(), "c".into()], "c".into()), "back from the open thread wraps to the last");
    cx.simulate_modifiers_change(Modifiers::control());
    cx.run_until_parked();
    assert!(the_switcher(cx).is_some(), "letting shift go keeps the walk");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("c"));
}

#[gpui_kit::test]
fn the_thread_chords_walk_only_the_open_tasks_threads(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w2", "x"), ("w1", "b"), ("w2", "y")]);
    go(&shell, cx, "w2", "x");
    go(&shell, cx, "w1", "a");
    hold(cx, "thread.next", "mod+alt+arrowdown");
    assert_eq!(cards(cx), (vec!["a".into(), "b".into()], "b".into()), "w2's threads are not walked, however recent");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("b"));
}

#[gpui_kit::test]
fn nothing_opens_with_no_other_thread_to_land_on(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w2", "x")]);
    go(&shell, cx, "w1", "a");
    hold(cx, "thread.next", "mod+alt+arrowdown");
    assert!(the_switcher(cx).is_none(), "the task on screen holds one thread");
    wait(cx, PAINT_DELAY);
    assert!(!painted(cx));
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"));
}

#[gpui_kit::test]
fn escape_and_the_window_losing_focus_leave_the_person_where_they_were(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    let before = cx.update(|window, cx| window.focused(cx));

    hold(cx, "workspace.next", "ctrl+tab");
    wait(cx, PAINT_DELAY);
    assert!(painted(cx));
    cx.simulate_keystrokes("escape");
    assert!(the_switcher(cx).is_none(), "Escape takes it down");
    assert_eq!(cx.update(|window, cx| window.focused(cx)), before, "and gives the keys back");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"), "the key coming up after opens nothing");

    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    hold(cx, "workspace.next", "ctrl+tab");
    assert!(the_switcher(cx).is_some());
    cx.deactivate_window();
    assert!(the_switcher(cx).is_none(), "the window losing focus takes it down");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"));
}

/// The palette opening while the switcher holds the keys, as mod+k forwarded from under it opens it off the Mac.
#[gpui_kit::test]
fn a_palette_opened_over_the_switcher_keeps_the_keys(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    // GPUI tells a focus handle it lost focus only in an active window.
    cx.update(|window, _| window.activate_window());
    hold(cx, "workspace.next", "ctrl+tab");
    let frame = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    };
    frame(cx);
    cx.update(desktop_gpui::screens::palette::palette::toggle);
    cx.run_until_parked();
    frame(cx);
    let held = cx.update(|window, _| window.context_stack().iter().any(|c| c.contains("Palette")));
    assert!(held, "the palette's field keeps the keys once the switcher is gone");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.update(|window, cx| desktop_gpui::screens::palette::palette::is_open(window, cx)), "and Escape shuts it");
}

#[gpui_kit::test]
fn the_painted_switcher_takes_every_press_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    let tile = cx.debug_bounds("tile-b").expect("b's tile is drawn").center();
    hold(cx, "workspace.next", "ctrl+tab");
    wait(cx, PAINT_DELAY);
    assert!(painted(cx));
    let held = Modifiers::control();
    cx.simulate_mouse_down(tile, MouseButton::Left, held);
    cx.simulate_mouse_up(tile, MouseButton::Left, held);
    cx.run_until_parked();
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"), "the tile under the scrim takes no press");
    assert!(the_switcher(cx).is_some(), "and the walk stands");
}

#[gpui_kit::test]
fn a_press_on_the_palettes_scrim_shuts_it_and_takes_nothing_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    let tile = cx.debug_bounds("tile-b").expect("b's tile is drawn").center();
    cx.simulate_keystrokes(&keys::keystroke("mod+k", MAC).unwrap());
    cx.run_until_parked();
    cx.simulate_click(tile, Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.update(|window, cx| desktop_gpui::screens::palette::palette::is_open(window, cx)), "the press shuts the palette");
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"), "and the tile under the scrim takes nothing");
}

#[gpui_kit::test]
fn the_task_step_walks_a_sub_thread_as_the_chord_does(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads_under(&shell, cx, &[("w1", "a", None), ("w1", "s", Some("a")), ("w2", "x", None)]);
    go(&shell, cx, "w1", "a");
    hold(cx, "thread.next", "mod+alt+arrowdown");
    assert_eq!(cards(cx), (vec!["a".into(), "s".into()], "s".into()), "the chord walks the sub-thread under its opener");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("s"));
    go(&shell, cx, "w1", "a");
    cx.dispatch_action(keys::StepThread(1));
    cx.run_until_parked();
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("s"), "the palette's Next thread steps along the same walk");
}

#[gpui_kit::test]
fn letting_go_moves_the_window_the_walk_was_made_in(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    let other_home = tempfile::tempdir().unwrap();
    let env: Env = [("HOME".to_owned(), other_home.path().to_string_lossy().into_owned())].into_iter().collect();
    let state = other_home.path().join("state.json");
    let mut other = None;
    // Opened second, so the app's router names this window's shell.
    cx.add_window(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime(), Launch::default(), window, cx));
        other = Some(view.clone());
        Root::new(view, window, cx)
    });
    let other = other.unwrap();
    cx.run_until_parked();
    hold(cx, "workspace.next", "ctrl+tab");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("b"), "the window the switcher stood in opens the thread");
    assert_eq!(on_screen(&other, cx), None, "the other window stays where it was");
}

#[gpui_kit::test]
fn other_chords_reach_the_shell_while_the_switcher_stands(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b")]);
    go(&shell, cx, "w1", "a");
    hold(cx, "workspace.next", "ctrl+tab");
    assert!(shell.read_with(cx, |s, _| s.sidebar_open()));
    let typed = keys::keystroke("mod+b", MAC).unwrap();
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |s, _| s.sidebar_open()), "mod+b toggles the sidebar under the switcher");
    assert!(the_switcher(cx).is_some(), "and the walk stands");
    hold(cx, "workspace.next", "ctrl+tab");
    assert_eq!(cards(cx).1, "a", "the switch chord still steps it");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some("a"));
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

fn key_of(shell: &Entity<Shell>, cx: &mut VisualTestContext, title: &str) -> Option<(String, String)> {
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.read_with(cx, |s, _| s.threads().iter().find(|t| t.title == title).map(|t| (t.workspace_id.clone(), t.key.clone())))
}

fn shown(cx: &mut VisualTestContext) -> Vec<String> {
    let switcher = the_switcher(cx).expect("the switcher is open");
    switcher.read_with(cx, |s, cx| s.shown(cx).into_iter().map(|c| c.title).collect())
}

#[gpui_kit::test]
fn the_cards_read_the_hosts_threads_and_follow_it_while_the_window_lives(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "both threads", |cx| key_of(&shell, cx, "first").is_some() && key_of(&shell, cx, "second").is_some());
    let (workspace, first) = key_of(&shell, cx, "first").unwrap();
    go(&shell, cx, &workspace, &first);

    hold(cx, "workspace.next", "ctrl+tab");
    wait(cx, PAINT_DELAY);
    assert!(painted(cx));
    assert_eq!(shown(cx), ["first", "second"], "a card per thread the host lists, the open one first");
    let place = the_switcher(cx).unwrap().read_with(cx, |s, cx| s.shown(cx)[0].place.clone());
    assert!(!place.is_empty(), "each card names the computer its thread runs on");

    let (_, second) = key_of(&shell, cx, "second").unwrap();
    runtime().block_on(async {
        let client = host.client().await;
        let delete = common::ask::<wsp_host_client::protocol::SessionsDeleteRequest>(json!({ "threadId": second, "processes": "keep" }));
        client.call(delete).await.unwrap();
    });
    until(cx, "the deleted thread's card to go", |cx| shown(cx) == ["first"]);
    let gone: &'static str = Box::leak(format!("switcher-card-{second}").into_boxed_str());
    assert!(cx.debug_bounds(gone).is_none(), "the window drew it gone without the switcher reopening");
    hold(cx, "workspace.next", "ctrl+tab");
    let_go(cx);
    assert_eq!(on_screen(&shell, cx).as_deref(), Some(first.as_str()));

    host.wsp(&["run", "repo", "third", "--agent", "claude"]);
    until(cx, "the new thread", |cx| key_of(&shell, cx, "third").is_some());
    hold(cx, "workspace.next", "ctrl+tab");
    wait(cx, PAINT_DELAY);
    assert_eq!(shown(cx), ["first", "third"], "a thread made on the host since is walked without reconnecting");
    let_go(cx);
    let (_, third) = key_of(&shell, cx, "third").unwrap();
    assert_eq!(on_screen(&shell, cx), Some(third));
}

#[gpui_kit::test]
fn the_thread_chords_step_the_switcher_from_a_focused_field(cx: &mut TestAppContext) {
    // gpui-kit binds mod+alt+up and down to a field's add-a-cursor on a Mac and on Windows; bound here as it binds them
    // there, so the chord meets the same field binding on every platform.
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new(&keys::keystroke("mod+alt+arrowup", MAC).unwrap(), AddCursorAbove, Some("Input")),
            KeyBinding::new(&keys::keystroke("mod+alt+arrowdown", MAC).unwrap(), AddCursorBelow, Some("Input")),
        ])
    });
    let (shell, cx, _home) = hostless(cx);
    threads(&shell, cx, &[("w1", "a"), ("w1", "b"), ("w1", "c")]);
    go(&shell, cx, "w1", "a");
    let sidebar = shell.read_with(cx, |s, _| s.sidebar().clone());
    sidebar.update_in(cx, |s, window, cx| s.open_rename("b", window, cx));
    cx.run_until_parked();
    assert!(sidebar.read_with(cx, |s, cx| s.rename_box(cx)).is_some(), "a field holds the keys");
    hold(cx, "thread.next", "mod+alt+arrowdown");
    assert_eq!(cards(cx).1, "b", "the chord opens the switcher over the field");
    hold(cx, "thread.previous", "mod+alt+arrowup");
    assert_eq!(cards(cx).1, "a", "and steps it");
    let_go(cx);
}
