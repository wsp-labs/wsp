// SPDX-License-Identifier: AGPL-3.0-only
//! Pictures read by the host and handed over as bytes, after `apps/web/src/slate/pieces/image.tsx`: a file on the
//! thread's computer, or an address the host fetches once the person allows its domain for the thread, so this window
//! never requests one itself. One image fits the panel's width with its caption under it; several sit in a row that
//! scrolls sideways, in a grid, or as a before and after under one slider. Each opens large in a dialog, zoomed and
//! panned in one frame, with previous and next where there are several. A picture loading, missing or waiting on the
//! person's word keeps its frame at the size the picture would take.

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _, WindowExt as _};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    canvas, div, img, prelude::*, px, relative, AnyElement, App, Bounds, Context, Entity, FocusHandle, FontWeight, KeyDownEvent,
    MouseButton, ObjectFit, Pixels, RenderImage, SharedString, Task, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol;

use super::{look, quiet, zoom, PieceCx};
use crate::screens::slate::{call, refusal, Slate, SlateScreen};
use crate::ui::{spinner, tip, ActivePalette, Button, Glyph};

/// A frame's tallest inline, so a phone's screenshot never swallows the slate.
const INLINE_CAP: f32 = 420.;
/// The shape a frame holds before its picture is known: a laptop screen's.
const UNKNOWN_RATIO: f32 = 10. / 16.;
/// The dialog fits a picture whole down to this scale.
const OVERVIEW_LEAST: f32 = 0.2;
/// `SLATE_LIMITS.imagesPerList`.
const IMAGES_PER_LIST: usize = 60;
/// The pictures one panel keeps decoded and the bytes they may take, the least lately drawn away from view let go
/// first; a picture near view in the panel's last draw is never let go, and one let go is asked again when it comes near.
/// So a panel holds 64 pictures, or the ones near view, each thumbnail at most one `THUMB_BOX` (1,024,000 bytes).
const KEPT: usize = 64;
const KEPT_BYTES: u64 = 64 << 20;
/// A picture is held at most this many pixels on its longer side.
const LONGEST: u32 = 2048;
/// A thumbnail holds only the part of its picture its box shows, at most this size: its widest box at a scale of 2.
const THUMB_BOX: (u32, u32) = (640, 400);
/// A thumbnail's `aspect-[16/10]` and the strip's `w-40`.
const THUMB_RATIO: f32 = 10. / 16.;
/// The widest a thumbnail's box is inside its 1 px edge, once its height is rounded, for a box 20 px wide or more.
const THUMB_WIDEST: f32 = 1.65;
const STRIP_THUMB: f32 = 160.;

pub mod words {
    pub fn ask(domain: &str) -> String {
        format!("Show images from {domain} in this thread? wsp fetches them, not this window.")
    }
    pub const ALLOW: &str = "Allow";
    pub const NONE: &str = "No images yet";
    pub const EXPAND: &str = "Expand";
    pub fn pair(count: usize) -> String {
        format!("A before and after shows two images; this list has {count}")
    }
    pub fn too_many(count: usize) -> String {
        format!(
            "This list holds {count} images and a slate shows at most {}, so it shows none; show a page of them",
            super::IMAGES_PER_LIST
        )
    }
}

/// A picture as the window holds it: decoded, with the size it was drawn at before any shrinking.
#[derive(Clone)]
pub struct Pic {
    pub image: Arc<RenderImage>,
    pub w: f32,
    pub h: f32,
    /// The part of the picture the image holds, in the picture's own pixels, when it holds less than the whole.
    pub part: Option<Bounds<f32>>,
}

/// What a frame holds.
#[derive(Clone)]
pub enum Shot {
    Loading,
    Shown(Pic),
    Asking(String),
    Refused(String),
}

/// An answer the host gave for a src that is not a picture.
#[derive(Clone)]
enum Said {
    Asking(String),
    Refused(String),
}

struct Held {
    pic: Pic,
    version: Option<String>,
    used: u64,
    /// Held at `LONGEST`, not a thumbnail's size.
    full: bool,
    bytes: u64,
}

/// What an ask was for: the slate's mark it was made at, and whether it wanted the picture whole.
struct Asked {
    mark: String,
    full: bool,
}

/// Each thread's pictures, so the bytes the slates hold can be read.
#[derive(Default)]
struct ShotsByThread(HashMap<String, WeakEntity<Shots>>);

impl gpui_kit::Global for ShotsByThread {}

/// The decoded bytes every slate panel holds.
pub fn held_bytes(cx: &App) -> u64 {
    held_each(cx).iter().sum()
}

/// The decoded bytes of each picture every slate panel holds.
pub fn held_each(cx: &App) -> Vec<u64> {
    let Some(all) = cx.try_global::<ShotsByThread>() else { return Vec::new() };
    all.0.values().filter_map(|s| s.upgrade()).flat_map(|s| s.read(cx).held.values().map(|h| h.bytes).collect::<Vec<_>>()).collect()
}

/// The panel's pictures by src, and what it asked for each.
pub struct Shots {
    held: HashMap<String, Held>,
    said: HashMap<String, Said>,
    /// The mark each src was last asked at: the slate's version, its runs ended and the domains allowed.
    asked: HashMap<String, Asked>,
    /// Each src's last ask, numbered from `asks` so an answer to an ask from before a sweep forgot the src never lands.
    turns: HashMap<String, u64>,
    asks: u64,
    used: u64,
    /// Whether each src's frame was within a screen of its scroll box when last drawn.
    near: HashMap<String, bool>,
    /// The panel draw each src was last drawn in, and the draw now going.
    drawn: HashMap<String, u64>,
    draw: u64,
    /// Set while a sweep waits on the end of the panel's draw.
    sweeping: bool,
    /// The src the open dialog shows, near however the panel draws.
    viewing: Option<String>,
    /// The mark the panel last drew at, for the dialog's asks.
    mark: String,
    /// Rises each time the person allows a domain, so every picture asks again.
    pub round: u64,
}

