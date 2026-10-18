// SPDX-License-Identifier: AGPL-3.0-only
//! The command palette through the road the app takes: the shell inside gpui-kit's Root after `gpui_kit::init`, wsp's
//! themes and the layers' registration, the default chords bound as at launch. The keys, the pointer and the runs
//! are checked on a window with no host; the message search, Copy as Markdown and the page of projects on a host
//! started for these tests alone.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys::{self, DEFAULTS};
use desktop_gpui::screens::palette::palette::{self as pal, Palette};
use desktop_gpui::screens::palette::{frame, overlays};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::sidebar::ThreadItem;
use gpui_kit::component::Root;
use gpui_kit::{point, px, size, AppContext, Entity, Modifiers, SharedString, TestAppContext, VisualTestContext};
use wsp_host_client::Env;

const MAC: bool = cfg!(target_os = "macos");

/// The shell in a 1280 by 800 window, as main.rs builds it, on `state` with `env`.
fn open(cx: &mut TestAppContext, state: std::path::PathBuf, env: Env) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
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

/// A window on a state no host serves, its sidebar holding threads in two tasks and the first one open; its HOME
/// goes with the folder handed back.
fn hostless(cx: &mut TestAppContext) -> (tempfile::TempDir, Entity<Shell>, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    let home = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (shell, cx) = open(cx, dir.path().join("state.json"), home);
    let item = |w: &str, t: &str| ThreadItem { workspace_id: w.into(), thread_id: t.into(), project: "p".into(), title: t.into() };
    shell.update_in(cx, |shell, window, cx| {
        shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("w1", "a"), item("w1", "b"), item("w2", "c")], cx));
        shell.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx);
    });
    cx.run_until_parked();
    (dir, shell, cx)
}

fn press_chord(cx: &mut VisualTestContext, chord: &str) {
    let typed = keys::keystroke(chord, MAC).unwrap_or_else(|| panic!("{chord} does not read"));
    cx.simulate_keystrokes(&typed);
    cx.run_until_parked();
}

fn palette(cx: &mut VisualTestContext) -> Option<Entity<Palette>> {
    cx.update(|window, cx| pal::palette(window, cx))
}

fn active(cx: &mut VisualTestContext) -> Option<SharedString> {
    let palette = palette(cx).expect("the palette is open");
    palette.read_with(cx, |p, _| p.active_row())
}

fn listed(cx: &mut VisualTestContext, group: &str) -> Vec<String> {
    let palette = palette(cx).expect("the palette is open");
    palette.read_with(cx, |p, _| {
        p.lists().iter().filter(|g| g.value == group).flat_map(|g| g.items.iter().map(|i| i.value.to_string())).collect()
    })
}

