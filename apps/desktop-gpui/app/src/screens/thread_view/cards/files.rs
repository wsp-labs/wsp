// SPDX-License-Identifier: AGPL-3.0-only
//! `ChatFileRow`: a person's message's attachments, its images as thumbnails, and the dialog that opens one whole.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, img, prelude::*, px, AnyElement, App, Context, Image, ImageFormat, ImageSource, ObjectFit, RenderImage, SharedString, Window,
};
use wsp_host_client::protocol::SessionsAttachmentRequest;

use super::META;
use crate::thread_view::ThreadView;
use crate::transcript::{FileTile, Row};
use crate::ui::{ActivePalette, Glyph, Palette};

/// One attachment of a person's message as the bubble holds it.
#[derive(Clone)]
pub enum Shot {
    /// Not an image: its tile draws off its record alone.
    File,
    Asking,
    /// The image, its size in pixels where its header says it, and the thumbnail the bubble draws, where one was cut.
    Held(Arc<Image>, Option<(f32, f32)>, Option<Arc<RenderImage>>),
    /// The host keeps none, or did not answer; asked again only once the view drops the row's kept state and draws it anew.
    Missing,
}

/// The thumbnail's side in pixels: the 56 px tile at a Retina scale.
const THUMB_PX: u32 = 112;

/// An image a row let go of, which the next draw takes out of GPUI's cache and the window's atlas.
pub struct Released {
    image: Arc<Image>,
    thumb: Option<Arc<RenderImage>>,
}

thread_local! {
    /// The images opened whole, whose full pixels GPUI decoded and keeps until their row lets go of them.
    pub(super) static OPENED: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
}

/// Takes out the attachments of the rows `keep` turns away; a row drew its images only once it held them all.
pub fn drop_images(images: &mut HashMap<String, Vec<Shot>>, released: &mut Vec<Released>, keep: impl Fn(&str) -> bool) {
    for (_, shots) in images.extract_if(|key, _| !keep(key)) {
        if shots.iter().all(|s| !matches!(s, Shot::Asking | Shot::Missing)) {
            released.extend(shots.into_iter().filter_map(|s| match s {
                Shot::Held(image, _, thumb) => Some(Released { image, thumb }),
                _ => None,
            }));
        }
    }
}

/// Takes the images rows let go of out of the window's atlas and GPUI's cache. An image's full pixels are only asked
/// for where GPUI decoded them, for a bubble with no thumbnail or for the dialog, since asking starts a decode.
pub fn let_go(released: Vec<Released>, window: &mut Window, cx: &mut App) {
    for Released { image, thumb } in released {
        let opened = OPENED.with(|o| o.borrow_mut().remove(&image.id()));
        if let Some(thumb) = thumb {
            cx.drop_image(thumb, Some(window));
            if !opened {
                continue;
            }
        }
        if let Some(decoded) = image.clone().get_render_image(window, cx) {
            cx.drop_image(decoded, Some(window));
        }
        image.remove_asset(cx);
    }
}

/// A message's image as the host sent it, made ready off the UI thread: its bytes for the dialog, its size, and the
/// thumbnail the bubble draws in place of the whole image GPUI would decode and upload for a 56 px tile.
pub fn held(media: &str, base64: &str) -> Option<Shot> {
    let format = image_format(media)?;
    let bytes = unbase64(base64)?;
    let size = image_size(&bytes);
    let thumb = if format == ImageFormat::Gif { None } else { thumbnail(&bytes, media) };
    Some(Shot::Held(Arc::new(Image::from_bytes(format, bytes)), size, thumb))
}

