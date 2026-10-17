// SPDX-License-Identifier: AGPL-3.0-only
//! A computer's icon, after `ComputerGlyph.tsx`: the person's pick where they made one, else what the computer is; a
//! cloud as its provider's mark. The two Mac shapes and the marks are not lucide's, so they are drawn from their own
//! svg in the glyph's ink.

use std::sync::Arc;

use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{img, prelude::*, AnyElement, App, Hsla, Image, ImageFormat};
use wsp_host_client::protocol::{PlaceView, PlaceViewMac, Preferences, PreferencesComputerLookValueIcon as Pick};

use super::words::{is_here, is_provider};
use crate::ui::{size, ActivePalette, Glyph};

const MAC_MINI: &str = r#"<rect x="3" y="8" width="18" height="7" rx="2"/><path d="M7 18h10"/>"#;
const MAC_STUDIO: &str = r#"<rect x="4" y="6" width="16" height="12" rx="2"/><path d="M8 14h.01"/><path d="M11 14h2"/>"#;
/// Boat's mark, from boat.dev/icon.svg, filled in the ink.
const BOAT: (&str, &str) = (
    "0 0 35.5 35.5",
    r#"<path transform="translate(4.6 0)" d="M20.9863 0C20.9693 0.0677594 16.9743 16.0113 18.71 19.7461C20.4455 23.4795 26.2361 27.1194 26.2637 27.1367C26.248 27.1345 20.1121 26.2731 16.2959 27.6826V22.666H11.2637V27.6982H16.2549C16.0728 27.7666 15.895 27.8378 15.7246 27.917C11.9935 29.6513 8.3558 35.436 8.33398 35.4707C8.34016 35.4268 9.28754 28.6619 7.55371 24.9316C5.81721 21.1961 0.0210234 17.5542 0 17.541C0.024656 17.5445 6.80349 18.4971 10.5391 16.7607C14.2749 15.0235 20.9628 0.0527921 20.9863 0Z"/>"#,
);
/// Solari's mark, traced from its favicon at the glyphs' 2 px stroke.
const SOLARI: (&str, &str) = (
    "0 0 24 24",
    r#"<path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" d="M19.97 11.27A8 8 0 0 1 9.07 19.45M4.97 15.82A8 8 0 0 1 17.63 6.32M2 19C9 18.5 16 13.5 22 5"/>"#,
);

fn picked(place: &PlaceView, preferences: Option<&Preferences>) -> Option<Pick> {
    preferences.and_then(|p| p.computer_look.get(&place.id)).map(|look| look.icon)
}

/// `defaultComputerIcon`.
pub fn default_icon(place: &PlaceView) -> Pick {
    if is_provider(place) {
        return Pick::Cloud;
    }
    match &place.mac {
        Some(PlaceViewMac::Macbook) => Pick::Laptop,
        Some(PlaceViewMac::Imac) => Pick::Desktop,
        Some(PlaceViewMac::MacMini) => Pick::MacMini,
        Some(PlaceViewMac::MacStudio) => Pick::MacStudio,
        Some(PlaceViewMac::MacPro) => Pick::Tower,
        None if is_here(place) => Pick::Desktop,
        None => Pick::Server,
    }
}

/// An svg in one ink, as lucide draws its glyphs or as a mark fills its paths.
fn drawn(view_box: &str, body: &str, stroked: bool, ink: Hsla) -> AnyElement {
    stroked_svg(view_box, body, stroked.then_some(2.), ink, size::GLYPH)
}

/// An svg drawn in `ink` at `side`: stroked at the width given, or filled where none is.
pub fn stroked_svg(view_box: &str, body: &str, stroke: Option<f32>, ink: Hsla, side: gpui_kit::Pixels) -> AnyElement {
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

/// `ComputerGlyph` at the glyph's size in the foreground at 80%, as a frame holds it.
pub fn computer_glyph(place: &PlaceView, preferences: Option<&Preferences>, cx: &App) -> AnyElement {
    glyph_in(place, preferences, cx.palette().foreground.opacity(0.8))
}

/// `ComputerGlyph` in the ink its row gives it.
pub fn glyph_in(place: &PlaceView, preferences: Option<&Preferences>, ink: Hsla) -> AnyElement {
    let pick = picked(place, preferences);
    if pick.is_none() && is_provider(place) {
        match place.name.as_str() {
            "box" => return drawn(BOAT.0, BOAT.1, false, ink),
            "solari" => return drawn(SOLARI.0, SOLARI.1, true, ink),
            _ => {}
        }
    }
    let glyph = match pick.unwrap_or_else(|| default_icon(place)) {
        Pick::Laptop => Glyph::Laptop,
        Pick::Desktop => Glyph::Monitor,
        Pick::MacMini => return drawn("0 0 24 24", MAC_MINI, true, ink),
        Pick::MacStudio => return drawn("0 0 24 24", MAC_STUDIO, true, ink),
        Pick::Tower => Glyph::PcCase,
        Pick::Server => Glyph::Server,
        Pick::Cloud => Glyph::Cloud,
        Pick::Cpu => Glyph::Cpu,
        Pick::Drive => Glyph::HardDrive,
        Pick::Container => Glyph::Box,
        Pick::Home => Glyph::House,
    };
    Icon::new(glyph).with_size(size::GLYPH).flex_none().text_color(ink).into_any_element()
}
