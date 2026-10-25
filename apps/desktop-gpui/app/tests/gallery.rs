// SPDX-License-Identifier: AGPL-3.0-only
//! The gallery and its pieces drawn by GPUI's real offscreen renderer (WGPU on Linux, Metal on macOS), read back as
//! pixels: the theme switch restyles every section, and the corner radii and drawn edges hold to the web's sizes.
//! Without libtest's harness, as `pixels.rs` explains. `GALLERY_PNG=<folder>` writes each section's capture there in
//! every theme (or those `GALLERY_THEMES` lists), and each piece alone under `pieces/`.

mod web;

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::gallery::{Gallery, Only, Piece, Section, CONTENT, PAD};
use desktop_gpui::ui::{
    self, chips, glyph_frame, ActivePalette, AlertDialog, Button, Card, CardItem, Chip, CopyRow, Dialog, Field, Glyph, Line, Segmented,
    Select, Side, Skeleton, Sparkline, Still, Switch, UiAssets,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, prelude::*, px, size, AnyElement, AnyWindowHandle, AppContext, Context, Entity, HeadlessAppContext, InputEvent as _, Pixels,
    Window,
};
use image::RgbaImage;

const SEGMENTED: &str = include_str!("../../../web/src/components/ui/segmented-control.tsx");
const SWITCH: &str = include_str!("../../../web/src/components/ui/switch.tsx");
const CHIPS: &str = include_str!("../../../web/src/components/ui/chips.tsx");
const SKELETON: &str = include_str!("../../../web/src/components/ui/skeleton.tsx");
const SELECT: &str = include_str!("../../../web/src/components/ui/select.tsx");
const ALERT: &str = include_str!("../../../web/src/components/ui/alert-dialog.tsx");

/// Tall enough for the longest section; a capture is cut to the section's own box.
const HEIGHT: f32 = 2400.;

fn app(theme: &str) -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    cx
}

