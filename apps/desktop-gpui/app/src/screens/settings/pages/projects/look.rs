// SPDX-License-Identifier: AGPL-3.0-only
//! How a project or a recipe is drawn (`projects/look.tsx`): the image the person chose, else the glyph they picked
//! from the projects' set in its hue, and the two selects that pick them (`LookPicker.tsx`), saved on the pick.

use std::sync::Arc;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{div, img, prelude::*, px, AnyElement, App, Entity, Hsla, ObjectFit, Pixels, RenderImage, SharedString, Window};

use crate::screens::settings::page::Ctx;
use crate::ui::{ActivePalette, Glyph, SelectItem, SelectState};

/// `PROJECT_GLYPHS`, in `ProjectIcon`'s order.
pub const ICONS: [(&str, Glyph); 20] = [
    ("folder", Glyph::Folder),
    ("code", Glyph::Code),
    ("terminal", Glyph::Terminal),
    ("globe", Glyph::Globe),
    ("rocket", Glyph::Rocket),
    ("box", Glyph::Box),
    ("database", Glyph::Database),
    ("server", Glyph::Server),
    ("cpu", Glyph::Cpu),
    ("zap", Glyph::Zap),
    ("flame", Glyph::Flame),
    ("leaf", Glyph::Leaf),
    ("star", Glyph::Star),
    ("heart", Glyph::Heart),
    ("book", Glyph::Book),
    ("music", Glyph::Music),
    ("camera", Glyph::Camera),
    ("gamepad", Glyph::Gamepad2),
    ("shield", Glyph::Shield),
    ("wrench", Glyph::Wrench),
];

