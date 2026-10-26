// SPDX-License-Identifier: AGPL-3.0-only
//! The sheet a project's image is chosen in (`ImageDialog.tsx`): the file fitted to the one PNG a project wears, shown
//! at the sizes it draws at on the person's own light and dark themes, and kept only on Use icon. Another file dropped
//! or pasted on the sheet replaces the one shown.

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    div, prelude::*, px, size, AnyElement, App, AsyncApp, ClipboardEntry, DevicePixels, Entity, ExternalPaths, Hsla, PathPromptOptions,
    RenderImage, SharedString, SvgRenderer, SvgSize, WeakEntity, Window,
};
use serde_json::json;
use wsp_host_client::protocol::ProjectsIconRequest;

use super::fit::{self, Refused};
use super::image::render_image;
use super::look::image_mark;
use crate::screens::dialogs::layer;
use crate::screens::settings::model::req;
use crate::screens::settings::pages::devices::CANCEL;
use crate::screens::settings::{Acts, DIALOG};
use crate::ui::theme::theme_for;
use crate::ui::{is_paste, spinner, ActivePalette, AlertDialog, Button, Palette, Refusal, Side};

/// `PROJECTS_WORDS`.
const IMAGE_TITLE: &str = "Project icon";
const IMAGE_LINE: &str = "How it draws on your light and dark themes.";
const IMAGE_NOTE: &str = "Drop or paste another image to replace this one.";
const USE_IMAGE: &str = "Use icon";

/// A file the person gave: a path picked, dropped or copied, or a picture's own bytes off the clipboard.
pub enum Source {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

impl Source {
    /// What a drop or a paste carries, if it carries a file.
    pub fn of_paths(paths: &ExternalPaths) -> Option<Source> {
        paths.paths().first().cloned().map(Source::Path)
    }

    /// The clipboard's first file or picture.
    pub fn of_clipboard(cx: &App) -> Option<Source> {
        cx.read_from_clipboard()?.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(Source::Bytes(image.bytes().to_vec())),
            ClipboardEntry::ExternalPaths(paths) => Source::of_paths(paths),
            ClipboardEntry::String(_) => None,
        })
    }

    fn bytes(self) -> Result<Vec<u8>, Refused> {
        match self {
            Source::Bytes(bytes) => Ok(bytes),
            Source::Path(path) => {
                // Refused off its size before a byte of it is read.
                let size = std::fs::metadata(&path).map_err(|_| fit::unreadable())?.len();
                if size > fit::IMAGE_MAX_BYTES {
                    return Err(fit::too_big());
                }
                std::fs::read(&path).map_err(|_| fit::unreadable())
            }
        }
    }
}

/// One theme the image is shown on.
struct Ground {
    word: SharedString,
    palette: Palette,
}

pub struct Sheet {
    project: String,
    acts: Acts,
    grounds: [Ground; 2],
    fitted: Option<(String, Arc<RenderImage>)>,
    refusal: Option<Refused>,
    fitting: bool,
    saving: bool,
    /// Which file is being fitted, so a slow fit of a file since replaced does not land.
    turn: u64,
}

