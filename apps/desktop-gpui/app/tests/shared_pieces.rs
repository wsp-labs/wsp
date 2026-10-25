// SPDX-License-Identifier: AGPL-3.0-only
//! The shared chart, sparkline, skeleton, tooltip and right press menu drawn by GPUI's real offscreen renderer and read
//! back as pixels and boxes. `SHARED_PNG=<folder>` writes each scene there. Without libtest's harness, as `pixels.rs`
//! explains.

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::ui::{self, Axis, Chart, ChartLine, Side, Sparkline, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{div, prelude::*, px, size, AnyWindowHandle, Context, HeadlessAppContext, Window};
use image::RgbaImage;

const W: f32 = 480.;
const H: f32 = 360.;

fn app(theme: &str, still: bool) -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(still));
        ui::theme::set_theme(theme, cx);
    });
    cx
}

struct Scene(fn(&mut Window, &mut gpui_kit::App) -> gpui_kit::AnyElement);

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use desktop_gpui::ui::ActivePalette as _;
        div().size_full().bg(cx.palette().background).child((self.0)(window, cx))
    }
}

fn open(cx: &mut HeadlessAppContext, scene: fn(&mut Window, &mut gpui_kit::App) -> gpui_kit::AnyElement) -> AnyWindowHandle {
    cx.open_window(size(px(W), px(H)), move |window, cx| {
        let view = cx.new(|_| Scene(scene));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .unwrap()
    .into()
}

fn draw(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> RgbaImage {
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

fn chart(_: &mut Window, _: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    let line = |key: &'static str, points: Vec<f64>, ink| ChartLine { key: key.into(), label: key.into(), points, ink };
    let lines = vec![
        line("lead", vec![10., 60., 35., 80., 50., 90., 70.], gpui_kit::rgb(0x2563eb).into()),
        line("rest", vec![5., 20., 45., 25., 30., 15., 40.], gpui_kit::rgb(0xdc2626).into()),
    ];
    let chart = Chart::new("plot", (0..7).map(f64::from).collect(), lines, Vec::new())
        .axis(Axis { from: 0., to: 100., parts: 4 })
        .height(px(160.))
        .fill(false);
    div().p(px(24.)).w(px(420.)).child(chart).into_any_element()
}

fn sparklines(_: &mut Window, cx: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    use desktop_gpui::ui::ActivePalette as _;
    let p = cx.palette().clone();
    let wave = |a: f32, b: f32| (0..=20).map(|i| (a + (b - a) * i as f32 / 20., 0.5 + 0.4 * (i as f32 * 0.7).sin())).collect::<Vec<_>>();
    gpui_kit::component::v_flex()
        .p(px(24.))
        .gap(px(16.))
        .w(px(420.))
        .child(Sparkline::new(wave(0., 1.), p.hue_blue).height(px(40.)))
        .child(Sparkline::new(wave(0., 1.), p.hue_violet).no_rules().fill(0.4).height(px(18.)))
        .child(Sparkline::new(wave(0., 1.), p.hue_teal).bare().height(px(16.)))
        .into_any_element()
}

type SceneFn = fn(&mut Window, &mut gpui_kit::App) -> gpui_kit::AnyElement;

const SCENES: &[(&str, SceneFn)] =
    &[("chart", chart), ("sparklines", sparklines), ("chart-flat", chart_flat), ("sparkline-flat", sparkline_flat)];

const LEAD: u32 = 0x2563eb;
const REST: u32 = 0xdc2626;
/// The flat chart's plot: its top, height and the window x a column is read at, in points.
const PLOT_TOP: f32 = 24.;
const PLOT_H: f32 = 160.;
const COLUMN: f32 = 300.;

/// Two flat lines on a fixed axis, the lead at 60 and the rest at 20, so a column across each reads its stroke's width.
fn chart_flat(_: &mut Window, _: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    let line = |key: &'static str, v: f64, ink: u32| ChartLine {
        key: key.into(),
        label: key.into(),
        points: vec![v; 7],
        ink: gpui_kit::rgb(ink).into(),
    };
    let chart = Chart::new("flat", (0..7).map(f64::from).collect(), vec![line("lead", 60., LEAD), line("rest", 20., REST)], Vec::new())
        .axis(Axis { from: 0., to: 100., parts: 4 })
        .height(px(PLOT_H))
        .fill(false);
    div().pt(px(PLOT_TOP)).pl(px(24.)).w(px(444.)).child(chart).into_any_element()
}

const SPARK_H: f32 = 96.;

/// One flat run at 0.6 on the rules, no fill, 96 px tall from the window's top.
fn sparkline_flat(_: &mut Window, _: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    let points = (0..=10).map(|i| (i as f32 / 10., 0.6)).collect();
    div().pl(px(24.)).w(px(444.)).child(Sparkline::new(points, gpui_kit::rgb(LEAD).into()).fill(0.).height(px(SPARK_H))).into_any_element()
}

/// How much of one pixel's width of `ink` a run of pixels holds over `ground`, read on the channel where they differ most.
fn ink_in(pixels: impl Iterator<Item = [u8; 3]>, ground: [u8; 3], ink: [u8; 3]) -> f32 {
    let c = (0..3).max_by_key(|&c| ground[c].abs_diff(ink[c])).unwrap();
    let full = ground[c] as f32 - ink[c] as f32;
    pixels.map(|p| ((ground[c] as f32 - p[c] as f32) / full).clamp(0., 1.)).sum()
}

fn rgb_of(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

fn hsla_rgb(color: gpui_kit::Hsla) -> [u8; 3] {
    let c = gpui_kit::Rgba::from(color);
    [(c.r * 255.).round() as u8, (c.g * 255.).round() as u8, (c.b * 255.).round() as u8]
}

/// The ink down one column between two heights in points, in device pixels of `ink`'s full width.
fn column(image: &RgbaImage, x: f32, from: f32, to: f32, ground: [u8; 3], ink: [u8; 3]) -> f32 {
    let scale = image.width() as f32 / W;
    let (x, from, to) = ((x * scale) as u32, (from * scale) as u32, (to * scale) as u32);
    ink_in(
        (from..to).map(|y| {
            let p = image.get_pixel(x, y).0;
            [p[0], p[1], p[2]]
        }),
        ground,
        ink,
    )
}

/// The lengths of the inked and bare runs along one device row from `x` points on, the first `n` of them.
fn runs(image: &RgbaImage, x: f32, row: u32, ground: [u8; 3], n: usize) -> Vec<u32> {
    let scale = image.width() as f32 / W;
    let mut out = Vec::new();
    let (mut at, mut inked, mut len) = ((x * scale) as u32, None, 0);
    while out.len() < n && at < image.width() {
        let p = image.get_pixel(at, row).0;
        let here = [p[0], p[1], p[2]] != ground;
        if inked.is_some_and(|i| i != here) {
            out.push(len);
            len = 0;
        }
        inked = Some(here);
        len += 1;
        at += 1;
    }
    out
}

fn near(what: &str, got: f32, want: f32, within: f32) {
    assert!((got - want).abs() <= within, "{what}: {got}, want {want} within {within}");
}

/// The chart's strokes at the web's widths in points, scaled to the device as Electron draws them at scale 2 (the
/// W1-USE-017 capture: the lead about 3.5 device px, gridlines two full rows of the border's ink dashed 4 on 8): the
/// lead 1.75 wide, the rest 1.4, the floor 1 and the gridlines 1 dashed 2 on 4, and the hover line 1.
fn the_chart_strokes_at_the_webs_widths() {
    let mut cx = app("paper", true);
    let handle = open(&mut cx, chart_flat);
    let (ground, border, fg) = cx.update(|cx| {
        use desktop_gpui::ui::ActivePalette as _;
        let p = cx.palette();
        (hsla_rgb(p.background), hsla_rgb(p.border), hsla_rgb(p.foreground))
    });
    let image = draw(&mut cx, handle);
    let scale = image.width() as f32 / W;
    let level = |v: f32| PLOT_TOP + PLOT_H - 6. - v / 100. * (PLOT_H - 12.);
    near("the lead's width", column(&image, COLUMN, level(60.) - 4., level(60.) + 4., ground, rgb_of(LEAD)), 1.75 * scale, 0.3);
    near("the rest's width", column(&image, COLUMN, level(20.) - 4., level(20.) + 4., ground, rgb_of(REST)), 1.4 * scale, 0.3);
    near("the floor's width", column(&image, COLUMN, level(0.) - 3., level(0.) + 3., ground, border), scale, 0.15);
    let row = (level(25.) * scale) as u32;
    // The first run may start inside a dash, so it is not read.
    let dashes = &runs(&image, COLUMN, row, ground, 7)[1..];
    let (on, off) = ((2. * scale) as u32, (4. * scale) as u32);
    assert!(
        dashes.iter().all(|&d| d == on || d == off) && dashes.contains(&off),
        "a gridline's dashes 2 on 4 points, {on} on {off} device pixels: {dashes:?}"
    );
    let dash = COLUMN + (dashes_start(&image, COLUMN, row, ground) as f32 + 0.5) / scale;
    // A 1 point line on a whole point fills its device rows with the border's ink, as Electron's #E4E4E4 rows do.
    let darkest = ((level(25.) - 3.) * scale) as u32..((level(25.) + 3.) * scale) as u32;
    let darkest = darkest.map(|y| image.get_pixel((dash * scale) as u32, y).0[0]).min().unwrap();
    assert!(darkest.abs_diff(border[0]) <= 2, "a gridline's rows in the border's full ink: {darkest} against {}", border[0]);
    near("a gridline's width", column(&image, dash, level(25.) - 3., level(25.) + 3., ground, border), scale, 0.15);

    let hover = gpui_kit::point(px(COLUMN), px(level(80.)));
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::InputEvent as _;
        let event = gpui_kit::MouseMoveEvent { position: hover, pressed_button: None, modifiers: gpui_kit::Modifiers::default() };
        window.dispatch_event(event.to_platform_input(), cx);
    })
    .unwrap();
    let image = draw(&mut cx, handle);
    // The hover line is the foreground at 30% over the ground, read across one row between the lines and the tooltip.
    let line = ground.iter().zip(fg).map(|(g, f)| (*g as f32 * 0.7 + f as f32 * 0.3).round() as u8).collect::<Vec<_>>();
    let y = (level(45.) * scale) as u32;
    let across = ink_in(
        (0..image.width()).map(|x| {
            let p = image.get_pixel(x, y).0;
            [p[0], p[1], p[2]]
        }),
        ground,
        [line[0], line[1], line[2]],
    );
    near("the hover line's width", across, scale, 0.15);
}

/// How many device pixels from `x` points along `row` the first dash starts.
fn dashes_start(image: &RgbaImage, x: f32, row: u32, ground: [u8; 3]) -> u32 {
    let scale = image.width() as f32 / W;
    let from = (x * scale) as u32;
    (from..image.width())
        .find(|&at| {
            let p = image.get_pixel(at, row).0;
            [p[0], p[1], p[2]] != ground
        })
        .unwrap()
        - from
}

/// A row's right press menu off the Mac, open at (40, 40) on a process row's acts.
struct MenuOwner {
    menu: ui::context_menu::ContextMenu,
    opened: bool,
}

impl Render for MenuOwner {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use desktop_gpui::ui::ActivePalette as _;
        if !self.opened {
            self.opened = true;
            cx.defer_in(window, |this, window, cx| {
                let item = |label: &'static str, group: &'static str, destructive: bool| ui::context_menu::MenuItem {
                    label: label.into(),
                    group,
                    refusal: None,
                    keys: None,
                    destructive,
                };
                let items = vec![
                    item("Open thread", "go", false),
                    item("Copy PID", "copy", false),
                    item("Copy command", "copy", false),
                    item("End", "end", true),
                ];
                this.menu.open(gpui_kit::point(px(40.), px(40.)), items, window, cx, |_, _, _, _| {});
            });
        }
        div().size_full().bg(cx.palette().background).children(self.menu.overlay())
    }
}

fn menu_open(theme: &str) -> (HeadlessAppContext, AnyWindowHandle) {
    let mut cx = app(theme, true);
    let handle: AnyWindowHandle = cx
        .open_window(size(px(W), px(H)), |window, cx| {
            let view = cx.new(|_| MenuOwner { menu: Default::default(), opened: false });
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap()
        .into();
    (cx, handle)
}

fn menu_shot(theme: &str) -> RgbaImage {
    let (mut cx, handle) = menu_open(theme);
    draw(&mut cx, handle)
}

/// The menu's rows from its top at the pointer, (40, 40): its edge and padding, then 28 px rows with a 9 px rule
/// between groups, as `ui::context_menu`'s own test holds them.
const MENU_ROWS: [f32; 4] = [45., 82., 110., 147.];

/// `ROW_CLASS`: the lit row on the accent ground, from the first row as the menu opens, and End's words in the
/// destructive ink only while End is lit, neutral at rest.
fn the_page_menu_lights_its_first_row_and_reds_end_only_when_lit() {
    let (mut cx, handle) = menu_open("paper");
    let (popover, accent) = cx.update(|cx| {
        use desktop_gpui::ui::ActivePalette as _;
        (hsla_rgb(cx.palette().popover), hsla_rgb(cx.palette().accent))
    });
    let at_rest = draw(&mut cx, handle);
    let scale = at_rest.width() as f32 / W;
    let ground = |image: &RgbaImage, row: usize| {
        let p = image.get_pixel(((40. + 150.) * scale) as u32, ((MENU_ROWS[row] + 14.) * scale) as u32).0;
        [p[0], p[1], p[2]]
    };
    let red = |image: &RgbaImage| {
        let (x0, x1, y0, y1) = (
            ((40. + 9.) * scale) as u32,
            ((40. + 60.) * scale) as u32,
            (MENU_ROWS[3] * scale) as u32,
            ((MENU_ROWS[3] + 28.) * scale) as u32,
        );
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let p = image.get_pixel(x, y).0;
                p[0] as i32 - p[1] as i32 > 60
            })
            .count()
    };
    assert_eq!(ground(&at_rest, 0), accent, "the first row is lit as the menu opens");
    assert_eq!(ground(&at_rest, 3), popover, "End is not lit");
    assert_eq!(red(&at_rest), 0, "End's words are neutral at rest");
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::InputEvent as _;
        let event =
            gpui_kit::KeyDownEvent { keystroke: gpui_kit::Keystroke::parse("end").unwrap(), is_held: false, prefer_character_input: false };
        window.dispatch_event(event.to_platform_input(), cx);
    })
    .unwrap();
    let lit = draw(&mut cx, handle);
    assert_eq!(ground(&lit, 3), accent, "End lit by the End key");
    assert!(red(&lit) > 20, "End's words take the destructive ink while lit");
}

