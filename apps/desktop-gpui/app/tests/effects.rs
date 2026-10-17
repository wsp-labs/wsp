// SPDX-License-Identifier: AGPL-3.0-only
//! Backdrop blur and rounded clipping of children, drawn by GPUI's real offscreen renderer (WGPU on Linux, Metal on
//! macOS) and compared with Chromium's rendering of the same CSS in effects/chromium/, made by effects/reference.sh.
//! Captures and diff images go to $EFFECTS_OUT, by default effects/ in cargo's target folder. Runs without libtest's
//! harness because AppKit panics unless it starts on the main thread.

#[path = "effects/compare.rs"]
mod compare;
#[path = "effects/scene.rs"]
mod scene;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::Assets;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    canvas, div, img, point, prelude::*, px, rgb, size, AnyWindowHandle, AppContext, AssetSource, Bounds, BoxShadow, Context,
    HeadlessAppContext, PathBuilder, RenderImage, SharedString, Window, WindowBounds, WindowOptions,
};
use scene::{Scene, H, W};

/// Per-pixel tolerance, the largest channel difference a pixel may have from Chromium's, and how many pixels of a
/// capture may go over it. Measured on WGPU over Mesa lavapipe; see the report for each scene's numbers.
fn tolerance(scene: Scene) -> (u8, usize) {
    match scene {
        Scene::PlainLight | Scene::Clip | Scene::ClipNested => (2, 0),
        Scene::BlurLight | Scene::BlurDark | Scene::BlurTwo | Scene::BlurRadii | Scene::BlurTintDark | Scene::BlurShadow => (4, 0),
    }
}

fn out_dir() -> PathBuf {
    std::env::var_os("EFFECTS_OUT").map(PathBuf::from).unwrap_or_else(|| {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"));
        target.join("effects")
    })
}

struct Effects {
    scene: Scene,
    scroll: f32,
}

impl Render for Effects {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        scene::draw(self.scene, self.scroll)
    }
}

fn render<V: Render>(cx: &mut HeadlessAppContext, view: impl FnOnce() -> V + 'static) -> image::RgbaImage {
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Default::default(), size: size(px(W), px(H)) })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| view()))
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
    let image = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    cx.update_window(handle, |_, window, _| window.remove_window()).unwrap();
    image
}

/// gpui-kit's assets, and a filled square for the svg below.
struct TestAssets;

