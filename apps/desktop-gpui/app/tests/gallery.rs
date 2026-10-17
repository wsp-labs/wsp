// SPDX-License-Identifier: AGPL-3.0-only
//! The gallery and its pieces drawn by GPUI's real offscreen renderer (WGPU on Linux, Metal on macOS), read back as
//! pixels: the theme switch restyles every section, and the corner radii and drawn edges hold to the web's sizes.
//! Without libtest's harness, as `pixels.rs` explains. `GALLERY_PNG=<folder>` writes each section's capture there in
//! every theme, and each piece alone under `pieces/`.

mod web;

use std::path::PathBuf;
use std::sync::Arc;

use desktop_gpui::gallery::{Gallery, Only, Piece, Section, CONTENT, PAD};
use desktop_gpui::ui::{
    self, glyph_frame, ActivePalette, Button, Card, CardItem, CopyRow, Dialog, Field, Glyph, Line, Select, Side, Still, UiAssets,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::{ActiveTheme, IndexPath, Root};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, prelude::*, px, size, AnyElement, AnyWindowHandle, AppContext, Context, Entity, HeadlessAppContext, Pixels, SharedString, Window,
};
use image::RgbaImage;

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
            .update_window(self.handle, |_, _, cx| picker.update(cx, |_, cx| cx.emit(SelectEvent::Confirm(Some(SharedString::from(word))))))
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
        for theme in ui::theme::THEMES.iter().map(|t| t.id) {
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
}

/// The faces the pieces draw with are the first of `--font-sans-system` and `--font-mono-system` this machine has,
/// as Chromium picks them for the web app; the test prints them (Liberation Sans and DejaVu Sans Mono on Boat).
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
                if changed < 64 {
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
    let mut cx = app("graphite");
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
    let ground = lum(image.get_pixel(2, 2));
    let start = (f32::from(PAD) * scale) as u32 - 6;
    // A line through the piece's middle needs its middle, found from its first lit row and column.
    let lit = |x: u32, y: u32| lum(image.get_pixel(x, y)) > ground + 1.;
    let top = (start..image.height()).find(|&y| (start..image.width()).any(|x| lit(x, y))).unwrap();
    let left = (start..image.width()).find(|&x| (start..image.height()).any(|y| lit(x, y))).unwrap();
    let bottom = (top..image.height()).take_while(|&y| (left..image.width()).any(|x| lit(x, y))).last().unwrap();
    let right = (left..image.width()).take_while(|&x| (top..=bottom).any(|y| lit(x, y))).last().unwrap();
    let (mx, my) = ((left + right) / 2, (top + bottom) / 2);
    let row = |xs: &mut dyn Iterator<Item = u32>| xs.map(|x| lum(image.get_pixel(x, my))).collect::<Vec<_>>();
    let col = |ys: &mut dyn Iterator<Item = u32>| ys.map(|y| lum(image.get_pixel(mx, y))).collect::<Vec<_>>();
    let l = start as f32 + edge(&row(&mut (start..mx)), ground);
    let r = (right + 6) as f32 - edge(&row(&mut (mx..right + 6).rev()), ground);
    let t = start as f32 + edge(&col(&mut (start..my)), ground);
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

fn select_state(window: &mut Window, cx: &mut gpui_kit::App) -> Entity<ui::SelectList> {
    let items = SearchableVec::new(vec![SharedString::from("The same window")]);
    cx.new(|cx| ui::SelectList::new(items, Some(IndexPath::new(0)), window, cx))
}

fn the_pieces_draw_the_web_radii_and_control_sizes() {
    let select_classes = web::constant(web::LAYOUT, "SELECT_WIDTH");
    let field_classes = web::constant(web::LAYOUT, "ROW_FIELD");
    let (image, scale) = shoot(|window, cx| Select::new(&select_state(window, cx)).into_any_element());
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

    let (image, scale) = shoot(|_, _| Card::new().item(Line::new("l", "Disk").value("9.4 GB")).into_any_element());
    near("the card's radius", measure(&image, scale).radius, web::px(web::constant(web::ROWS, "CARD_SURFACE"), "rounded"));

    // A first child that paints a fill keeps inside the card's corner, as the web's overflow-hidden card clips it.
    let (image, scale) = shoot(|_, cx| {
        let fill = cx.palette().foreground;
        Card::new().item(CardItem::placed(move |corners| corners.apply(div().h(px(40.)).bg(fill)).into_any_element())).into_any_element()
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

    let copy = web::literal(web::SHEET_PARTS, "flex h-10 w-full items-center gap-3 rounded-");
    let (image, scale) =
        shoot(|_, _| div().w(px(480.)).child(CopyRow::new("c", "curl -fsSL https://wsp.sh/install | sh")).into_any_element());
    let m = measure(&image, scale);
    near("the copy row's height", m.height, web::px(copy, "h"));
    near("the copy row's radius", m.radius, web::px(copy, "rounded"));

    let popup = web::constant(web::DIALOG_STYLES, "DIALOG_POPUP_CLASS");
    let (image, scale) = shoot(|_, _| Dialog::new("Add a computer").count(2, 5).into_any_element());
    let m = measure(&image, scale);
    near("the dialog's width", m.width, web::px(web::constant(web::STEP_DIALOG, "STEP_WIDTH"), "max-w"));
    near("the dialog's radius", m.radius, web::px(popup, "rounded"));
}