fn draw(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> f32 {
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap()
}

struct Shot {
    cx: HeadlessAppContext,
    handle: AnyWindowHandle,
    gallery: Entity<Gallery>,
}

impl Shot {
    fn open(theme: &str, only: Only) -> Shot {
        let mut cx = app(theme);
        let mut gallery = None;
        let handle = cx
            .open_window(size(CONTENT + PAD * 2., px(HEIGHT)), |window, cx| {
                let view = cx.new(|cx| Gallery::new(Some(only), window, cx));
                gallery = Some(view.clone());
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap()
            .into();
        Shot { cx, handle, gallery: gallery.unwrap() }
    }

    fn capture(&mut self) -> RgbaImage {
        let scale = draw(&mut self.cx, self.handle);
        let drawn = self.cx.update(|cx| self.gallery.read(cx).drawn()).expect("the section drew");
        let image = self.cx.capture_screenshot(self.handle).expect("offscreen rendering must be available");
        let device = |v: Pixels| (f32::from(v) * scale).round() as u32;
        let (x, y) = (device(drawn.origin.x), device(drawn.origin.y));
        let (w, h) = (device(drawn.size.width).min(image.width() - x), device(drawn.size.height).min(image.height() - y));
        image::imageops::crop_imm(&image, x, y, w, h).to_image()
    }

    /// Picks a theme in the gallery's picker, as a click on its item confirms it.
    fn pick(&mut self, word: &'static str) {
        let picker = self.cx.update(|cx| self.gallery.read(cx).picker().clone());
        self.cx
            .update_window(self.handle, |_, _, cx| {
                picker.update(cx, |s, cx| s.confirm(ui::theme::THEMES.iter().position(|t| t.word == word).unwrap(), cx))
            })
            .unwrap();
    }
}

fn diff(a: &[u8], b: &[u8]) -> u32 {
    a.iter().zip(b).take(3).map(|(a, b)| a.abs_diff(*b) as u32).sum()
}

fn main() {
    let out = std::env::var_os("GALLERY_PNG").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out.join("pieces")).unwrap();
        // `GALLERY_THEMES=paper,graphite` keeps a capture to those themes.
        let only = std::env::var("GALLERY_THEMES").ok();
        let wanted = |id: &str| only.as_deref().is_none_or(|o| o.split(',').any(|t| t == id));
        for theme in ui::theme::THEMES.iter().map(|t| t.id).filter(|id| wanted(id)) {
            for section in Section::ALL {
                Shot::open(theme, Only::Section(section)).capture().save(out.join(format!("{}-{theme}.png", section.name()))).unwrap();
            }
            for &piece in Piece::ALL {
                Shot::open(theme, Only::Piece(piece)).capture().save(out.join(format!("pieces/{}-{theme}.png", piece.name()))).unwrap();
            }
        }
    }
    the_faces_are_the_web_stacks_first_installed();
    println!("test the_faces_are_the_web_stacks_first_installed ... ok");
    switching_the_theme_restyles_every_piece();
    println!("test switching_the_theme_restyles_every_piece ... ok");
    the_pieces_draw_the_web_radii_and_control_sizes();
    println!("test the_pieces_draw_the_web_radii_and_control_sizes ... ok");
    the_sparkline_draws_the_points_it_is_given();
    println!("test the_sparkline_draws_the_points_it_is_given ... ok");
    a_held_field_draws_its_text_at_the_webs_64_percent();
    println!("test a_held_field_draws_its_text_at_the_webs_64_percent ... ok");
    the_row_menu_opens_on_the_twelve_px_surface();
    println!("test the_row_menu_opens_on_the_twelve_px_surface ... ok");
}

/// A row's menu, opened by a press on its trigger, draws `MenuPopup`'s `rounded-[12px]`, read off the popup's own corner
/// under the trigger, so a menu that stops passing `ui::menu::surface` draws the theme's popover radius and fails.
fn the_row_menu_opens_on_the_twelve_px_surface() {
    let mut cx = app("graphite");
    let run: ui::act::Run = std::rc::Rc::new(|_, _| {});
    let acts = std::rc::Rc::new(vec![ui::menu::MenuAct {
        group: 0,
        glyph: None,
        title: "Copy".into(),
        destructive: false,
        held: false,
        why: None,
        keys: None,
        run,
    }]);
    let handle: AnyWindowHandle = cx
        .open_window(size(px(688.), px(400.)), |window, cx| {
            let piece = move |_: &mut Window, _: &mut Context<Lone>| ui::menu::row_menu("row", acts.clone()).into_any_element();
            cx.new(|cx| Root::new(cx.new(|_| Lone(Box::new(piece))), window, cx))
        })
        .unwrap()
        .into();
    draw(&mut cx, handle);
    cx.update_window(handle, |_, window, cx| {
        // The trigger's centre: its 20 px square at the padding.
        let position = gpui_kit::point(PAD + px(10.), PAD + px(10.));
        let (modifiers, button) = (gpui_kit::Modifiers::default(), gpui_kit::MouseButton::Left);
        window.dispatch_event(gpui_kit::MouseMoveEvent { position, pressed_button: None, modifiers }.to_platform_input(), cx);
        window.dispatch_event(
            gpui_kit::MouseDownEvent { position, modifiers, button, click_count: 1, first_mouse: false }.to_platform_input(),
            cx,
        );
        window.dispatch_event(gpui_kit::MouseUpEvent { position, modifiers, button, click_count: 1 }.to_platform_input(), cx);
    })
    .unwrap();
    let scale = draw(&mut cx, handle);
    let image = cx.capture_screenshot(handle).unwrap();
    near("the row menu's radius", measure_below(&image, scale, PAD + px(24.)).radius, 12.);
}

/// The faces the pieces draw with are the first of `--font-sans-system` and `--font-mono-system` this machine has,
/// as Chromium picks them for the web app; the test prints them (Liberation Sans and DejaVu Sans Mono on Linux).
fn the_faces_are_the_web_stacks_first_installed() {
    let mut cx = app("paper");
    let (sans, mono, installed) =
        cx.update(|cx| (cx.theme().font_family.clone(), cx.theme().mono_font_family.clone(), cx.text_system().all_font_names()));
    let want = |stack: &[&str]| ui::theme::first_installed(stack, &installed, cfg!(target_os = "macos"));
    assert_eq!(Some(sans.clone()), want(&ui::theme::SANS));
    assert_eq!(Some(mono.clone()), want(&ui::theme::MONO));
    assert!(sans.starts_with('.') || installed.iter().any(|f| f == sans.as_ref()), "{sans} is not installed");
    println!("    sans {sans}, mono {mono}");
}

/// Each section drawn in Paper, then switched to Graphite through the gallery's picker, must match the same section
/// opened in Graphite, and every 16 px tile that holds a piece in Paper must have changed.
fn switching_the_theme_restyles_every_piece() {
    for section in Section::ALL {
        let mut shot = Shot::open("paper", Only::Section(section));
        let before = shot.capture();
        shot.pick("Graphite");
        let after = shot.capture();
        let fresh = Shot::open("graphite", Only::Section(section)).capture();
        assert_eq!(after.dimensions(), fresh.dimensions(), "{}: the switch moved the layout", section.name());
        let stale = after.pixels().zip(fresh.pixels()).filter(|(a, b)| diff(&a.0, &b.0) > 6).count();
        assert_eq!(stale, 0, "{}: {stale} pixels differ from a gallery opened in Graphite", section.name());

        let ground = before.get_pixel(1, 1).0;
        let (w, h) = (before.width().min(after.width()), before.height().min(after.height()));
        let mut unchanged = Vec::new();
        for ty in (0..h / 16).map(|t| t * 16) {
            for tx in (0..w / 16).map(|t| t * 16) {
                let tile = || (ty..ty + 16).flat_map(move |y| (tx..tx + 16).map(move |x| (x, y)));
                if !tile().any(|(x, y)| diff(&before.get_pixel(x, y).0, &ground) > 6) {
                    continue;
                }
                let changed = tile().filter(|&(x, y)| diff(&before.get_pixel(x, y).0, &after.get_pixel(x, y).0) > 6).count();
                // A switch's thumb is white on both sides, as `--primary-foreground` is.
                let white = |p: &image::Rgba<u8>| p.0[..3].iter().all(|&c| c >= 250);
                let kept: Vec<_> = tile()
                    .filter(|&(x, y)| diff(&before.get_pixel(x, y).0, &after.get_pixel(x, y).0) <= 6)
                    .filter(|&(x, y)| diff(&before.get_pixel(x, y).0, &ground) > 6)
                    .collect();
                // Only the Controls section draws a switch.
                let only_white = section == Section::Controls
                    && !kept.is_empty()
                    && kept.iter().filter(|&&(x, y)| white(after.get_pixel(x, y))).count() * 10 >= kept.len() * 9;
                if changed < 64 && !only_white {
                    unchanged.push((tx, ty, changed));
                }
            }
        }
        assert!(unchanged.is_empty(), "{}: tiles that kept their Paper look (x, y, pixels changed of 256): {unchanged:?}", section.name());
    }
}

type Draw = Box<dyn Fn(&mut Window, &mut Context<Lone>) -> AnyElement>;

struct Lone(Draw);

impl Render for Lone {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background).p(PAD).child((self.0)(window, cx))
    }
}

