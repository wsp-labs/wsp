// SPDX-License-Identifier: AGPL-3.0-only
//! The app's text styles drawn by GPUI's offscreen renderer and by headless Chrome (text/reference.sh), compared row by
//! row: where each row's ink lands, how wide it is, and the layout width each side shaped. The rows are the samples
//! in text/samples.html, which both sides read, each at the top of its box, and the short one also nudged a quarter
//! pixel and centred in a box, which can start it on half a device pixel, and for the styles marked `second`, a second
//! line, whose glyphs a fractional line height can put on half a device pixel. Prints the table; fails when a row lands or
//! measures off Chrome's by more than the bounds below. Chrome must run at a real device scale
//! (`--force-device-scale-factor`): under emulated scale, as Playwright's `deviceScaleFactor` sets it, Blink rounds font
//! metrics in CSS pixels and puts text up to a device pixel elsewhere. `TEXT_REFERENCE=<folder>` reads `<os>.png` and
//! `<os>.json` from there instead of text/chromium/, and `TEXT_OUT=<folder>` keeps the capture and the table. Without
//! libtest's harness, as `pixels.rs` explains.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_gpui::ui::{self, Side, UiAssets};
use gpui_kit::component::ActiveTheme;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, font, prelude::*, px, rgb, size, AnyWindowHandle, AppContext, Context, FontWeight, HeadlessAppContext, SharedString, TextRun,
    Window, WindowBounds, WindowOptions,
};
use image::RgbaImage;
use serde::Deserialize;

const SAMPLES: &str = include_str!("text/samples.html");
// GPUI's offscreen target stops at 8192 device pixels.
const HEIGHT: f32 = 4096.;
const OS: &str = if cfg!(target_os = "macos") { "macos" } else { "linux" };

/// How far a row's ink may sit from Chrome's, in device pixels: its bottom edge, and its weighted vertical center.
const BOTTOM_BOUND: i32 = 0;
const CENTER_BOUND: f32 = 0.25;
/// How far a row's shaped width may differ from Chrome's, as a share of Chrome's.
const WIDTH_BOUND: f32 = 0.005;

