// SPDX-License-Identifier: AGPL-3.0-only
//! What both tabs draw with: the locked surface (`data-locked`'s 14 px radius) where the web draws one bare, the page's
//! one way of naming a thing, its glyphs, the quiet line and the hover's skin.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, img, prelude::*, px, relative, AnyElement, App, Div, FontWeight, Hsla, Pixels, SharedString};

use wsp_host_client::protocol::{PlaceView, Preferences};

use super::model::Split;
use crate::screens::settings::pages::projects::look;
use crate::ui::{catalog, size, ActivePalette, ComputerIcon, Glyph};

/// `rounded-xl` off `--radius`, which `data-locked` gives the card.
pub const CARD_RADIUS: Pixels = px(14.);
/// `CARD_PAD`: the inset every card on this page keeps its content at.
pub const CARD_PAD: Pixels = px(20.);
/// The `gap-10` between the page's blocks.
pub const BLOCK_GAP: Pixels = px(40.);

/// Text at a size with the 1.5 line height a size without its own leading takes.
pub fn sized(el: Div, size: f32) -> Div {
    el.text_size(px(size)).line_height(px(size * 1.5))
}

/// `CARD_SURFACE` under `data-locked`.
pub fn surface(cx: &App) -> Div {
    let p = cx.palette();
    v_flex().rounded(CARD_RADIUS).border_1().border_color(p.border.opacity(0.6)).bg(p.card.opacity(0.4))
}

/// The faint rule between rows and under a table.
pub fn rule(cx: &App) -> Hsla {
    cx.palette().border.opacity(0.5)
}

/// `QUIET`: 13 px on a 20 px line, muted.
pub fn quiet(text: impl Into<SharedString>, cx: &App) -> Div {
    div().text_size(px(13.)).line_height(px(20.)).text_color(cx.palette().muted_foreground).child(text.into())
}

/// `GLYPH_FRAME` around any glyph.
pub fn frame(glyph: impl IntoElement, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.foreground.opacity(0.04))
        .child(glyph)
}

/// The agent's mark where the catalog draws one; a mark with no inks of its own takes `around`.
pub fn agent_glyph(agent: &str, size: Pixels, around: Hsla, cx: &App) -> Option<AnyElement> {
    catalog::image(agent, cx.theme().is_dark(), around).map(|image| img(image).size(size).flex_none().into_any_element())
}

/// What the page knows of the host's computers and projects, for the glyph each wears.
#[derive(Clone, Copy)]
pub struct Looks<'a> {
    pub projects: &'a [String],
    pub places: &'a [PlaceView],
    pub preferences: Option<&'a Preferences>,
}

impl Looks<'_> {
    /// The icon a computer the host lists wears, its pick or what it is; none for one it does not list.
    pub fn computer(&self, id: &str) -> Option<ComputerIcon> {
        self.places.iter().find(|place| place.id == id).map(|place| ComputerIcon::of(place, self.preferences))
    }

    /// The icon and the hue a project picked, the folder and neutral until it picks.
    pub fn project(&self, id: &str) -> (String, String) {
        let picked = self.preferences.and_then(|p| p.project_look.get(id));
        let icon = picked.and_then(|look| look.icon.as_ref()).map_or_else(|| "folder".to_owned(), |icon| icon.to_string());
        let hue = picked.and_then(|look| look.hue.as_ref()).map_or_else(|| "neutral".to_owned(), |hue| hue.to_string());
        (icon, hue)
    }
}

