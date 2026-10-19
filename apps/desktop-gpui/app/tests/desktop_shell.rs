// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's duties around the window: the switch's files and its way back, a second launch's link over the lock's
//! socket, the links held for the next window, and the window controls a Linux desktop's frame takes from the app.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::desktop::{frame, links, switch};
use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::dialogs::startup::{self, Claim};
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AppContext, Context, Pixels, TestAppContext, Window, WindowControls};

/// Records whether the switch file still stood when today's app was opened.
struct Fake<'a> {
    home: &'a Path,
    stood: Cell<Option<bool>>,
    fails: bool,
}

impl switch::Opener for Fake<'_> {
    fn open(&self, _app: &Path) -> io::Result<()> {
        self.stood.set(Some(switch::switch_file(self.home).exists()));
        if self.fails {
            Err(io::Error::other("no such app"))
        } else {
            Ok(())
        }
    }
}

/// Catches: a way back that opens today's app before it takes the switch away, so today's app, reading the switch on
/// its launch, hands straight back to this one.
#[test]
fn the_way_back_takes_the_switch_away_before_it_opens() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("desktop")).unwrap();
    std::fs::write(home.path().join("desktop").join("experimental"), "").unwrap();
    assert!(switch::read(home.path()).on, "the file's presence is the switch on");
    let fake = Fake { home: home.path(), stood: Cell::new(None), fails: false };
    switch::go_back(home.path(), Path::new("/Applications/wsp.app/Contents/MacOS/wsp"), &fake).unwrap();
    assert_eq!(fake.stood.get(), Some(false), "the switch file went before today's app opened");
    assert!(!switch::read(home.path()).on);
    let again = Fake { home: home.path(), stood: Cell::new(None), fails: true };
    assert!(switch::go_back(home.path(), Path::new("/x/wsp"), &again).is_err(), "an open that failed says so, so the app stays");
    assert_eq!(again.stood.get(), Some(false), "and a switch already off is no failure of its own");
}

/// Catches: a launch that leaves today's app no way to find this one, or a path file half written or another's alone.
#[test]
fn a_launch_writes_where_this_app_is() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    // SAFETY: umask only swaps the process mask; a strict one shows the file's mode is set, not inherited.
    let was = unsafe { libc::umask(0o077) };
    let written = switch::write_path(home.path(), Path::new("/opt/wsp gpui/desktop-gpui"));
    // SAFETY: as above; the usual mask shows the folder's mode is the code's own.
    unsafe { libc::umask(0o022) };
    let usual = tempfile::tempdir().unwrap();
    let made = switch::write_path(usual.path(), Path::new("/opt/wsp gpui/desktop-gpui"));
    // SAFETY: as above, putting the mask back.
    unsafe { libc::umask(was) };
    written.unwrap();
    made.unwrap();
    let dir = std::fs::metadata(usual.path().join("desktop")).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir, 0o700, "the desktop folder it made is this login's alone");
    let file = home.path().join("desktop").join(switch::PATH_FILE);
    assert_eq!(switch::path_file(home.path()), file);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "/opt/wsp gpui/desktop-gpui");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o644);
    switch::write_path(home.path(), Path::new("/b/desktop-gpui")).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "/b/desktop-gpui", "each launch writes it again");
    let names: Vec<_> = std::fs::read_dir(home.path().join("desktop")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, [switch::PATH_FILE], "no part file is left");
}

/// Catches: a shim the person's wsp command holds that names an app the menu then offers, or one naming nothing that
/// still shows the row.
#[test]
fn the_way_back_is_offered_only_where_the_command_names_an_app() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(switch::named_app(home.path()), None, "no command, no way back");
    let app = home.path().join("app").join("wsp");
    std::fs::create_dir_all(app.parent().unwrap()).unwrap();
    std::fs::write(&app, "").unwrap();
    std::fs::create_dir_all(home.path().join("bin")).unwrap();
    let shim = format!("#!/bin/sh\nELECTRON_RUN_AS_NODE=1 exec '{}' '/x/cli.mjs' \"$@\"\n", app.display());
    std::fs::write(home.path().join("bin").join("wsp"), shim).unwrap();
    assert_eq!(switch::named_app(home.path()), Some(app.clone()));
    std::fs::remove_file(&app).unwrap();
    assert_eq!(switch::named_app(home.path()), None, "an app that is gone is no way back");
}

