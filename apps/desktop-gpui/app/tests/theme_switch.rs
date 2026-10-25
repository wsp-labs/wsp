// SPDX-License-Identifier: AGPL-3.0-only
//! A theme picked on Settings > Appearance paints every view of the window in the new palette on the first frame
//! after Settings closes, by any way out, the same as a whole redraw paints it; and a switch renders each view a few
//! times and then stops.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::settings::SettingsScreen;
use desktop_gpui::screens::{composer, Route};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::theme::{Side, THEMES};
use desktop_gpui::ui::{self};
use gpui_kit::{px, size, AppContext, Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;

const WAIT: Duration = Duration::from_secs(30);

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The shell as the app roots it, 1280 by 800, on a host with one project and one finished thread, the thread open.
fn launched(cx: &mut TestAppContext) -> (common::Throwaway, Entity<Shell>, Route, &mut VisualTestContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
        desktop_gpui::screens::palette::init(cx);
        composer::init(cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        shell = Some(view.clone());
        desktop_gpui::screens::app_shell::vibrancy::clear_root(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.update(|window, _| window.activate_window());
    let shell = shell.unwrap();
    until(cx, "the thread in the sidebar", |cx| shell.read_with(cx, |s, cx| !s.sidebar().read(cx).items().is_empty()));
    let item = shell.read_with(cx, |s, cx| s.sidebar().read(cx).items()[0].clone());
    let thread = Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() };
    let go = thread.clone();
    shell.update_in(cx, |s, window, cx| s.go(go, window, cx));
    until(cx, "the thread's rows", |cx| cx.debug_bounds("sidebar-age").is_some());
    (host, shell, thread, cx)
}

fn mounted(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Entity<SettingsScreen>> {
    shell.read_with(cx, |s, _| s.screen_view()).and_then(|view| view.downcast::<SettingsScreen>().ok())
}

/// Every quad, glyph and underline the last frame painted, with its color.
fn painted(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|q| format!("quad {:?} {:?} {:?}", q.bounds, q.background, q.border_color));
        let glyphs = window.painted_monochrome_sprites().into_iter().map(|s| format!("glyph {:?} {:?}", s.bounds, s.color));
        let lines = window.painted_underlines().into_iter().map(|u| format!("line {:?} {:?}", u.bounds, u.color));
        quads.chain(glyphs).chain(lines).collect()
    })
}

fn differ(a: &[String], b: &[String]) -> Vec<String> {
    let only_a = a.iter().filter(|x| !b.contains(x)).take(4).map(|x| format!("- {x}"));
    let only_b = b.iter().filter(|x| !a.contains(x)).take(4).map(|x| format!("+ {x}"));
    only_a.chain(only_b).collect()
}

#[derive(Clone, Copy, Debug)]
enum Out {
    Back,
    Shortcut,
    Sidebar,
}

/// Opens Appearance and picks `id` and `transparency` there as the page's cards and switch do: the pointer over the
/// card, then the press.
fn pick(shell: &Entity<Shell>, id: &'static str, transparency: bool, cx: &mut VisualTestContext) {
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("settings:appearance".into()), window, cx));
    until(cx, "Settings", |cx| mounted(shell, cx).is_some());
    let screen = mounted(shell, cx).unwrap();
    until(cx, "Appearance with its reads", |cx| mounted(shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some())));
    // The page lists the themes of the side it draws, so the side goes first.
    let side = if ui::theme::theme_by_id(id).unwrap().side == Side::Dark { "dark" } else { "light" };
    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": side, "transparency": transparency }), cx));
    let card: &'static str = Box::leak(format!("theme-option-{id}").into_boxed_str());
    until(cx, "the side's themes", |cx| cx.debug_bounds(card).is_some());
    let at = cx.debug_bounds(card).unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
}

fn leave(out: Out, thread: &Route, cx: &mut VisualTestContext) {
    match out {
        Out::Back => {
            let back = cx.debug_bounds("settings-back").expect("Settings' Back row").center();
            cx.simulate_mouse_move(back, None, Modifiers::none());
            cx.simulate_click(back, Modifiers::none());
        }
        Out::Shortcut => cx.dispatch_action(keys::ToggleSettings),
        Out::Sidebar => {
            let route = thread.clone();
            cx.update(|_, cx| desktop_gpui::screens::app_shell::navigate(route, cx));
        }
    }
}