fn route(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Route {
    shell.read_with(cx, |s, _| s.route().clone())
}

fn typed(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    cx.run_until_parked();
}

fn mod_k(cx: &mut VisualTestContext) {
    press_chord(cx, "mod+k");
}

/// Runs the app's tasks until `done` holds, as the host answers from its own threads.
fn wait_for(cx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < until, "never saw {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn both_chords_open_it_where_the_web_puts_it_and_mod_k_or_escape_shut_it(cx: &mut TestAppContext) {
    let (_home, _, cx) = hostless(cx);
    for chord in DEFAULTS.iter().filter(|r| r.command == "commandPalette.toggle").map(|r| r.chord) {
        assert!(palette(cx).is_none());
        press_chord(cx, chord);
        assert!(palette(cx).is_some(), "{chord} opens the palette");
        let popup = cx.debug_bounds("palette-popup").expect("the popup is drawn");
        assert_eq!(popup.size.width, frame::POPUP_WIDTH, "max-w-xl");
        assert_eq!(popup.origin.y, px(80.), "10vh down from the top");
        assert_eq!(popup.origin.x, (px(1280.) - frame::POPUP_WIDTH) / 2., "centred");
        assert!(popup.size.height <= frame::POPUP_MAX, "max-h-105");
        let scrim = cx.debug_bounds("palette-scrim").expect("the scrim is drawn");
        assert_eq!(scrim.size, size(px(1280.), px(800.)), "the scrim covers the window");
        press_chord(cx, chord);
        assert!(palette(cx).is_none(), "{chord} again shuts it from inside its own field");
    }
    mod_k(cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(palette(cx).is_none(), "Escape shuts it");
    assert!(cx.debug_bounds("palette-popup").is_none(), "and nothing of it is drawn");
}

#[gpui_kit::test]
fn a_press_inside_the_popup_keeps_it_and_one_on_the_scrim_shuts_it(cx: &mut TestAppContext) {
    let (_home, _, cx) = hostless(cx);
    mod_k(cx);
    let popup = cx.debug_bounds("palette-popup").unwrap();
    cx.simulate_click(point(popup.center().x, popup.origin.y + px(20.)), Modifiers::none());
    cx.run_until_parked();
    assert!(palette(cx).is_some(), "a press on the search row keeps it open");
    let label = popup.origin + point(px(30.), px(65.));
    cx.simulate_click(label, Modifiers::none());
    cx.run_until_parked();
    assert!(palette(cx).is_some(), "a press on a group's label keeps it open");
    cx.simulate_click(point(px(20.), px(780.)), Modifiers::none());
    cx.run_until_parked();
    assert!(palette(cx).is_none(), "a press on the scrim shuts it");
}

#[gpui_kit::test]
fn arrows_walk_the_rows_that_run_and_wrap(cx: &mut TestAppContext) {
    let (_home, _, cx) = hostless(cx);
    mod_k(cx);
    assert_eq!(active(cx).as_deref(), Some("action:add-project"), "the first row that runs is lit; New thread is held with no host");
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    assert_eq!(active(cx).as_deref(), Some("action:copy-markdown"));
    cx.simulate_keystrokes("up up");
    cx.run_until_parked();
    assert_eq!(active(cx).as_deref(), Some("workspace:w2"), "up from the first wraps to the last");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let list = cx.debug_bounds("palette-list").expect("the list is drawn");
    let row = cx.debug_bounds("palette-row-workspace:w2").expect("the last row is drawn");
    assert!(row.top() >= list.top() && row.bottom() <= list.bottom(), "the list scrolls the lit row into view: {row:?} in {list:?}");
    let described = cx.debug_bounds("palette-row-action:add-project").expect("Add a project is drawn");
    assert_eq!(described.size.height, px(48.), "a row with a line under its title keeps its 48 px however long the list");
    typed(cx, "toggle side");
    assert_eq!(active(cx).as_deref(), Some("action:toggle-sidebar"), "typing makes the first row the one Enter runs");
    let lit = palette(cx).expect("the palette is open").read_with(cx, |p, _| p.lit_row());
    assert_eq!(lit, None, "and draws it unlit, as the web's list does after a keystroke");
}

#[gpui_kit::test]
fn shutting_it_hands_the_keys_back_to_what_held_them(cx: &mut TestAppContext) {
    let (_home, shell, cx) = hostless(cx);
    let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
    // The right panel holds the keys, not the shell's root, which takes them back on its own once a focus is lost.
    press_chord(cx, "mod+alt+b");
    let panel = shell.read_with(cx, |s, cx| gpui_kit::Focusable::focus_handle(s.panel().read(cx), cx));
    cx.update(|window, cx| window.focus(&panel, cx));
    cx.run_until_parked();
    let before = focused(cx).expect("the panel holds the keys");
    assert_eq!(before, panel);
    mod_k(cx);
    assert_ne!(focused(cx).as_ref(), Some(&before), "the palette's field takes the keys");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(focused(cx), Some(before), "Escape gives them back");
}

#[gpui_kit::test]
fn enter_runs_the_lit_row_after_the_palette_shuts(cx: &mut TestAppContext) {
    let (_home, shell, cx) = hostless(cx);
    mod_k(cx);
    typed(cx, "toggle sidebar");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(palette(cx).is_none(), "the palette shuts before its row runs");
    assert!(!shell.read_with(cx, |s, _| s.sidebar_open()), "Toggle sidebar reached the shell");
    mod_k(cx);
    typed(cx, "settings");
    assert_eq!(listed(cx, "actions").first().map(String::as_str), Some("action:settings"));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(route(&shell, cx), Route::Screen("settings".into()), "Settings opens Settings");
    mod_k(cx);
    typed(cx, "settings");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(route(&shell, cx), Route::Screen("settings".into()), "and keeps it open where it stands open");
}

#[gpui_kit::test]
fn the_next_task_row_steps_at_once_and_leaves_the_switcher_shut(cx: &mut TestAppContext) {
    let (_home, shell, cx) = hostless(cx);
    let switcher_open = |cx: &mut VisualTestContext| cx.update(|window, cx| overlays::of(window, cx).unwrap().read(cx).switcher.is_some());
    mod_k(cx);
    typed(cx, "next task");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(route(&shell, cx), Route::Thread { workspace_id: "w2".into(), thread_id: "c".into() }, "Next task lands on the next task");
    assert!(!switcher_open(cx), "and no switcher waits for a key to come up");
}

#[gpui_kit::test]
fn a_click_runs_its_row_and_a_held_row_takes_no_press(cx: &mut TestAppContext) {
    let (_home, shell, cx) = hostless(cx);
    mod_k(cx);
    typed(cx, "new thread");
    let held = cx.debug_bounds("palette-row-action:new-thread").expect("the held row is drawn");
    cx.simulate_click(held.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(palette(cx).is_some(), "a held row runs nothing and keeps the palette");
    cx.simulate_keystrokes("secondary-a backspace");
    typed(cx, "toggle sidebar");
    let row = cx.debug_bounds("palette-row-action:toggle-sidebar").unwrap();
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(palette(cx).is_none());
    assert!(!shell.read_with(cx, |s, _| s.sidebar_open()));
}

/// The shared host's window, once its sidebar lists the first thread.
fn live(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, String, String) {
    let host = &common::shared().host;
    let (shell, cx) = open(cx, host.state(), host.env());
    wait_for(cx, "the first thread in the sidebar", |cx| {
        shell.read_with(cx, |s, cx| s.connected() && !s.sidebar().read(cx).items().is_empty())
    });
    let item =
        shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().iter().find(|i| i.title == "first").cloned().expect("the first thread"));
    (shell, cx, item.workspace_id, item.thread_id)
}

#[gpui_kit::test]
fn the_hosts_message_hits_list_under_in_messages(cx: &mut TestAppContext) {
    let (_, cx, _, thread) = live(cx);
    mod_k(cx);
    typed(cx, "ok");
    let hit = format!("message:{thread}");
    cx.executor().advance_clock(pal::SEARCH_WAIT - Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(listed(cx, "messages-search").is_empty(), "nothing is asked before the typing has rested 200 ms");
    cx.executor().advance_clock(Duration::from_millis(1));
    wait_for(cx, "the hit on the first thread", |cx| listed(cx, "messages-search").contains(&hit));
    let says = palette(cx)
        .unwrap()
        .read_with(cx, |p, _| p.lists().iter().flat_map(|g| &g.items).find(|i| i.value == hit).and_then(|i| i.says.clone()));
    assert!(format!("{says:?}").contains("ok"), "the row says the snippet: {says:?}");
    typed(cx, "x");
    cx.executor().advance_clock(pal::SEARCH_WAIT);
    cx.run_until_parked();
    assert!(listed(cx, "messages-search").is_empty(), "hits for other words are not drawn: {:?}", listed(cx, "messages-search"));
}

#[gpui_kit::test]
fn copy_as_markdown_reads_the_whole_thread_from_the_host(cx: &mut TestAppContext) {
    let (shell, cx, workspace, thread) = live(cx);
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id: workspace, thread_id: thread }, window, cx));
    cx.run_until_parked();
    cx.update(|_, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(String::new())));
    mod_k(cx);
    typed(cx, "markdown");
    wait_for(cx, "Copy as Markdown for the open thread", |cx| active(cx).as_deref() == Some("action:copy-markdown"));
    cx.simulate_keystrokes("enter");
    let clip = |cx: &mut VisualTestContext| cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default());
    wait_for(cx, "the thread on the clipboard", |cx| !clip(cx).is_empty());
    let text = clip(cx);
    assert!(text.starts_with("**You**"), "{text}");
    assert!(text.contains("\nfirst\n") && text.contains("**Agent**") && text.contains("ok"), "{text}");
}

