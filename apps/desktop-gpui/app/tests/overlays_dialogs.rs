// SPDX-License-Identifier: AGPL-3.0-only
//! The palette, the toasts and the shell's chords with a dialog standing on the layer: the window main.rs
//! builds, the shell inside gpui-kit's Root with the default key table bound as at launch.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys::{self, DEFAULTS};
use desktop_gpui::screens::dialogs::{self, layer, layer::Closes};
use desktop_gpui::screens::palette::palette::{self as pal};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::host::{Change, Snapshot};
use desktop_gpui::ui::{toast, Notice};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AppContext, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::json;
use wsp_host_client::Env;

const MAC: bool = cfg!(target_os = "macos");

fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
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

/// The shell on a home of its own that no host serves, its sidebar holding threads a and b, a open.
fn hostless(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() * 1000.;
    let sessions = ["a", "b"]
        .iter()
        .enumerate()
        .map(|(i, t)| {
            json!({
                "id": format!("r-{t}"), "threadId": t, "workspaceId": "w1", "harness": "claude", "status": "completed",
                "prompt": format!("thread {t}"), "project": { "id": "p1", "name": "repo" },
                "startedAt": now - 120_000. - i as f64 * 1_000., "endedAt": now - 60_000., "readAt": now - 60_000.,
            })
        })
        .collect();
    let sessions = desktop_gpui::sidebar::shared::rows_of(sessions);
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
    let route = Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    cx.run_until_parked();
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

fn stand(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let view = cx.new(|_| Probe).into();
        layer::open("probe", view, Closes::Outside, None, window, cx);
    });
    cx.run_until_parked();
}

fn top(cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.update(|_, cx| layer::top(cx))
}

fn press_chord(cx: &mut VisualTestContext, chord: &str) {
    let typed = keys::keystroke(chord, MAC).unwrap_or_else(|| panic!("{chord} does not read"));
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
}

fn palette_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| pal::is_open(window, cx))
}

/// Catches: the palette drawn under the dialog's scrim, so what is typed goes into a palette nobody sees and a press on
/// it lands on the scrim and closes the dialog.
#[gpui_kit::test]
fn the_palette_opens_over_a_dialog_and_takes_its_keys_and_presses(cx: &mut TestAppContext) {
    let (_, cx, _home) = hostless(cx);
    stand(cx);
    press_chord(cx, "mod+k");
    assert!(palette_open(cx), "mod+k opens the palette over the dialog");
    cx.simulate_input("zzqx");
    cx.run_until_parked();
    let palette = cx.update(|window, cx| pal::palette(window, cx)).expect("the palette is open");
    assert!(palette.read_with(cx, |p, _| p.lists().is_empty()), "the palette's field takes what is typed");
    let popup = cx.debug_bounds("palette-popup").expect("the popup is drawn");
    cx.simulate_click(popup.center(), Modifiers::none());
    assert!(palette_open(cx), "a press on the palette keeps it open");
    assert_eq!(top(cx), Some("probe"), "and never reaches the dialog's scrim");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!palette_open(cx), "the first Escape shuts the palette");
    assert_eq!(top(cx), Some("probe"), "and leaves the dialog");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None, "the next closes the dialog");
}

/// Catches: Escape on a dialog over a page also dismissing a toast the dialog's scrim hides.
#[gpui_kit::test]
fn escape_on_a_dialog_over_a_page_leaves_the_toast_under_it(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("settings".into()), window, cx));
    cx.run_until_parked();
    cx.update(|window, cx| desktop_gpui::ui::notify_in(window, cx, Notice::note("first")));
    cx.run_until_parked();
    let standing = |cx: &mut VisualTestContext| {
        let toasts = cx.update(|window, cx| toast::of(window, cx)).expect("ui::init registers the stack");
        toasts.read_with(cx, |t, _| t.standing().len())
    };
    assert_eq!(standing(cx), 1);
    stand(cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(top(cx), None, "Escape closes the dialog");
    assert_eq!(standing(cx), 1, "and leaves the toast under it");
    assert_eq!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen("settings".into()), "and the page");
}

/// The web's dispatcher lets every chord but Escape through to the shell under a dialog (`KeybindingDispatcher.tsx`).
#[gpui_kit::test]
fn the_shells_chords_act_under_a_dialog_as_on_the_web(cx: &mut TestAppContext) {
    let (shell, cx, _home) = hostless(cx);
    stand(cx);
    let open = shell.read_with(cx, |s, _| s.sidebar_open());
    let chord = DEFAULTS.iter().find(|r| r.command == "sidebar.toggle").expect("the table binds the sidebar's toggle").chord;
    press_chord(cx, chord);
    assert_ne!(shell.read_with(cx, |s, _| s.sidebar_open()), open, "{chord} moves the sidebar behind the dialog");
    assert_eq!(top(cx), Some("probe"), "and the dialog stands");
    let chord = DEFAULTS.iter().find(|r| r.command == "settings.toggle").expect("the table binds Settings").chord;
    press_chord(cx, chord);
    until(cx, "Settings under the dialog", |cx| matches!(shell.read_with(cx, |s, _| s.route().clone()), Route::Screen(_)));
    assert_eq!(top(cx), Some("probe"), "the dialog stays over Settings");
}

/// Catches: the palette's Add a project and Add a computer rows doing nothing, or landing on a blank Settings page,
/// where the sidebar's own buttons open their dialogs.
#[gpui_kit::test]
fn the_palettes_add_rows_open_their_dialogs(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let (shell, cx) = open(cx, host.state(), host.env());
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    for (words, key) in [("add a project", dialogs::ADD_PROJECT), ("add a computer", dialogs::ADD_COMPUTER)] {
        press_chord(cx, "mod+k");
        cx.simulate_input(words);
        cx.simulate_keystrokes("enter");
        until(cx, words, |cx| top(cx) == Some(key));
        assert!(!palette_open(cx), "the row shuts the palette");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(top(cx), None);
    }
}
