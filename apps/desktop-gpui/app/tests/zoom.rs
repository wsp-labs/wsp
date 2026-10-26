// SPDX-License-Identifier: AGPL-3.0-only
//! The View menu's zoom scales the whole window as Chromium's page zoom does, so what reaches the window from the
//! platform (the mouse) and what goes back to it (the input method's bounds) follow the zoom. The test window reports
//! the platform's pixels; each press of the chord here is the app's own path.

use std::cell::Cell;
use std::rc::Rc;

use desktop_gpui::desktop::{frame, zoom};
use desktop_gpui::screens::app_shell::menu::Os;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, App, AppContext, Bounds, Context, Decorations, Entity, ExternalPaths, FileDropEvent, Modifiers, Pixels,
    ScrollDelta, ScrollHandle, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, Window,
};

/// The Mac's chords, which a test presses on any platform; Linux binds none.
const M: &str = "cmd";

fn init_on(os: Os, cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        frame::init(cx);
        zoom::init(os, cx);
    });
}

fn init(cx: &mut TestAppContext) {
    init_on(Os::Mac, cx);
}

/// Two 100 px boxes side by side at the window's top left, each counting the clicks it takes.
struct Boxes {
    focus: gpui_kit::FocusHandle,
    left: Rc<Cell<u32>>,
    right: Rc<Cell<u32>>,
}

impl Render for Boxes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let counted =
            |id: &'static str, took: Rc<Cell<u32>>| div().id(id).w(px(100.)).h(px(100.)).on_click(move |_, _, _| took.set(took.get() + 1));
        div()
            .size_full()
            .track_focus(&self.focus)
            .child(div().flex().child(counted("left", self.left.clone())).child(counted("right", self.right.clone())))
    }
}

fn press(vcx: &mut VisualTestContext, chord: &str) {
    vcx.simulate_keystrokes(chord);
    vcx.run_until_parked();
}

/// Catches: a click read at the platform's position instead of the zoomed window's, which lands on what the window
/// would draw there unzoomed.
#[gpui_kit::test]
fn a_click_lands_on_what_the_zoomed_window_draws_there(cx: &mut TestAppContext) {
    init(cx);
    let (left, right) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (l, r) = (left.clone(), right.clone());
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Boxes { focus, left: l, right: r }
        });
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    let at = point(px(110.), px(50.));
    vcx.simulate_click(at, Modifiers::none());
    assert_eq!((left.get(), right.get()), (0, 1), "unzoomed, 110 px is inside the right box");

    press(vcx, &format!("{M}-= {M}-="));
    assert!((vcx.update(|window, _| window.zoom()) - 1.2).abs() < 1e-6);
    vcx.simulate_click(at, Modifiers::none());
    assert_eq!((left.get(), right.get()), (1, 1), "at 1.2x the left box is drawn out to 120 px, so 110 px is on it");
}

struct Field(Entity<InputState>);

impl Render for Field {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(div().w(px(240.)).child(Input::new(&self.0)))
    }
}

/// Within one device pixel, half a point on the test window's 2x screen, as layout snaps to the zoomed device grid.
fn near(got: Bounds<Pixels>, want: Bounds<Pixels>, what: &str) {
    let parts = |b: Bounds<Pixels>| [b.origin.x, b.origin.y, b.size.width, b.size.height].map(f32::from);
    let off = parts(got).iter().zip(parts(want)).any(|(g, w)| (g - w).abs() > 0.5);
    assert!(!off, "{what}: got {got:?}, want {want:?}");
}

fn scaled(b: Bounds<Pixels>, by: f32) -> Bounds<Pixels> {
    b.map(|v| v * by)
}

/// Catches: the input method handed the window's own pixels at a zoom (its candidate window then opens short of the
/// text), a point from the input method read unzoomed, or the candidate position the window pushes left unzoomed.
#[gpui_kit::test]
fn the_input_method_reads_a_fields_bounds_in_the_platforms_pixels(cx: &mut TestAppContext) {
    init(cx);
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let state = cx.new(|cx| InputState::new(window, cx).default_value("hello world"));
        state.update(cx, |s, cx| s.focus(window, cx));
        let view = cx.new(|_| Field(state));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    let ime_position = |vcx: &mut VisualTestContext| {
        vcx.update(|window, cx| {
            window.invalidate_character_coordinates();
            window.simulate_next_frame(cx);
        });
        vcx.platform_ime_position().expect("the window handed the input method a position")
    };
    let actual = vcx.platform_bounds_for_range(6..7).expect("the field answers the input method");
    let actual_ime = ime_position(vcx);
    let probe = point(actual.origin.x + px(1.), actual.origin.y + actual.size.height / 2.);
    assert_eq!(vcx.platform_character_index_for_point(probe), Some(6));

    press(vcx, &format!("{M}-= {M}-="));
    let zoomed = vcx.platform_bounds_for_range(6..7).unwrap();
    near(zoomed, scaled(actual, 1.2), "the input method's bounds for the 'w'");
    near(ime_position(vcx), scaled(actual_ime, 1.2), "the candidate position the window pushes");
    assert_eq!(vcx.platform_character_index_for_point(probe * 1.2), Some(6), "the input method's point, at 1.2x");
}