#[gpui_kit::test]
fn the_page_of_projects_goes_back_on_backspace_and_picks_on_a_mod_digit(cx: &mut TestAppContext) {
    let (shell, cx, _, _) = live(cx);
    mod_k(cx);
    typed(cx, "new thread in");
    wait_for(cx, "New thread in... with a project to pick", |cx| active(cx).as_deref() == Some("page:new-thread"));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let page = palette(cx).expect("the page keeps the palette open");
    assert!(page.read_with(cx, |p, _| p.on_page()));
    let rows = listed(cx, "projects");
    assert!(
        rows.first().is_some_and(|r| r.starts_with("project:")) && rows.last().map(String::as_str) == Some("action:new-project"),
        "{rows:?}"
    );
    assert!(cx.debug_bounds("palette-back").is_some(), "Back stands where the search glyph stood");
    assert_eq!(page.read_with(cx, |p, _| p.lit_row()), None, "the page Enter opens draws no row lit");
    let row = |at: usize| rows.get(at).map(|r| SharedString::from(r.clone()));
    assert_eq!(page.read_with(cx, |p, _| p.active_row()), row(0), "though Enter would run the first");
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    assert_eq!(page.read_with(cx, |p, _| p.lit_row()), row(1), "down lights the row after the one Enter would run");
    typed(cx, "zzqx");
    assert!(page.read_with(cx, |p, _| p.lists().is_empty()));
    cx.simulate_keystrokes("backspace backspace backspace backspace");
    cx.run_until_parked();
    assert!(page.read_with(cx, |p, _| p.on_page()), "Backspace deletes the words first");
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert!(!page.read_with(cx, |p, _| p.on_page()), "Backspace on an empty search goes back");
    typed(cx, "new thread in");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    press_chord(cx, "mod+1");
    assert!(palette(cx).is_none(), "a mod digit runs the page's row");
    let Route::Screen(at) = route(&shell, cx) else { panic!("the page's row opens New thread") };
    assert!(at.starts_with("composer:"), "{at}");
}