fn wait_for<T>(what: &str, mut got: impl FnMut() -> Option<T>) -> T {
    let limit = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(value) = got() {
            return value;
        }
        assert!(Instant::now() < limit, "{what} did not arrive");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Catches: a second launch's link lost on the way to the first app, as the bare connect lost it, or a bare knock
/// read as a link, or a link as a raise.
#[test]
fn a_second_launch_hands_its_link_to_the_first() {
    let home = tempfile::tempdir().unwrap();
    let Ok(Claim::Held(mut first)) = startup::claim(home.path()) else { panic!("the first launch takes the home") };
    let (mut raised, mut links) = (first.raised().unwrap(), first.links().unwrap());
    assert!(matches!(startup::claim_with(home.path(), Some("wsp://thread/t1")), Ok(Claim::Taken)));
    let link = wait_for("the link", || links.try_recv().ok());
    assert_eq!(link, "wsp://thread/t1");
    assert!(raised.try_recv().is_err(), "a link is no bare raise");
    assert!(matches!(startup::claim(home.path()), Ok(Claim::Taken)));
    wait_for("the bare knock", || raised.try_recv().ok());
    assert!(links.try_recv().is_err(), "a bare knock carries no link");
    drop(first);
}

/// Catches: a link that arrives before the app window holding the first it got, or one naming nothing taking the
/// place of one held.
#[gpui_kit::test]
fn links_before_the_window_wait_and_the_last_wins(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(links::held(cx), None);
        links::open("wsp://thread/t1", cx);
        links::open("wsp://settings/general", cx);
        links::open("wsp://thread/a/b", cx);
        let held = links::held(cx).expect("a link waits for the window");
        assert_eq!((held.kind, held.id.as_str()), (links::LinkKind::Settings, "general"));
    });
}

struct Controls(WindowControls);

impl Render for Controls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(frame::window_controls(self.0, false, cx))
    }
}

/// Catches: controls drawn off the window's top right, off the header's 52 px, or offering what the window manager
/// does not honour.
#[gpui_kit::test]
fn the_drawn_controls_stand_at_the_top_right(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Dark, cx);
    });
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| Controls(WindowControls::default()));
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    vcx.run_until_parked();
    let close = vcx.debug_bounds("window-close").expect("close is drawn");
    assert_eq!((close.right(), close.top(), close.size.width, close.size.height), (px(1280.), px(0.), frame::CONTROL_WIDTH, px(52.)));
    let min = vcx.debug_bounds("window-minimize").expect("minimize is drawn");
    let max = vcx.debug_bounds("window-maximize").expect("maximize is drawn");
    assert_eq!((min.right(), max.right()), (max.left(), close.left()), "minimize, maximize, close, left to right");

    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| Controls(WindowControls { minimize: false, maximize: false, ..WindowControls::default() }));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    assert!(vcx.debug_bounds("window-close").is_some(), "close always");
    assert!(vcx.debug_bounds("window-minimize").is_none() && vcx.debug_bounds("window-maximize").is_none());
}

/// Catches: the frame's controls standing in a window the system frames, where the desktop draws its own.
#[gpui_kit::test]
fn a_system_framed_window_draws_no_controls(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Dark, cx);
        frame::init(cx);
    });
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Empty::new(window, cx));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    assert!(vcx.debug_bounds("window-controls").is_none());
    assert_eq!(vcx.update(|window, _| frame::controls_room(window)), px(0.));
}