impl Shots {
    fn new(thread: &str, cx: &mut Context<Self>) -> Self {
        let me = cx.entity().downgrade();
        let all = cx.default_global::<ShotsByThread>();
        all.0.retain(|_, s| s.upgrade().is_some());
        all.0.insert(thread.to_owned(), me);
        cx.on_release(|shots: &mut Shots, cx| {
            for (_, held) in shots.held.drain() {
                cx.drop_image(held.pic.image, None);
            }
        })
        .detach();
        Shots {
            held: HashMap::new(),
            said: HashMap::new(),
            asked: HashMap::new(),
            turns: HashMap::new(),
            asks: 0,
            used: 0,
            near: HashMap::new(),
            drawn: HashMap::new(),
            draw: 0,
            sweeping: false,
            viewing: None,
            mark: String::new(),
            round: 0,
        }
    }

    /// Notes whether `src`'s frame is near view; true when it just came near and has yet to be asked.
    fn set_near(&mut self, src: &str, near: bool) -> bool {
        let was = self.near.insert(src.to_owned(), near).unwrap_or(false);
        near && !was && !self.asked.contains_key(src)
    }

    pub fn shot(&self, src: &str) -> Shot {
        if let Some(h) = self.held.get(src) {
            return Shot::Shown(h.pic.clone());
        }
        match self.said.get(src) {
            Some(Said::Asking(d)) => Shot::Asking(d.clone()),
            Some(Said::Refused(w)) => Shot::Refused(w.clone()),
            None => Shot::Loading,
        }
    }

    /// The shot for `src`, noting it drawn; and, when it is near view or `reached` and is to be asked again at `mark`,
    /// the version it holds and the turn of the ask. A `full` ask wants the picture whole, not at a thumbnail's size.
    fn want(&mut self, src: &str, mark: &str, full: bool, reached: bool) -> (Shot, Option<Ask>) {
        self.used += 1;
        let used = self.used;
        let held_full = self.held.get_mut(src).map(|h| {
            h.used = used;
            h.full
        });
        if reached {
            self.viewing = Some(src.to_owned());
        } else {
            self.drawn.insert(src.to_owned(), self.draw);
        }
        let near = self.is_near(src);
        let asked = self.asked.get(src);
        let stale = asked.is_none_or(|a| a.mark != mark);
        let grow = full && held_full == Some(false) && !asked.is_some_and(|a| a.full);
        let ask = (near && (stale || grow)).then(|| {
            let full = full || held_full == Some(true);
            self.asked.insert(src.to_owned(), Asked { mark: mark.to_owned(), full });
            self.said.remove(src);
            self.asks += 1;
            self.turns.insert(src.to_owned(), self.asks);
            let have = self.held.get(src).filter(|h| h.full || !full).and_then(|h| h.version.clone());
            Ask { have, turn: self.asks, full }
        });
        (self.shot(src), ask)
    }

    fn land(&mut self, src: String, turn: u64, full: bool, answer: Answer, cx: &mut Context<Self>) {
        if self.turns.get(&src) != Some(&turn) {
            return;
        }
        match answer {
            Answer::Unchanged => return,
            Answer::Pic(pic, version) => {
                self.said.remove(&src);
                self.used += 1;
                let bytes = pic.image.as_bytes(0).map_or(0, |b| b.len() as u64);
                let held = Held { pic, version, used: self.used, full, bytes };
                if let Some(old) = self.held.insert(src.clone(), held) {
                    cx.drop_image(old.pic.image, None);
                }
                self.let_go(&src, cx);
            }
            Answer::Said(said) => {
                if let Some(old) = self.held.remove(&src) {
                    cx.drop_image(old.pic.image, None);
                }
                self.said.insert(src, said);
            }
        }
        cx.notify();
    }

    fn is_near(&self, src: &str) -> bool {
        self.viewing.as_deref() == Some(src) || self.near.get(src).copied().unwrap_or(false)
    }

    /// Ends a panel draw: a picture it did not draw, and the dialog does not show, is let go and forgotten, as the web
    /// lets go of an image it unmounts.
    fn sweep(&mut self, cx: &mut Context<Self>) {
        self.sweeping = false;
        let draw = self.draw;
        self.draw += 1;
        self.drawn.retain(|_, d| *d == draw);
        let (drawn, viewing) = (std::mem::take(&mut self.drawn), self.viewing.clone());
        let kept = |src: &str| drawn.contains_key(src) || viewing.as_deref() == Some(src);
        self.near.retain(|src, _| kept(src));
        self.asked.retain(|src, _| kept(src));
        self.said.retain(|src, _| kept(src));
        self.turns.retain(|src, _| kept(src));
        let gone: Vec<String> = self.held.keys().filter(|src| !kept(src)).cloned().collect();
        for src in gone {
            if let Some(h) = self.held.remove(&src) {
                cx.drop_image(h.pic.image, None);
            }
        }
        self.drawn = drawn;
        self.let_go("", cx);
    }

    /// Lets go of the least lately drawn pictures away from view while the panel holds more than it keeps.
    fn let_go(&mut self, landed: &str, cx: &mut Context<Self>) {
        loop {
            let bytes: u64 = self.held.values().map(|h| h.bytes).sum();
            if self.held.len() <= KEPT && bytes <= KEPT_BYTES {
                return;
            }
            let away = self
                .held
                .iter()
                .filter(|(src, _)| src.as_str() != landed && !self.is_near(src))
                .min_by_key(|(_, h)| h.used)
                .map(|(src, _)| src.clone());
            let Some(oldest) = away else { return };
            if let Some(gone) = self.held.remove(&oldest) {
                cx.drop_image(gone.pic.image, None);
            }
            self.asked.remove(&oldest);
        }
    }
}

/// An ask to send: the version held, its turn, and whether it wants the picture whole.
struct Ask {
    have: Option<String>,
    turn: u64,
    full: bool,
}

enum Answer {
    Pic(Pic, Option<String>),
    Unchanged,
    Said(Said),
}