#[derive(Deserialize, Clone)]
struct Spec {
    row: f32,
    left: f32,
    width: f32,
    styles: Vec<Style>,
    weights: Vec<f32>,
    places: Vec<String>,
    #[serde(rename = "box")]
    frame: f32,
    texts: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct Style {
    name: String,
    family: String,
    size: f32,
    line: f32,
    #[serde(default)]
    second: bool,
}

#[derive(Clone)]
struct Sample {
    style: Style,
    weight: f32,
    text: String,
    place: String,
    top: f32,
}

#[derive(Deserialize)]
struct Reference {
    sans: String,
    mono: String,
    rows: Vec<Layout>,
}

#[derive(Deserialize)]
struct Layout {
    width: f32,
    baseline: f32,
}

fn spec() -> Spec {
    let start = SAMPLES.find(r#"type="application/json">"#).expect("samples block") + r#"type="application/json">"#.len();
    let end = start + SAMPLES[start..].find("</script>").unwrap();
    serde_json::from_str(&SAMPLES[start..end]).expect("samples.html's samples parse")
}

fn samples(spec: &Spec) -> Vec<Sample> {
    let mut rows = Vec::new();
    for style in &spec.styles {
        for &weight in &spec.weights {
            for place in &spec.places {
                for text in if place == "top" { &spec.texts[..] } else { &spec.texts[..1] } {
                    if place == "second" && !style.second {
                        continue;
                    }
                    let text = if place == "second" { format!("{text}\n{text}") } else { text.clone() };
                    let nudge = if place == "nudged" { 0.25 } else { 0. };
                    let top = rows.len() as f32 * spec.row + nudge;
                    rows.push(Sample { style: style.clone(), weight, text, place: place.clone(), top });
                }
            }
        }
    }
    rows
}

struct Rows {
    left: f32,
    frame: f32,
    samples: Vec<Sample>,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (sans, mono) = (cx.theme().font_family.clone(), cx.theme().mono_font_family.clone());
        div().size_full().bg(rgb(0xffffff)).children(self.samples.iter().map(|s| {
            let text = div()
                .whitespace_nowrap()
                .font_family(if s.style.family == "mono" { mono.clone() } else { sans.clone() })
                .text_size(px(s.style.size))
                .line_height(px(s.style.line))
                .font_weight(FontWeight(s.weight))
                .text_color(rgb(0x000000))
                .child(SharedString::from(s.text.clone()));
            let row = div().absolute().top(px(s.top)).left(px(self.left));
            if s.place == "centered" {
                row.flex().items_center().h(px(self.frame)).child(text)
            } else {
                row.child(text)
            }
        }))
    }
}

/// A row's ink, in device pixels, from a capture: the columns holding a pixel at least a quarter dark, the last line
/// of pixels whose darkness is at least a quarter of the darkest line's (so a round glyph's faint overshoot does not
/// count), and the vertical center of all its darkness.
#[derive(Clone, Copy, Debug)]
struct Ink {
    left: u32,
    right: u32,
    bottom: u32,
    center: f32,
}

fn ink(image: &RgbaImage, top: u32, bottom: u32) -> Option<Ink> {
    let (mut left, mut right) = (u32::MAX, 0);
    let mut lines = Vec::new();
    for y in top..bottom.min(image.height()) {
        let mut line = 0f64;
        for x in 0..image.width() {
            let p = image.get_pixel(x, y);
            let dark = 255. - (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.;
            line += dark;
            if dark >= 64. {
                (left, right) = (left.min(x), right.max(x + 1));
            }
        }
        lines.push(line);
    }
    let (weight, darkest) = (lines.iter().sum::<f64>(), lines.iter().cloned().fold(0., f64::max));
    let moment: f64 = lines.iter().enumerate().map(|(y, line)| y as f64 * line).sum();
    let last = lines.iter().rposition(|&line| line >= darkest / 4.)? as u32 + 1;
    (weight > 0. && left != u32::MAX).then(|| Ink { left, right, bottom: last, center: (moment / weight) as f32 })
}

/// Of the pixels in a band that either capture inks, the share whose channels differ by more than 8 between the two.
fn differing(a: &RgbaImage, b: &RgbaImage, top: u32, bottom: u32) -> (u64, u64) {
    let (mut inked, mut differ) = (0, 0);
    for y in top..bottom.min(a.height()) {
        for x in 0..a.width() {
            let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
            if p.0[..3].iter().chain(&q.0[..3]).any(|&c| c < 247) {
                inked += 1;
                differ += u64::from(p.0[..3].iter().zip(&q.0[..3]).any(|(c, d)| c.abs_diff(*d) > 8));
            }
        }
    }
    (differ, inked)
}

fn out_dir() -> PathBuf {
    std::env::var_os("TEXT_OUT").map(PathBuf::from).unwrap_or_else(|| {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"));
        target.join("text")
    })
}

fn main() {
    let spec = spec();
    let rows = samples(&spec);
    assert!(
        rows.len() as f32 * spec.row <= HEIGHT,
        "samples.html's rows run past the {HEIGHT} px window, which is at GPUI's 8192 device px cap; shrink the rows"
    );
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (left, frame, view_rows) = (spec.left, spec.frame, rows.clone());
    let (handle, _): (AnyWindowHandle, _) = cx
        .update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(gpui_kit::Bounds {
                    origin: Default::default(),
                    size: size(px(spec.width), px(HEIGHT)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, move |_, cx| cx.new(|_| Rows { left, frame, samples: view_rows }))
        })
        .unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
        cx.run_until_parked();
    }
    let scale = cx.update_window(handle, |_, window, _| window.scale_factor()).unwrap();
    let shot = cx.capture_screenshot(handle).expect("offscreen rendering must be available");
    let (sans, mono, widths) = cx
        .update_window(handle, |_, window, cx| {
            let (sans, mono) = (cx.theme().font_family.clone(), cx.theme().mono_font_family.clone());
            let widths: Vec<f32> = rows
                .iter()
                .map(|s| {
                    let mut f = font(if s.style.family == "mono" { mono.clone() } else { sans.clone() });
                    f.weight = FontWeight(s.weight);
                    let line = s.text.lines().next().unwrap_or_default();
                    let run = TextRun {
                        len: line.len(),
                        font: f,
                        color: rgb(0).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    f32::from(window.text_system().shape_line(line.to_string().into(), px(s.style.size), &[run], None).width)
                })
                .collect();
            (sans, mono, widths)
        })
        .unwrap();
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap();
    shot.save(out.join(format!("gpui-{OS}.png"))).unwrap();

    let folder = std::env::var_os("TEXT_REFERENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/text/chromium"));
    let (png, json) = (folder.join(format!("{OS}.png")), folder.join(format!("{OS}.json")));
    assert!(
        json.exists() && png.exists(),
        "no Chrome reference at {} and {}; run tests/text/reference.sh (gpui drew sans {sans}, mono {mono} at scale {scale})",
        png.display(),
        json.display()
    );
    let reference: Reference = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).expect("reference json");
    let chrome = image::open(&png).unwrap().into_rgba8();
    assert_eq!(reference.rows.len(), rows.len(), "{} is from another samples.html; rerun reference.sh", json.display());
    let as_gpui =
        |face: &str| if matches!(face, "BlinkMacSystemFont" | "system-ui") { ".SystemUIFont".to_string() } else { face.to_string() };
    assert_eq!(
        (as_gpui(&reference.sans), as_gpui(&reference.mono)),
        (sans.to_string(), mono.to_string()),
        "chrome and gpui drew other faces"
    );
    assert_eq!((chrome.width(), chrome.height()), (shot.width(), shot.height()), "chrome and gpui captures differ in size");

