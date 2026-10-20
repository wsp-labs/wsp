// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's frosted glass, the web app's glass-backdrop (apps/web/src/index.css and ComposerSurface.tsx): the
//! page under it blurred 12 px and saturated 1.14 in light mode, 16 px and 1.08 in dark, under a tint of the ground
//! at 55% in light mode and of the ink at 3% in dark, inside a hairline of the ink at 10% and 9%.

use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, prelude::*, px, rgb, App, Div, Hsla, Pixels};

/// The composer's column, max-w-3xl, and the height of an empty composer with its tray.
pub const WIDTH: Pixels = px(768.);
pub const HEIGHT: Pixels = px(120.);
const RADIUS: Pixels = px(22.);

/// Whether the docked composer blurs the page under it: on unless `DESKTOP_GPUI_COMPOSER_GLASS=off`, which a bench run sets
/// to time the frame without it.
pub fn composer_blurs() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("DESKTOP_GPUI_COMPOSER_GLASS").is_none_or(|v| v != "off"))
}

/// `--glass-ground-color` on the Mac (`.desktop-mac` in index.css): what the composer's blurred copy is laid over, since
/// a copy of the see-through page is see-through too and the sharp page would show through it.
pub fn mac_ground(dark: bool) -> Hsla {
    rgb(if dark { 0x232929 } else { 0xecedec }).into()
}

pub fn composer_glass(cx: &App) -> Div {
    let theme = cx.theme();
    let glass = div().w(WIDTH).h(HEIGHT).rounded(RADIUS).border_1();
    if theme.is_dark() {
        glass
            .bg(theme.foreground.opacity(0.03))
            .border_color(theme.foreground.opacity(0.09))
            .backdrop_blur(px(16.))
            .backdrop_saturation(1.08)
    } else {
        glass
            .bg(theme.background.opacity(0.55))
            .border_color(theme.foreground.opacity(0.1))
            .backdrop_blur(px(12.))
            .backdrop_saturation(1.14)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::Rgba;

    #[test]
    fn the_mac_grounds_are_the_web_sheets() {
        let index = std::fs::read_to_string(format!("{}/../../web/src/index.css", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let after = |block: &str| {
            let at = index.find(block).unwrap_or_else(|| panic!("no {block}"));
            let line = index[at..].lines().find(|l| l.trim_start().starts_with("--glass-ground-color:")).unwrap();
            line.split_once(':').unwrap().1.trim().trim_end_matches(';').to_owned()
        };
        let css = |dark: bool| {
            let c: Rgba = super::mac_ground(dark).into();
            format!("rgb({} {} {})", (c.r * 255.).round(), (c.g * 255.).round(), (c.b * 255.).round())
        };
        assert_eq!(css(false), after(".desktop-mac {\n  --glass-opacity"));
        assert_eq!(css(true), after(".desktop-mac.dark {\n  --glass-ground-color"));
    }
}