impl Sheet {
    fn take(this: &Entity<Sheet>, source: Source, cx: &mut App) {
        let turn = this.update(cx, |s, cx| {
            s.turn += 1;
            s.fitting = true;
            s.refusal = None;
            if let Some((_, old)) = s.fitted.take() {
                cx.drop_image(old, None);
            }
            s.turn
        });
        let acts = this.read(cx).acts.clone();
        acts.notify(cx);
        let svg = cx.svg_renderer();
        let fitted = cx.background_spawn(async move {
            let png = fit::fit(&source.bytes()?, |bytes| drawn_svg(&svg, bytes))?;
            let picture = image::load_from_memory_with_format(&png, image::ImageFormat::Png).map_err(|_| fit::unreadable())?.to_rgba8();
            Ok::<_, Refused>((png, picture))
        });
        let this = this.downgrade();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let fitted = fitted.await;
            _ = this.update(cx, |s, cx| {
                if s.turn != turn {
                    return;
                }
                s.fitting = false;
                match fitted {
                    Ok((png, picture)) => {
                        s.fitted = Some((base64::engine::general_purpose::STANDARD.encode(png), Arc::new(render_image(picture))))
                    }
                    Err(refused) => s.refusal = Some(refused),
                }
                s.acts.notify(cx);
            });
        })
        .detach();
    }

    /// Use icon: the fitted PNG to the host, the sheet gone once it is kept, else the host's refusal under the grounds.
    fn save(this: &Entity<Sheet>, cx: &mut App) {
        let (project, png, acts) = {
            let s = this.read(cx);
            let Some((png, _)) = s.fitted.as_ref().filter(|_| !s.saving) else { return };
            (s.project.clone(), png.clone(), s.acts.clone())
        };
        this.update(cx, |s, _| s.saving = true);
        acts.notify(cx);
        let this = this.downgrade();
        acts.call(req::<ProjectsIconRequest>(json!({ "projectId": project, "png": png })), cx, move |answer, cx| {
            _ = this.update(cx, |s, cx| {
                s.saving = false;
                match answer {
                    Ok(_) => s.acts.close_dialog(cx),
                    Err(said) => s.refusal = Some(Refused { said, fix: String::new() }),
                }
                s.acts.notify(cx);
            });
        });
    }
}

/// An SVG drawn 128 px wide as the fit reads it, straight RGBA.
fn drawn_svg(svg: &SvgRenderer, bytes: &[u8]) -> Option<image::RgbaImage> {
    let parsed = svg.parse_svg(bytes).ok()?;
    let width = DevicePixels(fit::ICON_PX as i32);
    let drawn = svg.render_parsed(&parsed, SvgSize::Size(size(width, width))).ok()?;
    let at = drawn.size(0);
    let mut picture = image::RgbaImage::from_raw(at.width.0 as u32, at.height.0 as u32, drawn.as_bytes(0)?.to_vec())?;
    for pixel in picture.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(picture)
}

/// Opens the sheet over `source` for a project.
pub fn open(project: &str, source: Source, acts: &Acts, prefs: Option<(&str, &str)>, cx: &mut App) {
    let (light, dark) = prefs.unwrap_or_default();
    let ground = |side, picked| {
        let entry = theme_for(side, picked);
        Ground { word: entry.word.into(), palette: entry.palette() }
    };
    let grounds = [ground(Side::Light, light), ground(Side::Dark, dark)];
    let sheet = cx.new(|cx| {
        cx.on_release(|s: &mut Sheet, cx| {
            if let Some((_, image)) = s.fitted.take() {
                cx.drop_image(image, None);
            }
        })
        .detach();
        Sheet {
            project: project.to_owned(),
            acts: acts.clone(),
            grounds,
            fitted: None,
            refusal: None,
            fitting: false,
            saving: false,
            turn: 0,
        }
    });
    Sheet::take(&sheet, source, cx);
    acts.open_dialog(crate::screens::settings::page::DialogBuild::dialog(move |_, window, cx| draw(&sheet, window, cx)), cx);
}

/// Choose an image: the system's file chooser, then the sheet over the file picked.
pub fn choose(project: &str, acts: &Acts, prefs: Option<(String, String)>, cx: &mut App) {
    let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
    let (project, acts) = (project.to_owned(), acts.clone());
    cx.spawn(async move |cx: &mut AsyncApp| {
        let path = match picked.await {
            Ok(Ok(Some(paths))) => paths.into_iter().next(),
            Ok(Err(failed)) => {
                cx.update(|cx| acts.failed(format!("The file chooser did not open: {failed}"), cx));
                None
            }
            _ => None,
        };
        let Some(path) = path else { return };
        cx.update(|cx| open(&project, Source::Path(path), &acts, prefs.as_ref().map(|(l, d)| (l.as_str(), d.as_str())), cx));
    })
    .detach();
}