    let mut table = vec![
        format!("chrome drew sans {}, mono {}; gpui drew sans {sans}, mono {mono}; device scale {scale}", reference.sans, reference.mono),
        "deltas are gpui minus chrome: widths and chrome's layout baseline in css px; ink bottom, center and width in device px".into(),
        format!(
            "{:16} {:>6} {:17} {:>9} {:>9} {:>7} {:>6} {:>8} {:>6} {:>7} {:>6} {:>6}",
            "style", "weight", "text", "chrome w", "gpui w", "dw", "dw %", "chrome b", "bottom", "center", "ink w", "px %"
        ),
    ];
    let mut failed = Vec::new();
    let mut totals = (0u64, 0u64);
    for ((sample, layout), width) in rows.iter().zip(&reference.rows).zip(&widths) {
        let place = if sample.place == "top" { String::new() } else { format!(" {}", sample.place) };
        let first = sample.text.lines().next().unwrap_or_default();
        let short = format!("{}{place}", if first.len() > 9 { "prompt" } else { first });
        let (top, bottom) = ((sample.top * scale) as u32, ((sample.top + spec.row) * scale) as u32);
        let (differ, inked) = differing(&chrome, &shot, top, bottom);
        totals = (totals.0 + differ, totals.1 + inked);
        let (Some(c), Some(g)) = (ink(&chrome, top, bottom), ink(&shot, top, bottom)) else {
            failed.push(format!("{} {} {short}: no ink", sample.style.name, sample.weight));
            continue;
        };
        let (bottom, center) = (g.bottom as i32 - c.bottom as i32, g.center - c.center);
        let inked_off = bottom.abs() > BOTTOM_BOUND || center.abs() > CENTER_BOUND;
        let ink_w = (g.right - g.left) as i32 - (c.right - c.left) as i32;
        let ink_columns = format!("{bottom:>+6} {center:>+7.2} {ink_w:>+6} {:>6.2}", 100. * differ as f64 / inked.max(1) as f64);
        let dw = width - layout.width;
        let off = inked_off || dw.abs() > WIDTH_BOUND * layout.width;
        table.push(format!(
            "{:16} {:>6} {:17} {:>9.2} {:>9.2} {:>+7.2} {:>+6.2} {:>8.2} {ink_columns}{}",
            sample.style.name,
            sample.weight,
            short,
            layout.width,
            width,
            dw,
            100. * dw / layout.width,
            layout.baseline,
            if off { "  off" } else { "" }
        ));
        if off {
            failed.push(format!("{} {} {short}", sample.style.name, sample.weight));
        }
    }
    table.push(format!("inked pixels differing by more than 8: {:.2}% of {}", 100. * totals.0 as f64 / totals.1 as f64, totals.1));
    let table = table.join("\n");
    println!("{table}");
    std::fs::write(out.join(format!("table-{OS}.txt")), format!("{table}\n")).unwrap();
    println!("capture and table in {}", out.display());
    assert!(failed.is_empty(), "off chrome's text: {failed:?}");
    println!("test text_lands_where_chromium_draws_it ... ok");
}