/// Catches: a cached view, or one drawn in place, that keeps painting the old palette after Settings closes, and a way
/// out of Settings that puts back the theme the window drew before the pick.
#[gpui_kit::test]
fn a_theme_picked_in_settings_paints_every_view_on_the_next_frame(cx: &mut TestAppContext) {
    let (_host, shell, thread, cx) = launched(cx);
    let mut wrong = Vec::new();
    for transparency in [true, false] {
        for entry in &THEMES {
            for out in [Out::Back, Out::Shortcut, Out::Sidebar] {
                pick(&shell, entry.id, transparency, cx);
                leave(out, &thread, cx);
                cx.run_until_parked();
                let left = mounted(&shell, cx).is_none() && shell.read_with(cx, |s, _| s.route() == &thread);
                assert!(left, "{out:?} left Settings for the thread");
                let next = painted(cx);
                let theme = cx.update(|_, cx| ui::theme::current(cx).id);
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                let whole = painted(cx);
                let case = format!("{} transparency {transparency} by {out:?}", entry.id);
                if theme != entry.id {
                    wrong.push(format!("{case}: the window draws {theme}"));
                } else if next != whole {
                    wrong.push(format!("{case}: the next frame painted otherwise than a whole redraw: {:?}", differ(&next, &whole)));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} of 48 switches went wrong:\n{}", wrong.len(), wrong.join("\n"));
}

#[allow(dead_code)]
#[path = "../src/screens/settings/door.rs"]
mod door;

/// A preferences record as the door's host keeps it: dark, in `dark`.
fn record(dark: &str) -> serde_json::Value {
    json!({ "theme": "dark", "lightTheme": "paper", "darkTheme": dark, "sidebarMode": "list", "terminalSize": "app",
        "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false, "transparency": true })
}

/// The shell as the app roots it on a door that serves an empty host whose preferences.set waits for its release.
fn on_door(set: serde_json::Value, cx: &mut TestAppContext) -> (door::Door, Entity<Shell>, &mut VisualTestContext) {
    let door = door::Door::start(json!({
        "sessions.list": { "reply": { "sessions": [] } },
        "workspaces.list": { "reply": { "workspaces": [] } },
        "preferences.get": { "reply": { "preferences": record("graphite") } },
        "preferences.set": set,
    }));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, env) = (door.state(), door.env());
    let launch = Launch { screen: Some("settings:appearance".to_owned()), ..Launch::default() };
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, door::runtime(), launch, window, cx));
        shell = Some(view.clone());
        desktop_gpui::screens::app_shell::vibrancy::clear_root(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.update(|window, _| window.activate_window());
    (door, shell.unwrap(), cx)
}

/// The theme the window draws and the one Settings shows picked, when Settings is open.
fn themes(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (&'static str, Option<String>) {
    let drawn = cx.update(|_, cx| ui::theme::current(cx).id);
    let picked = mounted(shell, cx).and_then(|s| s.read_with(cx, |s, _| s.data().preferences.as_ref().map(|p| p.dark_theme.clone())));
    (drawn, picked)
}

/// The notices standing about a save; the door refuses Settings' other reads, which say so too.
fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let standing = ui::toast::of(window, cx).map(|t| t.read(cx).standing()).unwrap_or_default();
        standing.iter().map(|n| n.text.to_string()).filter(|text| text.contains("not saved")).collect()
    })
}

/// Picks Moss, closes Settings and opens it again while the host has not answered the write, Settings reading the
/// host's record from before it again; the host's answer is then let go.
fn moss_while_the_host_waits(door: &door::Door, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    until(cx, "Appearance with the host's record", |cx| {
        themes(shell, cx) == ("graphite", Some("graphite".into())) && cx.debug_bounds("theme-option-moss").is_some()
    });
    let moss = cx.debug_bounds("theme-option-moss").unwrap().center();
    cx.simulate_mouse_move(moss, None, Modifiers::none());
    cx.simulate_click(moss, Modifiers::none());
    until(cx, "the write sent", |_| !door.asked_of("preferences.set").is_empty());
    assert_eq!(themes(shell, cx), ("moss", Some("moss".into())), "the pick shows at once, in the window and in Settings");
    cx.dispatch_action(keys::ToggleSettings);
    cx.run_until_parked();
    assert!(mounted(shell, cx).is_none(), "the toggle closed Settings");
    assert_eq!(themes(shell, cx), ("moss", None), "closing Settings while the write is out keeps the pick");
    let next = painted(cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        next == painted(cx),
        "the frame after Settings closed painted otherwise than a whole redraw: {:?}",
        differ(&next, &painted(cx))
    );
    let reads = door.asked_of("preferences.get").len();
    shell.update_in(cx, |s, window, cx| s.go(Route::Screen("settings:appearance".into()), window, cx));
    until(cx, "Settings reading the host's record again", |cx| {
        door.asked_of("preferences.get").len() > reads
            && mounted(shell, cx).is_some_and(|s| s.read_with(cx, |s, _| s.data().preferences.is_some()))
    });
    std::thread::sleep(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(themes(shell, cx), ("moss", Some("moss".into())), "Settings opened again, the host's older record read, shows the pick");
    door.release();
}

/// Catches: a write the host has not answered that closing Settings takes back, Settings opened again showing the
/// host's record from before the write, and the window and Settings showing two themes at any point.
#[gpui_kit::test]
fn a_slow_theme_save_keeps_settings_and_the_window_on_the_pick(cx: &mut TestAppContext) {
    let set = json!({ "hold": true, "pushAfter": [{ "type": "preferences.changed", "preferences": record("moss") }], "reply": {} });
    let (door, shell, cx) = on_door(set, cx);
    moss_while_the_host_waits(&door, &shell, cx);
    // The answer and the record it pushes come over the socket in a moment; nothing the window shows may move.
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
        assert_eq!(themes(&shell, cx), ("moss", Some("moss".into())), "while the host answers");
    }
    assert_eq!(themes(&shell, cx), ("moss", Some("moss".into())), "the host's answer leaves the pick");
    assert!(toasts(cx).is_empty(), "a save that stood says nothing: {:?}", toasts(cx));
}

/// Catches: a refused write whose failure goes unsaid because the Settings that made it has closed, and a refusal that
/// leaves the window and Settings on two themes.
#[gpui_kit::test]
fn a_refused_theme_save_says_so_and_puts_the_hosts_theme_back_everywhere(cx: &mut TestAppContext) {
    let set = json!({ "hold": true, "error": "The state file is read-only." });
    let (door, shell, cx) = on_door(set, cx);
    moss_while_the_host_waits(&door, &shell, cx);
    until(cx, "the refusal said", |cx| !toasts(cx).is_empty());
    assert_eq!(themes(&shell, cx), ("graphite", Some("graphite".into())), "the host's theme again, in the window and in Settings");
    assert_eq!(toasts(cx), ["That setting was not saved: The state file is read-only. It shows the host's value again."]);
}

/// Catches: a theme picked on Appearance that renders the views more than once for each of the pointer's move, press
/// and release and the pick laid on, or sets off renders that do not stop.
#[gpui_kit::test]
fn a_theme_pick_renders_each_view_at_most_four_times_and_stops(cx: &mut TestAppContext) {
    let (door, shell, cx) = on_door(json!({ "reply": {} }), cx);
    until(cx, "Appearance with the host's record", |cx| cx.debug_bounds("theme-option-moss").is_some());
    let frame = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    };
    // The page's arrival animates its notices for a while; the picks start once a third of a second draws nothing.
    let mut still = 0;
    for _ in 0..600 {
        std::thread::sleep(Duration::from_millis(5));
        frame(cx);
        still = if cx.update(|window, _| window.take_renders()).is_empty() { still + 1 } else { 0 };
        if still == 20 {
            break;
        }
    }
    assert_eq!(still, 20, "the page came to rest before the picks");
    for id in ["moss", "denim", "pitch", "graphite"] {
        door.rules(json!({
            "sessions.list": { "reply": { "sessions": [] } },
            "workspaces.list": { "reply": { "workspaces": [] } },
            "preferences.get": { "reply": { "preferences": record(id) } },
            "preferences.set": { "push": [{ "type": "preferences.changed", "preferences": record(id) }], "reply": {} },
        }));
        let sets = door.asked_of("preferences.set").len();
        cx.update(|window, _| window.take_renders());
        let card: &'static str = Box::leak(format!("theme-option-{id}").into_boxed_str());
        let at = cx.debug_bounds(card).unwrap().center();
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.simulate_click(at, Modifiers::none());
        until(cx, "the write sent", |_| door.asked_of("preferences.set").len() > sets);
        // The answer and the record pushed with it come over the socket in a moment.
        for _ in 0..25 {
            std::thread::sleep(Duration::from_millis(20));
            frame(cx);
        }
        assert_eq!(themes(&shell, cx), (id, Some(id.to_owned())));
        let renders = cx.update(|window, _| window.take_renders());
        let most = renders.values().copied().max().unwrap_or(0);
        assert!(most <= 4, "{id}: a view rendered {most} times for one pick: {renders:?}");
        for _ in 0..30 {
            frame(cx);
        }
        let after = cx.update(|window, _| window.take_renders());
        assert!(after.is_empty(), "{id}: views still rendering half a second after the pick: {after:?}");
    }
}