/// `piece` alone at the window's 24 px padding, in Graphite, where every piece's edge and fill are lighter than the
/// page and every shadow darker, so the piece's own shape is the pixels lighter than the page.
fn shoot(piece: impl Fn(&mut Window, &mut Context<Lone>) -> AnyElement + 'static) -> (RgbaImage, f32) {
    shoot_in("graphite", piece)
}

fn shoot_in(theme: &str, piece: impl Fn(&mut Window, &mut Context<Lone>) -> AnyElement + 'static) -> (RgbaImage, f32) {
    let mut cx = app(theme);
    let handle = cx
        .open_window(size(px(688.), px(400.)), |window, cx| cx.new(|cx| Root::new(cx.new(|_| Lone(Box::new(piece))), window, cx)))
        .unwrap();
    let scale = draw(&mut cx, handle.into());
    (cx.capture_screenshot(handle.into()).unwrap(), scale)
}

fn lum(p: &image::Rgba<u8>) -> f32 {
    (p.0[0] as f32 + p.0[1] as f32 + p.0[2] as f32) / 3.
}

/// Where the piece's edge crosses half coverage along a run of pixels, in device px from the run's start.
fn edge(run: &[f32], ground: f32) -> f32 {
    let first = run.iter().position(|&v| v > ground + 1.).expect("the run meets the piece");
    let peak = run[first..(first + 4).min(run.len())].iter().copied().fold(ground, f32::max);
    let cover = |v: f32| ((v - ground) / (peak - ground)).clamp(0., 1.);
    let at = run.iter().position(|&v| cover(v) >= 0.5).unwrap();
    if at == 0 {
        return 0.;
    }
    let (a, b) = (cover(run[at - 1]), cover(run[at]));
    at as f32 - 1. + (0.5 - a) / (b - a) + 0.5
}

struct Measured {
    width: f32,
    height: f32,
    radius: f32,
}

/// The piece's drawn box and its top-left corner radius, in logical px.
fn measure(image: &RgbaImage, scale: f32) -> Measured {
    measure_below(image, scale, PAD)
}