/// The usage head's refresh at (200, 100), hovered until its tooltip shows.
fn refresh(_: &mut Window, _: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    let button = ui::Button::icon_xs("refresh", ui::Glyph::RefreshCw).tooltip("Read usage and limits again");
    div().size_full().child(div().absolute().left(px(200.)).top(px(100.)).child(button)).into_any_element()
}

fn tooltip_shot(theme: &str) -> RgbaImage {
    let mut cx = app(theme, true);
    let handle = open(&mut cx, refresh);
    draw(&mut cx, handle);
    cx.update_window(handle, |_, window, cx| {
        use gpui_kit::InputEvent as _;
        let event = gpui_kit::MouseMoveEvent {
            position: gpui_kit::point(px(212.), px(112.)),
            pressed_button: None,
            modifiers: gpui_kit::Modifiers::default(),
        };
        window.dispatch_event(event.to_platform_input(), cx);
    })
    .unwrap();
    for _ in 0..3 {
        cx.advance_clock(std::time::Duration::from_millis(400));
        draw(&mut cx, handle);
    }
    draw(&mut cx, handle)
}

const BAR: (f32, f32, f32, f32) = (24., 24., 320., 40.);

fn skeleton_bar(_: &mut Window, _: &mut gpui_kit::App) -> gpui_kit::AnyElement {
    let (x, y, w, h) = BAR;
    div().pl(px(x)).pt(px(y)).child(ui::Skeleton::new(px(w), px(h))).into_any_element()
}

