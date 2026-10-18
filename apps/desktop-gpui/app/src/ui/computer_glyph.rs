// SPDX-License-Identifier: AGPL-3.0-only
//! A computer's icon, after `ComputerGlyph.tsx`: the person's pick where they made one, else what the computer is; a
//! cloud as its provider's mark, read off the catalog. The two Mac shapes and the marks are not lucide's, so they are
//! drawn from their own svg in the glyph's ink. Every page that draws a computer draws it through here.

use std::sync::Arc;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{img, prelude::*, AnyElement, App, Hsla, Image, ImageFormat, Pixels};
use wsp_host_client::protocol::{PlaceView, PlaceViewKind, PlaceViewMac, Preferences, PreferencesComputerLookValueIcon as Pick};

use super::icons::Glyph;
use super::size;
use super::theme::ActivePalette;
use crate::screens::settings::words::HERE;

const MAC_MINI: &str = r#"<rect x="3" y="8" width="18" height="7" rx="2"/><path d="M7 18h10"/>"#;
const MAC_STUDIO: &str = r#"<rect x="4" y="6" width="16" height="12" rx="2"/><path d="M8 14h.01"/><path d="M11 14h2"/>"#;

/// The catalog's provider marks (`PROVIDER_MARKS` in packages/catalog/src/marks), read off their modules.
const PROVIDER_MARKS: [&str; 2] = [
    include_str!("../../../../../packages/catalog/src/marks/solari.ts"),
    include_str!("../../../../../packages/catalog/src/marks/boat.ts"),
];

/// What a module writes after `key`, up to `close`.
fn field<'a>(src: &'a str, key: &str, close: char) -> Option<&'a str> {
    let at = src.find(key)? + key.len();
    Some(&src[at..at + src[at..].find(close)?])
}

/// `providerMark`: a cloud's mark by its provider id, its whole svg; none where it draws the cloud.
pub fn provider_mark(provider: &str) -> Option<&'static str> {
    PROVIDER_MARKS.iter().find(|src| field(src, "provider: \"", '"') == Some(provider)).and_then(|src| field(src, "svg: `", '`'))
}

/// What a computer is drawn as: a lucide glyph, or an svg of its own, stroked as lucide draws or filled as a mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputerIcon {
    Lucide(Glyph),
    Svg {
        view_box: &'static str,
        body: &'static str,
        stroked: bool,
    },
    /// A provider's mark from the catalog, its own svg whole, in the ink.
    Mark(&'static str),
}

/// `defaultComputerIcon`: a cloud, the Mac it says it is, a desktop for the computer the host runs on, else a server.
pub fn default_icon(place: &PlaceView) -> Pick {
    if matches!(place.kind, PlaceViewKind::Provider) {
        return Pick::Cloud;
    }
    match &place.mac {
        Some(PlaceViewMac::Macbook) => Pick::Laptop,
        Some(PlaceViewMac::Imac) => Pick::Desktop,
        Some(PlaceViewMac::MacMini) => Pick::MacMini,
        Some(PlaceViewMac::MacStudio) => Pick::MacStudio,
        Some(PlaceViewMac::MacPro) => Pick::Tower,
        None | Some(PlaceViewMac::Unknown) if place.id == HERE => Pick::Desktop,
        None | Some(PlaceViewMac::Unknown) => Pick::Server,
    }
}

impl From<Glyph> for ComputerIcon {
    fn from(glyph: Glyph) -> Self {
        ComputerIcon::Lucide(glyph)
    }
}

impl ComputerIcon {
    /// `ComputerGlyph`'s pick: the person's icon, else a cloud's provider mark, else `default_icon`.
    pub fn of(place: &PlaceView, preferences: Option<&Preferences>) -> ComputerIcon {
        let picked = preferences.and_then(|p| p.computer_look.get(&place.id)).map(|look| look.icon);
        if let Some(mark) =
            picked.is_none().then_some(place).filter(|p| matches!(p.kind, PlaceViewKind::Provider)).and_then(|p| provider_mark(&p.name))
        {
            return ComputerIcon::Mark(mark);
        }
        let lucide = |g| ComputerIcon::Lucide(g);
        match picked.unwrap_or_else(|| default_icon(place)) {
            Pick::Laptop => lucide(Glyph::Laptop),
            Pick::Desktop => lucide(Glyph::Monitor),
            Pick::MacMini => ComputerIcon::Svg { view_box: "0 0 24 24", body: MAC_MINI, stroked: true },
            Pick::MacStudio => ComputerIcon::Svg { view_box: "0 0 24 24", body: MAC_STUDIO, stroked: true },
            Pick::Tower => lucide(Glyph::PcCase),
            Pick::Server => lucide(Glyph::Server),
            Pick::Cloud => lucide(Glyph::Cloud),
            Pick::Cpu => lucide(Glyph::Cpu),
            Pick::Drive => lucide(Glyph::HardDrive),
            Pick::Container => lucide(Glyph::Box),
            Pick::Home => lucide(Glyph::House),
            Pick::Unknown => ComputerIcon::of(place, None),
        }
    }