/// The glyph a split row leads with: its agent's mark, the computer's own glyph, the project's look, the source, or the
/// split's own stand-in where the value has none, so every row's name starts at one edge.
pub fn split_glyph(split: Split, key: &str, agent: Option<&str>, looks: Looks, cx: &App) -> Option<AnyElement> {
    let p = cx.palette();
    if let Some(mark) = agent.and_then(|agent| agent_glyph(agent, px(16.), p.foreground, cx)) {
        return Some(mark);
    }
    let icon = |glyph: Glyph, color: Hsla| Some(Icon::new(glyph).with_size(px(16.)).text_color(color).into_any_element());
    if let Some(computer) = looks.computer(key).filter(|_| split == Split::Computer) {
        return Some(computer.draw(p.foreground.opacity(0.8), size::GLYPH));
    }
    match split {
        Split::Project if looks.projects.iter().any(|id| id == key) => {
            let (glyph, hue) = looks.project(key);
            Some(look::mark(&glyph, &hue, cx).into_any_element())
        }
        Split::Source if key == "log" => icon(Glyph::SquareTerminal, p.muted_foreground),
        Split::Source => icon(Glyph::MessagesSquare, p.muted_foreground),
        Split::Project => icon(Glyph::CircleDashed, p.muted_foreground),
        Split::Computer => icon(Glyph::Monitor, p.muted_foreground),
        Split::Account => icon(Glyph::UserRound, p.muted_foreground),
        Split::Model | Split::Agent => icon(Glyph::Box, p.muted_foreground),
    }
}

/// `Identity`: the mark in its frame, the title, and under it a line or a figure.
pub fn identity(
    mark: Option<AnyElement>,
    title: impl Into<SharedString>,
    line: Option<String>,
    under: Option<AnyElement>,
    cx: &App,
) -> Div {
    let p = cx.palette();
    h_flex().flex_1().min_w_0().gap_3().children(mark.map(|mark| frame(mark, cx))).child(
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(
                div()
                    .debug_selector(|| "usage-title".into())
                    .truncate()
                    .text_size(px(15.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(title.into()),
            )
            .children(
                line.filter(|l| !l.is_empty())
                    .map(|line| div().truncate().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(line)),
            )
            .children(under),
    )
}

/// A bar a person reads as a share: its track and its fill.
pub fn share_bar(share: f64, ink: Hsla, cx: &App) -> Div {
    let p = cx.palette();
    div()
        .mt_1()
        .h_1()
        .w_full()
        .max_w(px(256.))
        .rounded_full()
        .bg(p.foreground.opacity(0.08))
        .child(div().h_full().rounded_full().bg(ink).w(relative((share.max(0.01)) as f32)))
}

/// `TooltipPopup`'s skin: the popover with its edge and shadow, 8 px round, `py-1 px-2`.
pub fn tip_skin(cx: &App) -> Div {
    let p = cx.palette();
    v_flex()
        .flex_none()
        .rounded(size::BUTTON_RADIUS)
        .border_1()
        .border_color(p.border)
        .bg(p.popover)
        .shadow(p.tooltip_shadow.clone())
        .px_2()
        .py_1()
        .whitespace_nowrap()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn computers_and_projects_wear_the_looks_picked_for_them() {
        let places: Vec<PlaceView> = serde_json::from_value(json!([
            { "id": "here", "kind": "computer", "name": "mini", "default": true },
            { "id": "box1", "kind": "computer", "name": "box1", "default": false },
        ]))
        .unwrap();
        let preferences: Preferences = serde_json::from_value(json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app",
            "terminalZoom": {}, "access": {}, "projectOrder": [], "labs": false,
            "computerLook": { "here": { "icon": "laptop" } },
            "projectLook": { "p1": { "icon": "rocket", "hue": "blue" }, "p2": { "hue": "neutral" } } }))
        .unwrap();
        let projects = ["p1".to_owned(), "p2".to_owned()];
        let looks = Looks { projects: &projects, places: &places, preferences: Some(&preferences) };
        assert_eq!(looks.computer("here"), Some(ComputerIcon::Lucide(Glyph::Laptop)), "the computer's own pick");
        assert_eq!(looks.computer("box1"), Some(ComputerIcon::Lucide(Glyph::Server)), "what a joined computer is");
        assert_eq!(looks.computer("studio"), None, "a computer the host does not list takes the split's stand-in");
        assert_eq!(looks.project("p1"), ("rocket".to_owned(), "blue".to_owned()));
        assert_eq!(looks.project("p2"), ("folder".to_owned(), "neutral".to_owned()));
        assert_eq!(looks.project("p3"), ("folder".to_owned(), "neutral".to_owned()));
    }
}