impl AssetSource for TestAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if path == "effects/square.svg" {
            return Ok(Some(Cow::Borrowed(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24"/></svg>"#,
            )));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

/// Every kind of primitive the shaders draw, each filling one corner of a rounded box that hides its overflow: a
/// path, an svg (a monochrome sprite), an image (a polychrome sprite), a shadow, and in a second box text.
struct Kinds;

const KIND_BOX: f32 = 240.;
const KIND_RADIUS: f32 = 48.;
const PATH_RED: u32 = 0xdc2626;
const SVG_BLUE: u32 = 0x2563eb;
const IMAGE_GREEN: u32 = 0x00a000;
const SHADOW_VIOLET: u32 = 0x7c3aed;
const TEXT_INK: u32 = 0x111111;

impl Render for Kinds {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let quarter = KIND_BOX / 2.;
        let path = canvas(
            |_, _, _| {},
            |bounds, _, window, _| {
                let mut builder = PathBuilder::fill();
                builder.move_to(bounds.origin);
                builder.line_to(bounds.top_right());
                builder.line_to(bounds.bottom_right());
                builder.line_to(bounds.bottom_left());
                builder.close();
                window.paint_path(builder.build().unwrap(), rgb(PATH_RED));
            },
        );
        let pixels = image::RgbaImage::from_pixel(8, 8, image::Rgba([0x00, 0xa0, 0x00, 0xff]));
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
        let shadow = BoxShadow {
            color: rgb(SHADOW_VIOLET).into(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(40.),
            inset: false,
        };
        let quadrant = |x: f32, y: f32| div().absolute().left(px(x)).top(px(y)).size(px(quarter));
        let shapes = div()
            .absolute()
            .left(px(100.))
            .top(px(100.))
            .size(px(KIND_BOX))
            .rounded(px(KIND_RADIUS))
            .overflow_hidden()
            .child(quadrant(0., 0.).child(path.size_full()))
            .child(quadrant(quarter, 0.).child(svg_square()))
            .child(quadrant(0., quarter).child(img(image).size_full()))
            .child(div().absolute().left(px(KIND_BOX - 10.)).top(px(KIND_BOX - 10.)).size(px(20.)).shadow(vec![shadow]));
        let text = div()
            .absolute()
            .left(px(380.))
            .top(px(100.))
            .size(px(KIND_BOX))
            .rounded(px(KIND_RADIUS))
            .border_8()
            .border_color(gpui_kit::rgba(0x0000_0040))
            .overflow_hidden()
            .text_size(px(60.))
            .line_height(px(40.))
            .text_color(rgb(TEXT_INK))
            .children((0..8).map(|_| div().whitespace_nowrap().child("\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}")));
        div().size_full().bg(rgb(0xf4f4f5)).child(shapes).child(text)
    }
}

fn svg_square() -> impl IntoElement {
    gpui_kit::svg().path("effects/square.svg").size_full().text_color(rgb(SVG_BLUE))
}

/// Inside each corner's arc the child's color shows; just outside it, inside the box's square corner, the page.
fn the_rounded_clip_cuts_every_kind_of_primitive(cx: &mut HeadlessAppContext, out: &Path) -> Vec<String> {
    let shot = render(cx, || Kinds);
    shot.save(out.join("gpui-clip-kinds.png")).unwrap();
    let at = |x: f32, y: f32| rgb_at(&shot, x, y);
    let near = |a: [u8; 3], b: u32| near(a, b, 3);
    let (page, inset, cut) = (0xf4f4f5, 22., 3.);
    let mut failed = Vec::new();
    // Each corner as (its x, its y, which way is inward, the color that fills it).
    let corners = [
        ("path", 100., 100., (1., 1.), PATH_RED),
        ("svg", 340., 100., (-1., 1.), SVG_BLUE),
        ("image", 100., 340., (1., -1.), IMAGE_GREEN),
        ("shadow", 340., 340., (-1., -1.), SHADOW_VIOLET),
        ("text", 380., 100., (1., 1.), TEXT_INK),
        ("text", 620., 340., (-1., -1.), TEXT_INK),
    ];
    for (kind, x, y, (dx, dy), color) in corners {
        let (inside, outside) = (at(x + dx * inset, y + dy * inset), at(x + dx * cut, y + dy * cut));
        let ok = near(inside, color) && near(outside, page);
        println!(
            "{:4} clip-kinds {kind:7} corner ({x}, {y}): inside {inside:?}, outside the arc {outside:?}",
            if ok { "ok" } else { "FAIL" }
        );
        if !ok {
            failed.push(format!("clip-kinds {kind} ({x}, {y})"));
        }
    }
    // Between the border's outer and inner arcs the text shows through the translucent border only if the clip took
    // the border box instead of the padding box.
    for (x, y, (dx, dy)) in [(380., 100., (1., 1.)), (620., 340., (-1., -1.))] {
        let seen = at(x + dx * 17., y + dy * 17.);
        let ok = near(seen, 0xb7b7b8);
        println!("{:4} clip-kinds border  corner ({x}, {y}): between the arcs {seen:?}", if ok { "ok" } else { "FAIL" });
        if !ok {
            failed.push(format!("clip-kinds border ({x}, {y})"));
        }
    }
    failed
}

fn rgb_at(shot: &image::RgbaImage, x: f32, y: f32) -> [u8; 3] {
    let scale = shot.width() as f32 / W;
    let p = shot.get_pixel((x * scale) as u32, (y * scale) as u32);
    [p[0], p[1], p[2]]
}

fn near(a: [u8; 3], b: u32, tolerance: u8) -> bool {
    a.iter().zip([(b >> 16) as u8, (b >> 8) as u8, b as u8]).all(|(a, b)| a.abs_diff(b) <= tolerance)
}

/// The pixels where a scene's break shows, checked on their own so the report names the break.
fn samples(scene: Scene, name: &str, actual: &image::RgbaImage, expected: &image::RgbaImage, tolerance: u8) -> Vec<String> {
    let mut failed = Vec::new();
    let mut check = |what: String, ok: bool, seen: String| {
        println!("{:4} {name:16} {what}: {seen}", if ok { "ok" } else { "FAIL" });
        if !ok {
            failed.push(format!("{name} {what}"));
        }
    };
    match scene {
        // Just outside the panel's arc the page shows, though a card's fill reaches there inside the card's own clip.
        Scene::ClipNested => {
            let (page, card) = (0xf4f4f5, scene::CUT_CARD);
            for (x, y, (dx, dy)) in [(160., 40., (1., 1.)), (480., 40., (-1., 1.)), (160., 440., (1., -1.)), (480., 440., (-1., -1.))] {
                let (inside, outside) = (rgb_at(actual, x + dx * 15., y + dy * 15.), rgb_at(actual, x + dx * 3., y + dy * 3.));
                let ok = near(inside, card, tolerance) && near(outside, page, tolerance);
                check(format!("panel corner ({x}, {y})"), ok, format!("inside the arc {inside:?}, outside the arc {outside:?}"));
            }
        }
        // The shadow of the composer scrolled out of the pane's top, below its 4 px left in the pane.
        Scene::BlurShadow => {
            for (x, y) in [(70., 206.), (320., 205.), (320., 212.), (560., 208.)] {
                let (seen, chromium) = (rgb_at(actual, x, y), rgb_at(expected, x, y));
                let ok = seen.iter().zip(chromium).all(|(a, b)| a.abs_diff(b) <= tolerance);
                check(format!("shadow below the pane's top at ({x}, {y})"), ok, format!("{seen:?}, Chromium {chromium:?}"));
            }
        }
        _ => {}
    }
    failed
}

fn main() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(TestAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    let (out, references) = (out_dir(), Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/effects/chromium"));
    let mut failed = Vec::new();
    for scene in Scene::ALL {
        let (tolerance, allowed) = tolerance(scene);
        for &scroll in scene.scrolls() {
            let name = format!("{}-{scroll}", scene.name());
            let actual = render(&mut cx, move || Effects { scene, scroll });
            std::fs::create_dir_all(&out).unwrap();
            actual.save(out.join(format!("gpui-{name}.png"))).unwrap();
            let reference = references.join(format!("{name}.png"));
            let expected = image::open(&reference).unwrap_or_else(|e| panic!("{}: {e}", reference.display())).into_rgba8();
            let stats = compare::compare(&actual, &expected, tolerance, &out.join(format!("diff-{name}.png")));
            let verdict = if stats.over > allowed { "FAIL" } else { "ok" };
            println!("{verdict:4} {name:16} tolerance {tolerance}: {stats}");
            failed.extend(samples(scene, &name, &actual, &expected, tolerance));
            if stats.over > allowed {
                failed.push(name);
            }
        }
    }
    failed.extend(the_rounded_clip_cuts_every_kind_of_primitive(&mut cx, &out));
    println!("captures and diffs in {}", out.display());
    assert!(failed.is_empty(), "failed: {failed:?}");
    println!("test effects_match_chromium ... ok");
}