/// Catches: the chords leaving the window's zoom where it was, or Actual Size not going back to 1.
#[gpui_kit::test]
fn the_chords_zoom_the_window_and_actual_size_resets_it(cx: &mut TestAppContext) {
    init(cx);
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Boxes { focus, left: Rc::default(), right: Rc::default() }
        });
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    let zoom = |vcx: &mut VisualTestContext| vcx.update(|window, _| (window.zoom(), window.rem_size()));
    let viewport = vcx.update(|window, _| window.viewport_size());
    press(vcx, &format!("{M}-="));
    let (z, rem) = zoom(vcx);
    assert!((z - 1.2f32.sqrt()).abs() < 1e-6, "one step is half a level: {z}");
    assert_eq!(rem, px(16.), "the rem stays the theme's; the window scales instead");
    let shrunk = vcx.update(|window, _| window.viewport_size());
    assert!((f32::from(shrunk.width) * z - f32::from(viewport.width)).abs() < 0.01, "{shrunk:?} at {z} of {viewport:?}");
    press(vcx, &format!("{M}-- {M}--"));
    assert!((zoom(vcx).0 - 1. / 1.2f32.sqrt()).abs() < 1e-6);
    press(vcx, &format!("{M}-0"));
    assert_eq!(zoom(vcx).0, 1.);
}

/// Catches: the app-drawn border finding its right edge from the window's size on screen, which at 1.2x lies past
/// where the zoomed window draws it, so a press on the drawn edge resizes nothing. Only Linux's drawn frame has its
/// resize band outside the content.
#[cfg(target_os = "linux")]
#[gpui_kit::test]
fn a_press_on_the_drawn_right_edge_resizes_a_zoomed_window(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, ResizeEdge, Tiling};
    init(cx);
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Boxes { focus, left: Rc::default(), right: Rc::default() }
        });
        Root::new(view, window, cx)
    });
    vcx.simulate_window_decorations(Decorations::Client { tiling: Tiling::default() });
    press(vcx, &format!("{M}-= {M}-="));
    let viewport = vcx.update(|window, _| window.viewport_size());
    // gpui-kit's border draws its shadow 20 px wide on Linux; the press lands 2 px outside the frame.
    let at = point(px((f32::from(viewport.width) - 20. + 2.) * 1.2), px(f32::from(viewport.height) / 2. * 1.2));
    vcx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    vcx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    assert_eq!(vcx.platform_window_resizes(), vec![ResizeEdge::Right]);
}

/// Catches: the app drawing resize edges into a zoomed window whose frame is the system's, a Mac's always, which would
/// take a press on the content's right edge from the content.
#[gpui_kit::test]
fn a_zoomed_window_in_the_systems_frame_leaves_its_edges_to_the_system(cx: &mut TestAppContext) {
    init(cx);
    let (left, right) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (l, r) = (left.clone(), right.clone());
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Boxes { focus, left: l, right: r }
        });
        Root::new(view, window, cx)
    });
    vcx.simulate_window_decorations(Decorations::Server);
    press(vcx, &format!("{M}-= {M}-="));
    vcx.simulate_resize(gpui_kit::size(px(240.), px(240.)));
    // 2 px in from the 200 px content's right edge, on the right box, where Linux's drawn frame has its resize band.
    vcx.simulate_click(point(px(198. * 1.2), px(50. * 1.2)), Modifiers::none());
    assert_eq!(vcx.platform_window_resizes(), vec![], "no resize asked of the platform");
    assert_eq!((left.get(), right.get()), (0, 1), "the press is the content's");
}

fn boxes_window(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Boxes { focus, left: Rc::default(), right: Rc::default() }
        });
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    vcx
}

/// Catches: the window binding zoom chords on Linux, where Electron sets no menu and so no accelerator.
#[gpui_kit::test]
fn linux_binds_no_window_zoom_chord(cx: &mut TestAppContext) {
    init_on(Os::Linux, cx);
    let vcx = boxes_window(cx);
    press(vcx, "ctrl-= ctrl-= cmd-= ctrl--");
    assert_eq!(vcx.update(|window, _| window.zoom()), 1.);
}