/// The skeleton's sweep leans as `linear-gradient(120deg, ...)` does: down the bar, the band's brightest point moves
/// left by the rows' drop times cot 60 degrees.
fn the_skeleton_band_leans_at_120_degrees() {
    let mut cx = app("paper", false);
    let handle = open(&mut cx, skeleton_bar);
    let border = cx.update(|cx| {
        use desktop_gpui::ui::ActivePalette as _;
        hsla_rgb(cx.palette().border)
    });
    let (x, y, w, h) = BAR;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let image = draw(&mut cx, handle);
        let scale = image.width() as f32 / W;
        // The brightest device pixel along the row `dy` points into the bar, while the band stands well inside it.
        let peak = |dy: f32| {
            let row = ((y + dy) * scale) as u32;
            let (at, lum) = (((x + 8.) * scale) as u32..((x + w - 8.) * scale) as u32)
                .map(|px| (px, image.get_pixel(px, row).0[0]))
                .max_by_key(|(_, lum)| *lum)
                .unwrap();
            let at = at as f32 / scale - x;
            (lum > border[0] + 8 && at > 40. && at < w - 40.).then_some(at)
        };
        if let (Some(top), Some(bottom)) = (peak(1.), peak(h - 1.)) {
            let lean = (60f32).to_radians().tan().recip() * (h - 2.);
            near("the band's lean from the top row to the bottom", top - bottom, lean, 1.5);
            return;
        }
        assert!(std::time::Instant::now() < until, "the band never stood inside the bar");
    }
}