/// `measure` of the first piece whose top is at or below `y`.
fn measure_below(image: &RgbaImage, scale: f32, y: Pixels) -> Measured {
    let ground = lum(image.get_pixel(2, 2));
    let start = (f32::from(PAD) * scale) as u32 - 6;
    let start_y = (f32::from(y) * scale) as u32 - 6;
    // A line through the piece's middle needs its middle, found from its first lit row and column.
    let lit = |x: u32, y: u32| lum(image.get_pixel(x, y)) > ground + 1.;
    let top = (start_y..image.height()).find(|&y| (start..image.width()).any(|x| lit(x, y))).unwrap();
    let left = (start..image.width()).find(|&x| (top..image.height()).any(|y| lit(x, y))).unwrap();
    let bottom = (top..image.height()).take_while(|&y| (left..image.width()).any(|x| lit(x, y))).last().unwrap();
    let right = (left..image.width()).take_while(|&x| (top..=bottom).any(|y| lit(x, y))).last().unwrap();
    let (mx, my) = ((left + right) / 2, (top + bottom) / 2);
    let row = |xs: &mut dyn Iterator<Item = u32>| xs.map(|x| lum(image.get_pixel(x, my))).collect::<Vec<_>>();
    let col = |ys: &mut dyn Iterator<Item = u32>| ys.map(|y| lum(image.get_pixel(mx, y))).collect::<Vec<_>>();
    let l = start as f32 + edge(&row(&mut (start..mx)), ground);
    let r = (right + 6) as f32 - edge(&row(&mut (mx..right + 6).rev()), ground);
    let t = start_y as f32 + edge(&col(&mut (start_y..my)), ground);
    let b = (bottom + 6) as f32 - edge(&col(&mut (my..bottom + 6).rev()), ground);
    // The arc meets the diagonal at r(1 - 1/sqrt 2) in from each edge.
    let (x0, y0) = (l.floor() as u32, t.floor() as u32);
    let diagonal: Vec<f32> = (0..((right - left).min(bottom - top) / 2)).map(|k| lum(image.get_pixel(x0 + k, y0 + k))).collect();
    let e = edge(&diagonal, ground);
    let along = ((x0 as f32 + e - l) + (y0 as f32 + e - t)) / 2.;
    Measured { width: (r - l) / scale, height: (b - t) / scale, radius: along / (1. - std::f32::consts::FRAC_1_SQRT_2) / scale }
}

#[track_caller]
fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= 1., "{what}: drawn at {got:.2} px, the web writes {want} px");
    println!("    {what}: drawn at {got:.2} px, the web writes {want} px");
}

fn select_state(cx: &mut gpui_kit::App) -> Entity<ui::SelectState<&'static str>> {
    cx.new(|_| ui::SelectState::new(vec![ui::SelectItem::new("same", "The same window")], Some(&"same")))
}