    /// The icon `side` square in `ink`.
    pub fn draw(self, ink: Hsla, side: Pixels) -> AnyElement {
        match self {
            ComputerIcon::Lucide(glyph) => Icon::new(glyph).with_size(side).flex_none().text_color(ink).into_any_element(),
            ComputerIcon::Svg { view_box, body, stroked } => stroked_svg(view_box, body, stroked.then_some(2.), ink, side),
            ComputerIcon::Mark(svg) => mark_svg(svg, ink, side),
        }
    }
}

/// `ComputerGlyph` with the `GLYPH` class, as a frame or a head row holds it: 16 px in the foreground at 80%.
pub fn computer_glyph(place: &PlaceView, preferences: Option<&Preferences>, cx: &App) -> AnyElement {
    ComputerIcon::of(place, preferences).draw(cx.palette().foreground.opacity(0.8), size::GLYPH)
}

/// A whole svg in `ink` at `side`: filled in it, and any part drawn in `currentColor` drawn in it, at its alpha.
pub fn mark_svg(svg: &str, ink: Hsla, side: Pixels) -> AnyElement {
    let rgba = ink.to_rgb();
    let hex = format!("#{:02x}{:02x}{:02x}", (rgba.r * 255.).round() as u8, (rgba.g * 255.).round() as u8, (rgba.b * 255.).round() as u8);
    let paint = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" fill="{hex}" fill-opacity="{a}" stroke-opacity="{a}" "#, a = rgba.a);
    let svg = svg.replacen("<svg ", &paint, 1).replace("currentColor", &hex);
    img(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))).size(side).flex_none().into_any_element()
}

/// An svg drawn in `ink` at `side`: stroked at the width given, or filled where none is.
pub fn stroked_svg(view_box: &str, body: &str, stroke: Option<f32>, ink: Hsla, side: Pixels) -> AnyElement {
    let rgba = ink.to_rgb();
    let hex = format!("#{:02x}{:02x}{:02x}", (rgba.r * 255.).round() as u8, (rgba.g * 255.).round() as u8, (rgba.b * 255.).round() as u8);
    let paint = if let Some(width) = stroke {
        format!(
            r#"fill="none" stroke="{hex}" stroke-opacity="{}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round""#,
            rgba.a
        )
    } else {
        format!(r#"fill="{hex}" fill-opacity="{}""#, rgba.a)
    };
    let body = body.replace("currentColor", &hex);
    let svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" preserveAspectRatio="xMidYMid" {paint}>{body}</svg>"#);
    img(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))).size(side).flex_none().into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn place(v: serde_json::Value) -> PlaceView {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_mac_mini_and_a_cloud_draw_one_way_everywhere() {
        let mini = place(json!({ "id": "here", "kind": "computer", "name": "mini", "default": true, "mac": "mac-mini" }));
        assert!(matches!(ComputerIcon::of(&mini, None), ComputerIcon::Svg { body: MAC_MINI, .. }));
        let solari = place(json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false }));
        assert!(matches!(ComputerIcon::of(&solari, None), ComputerIcon::Mark(svg) if svg.contains("M19.97 11.27")));
        let boat = place(json!({ "id": "box", "kind": "provider", "name": "box", "default": false }));
        assert!(matches!(ComputerIcon::of(&boat, None), ComputerIcon::Mark(svg) if svg.starts_with("<svg viewBox=\"0 0 35.5 35.5\">")));
        let other = place(json!({ "id": "x", "kind": "provider", "name": "x", "default": false }));
        assert_eq!(ComputerIcon::of(&other, None), ComputerIcon::Lucide(Glyph::Cloud));
        let joined = place(json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false }));
        assert_eq!(ComputerIcon::of(&joined, None), ComputerIcon::Lucide(Glyph::Server));
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn the_marks_are_the_catalogs() {
        let read = crate::screens::settings::words::tests::catalog_read();
        let theirs = read["providerMarks"].as_array().unwrap();
        assert_eq!(theirs.len(), PROVIDER_MARKS.len(), "a provider mark the catalog lists is not read here");
        for pair in theirs {
            assert_eq!(provider_mark(pair[0].as_str().unwrap()), pair[1].as_str(), "{}", pair[0]);
        }
    }
}
