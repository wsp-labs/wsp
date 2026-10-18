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
    canvas, div, img, prelude::*, px, AnyElement, App, Context, Entity, FocusHandle, FontWeight, Image, ImageFormat, KeyDownEvent,
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
/// The pictures one panel keeps decoded, the least lately drawn let go first; past it a picture drawn again waits for
/// the slate's next write.
const KEPT: usize = 64;
/// A picture is held at most this many pixels on its longer side.
const LONGEST: u32 = 2048;
/// A thumbnail's `aspect-[16/10]` and the strip's `w-40`.
const THUMB_RATIO: f32 = 10. / 16.;
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
}

/// The panel's pictures by src, and what it asked for each.
pub struct Shots {
    held: HashMap<String, Held>,
    said: HashMap<String, Said>,
    /// The mark each src was last asked at: the slate's version, its runs ended and the domains allowed.
    asked: HashMap<String, String>,
    turns: HashMap<String, u64>,
    used: u64,
    /// Rises each time the person allows a domain, so every picture asks again.
    pub round: u64,
}

impl Shots {
    fn new(cx: &mut Context<Self>) -> Self {
        cx.on_release(|shots: &mut Shots, cx| {
            for (_, held) in shots.held.drain() {
                cx.drop_image(held.pic.image, None);
            }
        })
        .detach();
        Shots { held: HashMap::new(), said: HashMap::new(), asked: HashMap::new(), turns: HashMap::new(), used: 0, round: 0 }
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

    /// The shot for `src`, noting it drawn; and the version it holds when it is to be asked again at `mark`.
    fn want(&mut self, src: &str, mark: &str) -> (Shot, Option<(Option<String>, u64)>) {
        self.used += 1;
        let used = self.used;
        if let Some(h) = self.held.get_mut(src) {
            h.used = used;
        }
        let ask = (self.asked.get(src).map(String::as_str) != Some(mark)).then(|| {
            self.asked.insert(src.to_owned(), mark.to_owned());
            self.said.remove(src);
            let turn = self.turns.entry(src.to_owned()).or_default();
            *turn += 1;
            (self.held.get(src).and_then(|h| h.version.clone()), *turn)
        });
        (self.shot(src), ask)
    }

    fn land(&mut self, src: String, turn: u64, answer: Answer, cx: &mut Context<Self>) {
        if self.turns.get(&src) != Some(&turn) {
            return;
        }
        match answer {
            Answer::Unchanged => return,
            Answer::Pic(pic, version) => {
                self.said.remove(&src);
                self.used += 1;
                if let Some(old) = self.held.insert(src, Held { pic, version, used: self.used }) {
                    cx.drop_image(old.pic.image, None);
                }
                while self.held.len() > KEPT {
                    let Some(oldest) = self.held.iter().min_by_key(|(_, h)| h.used).map(|(s, _)| s.clone()) else { break };
                    if let Some(gone) = self.held.remove(&oldest) {
                        cx.drop_image(gone.pic.image, None);
                    }
                }
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

/// The bytes as a picture, shrunk to `LONGEST` on its longer side.
fn decode(src: &str, media: &str, bytes: &str, svg: gpui_kit::SvgRenderer) -> Result<Pic, String> {
    let drew = || format!("{src} did not draw");
    let bytes = base64::engine::general_purpose::STANDARD.decode(bytes).map_err(|_| drew())?;
    let format = ImageFormat::from_mime_type(media).ok_or_else(drew)?;
    let image = Image::from_bytes(format, bytes).to_image_data(svg).map_err(|_| drew())?;
    let size = image.size(0);
    let (w, h) = (size.width.0.max(1) as u32, size.height.0.max(1) as u32);
    if w.max(h) <= LONGEST {
        return Ok(Pic { image, w: w as f32, h: h as f32 });
    }
    let bgra = image.as_bytes(0).ok_or_else(drew)?.to_vec();
    let buffer = image::RgbaImage::from_raw(w, h, bgra).ok_or_else(drew)?;
    let k = LONGEST as f32 / w.max(h) as f32;
    let small = image::imageops::thumbnail(&buffer, ((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1));
    Ok(Pic { image: Arc::new(RenderImage::new(vec![image::Frame::new(small)])), w: w as f32, h: h as f32 })
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
    window.use_keyed_state(SharedString::from(format!("slate-image-shots-{thread}")), cx, |_, cx| Shots::new(cx))
}

/// The shot for each src, asking the host again for any whose mark moved: at every write of the slate, every run on
/// it that finishes and every domain the person allows. The ask waits until the panel is drawn, which holds it.
fn shots_for(p: &PieceCx, srcs: &[String], window: &mut Window, cx: &mut App) -> (Entity<Shots>, Vec<Shot>) {
    let store = shots(&p.slate.record.thread_id, window, cx);
    let round = store.read(cx).round;
    let mark = format!("{}:{}:{round}", p.slate.record.version, runs_ended(p.slate));
    let mut out = Vec::new();
    for src in srcs {
        let (shot, ask) = store.update(cx, |s, _| s.want(src, &mark));
        out.push(shot);
        let Some((have, turn)) = ask else { continue };
        let (screen, store, src) = (p.screen.clone(), store.clone(), src.clone());
        cx.defer(move |cx| {
            let Ok(asking) = screen.update(cx, |s, cx| s.image(src.clone(), have, cx)) else { return };
            let svg = cx.svg_renderer();
            cx.spawn(async move |cx| {
                let answer = match asking.await {
                    Err(why) => Answer::Said(Said::Refused(why)),
                    Ok(v) if v.get("unchanged").is_some() => Answer::Unchanged,
                    Ok(v) if v.get("ask").is_some() => {
                        Answer::Said(Said::Asking(v["ask"]["domain"].as_str().unwrap_or_default().to_owned()))
                    }
                    Ok(v) if v.get("problem").is_some() => {
                        Answer::Said(Said::Refused(sentence(v["problem"]["message"].as_str().unwrap_or_default())))
                    }
                    Ok(v) => {
                        let (media, bytes) =
                            (v["mediaType"].as_str().unwrap_or_default().to_owned(), v["bytes"].as_str().unwrap_or_default().to_owned());
                        let version = v.get("version").and_then(Value::as_str).map(str::to_owned);
                        let named = src.clone();
                        match cx.background_executor().spawn(async move { decode(&named, &media, &bytes, svg) }).await {
                            Ok(pic) => Answer::Pic(pic, version),
                            Err(why) => Answer::Said(Said::Refused(why)),
                        }
                    }
                };
                store.update(cx, |s, cx| s.land(src, turn, answer, cx));
            })
            .detach();
        });
    }
    (store, out)
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
    img(pic.image.clone()).absolute().left(px((bw - w) / 2.)).top(px(top)).w(px(w)).h(px(h)).into_any_element()
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
        Shot::Loading => h_flex()
            .debug_selector(|| "slate-image-loading".into())
            .absolute()
            .inset_0()
            .justify_center()
            .child(spinner(cx))
            .into_any_element(),
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

/// A diagram as its caption and its Mermaid source in the mono, `diagram.tsx`'s own `source` line; the drawing waits
/// on the owner's word.
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
    let (store, shots) = shots_for(p, std::slice::from_ref(&src), window, cx);
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
    let (store, shots) = shots_for(p, &srcs, window, cx);
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
        .min_w_0()
        .gap(px(6.))
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
                .shadow(vec![gpui_kit::BoxShadow {
                    color: pal.keycap_top,
                    offset: gpui_kit::point(px(0.), px(1.)),
                    blur_radius: px(0.),
                    spread_radius: px(0.),
                    inset: true,
                }])
                .child(Icon::new(Glyph::GripVertical).with_size(px(12.)).text_color(pal.muted_foreground))
                .into_any_element(),
        ]
    };
    let (down, moving, keys, up) = (move_to.clone(), move_to, split.clone(), dragging.clone());
    let (o1, o2, d1, d2) = (bounds.clone(), bounds, dragging.clone(), dragging);
    let ring = focus.is_focused(window) && window.last_input_was_keyboard();
    v_flex()
        .debug_selector(|| "slate-images-compare".into())
        .min_w_0()
        .gap(px(8.))
        .child(
            frame(cx)
                .id(p.el("compare"))
                .track_focus(&focus)
                .h(px(height))
                .when(ring, |el| el.border_color(pal.ring.opacity(0.4)))
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
    let viewer = cx.new(|cx| Viewer { items, at, shots, screen, focus: cx.focus_handle() });
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
        let shot = self.shots.read(cx).shot(&item.src);
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
            Shot::Shown(pic) => img(pic.image.clone()).size_full().into_any_element(),
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
    use super::sentence;

    #[test]
    fn a_problem_reads_as_a_sentence_unless_it_opens_on_a_path() {
        assert_eq!(sentence("this list holds 61 images"), "This list holds 61 images");
        assert_eq!(sentence("shots/missing.png is not there"), "shots/missing.png is not there");
    }
}