/// The live band's plot strokes at the web's widths too, scaled to the device: its line 1.5 wide, its rules 1 dashed 2
/// on 3, its floor 1.
fn the_sparkline_strokes_at_the_webs_widths() {
    let mut cx = app("paper", true);
    let handle = open(&mut cx, sparkline_flat);
    let (ground, border) = cx.update(|cx| {
        use desktop_gpui::ui::ActivePalette as _;
        (hsla_rgb(cx.palette().background), hsla_rgb(cx.palette().border))
    });
    let image = draw(&mut cx, handle);
    let scale = image.width() as f32 / W;
    let pad = SPARK_H / 96.;
    let y_at = |v: f32| pad + (SPARK_H - pad * 2.) * (1. - v);
    near("the line's width", column(&image, COLUMN, y_at(0.6) - 4., y_at(0.6) + 4., ground, rgb_of(LEAD)), 1.5 * scale, 0.3);
    near("the floor's width", column(&image, COLUMN, SPARK_H - pad - 3., SPARK_H - pad + 3., ground, border), scale, 0.15);
    let dashes = &runs(&image, COLUMN, (y_at(0.25) * scale) as u32, ground, 7)[1..];
    let (on, off) = ((2. * scale) as u32, (3. * scale) as u32);
    assert!(
        dashes.iter().all(|&d| d == on || d == off) && dashes.contains(&off),
        "a rule's dashes 2 on 3 points, {on} on {off} device pixels: {dashes:?}"
    );
}