/// A problem's words as the panel says them, capital first, unless they open on a path.
fn sentence(s: &str) -> String {
    let lower_word = s.split_once(' ').is_some_and(|(w, _)| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase()));
    if !lower_word {
        return s.to_owned();
    }
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// The bytes as a picture, shrunk to `LONGEST` on its longer side, or for a thumbnail cut to what its box shows and
/// shrunk into `THUMB_BOX`. Its size is read before its pixels, which decode once, first frame only, into the buffer the
/// window keeps.
fn decode(src: &str, media: &str, bytes: &str, full: bool) -> Result<Pic, String> {
    let drew = || format!("{src} did not draw");
    let bytes = base64::engine::general_purpose::STANDARD.decode(bytes).map_err(|_| drew())?;
    let kind = image::ImageFormat::from_mime_type(media).ok_or_else(drew)?;
    let reader = || image::ImageReader::with_format(std::io::Cursor::new(&bytes), kind);
    let (w, h) = reader().into_dimensions().map_err(|_| drew())?;
    if w == 0 || h == 0 {
        return Err(drew());
    }
    if !fits(w, h) {
        return Err(drew());
    }
    let decoded = reader().decode().map_err(|_| drew())?;
    let part = if full { None } else { thumb_part(w, h) };
    let (x, y, pw, ph) = part.unwrap_or((0, 0, w, h));
    let (nw, nh) = held_size(pw, ph, full);
    // Shrunk straight from the decoded pixels, so a large picture is never held twice at its full size.
    let mut pixels = match part {
        None if (nw, nh) == (w, h) => decoded.into_rgba8(),
        None => image::imageops::thumbnail(&decoded, nw, nh),
        Some(_) => {
            let shown = image::imageops::crop_imm(&decoded, x, y, pw, ph);
            if (nw, nh) == (pw, ph) {
                shown.to_image()
            } else {
                image::imageops::thumbnail(&*shown, nw, nh)
            }
        }
    };
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let part = part.map(|(x, y, pw, ph)| Bounds::new(gpui_kit::point(x as f32, y as f32), gpui_kit::size(pw as f32, ph as f32)));
    Ok(Pic { image: Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])), w: w as f32, h: h as f32, part })
}

/// The part of a `w` by `h` picture a thumbnail's box shows, as `object-cover object-top` lays it: the top of a tall
/// picture and the middle of a wide one; none when the box shows it whole.
fn thumb_part(w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let tall = (w as f32 * THUMB_RATIO).ceil() as u32;
    let wide = (h as f32 * THUMB_WIDEST).ceil() as u32;
    if tall < h {
        Some((0, 0, w, tall))
    } else if wide < w {
        Some(((w - wide) / 2, 0, wide, h))
    } else {
        None
    }
}

/// `decode` on a background thread, a picture past `ALONE_PIXELS` waiting its turn without holding the thread, so a
/// gallery of large pictures peaks at one full decode, not one per background thread.
async fn decoded(src: String, media: String, bytes: String, full: bool) -> Result<Pic, String> {
    let _alone = if large(&media, &bytes) { Some(ALONE.lock().await) } else { None };
    decode(&src, &media, &bytes, full)
}

/// 4096 x 4096: past it a picture decodes alone.
const ALONE_PIXELS: u64 = 1 << 24;

static ALONE: futures::lock::Mutex<()> = futures::lock::Mutex::new(());

/// Whether the bytes' header names a picture past `ALONE_PIXELS`.
fn large(media: &str, bytes: &str) -> bool {
    let (Some(kind), Ok(raw)) = (image::ImageFormat::from_mime_type(media), base64::engine::general_purpose::STANDARD.decode(bytes)) else {
        return false;
    };
    image::ImageReader::with_format(std::io::Cursor::new(raw), kind)
        .into_dimensions()
        .is_ok_and(|(w, h)| u64::from(w) * u64::from(h) > ALONE_PIXELS)
}

/// The image crate's own cap on one decode.
const LARGEST_DECODE: u64 = 512 << 20;

/// Whether a picture of this size decodes as RGBA under the image crate's own cap.
fn fits(w: u32, h: u32) -> bool {
    u64::from(w) * u64::from(h) * 4 <= LARGEST_DECODE
}

/// The size a picture, or a thumbnail's part of one, of `w` by `h` is held at.
fn held_size(w: u32, h: u32, full: bool) -> (u32, u32) {
    let k = if full { LONGEST as f32 / w.max(h) as f32 } else { (THUMB_BOX.0 as f32 / w as f32).min(THUMB_BOX.1 as f32 / h as f32) };
    if k >= 1. {
        return (w, h);
    }
    (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1))
}

impl SlateScreen {
    /// A picture's bytes from the host's `slates.image`, naming the version held so an unchanged file is not sent again.
    pub fn image(&mut self, src: String, have: Option<String>, cx: &mut Context<Self>) -> Task<Result<Value, String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self.host().run(move |host| async move {
            let mut params = json!({ "threadId": thread, "src": src });
            if let Some(have) = have {
                params["have"] = json!(have);
            }
            call::<protocol::SlatesImageRequest>(&host, params).await.map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
        });
        cx.spawn(async move |_, _| asking.await.map_err(|f| refusal(&f)))
    }
}

/// How many times the slate's runs have finished, which moves when a run ends and not while it ticks along.
fn runs_ended(slate: &Slate) -> u64 {
    let Some(doc) = slate.doc() else { return 0 };
    doc.runs
        .keys()
        .filter_map(|name| slate.record.values.get(name)?.as_object())
        .map(|run| {
            let runs = run.get("runs").and_then(Value::as_u64).unwrap_or_default();
            if run.get("state").and_then(Value::as_str) == Some("running") {
                runs.saturating_sub(1)
            } else {
                runs
            }
        })
        .sum()
}

/// The panel's pictures, kept while a piece draws one.
fn shots(thread: &str, window: &mut Window, cx: &mut App) -> Entity<Shots> {
    window.use_keyed_state(SharedString::from(format!("slate-image-shots-{thread}")), cx, |_, cx| Shots::new(thread, cx))
}

/// The shot for each src, asking the host again for any near view whose mark moved: at every write of the slate, every
/// run on it that finishes and every domain the person allows. The ask waits until the panel is drawn, which holds it.
fn shots_for(p: &PieceCx, srcs: &[String], full: bool, window: &mut Window, cx: &mut App) -> (Entity<Shots>, Vec<Shot>) {
    let store = shots(&p.slate.record.thread_id, window, cx);
    let round = store.read(cx).round;
    let mark = format!("{}:{}:{round}", p.slate.record.version, runs_ended(p.slate));
    let mut out = Vec::new();
    let sweep = store.update(cx, |s, _| !std::mem::replace(&mut s.sweeping, true));
    if sweep {
        // Runs once the window's draw is done, after every piece and probe of this draw has marked its srcs.
        let store = store.clone();
        cx.defer(move |cx| store.update(cx, |s, cx| s.sweep(cx)));
    }
    for src in srcs {
        let (shot, ask) = store.update(cx, |s, _| {
            s.mark.clone_from(&mark);
            s.want(src, &mark, full, false)
        });
        out.push(shot);
        if let Some(ask) = ask {
            send(&store, &p.screen, src.clone(), ask, cx);
        }
    }
    (store, out)
}

