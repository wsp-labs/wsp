// SPDX-License-Identifier: AGPL-3.0-only
//! A computer's icon, after `ComputerGlyph.tsx`: the person's pick where they made one, else what the computer is; a
//! cloud as its provider's mark. The two Mac shapes and the marks are not lucide's, so they are drawn from their own
//! svg in the glyph's ink. Every page that draws a computer draws it through here.

use std::sync::Arc;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{img, prelude::*, AnyElement, App, Hsla, Image, ImageFormat, Pixels};
use wsp_host_client::protocol::{PlaceView, PlaceViewKind, PlaceViewMac, Preferences, PreferencesComputerLookValueIcon as Pick};

use super::icons::Glyph;
use super::size;
use super::theme::ActivePalette;

/// `HERE_PLACE_ID`.
const HERE: &str = "here";

const MAC_MINI: &str = r#"<rect x="3" y="8" width="18" height="7" rx="2"/><path d="M7 18h10"/>"#;
const MAC_STUDIO: &str = r#"<rect x="4" y="6" width="16" height="12" rx="2"/><path d="M8 14h.01"/><path d="M11 14h2"/>"#;
/// Boat's mark, from boat.dev/icon.svg, filled in the ink.
const BOAT: &str = r#"<path transform="translate(4.6 0)" d="M20.9863 0C20.9693 0.0677594 16.9743 16.0113 18.71 19.7461C20.4455 23.4795 26.2361 27.1194 26.2637 27.1367C26.248 27.1345 20.1121 26.2731 16.2959 27.6826V22.666H11.2637V27.6982H16.2549C16.0728 27.7666 15.895 27.8378 15.7246 27.917C11.9935 29.6513 8.3558 35.436 8.33398 35.4707C8.34016 35.4268 9.28754 28.6619 7.55371 24.9316C5.81721 21.1961 0.0210234 17.5542 0 17.541C0.024656 17.5445 6.80349 18.4971 10.5391 16.7607C14.2749 15.0235 20.9628 0.0527921 20.9863 0Z"/>"#;
/// Solari's mark, traced from its favicon at the glyphs' 2 px stroke.
const SOLARI: &str = r#"<path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M19.97 11.27A8 8 0 0 1 9.07 19.45M4.97 15.82A8 8 0 0 1 17.63 6.32M2 19C9 18.5 16 13.5 22 5"/>"#;

/// What a computer is drawn as: a lucide glyph, or an svg of its own, stroked as lucide draws or filled as a mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputerIcon {
    Lucide(Glyph),
    Svg { view_box: &'static str, body: &'static str, stroked: bool },
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
        if picked.is_none() && matches!(place.kind, PlaceViewKind::Provider) {
            match place.name.as_str() {
                "box" => return ComputerIcon::Svg { view_box: "0 0 35.5 35.5", body: BOAT, stroked: false },
                "solari" => return ComputerIcon::Svg { view_box: "0 0 24 24", body: SOLARI, stroked: true },
                _ => {}
            }
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
        }
    }
}

/// `ComputerGlyph` with the `GLYPH` class, as a frame or a head row holds it: 16 px in the foreground at 80%.
pub fn computer_glyph(place: &PlaceView, preferences: Option<&Preferences>, cx: &App) -> AnyElement {
    ComputerIcon::of(place, preferences).draw(cx.palette().foreground.opacity(0.8), size::GLYPH)
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
        assert!(matches!(ComputerIcon::of(&solari, None), ComputerIcon::Svg { body: SOLARI, .. }));
        let other = place(json!({ "id": "x", "kind": "provider", "name": "x", "default": false }));
        assert_eq!(ComputerIcon::of(&other, None), ComputerIcon::Lucide(Glyph::Cloud));
        let joined = place(json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false }));
        assert_eq!(ComputerIcon::of(&joined, None), ComputerIcon::Lucide(Glyph::Server));
    }
}