/// `ProjectHue`'s options.
pub const HUES: [&str; 9] = ["neutral", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink"];

/// The glyph for an icon's word, the folder for one this app does not know.
pub fn glyph_of(icon: &str) -> Glyph {
    ICONS.iter().find(|(word, _)| *word == icon).map_or(Glyph::Folder, |(_, g)| *g)
}

/// A word as the selects say it: its first letter up.
pub fn word(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// `PROJECT_HUES`' text: a hue the person picked, else the row's own ink.
pub fn hue_ink(hue: &str, cx: &App) -> Hsla {
    let p = cx.palette();
    match hue {
        "red" => p.hue_red,
        "orange" => p.hue_orange,
        "amber" => p.hue_amber,
        "green" => p.hue_green,
        "teal" => p.hue_teal,
        "blue" => p.hue_blue,
        "violet" => p.hue_violet,
        "pink" => p.hue_pink,
        _ => p.foreground,
    }
}

/// A project's glyph at 16 px in its hue.
pub fn mark(icon: &str, hue: &str, cx: &App) -> Icon {
    Icon::new(glyph_of(icon)).with_size(px(16.)).flex_none().text_color(hue_ink(hue, cx))
}

/// `ProjectIconImage`: the image a project wears in the glyph's own box, clipped to an app icon's 22% corners on a
/// quiet ground of `ink`.
pub fn image_mark(image: Arc<RenderImage>, size: Pixels, ink: Hsla) -> AnyElement {
    img(image).flex_none().size(size).rounded(size * 0.22).bg(ink.opacity(0.04)).object_fit(ObjectFit::Contain).into_any_element()
}

/// The image a project wears, once the host's copy is here; held for this frame.
pub fn project_image(ctx: &Ctx, id: &str) -> Option<Arc<RenderImage>> {
    let hash = ctx.data.preferences()?.project_icon.get(id)?;
    ctx.data.project_images.draw(hash.as_str())
}

/// `ProjectGlyph`: a project's mark wherever it is named, its image where it wears one and it has landed, else its
/// glyph in its hue, or in `ink` where no hue was picked.
pub fn project_mark(ctx: &Ctx, id: &str, ink: Option<Hsla>, cx: &App) -> AnyElement {
    if let Some(image) = project_image(ctx, id) {
        return image_mark(image, px(16.), cx.palette().foreground);
    }
    let hue = project_hue(ctx, id);
    let mark = mark(&project_icon(ctx, id), &hue, cx);
    match ink.filter(|_| hue == "neutral") {
        Some(ink) => mark.text_color(ink).into_any_element(),
        None => mark.into_any_element(),
    }
}

/// The icon a project wears, the folder until one is picked.
pub fn project_icon(ctx: &Ctx, id: &str) -> String {
    ctx.data
        .preferences()
        .and_then(|p| p.project_look.get(id))
        .and_then(|look| look.icon.as_ref())
        .map_or_else(|| "folder".to_owned(), |icon| icon.to_string())
}

/// The hue a project wears, neutral until one is picked.
pub fn project_hue(ctx: &Ctx, id: &str) -> String {
    ctx.data
        .preferences()
        .and_then(|p| p.project_look.get(id))
        .and_then(|look| look.hue.as_ref())
        .map_or_else(|| "neutral".to_owned(), |hue| hue.to_string())
}

/// A select kept while drawn, and the items it was last given, so new items reach it only when they change.
struct Held {
    state: Entity<SelectState<String>>,
    signature: String,
}

/// A select under `key` holding `items`, on `value`; `signature` names the items, so they are laid in again only when
/// it moves. A pick is the caller's `Select::on_change`.
pub fn select_state(
    key: SharedString,
    items: impl Fn() -> Vec<SelectItem<String>>,
    signature: String,
    value: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<String>> {
    let value = value.to_owned();
    let held = window.use_keyed_state(key, cx, |_, cx| {
        let state = cx.new(|_| SelectState::new(items(), Some(&value)));
        Held { state, signature: signature.clone() }
    });
    let (state, moved) = {
        let held = held.read(cx);
        (held.state.clone(), held.signature != signature)
    };
    if moved {
        state.update(cx, |s, cx| s.set_items(items(), cx));
        held.update(cx, |h, _| h.signature = signature);
    }
    if state.read(cx).value() != Some(&value) {
        state.update(cx, |s, cx| s.set_value(Some(&value), cx));
    }
    state
}

/// `IconSelect`'s items: every glyph in the hue the project wears, by its word.
pub fn icon_items(hue: &str) -> Vec<SelectItem<String>> {
    ICONS
        .iter()
        .map(|(name, _)| {
            let (icon, hue) = (name.to_string(), hue.to_owned());
            SelectItem::new(name.to_string(), word(name)).mark(move |cx| mark(&icon, &hue, cx).into_any_element())
        })
        .collect()
}

/// `HueSelect`'s items: each hue by its word after its 12 px swatch; neutral's swatch is the muted ink.
pub fn hue_items() -> Vec<SelectItem<String>> {
    HUES.iter()
        .map(|hue| {
            let ink = hue.to_string();
            SelectItem::new(hue.to_string(), word(hue)).mark(move |cx| {
                let fill = if ink == "neutral" { cx.palette().muted_foreground } else { hue_ink(&ink, cx) };
                div().size(px(12.)).flex_none().rounded_full().bg(fill).into_any_element()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ProjectIcon`'s words in order, each with the lucide drawing `PROJECT_GLYPHS` gives it.
    const WEB: [(&str, &str); 20] = [
        ("folder", "folder"),
        ("code", "code"),
        ("terminal", "terminal"),
        ("globe", "globe"),
        ("rocket", "rocket"),
        ("box", "box"),
        ("database", "database"),
        ("server", "server"),
        ("cpu", "cpu"),
        ("zap", "zap"),
        ("flame", "flame"),
        ("leaf", "leaf"),
        ("star", "star"),
        ("heart", "heart"),
        ("book", "book"),
        ("music", "music"),
        ("camera", "camera"),
        ("gamepad", "gamepad-2"),
        ("shield", "shield"),
        ("wrench", "wrench"),
    ];

    #[test]
    fn every_icon_has_its_glyph_and_its_word() {
        assert_eq!(ICONS.map(|(word, _)| word), WEB.map(|(word, _)| word), "the select lists ProjectIcon's words in its order");
        for (icon, lucide) in WEB {
            assert_eq!(glyph_of(icon).path().as_ref(), format!("icons/{lucide}.svg"), "{icon} draws lucide's {lucide}");
        }
        assert_eq!(glyph_of("nonsense"), Glyph::Folder);
        assert_eq!(word("gamepad"), "Gamepad");
        assert_eq!(word(""), "");
    }
}