/// Asks the host for `src` once the frame is drawn, and lands its answer.
fn send(store: &Entity<Shots>, screen: &WeakEntity<SlateScreen>, src: String, ask: Ask, cx: &mut App) {
    let (screen, store) = (screen.clone(), store.clone());
    let Ask { have, turn, full } = ask;
    cx.defer(move |cx| {
        let Ok(asking) = screen.update(cx, |s, cx| s.image(src.clone(), have, cx)) else { return };
        cx.spawn(async move |cx| {
            let answer = match asking.await {
                Err(why) => Answer::Said(Said::Refused(why)),
                Ok(v) if v.get("unchanged").is_some() => Answer::Unchanged,
                Ok(v) if v.get("ask").is_some() => Answer::Said(Said::Asking(v["ask"]["domain"].as_str().unwrap_or_default().to_owned())),
                Ok(v) if v.get("problem").is_some() => {
                    Answer::Said(Said::Refused(sentence(v["problem"]["message"].as_str().unwrap_or_default())))
                }
                Ok(v) => {
                    let (media, bytes) =
                        (v["mediaType"].as_str().unwrap_or_default().to_owned(), v["bytes"].as_str().unwrap_or_default().to_owned());
                    let version = v.get("version").and_then(Value::as_str).map(str::to_owned);
                    let named = src.clone();
                    match cx.background_executor().spawn(decoded(named, media, bytes, full)).await {
                        Ok(pic) => Answer::Pic(pic, version),
                        Err(why) => Answer::Said(Said::Refused(why)),
                    }
                }
            };
            store.update(cx, |s, cx| s.land(src, turn, full, answer, cx));
        })
        .detach();
    });
}