fn the_pieces_draw_the_web_radii_and_control_sizes() {
    let select_classes = web::constant(web::LAYOUT, "SELECT_WIDTH");
    let field_classes = web::constant(web::LAYOUT, "ROW_FIELD");
    let (image, scale) = shoot(|_, cx| Select::new(&select_state(cx)).into_any_element());
    let m = measure(&image, scale);
    near("the select's width", m.width, web::px(select_classes, "w"));
    near("the select's height", m.height, web::px(select_classes, "h"));
    near("the select's radius", m.radius, web::px(select_classes, "rounded"));

    let (image, scale) = shoot(|window, cx| {
        let state = cx.new(|cx| InputState::new(window, cx).default_value("~/code"));
        Field::new(&state).into_any_element()
    });
    let m = measure(&image, scale);
    near("the field's width", m.width, web::px(web::literal(web::PICK_LISTS, "w-44 max-sm:w-36"), "w"));
    near("the field's height", m.height, web::px(field_classes, "h"));
    near("the field's radius", m.radius, web::px(field_classes, "rounded"));

    let (image, scale) = shoot(|_, _| Card::new("card").item(Line::new("l", "Disk").value("9.4 GB")).into_any_element());
    near("the card's radius", measure(&image, scale).radius, web::px(web::constant(web::ROWS, "CARD_SURFACE"), "rounded"));

    // A first child that paints a fill keeps inside the card's corner, as the web's overflow-hidden card clips it.
    let (image, scale) = shoot(|_, cx| {
        let fill = cx.palette().foreground;
        Card::new("card")
            .item(CardItem::placed(move |corners| corners.apply(div().h(px(40.)).bg(fill)).into_any_element()))
            .into_any_element()
    });
    let corner = image.get_pixel(((f32::from(PAD) + 2.5) * scale) as u32, ((f32::from(PAD) + 2.5) * scale) as u32);
    assert!(lum(corner) < 128., "a filled first child pokes out of the card's rounded corner: {corner:?}");
    println!("    a filled first child stays inside the card's corner: {corner:?} at 2.5 px in");

    let (image, scale) = shoot(|_, cx| glyph_frame(Glyph::Monitor, cx).into_any_element());
    let m = measure(&image, scale);
    near("the glyph frame's side", m.width, web::px(web::constant(web::LAYOUT, "GLYPH_FRAME"), "size"));
    near("the glyph frame's radius", m.radius, web::px(web::constant(web::LAYOUT, "GLYPH_FRAME"), "rounded"));

    let control_radius = web::INDEX_CSS.split("--control-radius:").nth(1).unwrap().split("rem").next().unwrap().trim();
    let (image, scale) = shoot(|_, _| Button::outline("b", "Back").into_any_element());
    near("the button's radius (--control-radius)", measure(&image, scale).radius, control_radius.parse::<f32>().unwrap() * 16.);

    // `KEYCAP_BEVEL` lights the row just inside the top border, where CSS keeps an inset shadow.
    let (image, scale) = shoot(|_, _| Button::primary("b", "Next").into_any_element());
    let (x, y) = (((f32::from(PAD) + 26.) * scale) as u32, (f32::from(PAD) * scale) as u32);
    let (bevel, fill) = (lum(image.get_pixel(x, y + (1.5 * scale) as u32)), lum(image.get_pixel(x, y + (4. * scale) as u32)));
    assert!(bevel > fill + 3., "the keycap's top light is not drawn inside its border: {bevel:.1} against the fill's {fill:.1}");
    println!("    the keycap's top light inside the border: {bevel:.1} against the fill's {fill:.1}");

    let copy = web::literal(web::SHEET_PARTS, "flex h-10 w-full items-center gap-3 rounded-");
    let (image, scale) =
        shoot(|_, _| div().w(px(480.)).child(CopyRow::new("c", "curl -fsSL https://wsp.sh/install | sh")).into_any_element());
    let m = measure(&image, scale);
    near("the copy row's height", m.height, web::px(copy, "h"));
    near("the copy row's radius", m.radius, web::px(copy, "rounded"));

    let segmented = web::literal(SEGMENTED, "inline-flex h-8");
    let (image, scale) = shoot(|_, _| Segmented::new("s", 0).segment("Enter").segment("Ctrl Enter").into_any_element());
    let m = measure(&image, scale);
    near("the segmented control's height", m.height, web::px(segmented, "h"));
    near("the segmented control's radius", m.radius, web::px(segmented, "rounded"));

    // `sm:[--thumb-size:--spacing(4)]`: the track is the thumb and 2 px high, twice it less 2 px wide, fully round.
    let thumb = SWITCH.split("sm:[--thumb-size:--spacing(").nth(1).unwrap().split(')').next().unwrap().parse::<f32>().unwrap() * 4.;
    let (image, scale) = shoot(|_, _| Switch::new("s", true).into_any_element());
    let m = measure(&image, scale);
    near("the switch's width", m.width, thumb * 2. - 2.);
    near("the switch's height", m.height, thumb + 2.);
    near("the switch's radius", m.radius, (thumb + 2.) / 2.);

    let chip = web::constant(CHIPS, "CHIP");
    let (image, scale) = shoot(|_, cx| chips("c", vec![Chip::new("8 cores").glyph(Glyph::Cpu)], cx).into_any_element());
    let m = measure(&image, scale);
    near("a chip's height", m.height, web::px(chip, "h"));
    near("a chip's radius", m.radius, web::px(chip, "rounded"));

    let (image, scale) = shoot(|_, _| Skeleton::new(px(160.), px(14.)).into_any_element());
    near(
        "a skeleton bar's radius",
        measure(&image, scale).radius,
        web::px(web::literal(SKELETON, "relative overflow-hidden rounded-"), "rounded"),
    );

    let list = web::literal(SELECT, "dropdown-glass dark:popover-shadow");
    let (image, scale) = shoot(|_, cx| {
        let state = cx.new(|cx| {
            let mut s =
                ui::SelectState::new(vec![ui::SelectItem::new(0, "The same window"), ui::SelectItem::new(1, "A new window")], Some(&0));
            s.set_open(true, cx);
            s
        });
        div().pt(px(8.)).pl(px(8.)).child(Select::new(&state)).into_any_element()
    });
    near("the select list's radius", measure(&image, scale).radius, web::px(list, "rounded"));

    let alert = web::literal(ALERT, "row-start-2 max-h-full");
    let (image, scale) = shoot(|_, _| AlertDialog::new("Remove spoo-landing?").act(Button::destructive("r", "Remove")).into_any_element());
    let m = measure(&image, scale);
    near("the alert dialog's width", m.width, web::px(alert, "max-w"));
    near("the alert dialog's radius", m.radius, web::px(web::constant(web::DIALOG_STYLES, "DIALOG_POPUP_CLASS"), "rounded"));

    let popup = web::constant(web::DIALOG_STYLES, "DIALOG_POPUP_CLASS");
    let (image, scale) = shoot(|_, _| Dialog::new("Add a computer").count(2, 5).into_any_element());
    let m = measure(&image, scale);
    near("the dialog's width", m.width, web::px(web::constant(web::STEP_DIALOG, "STEP_WIDTH"), "max-w"));
    near("the dialog's radius", m.radius, web::px(popup, "rounded"));
}