/// A window's content holding the focus, as the shell holds it.
struct Empty(gpui_kit::FocusHandle);

impl Empty {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Empty(focus)
    }
}

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_focus(&self.0)
    }
}

/// Catches: a zoom chord that leaves the rem where it was, or a window that does not follow the run's zoom.
#[gpui_kit::test]
fn the_zoom_chords_move_every_windows_rem(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Dark, cx);
        frame::init(cx);
        desktop_gpui::desktop::zoom::init(cfg!(target_os = "macos"), cx);
    });
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Empty::new(window, cx));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    let m = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    let rem = |vcx: &mut gpui_kit::VisualTestContext| vcx.update(|window, _| window.rem_size());
    let near = |got: Pixels, want: f32| assert!((f32::from(got) - want).abs() < 1e-3, "rem {got:?}, not {want}");
    near(rem(vcx), 16.);
    vcx.simulate_keystrokes(&format!("{m}-="));
    vcx.run_until_parked();
    near(rem(vcx), 16. * 1.2f32.sqrt());
    vcx.simulate_keystrokes(&format!("{m}-- {m}--"));
    vcx.run_until_parked();
    near(rem(vcx), 16. / 1.2f32.sqrt());
    vcx.simulate_keystrokes(&format!("{m}-0"));
    vcx.run_until_parked();
    near(rem(vcx), 16.);
}

/// A focused terminal with its own zoom binding, as the shell binds `terminal.zoomIn`, counting the presses it takes.
struct Terminal(gpui_kit::FocusHandle, std::rc::Rc<Cell<u32>>);

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let took = self.1.clone();
        div()
            .size_full()
            .key_context(keys::TERMINAL)
            .track_focus(&self.0)
            .on_action(move |_: &keys::ZoomInTerminal, _, _| took.set(took.get() + 1))
    }
}

/// Catches: the window's zoom taking the chord from a focused terminal, whose text size it means there.
#[gpui_kit::test]
fn a_focused_terminal_keeps_the_zoom_chords(cx: &mut TestAppContext) {
    let m = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Dark, cx);
        frame::init(cx);
        // Bound first, so the window's chord, bound after it, would win a tie of depth.
        cx.bind_keys([gpui_kit::KeyBinding::new(&format!("{m}-="), keys::ZoomInTerminal, Some(keys::TERMINAL))]);
        desktop_gpui::desktop::zoom::init(cfg!(target_os = "macos"), cx);
    });
    let took = std::rc::Rc::new(Cell::new(0));
    let counted = took.clone();
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Terminal(focus, counted)
        });
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    vcx.simulate_keystrokes(&format!("{m}-="));
    vcx.run_until_parked();
    assert_eq!(took.get(), 1, "the terminal took the press");
    assert_eq!(vcx.update(|window, _| window.rem_size()), px(16.), "and the window did not zoom");
}