/// Watches whether a frame is within a screen of the box it scrolls in, as the web's `rootMargin: 100%` does, so its
/// picture is asked for only then. It sits outside the frame, whose own clip would hide the scroll box's.
fn near_probe(store: &Entity<Shots>, src: &str) -> AnyElement {
    let (store, src) = (store.clone(), src.to_owned());
    canvas(
        move |bounds, window, cx| {
            let view = window.content_mask().bounds;
            let reach = gpui_kit::Bounds {
                origin: view.origin - gpui_kit::point(view.size.width, view.size.height),
                size: gpui_kit::size(view.size.width * 3., view.size.height * 3.),
            };
            let near = reach.intersects(&bounds);
            store.update(cx, |s, cx| {
                if s.set_near(&src, near) {
                    cx.notify();
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// The width a box was laid out at last frame, read by a probe filling it.
fn width_of(key: SharedString, window: &mut Window, cx: &mut App) -> (f32, AnyElement) {
    let width = window.use_keyed_state(key, cx, |_, _| px(0.));
    let probe = width.clone();
    let probe = canvas(
        move |bounds, _, cx| {
            probe.update(cx, |w, cx| {
                if *w != bounds.size.width {
                    *w = bounds.size.width;
                    cx.notify();
                }
            })
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element();
    (f32::from(*width.read(cx)), probe)
}

/// How tall a picture stands at this width, held to the cap; one not known yet takes the laptop screen's shape.
fn height_at(shot: &Shot, width: f32) -> f32 {
    let ratio = match shot {
        Shot::Shown(pic) => pic.h / pic.w,
        _ => UNKNOWN_RATIO,
    };
    INLINE_CAP.min((width * ratio).round())
}

/// `CARD_SURFACE` as a picture's frame.
fn frame(cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    div()
        .relative()
        .w_full()
        .overflow_hidden()
        .rounded(crate::ui::size::CARD_RADIUS)
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.4))
}

/// How a picture fills its box.
#[derive(Clone, Copy, PartialEq)]
enum Fit {
    /// Whole, centred: `object-contain`.
    Contain,
    /// Filling the box, cut at the bottom and sides: `object-cover object-top`.
    Cover,
}

/// A picture laid in a box of `size` (less the frame's edge), as CSS's object fit lays it.
fn picture(pic: &Pic, fit: Fit, size: (f32, f32)) -> AnyElement {
    let (bw, bh) = (size.0 - 2., size.1 - 2.);
    if bw <= 0. || bh <= 0. {
        return img(pic.image.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Contain).into_any_element();
    }
    let k = match fit {
        Fit::Contain => (bw / pic.w).min(bh / pic.h),
        Fit::Cover => (bw / pic.w).max(bh / pic.h),
    };
    let (w, h) = (pic.w * k, pic.h * k);
    let top = if fit == Fit::Cover { 0. } else { (bh - h) / 2. };
    let part = pic.part.unwrap_or(Bounds::new(gpui_kit::point(0., 0.), gpui_kit::size(pic.w, pic.h)));
    img(pic.image.clone())
        .absolute()
        .left(px((bw - w) / 2. + part.origin.x * k))
        .top(px(top + part.origin.y * k))
        .w(px(part.size.width * k))
        .h(px(part.size.height * k))
        .into_any_element()
}

/// The person's word on a domain, asked inside the frame its picture would fill; nothing is fetched before it. A
/// thumbnail holds the domain alone, its question on hover.
fn allow_domain(
    id: SharedString,
    domain: &str,
    compact: bool,
    screen: &WeakEntity<SlateScreen>,
    store: &Entity<Shots>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = window.use_keyed_state(SharedString::from(format!("{id}-allow")), cx, |_, _| (false, None::<String>));
    let (busy, refused) = state.read(cx).clone();
    let p = cx.palette();
    let said = refused.unwrap_or_else(|| if compact { domain.to_owned() } else { words::ask(domain) });
    let (screen, store, key) = (screen.clone(), store.clone(), format!("domain:{domain}"));
    let allow =
        Button::outline(SharedString::from(format!("{id}-allow-button")), words::ALLOW).xs().busy(busy).on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                *s = (true, None);
                cx.notify();
            });
            let Ok(asking) = screen.update(cx, |s, cx| s.approve(key.clone(), "thread", cx)) else { return };
            let (state, store) = (state.clone(), store.clone());
            cx.spawn(async move |cx| {
                let answer = asking.await;
                state.update(cx, |s, cx| {
                    *s = (false, answer.as_ref().err().cloned());
                    cx.notify();
                });
                if answer.is_ok() {
                    store.update(cx, |s, cx| {
                        s.round += 1;
                        cx.notify();
                    });
                }
            })
            .detach();
        });
    v_flex()
        .id(SharedString::from(format!("{id}-asking")))
        .debug_selector(|| "slate-image-asking".into())
        .absolute()
        .inset_0()
        .justify_center()
        .items_start()
        .gap(px(if compact { 6. } else { 8. }))
        .px(px(if compact { 12. } else { 16. }))
        .py(px(if compact { 8. } else { 12. }))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .when(compact, |el| el.tooltip(tip(words::ask(domain))))
        .child(div().max_w_full().when(compact, |el| el.truncate()).child(said))
        .child(allow)
        .into_any_element()
}

/// What a frame holds: the picture, the spinner while it loads, the domain's question, or one muted line where it
/// cannot be shown.
#[allow(clippy::too_many_arguments)]
fn inside(
    id: SharedString,
    shot: &Shot,
    fit: Fit,
    size: (f32, f32),
    screen: &WeakEntity<SlateScreen>,
    store: &Entity<Shots>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette();
    match shot {
        Shot::Shown(pic) => picture(pic, fit, size),
        Shot::Loading => {
            let spinner = spinner(cx).size(px(16.)).key(SharedString::from(format!("{id}-spinner")));
            h_flex()
                .debug_selector(|| "slate-image-loading".into())
                .absolute()
                .inset_0()
                .justify_center()
                .child(div().debug_selector(move || format!("{id}-loading")).child(spinner))
                .into_any_element()
        }
        Shot::Asking(domain) => allow_domain(id, domain, fit == Fit::Cover, screen, store, window, cx),
        Shot::Refused(why) => h_flex()
            .debug_selector(|| "slate-image-refused".into())
            .absolute()
            .inset_0()
            .px(px(16.))
            .py(px(12.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .child(why.clone())
            .into_any_element(),
    }
}

fn caption(text: Option<String>, cx: &App) -> Option<gpui_kit::Div> {
    let text = text.filter(|t| !t.is_empty())?;
    Some(div().min_w_0().truncate().text_size(px(12.)).line_height(px(16.)).text_color(cx.palette().muted_foreground).child(text))
}

/// One picture of a list: its src and caption.
#[derive(Clone)]
struct Item {
    src: String,
    caption: Option<String>,
}

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "image" => Some(image(p, window, cx)),
        "images" => Some(images(p, window, cx)),
        "diagram" => diagram(p, cx),
        _ => None,
    }
}

/// A diagram as its caption and its Mermaid source in the mono, `diagram.tsx`'s own `source` line; the diagram
/// itself is not drawn.
fn diagram(p: &PieceCx, cx: &App) -> Option<AnyElement> {
    let pal = cx.palette();
    let code = p.str("value").unwrap_or_default();
    let label = p.str("label");
    let caption = |text: String| div().min_w_0().text_size(px(13.)).line_height(px(20.)).text_color(pal.foreground).child(text);
    if code.is_empty() {
        return label.map(|l| caption(l).into_any_element());
    }
    let mono = gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone();
    Some(
        v_flex()
            .debug_selector(|| "slate-diagram".into())
            .min_w_0()
            .gap(px(10.))
            .children(label.map(|l| h_flex().min_h(px(24.)).child(caption(l))))
            .child(
                div()
                    .debug_selector(|| "slate-diagram-source".into())
                    .min_w_0()
                    .pt(px(4.))
                    .pb(px(8.))
                    .font_family(mono)
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(pal.muted_foreground)
                    .child(code),
            )
            .into_any_element(),
    )
}

fn image(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let src = p.str("src").unwrap_or_default();
    let cap = p.str("caption");
    let (store, shots) = shots_for(p, std::slice::from_ref(&src), true, window, cx);
    let shot = shots.into_iter().next().unwrap_or(Shot::Loading);
    let (width, probe) = width_of(p.el("width"), window, cx);
    let height = height_at(&shot, if width > 0. { width } else { 508. });
    let item = Item { src: src.clone(), caption: cap.clone() };
    let shown = matches!(shot, Shot::Shown(_));
    let open = {
        let (store, screen, item) = (store.clone(), p.screen.clone(), item.clone());
        move |window: &mut Window, cx: &mut App| open_viewer(vec![item.clone()], 0, store.clone(), screen.clone(), window, cx)
    };
    let opener = open.clone();
    let body = inside(p.el("pic"), &shot, Fit::Contain, (width, height), &p.screen, &store, window, cx);
    v_flex()
        .debug_selector(|| "slate-image".into())
        .relative()
        .min_w_0()
        .gap(px(8.))
        .child(probe)
        .child(near_probe(&store, &src))
        .child(
            frame(cx)
                .id(p.el("frame"))
                .debug_selector(|| "slate-image-frame".into())
                .h(px(height))
                .when(shown, |el| el.cursor_pointer().on_click(move |_, window, cx| opener(window, cx)))
                .child(body)
                .test_support(),
        )
        .child(h_flex().min_h(px(24.)).gap(px(12.)).child(div().flex_1().min_w_0().children(caption(cap, cx))).when(shown, |el| {
            el.child(
                Button::icon_xs(p.el("expand"), Glyph::Maximize2).tooltip(words::EXPAND).on_click(move |_, window, cx| open(window, cx)),
            )
        }))
        .into_any_element()
}

fn images(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(rows) = &p.rows else { return div().into_any_element() };
    let items: Vec<Item> = rows
        .iter()
        .map(|r| Item { src: look::str(r.props.get("src")).unwrap_or_default(), caption: look::str(r.props.get("caption")) })
        .collect();
    let layout = p.str("layout").unwrap_or_else(|| "gallery".into());
    let pair = layout == "compare";
    let over = items.len() > IMAGES_PER_LIST;
    let shown: Vec<Item> = if over {
        Vec::new()
    } else if pair {
        items.iter().take(2).cloned().collect()
    } else {
        items.clone()
    };
    let srcs: Vec<String> = shown.iter().map(|i| i.src.clone()).collect();
    let (store, shots) = shots_for(p, &srcs, pair, window, cx);
    let (width, probe) = width_of(p.el("width"), window, cx);
    let width = if width > 0. { width } else { 508. };
    let body = if over {
        quiet(words::too_many(items.len()), cx)
    } else if items.is_empty() {
        quiet(words::NONE, cx)
    } else if layout == "strip" {
        strip(p, &shown, &shots, &store, window, cx)
    } else if pair {
        slider(p, &shown, &shots, &store, width, window, cx)
    } else {
        gallery(p, &shown, &shots, &store, width, window, cx)
    };
    v_flex()
        .debug_selector(|| "slate-images".into())
        .relative()
        .min_w_0()
        .gap(px(8.))
        .child(probe)
        .child(body)
        .when(pair && items.len() != 2, |el| el.child(quiet(words::pair(items.len()), cx)))
        .into_any_element()
}

/// One thumbnail: its frame at 16 by 10, cropped to fill, and its caption.
#[allow(clippy::too_many_arguments)]
fn thumb(
    p: &PieceCx,
    all: &[Item],
    at: usize,
    shot: &Shot,
    store: &Entity<Shots>,
    w: f32,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::Div {
    let h = (w * THUMB_RATIO).round();
    let id = p.el(&format!("thumb-{at}"));
    let body = inside(id.clone(), shot, Fit::Cover, (w, h), &p.screen, store, window, cx);
    let shown = matches!(shot, Shot::Shown(_));
    let (items, store, screen) = (all.to_vec(), store.clone(), p.screen.clone());
    v_flex()
        .debug_selector(|| "slate-image-thumb".into())
        .relative()
        .min_w_0()
        .gap(px(6.))
        .child(near_probe(&store, &all[at].src))
        .child(
            frame(cx)
                .id(id.clone())
                .debug_selector(move || id.to_string())
                .h(px(h))
                .when(shown, |el| {
                    el.cursor_pointer()
                        .on_click(move |_, window, cx| open_viewer(items.clone(), at, store.clone(), screen.clone(), window, cx))
                })
                .child(body)
                .test_support(),
        )
        .children(caption(all[at].caption.clone(), cx))
}

fn gallery(
    p: &PieceCx,
    items: &[Item],
    shots: &[Shot],
    store: &Entity<Shots>,
    width: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let cols = if width >= 400. { 3 } else { 2 };
    let w = ((width - 12. * (cols as f32 - 1.)) / cols as f32).floor();
    let mut rows = Vec::new();
    for (r, chunk) in shots.chunks(cols).enumerate() {
        let cells: Vec<_> = chunk
            .iter()
            .enumerate()
            .map(|(c, shot)| thumb(p, items, r * cols + c, shot, store, w, window, cx).w(px(w)).flex_none())
            .collect();
        rows.push(h_flex().items_start().gap(px(12.)).children(cells));
    }
    v_flex().debug_selector(|| "slate-images-gallery".into()).gap(px(16.)).children(rows).into_any_element()
}

fn strip(p: &PieceCx, items: &[Item], shots: &[Shot], store: &Entity<Shots>, window: &mut Window, cx: &mut App) -> AnyElement {
    let cells: Vec<_> = shots
        .iter()
        .enumerate()
        .map(|(at, shot)| thumb(p, items, at, shot, store, STRIP_THUMB, window, cx).w(px(STRIP_THUMB)).flex_none())
        .collect();
    div()
        .id(p.el("strip"))
        .debug_selector(|| "slate-images-strip".into())
        .min_w_0()
        .overflow_x_scroll()
        .child(h_flex().items_start().gap(px(12.)).children(cells))
        .into_any_element()
}

/// Before and after in one frame: the after drawn over the before right of a line the person drags or moves with the
/// arrow keys.
#[allow(clippy::too_many_arguments)]
fn slider(p: &PieceCx, items: &[Item], shots: &[Shot], store: &Entity<Shots>, width: f32, window: &mut Window, cx: &mut App) -> AnyElement {
    let pal = cx.palette().clone();
    let split = window.use_keyed_state(p.el("split"), cx, |_, _| 50f32);
    let focus = window.use_keyed_state(p.el("split-focus"), cx, |_, cx| cx.focus_handle().tab_stop(true)).read(cx).clone();
    let dragging = window.use_keyed_state(p.el("split-drag"), cx, |_, _| false);
    let at = *split.read(cx);
    let (before, after) = (shots.first().cloned().unwrap_or(Shot::Loading), shots.get(1).cloned().unwrap_or(Shot::Loading));
    let height = height_at(&before, width).max(height_at(&after, width));
    let both = matches!((&before, &after), (Shot::Shown(_), Shot::Shown(_)));
    let size = (width, height);
    let inner = width - 2.;
    let line = (inner * at / 100.).round();
    let move_to = {
        let split = split.clone();
        move |x: Pixels, origin: Pixels, cx: &mut App| {
            let to = (f32::from(x - origin) / inner * 100.).clamp(0., 100.);
            split.update(cx, |s, cx| {
                *s = to;
                cx.notify();
            });
        }
    };
    let bounds = window.use_keyed_state(p.el("split-box"), cx, |_, _| px(0.));
    let probe = {
        let bounds = bounds.clone();
        canvas(move |b, _, cx| bounds.update(cx, |o, _| *o = b.origin.x), |_, _, _, _| {}).absolute().size_full()
    };
    let body = if !both {
        let one = if matches!(before, Shot::Shown(_)) { &after } else { &before };
        vec![inside(p.el("pic-0"), one, Fit::Contain, size, &p.screen, store, window, cx)]
    } else {
        let under = inside(p.el("pic-0"), &before, Fit::Contain, size, &p.screen, store, window, cx);
        let over = inside(p.el("pic-1"), &after, Fit::Contain, size, &p.screen, store, window, cx);
        vec![
            under,
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(line))
                .right_0()
                .overflow_hidden()
                .child(div().absolute().top_0().left(px(-line)).w(px(inner)).h(px(height - 2.)).child(over))
                .into_any_element(),
            div().absolute().top_0().bottom_0().left(px(line)).w(px(1.)).bg(pal.foreground.opacity(0.7)).into_any_element(),
            h_flex()
                .absolute()
                .top(px((height - 2.) / 2. - 14.))
                .left(px(line - 10.))
                .w(px(20.))
                .h(px(28.))
                .justify_center()
                .rounded(px(8.))
                .border_1()
                .border_color(pal.border)
                .bg(pal.background)
                .shadow(crate::ui::controls::bevel(&pal))
                .child(Icon::new(Glyph::GripVertical).with_size(px(12.)).text_color(pal.muted_foreground))
                .into_any_element(),
        ]
    };
    let (down, moving, keys, up) = (move_to.clone(), move_to, split.clone(), dragging.clone());
    let (o1, o2, d1, d2) = (bounds.clone(), bounds, dragging.clone(), dragging);
    let ring = focus.is_focused(window) && window.last_input_was_keyboard();
    v_flex()
        .debug_selector(|| "slate-images-compare".into())
        .relative()
        .min_w_0()
        .gap(px(8.))
        .children(items.iter().map(|i| near_probe(store, &i.src)))
        .child(
            frame(cx)
                .id(p.el("compare"))
                .track_focus(&focus)
                .h(px(height))
                .when(ring, |el| el.shadow(vec![crate::ui::focus::ring(pal.ring.opacity(0.4))]))
                .when(both, |el| el.cursor_ew_resize())
                .child(probe)
                .children(body)
                .on_mouse_down(MouseButton::Left, move |e, _, cx| {
                    if both {
                        d1.update(cx, |d, _| *d = true);
                        down(e.position.x, *o1.read(cx) + px(1.), cx);
                    }
                })
                .on_mouse_move(move |e, _, cx| {
                    if *d2.read(cx) && e.pressed_button == Some(MouseButton::Left) {
                        moving(e.position.x, *o2.read(cx) + px(1.), cx);
                    }
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| up.update(cx, |d, _| *d = false))
                .on_key_down(move |e: &KeyDownEvent, _, cx| {
                    let by = match e.keystroke.key.as_str() {
                        "left" => -5.,
                        "right" => 5.,
                        _ => return,
                    };
                    cx.stop_propagation();
                    keys.update(cx, |s, cx| {
                        *s = (*s + by).clamp(0., 100.);
                        cx.notify();
                    });
                }),
        )
        .child(
            h_flex()
                .min_h(px(24.))
                .gap(px(12.))
                .child(div().flex_1().min_w_0().children(caption(items.first().and_then(|i| i.caption.clone()), cx)))
                .children(caption(items.get(1).and_then(|i| i.caption.clone()), cx)),
        )
        .into_any_element()
}

/// The dialog a picture opens large in: one at a time, zoomed and panned in its frame.
pub struct Viewer {
    items: Vec<Item>,
    at: usize,
    shots: Entity<Shots>,
    screen: WeakEntity<SlateScreen>,
    focus: FocusHandle,
}

/// `EXPANDED_POPUP`: most of the window, `h-[85vh] w-[90vw] max-w-[min(90vw,1400px)]`.
fn popup_size(window: &Window) -> (Pixels, Pixels) {
    let view = window.viewport_size();
    ((view.width * 0.9).min(px(1400.)), view.height * 0.85)
}

fn open_viewer(items: Vec<Item>, at: usize, shots: Entity<Shots>, screen: WeakEntity<SlateScreen>, window: &mut Window, cx: &mut App) {
    let viewer = cx.new(|cx| {
        cx.on_release(|v: &mut Viewer, cx| v.shots.update(cx, |s, _| s.viewing = None)).detach();
        Viewer { items, at, shots, screen, focus: cx.focus_handle() }
    });
    let focus = viewer.read(cx).focus.clone();
    let (w, h) = popup_size(window);
    let top = (window.viewport_size().height - h) / 2.;
    window.open_dialog(cx, move |dialog, _, _| {
        let viewer = viewer.clone();
        dialog.close_button(false).w(w).h(h).margin_top(top).p_0().content(move |content, _, _| content.child(viewer.clone()))
    });
    window.focus(&focus, cx);
}

impl Viewer {
    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.items.len() as isize;
        if n > 1 {
            self.at = ((self.at as isize + by + n) % n) as usize;
            cx.notify();
        }
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let (w, h) = popup_size(window);
        let item = self.items[self.at].clone();
        let (shot, ask) = self.shots.update(cx, |s, _| {
            let mark = s.mark.clone();
            s.want(&item.src, &mark, true, true)
        });
        if let Some(ask) = ask {
            send(&self.shots, &self.screen, item.src.clone(), ask, cx);
        }
        let natural = match &shot {
            Shot::Shown(pic) => (pic.w, pic.h),
            _ => (1., 1.),
        };
        let zoom = zoom::state(SharedString::from(format!("slate-image-zoom-{}", self.at)), natural, OVERVIEW_LEAST, window, cx);
        let count = self.items.len();
        let me = cx.entity().downgrade();
        let icon = |id: &'static str, glyph: Glyph, label: &'static str, run: Box<dyn Fn(&mut App)>| {
            Button::icon_xs(id, glyph).tooltip(label).on_click(move |_, _, cx| run(cx))
        };
        let (zout, zin, zfit) = (zoom.clone(), zoom.clone(), zoom.clone());
        let (prev, next) = (me.clone(), me);
        let stepper = (count > 1).then(|| {
            h_flex()
                .flex_none()
                .gap(px(4.))
                .child(icon(
                    "slate-image-prev",
                    Glyph::ChevronLeft,
                    "Previous",
                    Box::new(move |cx| _ = prev.update(cx, |v, cx| v.step(-1, cx))),
                ))
                .child(
                    div()
                        .debug_selector(|| "slate-image-at".into())
                        .min_w(px(48.))
                        .text_center()
                        .font_family(gpui_kit::component::ActiveTheme::theme(&**cx).mono_font_family.clone())
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(p.muted_foreground)
                        .child(format!("{} of {count}", self.at + 1)),
                )
                .child(icon(
                    "slate-image-next",
                    Glyph::ChevronRight,
                    "Next",
                    Box::new(move |cx| _ = next.update(cx, |v, cx| v.step(1, cx))),
                ))
        });
        let body = match &shot {
            Shot::Shown(pic) => match pic.part {
                None => img(pic.image.clone()).size_full().into_any_element(),
                // A thumbnail's part, in its place, until the whole picture lands.
                Some(part) => div()
                    .relative()
                    .size_full()
                    .child(
                        img(pic.image.clone())
                            .absolute()
                            .left(relative(part.origin.x / pic.w))
                            .top(relative(part.origin.y / pic.h))
                            .w(relative(part.size.width / pic.w))
                            .h(relative(part.size.height / pic.h)),
                    )
                    .into_any_element(),
            },
            other => div()
                .relative()
                .h(px(160.))
                .w(px(f32::from(w) - 40.))
                .child(inside(
                    SharedString::from("slate-image-dialog"),
                    other,
                    Fit::Contain,
                    (f32::from(w) - 40., 160.),
                    &self.screen,
                    &self.shots,
                    window,
                    cx,
                ))
                .into_any_element(),
        };
        let shown = matches!(shot, Shot::Shown(_));
        v_flex()
            .id("slate-image-expanded")
            .debug_selector(|| "slate-image-expanded".into())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "left" => this.step(-1, cx),
                "right" => this.step(1, cx),
                _ => {}
            }))
            .w(w)
            .h(h)
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(16.))
                    .pb(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .child(item.caption.clone().unwrap_or(item.src.clone())),
                    )
                    .children(stepper)
                    .child(icon(
                        "slate-image-zoom-out",
                        Glyph::Minus,
                        "Zoom out",
                        Box::new(move |cx| {
                            zout.update(cx, |z, cx| {
                                z.zoom(1. / 1.25);
                                cx.notify();
                            })
                        }),
                    ))
                    .child(icon(
                        "slate-image-zoom-in",
                        Glyph::Plus,
                        "Zoom in",
                        Box::new(move |cx| {
                            zin.update(cx, |z, cx| {
                                z.zoom(1.25);
                                cx.notify();
                            })
                        }),
                    ))
                    .child(icon(
                        "slate-image-fit",
                        Glyph::Scan,
                        "Fit",
                        Box::new(move |cx| {
                            zfit.update(cx, |z, cx| {
                                z.fit();
                                cx.notify();
                            })
                        }),
                    ))
                    .child(
                        Button::icon_xs("slate-image-close", Glyph::X).tooltip("Close").on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
            .child(div().flex_1().min_h_0().mx(px(20.)).mb(px(20.)).rounded(px(14.)).bg(p.background).overflow_hidden().child(if shown {
                zoom::frame(SharedString::from("slate-image-frame"), &zoom, body, cx).size_full().into_any_element()
            } else {
                body
            }))
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use super::{decode, sentence, Answer, Said, Shot, Shots};

    #[test]
    fn a_problem_reads_as_a_sentence_unless_it_opens_on_a_path() {
        assert_eq!(sentence("this list holds 61 images"), "This list holds 61 images");
        assert_eq!(sentence("shots/missing.png is not there"), "shots/missing.png is not there");
    }

    #[gpui_kit::test]
    fn an_answer_to_an_ask_from_before_a_sweep_forgot_its_src_does_not_land(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext as _;
        let shots = cx.new(|cx| Shots::new("t", cx));
        shots.update(cx, |s, cx| {
            let ask = |s: &mut Shots| {
                s.set_near("a.png", true);
                s.want("a.png", "1:0:0", false, false).1.expect("near and never asked, so asked").turn
            };
            let old = ask(s);
            // Two panel draws without it: the first ends the draw it was drawn in, the second forgets it.
            s.sweep(cx);
            s.sweep(cx);
            ask(s);
            s.land("a.png".into(), old, false, Answer::Said(Said::Refused("the old answer".into())), cx);
            assert!(matches!(s.shot("a.png"), Shot::Loading), "the old answer landed over the new ask");
        });
    }

    #[test]
    fn a_gif_is_held_as_its_first_frame_alone() {
        let mut gif = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
            for rgb in [[220, 20, 20], [20, 220, 20], [20, 20, 220]] {
                let frame = image::RgbaImage::from_pixel(4, 4, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
                encoder.encode_frame(image::Frame::new(frame)).unwrap();
            }
        }
        let bytes = base64::engine::general_purpose::STANDARD.encode(&gif);
        let pic = decode("a.gif", "image/gif", &bytes, true).unwrap();
        assert_eq!(pic.image.frame_count(), 1);
        assert_eq!((pic.w, pic.h), (4., 4.));
        let first = pic.image.as_bytes(0).unwrap();
        assert!(first[2] > 150 && first[0] < 100, "the first frame, red, in BGRA: {:?}", &first[..4]);
    }

    /// A GIF of `frames` one-pixel frames on a square `screen`: fifteen bytes a frame.
    fn tiny_frames(screen: u16, frames: usize) -> String {
        let mut b = b"GIF89a".to_vec();
        b.extend(screen.to_le_bytes());
        b.extend(screen.to_le_bytes());
        b.extend([0x80, 0, 0, 0, 0, 0, 255, 255, 255]);
        for _ in 0..frames {
            b.extend([0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 0x01, 0]);
        }
        b.push(0x3B);
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    /// A PNG's signature, its header naming `w` by `h` RGB pixels, an empty stream and its end: no pixel data at all.
    fn png_header(w: u32, h: u32) -> String {
        fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            out.extend((data.len() as u32).to_be_bytes());
            let body: Vec<u8> = kind.iter().chain(data).copied().collect();
            let mut crc = !0u32;
            for byte in &body {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
                }
            }
            out.extend(&body);
            out.extend((!crc).to_be_bytes());
        }
        let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = w.to_be_bytes().to_vec();
        header.extend(h.to_be_bytes());
        header.extend([8, 2, 0, 0, 0]);
        chunk(&mut b, b"IHDR", &header);
        chunk(&mut b, b"IDAT", &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
        chunk(&mut b, b"IEND", &[]);
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    #[test]
    fn a_picture_past_the_cap_is_refused_by_its_header_before_a_pixel_decodes() {
        // 432 MB as RGB, under the image crate's own cap, so only the size read first stops it.
        let refused = decode("wide.png", "image/png", &png_header(12_000, 12_000), true).err();
        assert_eq!(refused.as_deref(), Some("wide.png did not draw"));
        assert!(!super::fits(12_000, 12_000));
        assert!(super::fits(8_192, 8_192));
    }

    #[test]
    fn a_few_hundred_bytes_of_gif_hold_one_frame_and_a_huge_screen_does_not_draw() {
        let pic = decode("big.gif", "image/gif", &tiny_frames(2048, 16), true).expect("a GIF draws");
        assert_eq!(pic.image.frame_count(), 1, "260 bytes on a 2048 px screen keep one frame, not sixteen");
        assert!(decode("huge.gif", "image/gif", &tiny_frames(u16::MAX, 1), true).is_err(), "a 65535 px screen does not draw");
    }
}