/// The palette at rest in Paper and Graphite drawn by GPUI's offscreen renderer at 1280 x 800, written to the folder
/// `PALETTE_PNG` names as `rest-<theme>.png` and `page-<theme>.png`, for a look against Electron's shots of the same
/// size and scale. On a window with no host its rows are the ones a hostless window lists.
#[test]
#[ignore = "writes captures for a look; run with --ignored and PALETTE_PNG=<folder>"]
fn shoots_the_palette() {
    use desktop_gpui::screens::app_shell::assets::AppAssets;
    use desktop_gpui::ui::Still;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AnyWindowHandle, Bounds, HeadlessAppContext, WindowBounds, WindowOptions};
    let Some(folder) = std::env::var_os("PALETTE_PNG").map(std::path::PathBuf::from) else { return };
    for theme in ["paper", "graphite"] {
        for page in [false, true] {
            let mut cx = HeadlessAppContext::with_platform(
                gpui_kit::platform::current_platform(true).text_system(),
                Arc::new(AppAssets),
                gpui_kit::platform::current_headless_renderer,
            );
            cx.update(|cx| {
                gpui_kit::init(cx);
                desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
                desktop_gpui::screens::palette::init(cx);
                cx.set_global(Still(true));
                desktop_gpui::ui::theme::set_theme(theme, cx);
            });
            let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
            let dir = tempfile::tempdir().unwrap();
            let (state, home) =
                (dir.path().join("state.json"), [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect());
            let (handle, shell) = cx.update(|cx| {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(1280.), px(800.)) })),
                    focus: false,
                    show: false,
                    ..Default::default()
                };
                let mut shell = None;
                let handle = cx
                    .open_window(options, |window, cx| {
                        let view = cx.new(|cx| Shell::new(state, home, runtime, Launch::default(), window, cx));
                        shell = Some(view.clone());
                        cx.new(|cx| Root::new(view, window, cx))
                    })
                    .unwrap();
                let handle: AnyWindowHandle = handle.into();
                (handle, shell.unwrap())
            });
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                let item =
                    |w: &str, t: &str| ThreadItem { workspace_id: w.into(), thread_id: t.into(), project: "p".into(), title: t.into() };
                shell.update(cx, |shell, cx| {
                    shell.sidebar().update(cx, |s, cx| s.set_items(vec![item("w1", "a"), item("w1", "b"), item("w2", "c")], cx));
                    shell.go(Route::Thread { workspace_id: "w1".into(), thread_id: "a".into() }, window, cx);
                });
                pal::open(window, cx);
            })
            .unwrap();
            cx.update(|cx| desktop_gpui::ui::theme::set_theme(theme, cx));
            if page {
                cx.update_window(handle, |_, window, cx| {
                    let palette = pal::palette(window, cx).unwrap();
                    palette.update(cx, |p, cx| p.show_page(window, cx));
                })
                .unwrap();
            }
            for _ in 0..3 {
                cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
                cx.run_until_parked();
            }
            let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
            image.save(folder.join(format!("{}-{theme}.png", if page { "page" } else { "rest" }))).unwrap();
        }
    }
}