/// One theme's ground in its own sidebar fill, the image on it at 12, 16 and 40 px on one baseline, its name under.
fn ground(ground: &Ground, image: Option<&Arc<RenderImage>>, fitting: bool, ink: Hsla, cx: &App) -> AnyElement {
    let p = &ground.palette;
    let shown: Vec<AnyElement> = match image {
        Some(image) if !fitting => [12., 16., 40.]
            .into_iter()
            .map(|at| {
                div()
                    .debug_selector(|| "project-icon-preview".into())
                    .child(image_mark(image.clone(), px(at), p.foreground))
                    .into_any_element()
            })
            .collect(),
        _ => Vec::new(),
    };
    v_flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap_2()
        .child(
            h_flex()
                .debug_selector(|| "project-icon-ground".into())
                .h(px(80.))
                .w_full()
                .items_end()
                .justify_center()
                .gap_4()
                .px_4()
                .pb_5()
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.sidebar)
                .text_color(p.sidebar_foreground)
                .when(fitting, |el| el.child(div().mb_1().self_center().text_color(p.muted_foreground).child(spinner(cx))))
                .children(shown),
        )
        .child(div().text_size(px(12.)).line_height(px(16.)).text_color(ink).child(ground.word.clone()))
        .into_any_element()
}

fn draw(sheet: &Entity<Sheet>, window: &mut Window, cx: &mut App) -> AnyElement {
    let s = sheet.read(cx);
    let ink = cx.palette().muted_foreground;
    let image = s.fitted.as_ref().map(|(_, image)| image);
    let grounds = h_flex().gap_3().children(s.grounds.iter().map(|g| ground(g, image, s.fitting, ink, cx)));
    let refusal = match &s.refusal {
        Some(refused) => {
            let said = Refusal::new().said(refused.said.clone());
            if refused.fix.is_empty() {
                said
            } else {
                said.fix(refused.fix.clone())
            }
        }
        None => Refusal::new().quiet(IMAGE_NOTE),
    };
    let cancel = {
        let acts = s.acts.clone();
        Button::outline("project-icon-cancel", CANCEL).on_click(move |_, _, cx| acts.close_dialog(cx))
    };
    let held = s.fitted.is_none() || s.saving;
    let keep = {
        let sheet = sheet.clone();
        Button::primary("project-icon-use", USE_IMAGE).held(held).on_click(move |_, _, cx| Sheet::save(&sheet, cx))
    };
    let dialog = AlertDialog::new(IMAGE_TITLE)
        .layered()
        .on_outside(layer::dismiss)
        .sentence(IMAGE_LINE)
        .child(v_flex().gap_3().pt_1().child(grounds).child(refusal))
        .act(cancel)
        .act(keep);
    let (dropped, pasted) = (sheet.downgrade(), sheet.downgrade());
    let focus = window.use_keyed_state("project-icon-sheet-focus", cx, |_, cx| cx.focus_handle());
    let focus = focus.read(cx).clone();
    // Only while it stands: the layer draws it on for its fade out once closed, and focus has gone back by then.
    let standing = layer::view(DIALOG, cx).is_some_and(|view| view.entity_id() == window.current_view());
    if standing && !focus.contains_focused(window, cx) {
        window.focus(&focus, cx);
    }
    div()
        .id("project-icon-sheet")
        .debug_selector(|| "project-icon-sheet".into())
        .track_focus(&focus)
        .on_drop(move |paths: &ExternalPaths, _, cx| replace(&dropped, Source::of_paths(paths), cx))
        .on_key_down(move |event, _, cx| {
            if is_paste(event) {
                replace(&pasted, Source::of_clipboard(cx), cx);
                cx.stop_propagation();
            }
        })
        .child(dialog)
        .into_any_element()
}

fn replace(sheet: &WeakEntity<Sheet>, source: Option<Source>, cx: &mut App) {
    if let (Some(sheet), Some(source)) = (sheet.upgrade(), source) {
        Sheet::take(&sheet, source, cx);
    }
}