/// The image's middle square, as `object-cover` shows it in a square tile, shrunk to `THUMB_PX`, in the BGRA GPUI draws.
pub fn thumbnail(bytes: &[u8], media: &str) -> Option<Arc<RenderImage>> {
    let format = image::ImageFormat::from_mime_type(media)?;
    let decoded = image::load_from_memory_with_format(bytes, format).ok()?;
    let (w, h) = (decoded.width(), decoded.height());
    let side = w.min(h);
    if side == 0 {
        return None;
    }
    let square = decoded.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let mut pixels = if side > THUMB_PX { image::imageops::thumbnail(&square, THUMB_PX, THUMB_PX) } else { square.into_rgba8() };
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

/// `attachmentLine`: what an image the bubble holds no bytes of reads as, the line the command line prints.
pub fn attachment_line(tile: &FileTile) -> String {
    let size = crate::ui::fmt_bytes(tile.bytes.unwrap_or(0));
    match tile.mime.as_deref().and_then(image_word) {
        Some(word) => format!("[image {size} {word}]"),
        None => format!("[file {size} {}]", tile.name),
    }
}

fn image_word(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn image_format(mime: &str) -> Option<ImageFormat> {
    match mime {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

/// Standard base64 with or without padding; None on anything else.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = clean.strip_suffix(b"==").or_else(|| clean.strip_suffix(b"=")).unwrap_or(&clean);
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let v: Vec<u8> = chunk.iter().map(|c| value(*c)).collect::<Option<_>>()?;
        if v.len() == 1 {
            return None;
        }
        let n = v.iter().enumerate().fold(0u32, |n, (i, x)| n | (*x as u32) << (18 - 6 * i));
        out.extend_from_slice(&n.to_be_bytes()[1..v.len()]);
    }
    Some(out)
}

/// An image's size in pixels off its header, for the dialog to open at: PNG, GIF, JPEG and WebP.
pub fn image_size(b: &[u8]) -> Option<(f32, f32)> {
    let be16 = |i: usize| Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?) as f32);
    let le16 = |i: usize| Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as f32);
    let le24 = |i: usize| Some(*b.get(i)? as f32 + (*b.get(i + 1)? as f32) * 256. + (*b.get(i + 2)? as f32) * 65_536.);
    if b.starts_with(b"\x89PNG") {
        let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
        return Some((w as f32, h as f32));
    }
    if b.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        return match b.get(12..16)? {
            b"VP8X" => Some((le24(24)? + 1., le24(27)? + 1.)),
            b"VP8L" => {
                let bits = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
                Some(((bits & 0x3fff) as f32 + 1., ((bits >> 14) & 0x3fff) as f32 + 1.))
            }
            _ => Some(((le16(26)? as u32 & 0x3fff) as f32, (le16(28)? as u32 & 0x3fff) as f32)),
        };
    }
    if b.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xff {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + be16(i + 2)? as usize;
        }
    }
    None
}

/// Asks the host for each image of the row's message it keeps, once per time the row is drawn anew.
fn recall(view: &mut ThreadView, row: &Row, files: &[FileTile], cx: &mut Context<ThreadView>) {
    if view.images.contains_key(&row.key) || !files.iter().any(|f| f.image) {
        return;
    }
    let (workspace, thread) = (view.followed_workspace().map(str::to_owned), view.followed().map(str::to_owned));
    let shots = files
        .iter()
        .map(|f| match (f.image, &f.id, &workspace, &thread) {
            (false, ..) => Shot::File,
            (true, Some(_), Some(_), Some(_)) => Shot::Asking,
            _ => Shot::Missing,
        })
        .collect();
    view.images.insert(row.key.clone(), shots);
    let (Some(workspace), Some(thread)) = (workspace, thread) else { return };
    for (slot, tile) in files.iter().enumerate() {
        let (true, Some(request)) = (tile.image, tile.id.clone()) else { continue };
        let ask = SessionsAttachmentRequest {
            workspace_id: workspace.clone(),
            thread_id: thread.clone(),
            request_id: request,
            index: tile.index as u64,
        };
        let key = row.key.clone();
        view.call(ask, cx, move |_, kept, cx| {
            take(key, slot, kept.ok().map(|kept| (kept.attachment.media_type, kept.attachment.bytes)), cx)
        });
    }
}

/// Lays the host's answer for slot `slot` of row `key`, a media type and base64, into the row once it is decoded off
/// the UI thread: a screenshot's base64 and pixels take tens of ms, which the UI thread would spend inside a scroll.
pub(super) fn take(key: String, slot: usize, kept: Option<(String, String)>, cx: &mut Context<ThreadView>) {
    let shot = cx.background_spawn(async move { kept.and_then(|(media, bytes)| held(&media, &bytes)) });
    cx.spawn(async move |view, cx| {
        let shot = shot.await.unwrap_or(Shot::Missing);
        _ = view.update(cx, |view, cx| {
            crate::scroll_trace::note(|w| w.images += 1);
            if let Some(slot) = view.images.get_mut(&key).and_then(|shots| shots.get_mut(slot)) {
                *slot = shot;
            }
            cx.notify();
        });
    })
    .detach();
}