/// Pumps the window until `done` holds, up to 30 s.
fn until(cx: &mut gpui_kit::VisualTestContext, what: &str, mut done: impl FnMut(&mut gpui_kit::VisualTestContext) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    while !done(cx) {
        assert!(Instant::now() < limit, "{what} did not happen");
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

/// Catches: a link held before the window that the first window does not open on, a link with the window up that
/// moves nothing, a link to a thread the host lacks that says nothing, and a malformed link that moves the window.
#[gpui_kit::test]
fn links_open_in_the_app_window(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::palette::init(cx);
        links::open("wsp://settings/usage", cx);
    });
    let host = &common::shared().host;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let (state, env) = (host.state(), host.env());
    let mut shell = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        links::attach(window, &view, cx);
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    vcx.simulate_resize(size(px(1280.), px(800.)));
    let route = |vcx: &mut gpui_kit::VisualTestContext| shell.read_with(vcx, |s, _| s.route().clone());
    until(vcx, "the held link's opening", |vcx| route(vcx) == Route::Screen("settings:usage".into()));
    assert_eq!(vcx.update(|_, cx| links::held(cx)), None, "the first window took the held link");

    until(vcx, "the host's threads", |vcx| shell.read_with(vcx, |s, cx| !s.sidebar().read(cx).items().is_empty()));
    let item = shell.read_with(vcx, |s, cx| s.sidebar().read(cx).items()[0].clone());
    vcx.update(|_, cx| links::open(&format!("wsp://thread/{}", item.thread_id), cx));
    let opened = Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() };
    until(vcx, "the thread's opening", |vcx| route(vcx) == opened);

    vcx.update(|_, cx| links::open("wsp://thread/a/b", cx));
    vcx.update(|_, cx| links::open("wsp://thread/nope", cx));
    let said = |vcx: &mut gpui_kit::VisualTestContext| {
        vcx.update(|window, cx| {
            let toasts = desktop_gpui::ui::toast::of(window, cx).expect("the window's toasts");
            toasts.read(cx).standing().iter().map(|n| n.text.to_string()).collect::<Vec<_>>()
        })
    };
    until(vcx, "the gone line", |vcx| !said(vcx).is_empty());
    assert_eq!(said(vcx), ["That link names a thread this wsp does not have."]);
    assert_eq!(route(vcx), opened, "neither the malformed link nor the gone one moved the window");
}

/// A header row as the frame: a button, the panel toggles, and its own ground between them.
struct Header;

impl Render for Header {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        frame::drag_region(gpui_kit::component::h_flex().id("row").debug_selector(|| "row".into()).w_full().h(px(52.)).items_center())
            .child(desktop_gpui::ui::Button::icon("new-thread", desktop_gpui::ui::Glyph::SquarePen).on_click(|_, _, _| {}))
            .child(div().id("ground").debug_selector(|| "ground".into()).w(px(200.)).h_full())
            .child(desktop_gpui::screens::app_shell::frame::layout_controls(false, false, |_, _, _| {}, |_, _, _| {}, cx))
    }
}

fn header(cx: &mut TestAppContext) -> &mut gpui_kit::VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Dark, cx);
    });
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| Header);
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(size(px(1280.), px(800.)));
    vcx.run_until_parked();
    vcx
}

fn center(vcx: &mut gpui_kit::VisualTestContext, selector: &'static str) -> gpui_kit::Point<Pixels> {
    vcx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center()
}

/// A press then a 1 px move with the button held; a window move on the test platform's window panics.
fn press_and_nudge(vcx: &mut gpui_kit::VisualTestContext, at: gpui_kit::Point<Pixels>) {
    vcx.simulate_mouse_down(at, gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    vcx.simulate_mouse_move(at + gpui_kit::point(px(1.), px(0.)), gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
    vcx.simulate_mouse_up(at + gpui_kit::point(px(1.), px(0.)), gpui_kit::MouseButton::Left, gpui_kit::Modifiers::none());
}

/// Catches: a press on a header button or toggle that a tremor of the pointer turns into a window move, losing the click.
#[gpui_kit::test]
fn a_press_on_a_header_button_does_not_move_the_window(cx: &mut TestAppContext) {
    let vcx = header(cx);
    let button = center(vcx, "button");
    press_and_nudge(vcx, button);
    // The drawer's toggle, 28 px wide, starts where the ground ends.
    let ground = vcx.debug_bounds("ground").expect("the ground is drawn");
    press_and_nudge(vcx, gpui_kit::point(ground.right() + px(14.), ground.center().y));
}

/// The row's own ground still moves the window: the test platform's window move is unimplemented, so it panics.
#[gpui_kit::test]
#[should_panic(expected = "not implemented")]
fn a_press_on_the_header_ground_moves_the_window(cx: &mut TestAppContext) {
    let vcx = header(cx);
    let ground = center(vcx, "ground");
    press_and_nudge(vcx, ground);
}