/// Catches: a window opened at a launch keeping actual size though the store kept a level, as a window that does not
/// read the run's zoom when it opens would.
#[gpui_kit::test]
fn a_window_opens_at_the_level_the_store_kept(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("desktop-gpui-zoom-opens-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("local-store.json");
    std::fs::write(&path, format!("{{\"{}\": 1.0}}", zoom::KEY)).unwrap();
    cx.update(|cx| desktop_gpui::local_store::init(&path, cx));
    init(cx);
    let vcx = boxes_window(cx);
    let (z, viewport, screen) = vcx.update(|window, _| (window.zoom(), window.viewport_size(), window.window_bounds().get_bounds().size));
    assert!((z - 1.2).abs() < 1e-6, "zoom {z}");
    assert!((f32::from(viewport.width) * 1.2 - f32::from(screen.width)).abs() < 0.01, "{viewport:?} of {screen:?}");
    assert!((f32::from(viewport.height) * 1.2 - f32::from(screen.height)).abs() < 0.01, "{viewport:?} of {screen:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

struct Scroller(ScrollHandle, gpui_kit::FocusHandle);

impl Render for Scroller {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.1)
            .child(div().id("scroller").w(px(100.)).h(px(100.)).overflow_y_scroll().track_scroll(&self.0).child(div().h(px(1000.))))
    }
}

/// A focus handle focused in `window`, so the window's chords reach the view.
fn focused(window: &mut Window, cx: &mut App) -> gpui_kit::FocusHandle {
    let focus = cx.focus_handle();
    focus.focus(window, cx);
    focus
}

/// Catches: a trackpad's scroll read at the platform's position, which lands past the zoomed list, or by the platform's
/// distance, which moves the content 1.2 times as far as the fingers.
#[gpui_kit::test]
fn a_pixel_scroll_moves_the_zoomed_content_as_far_as_the_fingers(cx: &mut TestAppContext) {
    init(cx);
    let handle = ScrollHandle::new();
    let held = handle.clone();
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| Scroller(held, focused(window, cx)));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    press(vcx, &format!("{M}-= {M}-="));
    // A move first, as a hand on the trackpad makes, so the window is back on the mouse after the chords.
    vcx.simulate_mouse_move(point(px(110.), px(50.)), None, Modifiers::none());
    vcx.simulate_event(ScrollWheelEvent {
        position: point(px(110.), px(50.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-60.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    vcx.run_until_parked();
    let y = f32::from(handle.offset().y);
    assert!((y + 50.).abs() < 0.01, "60 points at 1.2x is 50 of the window's pixels; scrolled {y}");
}

struct Drops(Rc<Cell<u32>>, Rc<Cell<u32>>, gpui_kit::FocusHandle);

impl Render for Drops {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let target = |id: &'static str, took: Rc<Cell<u32>>| {
            div().id(id).w(px(100.)).h(px(100.)).on_drop(move |_: &ExternalPaths, _, _| took.set(took.get() + 1))
        };
        div()
            .size_full()
            .track_focus(&self.2)
            .child(div().flex().child(target("left", self.0.clone())).child(target("right", self.1.clone())))
    }
}

/// Catches: files dropped from another app read at the platform's position, which at 1.2x lands on the box to the
/// right of the one drawn under the pointer.
#[gpui_kit::test]
fn files_dropped_land_on_what_the_zoomed_window_draws_there(cx: &mut TestAppContext) {
    init(cx);
    let (left, right) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (l, r) = (left.clone(), right.clone());
    let (_, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| Drops(l, r, focused(window, cx)));
        Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    press(vcx, &format!("{M}-= {M}-="));
    let position = point(px(110.), px(50.));
    let paths = ExternalPaths(vec![std::path::PathBuf::from("/tmp/a.txt")].into());
    assert!((vcx.update(|window, _| window.zoom()) - 1.2).abs() < 1e-6);
    vcx.simulate_mouse_move(position, None, Modifiers::none());
    vcx.simulate_event(FileDropEvent::Entered { position, paths });
    vcx.simulate_event(FileDropEvent::Pending { position });
    vcx.simulate_event(FileDropEvent::Submit { position });
    assert_eq!((left.get(), right.get()), (1, 0), "at 1.2x the left box is drawn out to 120 px");
}

struct Menu(Entity<PopupMenu>, gpui_kit::FocusHandle);

impl Render for Menu {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.1)
            .child(div().absolute().top_0().left_0().debug_selector(|| "menu".into()).child(self.0.clone()))
    }
}

/// Catches: a long menu capped at half the window's height on screen, which at 1.2x lets it run past half the
/// window it is drawn in.
#[gpui_kit::test]
fn a_long_menu_stops_at_half_the_zoomed_window(cx: &mut TestAppContext) {
    init(cx);
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let menu =
            PopupMenu::build(window, cx, |menu, _, _| (0..60).fold(menu.scrollable(true), |menu, ix| menu.label(format!("Row {ix}"))));
        let view = cx.new(|cx| Menu(menu, focused(window, cx)));
        Root::new(view, window, cx)
    });
    vcx.simulate_resize(gpui_kit::size(px(1000.), px(800.)));
    press(vcx, &format!("{M}-= {M}-="));
    assert!((vcx.update(|window, _| window.zoom()) - 1.2).abs() < 1e-6);
    let half = f32::from(vcx.update(|window, _| window.viewport_size().height)) / 2.;
    let height = f32::from(vcx.debug_bounds("menu").expect("the menu is drawn").size.height);
    assert!(height <= half + 0.5 && height > half - 40., "menu {height} px tall in a window {half} px to its middle");
}