/// `ChatFileRow`: images as thumbnails once all are held, else one mono line each; every other file as its tile.
pub fn files(view: &mut ThreadView, row: &Row, files: &[FileTile], p: &Palette, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    if files.is_empty() {
        return None;
    }
    recall(view, row, files, cx);
    let shots = view.images.get(&row.key).cloned().unwrap_or_default();
    let held = files.iter().enumerate().all(|(i, f)| !f.image || matches!(shots.get(i), Some(Shot::Held(..))));
    let mono = cx.theme().mono_font_family.clone();
    let mut tiles: Vec<AnyElement> = Vec::new();
    for (i, tile) in files.iter().enumerate() {
        match (tile.image, shots.get(i)) {
            (true, Some(Shot::Held(image, size, small))) if held => {
                tiles.push(thumb(&row.key, i, &tile.name, image.clone(), small.clone(), *size, p))
            }
            (true, _) => {}
            (false, _) => tiles.push(file_tile(&tile.name, tile.bytes.unwrap_or(0), p, &mono)),
        }
    }
    let lines: Vec<String> = if held { Vec::new() } else { files.iter().filter(|f| f.image).map(attachment_line).collect() };
    Some(
        v_flex()
            .when(!tiles.is_empty(), |el| {
                el.child(h_flex().debug_selector(|| "tiles".into()).flex_wrap().gap(px(6.)).mb(px(8.)).children(tiles))
            })
            .when(!lines.is_empty(), |el| {
                el.child(
                    v_flex()
                        .debug_selector(|| "file-lines".into())
                        .gap(px(2.))
                        .mb(px(4.))
                        .font_family(mono.clone())
                        .text_size(META.0)
                        .line_height(META.1)
                        .text_color(p.muted_foreground)
                        .children(lines),
                )
            })
            .into_any_element(),
    )
}

/// `ChatImageThumb`: 56 px square, 10 px round, a 1 px edge at 60%, the image covering it; a press opens it whole.
fn thumb(
    key: &str,
    at_ix: usize,
    name: &str,
    image: Arc<Image>,
    small: Option<Arc<RenderImage>>,
    size: Option<(f32, f32)>,
    p: &Palette,
) -> AnyElement {
    let name = SharedString::from(name.to_owned());
    let shown = image.clone();
    let source = small.map_or_else(|| ImageSource::Image(image.clone()), ImageSource::Render);
    div()
        .id(SharedString::from(format!("thumb-{key}-{at_ix}")))
        .debug_selector(|| "thumb".into())
        .flex_none()
        .size(px(56.))
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.5))
        .cursor_pointer()
        .child(img(source).size_full().object_fit(ObjectFit::Cover))
        .on_click(move |_, window, cx| open_image(name.clone(), shown.clone(), size, window, cx))
        .into_any_element()
}

/// The image at full size in a dialog, `w-auto max-w-[90vw] p-2`, the image `max-h-[80vh] max-w-[86vw]`, 8 px round.
fn open_image(name: SharedString, image: Arc<Image>, size: Option<(f32, f32)>, window: &mut Window, cx: &mut App) {
    use crate::screens::dialogs::layer::{self, Closes};
    OPENED.with(|o| o.borrow_mut().insert(image.id()));
    let view = cx.new(|_| ImagePreview { name, image, size });
    layer::open(IMAGE_PREVIEW, view.into(), Closes::Outside, None, window, cx);
}

/// The dialogs layer's name for a message's image opened whole.
pub const IMAGE_PREVIEW: &str = "image-preview";

/// What the image dialog draws on the layer, which gives it its backdrop, its place, Escape and the press outside.
struct ImagePreview {
    name: SharedString,
    image: Arc<Image>,
    size: Option<(f32, f32)>,
}

impl Render for ImagePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let view = window.viewport_size();
        let (max_w, max_h) = (view.width * 0.86, view.height * 0.8);
        let (w, h) = match self.size {
            Some((w, h)) => {
                let scale = (max_w / px(w)).min(max_h / px(h)).min(1.);
                (px(w) * scale, px(h) * scale)
            }
            None => (max_w, max_h),
        };
        div().p(px(8.)).rounded(px(14.)).border_1().border_color(p.popover_edge).bg(p.popover).shadow(p.dialog_shadow.clone()).child(
            img(ImageSource::Image(self.image.clone()))
                .debug_selector(|| "image-dialog".into())
                .id(self.name.clone())
                .w(w)
                .h(h)
                .rounded(px(8.))
                .object_fit(ObjectFit::Contain),
        )
    }
}

/// `ChatFileTile`: as tall as a thumbnail, at most 192 px wide, the file glyph and its name over its weight.
fn file_tile(name: &str, size: u64, p: &Palette, mono: &SharedString) -> AnyElement {
    h_flex()
        .debug_selector(|| "file-tile".into())
        .flex_none()
        .h(px(56.))
        .max_w(px(192.))
        .gap(px(8.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.5))
        .pl(px(10.))
        .pr(px(12.))
        .child(Icon::new(Glyph::FileText).with_size(px(16.)).text_color(p.muted_foreground))
        .child(
            v_flex()
                .min_w_0()
                .child(div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.foreground).child(name.to_owned()))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(META.0)
                        .line_height(META.1)
                        .text_color(p.muted_foreground)
                        .child(crate::ui::fmt_bytes(size)),
                ),
        )
        .into_any_element()
}