/// A peak drawn as given: a run up from the bottom left to the top middle and down to the bottom right, 400 by 100 px
/// with no fill, lights the line at its points and halfway along each leg, and nothing either side of it.
fn the_sparkline_draws_the_points_it_is_given() {
    let (image, scale) = shoot(|_, cx| {
        let ink = cx.palette().hue_blue;
        div()
            .w(px(400.))
            .child(Sparkline::new(vec![(0., 0.), (0.5, 1.), (1., 0.)], ink).no_rules().fill(0.).height(px(100.)))
            .into_any_element()
    });
    let ground = *image.get_pixel(2, 2);
    let at = |x: f32, y: f32| *image.get_pixel(((f32::from(PAD) + x) * scale) as u32, ((f32::from(PAD) + y) * scale) as u32);
    // `CHART_PAD`: the run's top and bottom stand 1/96 of the height inside the box.
    let y = |v: f32| 100. / 96. + (100. - 200. / 96.) * (1. - v);
    // Each point is read where the run passes: v is 1 less x's distance from 200.
    for (x, v) in [(2., 0.01), (100., 0.5), (199., 0.995), (300., 0.5), (398., 0.01)] {
        let p = at(x, y(v));
        assert!(diff(&p.0, &ground.0) > 60, "the line is not drawn at ({x}, {:.1}): {p:?} against the page's {ground:?}", y(v));
    }
    for (x, yy) in [(100., 20.), (200., 60.), (300., 20.)] {
        let p = at(x, yy);
        assert!(diff(&p.0, &ground.0) <= 6, "something is drawn off the line at ({x}, {yy}): {p:?}");
    }
    println!("    the run lights its points and its legs, and nothing beside them");
}

/// A held field's value on Paper takes the web's `opacity-64` over the page, not a disabled gpui-kit input's half: its
/// strongest stroke stands 64% of the way from the page to the same field's stroke unheld (on Linux, 116 against 40 on a
/// page of 252). Within 3 levels, a stroke's own rounding; half would land 25 levels lighter, a third 70.
fn a_held_field_draws_its_text_at_the_webs_64_percent() {
    let strongest = |held: bool| {
        let (image, scale) = shoot_in("paper", move |window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx).default_value("~/code"));
            Field::new(&state).held(held).into_any_element()
        });
        let (x0, y0) = (((f32::from(PAD) + 4.) * scale) as u32, ((f32::from(PAD) + 4.) * scale) as u32);
        let stroke = (x0..x0 + (160. * scale) as u32).flat_map(|x| (y0..y0 + (24. * scale) as u32).map(move |y| (x, y)));
        (lum(image.get_pixel(2, 2)), stroke.map(|(x, y)| lum(image.get_pixel(x, y))).fold(255., f32::min))
    };
    let ((page, full), (_, held)) = (strongest(false), strongest(true));
    let want = page + 0.64 * (full - page);
    assert!(
        (held - want).abs() <= 3.,
        "a held field's strongest stroke is {held:.1}, the web's 64% is {want:.1} (unheld {full:.1}, page {page:.1})"
    );
    println!("    a held field's strongest stroke: {held:.1}, the web's 64% is {want:.1} (unheld {full:.1}, page {page:.1})");
}