fn main() {
    if let Some(out) = std::env::var_os("SHARED_PNG").map(PathBuf::from) {
        std::fs::create_dir_all(&out).unwrap();
        for theme in ["paper", "graphite"] {
            for (name, scene) in SCENES {
                let mut cx = app(theme, true);
                let handle = open(&mut cx, *scene);
                draw(&mut cx, handle).save(out.join(format!("{name}-{theme}.png"))).unwrap();
            }
            if !cfg!(target_os = "macos") {
                menu_shot(theme).save(out.join(format!("menu-{theme}.png"))).unwrap();
            }
            tooltip_shot(theme).save(out.join(format!("tooltip-{theme}.png"))).unwrap();
        }
    }
    // `cargo test --test shared_pieces -- <words>` runs the tests whose names hold the words.
    let only: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let tests: [(&str, fn()); 4] = [
        ("the_chart_strokes_at_the_webs_widths", the_chart_strokes_at_the_webs_widths),
        ("the_sparkline_strokes_at_the_webs_widths", the_sparkline_strokes_at_the_webs_widths),
        ("the_skeleton_band_leans_at_120_degrees", the_skeleton_band_leans_at_120_degrees),
        ("the_page_menu_lights_its_first_row_and_reds_end_only_when_lit", the_page_menu_lights_its_first_row_and_reds_end_only_when_lit),
    ];
    for (name, test) in tests {
        // The Mac's right press menu is AppKit's, which the test platform cannot draw.
        if (cfg!(target_os = "macos") && name.contains("page_menu")) || !(only.is_empty() || only.iter().any(|o| name.contains(o.as_str())))
        {
            continue;
        }
        test();
        println!("test {name} ... ok");
    }
}
